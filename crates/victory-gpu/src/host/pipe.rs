//! The two-stage submission (`VICTORY_SUBMIT_PIPELINE=1`; off unless asked
//! for, and only with the relay): a submission sent ahead is prepared on the
//! thread that submits (what its lists do on the CPU, and what their shaders
//! may write, declared) and sent by a thread of its own (the copies in the
//! card's memory, the heaps' records, `ExecuteCommandLists`), so that the
//! next one is prepared while this one is sent.
//!
//! Why it is safe (handover/PIPELINE-DESIGN.md, "Step 1 findings"):
//! - The flight joins the line when it is prepared, before it is sent, with
//!   [`UNSENT`] for its fence: every look at the line (what flights may
//!   write, whether anything flies, a signal or a bottom-of-pipe write put
//!   behind it, a wait for landings) sees it in its place. The thread that
//!   lands flights waits for it to be sent.
//! - What its shaders may write is declared before the next preparing
//!   begins (`Shadow::writing` puts the ranges where refreshes leave them
//!   alone).
//! - Preparing takes [`PREPARE`], sending the submission lock: a wait for
//!   landings while preparing needs nothing the sending holds. Every other
//!   submission takes both, after the sends queued before it are done
//!   ([`sends_idle`]).
//!
//! Measured at the start (VIctory-fox, Series S, run fox-s-fetchskip): the
//! submitting thread 36 ms a frame, of it about 7.5 ms sending (copies in
//! the card's memory 5.8, the lists to the queue 1.6).

use super::*;

/// A flight's fence while it is prepared and not sent yet.
pub(super) const UNSENT: u64 = u64::MAX;

/// Whether submissions sent ahead go in two stages (`VICTORY_SUBMIT_PIPELINE=1`).
pub(super) fn on() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| {
        let on = std::env::var_os("VICTORY_SUBMIT_PIPELINE").is_some_and(|v| v == "1") && relay() && !two_queues::on();
        if on {
            tracing::info!("submissions sent ahead are prepared here and sent by a thread of their own (VICTORY_SUBMIT_PIPELINE=1)");
        }
        on
    })
}

/// Held while a submission is prepared, and by every submission that is not
/// prepared in two stages.
pub(super) static PREPARE: Mutex<()> = Mutex::new(());

/// Sends handed over and not done yet.
static PENDING: (Mutex<usize>, std::sync::Condvar) = (Mutex::new(0), std::sync::Condvar::new());

/// For a submission not prepared in two stages: [`PREPARE`], once every send
/// handed over is done, so that it goes to the card and into the line behind
/// them.
pub(super) fn serial() -> std::sync::MutexGuard<'static, ()> {
    let preparing = PREPARE.lock().unwrap_or_else(|e| e.into_inner());
    sends_idle();
    preparing
}

/// Waits until every send handed over is done.
pub(super) fn sends_idle() {
    if !on() {
        return;
    }
    let mut pending = PENDING.0.lock().unwrap_or_else(|e| e.into_inner());
    while *pending > 0 {
        pending = PENDING.1.wait(pending).unwrap_or_else(|e| e.into_inner());
    }
}

/// What a submission prepared here takes to the thread that sends it.
struct Handed {
    number: u64,
    lists: Vec<Option<ID3D12CommandList>>,
    /// Where each part after the first begins in `lists`.
    cuts: Vec<usize>,
    declared: Vec<Declared>,
    picked: crate::picked::Ranges,
    placed: Vec<(std::sync::Arc<crate::shadow::Shadow>, usize, Vec<u8>)>,
    heaps_synced: crate::descriptor::SyncedHeaps,
    /// When its preparing began and ended, and what settles took of it.
    started: std::time::Instant,
    prepared: std::time::Instant,
    settled: std::time::Duration,
}

// SAFETY: D3D12 objects (free-threaded) and host copies, which are shared
// between threads already; the lists are closed and nothing records into
// them.
unsafe impl std::marker::Send for Handed {}

/// What [`Host::prepare_late`] gives back.
struct Ready {
    handed: Handed,
    own: Option<card_steps::Taken>,
    back: Vec<Declared>,
    texture_back_ranges: Vec<(usize, usize)>,
    writes: Vec<(usize, crate::queue::Written, usize)>,
}

/// The thread that sends; started with the first submission handed over.
fn sender() -> &'static std::sync::mpsc::SyncSender<Handed> {
    static SENDER: OnceLock<std::sync::mpsc::SyncSender<Handed>> = OnceLock::new();
    SENDER.get_or_init(|| {
        let (to, from) = std::sync::mpsc::sync_channel::<Handed>(1);
        let made = std::thread::Builder::new().name("victory-send".into()).spawn(move || {
            crate::cores::keep_on_fast_cores("victory-send");
            let Some(host) = host() else {
                return;
            };
            for handed in from {
                host.send_prepared(handed);
                let mut pending = PENDING.0.lock().unwrap_or_else(|e| e.into_inner());
                *pending -= 1;
                if *pending == 0 {
                    PENDING.1.notify_all();
                }
            }
        });
        if let Err(e) = made {
            fail(format_args!("no thread to send prepared submissions (VICTORY_SUBMIT_PIPELINE=1): {e}"));
        }
        to
    })
}

impl Host {
    /// [`Host::send_late`] in two stages ([`on`]).
    pub(super) fn send_late_piped(&self, lists: Vec<crate::list::Detached>, order: Vec<usize>, parts: &[usize]) {
        let preparing = PREPARE.lock().unwrap_or_else(|e| e.into_inner());
        let ready = {
            let runs: Vec<&HostList> = order.iter().map(|&i| lists[i].host()).collect();
            self.prepare_late(&runs, parts)
        };
        let after = std::time::Instant::now();
        let told = !ready.writes.is_empty();
        let sizes = crate::memory::asked_sizes(order.iter().map(|&i| lists[i].host()));
        let Ready { handed, own, back, texture_back_ranges, writes } = ready;
        let number = handed.number;
        let most = {
            let mut line = line();
            let Line { line: items, written, lists: flying, .. } = &mut *line;
            join_ranges(written, back.iter().flat_map(|(_, ranges)| ranges.iter().copied()).chain(texture_back_ranges.iter().copied()));
            *flying += order.len();
            items.push(Late::Run(Flight { number, fence: UNSENT, lists, order, card: None, back, own, texture_back_ranges }));
            items.extend(writes.into_iter().map(|(at, what, bytes)| Late::Write(at, what, bytes)));
            *flying >= late_most()
        };
        *PENDING.0.lock().unwrap_or_else(|e| e.into_inner()) += 1;
        // In the order prepared: the next preparing waits for this hand-over.
        if sender().send(handed).is_err() {
            fail(format_args!("the thread that sends prepared submissions is gone (VICTORY_SUBMIT_PIPELINE=1)"));
        }
        drop(preparing);
        LATE_STATS[0].fetch_add(1, Ordering::Relaxed);
        // With the relay every settle is a wait for landings: it waits for
        // this flight to be sent too, and needs nothing held here.
        let mut unused = 0u64;
        if told && !late_writes() {
            self.settle(&mut unused, WHY_WRITES);
        } else if sizes {
            self.settle(&mut unused, WHY_READ);
        } else if most && relay_exact() {
            relay_wait(WHY_MOST, |line| line.lists < late_most());
        } else if most {
            self.settle(&mut unused, WHY_MOST);
        } else {
            LINE_MOVED.notify_one();
        }
        let _landing = crate::timeline::span("landed lists reset and given back");
        land();
        AFTER_SEND.fetch_add(after.elapsed().as_nanos() as u64, Ordering::Relaxed);
    }

    /// The first stage: [`Host::submit`] of a submission sent ahead, up to
    /// the copies in the card's memory, under [`PREPARE`].
    fn prepare_late(&self, runs: &[&HostList], parts: &[usize]) -> Ready {
        let called = std::time::Instant::now();
        #[cfg(feature = "diagnostics")]
        crate::copystat::submission_begins();
        let late = true;
        // With the relay a settle is a wait for landings, which uses nothing
        // of the submission lock (not held here: the sending holds it).
        let mut unused = 0u64;
        let mut stage = crate::timeline::span("submit: preparing (two stages)");
        // Upscalings these lists recorded, in submission order: the next
        // present tags the latest (frame generation).
        for run in runs {
            let id = run.take_upscale();
            if id != 0 {
                crate::streamline::submitted(id);
            }
            let id = run.take_hudless();
            if id != 0 {
                crate::streamline::hudless_submitted(id);
            }
        }
        let was_working = working(true);
        submitting(1, runs.len());
        // The round of the heaps' records asked for when the title submitted
        // these lists, if that is known (`crate::heap_marks`, 0: not).
        let heaps_mark = if late { crate::heap_marks::needed() } else { 0 };
        // Else with [`heap_sync_first`]: the heaps' records begun now, beside
        // the planning of steps done on the card and the look at flights.
        let first_heaps = (late && heaps_mark == 0 && heap_sync_first()).then(crate::descriptor::begin_sync_heaps);
        let ahead = Split::begin();
        SPLIT[11].fetch_add(called.elapsed().as_nanos() as u64, Ordering::Relaxed);
        stage.next("submit: steps given to the card");
        // Flights in the air ([`late`]): a submission that is waited for
        // settles them first, one sent ahead only if preparing it reads
        // what they write.
        // Or the reads are done on the card, behind the flights.
        let own = if late { self.card_steps(runs) } else { None };
        ahead.part(12);
        stage.next("submit: waiting for flights it reads from");
        let mut waited_for_flights = false;
        if !late {
            self.settle(&mut unused, WHY_SYNC);
        } else if own.is_none()
            && let Some(why) = late_blocked(runs)
        {
            waited_for_flights = true;
            if relay_exact() && why == WHY_READ {
                // Only until the flights that may write what is read have
                // landed: their copy-back's end, not the card's other work.
                relay_wait(why, |line| {
                    blocked_by(&line.written, runs, false).is_none() && !card_steps::copied_read(&card_steps::copies(line), runs)
                });
            } else {
                self.settle(&mut unused, why);
            }
        }
        ahead.part(13);
        // Descriptors the title copied into placed heaps directly, before
        // command lists that index those slots are submitted. (Begun first:
        // again after a wait for flights, whose landings wrote title memory,
        // as the round began here saw them; the first is not waited for.)
        // With a mark (`crate::heap_marks`): that round, unless the
        // submission waited for flights, whose landings came after it.
        let heaps_synced = match first_heaps {
            Some(first) if !waited_for_flights => {
                HEAP_SYNC_FIRST[0].fetch_add(1, Ordering::Relaxed);
                first
            }
            first => {
                if let Some(first) = first {
                    HEAP_SYNC_FIRST[1].fetch_add(1, Ordering::Relaxed);
                    std::mem::forget(first);
                }
                if heaps_mark != 0 && !waited_for_flights {
                    crate::descriptor::begin_sync_heaps_after(heaps_mark)
                } else {
                    crate::descriptor::begin_sync_heaps()
                }
            }
        };
        // That list runs first, and is one of the submission's from here on.
        let with: Vec<&HostList>;
        let runs = match &own {
            Some(own) => {
                with = std::iter::once(own.list()).chain(runs.iter().copied()).collect();
                &with[..]
            }
            None => runs,
        };
        let parts: Vec<usize> = parts.iter().map(|&n| n + usize::from(own.is_some())).collect();
        stage.next("submit: the sweep");
        let behind = late && flying();
        // Before this submission counts as started: what the sweep lets go
        // of is named by no list, and nothing runs now (behind flights the
        // sweep waits for a submission that finds none in the air: there is
        // one at every present).
        static SWEPT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        if !behind {
            if upcheck::on() {
                upcheck::at_rest(self);
            }
            sweep();
            SWEPT.store(now_ms(), Ordering::Relaxed);
        } else if late_free_present() && now_ms() > SWEPT.load(Ordering::Relaxed) + 1500 {
            // With the present sent ahead nothing else brings a moment
            // with nothing in the air: one is made for the sweep.
            self.settle(&mut unused, WHY_SYNC);
            if upcheck::on() {
                upcheck::at_rest(self);
            }
            sweep();
            SWEPT.store(now_ms(), Ordering::Relaxed);
        }
        let behind = late && flying();
        stage.next("submit: gathering what to ask about");
        let this_submission = STARTED.fetch_add(1, Ordering::AcqRel) + 1;
        // The ranges shaders write are counted for it (with the relay).
        crate::shadow::preparing(this_submission);
        let started = std::time::Instant::now();
        ahead.part(14);
        #[cfg(feature = "diagnostics")]
        let lock = (started - called).as_micros() as u64;
        #[cfg(feature = "diagnostics")]
        crate::copystat::add(crate::copystat::C::Submissions, 1);
        let mut lists: Vec<Option<ID3D12CommandList>> = Vec::with_capacity(2 * runs.len());
        // Where each run's lists start in `lists`.
        let mut firsts = Vec::with_capacity(runs.len());
        let mut batch = Batch::reused();
        let split = Split::begin();
        // The time a settle of flights takes inside this preparing: the
        // wait for the card and the copy-back have their own places on the
        // frame line (run 92 showed them twice, "preparing" 250 ms for 135).
        let mut settled = std::time::Duration::ZERO;
        // One write-watch asking for everything the submission refreshes
        // (`VICTORY_ASK_ONCE`): per copy, the span of its refreshed ranges.
        if crate::watch::once() {
            batch.copy_spans(runs);
            let Batch { spans, ranges, .. } = &mut batch;
            ranges.clear();
            ranges.extend(spans.iter().filter(|s| s.0.asked_ahead()).map(|s| (s.1, s.2)));
            SPLIT[8].fetch_add(split.so_far(), Ordering::Relaxed);
            stage.next("submit: asking the write watch");
            crate::watch::ask_once(&batch.ranges);
            split.part(0);
            stage.next("submit: title bytes under flights' writes");
            // Behind flights: the title wrote memory that a flight's shaders
            // may write too, and this submission refreshes it. The flights
            // are settled, and the asking made again: the copy-back wrote
            // title memory, which other copies of it must see.
            if upcheck::on() {
                upcheck::order(runs);
                if behind {
                    upcheck::reads(runs);
                }
            }
            let stale = if behind { batch.stale_under_flights() } else { Vec::new() };
            if !stale.is_empty() {
                let settling = std::time::Instant::now();
                self.settle_stale(&mut unused, runs, stale, &batch.ranges);
                settled += settling.elapsed();
            }
        } else if behind && !stale_under_flights(runs).is_empty() {
            let settling = std::time::Instant::now();
            self.settle(&mut unused, WHY_STALE);
            settled += settling.elapsed();
        }
        split.part(1);
        // What flights in the air may write.
        let air = line().written.clone();
        stage.next("submit: refreshes ahead of writes");
        if refresh_front() {
            batch.refresh_front(runs, &parts);
        } else if ahead_of_writes() {
            batch.refresh_ahead(runs, |list, i| own.as_ref().is_some_and(|own| own.takes(list, i)));
        }
        stage.next("submit: the lists' steps");
        for list in runs {
            list.run_deferred(&mut batch, &air, |i| own.as_ref().is_some_and(|own| own.takes(list, i)));
            firsts.push(lists.len());
            // (None unless `VICTORY_PRELUDE_UPLOADS=1`.)
            let uploads = list.prelude_uploads(&mut batch);
            if let Some(prelude) = list.prelude(&uploads) {
                lists.push(Some(prelude));
            }
            lists.push(list.list.cast().ok());
        }
        batch.fill();
        stage.next("submit: the last refreshes");
        batch.flush();
        crate::watch::ask_once_end();
        split.part(2);
        stage.next("submit: snapshots");
        // The snapshots of the ranges shaders may write, if they were left
        // for now (`crate::shadow::wide_back`): the refreshes are done, the
        // lists have not gone to the card.
        {
            #[cfg(feature = "diagnostics")]
            let snapshots = std::time::Instant::now();
            crate::shadow::take_snapshots();
            #[cfg(feature = "diagnostics")]
            crate::copystat::add(crate::copystat::C::SnapshotUs, snapshots.elapsed().as_micros() as u64);
        }
        split.part(3);
        stage.next("submit: copies in the card's memory");
        // Copies in the card's memory: what changed on their CPU side goes
        // up ahead of the lists, and a compare of what the shaders may
        // write follows them in the same submission.
        let flown = late && relay();
        if flown && self.card_results_low() {
            let settling = std::time::Instant::now();
            self.settle(&mut unused, WHY_MOST);
            settled += settling.elapsed();
        }
        // What shaders may write through views they pick: last, so that no
        // refresh of this submission leaves it alone.
        let picked = crate::picked::declare(runs);
        for (shadow, at, len) in &picked.written {
            shadow.writing(*at, *len);
        }
        let declared = declared_writes(runs);
        let back = crate::picked::with(declared.clone(), &picked.written);
        let mut texture_back_ranges = Vec::new();
        for list in runs {
            texture_back_ranges.extend(list.deferred().texture_backs.iter().map(crate::texback::Back::range));
        }
        let writes = runs.iter().flat_map(|l| l.deferred().writes.clone()).collect();
        let cuts: Vec<usize> = parts.iter().filter_map(|&n| firsts.get(n).copied()).collect();
        split.part(4);
        stage.end();
        working(was_working);
        submitting(0, 0);
        #[cfg(feature = "diagnostics")]
        crate::copystat::submission_ends(lock);
        Ready {
            handed: Handed {
                number: this_submission,
                lists,
                cuts,
                declared,
                picked,
                placed: std::mem::take(&mut batch.placed),
                heaps_synced,
                started,
                prepared: std::time::Instant::now(),
                settled,
            },
            own,
            back,
            texture_back_ranges,
            writes,
        }
    }

    /// The second stage, on the thread that sends: what goes up to copies in
    /// the card's memory, the heaps' records, the lists to the queue; then
    /// the flight in the line gets its fence.
    fn send_prepared(&self, handed: Handed) {
        let Handed { number, mut lists, cuts, declared, picked, placed, heaps_synced, started, prepared, settled } = handed;
        let mut stage = crate::timeline::span("send: copies in the card's memory");
        let mut submitted = self.submitted.lock().unwrap_or_else(|e| e.into_inner());
        let card = self.card_before(&declared, &picked, true);
        // The sends are taken: from here on the references hold what the
        // copies in the lists put.
        for (shadow, at, bytes) in placed {
            shadow.placed_copy(at, &bytes);
        }
        if let Some(check) = card.as_ref().and_then(|c| c.check.clone()) {
            lists.push(Some(check));
        }
        stage.next("send: waiting for the heaps' records");
        let heaps_began = std::time::Instant::now();
        drop(heaps_synced);
        if crate::heap_marks::on() {
            crate::heap_marks::heaps_waited(heaps_began.elapsed().as_nanos() as u64);
        }
        crate::descriptor::make_waiting();
        stage.next("send: sending");
        *submitted += 1;
        // SAFETY: closed lists; the fence is ours.
        let signalled = unsafe {
            let mut from = 0;
            for to in cuts.into_iter().chain([lists.len()]) {
                self.queue.ExecuteCommandLists(&lists[from..to]);
                from = to;
            }
            self.queue.Signal(&self.fence, *submitted)
        };
        awake::sent(&self.device);
        self.card_sent();
        if let Err(e) = signalled {
            fail(format_args!("a submission sent ahead could not be followed by a signal ({e})"));
        }
        {
            let mut line = line();
            let fence = *submitted;
            let flight = line.line.iter_mut().find_map(|item| match item {
                Late::Run(flight) if flight.number == number => Some(flight),
                _ => None,
            });
            match flight {
                Some(flight) => {
                    flight.fence = fence;
                    flight.card = card;
                }
                None => fail(format_args!("a prepared submission ({number}) was not in the line when it was sent")),
            }
            line.target = fence;
        }
        RELAY_MOVED.notify_all();
        LINE_MOVED.notify_one();
        drop(submitted);
        SUBMISSION_US[0].fetch_add((prepared - started).saturating_sub(settled).as_micros() as u64, Ordering::Relaxed);
    }
}

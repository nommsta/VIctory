//! `VICTORY_BVH_SIZES_PROBE=1` (diagnostics): when the title reads the
//! compacted sizes it asked the structure builder for.
//!
//! The title asks for a structure's compacted size (a program of one thread
//! stores it into a small buffer), then takes memory of that size and has
//! the structure copied there. Where its copies follow each other, the gap is
//! the slot's old value more often than the new one (runs rtt4, rtt5): it
//! read the slot before the size landed. This finds when it reads it: the
//! slot's 4 KiB page is made inaccessible when the size is asked for, and
//! the title's first access to it is logged (frame, value) and let
//! through. Our own code's accesses open it until the next frame (so title
//! reads in that time are missed).

use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use windows_sys::Win32::System::Memory::{
    MEM_IMAGE, MEMORY_BASIC_INFORMATION, PAGE_NOACCESS, VirtualProtect, VirtualQuery,
};

const PAGE: usize = 4096;

/// A guarded page: its slot asked for, the value then, the frame, the
/// protection it had.
struct Guarded {
    page: usize,
    slot: usize,
    was: u64,
    frame: u64,
    old: u32,
}

static GUARDED: Mutex<Vec<Guarded>> = Mutex::new(Vec::new());
static SAID: AtomicU64 = AtomicU64::new(0);

pub(crate) fn on() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("VICTORY_BVH_SIZES_PROBE").is_some_and(|v| v == "1"))
}

fn read(at: usize) -> u64 {
    // SAFETY: an 8-byte title slot whose page the caller made accessible.
    unsafe { (at as *const u64).read_volatile() }
}

/// The title asked for a compacted size into `slot` (a title address).
pub(crate) fn asked(slot: usize, frame: u64) {
    let page = slot & !(PAGE - 1);
    let mut g = GUARDED.lock().unwrap_or_else(|e| e.into_inner());
    if g.iter().any(|x| x.page == page) {
        return;
    }
    let was = read(slot);
    let mut old = 0;
    // SAFETY: a committed title page; given back as it was at the first access.
    if unsafe { VirtualProtect(page as *const _, PAGE, PAGE_NOACCESS, &mut old) } == 0 {
        return;
    }
    g.push(Guarded { page, slot, was, frame, old });
}

/// Whether code at `rip` is in an image Windows loaded (ours, the system's,
/// the driver's) rather than the title's, which our loader maps itself.
fn ours(rip: usize) -> bool {
    // SAFETY: plain data filled in by the query.
    let mut info: MEMORY_BASIC_INFORMATION = unsafe { std::mem::zeroed() };
    // SAFETY: a valid out pointer of the right size.
    let n = unsafe { VirtualQuery(rip as *const _, &mut info, size_of_val(&info)) };
    n != 0 && info.Type == MEM_IMAGE
}

/// Pages our own code touched: open until the next frame ([`frame`]).
static OPENED: Mutex<Vec<usize>> = Mutex::new(Vec::new());

/// At a new frame: pages our code opened are guarded again.
pub(crate) fn frame() {
    let opened = std::mem::take(&mut *OPENED.lock().unwrap_or_else(|e| e.into_inner()));
    let g = GUARDED.lock().unwrap_or_else(|e| e.into_inner());
    for page in opened {
        if g.iter().any(|x| x.page == page) {
            let mut old = 0;
            // SAFETY: a page guarded here.
            unsafe { VirtualProtect(page as *const _, PAGE, PAGE_NOACCESS, &mut old) };
        }
    }
}

/// An access to `at` by code at `rip` faulted: whether it was a guarded
/// page's (then the access is made again). For our own code the page is
/// opened until the next frame; the title's first access is logged and its
/// page given back.
pub(crate) fn fault(at: usize, rip: usize, write: bool) -> bool {
    let page = at & !(PAGE - 1);
    let mut g = GUARDED.lock().unwrap_or_else(|e| e.into_inner());
    let Some(i) = g.iter().position(|x| x.page == page) else {
        return false;
    };
    if ours(rip) {
        let mut old = 0;
        // SAFETY: a page guarded here, opened for one instruction.
        unsafe { VirtualProtect(page as *const _, PAGE, g[i].old, &mut old) };
        OPENED.lock().unwrap_or_else(|e| e.into_inner()).push(page);
        return true;
    }
    let x = g.swap_remove(i);
    let mut old = 0;
    // SAFETY: the page guarded above, given back its protection.
    unsafe { VirtualProtect(page as *const _, PAGE, x.old, &mut old) };
    let now = read(x.slot);
    let n = SAID.fetch_add(1, Ordering::Relaxed);
    if n < 3000 {
        tracing::info!(
            "sizes probe: slot {:#x} asked in frame {} (was {:#x}), first touched in frame {} at {:#x} ({} code, {}), slot now {:#x}",
            x.slot,
            x.frame,
            x.was,
            crate::frame::current(),
            at,
            if ours(rip) { "our" } else { "title" },
            if write { "write" } else { "read" },
            now
        );
    }
    true
}

# foxhound

A fork of [VIctory](https://github.com/Marvisak/VIctory), a compatibility layer that runs an Xbox Series title
natively on Windows from files **you** supply. foxhound is tuned for **NVIDIA RTX** GPUs: DLSS Super Resolution and
DLSS Frame Generation in place of the title's own upscaling, the Series X profile at a native 2560x1440, and fewer
stalls at scene cuts.

> **This repository contains no game files, code or assets.** You need your own legally obtained copy of the
> title. foxhound is not affiliated with or endorsed by the title's developer or publisher, nor by NVIDIA.
> See [docs/asset-policy.md](docs/asset-policy.md).

## Quick start, step by step

No AI assistant is needed. You need about 30 minutes, 10 GB of free disk space for the build and caches, and
your own copy of the title.

1. **Install the tools** (once). In a PowerShell window:

   ```powershell
   winget install Git.Git
   winget install Rustlang.Rustup
   winget install Microsoft.VisualStudio.2022.BuildTools --override "--add Microsoft.VisualStudio.Workload.VCTools --includeRecommended --passive"
   ```

   Close the window and open a new one, so that `git` and `cargo` are found.

2. **Get foxhound and build it** (10-15 minutes the first time):

   ```powershell
   git clone https://github.com/readmet3xt-lab/foxhound
   cd foxhound
   cargo build --release --locked -p victory
   ```

3. **Tell it where your copy is.** Copy `victory.example.toml` to `victory.toml` and set `game_dir` to the folder
   that holds `game_scarlett_master.exe`. Keep that folder outside the `foxhound` folder.

4. **Optional, for DLSS:** set up NVIDIA Streamline (see *Set up* below). Without it the game runs with the
   title's own upscaling.

5. **Start:**

   ```powershell
   .\scripts\play-foxhound.ps1 -Exe 'D:\Games\MyDump\game_scarlett_master.exe'
   ```

   If PowerShell answers that running scripts is disabled, allow your own scripts once with
   `Set-ExecutionPolicy -Scope CurrentUser RemoteSigned` and start again.

   The first start translates the title's shaders (2-4 minutes) before the game window opens. Later starts
   compile the pipelines that earlier sessions asked for (seconds; minutes after a new build or a new GPU
   driver). Do not close the window while it says so.

There are no downloads of a ready-made program here, and there will be none that include anything of the title.

## Where it stands

Tested on one PC: Windows 11, Ryzen 7 7800X3D, 32 GB RAM, **RTX 4070 SUPER 12 GB**, NVMe SSD; the Xbox Series
build **v1.0.2.1** of the title, Series X profile, 2560x1440 output, DLSS Quality, no Frame Generation. Frame times
are the layer's own count over one run from a new game to the first boat ride (7 minutes, 12,429 frames).

| | Measured |
|---|---|
| First boat ride (gameplay) | 33.3 ms, 30 fps, steady (the title's own limit) |
| Opening cutscenes | median 34 ms (33.7 ms in the latest run); 83% of all frames within 28-38 ms, 13% at 38-55 ms, 1% over 70 ms |
| Worst places | the first 25 s after a new game starts (one freeze of 5 s, hitches of 0.5 s) and one crowded scene (about 50 ms a frame for 15 s) |
| With `-FrameGen` | every frame the title renders is shown twice (2x presented) |
| Memory | 20 GB of RAM in use (28 GB committed: keep the page file on), 11.9 GB of VRAM in use on the 12 GB card |

Not tested yet: anything after the first boat ride. Combat, driving and the city load far more.

What is known to be wrong:

- **No ray tracing.** The title traces its indirect light, reflections and lamp shadows in its own compute
  shaders. They are off (`VICTORY_BVH_BUILDER=1` turns them on: about 3 fps and a broken picture today). An
  estimate stands in for indirect light and for reflections on opaque surfaces and glass: rooms are lit, reflections are
  blurry. Water reflects the sky only, not boats or islands.
- **Left out:** the heat-haze / in-scattering pass (it smears blocks of pixels, also on water) and the decal
  heap's defragmentation (5.6 ms of GPU a frame). `VICTORY_LEAVE_OUT` names them.
- **Hitches** while the title streams a new scene in (see the table).
- **Start of a new game:** the picture stands still for about 4 s at the first 3D frame (2,300 pipelines and
  3,500 buffers are set up in it), a few frames of another scene show after it, and the first shot is too dark
  for about 4 s.
- **Clouds:** square blocks in some clouds (reported, not traced yet).
- **Frame rate:** the title runs at 30 fps. `-Fps 60` lets frames start every 1/60 s, but the layer itself costs
  about 33 ms a frame today, so it stays near 30. 60 fps is the goal.

Fixed since the first public build (2026-11-27): dark rectangles on water beside boats, people and islands, and
dark slabs on water along the horizon and where rocks, plants or hulls meet it, and single dark tiles that
flickered along the horizon (three faults in how 64-lane compute programs are run on NVIDIA's 32-lane waves); bright sky-coloured reflections in the opening's dark rooms, also on glass and spectacles; shader
stutter after "building the shaders" (the launcher now prepares them itself).

## Questions people asked

**Is ray tracing on or off?** Off. The title does its ray tracing (indirect light, reflections, lamp shadows) in
its own compute shaders, which this layer cannot run fast enough yet. An estimate stands in for indirect light and
for reflections on opaque surfaces and glass. Water reflects the sky only; it does not mirror boats or islands yet.

**Is DLSS on?** With Streamline set up, DLSS Super Resolution (Quality) replaces the title's own upscaler by
default. Frame Generation is off unless you pass `-FrameGen`. DLSS gets the title's own motion vectors and depth:
the layer answers the title's FidelityFX upscaler call with DLSS.

**I built the shaders and it still stutters at every new scene.** Two causes, both fixed in the launcher now.
(1) A translation is only found again by a run with the same switches; the older instructions translated with
other switches than the launcher sets, so the run translated everything again while playing. (2) The GPU driver
compiles a pipeline the first time it is drawn; the launcher now compiles every pipeline earlier sessions used
before the game starts. A scene you enter for the first time ever can still stutter once.

**It gets smoother after a few launches. Why?** That is (2) above: each session writes down the pipelines it
needed, and the next start compiles them ahead.

**The game looks like slow motion.** The title advances its world by a fixed step per frame and expects 30 frames
a second. Below 30 fps everything, sound aside, runs slower than real time.

**What frame rate will I get?** One CPU thread of the layer limits the frame rate in most scenes, so a CPU with
fast cores and a large cache matters more than the GPU. Reports from the first public build (not measured by us):

| GPU | CPU | RAM | Reported |
|---|---|---|---|
| RTX 4070 Ti | Ryzen 7 9800X3D | 48 GB | "nice frames", playable |
| RTX 4070 SUPER | Ryzen 7 7800X3D | 32 GB | 30 fps (this is the PC it is tuned on) |
| RTX 4070 SUPER | Ryzen 7 5800X3D | 32 GB DDR4 | about 30 fps, frequent stutter |
| RTX 4070 SUPER | Core i5-12600KF | 32 GB DDR4 | 10-15 fps in the prologue, 30 in the opening |
| RTX 4090 | not given | 64 GB | below 30 half of the time, some stutter |
| RTX 3080 12 GB | Ryzen 7 5700X | 32 GB DDR4 | about 15 fps |
| RTX 2080 SUPER | Ryzen 5 2600 | not given | about 12 fps |
| RX 9060 XT 16 GB | Ryzen 5 7500F | 32 GB DDR5 | 10-25 fps |

**I have an AMD card.** It runs, but this fork's choices are made for NVIDIA cards. Try
[Belobug/VIctory-Belobug-AMD](https://github.com/Belobug/VIctory-Belobug-AMD), which is tuned on AMD cards.

**Does the game save?** Yes. Saves are in `%LOCALAPPDATA%\victory\data` and are kept between builds.

**Is this safe to run?** Everything here is source code that you build yourself; there is no ready-made program
to trust. The layer does not go online. It reads your copy of the title, writes its caches, logs and saves under
`%LOCALAPPDATA%\victory`, and nothing else.

**Where do I get the title?** From your own console and your own purchase. No game files are here, none are
linked, and requests for them are closed without an answer.

## What you need

- Windows 10 or 11 (x64), a CPU with AVX2, 32 GB of RAM or more, an SSD.
- An NVIDIA GeForce RTX GPU with a recent driver for DLSS (RTX 40 or later for Frame Generation, with
  hardware-accelerated GPU scheduling on). Any Direct3D 12 GPU with shader model 6.6 and mesh shaders runs the
  layer without DLSS.
- **Your own dump** of the title's Xbox Series build v1.0.2.1 (the folder with `game_scarlett_master.exe`).
  Other builds may start, but the image patches (1440p output, FidelityFX path, frame rate) check every byte
  they change and leave a differing build alone.
- [Rust](https://rustup.rs) (the toolchain is pinned in `rust-toolchain.toml`; rustup fetches it), and the
  **Visual Studio Build Tools** with "Desktop development with C++" (MSVC and a Windows SDK).
- For DLSS: NVIDIA's **Streamline SDK** (below). You download it yourself; nothing of NVIDIA's is in this
  repository.

## Build

```powershell
git clone https://github.com/readmet3xt-lab/foxhound
cd foxhound
cargo build --release --locked -p victory
```

The diagnostics build (frame timelines, captures, GPU timing) is optional:
`cargo build --locked --profile diag -p victory --features diagnostics`.

## Set up

1. **Point it at your copy.** Copy `victory.example.toml` to `victory.toml` (git-ignored) and set `game_dir` to
   your dump's folder. Keep your files **outside** this repository; the tools refuse otherwise.

   ```toml
   [paths]
   game_dir = 'D:\Games\MyDump'
   ```

2. **DLSS (optional, recommended on RTX).** Download the Streamline SDK release from
   <https://github.com/NVIDIA-RTX/Streamline/releases> (tested: `streamline-sdk-v2.14.16.zip`; it comes under
   NVIDIA's RTX SDK license, which you accept yourself), unpack it **outside** the repository, and point foxhound
   at its `bin\x64` folder:

   ```powershell
   [Environment]::SetEnvironmentVariable('VICTORY_STREAMLINE_DIR', 'D:\sdk\streamline-sdk-v2.14.16\bin\x64', 'User')
   ```

   Check it without the title: `target\release\victory.exe upscaler-check --device` (and `--present` for Frame
   Generation). Details: [docs/dlss.md](docs/dlss.md). Without Streamline the launcher runs without DLSS.

3. **Nothing else.** The launcher translates the shaders and compiles the pipelines before each start
   (`-PrepareOnly` does only that; `-NoPrepare` skips it).

## Run

```powershell
.\scripts\play-foxhound.ps1 -Exe 'D:\Games\MyDump\game_scarlett_master.exe'
```

| Option | What |
|---|---|
| *(none)* | Series X profile, 2560x1440, DLSS Quality (if Streamline is set up), all fixes on |
| `-Dlss off` / `dlaa` / `balanced` / `performance` | the title's own upscaling, or another DLSS mode |
| `-FrameGen` | DLSS Frame Generation (about +1.4 GB of VRAM) |
| `-SeriesS` | the Series S profile: 1280x720 inside, faster, softer |
| `-Res 1080to1440` / `4k` | scene at 1080p upscaled by the title / the stock 3840x2160 output |
| `-Build diag` | the diagnostics build |
| `-More @{VICTORY_X='1'}` | add or override any switch |
| `-PrepareOnly` / `-NoPrepare` | only translate shaders and compile pipelines / start without doing so |

Full screen: start the game, then run `.\scripts\fullscreen.ps1` in another window (borderless, the size of the
monitor the game is on).

Logs go to `%LOCALAPPDATA%\victory\logs`, saves to `%LOCALAPPDATA%\victory\data`. Close the game window to quit.

### Shaders and pipelines

Before the game window opens the launcher runs `victory.exe shaders` (the title's shaders translated, once per
build of foxhound and per set of switches: 2-4 minutes) and `victory.exe warm` (every pipeline that earlier sessions
asked for, compiled by the GPU driver: seconds, or minutes after a new build or driver). It runs both with the
switches of the run itself; translations made with other switches are not found by the run. Runs write down every
pipeline the title makes (`VICTORY_PIPELINE_BOOK=1`), so a scene stutters at most the first time it is ever seen.

The NVIDIA driver keeps its compiled pipelines **per program path**: a build run from another folder starts cold.

## Switches worth knowing

Every `VICTORY_...` switch is described in a comment where the code reads it. The launcher's full set is in
`scripts/play-foxhound.ps1`; these are the ones foxhound added or changed:

| Switch | Launcher | What |
|---|---|---|
| `VICTORY_DLSS`, `VICTORY_DLSS_MODE` | `quality` | DLSS Super Resolution through the title's FidelityFX dispatch |
| `VICTORY_FRAMEGEN` | off (`-FrameGen`) | DLSS Frame Generation, with the picture before the HUD tagged |
| `VICTORY_NATIVE_1440P` | on | Series X renders and outputs 2560x1440 (image patched) |
| `VICTORY_SOFT_VIEWS` | `copies` | the title's tiled-heap page mappings kept without Windows views: no freezes at scene cuts |
| `VICTORY_WC_NO_BACK`, `_MIN_KB` | `1`, `1024` | shader writes into big write-combined copies are not copied back |
| `VICTORY_AMBIENT`, `VICTORY_AMBIENT_SPECULAR` | `0.8`, `1` | estimates of indirect light and of reflections (no ray tracing) |
| `VICTORY_EARLY_DEPTH` | on | pixel programs keep the console's early depth test |
| `VICTORY_LOOPS_WAVE` | `1` (in the code) | compute programs that depend on the GPU's waves keep each wave together through their loops (`2`: every compute program, `0`: off); fixes water tiles dropped on NVIDIA |
| `VICTORY_LDS_FIRST_INIT` | on (in the code) | a thread group's first thread always sets up its shared memory (`0`: only if its lane runs, as the console); fixes dark water tiles at edges |
| `VICTORY_DSTORAGE_PRIORITY`, `VICTORY_DSTORAGE_WORKERS` | `-1`, all processors (in the code) | the threads that unpack streamed assets run below normal priority, so the frame's threads get the processors first (`0`: normal, as before) |
| `VICTORY_HELPER_PRIORITY` | `1` | the layer's threads that a frame waits for run above normal priority (`0`: normal) |
| `VICTORY_PRELUDE_UPLOADS`, `VICTORY_SHOW_NEW_LATER`, `VICTORY_TABLED_REREAD` | on | texture upload order and slot changes (flicker, device losses) |
| `VICTORY_SYNC_IN_PLACE`, `VICTORY_SPANS_QUICK`, `VICTORY_HEAP_SYNC_AT_SUBMIT` | on | cheaper descriptor-heap bookkeeping |
| `VICTORY_PORT_CONSTANTS` | off | the title's constants heap read in place: less CPU, but on a 12 GB card the GPU then waits (slower overall) |
| `VICTORY_FRAME_INTERVAL_MS` | off | experimental: the title's own 30 fps setting patched to a higher rate |
| `VICTORY_BVH_BUILDER` | off | the title's ray tracing (slow, broken) |

## How it works, and the findings

- [docs/architecture.md](docs/architecture.md): the layer's parts.
- [docs/findings.md](docs/findings.md): measurements behind the choices.
- [docs/dlss.md](docs/dlss.md): DLSS and Frame Generation.
- [docs/fork-ray-tracing.md](docs/fork-ray-tracing.md): the title's ray tracing and the light estimate.
- [docs/replay.md](docs/replay.md): recording a frame and replaying it.
- [docs/asset-policy.md](docs/asset-policy.md): what may and may not be in this repository.

## Contributing

Issues and pull requests are welcome. Never attach or commit anything from the game: files, dumps, shader dumps
or extracted data. Describe faults technically. `cargo xtask policy` (also the pre-commit hook in `.githooks/`)
checks the asset policy.

## Credits

A tree of other people's work with more on top. This repository starts from one commit; the history of the work
before it is in the projects named here, whose authors wrote most of this code.

- **[VIctory](https://github.com/Marvisak/VIctory)** by Marvisak and its contributors: the loader, the runtime
  services, the Direct3D 12 back end and the shader recompiler.
- **[fxceless/victory-fork](https://github.com/fxceless/victory-fork)** and the people whose pull requests it carries.
- **[avatarmanz/VIctory-7900XTX](https://github.com/avatarmanz/VIctory-7900XTX)**: the submission line, write
  guards, relay and much of the speed work this fork starts from.
- **[codec-d/cool_fox](https://github.com/codec-d/cool_fox)**: the base this fork was taken from.
- **[DexNom/VIctory](https://github.com/DexNom/VIctory)**: native 1440p output, the FidelityFX path on Series X,
  ray tracing fixes.
- **[Belobug/VIctory-Belobug-AMD](https://github.com/Belobug/VIctory-Belobug-AMD)**: the light estimate, renderer
  ports (constants heap in place, heap records), texture upload order, the frame rate switch; ported commits name
  their source.
- **[FQY55kai/ZJOU-VICTORY](https://github.com/FQY55kai/ZJOU-VICTORY)**, **[myokex/victory-amd](https://github.com/myokex/victory-amd)**
  (with Spexivus, joroyt and tightfall), Joshua Odom, **Geckomacabre** and **cxy0072019-sketch**.
- **[XCL](https://github.com/ivall/xcl)** (GPL-2.0-only) was read for how the same title behaves under another
  layer; no code of it is here.
- NVIDIA Streamline, DLSS and Reflex are NVIDIA's; they are loaded at run time from a folder you supply.
- Much of this fork's code was written with [Claude Code](https://claude.com/claude-code).

## License

GPL-3.0-or-later, as VIctory. See [LICENSE](LICENSE); the one file that is not covered is named in
[docs/asset-policy.md](docs/asset-policy.md).

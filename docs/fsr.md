# FSR through the PC FidelityFX SDK

`VICTORY_UPSCALER=fsr` makes the host do the title's FidelityFX upscaling with
AMD's PC FidelityFX SDK, in place of the title's own Xbox FSR. It works on every
GPU vendor, on Series S and Series X, at any `VICTORY_UPSCALE_MODE`. The DLSS
path ([dlss.md](dlss.md)) is the other choice. The output size is the size the
title outputs: `VICTORY_OUTPUT=WxH` on Series X (the launcher passes the desktop's
size), 1920x1080 on Series S.

Nothing AMD ships is in this repository. The SDK folder is read at run time,
and the loader DLL must have a valid Authenticode signature.

## Switches

| Variable | Values | Default | What |
|---|---|---|---|
| `VICTORY_UPSCALER` | `auto`, `dlss`, `fsr`, `off` | `auto` | `auto`: DLSS, or FSR where Streamline gives no DLSS; on Series S at 1080p, the title's own FSR4 instead (the only case it can do). `off`: the title's own upscaling always |
| `VICTORY_UPSCALE_MODE` | `native`, `dlaa`, `quality`, `balanced`, `performance`, `ultraperformance` | `quality` | the scene's size: 1, 2/3, 0.58, 1/2, 1/3 of the output |
| `VICTORY_FFX_DIR` | folder | `C:\dev\sdk\fidelityfx-sdk-v2.4.0\bin` | holds `amd_fidelityfx_loader_dx12.dll` and the upscaler DLLs beside it |
| `VICTORY_DLSS_JITTER`, `VICTORY_DLSS_MV` | `x,y` | `1,1` | multipliers for the jitter and the motion-vector scale (as for DLSS) |
| `VICTORY_FSR_PATH` | `1` | off | the title's own Xbox FSR instead (no host upscaler); keep it for comparison |

`VICTORY_UPSCALER=fsr` with `VICTORY_DLSS=1` is an error.

## How it works

- The app patches the title to its FidelityFX path at the mode's scale, as for
  DLSS. The title's `ffxDispatch` of an upscale is served by
  `victory_hle::modules::fidelityfx`, which gives it to `victory_gpu::ffx`.
- `ffx` records the SDK's dispatch into the title's list at that place, from
  the same colour, depth, motion vectors, exposure (not with auto-exposure),
  and reactive mask into the same output. The transparency mask is not used.
- The textures are given to FSR in the D3D12 states they are in. The API
  states are converted back from the host states.
- The context is made at the first upscaling on the host device, with the
  output as its largest render and upscale size, and with dynamic resolution
  on, so that the title's render size follows its mode. It is made again if the
  output size or the flags change. It is destroyed with the title's own
  context.
- The versions the loader offers are queried and logged, and the newest is
  asked for. On NVIDIA this is FSR 3.1 (FSR 4 is for RDNA only); on RDNA4 it is
  FSR 4. This is a run-time choice, not a build switch.
- Frame generation (`VICTORY_FRAMEGEN=1`) stays Streamline-only. With FSR, FSR
  upscales and the frame generation path only notes depth and motion vectors
  for the present.

## What to look for in the log

- At start: `FSR: the FidelityFX SDK loader is loaded` and the `upscaling` line
  of the app (`FSR on (PC FidelityFX SDK, ...)`).
- At the first upscaling: `FSR: the upscaler versions the SDK offers`
  (`offered`, `picked`), `FSR: the upscale context was made` (`version`), and
  `FSR: first upscaling recorded in place of the title's FidelityFX` (every
  parameter).
- Failures: `FSR: the upscaling was not recorded: ...` (the first eight).
- Missing SDK, signature or DLL: `FSR: ...; FSR stays off, the title's own
  upscaling runs`.

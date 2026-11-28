//! An SDR display of another size (`VICTORY_OUTPUT=WxH`; the old switch
//! `VICTORY_NATIVE_1440P=1` is 2560x1440) for the supported retail image
//! (the game, 1.0.2.1): stock, the title presents at 3840x2160 on Series X
//! (its scene drawn at 2560x1440 and scaled up by its own temporal pass) and
//! at 1920x1080 on Series S. The image's SDR display mode of the profile,
//! the scanout reservation, the temporal scene scale and the Settings
//! viewport load are patched before it is mapped: the title then makes an
//! output of the size asked.
//!
//! The title lists five SDR modes (720p, 936p, 1080p, 1440p, 4K, at
//! [`MODES`]) on both profiles and takes one by profile, not by the display
//! mode it is told (run s1440, 2026-11-28: Series S told 2560x1440 still
//! made a 1080p output). Series X takes the largest entry (4K), which is
//! written over. Series S takes the largest up to 1080p, so only its stock
//! 1920x1080 output is accepted (see [`prepare`]).
//!
//! Ported from DexNom's fork (DexNom/VIctory, `apps/victory/src/native_1440p.rs`,
//! GPL-3.0-or-later), its native 1440p part only. Every field is checked
//! against the stock bytes first: another build is refused, not patched.

use anyhow::{Context, Result, ensure};
use victory_formats::texture::{Dimension, Swizzle, Texture, TextureDesc};
use victory_hle::streamline::Mode;
use victory_loader::PreparedImage;

use crate::Console;

const RETAIL_BUILD: &str = "b99f876b74d4a77f03af5f878223b27bd647b5c35a79d5decd9f716504239e93";
/// The SDR mode list: width, height, refresh, 1 (16 bytes each).
const MODES: u32 = 0x0615_c0c0;
const STOCK_MODES: [[u32; 2]; 5] = [[1280, 720], [1664, 936], [1920, 1080], [2560, 1440], [3840, 2160]];
const SCANOUT_RESERVATION: u32 = 0x0247_ad4a;
const TEMPORAL_SCALE: u32 = 0x028f_1e7e;
const UI_VIEWPORT_LOAD: u32 = 0x0580_48d4;

/// The SDR mode entry `console` takes, and its stock size.
fn sdr_mode(console: Console) -> (u32, [u32; 2]) {
    match console {
        Console::SeriesX => (MODES + 4 * 16, [3840, 2160]),
        Console::SeriesS => (MODES + 2 * 16, [1920, 1080]),
    }
}

/// Patches `image` for an `output` display on `console`, the scene drawn at
/// `scene` by the title's own temporal pass (an upscaler sets it instead).
pub(super) fn prepare(image: &mut PreparedImage, console: Console, output: [u32; 2], scene: [u32; 2]) -> Result<()> {
    ensure!(
        image.original_hash.to_hex().as_str() == RETAIL_BUILD,
        "another output size is only known for the retail build {RETAIL_BUILD}"
    );
    let [width, height] = output;
    let (sdr_mode, stock) = sdr_mode(console);
    // Series S takes the largest mode up to 1080p: with its 1080p entry
    // written over it takes the 936p one, and the scanout pool's check
    // fails (title+0x247af2d, 0x540000 left over at 720p, 1440p and 4K
    // alike; runs s-out*, 2026-11-28). Where that limit is set is not found.
    ensure!(
        matches!(console, Console::SeriesX) || output == stock,
        "a {width}x{height} output is not known on Series S (it outputs 1920x1080 only); use Series X for another size"
    );
    // Series X takes the largest mode: every one larger than the output in
    // either side becomes the output (the 4K one always), so that the largest
    // is the output (run x-out1080: with the 4K one alone written over, a
    // 1080p output took the 1440p mode and overran its scanout pool).
    for (i, size) in STOCK_MODES.iter().enumerate() {
        let entry = MODES + 16 * i as u32;
        let mode = field::<16>(image, entry)?;
        let stock_mode: Vec<u8> = [size[0], size[1], 60, 1].iter().flat_map(|v| v.to_le_bytes()).collect();
        ensure!(mode[..] == stock_mode[..], "unexpected SDR mode {}x{}", size[0], size[1]);
        let larger = size[0] > width || size[1] > height;
        if entry == sdr_mode || (matches!(console, Console::SeriesX) && larger) {
            mode[..4].copy_from_slice(&width.to_le_bytes());
            mode[4..8].copy_from_slice(&height.to_le_bytes());
        }
    }

    // Two 64 KiB-aligned, tiled RGBA8 scanout buffers. The reservation is
    // rounded to 2 MiB, matching the title's allocation check.
    let texture = Texture::new(&TextureDesc {
        dimension: Dimension::Tex2d,
        width,
        height,
        depth_or_array: 1,
        mips: 1,
        format: 28,
        samples: 1,
        swizzle: Swizzle::SW_64KB_D_X,
    })
    .map_err(|error| anyhow::anyhow!("{width}x{height} scanout layout: {error:?}"))?;
    let pool_size =
        u32::try_from((texture.size.next_multiple_of(64 * 1024) * 2).next_multiple_of(2 << 20))?;
    let reservation = field::<5>(image, SCANOUT_RESERVATION)?;
    ensure!(*reservation == [0xb9, 0, 0, 0, 4], "unexpected scanout reservation");
    reservation[1..].copy_from_slice(&pool_size.to_le_bytes());

    let scale = field::<4>(image, TEMPORAL_SCALE)?;
    ensure!(*scale == (2.0f32 / 3.0).to_le_bytes(), "unexpected temporal scene scale");
    *scale = (scene[0] as f32 / width as f32).to_le_bytes();

    // Settings lays out its preview panels with a viewport setter of its
    // own that loads the legacy UI size (3840x2160) even when a smaller UI
    // surface was made: it reads the selected SDR mode instead.
    let viewport = field::<8>(image, UI_VIEWPORT_LOAD)?;
    ensure!(
        *viewport == [0xc5, 0xfb, 0x10, 0x0d, 0x10, 0xf0, 0x13, 0x05],
        "unexpected Settings viewport size load"
    );
    let displacement = (sdr_mode as i32) - (UI_VIEWPORT_LOAD as i32 + 8);
    viewport[4..].copy_from_slice(&displacement.to_le_bytes());
    Ok(())
}

/// `VICTORY_OUTPUT=WxH`, else 2560x1440 for the old switches
/// (`VICTORY_NATIVE_1440P=1`, `VICTORY_RENDER_1080P=1`); none: the stock output.
pub(super) fn asked_output() -> Result<Option<[u32; 2]>> {
    if let Ok(value) = std::env::var("VICTORY_OUTPUT") {
        let size = value
            .trim()
            .split_once(['x', 'X'])
            .and_then(|(w, h)| Some([w.trim().parse::<u32>().ok()?, h.trim().parse::<u32>().ok()?]))
            .filter(|[w, h]| (640..=7680).contains(w) && (360..=4320).contains(h))
            .with_context(|| format!("VICTORY_OUTPUT={value}: not WxH between 640x360 and 7680x4320"))?;
        return Ok(Some(size));
    }
    let on = |name: &str| std::env::var(name).as_deref() == Ok("1");
    Ok((on("VICTORY_NATIVE_1440P") || on("VICTORY_RENDER_1080P")).then_some([2560, 1440]))
}

/// The title's renderer selection: `mov eax, 6; cmovne eax, ecx` (the
/// title's own temporal pass, or on Series S with the upscaler there the
/// FidelityFX path, 7).
const RENDERER_SELECTION: u32 = 0x007e_9421;
const STOCK_RENDERER_SELECTION: [u8; 8] = [0xb8, 6, 0, 0, 0, 0x0f, 0x45, 0xc1];
/// The FidelityFX path's render size: the console check's result (Series S)
/// is noted as the FidelityFX quality mode and picks the scaled branch
/// (`movzx eax, al; mov [r14+0x90], eax; test al, al; jne`), whose scale
/// multiplies the output size.
const UPSCALER_SIZING: u32 = 0x028e_f69a;
const STOCK_UPSCALER_SIZING: [u8; 14] = [0x0f, 0xb6, 0xc0, 0x41, 0x89, 0x86, 0x90, 0, 0, 0, 0x84, 0xc0, 0x75, 0x0a];
/// The native branch's load of 1.0, and the scaled branch's of the scale.
const NATIVE_LOAD: u32 = 0x028e_f6a8;
const STOCK_NATIVE_LOAD: [u8; 10] = [0xc5, 0xfa, 0x10, 0x05, 0x28, 0x78, 0x6d, 0x03, 0xeb, 0x08];
const SCALED_LOAD: u32 = 0x028e_f6b2;
const STOCK_SCALED_LOAD: [u8; 8] = [0xc5, 0xfa, 0x10, 0x05, 0xa2, 0xe2, 0x86, 0x03];
/// The scale the scaled branch loads (shared with other code: not written).
const UPSCALER_SCALE: u32 = 0x0615_d95c;

/// Makes the title render through its FidelityFX upscaler path (renderer 7,
/// as Series S does) with the scene at `mode`'s scale of the output, so that
/// DLSS can take its upscale dispatches over (`victory_hle::modules::fidelityfx`).
/// On Series X the stock title uses its own temporal pass instead and never
/// loads its upscaler. The FidelityFX quality mode the title notes is
/// `mode`'s; the temporal scene scale (the title's own pass, and
/// `prepare`'s 1440p patch) is set to the same scale, as the stock Series X
/// image pairs them (2/3 each). Every patched instruction is checked against
/// the stock bytes first; nothing is written if one differs.
///
/// From DexNom's fork (DexNom/VIctory, `apps/victory/src/native_1440p.rs`,
/// `prepare_fsr_4k`, GPL-3.0-or-later), there for a 4K output; kept here
/// with the 1440p one.
pub(super) fn prepare_upscaler(image: &mut PreparedImage, mode: Mode) -> Result<()> {
    ensure!(
        image.original_hash.to_hex().as_str() == RETAIL_BUILD,
        "the upscaler path is only known for the retail build {RETAIL_BUILD}"
    );
    ensure!(*field::<8>(image, RENDERER_SELECTION)? == STOCK_RENDERER_SELECTION, "unexpected renderer selection");
    ensure!(*field::<14>(image, UPSCALER_SIZING)? == STOCK_UPSCALER_SIZING, "unexpected upscaler render size selection");
    ensure!(*field::<10>(image, NATIVE_LOAD)? == STOCK_NATIVE_LOAD, "unexpected upscaler native scale load");
    ensure!(*field::<8>(image, SCALED_LOAD)? == STOCK_SCALED_LOAD, "unexpected upscaler scaled load");
    ensure!(*field::<4>(image, UPSCALER_SCALE)? == (2.0f32 / 3.0).to_le_bytes(), "unexpected upscaler scale");
    let temporal = f32::from_le_bytes(*field::<4>(image, TEMPORAL_SCALE)?);
    ensure!(
        temporal == 2.0 / 3.0 || temporal == 1.0,
        "unexpected temporal scene scale {temporal} (stock 2/3, or 1 with the 1440p output)"
    );
    // Renderer 7 every time it is asked (the console check has run).
    *field::<8>(image, RENDERER_SELECTION)? = [0xb8, 7, 0, 0, 0, 0x90, 0x90, 0x90];
    // `mov eax, mode; mov [r14+0x90], eax; jmp scaled`.
    *field::<14>(image, UPSCALER_SIZING)? = [0xb8, mode.ffx_mode(), 0, 0, 0, 0x41, 0x89, 0x86, 0x90, 0, 0, 0, 0xeb, 0x0a];
    if mode != Mode::Quality {
        // The jump leaves the native branch unreached: its load's operand
        // becomes a literal of ours, which the scaled branch loads.
        let literal = NATIVE_LOAD + 4;
        *field::<4>(image, literal)? = mode.scale().to_le_bytes();
        let displacement = literal as i32 - (SCALED_LOAD as i32 + 8);
        *field::<4>(image, SCALED_LOAD + 4)? = displacement.to_le_bytes();
    }
    *field::<4>(image, TEMPORAL_SCALE)? = mode.scale().to_le_bytes();
    Ok(())
}

fn field<const N: usize>(image: &mut PreparedImage, rva: u32) -> Result<&mut [u8; N]> {
    let offset = image.info.rva_to_file_offset(rva).context("display profile field missing")? as usize;
    image
        .bytes
        .get_mut(offset..offset + N)
        .context("incomplete display profile field")?
        .try_into()
        .context("invalid display profile field length")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// With the retail executable named by `VICTORY_DISPLAY_PROFILE_TEST_IMAGE`
    /// (not part of the repository): the four fields hold what the output
    /// needs, for each profile and some sizes and scenes; without it,
    /// nothing is checked.
    #[test]
    fn patches_the_retail_image_for_another_output() {
        let Some(path) = std::env::var_os("VICTORY_DISPLAY_PROFILE_TEST_IMAGE") else {
            return;
        };
        let bytes = std::fs::read(path).unwrap();
        let cases = [([2560, 1440], [2560, 1440]), ([2560, 1440], [1920, 1080]), ([1920, 1080], [1920, 1080]), ([3440, 1440], [3440, 1440])];
        for console in [Console::SeriesX, Console::SeriesS] {
            let (sdr_mode, stock) = sdr_mode(console);
            for (output, scene) in cases {
                if matches!(console, Console::SeriesS) && output != stock {
                    let mut image = victory_loader::prepare(&bytes).unwrap();
                    assert!(prepare(&mut image, console, output, scene).is_err());
                    continue;
                }
                let mut image = victory_loader::prepare(&bytes).unwrap();
                prepare(&mut image, console, output, scene).unwrap();
                let mode = *field::<16>(&mut image, sdr_mode).unwrap();
                assert_eq!(mode[..4], output[0].to_le_bytes());
                assert_eq!(mode[4..8], output[1].to_le_bytes());
                if matches!(console, Console::SeriesX) {
                    // The output is the largest mode.
                    for i in 0..5 {
                        let m = *field::<16>(&mut image, MODES + 16 * i).unwrap();
                        let [w, h] = [0, 4].map(|at| u32::from_le_bytes(m[at..at + 4].try_into().unwrap()));
                        assert!(w <= output[0] && h <= output[1], "{w}x{h} over {output:?}");
                    }
                }
                let scale = f32::from_le_bytes(*field::<4>(&mut image, TEMPORAL_SCALE).unwrap());
                assert_eq!((output[0] as f32 * scale).round() as u32, scene[0]);
                let load = *field::<8>(&mut image, UI_VIEWPORT_LOAD).unwrap();
                let disp = i32::from_le_bytes(load[4..].try_into().unwrap());
                assert_eq!(UI_VIEWPORT_LOAD.wrapping_add(8).wrapping_add_signed(disp), sdr_mode);
                // Patched once, the image is no longer the stock one: refused.
                assert!(prepare(&mut image, console, output, scene).is_err());
            }
        }
    }

    /// With the retail executable (as above): the upscaler path at each
    /// scale, alone (4K output) and after the 1440p patch; the scaled
    /// branch then loads the mode's scale and the noted mode is its
    /// FidelityFX mode.
    #[test]
    fn patches_the_upscaler_path() {
        let Some(path) = std::env::var_os("VICTORY_DISPLAY_PROFILE_TEST_IMAGE") else {
            return;
        };
        let bytes = std::fs::read(path).unwrap();
        for native in [false, true] {
            for mode in [Mode::Dlaa, Mode::Quality, Mode::Balanced, Mode::Performance, Mode::UltraPerformance] {
                let mut image = victory_loader::prepare(&bytes).unwrap();
                if native {
                    prepare(&mut image, Console::SeriesX, [2560, 1440], [2560, 1440]).unwrap();
                }
                prepare_upscaler(&mut image, mode).unwrap();
                assert_eq!(*field::<8>(&mut image, RENDERER_SELECTION).unwrap(), [0xb8, 7, 0, 0, 0, 0x90, 0x90, 0x90]);
                let sizing = *field::<14>(&mut image, UPSCALER_SIZING).unwrap();
                assert_eq!(sizing[1], mode.ffx_mode());
                assert_eq!(sizing[12..], [0xeb, 0x0a]);
                // The jump lands on the scaled load.
                assert_eq!(UPSCALER_SIZING + 14 + 0x0a, SCALED_LOAD);
                let disp = i32::from_le_bytes(*field::<4>(&mut image, SCALED_LOAD + 4).unwrap());
                let loaded = (SCALED_LOAD + 8).wrapping_add_signed(disp);
                assert_eq!(f32::from_le_bytes(*field::<4>(&mut image, loaded).unwrap()), mode.scale());
                assert_eq!(f32::from_le_bytes(*field::<4>(&mut image, TEMPORAL_SCALE).unwrap()), mode.scale());
                // The shared constant stays as it was.
                assert_eq!(*field::<4>(&mut image, UPSCALER_SCALE).unwrap(), (2.0f32 / 3.0).to_le_bytes());
                // Patched once, refused.
                assert!(prepare_upscaler(&mut image, mode).is_err());
            }
        }
    }
}

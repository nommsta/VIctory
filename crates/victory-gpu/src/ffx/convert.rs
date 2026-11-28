//! What does not need the library: FidelityFX's reading of a D3D12 texture
//! (format, usage, state), the creation flags, and the choice of a version.

use windows::Win32::Graphics::Direct3D12::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::core::Interface;

use super::abi;
use crate::streamline::plan;

/// `ffxApiGetSurfaceFormatDX12` of the SDK: the API's surface format of a
/// DXGI format (0, unknown, for the rest).
pub(super) fn surface_format(format: DXGI_FORMAT) -> u32 {
    match format {
        DXGI_FORMAT_R32G32B32A32_TYPELESS => 1,
        DXGI_FORMAT_R32G32B32A32_UINT => 2,
        DXGI_FORMAT_R32G32B32A32_FLOAT => 3,
        DXGI_FORMAT_R16G16B16A16_FLOAT => 4,
        DXGI_FORMAT_R32G32_FLOAT => 6,
        DXGI_FORMAT_R8_UINT
        | DXGI_FORMAT_X32_TYPELESS_G8X24_UINT
        | DXGI_FORMAT_X24_TYPELESS_G8_UINT => 7,
        DXGI_FORMAT_R32_UINT
        | DXGI_FORMAT_R24G8_TYPELESS
        | DXGI_FORMAT_D24_UNORM_S8_UINT
        | DXGI_FORMAT_R24_UNORM_X8_TYPELESS => 8,
        DXGI_FORMAT_R8G8B8A8_TYPELESS => 9,
        DXGI_FORMAT_R8G8B8A8_UNORM => 10,
        DXGI_FORMAT_R8G8B8A8_SNORM => 11,
        DXGI_FORMAT_R8G8B8A8_UNORM_SRGB => 12,
        DXGI_FORMAT_B8G8R8A8_TYPELESS => 13,
        DXGI_FORMAT_B8G8R8A8_UNORM => 14,
        DXGI_FORMAT_B8G8R8A8_UNORM_SRGB => 15,
        DXGI_FORMAT_R11G11B10_FLOAT => 16,
        DXGI_FORMAT_R10G10B10A2_UNORM => 17,
        DXGI_FORMAT_R16G16_FLOAT => 18,
        DXGI_FORMAT_R16G16_UINT => 19,
        DXGI_FORMAT_R16_FLOAT => 21,
        DXGI_FORMAT_R16_UINT => 22,
        DXGI_FORMAT_R16_UNORM | DXGI_FORMAT_D16_UNORM => 23,
        DXGI_FORMAT_R16_SNORM => 24,
        DXGI_FORMAT_R8_UNORM | DXGI_FORMAT_A8_UNORM => 25,
        DXGI_FORMAT_R8G8_UNORM => 26,
        DXGI_FORMAT_R8G8_UINT => 27,
        DXGI_FORMAT_R32_FLOAT
        | DXGI_FORMAT_D32_FLOAT
        | DXGI_FORMAT_R32G8X24_TYPELESS
        | DXGI_FORMAT_D32_FLOAT_S8X24_UINT
        | DXGI_FORMAT_R32_FLOAT_X8X24_TYPELESS => 28,
        DXGI_FORMAT_R9G9B9E5_SHAREDEXP => 29,
        DXGI_FORMAT_R16G16B16A16_TYPELESS => 30,
        DXGI_FORMAT_R32G32_TYPELESS => 31,
        DXGI_FORMAT_R10G10B10A2_TYPELESS => 32,
        DXGI_FORMAT_R16G16_TYPELESS => 33,
        DXGI_FORMAT_R16_TYPELESS => 34,
        DXGI_FORMAT_R8_TYPELESS => 35,
        DXGI_FORMAT_R8G8_TYPELESS => 36,
        DXGI_FORMAT_R32_TYPELESS => 37,
        DXGI_FORMAT_R8_SNORM => 39,
        _ => 0,
    }
}

/// `ffxApiGetResourceDX12` of the SDK for a texture in the FidelityFX
/// `state`.
pub(super) fn describe(texture: &ID3D12Resource, state: u32) -> abi::Resource {
    // SAFETY: a live resource.
    let desc = unsafe { texture.GetDesc() };
    let depth = matches!(desc.Format, DXGI_FORMAT_D16_UNORM | DXGI_FORMAT_D32_FLOAT);
    let depth_stencil = matches!(
        desc.Format,
        DXGI_FORMAT_D24_UNORM_S8_UINT | DXGI_FORMAT_D32_FLOAT_S8X24_UINT
    );
    let mut usage = if depth {
        abi::USAGE_DEPTHTARGET
    } else if depth_stencil {
        abi::USAGE_DEPTHTARGET | abi::USAGE_STENCILTARGET
    } else {
        0
    };
    if desc
        .Flags
        .contains(D3D12_RESOURCE_FLAG_ALLOW_UNORDERED_ACCESS)
    {
        usage |= abi::USAGE_UAV;
    }
    let kind = match desc.Dimension {
        D3D12_RESOURCE_DIMENSION_TEXTURE1D => abi::TYPE_TEXTURE1D,
        D3D12_RESOURCE_DIMENSION_TEXTURE2D if desc.DepthOrArraySize == 6 => abi::TYPE_TEXTURE_CUBE,
        D3D12_RESOURCE_DIMENSION_TEXTURE2D => abi::TYPE_TEXTURE2D,
        D3D12_RESOURCE_DIMENSION_TEXTURE3D => abi::TYPE_TEXTURE3D,
        _ => 0,
    };
    abi::Resource {
        resource: texture.as_raw(),
        description: abi::ResourceDescription {
            kind,
            format: surface_format(desc.Format),
            width: desc.Width as u32,
            height: desc.Height,
            depth: u32::from(desc.DepthOrArraySize),
            mip_count: u32::from(desc.MipLevels),
            flags: 0,
            usage,
        },
        state,
    }
}

/// The FidelityFX state of a texture that is in the D3D12 resource `state`
/// (the inverse of `plan::host_state`); `None` for one a shader does not
/// read or write.
pub(super) fn ffx_state(state: u32) -> Option<u32> {
    Some(match state {
        0 => abi::STATE_COMMON,
        0x8 => abi::STATE_UNORDERED_ACCESS,
        0x40 => abi::STATE_COMPUTE_READ,
        0x80 => abi::STATE_PIXEL_READ,
        0xc0 => abi::STATE_PIXEL_COMPUTE_READ,
        0x800 => abi::STATE_COPY_SRC,
        0x400 => abi::STATE_COPY_DEST,
        0x4 => abi::STATE_RENDER_TARGET,
        0x10 => abi::STATE_DEPTH_ATTACHMENT,
        _ => return None,
    })
}

/// The PC API's creation flags for the title's (the Xbox API's: the same
/// bits for the first six). Dynamic resolution is on: the context is made
/// for the output as its largest render size, and the title renders at the
/// scale of its mode.
pub(super) fn create_flags(title: u32) -> u32 {
    const KNOWN: u32 = plan::FFX_HIGH_DYNAMIC_RANGE
        | plan::FFX_DISPLAY_RESOLUTION_MOTION_VECTORS
        | plan::FFX_MOTION_VECTORS_JITTER_CANCELLATION
        | plan::FFX_DEPTH_INVERTED
        | plan::FFX_DEPTH_INFINITE
        | plan::FFX_AUTO_EXPOSURE;
    (title & KNOWN) | abi::ENABLE_DYNAMIC_RESOLUTION
}

/// A version name ("3.1.6") as numbers.
fn numbers(name: &str) -> Option<Vec<u32>> {
    let digits = name.trim().trim_start_matches(['v', 'V']);
    digits
        .split('.')
        .map(|part| {
            part.split(|c: char| !c.is_ascii_digit())
                .next()?
                .parse()
                .ok()
        })
        .collect()
}

/// The index of the newest of the versions named (by their numbers; the
/// first if none reads as numbers).
pub(super) fn newest(names: &[String]) -> Option<usize> {
    let best = names
        .iter()
        .enumerate()
        .filter_map(|(i, n)| numbers(n).map(|v| (v, std::cmp::Reverse(i))))
        .max();
    best.map(|(_, i)| i.0).or((!names.is_empty()).then_some(0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn states_convert_back_to_the_api() {
        for ffx in [1, 2, 4, 8, 0xc, 0x10, 0x20, 0x100, 0x200] {
            let host = plan::host_state(ffx).unwrap();
            assert_eq!(ffx_state(host), Some(ffx), "state {ffx:#x}");
        }
        assert_eq!(
            ffx_state(plan::input_state(0xc)),
            Some(abi::STATE_PIXEL_COMPUTE_READ)
        );
        assert_eq!(
            ffx_state(plan::output_state(0x2)),
            Some(abi::STATE_UNORDERED_ACCESS)
        );
        assert_eq!(ffx_state(0x3), None);
    }

    #[test]
    fn flags_map_to_the_pc_api() {
        let title = plan::FFX_DEPTH_INVERTED
            | plan::FFX_AUTO_EXPOSURE
            | plan::FFX_HIGH_DYNAMIC_RANGE
            | 0x8000;
        assert_eq!(
            create_flags(title),
            abi::ENABLE_DEPTH_INVERTED
                | abi::ENABLE_AUTO_EXPOSURE
                | abi::ENABLE_HIGH_DYNAMIC_RANGE
                | abi::ENABLE_DYNAMIC_RESOLUTION
        );
        assert_eq!(
            plan::FFX_DISPLAY_RESOLUTION_MOTION_VECTORS,
            abi::ENABLE_DISPLAY_RESOLUTION_MOTION_VECTORS
        );
        assert_eq!(
            plan::FFX_MOTION_VECTORS_JITTER_CANCELLATION,
            abi::ENABLE_MOTION_VECTORS_JITTER_CANCELLATION
        );
        assert_eq!(plan::FFX_DEPTH_INFINITE, abi::ENABLE_DEPTH_INFINITE);
        assert_eq!(create_flags(0), abi::ENABLE_DYNAMIC_RESOLUTION);
    }

    #[test]
    fn formats_follow_the_sdk() {
        assert_eq!(surface_format(DXGI_FORMAT_R16G16B16A16_FLOAT), 4);
        assert_eq!(surface_format(DXGI_FORMAT_D32_FLOAT), 28);
        assert_eq!(surface_format(DXGI_FORMAT_R16G16_FLOAT), 18);
        assert_eq!(surface_format(DXGI_FORMAT_R8_UNORM), 25);
        assert_eq!(surface_format(DXGI_FORMAT_UNKNOWN), 0);
    }

    #[test]
    fn the_newest_version_is_picked_by_number() {
        let names = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(newest(&names(&["3.1.6", "4.1.2", "3.1.10"])), Some(1));
        assert_eq!(newest(&names(&["3.1.6", "3.1.10"])), Some(1));
        assert_eq!(newest(&names(&["FSR 4", "FSR 3"])), Some(0));
        assert_eq!(newest(&names(&["3.1.6", "3.1.6"])), Some(0));
        assert_eq!(newest(&[]), None);
    }
}

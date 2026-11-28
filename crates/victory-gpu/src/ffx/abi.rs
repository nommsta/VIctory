//! The part of AMD's PC FidelityFX API this crate calls (FidelityFX SDK 2.4:
//! `ffx_api.h`, `ffx_api_types.h`, `ffx_upscale.h`, `dx12/ffx_api_dx12.h`),
//! written down by hand from those headers: the chained descriptions, the
//! resource description, and the signatures of the loader's five exports. No
//! AMD header or binary is part of this repository; the loader is loaded at
//! run time (`super::init`). The layouts are asserted in the tests below.
#![allow(dead_code)]

use std::ffi::{c_char, c_void};

/// The header every description starts with.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Header {
    pub kind: u64,
    pub next: *mut Header,
}

impl Header {
    pub const fn new(kind: u64) -> Self {
        Header {
            kind,
            next: std::ptr::null_mut(),
        }
    }
}

// Description types (`FFX_API_*_DESC_TYPE_*`).
pub const CREATE_UPSCALE: u64 = 0x1_0000;
pub const CREATE_BACKEND_DX12: u64 = 0x02;
pub const CREATE_UPSCALE_VERSION: u64 = 0x1_000b;
pub const DISPATCH_UPSCALE: u64 = 0x1_0001;
pub const QUERY_GET_VERSIONS: u64 = 4;
pub const OVERRIDE_VERSION: u64 = 5;
pub const QUERY_GET_PROVIDER_VERSION: u64 = 6;

/// `FFX_UPSCALER_VERSION` (4.1.2): the API version the caller is built for.
pub const UPSCALER_API_VERSION: u32 = (4 << 22) | (1 << 12) | 2;

// `FfxApiCreateContextUpscaleFlags`.
pub const ENABLE_HIGH_DYNAMIC_RANGE: u32 = 1 << 0;
pub const ENABLE_DISPLAY_RESOLUTION_MOTION_VECTORS: u32 = 1 << 1;
pub const ENABLE_MOTION_VECTORS_JITTER_CANCELLATION: u32 = 1 << 2;
pub const ENABLE_DEPTH_INVERTED: u32 = 1 << 3;
pub const ENABLE_DEPTH_INFINITE: u32 = 1 << 4;
pub const ENABLE_AUTO_EXPOSURE: u32 = 1 << 5;
pub const ENABLE_DYNAMIC_RESOLUTION: u32 = 1 << 6;

// `FfxApiResourceState`.
pub const STATE_COMMON: u32 = 1 << 0;
pub const STATE_UNORDERED_ACCESS: u32 = 1 << 1;
pub const STATE_COMPUTE_READ: u32 = 1 << 2;
pub const STATE_PIXEL_READ: u32 = 1 << 3;
pub const STATE_PIXEL_COMPUTE_READ: u32 = STATE_PIXEL_READ | STATE_COMPUTE_READ;
pub const STATE_COPY_SRC: u32 = 1 << 4;
pub const STATE_COPY_DEST: u32 = 1 << 5;
pub const STATE_PRESENT: u32 = 1 << 7;
pub const STATE_RENDER_TARGET: u32 = 1 << 8;
pub const STATE_DEPTH_ATTACHMENT: u32 = 1 << 9;

// `FfxApiResourceUsage`, `FfxApiResourceType`.
pub const USAGE_UAV: u32 = 1 << 1;
pub const USAGE_DEPTHTARGET: u32 = 1 << 2;
pub const USAGE_STENCILTARGET: u32 = 1 << 5;
pub const TYPE_TEXTURE1D: u32 = 1;
pub const TYPE_TEXTURE2D: u32 = 2;
pub const TYPE_TEXTURE_CUBE: u32 = 3;
pub const TYPE_TEXTURE3D: u32 = 4;

/// `ffxCreateContextDescUpscale`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CreateUpscale {
    pub header: Header,
    pub flags: u32,
    pub max_render: [u32; 2],
    pub max_upscale: [u32; 2],
    pub message: usize,
}

/// `ffxCreateBackendDX12Desc`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct BackendDx12 {
    pub header: Header,
    pub device: *mut c_void,
}

/// `ffxCreateContextDescUpscaleVersion`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct UpscaleVersion {
    pub header: Header,
    pub version: u32,
}

/// `ffxOverrideVersion`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct OverrideVersion {
    pub header: Header,
    pub version_id: u64,
}

/// `ffxQueryDescGetVersions`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct QueryVersions {
    pub header: Header,
    pub create_desc_type: u64,
    pub device: *mut c_void,
    pub output_count: *mut u64,
    pub version_ids: *mut u64,
    pub version_names: *mut *const c_char,
}

/// `ffxQueryGetProviderVersion`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct QueryProviderVersion {
    pub header: Header,
    pub version_id: u64,
    pub version_name: *const c_char,
}

/// `FfxApiResourceDescription`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ResourceDescription {
    pub kind: u32,
    pub format: u32,
    pub width: u32,
    pub height: u32,
    pub depth: u32,
    pub mip_count: u32,
    pub flags: u32,
    pub usage: u32,
}

/// `FfxApiResource`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Resource {
    pub resource: *mut c_void,
    pub description: ResourceDescription,
    pub state: u32,
}

impl Resource {
    pub const EMPTY: Resource = Resource {
        resource: std::ptr::null_mut(),
        description: ResourceDescription {
            kind: 0,
            format: 0,
            width: 0,
            height: 0,
            depth: 0,
            mip_count: 0,
            flags: 0,
            usage: 0,
        },
        state: 0,
    };
}

/// `ffxDispatchDescUpscale` (the C `bool`s are bytes).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct DispatchUpscale {
    pub header: Header,
    pub list: *mut c_void,
    pub color: Resource,
    pub depth: Resource,
    pub motion_vectors: Resource,
    pub exposure: Resource,
    pub reactive: Resource,
    pub transparency_and_composition: Resource,
    pub output: Resource,
    pub jitter_offset: [f32; 2],
    pub motion_vector_scale: [f32; 2],
    pub render_size: [u32; 2],
    pub upscale_size: [u32; 2],
    pub enable_sharpening: u8,
    pub sharpness: f32,
    pub frame_time_delta: f32,
    pub pre_exposure: f32,
    pub reset: u8,
    pub camera_near: f32,
    pub camera_far: f32,
    pub camera_fov_angle_vertical: f32,
    pub view_space_to_meters_factor: f32,
    pub flags: u32,
}

pub type CreateContext = unsafe extern "C" fn(*mut *mut c_void, *mut Header, *const c_void) -> u32;
pub type DestroyContext = unsafe extern "C" fn(*mut *mut c_void, *const c_void) -> u32;
pub type Query = unsafe extern "C" fn(*mut *mut c_void, *mut Header) -> u32;
pub type Dispatch = unsafe extern "C" fn(*mut *mut c_void, *const Header) -> u32;

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{offset_of, size_of};

    /// The sizes and offsets the headers give (x64, MSVC).
    #[test]
    fn descriptions_lay_out_as_the_headers() {
        assert_eq!(size_of::<Header>(), 0x10);
        assert_eq!(size_of::<CreateUpscale>(), 0x30);
        assert_eq!(offset_of!(CreateUpscale, flags), 0x10);
        assert_eq!(offset_of!(CreateUpscale, max_render), 0x14);
        assert_eq!(offset_of!(CreateUpscale, max_upscale), 0x1c);
        assert_eq!(offset_of!(CreateUpscale, message), 0x28);
        assert_eq!(size_of::<BackendDx12>(), 0x18);
        assert_eq!(offset_of!(BackendDx12, device), 0x10);
        assert_eq!(size_of::<UpscaleVersion>(), 0x18);
        assert_eq!(size_of::<OverrideVersion>(), 0x18);
        assert_eq!(offset_of!(OverrideVersion, version_id), 0x10);
        assert_eq!(size_of::<QueryVersions>(), 0x38);
        assert_eq!(offset_of!(QueryVersions, create_desc_type), 0x10);
        assert_eq!(offset_of!(QueryVersions, device), 0x18);
        assert_eq!(offset_of!(QueryVersions, output_count), 0x20);
        assert_eq!(offset_of!(QueryVersions, version_ids), 0x28);
        assert_eq!(offset_of!(QueryVersions, version_names), 0x30);
        assert_eq!(size_of::<QueryProviderVersion>(), 0x20);
        assert_eq!(offset_of!(QueryProviderVersion, version_name), 0x18);
        assert_eq!(size_of::<ResourceDescription>(), 0x20);
        assert_eq!(size_of::<Resource>(), 0x30);
        assert_eq!(offset_of!(Resource, description), 0x8);
        assert_eq!(offset_of!(Resource, state), 0x28);
    }

    #[test]
    fn the_upscale_dispatch_lays_out_as_the_header() {
        assert_eq!(size_of::<DispatchUpscale>(), 0x1b0);
        assert_eq!(offset_of!(DispatchUpscale, list), 0x10);
        assert_eq!(offset_of!(DispatchUpscale, color), 0x18);
        assert_eq!(offset_of!(DispatchUpscale, depth), 0x48);
        assert_eq!(offset_of!(DispatchUpscale, motion_vectors), 0x78);
        assert_eq!(offset_of!(DispatchUpscale, exposure), 0xa8);
        assert_eq!(offset_of!(DispatchUpscale, reactive), 0xd8);
        assert_eq!(
            offset_of!(DispatchUpscale, transparency_and_composition),
            0x108
        );
        assert_eq!(offset_of!(DispatchUpscale, output), 0x138);
        assert_eq!(offset_of!(DispatchUpscale, jitter_offset), 0x168);
        assert_eq!(offset_of!(DispatchUpscale, motion_vector_scale), 0x170);
        assert_eq!(offset_of!(DispatchUpscale, render_size), 0x178);
        assert_eq!(offset_of!(DispatchUpscale, upscale_size), 0x180);
        assert_eq!(offset_of!(DispatchUpscale, enable_sharpening), 0x188);
        assert_eq!(offset_of!(DispatchUpscale, sharpness), 0x18c);
        assert_eq!(offset_of!(DispatchUpscale, frame_time_delta), 0x190);
        assert_eq!(offset_of!(DispatchUpscale, pre_exposure), 0x194);
        assert_eq!(offset_of!(DispatchUpscale, reset), 0x198);
        assert_eq!(offset_of!(DispatchUpscale, camera_near), 0x19c);
        assert_eq!(offset_of!(DispatchUpscale, camera_far), 0x1a0);
        assert_eq!(
            offset_of!(DispatchUpscale, camera_fov_angle_vertical),
            0x1a4
        );
        assert_eq!(
            offset_of!(DispatchUpscale, view_space_to_meters_factor),
            0x1a8
        );
        assert_eq!(offset_of!(DispatchUpscale, flags), 0x1ac);
    }

    #[test]
    fn the_constants_are_the_headers() {
        assert_eq!(UPSCALER_API_VERSION, 0x0100_1002);
        assert_eq!(STATE_PIXEL_COMPUTE_READ, 0xc);
    }
}

//! FSR in place of the title's FidelityFX upscaling, through AMD's PC
//! FidelityFX SDK loaded at run time from a folder of the user's
//! (`VICTORY_UPSCALER=fsr`, see `crate::streamline::Asked`).
//!
//! The title's `ffxDispatch` of an upscale (served by
//! `victory_hle::modules::fidelityfx`) is recorded here, at the same place of
//! the title's command list, from the same textures into the same output. The
//! SDK's loader picks the provider: on a GPU without FSR 4 (it is for RDNA
//! only) that is FSR 3.1. The versions it offers are listed and the newest is
//! asked for, at run time and not by build switch.
//!
//! - `VICTORY_FFX_DIR`: the folder of `amd_fidelityfx_loader_dx12.dll` and the
//!   upscaler DLLs beside it (default [`DEFAULT_DIR`]).
//!
//! The upscale context is made on the first upscaling, on the host device, for
//! the output as its largest render size (the title's render size follows its
//! mode and is given per upscaling, with dynamic resolution on). It is made
//! again if the output size or the flags change, and destroyed with the
//! title's own context ([`destroy`]). Anything missing leaves the title's own
//! upscaling as it was, with a warning in the log.

mod abi;
mod convert;
mod create;

use std::ffi::{CStr, c_void};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::sync::{Mutex, OnceLock};

use windows::Win32::Graphics::Direct3D12::ID3D12Resource;
use windows::core::Interface;

use crate::host::HostList;
use crate::streamline::{COLOUR, DEPTH, EXPOSURE, MOTION, OUTPUT, REACTIVE, Upscale, plan};

/// Where the loader is looked for without `VICTORY_FFX_DIR`.
pub const DEFAULT_DIR: &str = r"C:\dev\sdk\fidelityfx-sdk-v2.4.0\bin";
const LOADER: &str = "amd_fidelityfx_loader_dx12.dll";

/// The loader's functions, and the sign and scale fixes asked for
/// (`VICTORY_DLSS_JITTER`, `VICTORY_DLSS_MV`).
struct Api {
    _library: crate::streamline::Library,
    create: abi::CreateContext,
    destroy: abi::DestroyContext,
    query: abi::Query,
    dispatch: abi::Dispatch,
    jitter: [f32; 2],
    mvec: [f32; 2],
}

static API: OnceLock<Api> = OnceLock::new();

/// Whether the loader is in: upscalings can be taken.
pub(crate) fn ready() -> bool {
    API.get().is_some()
}

/// An upscale context: the output and flags it was made for.
struct Context {
    handle: *mut c_void,
    output: [u32; 2],
    flags: u32,
}

// SAFETY: the context is used under `CONTEXT`'s lock, or by its last owner.
unsafe impl Send for Context {}

impl Drop for Context {
    fn drop(&mut self) {
        let Some(api) = API.get() else { return };
        let mut handle = self.handle;
        // SAFETY: a context the loader made; nothing that names it is in flight.
        let code = unsafe { (api.destroy)(&mut handle, std::ptr::null()) };
        tracing::info!(code, output = ?self.output, "FSR: the upscale context was destroyed");
    }
}

static CONTEXT: Mutex<Option<Context>> = Mutex::new(None);

fn context() -> std::sync::MutexGuard<'static, Option<Context>> {
    CONTEXT.lock().unwrap_or_else(|e| e.into_inner())
}

// Upscalings done by FSR, and failed, in all.
static EVALUATED: AtomicU64 = AtomicU64::new(0);
static FAILED: AtomicU64 = AtomicU64::new(0);

/// Loads the loader from `dir`. Fails soft: the reason is returned for the
/// log, and the title's own upscaling stays.
pub(crate) fn init(dir: &Path, jitter: [f32; 2], mvec: [f32; 2]) -> Result<(), String> {
    use windows_sys::Win32::System::LibraryLoader::{
        GetProcAddress, LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR, LOAD_LIBRARY_SEARCH_SYSTEM32,
        LoadLibraryExW,
    };
    let path = std::path::absolute(dir.join(LOADER)).map_err(|e| e.to_string())?;
    if !path.is_file() {
        return Err(format!(
            "no {LOADER} in {} (VICTORY_FFX_DIR)",
            dir.display()
        ));
    }
    crate::streamline::verify_signature(&path)?;
    let file = crate::streamline::wide(&path);
    // Its own folder and System32 only, never the current folder.
    // SAFETY: a NUL-terminated full path.
    let module = unsafe {
        LoadLibraryExW(
            file.as_ptr(),
            std::ptr::null_mut(),
            LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_SYSTEM32,
        )
    };
    if module.is_null() {
        return Err(format!(
            "{LOADER} did not load ({})",
            std::io::Error::last_os_error()
        ));
    }
    let library = crate::streamline::Library(module);
    let find = |name: &CStr| {
        // SAFETY: a loaded module and a NUL-terminated name.
        unsafe { GetProcAddress(module, name.as_ptr().cast()) }
            .map(|f| f as *const c_void)
            .ok_or_else(|| format!("{LOADER} has no {}", name.to_string_lossy()))
    };
    // SAFETY (each): the export of that name has the header's signature.
    let api = Api {
        create: unsafe {
            std::mem::transmute::<*const c_void, abi::CreateContext>(find(c"ffxCreateContext")?)
        },
        destroy: unsafe {
            std::mem::transmute::<*const c_void, abi::DestroyContext>(find(c"ffxDestroyContext")?)
        },
        query: unsafe { std::mem::transmute::<*const c_void, abi::Query>(find(c"ffxQuery")?) },
        dispatch: unsafe {
            std::mem::transmute::<*const c_void, abi::Dispatch>(find(c"ffxDispatch")?)
        },
        jitter,
        mvec,
        _library: library,
    };
    let _ = API.set(api);
    tracing::info!(library = %path.display(), "FSR: the FidelityFX SDK loader is loaded; the version is chosen at the first upscaling");
    Ok(())
}

/// The title's upscaler context is destroyed: ours goes once submitted work
/// has run.
pub(crate) fn destroy() {
    if !ready() {
        return;
    }
    let old = context().take();
    if old.is_some() {
        crate::external::finish_submitted();
    }
    drop(old);
}

/// FSR in place of the title's upscaling, recorded into `host` (where the
/// title's list is now). Whether it recorded anything.
pub(crate) fn evaluate(host: &HostList, up: &Upscale) -> bool {
    let Some(api) = API.get() else { return false };
    let camera = up.camera;
    let fail = |why: &str| {
        let failed = FAILED.fetch_add(1, Relaxed) + 1;
        if failed <= 8 {
            tracing::warn!(
                failed,
                "FSR: the upscaling was not recorded: {why} (the output keeps what it had)"
            );
        }
        false
    };
    let used = |i: usize| {
        !up.resources[i].0.is_null()
            && (i != EXPOSURE || camera.flags & plan::FFX_AUTO_EXPOSURE == 0)
    };
    let mut textures: [Option<(ID3D12Resource, u32)>; 7] = Default::default();
    for i in [COLOUR, DEPTH, MOTION, EXPOSURE, REACTIVE, OUTPUT] {
        if !used(i) {
            continue;
        }
        let (resource, state) = up.resources[i];
        if i != OUTPUT && crate::external::prepare_input(host, resource).is_none() {
            return fail("an input could not be prepared");
        }
        let wanted = if i == OUTPUT {
            plan::output_state(state)
        } else {
            plan::input_state(state)
        };
        let Some((texture, legal)) = crate::external::host_texture(host, resource, wanted) else {
            return fail("a texture has no host copy");
        };
        if legal != wanted {
            return fail("a texture cannot be in the state FSR needs");
        }
        let Some(state) = convert::ffx_state(legal) else {
            return fail("a texture is in a state FSR does not read");
        };
        textures[i] = Some((texture, state));
    }
    let resource = |i: usize| {
        textures[i]
            .as_ref()
            .map_or(abi::Resource::EMPTY, |(t, state)| {
                convert::describe(t, *state)
            })
    };
    let desc = abi::DispatchUpscale {
        header: abi::Header::new(abi::DISPATCH_UPSCALE),
        list: host.list.as_raw(),
        color: resource(COLOUR),
        depth: resource(DEPTH),
        motion_vectors: resource(MOTION),
        exposure: resource(EXPOSURE),
        reactive: resource(REACTIVE),
        transparency_and_composition: abi::Resource::EMPTY,
        output: resource(OUTPUT),
        jitter_offset: [
            camera.jitter[0] * api.jitter[0],
            camera.jitter[1] * api.jitter[1],
        ],
        motion_vector_scale: [
            camera.motion_scale[0] * api.mvec[0],
            camera.motion_scale[1] * api.mvec[1],
        ],
        render_size: camera.render,
        upscale_size: camera.output,
        enable_sharpening: 0,
        sharpness: 0.0,
        frame_time_delta: up.frame_ms,
        pre_exposure: if up.pre_exposure > 0.0 {
            up.pre_exposure
        } else {
            1.0
        },
        reset: u8::from(camera.reset),
        camera_near: camera.near,
        camera_far: camera.far,
        camera_fov_angle_vertical: camera.fov_y,
        view_space_to_meters_factor: 1.0,
        flags: 0,
    };
    let mut slot = context();
    let flags = convert::create_flags(camera.flags);
    if slot
        .as_ref()
        .is_some_and(|c| c.output != camera.output || c.flags != flags)
    {
        // (The list being recorded may be the last to name it.)
        crate::host::retire(slot.take());
    }
    if slot.is_none() {
        match create::make(api, camera.output, camera.flags) {
            Ok(made) => *slot = Some(made),
            Err(why) => return fail(&why),
        }
    }
    let Some(context) = slot.as_mut() else {
        return false;
    };
    // SAFETY: a context of ours and a description that lives through the call.
    let code = unsafe { (api.dispatch)(&mut context.handle, &desc.header) };
    drop(slot);
    if code != 0 {
        return fail(&format!("ffxDispatch returned {code}"));
    }
    crate::external::output_written(up.resources[OUTPUT].0);
    let all = EVALUATED.fetch_add(1, Relaxed) + 1;
    if all == 1 {
        tracing::info!(
            render = ?camera.render,
            output = ?camera.output,
            hdr = camera.flags & plan::FFX_HIGH_DYNAMIC_RANGE != 0,
            depth_inverted = camera.depth_inverted(),
            auto_exposure = textures[EXPOSURE].is_none(),
            reactive = textures[REACTIVE].is_some(),
            display_motion_vectors = camera.display_motion_vectors(),
            jitter = ?desc.jitter_offset,
            motion_vector_scale = ?desc.motion_vector_scale,
            near = camera.near,
            far = camera.far,
            fov = camera.fov_y,
            pre_exposure = desc.pre_exposure,
            frame_ms = desc.frame_time_delta,
            states = ?textures.iter().map(|t| t.as_ref().map(|t| t.1)).collect::<Vec<_>>(),
            "FSR: first upscaling recorded in place of the title's FidelityFX"
        );
    }
    true
}

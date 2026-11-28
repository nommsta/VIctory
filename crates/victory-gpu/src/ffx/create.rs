//! Making the upscale context: the versions the SDK offers, the newest one asked for.

use std::ffi::{CStr, c_void};

use windows::core::Interface;

use super::{Api, Context, abi, convert};

/// The versions of the upscaler the loader offers for `device`: ids and names.
pub(super) fn versions(api: &Api, device: *mut c_void) -> Vec<(u64, String)> {
    let mut count = 0u64;
    let mut query = abi::QueryVersions {
        header: abi::Header::new(abi::QUERY_GET_VERSIONS),
        create_desc_type: abi::CREATE_UPSCALE,
        device,
        output_count: &mut count,
        version_ids: std::ptr::null_mut(),
        version_names: std::ptr::null_mut(),
    };
    // SAFETY: a description that lives through the call; no context is needed.
    if unsafe { (api.query)(std::ptr::null_mut(), &mut query.header) } != 0
        || count == 0
        || count > 64
    {
        return Vec::new();
    }
    let mut ids = vec![0u64; count as usize];
    let mut names = vec![std::ptr::null(); count as usize];
    query.version_ids = ids.as_mut_ptr();
    query.version_names = names.as_mut_ptr();
    // SAFETY: arrays of the capacity `count` says.
    if unsafe { (api.query)(std::ptr::null_mut(), &mut query.header) } != 0 {
        return Vec::new();
    }
    let n = (count as usize).min(ids.len());
    (0..n)
        .map(|i| {
            // SAFETY: a name the loader owns, NUL-terminated, or null.
            let name = if names[i].is_null() {
                String::new()
            } else {
                unsafe { CStr::from_ptr(names[i]) }
                    .to_string_lossy()
                    .into_owned()
            };
            (ids[i], name)
        })
        .collect()
}

/// Makes an upscale context on the host device, with the newest version the
/// loader offers.
pub(super) fn make(api: &Api, output: [u32; 2], title_flags: u32) -> Result<Context, String> {
    let host = crate::host::host().ok_or("no host device")?;
    let device = host.device.as_raw();
    let offered = versions(api, device);
    let names: Vec<String> = offered.iter().map(|v| v.1.clone()).collect();
    let picked = convert::newest(&names).map(|i| offered[i].clone());
    tracing::info!(offered = ?names, picked = picked.as_ref().map(|p| p.1.as_str()), "FSR: the upscaler versions the SDK offers");
    let flags = convert::create_flags(title_flags);
    let mut create = abi::CreateUpscale {
        header: abi::Header::new(abi::CREATE_UPSCALE),
        flags,
        max_render: output,
        max_upscale: output,
        message: 0,
    };
    let mut backend = abi::BackendDx12 {
        header: abi::Header::new(abi::CREATE_BACKEND_DX12),
        device,
    };
    let mut built_for = abi::UpscaleVersion {
        header: abi::Header::new(abi::CREATE_UPSCALE_VERSION),
        version: abi::UPSCALER_API_VERSION,
    };
    let mut over = abi::OverrideVersion {
        header: abi::Header::new(abi::OVERRIDE_VERSION),
        version_id: picked.as_ref().map_or(0, |p| p.0),
    };
    // (Linked last to first: nothing is written to a description once a
    // pointer to it is taken.)
    if picked.is_some() {
        built_for.header.next = &raw mut over.header;
    }
    backend.header.next = &raw mut built_for.header;
    create.header.next = &raw mut backend.header;
    let mut handle = std::ptr::null_mut();
    // SAFETY: a chain of descriptions that lives through the call.
    let code = unsafe { (api.create)(&mut handle, &mut create.header, std::ptr::null()) };
    if code != 0 || handle.is_null() {
        return Err(format!(
            "ffxCreateContext failed ({code}) for output {output:?}, flags {flags:#x}"
        ));
    }
    let mut provider = abi::QueryProviderVersion {
        header: abi::Header::new(abi::QUERY_GET_PROVIDER_VERSION),
        version_id: 0,
        version_name: std::ptr::null(),
    };
    // SAFETY: the context just made and a description that lives through the call.
    let asked = unsafe { (api.query)(&mut handle, &mut provider.header) };
    let version = if asked == 0 && !provider.version_name.is_null() {
        // SAFETY: a name the loader owns.
        unsafe { CStr::from_ptr(provider.version_name) }
            .to_string_lossy()
            .into_owned()
    } else {
        "?".into()
    };
    tracing::info!(version, output = ?output, flags = format_args!("{flags:#x}"), "FSR: the upscale context was made");
    Ok(Context {
        handle,
        output,
        flags,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ffx::{API, DEFAULT_DIR, LOADER, init};
    use crate::streamline::plan;
    use std::path::Path;

    /// With the SDK in its default folder: the loader loads, offers versions,
    /// and makes (and destroys) a context on the host device. Skipped
    /// without the folder or a host device.
    #[test]
    fn the_sdk_offers_versions_and_makes_a_context() {
        let dir = Path::new(DEFAULT_DIR);
        if !dir.join(LOADER).is_file() || crate::host::host().is_none() {
            return;
        }
        init(dir, [1.0; 2], [1.0; 2]).unwrap();
        let api = API.get().unwrap();
        let device = crate::host::host().unwrap().device.as_raw();
        let offered = versions(api, device);
        eprintln!("FSR versions offered: {offered:?}");
        assert!(!offered.is_empty(), "no upscaler version offered");
        let context = make(api, [1920, 1080], plan::FFX_DEPTH_INVERTED).unwrap();
        assert_eq!(
            (context.output, context.flags & abi::ENABLE_DEPTH_INVERTED),
            ([1920, 1080], abi::ENABLE_DEPTH_INVERTED)
        );
        drop(context);
    }
}

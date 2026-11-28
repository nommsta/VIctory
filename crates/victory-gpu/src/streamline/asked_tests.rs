//! The switches that choose the upscaler and its mode.

use super::{Asked, Mode};

fn asked(vars: &[(&str, &str)]) -> Result<Asked, String> {
    Asked::from_lookup(|name| {
        vars.iter()
            .find(|(n, _)| *n == name)
            .map(|(_, v)| v.to_string())
    })
}

#[test]
fn no_switch_asks_for_no_upscaler() {
    let a = asked(&[]).unwrap();
    assert!(!a.dlss && !a.fsr && !a.framegen);
    assert_eq!(a.mode, Mode::Quality);
    assert!(!asked(&[("VICTORY_UPSCALER", "none")]).unwrap().dlss);
}

#[test]
fn only_an_unnamed_upscaler_may_be_chosen_for_the_user() {
    assert!(asked(&[]).unwrap().auto);
    assert!(asked(&[("VICTORY_UPSCALER", "auto")]).unwrap().auto);
    for named in [("VICTORY_UPSCALER", "off"), ("VICTORY_UPSCALER", "fsr"), ("VICTORY_UPSCALER", "dlss"), ("VICTORY_DLSS", "1")] {
        assert!(!asked(&[named]).unwrap().auto, "{named:?}");
    }
}

#[test]
fn the_new_names_choose_the_upscaler_and_the_mode() {
    let a = asked(&[
        ("VICTORY_UPSCALER", "fsr"),
        ("VICTORY_UPSCALE_MODE", "native"),
    ])
    .unwrap();
    assert!(a.fsr && !a.dlss);
    assert_eq!(a.mode, Mode::Dlaa);
    let a = asked(&[
        ("VICTORY_UPSCALER", " DLSS "),
        ("VICTORY_UPSCALE_MODE", "ultraperformance"),
    ])
    .unwrap();
    assert!(a.dlss && !a.fsr);
    assert_eq!(a.mode, Mode::UltraPerformance);
    assert!(
        asked(&[("VICTORY_UPSCALER", "xess")])
            .unwrap_err()
            .contains("VICTORY_UPSCALER")
    );
    assert!(
        asked(&[("VICTORY_UPSCALE_MODE", "ultra")])
            .unwrap_err()
            .contains("VICTORY_UPSCALE_MODE")
    );
}

#[test]
fn the_old_names_still_mean_dlss() {
    let a = asked(&[("VICTORY_DLSS", "1"), ("VICTORY_DLSS_MODE", "balanced")]).unwrap();
    assert!(a.dlss && !a.fsr);
    assert_eq!(a.mode, Mode::Balanced);
    // The old mode name also sets the mode of FSR.
    let a = asked(&[
        ("VICTORY_UPSCALER", "fsr"),
        ("VICTORY_DLSS_MODE", "performance"),
    ])
    .unwrap();
    assert!(a.fsr);
    assert_eq!(a.mode, Mode::Performance);
    // Both names that agree are no conflict.
    let a = asked(&[
        ("VICTORY_UPSCALER", "dlss"),
        ("VICTORY_DLSS", "1"),
        ("VICTORY_UPSCALE_MODE", "dlaa"),
        ("VICTORY_DLSS_MODE", "native"),
    ])
    .unwrap();
    assert!(a.dlss);
    assert_eq!(a.mode, Mode::Dlaa);
}

#[test]
fn switches_that_disagree_are_an_error() {
    assert!(asked(&[("VICTORY_UPSCALER", "fsr"), ("VICTORY_DLSS", "1")]).is_err());
    assert!(
        asked(&[
            ("VICTORY_UPSCALE_MODE", "quality"),
            ("VICTORY_DLSS_MODE", "performance")
        ])
        .is_err()
    );
}

#[test]
fn frame_generation_alone_stays_at_the_output_size() {
    assert_eq!(
        asked(&[("VICTORY_FRAMEGEN", "1")]).unwrap().mode,
        Mode::Dlaa
    );
    assert_eq!(
        asked(&[("VICTORY_FRAMEGEN", "1"), ("VICTORY_UPSCALER", "fsr")])
            .unwrap()
            .mode,
        Mode::Quality
    );
}

#[test]
fn the_sdk_folder_has_a_default() {
    assert_eq!(
        asked(&[]).unwrap().ffx_dir,
        std::path::PathBuf::from(crate::ffx::DEFAULT_DIR)
    );
    assert_eq!(
        asked(&[("VICTORY_FFX_DIR", r"D:\sdk")]).unwrap().ffx_dir,
        std::path::PathBuf::from(r"D:\sdk")
    );
}

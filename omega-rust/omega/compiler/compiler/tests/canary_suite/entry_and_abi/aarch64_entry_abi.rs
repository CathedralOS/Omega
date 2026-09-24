use super::fixture_roster;
use crate::{CanaryCompileProduct, CanaryCompileSpec, compile, fs, pass_canary};

#[test]
fn aarch64_hfa_entry_argument_spreads_vector_registers() {
    let canary = pass_canary(fixture_roster::TARGETS_AARCH64_HFA_ENTRY_ARGUMENT);
    let build_dir =
        std::env::temp_dir().join(format!("omega-aarch64-hfa-entry-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CanaryCompileSpec {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("linux_arm64".into()),
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("flat f64 pair should cross-compile as an AAPCS64 HFA");

    let image = fs::read(build_dir.join("omega-program")).expect("read emitted AArch64 ELF");
    let store_d0 = 0xfd00_0200u32.to_le_bytes();
    let store_d1 = 0xfd00_0601u32.to_le_bytes();
    assert!(
        image
            .windows(16)
            .any(|window| { window[..4] == store_d0 && window[12..16] == store_d1 }),
        "expected entry prologue stores from d0 @ +0 and d1 @ +8"
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn aarch64_hfa_result_entry_loads_d0_d1_and_d2() {
    let canary = pass_canary(fixture_roster::TARGETS_AARCH64_HFA_RESULT_ENTRY);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-aarch64-hfa-result-entry-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CanaryCompileSpec {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("linux_arm64".into()),
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("AAPCS64 24-byte HFA entry result should load d0/d1/d2");

    let image = fs::read(build_dir.join("omega-program")).expect("read emitted AArch64 ELF");
    let register_mask = 0xffc0_03ffu32;
    assert!(
        image.windows(36).any(|window| {
            let first = u32::from_le_bytes(window[8..12].try_into().expect("first load"));
            let second = u32::from_le_bytes(window[20..24].try_into().expect("second load"));
            let third = u32::from_le_bytes(window[32..36].try_into().expect("third load"));
            first & register_mask == 0xfd40_0200
                && second & register_mask == 0xfd40_0201
                && third & register_mask == 0xfd40_0202
        }),
        "expected terminal HFA members loaded into d0, d1, and d2"
    );
    assert!(
        !image.windows(4).any(|window| {
            u32::from_le_bytes(window.try_into().expect("possible x8 store")) & register_mask
                == 0xf900_0208
        }),
        "24-byte HFA must not capture x8 as an indirect-result pointer"
    );
    let _ = fs::remove_dir_all(&build_dir);
}

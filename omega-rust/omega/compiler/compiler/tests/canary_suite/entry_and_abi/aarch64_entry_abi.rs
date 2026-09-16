use super::fixture_roster;
use super::write_cross_target_application_build;
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
fn aarch64_small_aggregate_entry_spreads_consecutive_x_registers() {
    let canary = pass_canary(fixture_roster::TARGETS_AARCH64_SMALL_AGGREGATE_ENTRY);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-aarch64-small-aggregate-entry-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CanaryCompileSpec {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("linux_arm64".into()),
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("small fixed aggregate should cross-compile through x1/x2");

    let image = fs::read(build_dir.join("omega-program")).expect("read emitted AArch64 ELF");
    let store_register_mask = 0xffc0_03ffu32;
    let store_x1 = 0xf900_0201u32;
    let store_x2 = 0xf900_0202u32;
    assert!(
        image.windows(24).any(|window| {
            let first = u32::from_le_bytes(window[8..12].try_into().expect("instruction"));
            let second = u32::from_le_bytes(window[20..24].try_into().expect("instruction"));
            first & store_register_mask == store_x1 && second & store_register_mask == store_x2
        }),
        "expected consecutive entry-prologue stores from x1 and x2"
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn aarch64_small_aggregate_entry_falls_wholly_to_the_stack() {
    let canary = pass_canary(fixture_roster::TARGETS_AARCH64_SMALL_AGGREGATE_STACK_ENTRY);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-aarch64-small-aggregate-stack-entry-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CanaryCompileSpec {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("linux_arm64".into()),
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("register-exhausted small aggregate should cross-compile from the stack");

    let image = fs::read(build_dir.join("omega-program")).expect("read emitted AArch64 ELF");
    // The canonical FPCR save enlarged the fixed prologue frame to 0x70
    // bytes. Incoming stack arguments begin immediately above that frame.
    let load_first = 0xf940_3bf1u32.to_le_bytes();
    let load_second = 0xf940_3ff1u32.to_le_bytes();
    assert!(
        image
            .windows(32)
            .any(|window| { window[8..12] == load_first && window[24..28] == load_second }),
        "expected consecutive incoming-stack loads for both aggregate fragments"
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn aarch64_large_aggregate_entry_copies_from_the_indirect_pointer() {
    let canary = pass_canary(fixture_roster::TARGETS_AARCH64_LARGE_AGGREGATE_ENTRY);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-aarch64-large-aggregate-entry-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CanaryCompileSpec {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("linux_arm64".into()),
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("large fixed aggregate should cross-compile through an x0 pointer");

    let image = fs::read(build_dir.join("omega-program")).expect("read emitted AArch64 ELF");
    let loads = [0xf940_0011u32, 0xf940_0411, 0xf940_0811];
    let store_mask = 0xffc0_03ffu32;
    assert!(
        image.windows(32).any(|window| {
            loads.iter().enumerate().all(|(fragment, expected)| {
                let load_offset = 8 + fragment * 8;
                let store_offset = load_offset + 4;
                let load = u32::from_le_bytes(
                    window[load_offset..load_offset + 4]
                        .try_into()
                        .expect("load instruction"),
                );
                let store = u32::from_le_bytes(
                    window[store_offset..store_offset + 4]
                        .try_into()
                        .expect("store instruction"),
                );
                load == *expected && store & store_mask == 0xf900_0211
            })
        }),
        "expected three x0-pointee loads copied into runtime-frame storage"
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn aarch64_large_aggregate_entry_loads_a_stack_passed_pointer() {
    let canary = pass_canary(fixture_roster::TARGETS_AARCH64_LARGE_AGGREGATE_STACK_ENTRY);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-aarch64-large-aggregate-stack-entry-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CanaryCompileSpec {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("linux_arm64".into()),
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("register-exhausted large aggregate should cross-compile through a stack pointer");

    let image = fs::read(build_dir.join("omega-program")).expect("read emitted AArch64 ELF");
    // The ordinary prologue reserves the 112-byte AArch64 function frame
    // before loading this first incoming stack argument.
    let pointer_load = 0xf940_3bf1u32;
    let pointee_loads = [0xf940_022au32, 0xf940_062a, 0xf940_0a2a];
    let store_mask = 0xffc0_03ffu32;
    assert!(
        image.windows(36).any(|window| {
            u32::from_le_bytes(window[..4].try_into().expect("pointer load")) == pointer_load
                && pointee_loads
                    .iter()
                    .enumerate()
                    .all(|(fragment, expected)| {
                        let load_offset = 12 + fragment * 8;
                        let store_offset = load_offset + 4;
                        let load = u32::from_le_bytes(
                            window[load_offset..load_offset + 4]
                                .try_into()
                                .expect("pointee load"),
                        );
                        let store = u32::from_le_bytes(
                            window[store_offset..store_offset + 4]
                                .try_into()
                                .expect("store instruction"),
                        );
                        load == *expected && store & store_mask == 0xf900_020a
                    })
        }),
        "expected the incoming-stack pointer load before three pointee copies"
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn aarch64_wide_aggregate_entry_uses_general_indirect_classification() {
    let canary = pass_canary(fixture_roster::TARGETS_AARCH64_WIDE_AGGREGATE_ENTRY);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-aarch64-wide-aggregate-entry-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CanaryCompileSpec {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("linux_arm64".into()),
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("record beyond the boundary handoff ceiling should use AAPCS64 indirect passing");

    let image = fs::read(build_dir.join("omega-program")).expect("read emitted AArch64 ELF");
    let loads = [
        0xf940_0011u32,
        0xf940_0411,
        0xf940_0811,
        0xf940_0c11,
        0xf940_1011,
    ];
    let store_mask = 0xffc0_03ffu32;
    assert!(
        image.windows(48).any(|window| {
            loads.iter().enumerate().all(|(fragment, expected)| {
                let load_offset = 8 + fragment * 8;
                let store_offset = load_offset + 4;
                let load = u32::from_le_bytes(
                    window[load_offset..load_offset + 4]
                        .try_into()
                        .expect("load instruction"),
                );
                let store = u32::from_le_bytes(
                    window[store_offset..store_offset + 4]
                        .try_into()
                        .expect("store instruction"),
                );
                load == *expected && store & store_mask == 0xf900_0211
            })
        }),
        "expected all five x0-pointee words copied into runtime-frame storage"
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn aarch64_small_result_entry_loads_x0_and_x1() {
    let canary = pass_canary(fixture_roster::TARGETS_AARCH64_SMALL_RESULT_ENTRY);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-aarch64-small-result-entry-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CanaryCompileSpec {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("linux_arm64".into()),
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("AAPCS64 two-word entry result should load x0/x1");

    let image = fs::read(build_dir.join("omega-program")).expect("read emitted AArch64 ELF");
    let register_mask = 0xffc0_03ffu32;
    assert!(
        image.windows(24).any(|window| {
            let first = u32::from_le_bytes(window[8..12].try_into().expect("first load"));
            let second = u32::from_le_bytes(window[20..24].try_into().expect("second load"));
            first & register_mask == 0xf940_0200 && second & register_mask == 0xf940_0201
        }),
        "expected terminal record fragments loaded into x0 and x1"
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn aggregate_literal_entry_result_uses_native_fragments() {
    let canary = pass_canary(fixture_roster::TARGETS_AGGREGATE_LITERAL_RESULT_ENTRY);
    for target in ["linux_x86_64", "linux_arm64", "uefi_x86_64"] {
        let scratch = std::env::temp_dir().join(format!(
            "omega-{target}-aggregate-literal-result-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        let src_dir = scratch.join("src");
        let out_dir = scratch.join("out");
        fs::create_dir_all(&src_dir).expect("scratch source directory");
        fs::copy(canary.join("main.omg"), src_dir.join("main.omg")).expect("copy canary");
        write_cross_target_application_build(&src_dir);
        compile(CanaryCompileSpec {
            root_path: src_dir.join("main.omg"),
            build_dir: Some(out_dir.clone()),
            target_name: Some(target.into()),
            product: CanaryCompileProduct::NativeArtifactAndPublish,
        })
        .expect("aggregate-literal entry result should cross-compile");

        let output_name = if target == "uefi_x86_64" {
            "omega-program.exe"
        } else {
            "omega-program"
        };
        let image = fs::read(out_dir.join(output_name)).expect("read emitted image");
        if target == "linux_x86_64" {
            assert!(
                image.windows(34).any(|window| {
                    window[10..13] == [0x49, 0x8b, 0x87] && window[27..30] == [0x49, 0x8b, 0x97]
                }),
                "SysV aggregate literal missing rax/rdx result loads"
            );
        } else if target == "linux_arm64" {
            let register_mask = 0xffc0_03ffu32;
            assert!(
                image.windows(24).any(|window| {
                    let first = u32::from_le_bytes(window[8..12].try_into().expect("first load"));
                    let second =
                        u32::from_le_bytes(window[20..24].try_into().expect("second load"));
                    first & register_mask == 0xf940_0200 && second & register_mask == 0xf940_0201
                }),
                "AAPCS64 aggregate literal missing x0/x1 result loads"
            );
        } else {
            assert!(
                image.windows(17).any(|window| {
                    window[0..2] == [0x49, 0xbf] && window[10..13] == [0x49, 0x89, 0x8f]
                }),
                "Microsoft x64 aggregate literal missing hidden-result capture"
            );
            assert!(
                image.windows(3).any(|window| window == [0x4d, 0x8b, 0xbe]),
                "Microsoft x64 aggregate literal missing scratch-to-pointee copy"
            );
            assert!(
                image.windows(3).any(|window| window == [0x49, 0x8b, 0x87]),
                "Microsoft x64 aggregate literal missing hidden pointer return"
            );
        }
        let _ = fs::remove_dir_all(&scratch);
    }
}

#[test]
fn indexed_scalar_entry_result_uses_native_registers() {
    let canary = pass_canary(fixture_roster::TARGETS_INDEXED_SCALAR_RESULT_ENTRY);
    for target in ["linux_x86_64", "linux_arm64"] {
        let scratch = std::env::temp_dir().join(format!(
            "omega-{target}-indexed-scalar-result-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        let src_dir = scratch.join("src");
        let out_dir = scratch.join("out");
        fs::create_dir_all(&src_dir).expect("scratch source directory");
        fs::copy(canary.join("main.omg"), src_dir.join("main.omg")).expect("copy canary");
        write_cross_target_application_build(&src_dir);
        compile(CanaryCompileSpec {
            root_path: src_dir.join("main.omg"),
            build_dir: Some(out_dir.clone()),
            target_name: Some(target.into()),
            product: CanaryCompileProduct::NativeArtifactAndPublish,
        })
        .expect("indexed scalar entry result should cross-compile");

        let image = fs::read(out_dir.join("omega-program")).expect("read emitted ELF");
        if target == "linux_x86_64" {
            assert!(
                image.windows(3).any(|window| window == [0x41, 0x8b, 0x87]),
                "SysV indexed scalar terminal missing eax scratch load"
            );
        } else {
            assert!(
                image.windows(4).any(|window| {
                    let instruction = u32::from_le_bytes(window.try_into().expect("word"));
                    instruction & !0x003f_fc00 == 0xb940_0200
                }),
                "AAPCS64 indexed scalar terminal missing w0 scratch load"
            );
        }
        let _ = fs::remove_dir_all(&scratch);
    }
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

#[test]
fn aarch64_large_result_entry_saves_x8_and_copies_through_it() {
    let canary = pass_canary(fixture_roster::TARGETS_AARCH64_LARGE_RESULT_ENTRY);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-aarch64-large-result-entry-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CanaryCompileSpec {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("linux_arm64".into()),
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("AAPCS64 indirect entry result should preserve and populate x8's pointer");

    let footprint_artifact = fs::read_to_string(build_dir.join("08_boundary_footprints.json"))
        .expect("AAPCS64 indirect-result footprint evidence should be written");
    assert!(
        footprint_artifact.contains("\"boundary_contract_fingerprint\": \"0x")
            && footprint_artifact.contains("\"origin\": \"call_return_mechanics\"")
            && footprint_artifact.contains("\"origin\": \"exit_indirect_result_copy\"")
            && footprint_artifact.contains("\"enumeration_complete\": false"),
        "AAPCS64 hidden-result copy must bind evidence to its contract without claiming final completeness"
    );

    let image = fs::read(build_dir.join("omega-program")).expect("read emitted AArch64 ELF");
    let register_mask = 0xffc0_03ffu32;
    assert!(
        image.windows(4).any(|window| {
            u32::from_le_bytes(window.try_into().expect("x8 store")) & register_mask == 0xf900_0208
        }),
        "expected incoming x8 hidden-result pointer captured in runtime storage"
    );
    let terminal_copy = [
        0xf940_0210u32,
        0xf940_0291,
        0xf900_0211,
        0xf940_0691,
        0xf900_0611,
        0xf940_0a91,
        0xf900_0a11,
        0x5280_001c,
    ];
    assert!(
        image.windows(terminal_copy.len() * 4).any(|window| {
            terminal_copy.iter().enumerate().all(|(index, expected)| {
                u32::from_le_bytes(
                    window[index * 4..index * 4 + 4]
                        .try_into()
                        .expect("terminal-copy instruction"),
                ) == *expected
            })
        }),
        "expected three terminal words copied through saved x8 with no x0 pointer return"
    );
    let _ = fs::remove_dir_all(&build_dir);
}

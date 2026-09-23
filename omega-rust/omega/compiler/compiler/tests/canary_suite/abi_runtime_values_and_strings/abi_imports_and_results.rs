use super::fixture_roster;
use crate::{
    CanaryCompileProduct, CanaryCompileSpec, Command, compile, compile_reviewed_repository_fixture,
    compile_rooted_canary_for_native_host, fs, hosted_main_program_entry_build_for, interpret,
    pass_canary, production_compile,
};
#[cfg(windows)]
use crate::{compile_rooted_canary_for_target, executable_name};
use compiler::CheckedCompileRequest;

#[test]
fn cross_aarch64_authored_scalar_float_preserves_vector_class() {
    let canary = pass_canary(fixture_roster::AARCH64_SCALAR_FLOAT_IMPORT_COMPILE);
    let scratch = std::env::temp_dir().join(format!(
        "omega-aarch64-scalar-float-import-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let src_dir = scratch.join("src");
    let out_dir = scratch.join("out");
    fs::create_dir_all(&src_dir).expect("scratch source directory");
    fs::copy(canary.join("main.omg"), src_dir.join("main.omg")).expect("copy canary");
    fs::write(
        src_dir.join("build.omg"),
        hosted_main_program_entry_build_for(&canary, "macos_arm64"),
    )
    .expect("write macos_arm64 build source");

    compile(CanaryCompileSpec {
        root_path: src_dir.join("main.omg"),
        build_dir: Some(out_dir.clone()),
        target_name: Some("macos_arm64".to_owned()),
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("authored scalar-float import should compile for macos_arm64");

    let image = fs::read(out_dir.join("omega-program")).expect("read emitted AArch64 Mach-O");
    let fmov_d0_x16 = (0x9e67_0000u32 | (16 << 5)).to_le_bytes();
    let fmov_x0_d0 = 0x9e66_0000u32.to_le_bytes();
    assert!(
        image.windows(4).any(|window| window == fmov_d0_x16),
        "the authored f64 argument must marshal into d0"
    );
    assert!(
        image.windows(4).any(|window| window == fmov_x0_d0),
        "the authored f64 result must spill from d0"
    );
    let footprints = fs::read_to_string(out_dir.join("08_boundary_footprints.json"))
        .expect("AArch64 authored scalar-float footprints should be written");
    assert!(
        footprints.contains("\"origin\": \"compiler_body_outbound_authored_float_import_result\""),
        "AArch64 authored scalar-float import must retain its final replay footprint"
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn cross_aarch64_small_aggregate_import_uses_consecutive_x_registers() {
    let canary = pass_canary(fixture_roster::AARCH64_SMALL_AGGREGATE_IMPORT_COMPILE);
    let scratch = std::env::temp_dir().join(format!(
        "omega-aarch64-small-aggregate-import-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let src_dir = scratch.join("src");
    let out_dir = scratch.join("out");
    fs::create_dir_all(&src_dir).expect("scratch source directory");
    fs::copy(canary.join("main.omg"), src_dir.join("main.omg")).expect("copy canary");
    fs::write(
        src_dir.join("build.omg"),
        hosted_main_program_entry_build_for(&canary, "macos_arm64"),
    )
    .expect("write macos_arm64 build source");

    compile(CanaryCompileSpec {
        root_path: src_dir.join("main.omg"),
        build_dir: Some(out_dir.clone()),
        target_name: Some("macos_arm64".to_owned()),
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("by-value small aggregate import should compile for macos_arm64");

    let image = fs::read(out_dir.join("omega-program")).expect("read emitted AArch64 Mach-O");
    let load_register_mask = 0xffc0_03ffu32;
    assert!(
        image.windows(8).any(|window| {
            let first = u32::from_le_bytes(window[0..4].try_into().expect("instruction"));
            let second = u32::from_le_bytes(window[4..8].try_into().expect("instruction"));
            first & load_register_mask == 0xf940_0201 && second & load_register_mask == 0xf940_0202
        }),
        "expected one aggregate source to feed consecutive x1/x2 fragments"
    );
    let footprints = fs::read_to_string(out_dir.join("08_boundary_footprints.json"))
        .expect("AArch64 authored aggregate footprints should be written");
    assert!(
        footprints
            .contains("\"origin\": \"compiler_body_outbound_authored_aggregate_import_result\""),
        "AArch64 authored aggregate import must retain its final replay footprint"
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn cross_sysv_small_aggregate_import_reaches_elf_dynamic_binding_blocker() {
    let canary = pass_canary(fixture_roster::SYSV_SMALL_AGGREGATE_IMPORT_COMPILE);
    let scratch = std::env::temp_dir().join(format!(
        "omega-sysv-small-aggregate-import-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);

    let diagnostics = production_compile(CanaryCompileSpec {
        root_path: canary.join("main.omg"),
        build_dir: Some(scratch.clone()),
        target_name: Some("linux_x86_64".to_owned()),
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect_err("ELF direct images do not have dynamic import binding yet");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("relocation references unknown symbol `omega_sysv_small_aggregate_probe`")
        }),
        "the source call must pass SysV selection/encoding/relocation and stop only at ELF dynamic binding: {diagnostics:#?}"
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn cross_aarch64_small_aggregate_import_falls_wholly_to_stack() {
    let canary = pass_canary(fixture_roster::AARCH64_SMALL_AGGREGATE_STACK_IMPORT_COMPILE);
    let scratch = std::env::temp_dir().join(format!(
        "omega-aarch64-small-aggregate-stack-import-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let src_dir = scratch.join("src");
    let out_dir = scratch.join("out");
    fs::create_dir_all(&src_dir).expect("scratch source directory");
    fs::copy(canary.join("main.omg"), src_dir.join("main.omg")).expect("copy canary");
    fs::write(
        src_dir.join("build.omg"),
        hosted_main_program_entry_build_for(&canary, "macos_arm64"),
    )
    .expect("write macos_arm64 build source");

    compile(CanaryCompileSpec {
        root_path: src_dir.join("main.omg"),
        build_dir: Some(out_dir.clone()),
        target_name: Some("macos_arm64".to_owned()),
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("register-exhausted small aggregate import should compile for macos_arm64");

    let image = fs::read(out_dir.join("omega-program")).expect("read emitted AArch64 Mach-O");
    for (name, instruction) in [
        ("16-byte outgoing reserve", 0xd100_43ffu32),
        ("first aggregate stack store", 0xf900_03f1u32),
        ("second aggregate stack store", 0xf900_07f1u32),
        ("outgoing stack restore", 0x9100_43ffu32),
    ] {
        let bytes = instruction.to_le_bytes();
        assert!(
            image.windows(4).any(|window| window == bytes),
            "AArch64 aggregate stack-import image missing {name}"
        );
    }
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn cross_aarch64_hfa_stack_import_copies_the_aggregate() {
    let canary = pass_canary(fixture_roster::AARCH64_HFA_STACK_IMPORT_COMPILE);
    let scratch =
        std::env::temp_dir().join(format!("omega-aarch64-hfa-stack-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let src_dir = scratch.join("src");
    let out_dir = scratch.join("out");
    fs::create_dir_all(&src_dir).expect("scratch source directory");
    fs::copy(canary.join("main.omg"), src_dir.join("main.omg")).expect("copy canary");
    fs::write(
        src_dir.join("build.omg"),
        hosted_main_program_entry_build_for(&canary, "macos_arm64"),
    )
    .expect("write macos_arm64 build source");

    compile(CanaryCompileSpec {
        root_path: src_dir.join("main.omg"),
        build_dir: Some(out_dir.clone()),
        target_name: Some("macos_arm64".to_owned()),
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("stack-resident HFA import should compile for macos_arm64");

    let image = fs::read(out_dir.join("omega-program")).expect("read emitted AArch64 Mach-O");
    for (name, instruction) in [
        ("16-byte outgoing reserve", 0xd100_43ffu32),
        ("first HFA member stack store", 0xf900_03f1u32),
        ("second HFA member stack store", 0xf900_07f1u32),
        ("outgoing stack restore", 0x9100_43ffu32),
    ] {
        let bytes = instruction.to_le_bytes();
        assert!(
            image.windows(4).any(|window| window == bytes),
            "AArch64 HFA stack-import image missing {name}"
        );
    }
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn cross_aarch64_hfa_result_import_spills_fragmented_result() {
    let canary = pass_canary(fixture_roster::AARCH64_HFA_RESULT_IMPORT_COMPILE);
    let scratch =
        std::env::temp_dir().join(format!("omega-aarch64-hfa-result-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let src_dir = scratch.join("src");
    let out_dir = scratch.join("out");
    fs::create_dir_all(&src_dir).expect("scratch source directory");
    fs::copy(canary.join("main.omg"), src_dir.join("main.omg")).expect("copy canary");
    fs::write(
        src_dir.join("build.omg"),
        hosted_main_program_entry_build_for(&canary, "macos_arm64"),
    )
    .expect("write macos_arm64 build source");

    compile(CanaryCompileSpec {
        root_path: src_dir.join("main.omg"),
        build_dir: Some(out_dir.clone()),
        target_name: Some("macos_arm64".to_owned()),
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("HFA-returning import should compile for macos_arm64");

    let image = fs::read(out_dir.join("omega-program")).expect("read emitted AArch64 Mach-O");
    let fmov_x17_d0 = (0x9e66_0000u32 | 17).to_le_bytes();
    let fmov_x17_d1 = (0x9e66_0000u32 | (1 << 5) | 17).to_le_bytes();
    assert!(
        image
            .windows(12)
            .any(|window| { window[0..4] == fmov_x17_d0 && window[8..12] == fmov_x17_d1 }),
        "expected consecutive d0/d1 result fragments to spill through x17"
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn cross_aarch64_erased_hfa_result_keeps_two_vector_fragments() {
    let canary = pass_canary(fixture_roster::AARCH64_ERASED_HFA_RESULT_IMPORT_COMPILE);
    let scratch = std::env::temp_dir().join(format!(
        "omega-aarch64-erased-hfa-result-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let src_dir = scratch.join("src");
    let out_dir = scratch.join("out");
    fs::create_dir_all(&src_dir).expect("scratch source directory");
    fs::copy(canary.join("main.omg"), src_dir.join("main.omg")).expect("copy canary");
    fs::write(
        src_dir.join("build.omg"),
        hosted_main_program_entry_build_for(&canary, "macos_arm64"),
    )
    .expect("write macos_arm64 build source");

    compile(CanaryCompileSpec {
        root_path: src_dir.join("main.omg"),
        build_dir: Some(out_dir.clone()),
        target_name: Some("macos_arm64".to_owned()),
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("erased-stripped HFA result should compile for macos_arm64");

    let image = fs::read(out_dir.join("omega-program")).expect("read emitted AArch64 Mach-O");
    let fmov_x17_d0 = (0x9e66_0000u32 | 17).to_le_bytes();
    let fmov_x17_d1 = (0x9e66_0000u32 | (1 << 5) | 17).to_le_bytes();
    assert!(
        image
            .windows(12)
            .any(|window| { window[0..4] == fmov_x17_d0 && window[8..12] == fmov_x17_d1 }),
        "erased evidence must not interrupt the d0/d1 result spill"
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn cross_aarch64_small_aggregate_result_import_spills_fragmented_result() {
    let canary = pass_canary(fixture_roster::AARCH64_SMALL_AGGREGATE_RESULT_IMPORT_COMPILE);
    let scratch = std::env::temp_dir().join(format!(
        "omega-aarch64-small-aggregate-result-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let src_dir = scratch.join("src");
    let out_dir = scratch.join("out");
    fs::create_dir_all(&src_dir).expect("scratch source directory");
    fs::copy(canary.join("main.omg"), src_dir.join("main.omg")).expect("copy canary");
    fs::write(
        src_dir.join("build.omg"),
        hosted_main_program_entry_build_for(&canary, "macos_arm64"),
    )
    .expect("write macos_arm64 build source");

    compile(CanaryCompileSpec {
        root_path: src_dir.join("main.omg"),
        build_dir: Some(out_dir.clone()),
        target_name: Some("macos_arm64".to_owned()),
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("small-aggregate-returning import should compile for macos_arm64");

    let image = fs::read(out_dir.join("omega-program")).expect("read emitted AArch64 Mach-O");
    let store_mask = 0xffc0_03ff;
    assert!(
        image.windows(8).any(|window| {
            let first = u32::from_le_bytes(window[0..4].try_into().unwrap());
            let second = u32::from_le_bytes(window[4..8].try_into().unwrap());
            first & store_mask == 0xf900_0200 && second & store_mask == 0xf900_0201
        }),
        "expected consecutive x0/x1 result fragments to spill through x16"
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn cross_aarch64_large_aggregate_import_uses_indirect_places() {
    let canary = pass_canary(fixture_roster::AARCH64_LARGE_AGGREGATE_IMPORT_COMPILE);
    let scratch = std::env::temp_dir().join(format!(
        "omega-aarch64-large-aggregate-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let src_dir = scratch.join("src");
    let out_dir = scratch.join("out");
    fs::create_dir_all(&src_dir).expect("scratch source directory");
    fs::copy(canary.join("main.omg"), src_dir.join("main.omg")).expect("copy canary");
    fs::write(
        src_dir.join("build.omg"),
        hosted_main_program_entry_build_for(&canary, "macos_arm64"),
    )
    .expect("write macos_arm64 build source");

    compile(CanaryCompileSpec {
        root_path: src_dir.join("main.omg"),
        build_dir: Some(out_dir.clone()),
        target_name: Some("macos_arm64".to_owned()),
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("large aggregate import should compile for macos_arm64");

    let image = fs::read(out_dir.join("omega-program")).expect("read emitted AArch64 Mach-O");
    for (name, instruction) in [
        ("32-byte caller-copy reserve", 0xd100_83ffu32),
        ("caller-copy pointer in x0", 0x9100_03e0u32),
        ("caller-copy restore", 0x9100_83ffu32),
    ] {
        assert!(
            image
                .windows(4)
                .any(|window| window == instruction.to_le_bytes()),
            "AArch64 large-aggregate image missing {name}"
        );
    }
    let footprints = fs::read_to_string(out_dir.join("08_boundary_footprints.json"))
        .expect("AArch64 authored aggregate-result footprints should be written");
    assert!(
        footprints.contains("\"origin\": \"compiler_body_outbound_authored_aggregate_result\""),
        "AArch64 authored aggregate result must retain its final replay footprint"
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn cross_win64_large_aggregate_import_uses_an_aligned_caller_copy() {
    let canary = pass_canary(fixture_roster::WIN64_LARGE_AGGREGATE_IMPORT_COMPILE);
    let scratch = std::env::temp_dir().join(format!(
        "omega-win64-large-aggregate-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);

    compile(CanaryCompileSpec {
        root_path: canary.join("main.omg"),
        build_dir: Some(scratch.clone()),
        target_name: Some("windows_x86_64".to_owned()),
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("by-reference aggregate import should compile for windows_x64");

    let image = fs::read(scratch.join("omega-program.exe")).expect("read emitted Win64 PE");
    assert!(
        image
            .windows(4)
            .any(|window| window == [0x48, 0x83, 0xec, 56]),
        "expected shadow space plus the aligned 24-byte caller copy"
    );
    for copy_offset in [32u32, 40, 48] {
        let mut store = vec![0x48, 0x89, 0x84, 0x24];
        store.extend(copy_offset.to_le_bytes());
        assert!(
            image.windows(store.len()).any(|window| window == store),
            "expected aggregate fragment at outgoing stack offset {copy_offset}"
        );
    }
    assert!(
        image
            .windows(8)
            .any(|window| window == [0x48, 0x8d, 0x94, 0x24, 32, 0, 0, 0]),
        "expected RDX to point at the aligned caller copy"
    );
    assert!(
        image
            .windows(4)
            .any(|window| window == [0x48, 0x83, 0xc4, 56]),
        "expected the complete outgoing area to be restored"
    );
    let footprints = fs::read_to_string(scratch.join("08_boundary_footprints.json"))
        .expect("Win64 authored aggregate footprints should be written");
    assert!(
        footprints
            .contains("\"origin\": \"compiler_body_outbound_authored_aggregate_import_result\""),
        "Win64 authored aggregate import must retain its final replay footprint"
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn cross_win64_direct_aggregate_import_loads_the_record_by_value() {
    let canary = pass_canary(fixture_roster::WIN64_DIRECT_AGGREGATE_IMPORT_COMPILE);
    let scratch = std::env::temp_dir().join(format!(
        "omega-win64-direct-aggregate-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);

    compile(CanaryCompileSpec {
        root_path: canary.join("main.omg"),
        build_dir: Some(scratch.clone()),
        target_name: Some("windows_x86_64".to_owned()),
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("direct aggregate import should compile for windows_x64");

    let image = fs::read(scratch.join("omega-program.exe")).expect("read emitted Win64 PE");
    assert!(
        image
            .windows(17)
            .any(|window| { window[0..2] == [0x49, 0xbb] && window[10..13] == [0x49, 0x8b, 0x93] }),
        "expected the eight-byte record loaded by value into RDX"
    );
    assert!(
        image
            .windows(4)
            .any(|window| window == [0x48, 0x83, 0xec, 40]),
        "a direct record must require only the ordinary shadow reservation"
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn cross_win64_direct_aggregate_result_spills_rax_by_value() {
    let canary = pass_canary(fixture_roster::WIN64_DIRECT_AGGREGATE_RESULT_IMPORT_COMPILE);
    let scratch = std::env::temp_dir().join(format!(
        "omega-win64-direct-aggregate-result-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);

    compile(CanaryCompileSpec {
        root_path: canary.join("main.omg"),
        build_dir: Some(scratch.clone()),
        target_name: Some("windows_x86_64".to_owned()),
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("direct aggregate result import should compile for windows_x64");

    let image = fs::read(scratch.join("omega-program.exe")).expect("read emitted Win64 PE");
    assert!(
        image
            .windows(17)
            .any(|window| { window[0..2] == [0x49, 0xbb] && window[10..13] == [0x49, 0x89, 0x83] }),
        "expected the eight-byte record spilled from RAX into aggregate storage"
    );
    let footprints = fs::read_to_string(scratch.join("08_boundary_footprints.json"))
        .expect("Win64 authored aggregate-result footprints should be written");
    assert!(
        footprints.contains("\"origin\": \"compiler_body_outbound_authored_aggregate_result\""),
        "Win64 authored aggregate result must retain its final replay footprint"
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn cross_win64_large_aggregate_result_uses_hidden_rcx_destination() {
    let canary = pass_canary(fixture_roster::WIN64_LARGE_AGGREGATE_RESULT_IMPORT_COMPILE);
    let scratch = std::env::temp_dir().join(format!(
        "omega-win64-large-aggregate-result-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);

    compile(CanaryCompileSpec {
        root_path: canary.join("main.omg"),
        build_dir: Some(scratch.clone()),
        target_name: Some("windows_x86_64".to_owned()),
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("indirect aggregate result import should compile for windows_x64");

    let image = fs::read(scratch.join("omega-program.exe")).expect("read emitted Win64 PE");
    assert!(
        image.windows(34).any(|window| {
            window[0..2] == [0x49, 0xbb]
                && window[10..13] == [0x49, 0x8d, 0x8b]
                && window[17..19] == [0x49, 0xbb]
                && window[27..30] == [0x49, 0x8b, 0x93]
        }),
        "expected hidden RCX result address followed by the declared seed in RDX"
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn float_operator_contracts_ignore_unrelated_private_free_machines() {
    let canary = pass_canary(fixture_roster::WIN64_SCALAR_FLOAT_IMPORT_COMPILE);
    compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &canary.join("main.omg"),
        Some("windows_x86_64"),
    ))
    .expect("public FloatSemantics calls must not select the private Nat machine");
}

#[test]
fn cross_win64_scalar_float_import_uses_positional_xmm_and_stack_locations() {
    let canary = pass_canary(fixture_roster::WIN64_SCALAR_FLOAT_IMPORT_COMPILE);
    let scratch =
        std::env::temp_dir().join(format!("omega-win64-scalar-float-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);

    compile(CanaryCompileSpec {
        root_path: canary.join("main.omg"),
        build_dir: Some(scratch.clone()),
        target_name: Some("windows_x86_64".to_owned()),
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("scalar-float import should compile for windows_x64");

    let image = fs::read(scratch.join("omega-program.exe")).expect("read emitted Win64 PE");
    for (name, opcode) in [
        ("second-position XMM1 load", [0xf2, 0x41, 0x0f, 0x10, 0x8b]),
        ("fourth-position XMM3 load", [0xf2, 0x41, 0x0f, 0x10, 0x9b]),
        ("XMM0 result store", [0xf2, 0x41, 0x0f, 0x11, 0x83]),
    ] {
        assert!(
            image.windows(opcode.len()).any(|window| window == opcode),
            "Win64 scalar-float image missing {name}"
        );
    }
    assert!(
        image.windows(15).any(|window| {
            window[0..3] == [0x49, 0x8b, 0x83]
                && window[7..11] == [0x48, 0x89, 0x84, 0x24]
                && window[11..15] == 32u32.to_le_bytes()
        }),
        "the fifth-position f64 must occupy outgoing stack slot 32"
    );
    let footprints = fs::read_to_string(scratch.join("08_boundary_footprints.json"))
        .expect("Win64 authored scalar-float footprints should be written");
    assert!(
        footprints.contains("\"origin\": \"compiler_body_outbound_authored_float_import_result\""),
        "Win64 authored scalar-float import must retain its final replay footprint"
    );
    let _ = fs::remove_dir_all(&scratch);
}

// A source-authored external import end to end: the program's bodyless
// `satisfies Beeper::beep via beeper_binding()` leaf binds the typed
// compile-time producer's evaluated `ForeignBinding::DllImport` value -- the
// normalized `PeByName { library: "msvcrt.dll", export: "abs" }` locator, not
// the KERNEL32 catalog default -- and abs(-42) delivers 42 through the result
// place (ZII would exit 71). NATIVE-ONLY: no interpreter provider exists for
// authored bindings, so unlike its neighbors this test runs no interp oracle.

#[test]
fn windows_external_import_canary_selects_evaluated_import_plan() {
    let canary = pass_canary(fixture_roster::WINDOWS_PROVIDES_IMPORT_EXIT);
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &canary.join("main.omg"),
        Some("windows_x86_64"),
    ))
    .expect("evaluated DllImport leaf should resolve the Beeper slot");
    assert_eq!(
        checked.selected_program_entry_machine(),
        Some("Main::main"),
        "the authored windows_x86_64 root must select Main::main as its program entry"
    );
    let [via_row] = checked.evaluated_via_bindings().rows() else {
        panic!("the beeper_binding() producer must retain one evaluated `via` row");
    };
    assert_ne!(via_row.via_source_span(), Default::default());
    let beeper_plan = checked
        .selected_provider_plans()
        .plans()
        .iter()
        .find(|plan| plan.schema.trait_name == "Beeper")
        .expect("Beeper must retain its selected evaluated import plan");
    assert_eq!(beeper_plan.provider_type, "");
    assert!(beeper_plan.covers_schema());
    assert_eq!(beeper_plan.rows.len(), 1);
    assert_eq!(beeper_plan.rows[0].method, "beep");
    let effects::provider_plan::ProviderBinding::Import { evaluated } =
        &beeper_plan.rows[0].binding
    else {
        panic!("Beeper must retain one evaluated import binding, never a string-backed row");
    };
    assert_eq!(
        evaluated.locator().target(),
        target::TargetProfile::WindowsX64
    );
    assert_eq!(
        evaluated.locator().locator(),
        &target::ForeignLocatorCandidate::PeByName {
            library: b"msvcrt.dll".to_vec(),
            export: b"abs".to_vec(),
        }
    );
    assert_eq!(
        evaluated.receipt().locator_identity_digest(),
        evaluated.locator().identity_digest()
    );
    assert_ne!(evaluated.receipt().identity_digest(), [0; 32]);
    assert_ne!(
        evaluated.receipt().producer_closure_digest().as_bytes(),
        [0; 32]
    );
    // The retained external-binding row rejoins the same normalized locator;
    // downstream stages must never re-derive it from raw strings.
    let retained = checked
        .external_binding_rows()
        .iter()
        .filter_map(|row| match &row.binding {
            calling_conventions::ExternalBindingKind::Import { locator } => Some(locator),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [retained] = retained.as_slice() else {
        panic!("one retained normalized import row expected");
    };
    assert_eq!(*retained, evaluated.locator());
}

#[cfg(windows)]
#[test]
fn windows_external_import_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::WINDOWS_PROVIDES_IMPORT_EXIT);

    let build_dir =
        std::env::temp_dir().join(format!("omega-external-import-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile_rooted_canary_for_target(&canary, build_dir.clone(), "windows_x86_64")
        .expect("source external import canary should compile from its Windows root");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("source external import canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the authored msvcrt abs import to deliver 42 (exit 70), got {:?}          (71 = result place read ZII or wrong DLL resolved)
stderr:
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// ERGONOMIC-wrapper breadth on windows_x64, second wave: WRAPPER rename (the
// two-path import call resolves each path PER ARGUMENT through the alias
// chain -- param-forwarded literals had no encodable sequence before),
// append (portable flag word 9), read_all, remove -- every step checked
// through its result enum. Windows-gated like the raw canaries.

#[cfg(windows)]
#[test]
fn windows_fs_wrapper_breadth_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::WINDOWS_WRAPPER_BREADTH_EXIT);
    let main_path = canary.join("main.omg");

    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("windows wrapper breadth canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (wrapper rename/append/read_all pass), got {}",
        outcome.exit_code
    );

    let build_dir = std::env::temp_dir().join(format!(
        "omega-fs-win-wrapper-breadth-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("windows wrapper breadth canary should compile from its authored root");

    let output = Command::new(build_dir.join(executable_name()))
        .current_dir(&build_dir)
        .output()
        .expect("windows wrapper breadth canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the wrapper breadth pass (exit 70), got {:?} (71 write_all;          72 rename; 73 old name still opens; 74/75 append open/write; 76-78          read_all count/head/tail; 82 remove)
stderr:
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// REPEATED dir-walk wrapper calls (the cross-context guard-slot regression
// pin): two same-shape read_dir_count calls from different states, each
// counting the same two entries. The second call's walk used to read the
// FIRST expansion's `i`/`path.len` frame slots at its tail-decision guard
// (the guard-operand fallback matched by (machine, state) across call
// contexts), copy nothing, seal "/*", and return Ok(0). Both engines must
// deliver 66 + 2 + 2 = 70. Windows-gated here (this host); the canary itself
// is portable-contract only, so a posix host exercises its own bodies.

#[cfg(windows)]
#[test]
fn repeated_dir_walk_scan_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::REPEATED_DIR_WALK_SCAN_EXIT);
    let main_path = canary.join("main.omg");

    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("repeated dir-walk canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (66 + 2 + 2 across two scans), got {}",
        outcome.exit_code
    );

    let build_dir =
        std::env::temp_dir().join(format!("omega-fs-repeat-scan-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("repeated dir-walk canary should compile from its authored root");

    let output = Command::new(build_dir.join(executable_name()))
        .current_dir(&build_dir)
        .output()
        .expect("repeated dir-walk canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected BOTH scans to count 2 (exit 70 = 66+2+2; 68 would be the second scan reading a stale cross-context guard slot and returning 0), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// Raw-seam BREADTH on windows_x64: the wired-but-previously-unverified msvcrt
// ops beyond the roundtrip -- sync (_commit), seek (_lseeki64), duplicate
// (_dup), set_permissions (_chmod), rename (dest pre-removed: msvcrt rename
// fails if dest exists, unlike POSIX replace), create_dir (_mkdir),
// remove_dir (_rmdir). Windows-gated like the roundtrip canary.

#[cfg(windows)]
#[test]
fn windows_fs_raw_breadth_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::WINDOWS_RAW_BREADTH_EXIT);
    let main_path = canary.join("main.omg");

    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("windows fs breadth canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (virtual-fs breadth pass), got {}",
        outcome.exit_code
    );

    let build_dir =
        std::env::temp_dir().join(format!("omega-fs-win-breadth-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("windows fs breadth canary should compile from its authored root");

    // Run from the temp build dir so probe files/dirs land there, not the repo.
    let output = Command::new(build_dir.join(executable_name()))
        .current_dir(&build_dir)
        .output()
        .expect("windows fs breadth canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the full breadth pass (exit 70), got {:?} (71-74 = \
         create/write/sync/close; 75-78 = reopen/seek/read/seeked bytes; 79-81 = \
         dup/read-through-dup/close dup; 82 chmod; 83 rename; 84 old name still \
         opens; 85 mkdir; 86 remove file; 87 rmdir)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// The callee-entry FIELD WRITE deferral face + same-callee-twice firing:
// `machine make(alive) { self.flag = alive; transition self.flag {..} }`
// called TWICE. (a) The spliced entry Mutation is neither LocalStorage nor
// HostCall, so the Case-B deferral never saw it -- the inline guard was
// emitted BEFORE the field write and read ZII false (wrong arm, silently,
// even single-call). (b) Two calls to the SAME callee splice
// indistinguishable ops; the fire scans now stop at the CONTIGUOUS run's
// end, else call 1's leaf fired after call 2's stores and read call 2's
// flag. Also pins decision 7: the second (bare Dead) result's unnamed
// common field reads 0, not call 1's hp=9 through the reused slot.

#[test]
fn runtime_value_call_entry_field_write_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_VALUE_CALL_ENTRY_FIELD_WRITE_EXIT);
    let main_path = canary.join("main.omg");

    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("entry-field-write canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (Alive{{hp:9}} then bare Dead with hp 0), got {}",
        outcome.exit_code
    );

    let build_dir =
        std::env::temp_dir().join(format!("omega-entry-field-write-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("entry-field-write canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("entry-field-write canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("entry-field-write canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected both same-callee calls to guard on THEIR OWN entry field write \
         (exit 70), got {:?} (71/73 = a guard read the wrong flag; 74 = stale hp \
         through the reused slot)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// The cross-callee collision's INTERNAL-op flavor: two callees sharing let
// names, each DIVIDING by the shared-named let, one caller state. The
// non-guard prelude used to re-emit scalar initializer writes wrong-timed
// with cross-callee resolution -- the duplicated division executed on the
// other callee's ZII operand and #DE-crashed. A negative exit status = the
// A value callee whose lets live in a POST-ENTRY state (entry -> `work`
// (div/mod lets) -> `emit` (mul+add let) -> return). Locks in that the #2B
// splice machinery delivers straight-line lets across non-entry states of an
// inlined value callee -- swap_digits(42) = 24 on both engines.

#[test]
fn runtime_value_callee_post_entry_lets_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_VALUE_CALLEE_POST_ENTRY_LETS_EXIT);
    let main_path = canary.join("main.omg");

    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("post-entry-lets canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 24,
        "interpreter oracle should exit 24 (swap_digits(42)), got {}",
        outcome.exit_code
    );

    let build_dir =
        std::env::temp_dir().join(format!("omega-post-entry-lets-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("post-entry-lets canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("post-entry-lets canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("post-entry-lets canary should run");

    assert_eq!(
        output.status.code(),
        Some(24),
        "expected post-entry-state lets to deliver swap_digits(42)=24, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// A VALUE machine reading self.<array>[<local index>] as its terminal RETURN
// (a bare trailing Expression statement) now hoists the runtime-indexed read, so
// the index materializes and the receiver base is threaded -- fill()=99 (was
// native 0, interp masked it).

#[test]
fn value_machine_self_array_local_index_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::VALUE_MACHINE_SELF_ARRAY_LOCAL_INDEX_EXIT);
    let main_path = canary.join("main.omg");

    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("value-machine self-array index canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 99,
        "interpreter oracle should exit 99 (self.buf[j] in a value machine), got {}",
        outcome.exit_code
    );

    let build_dir =
        std::env::temp_dir().join(format!("omega-value-machine-index-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile(CanaryCompileSpec {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("value-machine self-array index canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("value-machine self-array index canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("value-machine self-array index canary should run");

    assert_eq!(
        output.status.code(),
        Some(99),
        "expected a value machine's self.buf[j] terminal read to deliver 99, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// Sibling of the above for the PURE-CONST BINARY index sub-case: a value machine
// returns `self.buf[4 + 1]` (a machine-owned read indexed by a both-literal
// binary). The frontend leaves both-literal indices for the const fold, but the
// fixed-index resolver only accepted a bare `Integer`, so it fell through and
// read offset 0 natively (interp masked it). `fixed_indexed_target_path_in_table`
// now const-folds the binary. This is the shape the fs stat decode hits
// (`stat_buf[ST_*_OFF + k]` = `stat_buf[24 + 0]` after substitution).

#[test]
fn value_machine_const_index_self_array_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::VALUE_MACHINE_CONST_INDEX_SELF_ARRAY_EXIT);
    let main_path = canary.join("main.omg");

    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("value-machine const-index canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 99,
        "interpreter oracle should exit 99 (self.buf[4 + 1] in a value machine), got {}",
        outcome.exit_code
    );

    let build_dir = std::env::temp_dir().join(format!(
        "omega-value-machine-const-index-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile(CanaryCompileSpec {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("value-machine const-index canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("value-machine const-index canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("value-machine const-index canary should run");

    assert_eq!(
        output.status.code(),
        Some(99),
        "expected a value machine's self.buf[4 + 1] terminal read to deliver 99, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// Hardens the post-entry fold beyond 2 levels: a FOUR-deep chain with an
// intermediate read more than once (`a` -> b,c; `b` -> c,d). proc(5)=30.

#[test]
fn runtime_post_entry_deep_chain_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_POST_ENTRY_DEEP_CHAIN_EXIT);
    let main_path = canary.join("main.omg");

    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("post-entry deep-chain canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 30,
        "interpreter oracle should exit 30 (proc(5) 4-deep chain), got {}",
        outcome.exit_code
    );

    let build_dir =
        std::env::temp_dir().join(format!("omega-post-entry-deep-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("post-entry deep-chain canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("post-entry deep-chain canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("post-entry deep-chain canary should run");

    assert_eq!(
        output.status.code(),
        Some(30),
        "expected the 4-deep post-entry chain with a reused intermediate to deliver 30, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// A value callee whose POST-ENTRY state computes a CHAINED let (`rem` reads the
// prior post-entry let `scaled`). The straight-line initializer write now folds
// prior slot-less locals (fold_straight_line_prior_local_names) like the leaf
// path -- proc(100,7)=2 both engines (was native 0).

#[test]
fn runtime_post_entry_chained_let_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_POST_ENTRY_CHAINED_LET_EXIT);
    let main_path = canary.join("main.omg");

    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("post-entry chained-let canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 2,
        "interpreter oracle should exit 2 (proc(100,7) rem = 100 - (100/7)*7), got {}",
        outcome.exit_code
    );

    let build_dir =
        std::env::temp_dir().join(format!("omega-post-entry-chained-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("post-entry chained-let canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("post-entry chained-let canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("post-entry chained-let canary should run");

    assert_eq!(
        output.status.code(),
        Some(2),
        "expected the post-entry chained let (rem reads prior let scaled) to deliver 2, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// crash is back; 1 = the second callee's chain miscomputed.

#[test]
fn runtime_cross_callee_division_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_CROSS_CALLEE_DIVISION_EXIT);
    let main_path = canary.join("main.omg");

    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("cross-callee division canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (shared-named divisions deliver), got {}",
        outcome.exit_code
    );

    let build_dir =
        std::env::temp_dir().join(format!("omega-cross-division-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("cross-callee division canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("cross-callee division canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("cross-callee division canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected shared-named cross-callee divisions to deliver (exit 70), got {:?}          (negative status = the wrong-timed prelude division #DE is back)
stderr:
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// Two DIFFERENT callees with SAME-NAMED lets (`freq`, `shifted`) value-called
// from ONE caller state: the Mutation fallback write substitutes the callee's
// terminal and must resolve it in the CALLEE's context (branch_key). Resolved
// with the caller's key, the bare name fell through the cross-source-key
// ladder onto the OTHER callee's still-ZII local and clobbered the first
// call's delivered result (exit 1 = first clobbered; 2 = second).

#[test]
fn runtime_cross_callee_let_names_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_CROSS_CALLEE_LET_NAMES_EXIT);
    let main_path = canary.join("main.omg");

    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("cross-callee let-names canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (both callees' results delivered), got {}",
        outcome.exit_code
    );

    let build_dir =
        std::env::temp_dir().join(format!("omega-cross-callee-lets-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("cross-callee let-names canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("cross-callee let-names canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("cross-callee let-names canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected both same-named-let callees to deliver (exit 70), got {:?} \
         (1 = first call clobbered by the other callee's ZII local; 2 = second)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// An inlined callee that TRANSITIONS on a nested sibling call's result must
// read the WRITTEN result, not the pre-store ZII tag: a bare-call binding
// whose local ALSO has storage minted a call-result slot carrying the SAME
// name, so the guarded arms' by-NAME terminal writes landed in whichever
// slot matched first and the guard read the other's ZII. Every leg asserts a
// NON-ZII arm (ZII-coinciding arms pass under both correct and buggy
// emission): saturating_subtract/add exact values, is_greater_than true,
// is_less_than FALSE on a > b.

#[test]
fn runtime_nested_value_call_guard_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_NESTED_VALUE_CALL_GUARD_EXIT);
    let main_path = canary.join("main.omg");

    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("nested-guard canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (all non-ZII arms delivered), got {}",
        outcome.exit_code
    );

    let build_dir = std::env::temp_dir().join(format!("omega-nested-guard-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("nested-guard canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("nested-guard canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("nested-guard canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected every nested-call guard to read the written result (exit 70), got {:?} \
         (1/2 = saturating_subtract Ok arm; 3 = is_greater_than; 4 = is_less_than designed-false; \
         5/6 = saturating_add unclamped arm)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// Same-callee value calls at two sites returning a 16-byte STRUCT: each
// site's terminal construct must write ITS OWN `__call_result` slot. The
// scalar multi-site pins below never caught this because the struct
// DECOMPOSITION strategy re-resolves the slot as a NAME expression -- a
// shared anonymous "__call_result" name sent BOTH sites' member writes into
// the first site's slot, so the second call captured its own slot's ZII zero
// (exit 2 = second zeroed, 1 = first zeroed).

#[test]
fn runtime_two_site_struct_result_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_TWO_SITE_STRUCT_RESULT_EXIT);
    let main_path = canary.join("main.omg");

    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("two-site struct result canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (both struct results delivered), got {}",
        outcome.exit_code
    );

    let build_dir =
        std::env::temp_dir().join(format!("omega-two-site-struct-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("two-site struct result canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("two-site struct result canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("two-site struct result canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected both same-callee struct results delivered per-site (exit 70), got {:?} \
         (1 = first zeroed; 2 = second zeroed -- the shared __call_result name smear)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// Same-callee value calls at MULTIPLE sites per state deliver per-site on the
// dispatch path: two field stores, a discriminating three-site mix (field +
// const-indexed elements), and LC1b (let-then-copy-to-field). These are the
// shapes the retired shared-result-slot fence rejected or silently missed --
// the deferral contiguity work (faces #4/#5) made each call's capture fire at
// its own splice end, so the callee's internal `let` slot no longer smears
// the LAST result across every site.

#[test]
fn runtime_value_call_same_callee_sites_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_VALUE_CALL_SAME_CALLEE_SITES_EXIT);
    let main_path = canary.join("main.omg");

    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("same-callee-sites canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (every site holds its own call's result), got {}",
        outcome.exit_code
    );

    let build_dir =
        std::env::temp_dir().join(format!("omega-same-callee-sites-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("same-callee-sites canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("same-callee-sites canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("same-callee-sites canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected every same-callee call site to hold its own result (exit 70), got {:?} \
         (71/72 = field pair; 73-75 = three-site mix; 76/77 = LC1b let-then-copy)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// Value-machine calls as DIRECT transition arguments deliver per-argument
// (the retired validator stopgap's shape, now WORKING): call+literal,
// same callee twice, and two different callees, each parameter checked.
// Fixed by (a) deferring TransitionArgument-role leaf captures past the
// callee's spliced body ops, (b) pairing leaf expansions with their own
// call op by (role, call_ordinal), and (c) pairing delivery copies with
// the Nth transition-argument call record by rank.

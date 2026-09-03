use super::*;

fn application_build() -> String {
    format!(
        "machine build(builder: &mut Build) {{\n    builder.application(\"cross-target-canary\");\n}}\n"
    )
}

#[test]
fn cross_aarch64_authored_scalar_float_preserves_vector_class() {
    let canary = pass_canary("capabilities/aarch64_scalar_float_import_compile");
    let scratch = std::env::temp_dir().join(format!(
        "omega-aarch64-scalar-float-import-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let src_dir = scratch.join("src");
    let out_dir = scratch.join("out");
    fs::create_dir_all(&src_dir).expect("scratch source directory");
    fs::copy(canary.join("main.omg"), src_dir.join("main.omg")).expect("copy canary");
    fs::write(src_dir.join("build.omg"), application_build())
        .expect("write macos_arm64 build source");

    compile_with_auxiliary_artifacts(CanaryCompileSpec {
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
    let canary = pass_canary("capabilities/aarch64_small_aggregate_import_compile");
    let scratch = std::env::temp_dir().join(format!(
        "omega-aarch64-small-aggregate-import-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let src_dir = scratch.join("src");
    let out_dir = scratch.join("out");
    fs::create_dir_all(&src_dir).expect("scratch source directory");
    fs::copy(canary.join("main.omg"), src_dir.join("main.omg")).expect("copy canary");
    fs::write(src_dir.join("build.omg"), application_build())
        .expect("write macos_arm64 build source");

    compile_with_auxiliary_artifacts(CanaryCompileSpec {
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
    let canary = pass_canary("capabilities/sysv_small_aggregate_import_compile");
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
    let canary = pass_canary("capabilities/aarch64_small_aggregate_stack_import_compile");
    let scratch = std::env::temp_dir().join(format!(
        "omega-aarch64-small-aggregate-stack-import-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let src_dir = scratch.join("src");
    let out_dir = scratch.join("out");
    fs::create_dir_all(&src_dir).expect("scratch source directory");
    fs::copy(canary.join("main.omg"), src_dir.join("main.omg")).expect("copy canary");
    fs::write(src_dir.join("build.omg"), application_build())
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
    let canary = pass_canary("capabilities/aarch64_hfa_stack_import_compile");
    let scratch =
        std::env::temp_dir().join(format!("omega-aarch64-hfa-stack-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let src_dir = scratch.join("src");
    let out_dir = scratch.join("out");
    fs::create_dir_all(&src_dir).expect("scratch source directory");
    fs::copy(canary.join("main.omg"), src_dir.join("main.omg")).expect("copy canary");
    fs::write(src_dir.join("build.omg"), application_build())
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
    let canary = pass_canary("capabilities/aarch64_hfa_result_import_compile");
    let scratch =
        std::env::temp_dir().join(format!("omega-aarch64-hfa-result-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let src_dir = scratch.join("src");
    let out_dir = scratch.join("out");
    fs::create_dir_all(&src_dir).expect("scratch source directory");
    fs::copy(canary.join("main.omg"), src_dir.join("main.omg")).expect("copy canary");
    fs::write(src_dir.join("build.omg"), application_build())
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
    let canary = pass_canary("capabilities/aarch64_erased_hfa_result_import_compile");
    let scratch = std::env::temp_dir().join(format!(
        "omega-aarch64-erased-hfa-result-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let src_dir = scratch.join("src");
    let out_dir = scratch.join("out");
    fs::create_dir_all(&src_dir).expect("scratch source directory");
    fs::copy(canary.join("main.omg"), src_dir.join("main.omg")).expect("copy canary");
    fs::write(src_dir.join("build.omg"), application_build())
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
    let canary = pass_canary("capabilities/aarch64_small_aggregate_result_import_compile");
    let scratch = std::env::temp_dir().join(format!(
        "omega-aarch64-small-aggregate-result-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let src_dir = scratch.join("src");
    let out_dir = scratch.join("out");
    fs::create_dir_all(&src_dir).expect("scratch source directory");
    fs::copy(canary.join("main.omg"), src_dir.join("main.omg")).expect("copy canary");
    fs::write(src_dir.join("build.omg"), application_build())
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
    let canary = pass_canary("capabilities/aarch64_large_aggregate_import_compile");
    let scratch = std::env::temp_dir().join(format!(
        "omega-aarch64-large-aggregate-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let src_dir = scratch.join("src");
    let out_dir = scratch.join("out");
    fs::create_dir_all(&src_dir).expect("scratch source directory");
    fs::copy(canary.join("main.omg"), src_dir.join("main.omg")).expect("copy canary");
    fs::write(src_dir.join("build.omg"), application_build())
        .expect("write macos_arm64 build source");

    compile_with_auxiliary_artifacts(CanaryCompileSpec {
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
    let canary = pass_canary("capabilities/win64_large_aggregate_import_compile");
    let scratch = std::env::temp_dir().join(format!(
        "omega-win64-large-aggregate-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);

    compile_with_auxiliary_artifacts(CanaryCompileSpec {
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
    let canary = pass_canary("capabilities/win64_direct_aggregate_import_compile");
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
    let canary = pass_canary("capabilities/win64_direct_aggregate_result_import_compile");
    let scratch = std::env::temp_dir().join(format!(
        "omega-win64-direct-aggregate-result-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);

    compile_with_auxiliary_artifacts(CanaryCompileSpec {
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
    let canary = pass_canary("capabilities/win64_large_aggregate_result_import_compile");
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
fn cross_win64_scalar_float_import_uses_positional_xmm_and_stack_locations() {
    let canary = pass_canary("capabilities/win64_scalar_float_import_compile");
    let scratch =
        std::env::temp_dir().join(format!("omega-win64-scalar-float-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);

    compile_with_auxiliary_artifacts(CanaryCompileSpec {
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
// `satisfies Beeper::beep via Binding::DllImport("msvcrt.dll", "abs")` leaf
// binds, the import table names msvcrt.dll (the binding, not
// the KERNEL32 catalog default), and abs(-42) delivers 42 through the result
// place (ZII would exit 71). NATIVE-ONLY: no interpreter provider exists for
// authored bindings, so unlike its neighbors this test runs no interp oracle.
#[test]
fn windows_external_import_canary_selects_exact_free_import_plan() {
    let canary = pass_canary("capabilities/windows_provides_import_exit");
    let checked = compile_to_checked(&canary.join("main.omg"), None)
        .expect("free DllImport leaf should resolve the Beeper slot");
    assert_eq!(
        checked.selected_program_entry_machine(),
        None,
        "targetless checking must not select an authored target entry"
    );
    let beeper_plan = checked
        .selected_provider_plans()
        .plans()
        .iter()
        .find(|plan| plan.schema.trait_name == "Beeper")
        .expect("Beeper must retain its selected free DllImport plan");
    assert_eq!(beeper_plan.provider_type, "");
    assert!(beeper_plan.covers_schema());
    assert_eq!(beeper_plan.rows.len(), 1);
    assert_eq!(beeper_plan.rows[0].method, "beep");
    assert!(matches!(
        &beeper_plan.rows[0].binding,
        omega_effects::provider_plan::ProviderBinding::StringBackedImportBootstrap { library, symbol }
            if library == "msvcrt.dll" && symbol == "abs"
    ));
}

#[cfg(windows)]
#[test]
fn windows_external_import_exit_canary_runs() {
    let canary = pass_canary("capabilities/windows_provides_import_exit");

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
    let canary = pass_canary("filesystem/windows_wrapper_breadth_exit");
    let main_path = canary.join("main.omg");

    let checked = compile_to_checked(&main_path, None)
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
    let canary = pass_canary("filesystem/repeated_dir_walk_scan_exit");
    let main_path = canary.join("main.omg");

    let checked = compile_to_checked(&main_path, None)
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
    let canary = pass_canary("filesystem/windows_raw_breadth_exit");
    let main_path = canary.join("main.omg");

    let checked = compile_to_checked(&main_path, None)
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
    let canary = pass_canary("calls/runtime_value_call_entry_field_write_exit");
    let main_path = canary.join("main.omg");

    let checked = compile_to_checked(&main_path, None)
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
    let canary = pass_canary("calls/runtime_value_callee_post_entry_lets_exit");
    let main_path = canary.join("main.omg");

    let checked = compile_to_checked(&main_path, None)
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
    let canary = pass_canary("backend/value_machine_self_array_local_index_exit");
    let main_path = canary.join("main.omg");

    let checked = compile_to_checked(&main_path, None)
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
    let canary = pass_canary("backend/value_machine_const_index_self_array_exit");
    let main_path = canary.join("main.omg");

    let checked = compile_to_checked(&main_path, None)
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
    let canary = pass_canary("calls/runtime_post_entry_deep_chain_exit");
    let main_path = canary.join("main.omg");

    let checked = compile_to_checked(&main_path, None)
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
    let canary = pass_canary("calls/runtime_post_entry_chained_let_exit");
    let main_path = canary.join("main.omg");

    let checked = compile_to_checked(&main_path, None)
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
    let canary = pass_canary("calls/runtime_cross_callee_division_exit");
    let main_path = canary.join("main.omg");

    let checked = compile_to_checked(&main_path, None)
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
    let canary = pass_canary("calls/runtime_cross_callee_let_names_exit");
    let main_path = canary.join("main.omg");

    let checked = compile_to_checked(&main_path, None)
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
    let canary = pass_canary("calls/runtime_nested_value_call_guard_exit");
    let main_path = canary.join("main.omg");

    let checked = compile_to_checked(&main_path, None)
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
    let canary = pass_canary("calls/runtime_two_site_struct_result_exit");
    let main_path = canary.join("main.omg");

    let checked = compile_to_checked(&main_path, None)
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
    let canary = pass_canary("calls/runtime_value_call_same_callee_sites_exit");
    let main_path = canary.join("main.omg");

    let checked = compile_to_checked(&main_path, None)
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
#[test]
fn runtime_value_call_transition_args_exit_canary_runs() {
    let canary = pass_canary("calls/runtime_value_call_transition_args_exit");
    let main_path = canary.join("main.omg");

    let checked = compile_to_checked(&main_path, None)
        .expect("transition-args canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (all six params delivered), got {}",
        outcome.exit_code
    );

    let build_dir =
        std::env::temp_dir().join(format!("omega-transition-args-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("transition-args canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("transition-args canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("transition-args canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected every value-call transition argument to deliver ITS call's \
         result (exit 70), got {:?} (71/72 = call+literal; 73/74 = same-callee \
         pair; 75/76 = different-callee pair)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// The straight-line (guard-free) sibling: done(self.dbl(5), self.dbl(6))
// exits with b, which must hold ITS call's result (12) -- the historical
// bugs delivered call 1's result (10) or ZII 0.
#[test]
fn runtime_value_call_transition_args_straight_line_exit_canary_runs() {
    let canary = pass_canary("calls/runtime_value_call_transition_args_straight_line_exit");
    let main_path = canary.join("main.omg");

    let checked = compile_to_checked(&main_path, None)
        .expect("transition-args straight-line canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 12,
        "interpreter oracle should exit 12 (b = dbl(6)), got {}",
        outcome.exit_code
    );

    let build_dir =
        std::env::temp_dir().join(format!("omega-transition-args-sl-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("transition-args straight-line canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("transition-args straight-line canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("transition-args straight-line canary should run");

    assert_eq!(
        output.status.code(),
        Some(12),
        "expected b = dbl(6) = 12, got {:?} (10 = b read call 1's result; \
         0 = the capture never ran)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// The retired fence's own repro body, now a PASS: a guard-free
// (straight-line-scheduled) state with two same-callee value calls stored
// straight to fields. f=10 + g=12 -> sum exit 22; the historical shared-slot
// bug gave both fields the LAST result (24).
#[test]
fn runtime_value_call_shared_slot_straight_line_exit_canary_runs() {
    let canary = pass_canary("calls/runtime_value_call_shared_slot_straight_line_exit");
    let main_path = canary.join("main.omg");

    let checked = compile_to_checked(&main_path, None)
        .expect("shared-slot straight-line canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 22,
        "interpreter oracle should exit 22 (f=10 + g=12), got {}",
        outcome.exit_code
    );

    let build_dir =
        std::env::temp_dir().join(format!("omega-shared-slot-straight-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("shared-slot straight-line canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("shared-slot straight-line canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("shared-slot straight-line canary should run");

    assert_eq!(
        output.status.code(),
        Some(22),
        "expected f=10 + g=12 (exit 22), got {:?} (24 = both fields read the \
         LAST call's result -- the shared-slot bug is back)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// An enum-attached machine matching BARE `self` (`transition self {
// Signal::Green -> .. }` inside Signal::go_value, called as
// self.s.go_value()): the guard subject resolves to the attached value's
// TAG at the receiver's storage base, threaded to the CALLEE's machine via
// the expansion's branch_key (the caller's machine had resolved `self` to
// the caller's own attached data). Three discriminating cases incl. the
// non-ZII last tag + a bool designed-false leg.
#[test]
fn runtime_enum_self_method_exit_canary_runs() {
    let canary = pass_canary("calls/runtime_enum_self_method_exit");
    let main_path = canary.join("main.omg");

    let checked = compile_to_checked(&main_path, None)
        .expect("enum-self-method canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (all enum-self legs discriminate), got {}",
        outcome.exit_code
    );

    let build_dir =
        std::env::temp_dir().join(format!("omega-enum-self-method-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("enum-self-method canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("enum-self-method canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("enum-self-method canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected every bare-self enum-method leg to discriminate (exit 70), got {:?} \
         (71/72/73 = Green/Amber/Red legs; 74 = is_green designed-false took the \
         true arm)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// Dispatch-bodied value calls deliver into every result position: a STRUCT
// result assigned straight to a FIELD (stored 0 until the Mutation fire
// site gave field assignments a flush point), and a FREE machine with a
// runtime-selected branch bound to a let (returned garbage before). Arm
// bodies are PURE -- the effectful-arm shape is fenced separately
// (calls/value_call_effectful_arm_rejected).
#[test]
fn runtime_value_call_dispatch_results_exit_canary_runs() {
    let canary = pass_canary("calls/runtime_value_call_dispatch_results_exit");
    let main_path = canary.join("main.omg");

    let checked = compile_to_checked(&main_path, None)
        .expect("dispatch-results canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (struct-to-field + free-pick legs), got {}",
        outcome.exit_code
    );

    let build_dir =
        std::env::temp_dir().join(format!("omega-dispatch-results-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("dispatch-results canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("dispatch-results canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("dispatch-results canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected dispatch-bodied value-call results to deliver (exit 70), got {:?} \
         (71/72 = struct-to-field components; 73/74 = free-pick false/true branch)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// An inline arm guard over a callee SLICE PARAM (`path.len > 3`) lowers via
// leaf-binding resolution: the caller's LITERAL argument substitutes into the
// guard, `.len` folds to the byte length, and the ordered comparison decides
// each arm statically -- one arm per call site, and two sites hit OPPOSITE
// arms of the same callee. The re-entrant sibling (arm targeting a
// `terminates` walk) was fenced until call-with-return landed (now the
// promoted calls/runtime_inline_recursive_walk_exit family):
// these folds and that fence landed together and must stay together.
#[test]
fn runtime_value_call_literal_len_arm_guard_exit_canary_runs() {
    let canary = pass_canary("calls/runtime_value_call_literal_len_arm_guard_exit");
    let main_path = canary.join("main.omg");

    let checked = compile_to_checked(&main_path, None)
        .expect("literal-len arm-guard canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (opposite arms across two call sites), got {}",
        outcome.exit_code
    );

    let build_dir = std::env::temp_dir().join(format!(
        "omega-literal-len-arm-guard-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("literal-len arm-guard canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("literal-len arm-guard canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("literal-len arm-guard canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected literal-len arm guards to select one arm per site (exit 70), got {:?} \
         (71 = long-path site missed the big arm; 72 = short-path site missed small)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// A scalar value-call compared to an integer literal DIRECTLY in a guard
// subject discriminates (the historical always-true face): the syntax
// lowering hoists the call into a shared let temp typed from the callee's
// declared return, and the guard compares the local. Both designed-false
// legs and the designed-true leg are checked; the effectful-subject
// single-evaluation tripwire pins that match-over-call arms share ONE temp
// (per-arm temps re-ran the callee once per attempted arm).
#[test]
fn runtime_value_call_guard_subject_exit_canary_runs() {
    let canary = pass_canary("calls/runtime_value_call_guard_subject_exit");
    let main_path = canary.join("main.omg");

    let checked = compile_to_checked(&main_path, None)
        .expect("guard-subject canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (all three guard legs discriminate), got {}",
        outcome.exit_code
    );

    let build_dir =
        std::env::temp_dir().join(format!("omega-guard-subject-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("guard-subject canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("guard-subject canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("guard-subject canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected every value-call guard leg to DISCRIMINATE (exit 70), got {:?} \
         (71 = designed-false Equal took the true arm -- the always-true bug is \
         back; 72 = designed-true failed; 73 = NotEqual designed-false)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_effectful_guard_local_and_self_terminal_exit_canary_runs() {
    let canary = pass_canary("calls/runtime_effectful_guard_local_and_self_terminal_exit");
    let main_path = canary.join("main.omg");

    let checked = compile_to_checked(&main_path, None)
        .expect("effectful guard/local and self-terminal canary should reach checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter should return both call values and execute each call once, got {}",
        outcome.exit_code
    );

    let build_dir = std::env::temp_dir().join(format!(
        "omega-effectful-guard-local-self-terminal-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("effectful guard/local and self-terminal canary should compile");

    let executable = compilation.checked_native_executable_path().expect(
        "effectful guard/local and self-terminal canary should retain its executable receipt",
    );
    let output = Command::new(executable)
        .output()
        .expect("effectful guard/local and self-terminal canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "native execution should return both call values and execute each call once, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_guarded_effectful_transition_argument_exit_canary_runs() {
    let canary = pass_canary("calls/runtime_guarded_effectful_transition_argument_exit");
    let main_path = canary.join("main.omg");

    let checked = compile_to_checked(&main_path, None)
        .expect("guarded effectful transition-argument canary should reach checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter should preserve hot/cold execution plus ordered parameter/local delivery, got {}",
        outcome.exit_code
    );

    let build_dir = std::env::temp_dir().join(format!(
        "omega-guarded-effectful-transition-argument-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("guarded effectful transition-argument canary should compile");

    let executable = compilation.checked_native_executable_path().expect(
        "guarded effectful transition-argument canary should retain its executable receipt",
    );
    let output = Command::new(executable)
        .output()
        .expect("guarded effectful transition-argument canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "native execution should preserve hot/cold execution plus ordered parameter/local delivery, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// Deferral face #5: a NESTED value call inside the callee's entry
// (`self.flag = self.helper.check(1)` in Probe::make, guarded on flag,
// through an outer value call). The nested callee splices a THIRD
// source_key between the middle callee's ops, so defer/fire scans that
// stopped at the first foreign key never saw the flag mutation -- the
// outer leaf fired at the StateCall and computed the result from ZII
// flag=false. The splice run ends at the CALLER's next own op.
#[test]
fn runtime_value_call_nested_entry_call_exit_canary_runs() {
    let canary = pass_canary("calls/runtime_value_call_nested_entry_call_exit");
    let main_path = canary.join("main.omg");

    let checked = compile_to_checked(&main_path, None)
        .expect("nested-entry-call canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (both nested-entry shapes deliver), got {}",
        outcome.exit_code
    );

    let build_dir =
        std::env::temp_dir().join(format!("omega-nested-entry-call-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("nested-entry-call canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("nested-entry-call canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("nested-entry-call canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected both callee entries with nested value calls to guard on the \
         DELIVERED flag (exit 70), got {:?} (71 = nested-only entry read ZII; \
         72 = stores-around-the-nested-call variant read ZII)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// SHARED-NAME payload fields across variants through a value-call leaf
// terminal: `amount` in Deposit(amount) AND Transfer(to, amount). The
// leaf-path per-field decomposition passed case_variant: None, so `amount`
// resolved the FIRST variant's offset and Transfer's payload landed wrong
// (exit 72, silent -- the #38 collision class, previously fixed only on the
// mutation path). The write now tags payload fields with the constructed
// variant, matching the destructure side.
#[test]
fn runtime_value_call_shared_payload_name_exit_canary_runs() {
    let canary = pass_canary("calls/runtime_value_call_shared_payload_name_exit");
    let main_path = canary.join("main.omg");

    let checked = compile_to_checked(&main_path, None)
        .expect("shared-payload-name canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (Transfer{{to:42, amount:99}} delivered), got {}",
        outcome.exit_code
    );

    let build_dir =
        std::env::temp_dir().join(format!("omega-shared-payload-name-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("shared-payload-name canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("shared-payload-name canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("shared-payload-name canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected Transfer's shared-name payload to resolve ITS variant's offsets \
         (exit 70), got {:?} (72/73 = a field landed at Deposit's offset)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// A big multi-field StructLiteral payload through a value-call result slot with
// one CAST-valued field (`mode: mode as u32` -- the fs metadata_path shape,
// TASKS_FS.md blocker #2A). The leaf-path scalar write cascade had no convert
// arm, so the cast field silently dropped while its 15 siblings landed (exit 74).
#[test]
fn runtime_value_call_struct_payload_cast_field_exit_canary_runs() {
    let canary = pass_canary("calls/runtime_value_call_struct_payload_cast_field_exit");
    let main_path = canary.join("main.omg");

    // Interpreter oracle first: it must agree the exit is 70.
    let checked = compile_to_checked(&main_path, None)
        .expect("value-call cast-field payload canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (full 16-field payload incl. the cast field), got {}",
        outcome.exit_code
    );

    let build_dir = std::env::temp_dir().join(format!(
        "omega-value-call-cast-field-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("value-call cast-field payload canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("value-call cast-field payload canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("value-call cast-field payload canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the full 16-field payload incl. the cast-valued mode field (exit 70), \
         got {:?} (74 = the cast field arrived ZII)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_branch_leaf_multiple_named_conversion_exit_canary_runs() {
    let canary = pass_canary("calls/runtime_branch_leaf_multiple_named_conversion_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-branch-leaf-multiple-named-conversion-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("branch-leaf multiple named-conversion canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("branch-leaf named-conversion canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("branch-leaf multiple named-conversion canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected both conversion results to materialize before the branch-local binary initializer, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// A value-call whose ENTRY host call writes self state (read_line into the
// carrier) and whose leaf arms build payloads FROM that state: the Ok arm's
// StructLiteral takes `len` from the host-written carrier; the Error arm's
// `kind` comes from a NESTED value-call guarding on it (the fs wrapper's
// `let kind = self.last_error()` shape). Regression pin for TASKS_FS.md
// blocker #2B: the arm statements (straight-line expansion) used to be emitted
// ABOVE the entry host call, so the terminal copied pre-call ZII state --
// right tag, zero payload. Both stdin legs run interpreter-first.
#[test]
fn value_call_entry_host_state_payload_canary_runs() {
    let canary = run_canary("value_call_entry_host_state_payload");
    let main_path = canary.join("main.omg");

    let checked = compile_to_checked(&main_path, None)
        .expect("entry-host-state payload canary should compile to checked trees");
    for (stdin, expected) in [(&b"ok\n"[..], 70), (&b"no\n"[..], 75)] {
        let outcome = interpret(&checked, stdin);
        assert_eq!(
            outcome.exit_code,
            expected,
            "interpreter oracle should exit {expected} for stdin {:?}, got {}",
            String::from_utf8_lossy(stdin),
            outcome.exit_code
        );
    }

    let build_dir = std::env::temp_dir().join(format!(
        "omega-entry-host-state-payload-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile(CanaryCompileSpec {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("entry-host-state payload canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("entry-host-state payload canary should retain its executable receipt");
    for (stdin, expected) in [(&b"ok\n"[..], 70), (&b"no\n"[..], 75)] {
        let mut child = Command::new(executable)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("entry-host-state payload canary should start");
        child
            .stdin
            .as_mut()
            .expect("stdin should be piped")
            .write_all(stdin)
            .expect("entry-host-state payload input should be written");
        let output = child
            .wait_with_output()
            .expect("entry-host-state payload canary should finish");
        assert_eq!(
            output.status.code(),
            Some(expected),
            "expected exit {expected} for stdin {:?} (72 = Ok len read the pre-host-call ZII \
             carrier; 76 = Error kind lost through the nested value-call), got {:?}\nstderr:\n{}",
            String::from_utf8_lossy(stdin),
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let _ = fs::remove_dir_all(&build_dir);
}

// #66 carrier command-loop with a health gate: each iteration checks health, reads
// a line into a `[u8; 16]` carrier, resolves a Command, and loops until `quit`.
#[test]
fn contained_health_loop_command_branch_carrier_canary_runs() {
    let canary = run_canary("contained_health_loop_command_branch");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-contained-health-loop-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile(CanaryCompileSpec {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("carrier health-loop canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("carrier health-loop canary should retain its executable receipt");
    let mut child = Command::new(executable)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("carrier health-loop canary should start");
    child
        .stdin
        .as_mut()
        .expect("stdin should be piped")
        .write_all(b"look\nzzz\nquit\n")
        .expect("carrier health-loop input should be written");
    let output = child
        .wait_with_output()
        .expect("carrier health-loop canary should finish");

    assert_eq!(
        output.status.code(),
        Some(0),
        "expected carrier health-loop canary to exit 0, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "look\ninvalid\n",
        "expected the health-gated loop to resolve each command (Look, Invalid) then quit"
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// #66 carrier sequential reads: two `read_line`s into the same `[u8; 64]` carrier,
// each echoed -- the second read must overwrite the first line's bytes + length.
#[test]
fn runtime_stdin_line_buffering_carrier_canary_runs() {
    let canary = pass_canary("text/runtime_stdin_line_buffering_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-line-buffering-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("carrier line buffering canary should compile from its authored root");

    let executable = compilation
        .checked_native_executable_path()
        .expect("carrier line buffering canary should retain its executable receipt");
    let mut child = Command::new(executable)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("carrier line buffering canary should start");
    child
        .stdin
        .as_mut()
        .expect("stdin should be piped")
        .write_all(b"first\nsecond\n")
        .expect("carrier line buffering input should be written");
    let output = child
        .wait_with_output()
        .expect("carrier line buffering canary should finish");

    assert_eq!(
        output.status.code(),
        Some(0),
        "expected carrier line buffering canary to exit 0, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "first\nsecond\n",
        "expected each carrier read_line to echo its own line, the second overwriting the first"
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// #66 carrier stdin round-trip: `read_line` into a `[u8; 64] in Utf8` carrier
// (stdin straight into the inline bytes + len), then `write_line` the carrier back.
#[test]
fn runtime_text_storage_carrier_canary_runs() {
    let canary = pass_canary("text/runtime_text_storage");
    let build_dir =
        std::env::temp_dir().join(format!("omega-runtime-text-storage-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation =
        compile_rooted_canary_for_native_host_with_auxiliary_artifacts(&canary, build_dir.clone())
            .expect("carrier text storage canary should compile from its authored root");

    let report = fs::read_to_string(build_dir.join("backend_report.txt"))
        .expect("carrier text storage backend report should exist");
    assert!(
        report.contains("-> carrier") && report.contains("cap 64"),
        "carrier read must use the destination's 64-byte capacity, not the legacy String scratch capacity:\n{report}"
    );

    let executable = compilation
        .checked_native_executable_path()
        .expect("carrier text storage canary should retain its executable receipt");
    let mut child = Command::new(executable)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("carrier text storage canary should start");
    child
        .stdin
        .as_mut()
        .expect("stdin should be piped")
        .write_all(b"echo me\n")
        .expect("carrier text storage input should be written");
    let output = child
        .wait_with_output()
        .expect("carrier text storage canary should finish");

    assert_eq!(
        output.status.code(),
        Some(0),
        "expected carrier text storage canary to exit 0, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "> echo me\n",
        "expected the carrier read_line to round-trip the input line back through write_line"
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_stderr_write_exit_canary_runs() {
    // The `write_error` host capability mirrors `write` but targets the stderr
    // handle (GetStdHandle(-12)) instead of stdout (-11). The program must emit
    // its text on stderr only, leaving stdout empty, and exit with the requested
    // code.
    let canary = pass_canary("text/runtime_stderr_write_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-runtime-stderr-write-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime stderr write canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("runtime stderr write canary should retain its executable receipt");
    let output = Command::new(executable)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("runtime stderr write canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected runtime stderr write canary to exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        "hello-stderr\n",
        "expected runtime stderr write canary to emit its text on stderr"
    );
    assert!(
        output.stdout.is_empty(),
        "expected runtime stderr write canary to leave stdout empty, got {:?}",
        String::from_utf8_lossy(&output.stdout)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_stdin_line_buffering_exit_canary_runs() {
    let canary = pass_canary("text/runtime_stdin_line_buffering_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-stdin-line-buffering-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime stdin line buffering canary should compile from its authored root");

    let executable = compilation
        .checked_native_executable_path()
        .expect("runtime stdin line buffering canary should retain its executable receipt");
    let mut child = Command::new(executable)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("runtime stdin line buffering canary should start");
    child
        .stdin
        .as_mut()
        .expect("stdin should be piped")
        .write_all(b"hello\nworld\n")
        .expect("stdin line buffering input should be written");
    let output = child
        .wait_with_output()
        .expect("runtime stdin line buffering canary should finish");

    assert_eq!(
        output.status.code(),
        Some(0),
        "expected runtime stdin line buffering canary to exit 0, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "hello\nworld\n",
        "expected runtime stdin line buffering canary to preserve one logical line per read"
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_stdin_crlf_line_read_canary_runs() {
    // Windows terminals (and piped CRLF input) terminate each line with "\r\n".
    // The line reader must treat that as ONE terminator: a '\r'-ended line must
    // not leave the trailing '\n' to surface as a phantom empty line on the next
    // read_line. Reuses the two-read echo sample; with the bug the second read
    // returns "" and the output is "hello\n\n".
    let canary = pass_canary("text/runtime_stdin_line_buffering_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-stdin-crlf-line-read-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime stdin crlf line read canary should compile from its authored root");

    let executable = compilation
        .checked_native_executable_path()
        .expect("runtime stdin CRLF canary should retain its executable receipt");
    let mut child = Command::new(executable)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("runtime stdin crlf line read canary should start");
    child
        .stdin
        .as_mut()
        .expect("stdin should be piped")
        .write_all(b"hello\r\nworld\r\n")
        .expect("stdin crlf line read input should be written");
    let output = child
        .wait_with_output()
        .expect("runtime stdin crlf line read canary should finish");

    assert_eq!(
        output.status.code(),
        Some(0),
        "expected runtime stdin crlf line read canary to exit 0, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "hello\nworld\n",
        "expected CRLF input to read two clean lines (no phantom empty line from the trailing \\n)"
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_slice_alias_indexed_string_field_concat_exit_canary_runs() {
    let canary = pass_canary("text/runtime_slice_alias_indexed_string_field_concat_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-slice-alias-indexed-string-field-concat-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone()).expect(
        "runtime slice alias indexed string field concat canary should compile from its authored root",
    );

    let executable = compilation.checked_native_executable_path().expect(
        "runtime slice alias indexed string field concat canary should retain its executable receipt",
    );
    let output = Command::new(executable)
        .output()
        .expect("runtime slice alias indexed string field concat canary should run");

    assert_eq!(
        output.status.code(),
        Some(77),
        "expected runtime slice alias indexed string field concat canary to preserve alias-indexed string writes and exit 77, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_slice_indexed_string_guard_exit_canary_runs() {
    // A slice-indexed String field compared against a literal in guard
    // position: an EMPTY (default-zeroed) field takes the false arm, the
    // matching field takes the true arm, and a same-length differing field
    // takes the false arm. Exit 70 only when all three behave (the lying-guard
    // regression took the true arm unconditionally, exiting 71).
    let canary = pass_canary("text/runtime_slice_indexed_string_guard_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-slice-indexed-string-guard-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime slice indexed string guard canary should compile from its authored root");

    let executable = compilation
        .checked_native_executable_path()
        .expect("runtime slice indexed string guard canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("runtime slice indexed string guard canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected slice-indexed String guard compares to be content compares (empty != literal, match == literal, same-length differ != literal) and exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_slice_machine_indexed_string_guard_exit_canary_runs() {
    let canary = pass_canary("text/runtime_slice_machine_indexed_string_guard_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-slice-machine-indexed-string-guard-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone()).expect(
        "runtime slice machine-indexed string guard canary should compile from its authored root",
    );

    let executable = compilation.checked_native_executable_path().expect(
        "runtime slice machine-indexed string guard canary should retain its executable receipt",
    );
    let output = Command::new(executable)
        .output()
        .expect("runtime slice machine-indexed string guard canary should run");

    assert_eq!(
        output.status.code(),
        Some(72),
        "expected cross-region slice String writes and guards to preserve exact content and exit 72, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_string_field_literal_guard_exit_canary_runs() {
    // The storage-place sibling of the slice-indexed shape: a machine-owned
    // String field guard-compared against a literal (empty field takes the
    // false arm; written field takes the true arm).
    let canary = pass_canary("text/runtime_string_field_literal_guard_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-string-field-literal-guard-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime string field literal guard canary should compile from its authored root");

    let executable = compilation
        .checked_native_executable_path()
        .expect("runtime string field literal guard canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("runtime string field literal guard canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected machine String field guard compares against literals to be content compares and exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_local_array_indexed_string_guard_exit_canary_runs() {
    // The frame-BASE-indexed sibling of the slice-indexed shape: a LOCAL
    // inline fixed array's element String field guard-compared against a
    // literal at a runtime index (empty field takes the false arm, matching
    // takes true, same-length-differing takes false; the lying-guard
    // regression selected no compare and took the true arm unconditionally).
    let canary = pass_canary("text/runtime_local_array_indexed_string_guard_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-local-array-indexed-string-guard-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone()).expect(
        "runtime local array indexed string guard canary should compile from its authored root",
    );

    let executable = compilation.checked_native_executable_path().expect(
        "runtime local array indexed string guard canary should retain its executable receipt",
    );
    let output = Command::new(executable)
        .output()
        .expect("runtime local array indexed string guard canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected local-array-indexed String guard compares to be content compares (empty != literal, match == literal, same-length differ != literal) and exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_local_array_indexed_string_field_concat_exit_canary_runs() {
    let canary = pass_canary("text/runtime_local_array_indexed_string_field_concat_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-local-array-indexed-string-field-concat-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone()).expect(
        "runtime local-array-indexed string field concat canary should compile from its authored root",
    );

    let executable = compilation.checked_native_executable_path().expect(
        "runtime local-array-indexed string field concat canary should retain its executable receipt",
    );
    let output = Command::new(executable)
        .output()
        .expect("runtime local-array-indexed string field concat canary should run");
    assert_eq!(
        output.status.code(),
        Some(89),
        "expected frame-base-indexed text assembly to preserve `prefix omega!` and exit 89, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_slice_fixed_indexed_string_guard_exit_canary_runs() {
    // The CONSTANT-index sibling of the slice-indexed shape: a slice
    // element's String field guard-compared against a literal at a literal
    // index (`room_slice[0]`), lowering through the fixed-indexed place
    // (descriptor deref + folded constant offset). Same three regimes.
    let canary = pass_canary("text/runtime_slice_fixed_indexed_string_guard_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-slice-fixed-indexed-string-guard-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone()).expect(
        "runtime slice fixed indexed string guard canary should compile from its authored root",
    );

    let executable = compilation.checked_native_executable_path().expect(
        "runtime slice fixed indexed string guard canary should retain its executable receipt",
    );
    let output = Command::new(executable)
        .output()
        .expect("runtime slice fixed indexed string guard canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected fixed-indexed String guard compares to be content compares (empty != literal, match == literal, same-length differ != literal) and exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_pointee_string_guard_exit_canary_runs() {
    // The POINTEE sibling: a String field read through a `&mut Room` pointer
    // slot (local alias AND called-machine parameter), guard-compared against
    // a literal. The pre-fix regression here was an always-unequal compare:
    // the place resolved to the pointer slot's raw bytes rather than the
    // pointee's descriptor, so the MATCH regime took the false arm.
    let canary = pass_canary("text/runtime_pointee_string_guard_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-pointee-string-guard-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime pointee string guard canary should compile from its authored root");

    let executable = compilation
        .checked_native_executable_path()
        .expect("runtime pointee string guard canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("runtime pointee string guard canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected pointee String guard compares to be content compares (empty != literal, match == literal, same-length differ != literal, parameter shape included) and exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_mutable_carrier_parameter_concat_exit_canary_runs() {
    let canary = pass_canary("text/runtime_mutable_string_parameter_concat_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-mutable-carrier-parameter-concat-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone()).expect(
        "runtime mutable carrier parameter concat canary should compile from its authored root",
    );

    let executable = compilation.checked_native_executable_path().expect(
        "runtime mutable carrier parameter concat canary should retain its executable receipt",
    );
    let output = Command::new(executable)
        .output()
        .expect("runtime mutable carrier parameter concat canary should run");

    assert_eq!(
        output.status.code(),
        Some(77),
        "expected runtime mutable carrier parameter concat canary to preserve pointee carrier writes and exit 77, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_mutable_string_parameter_concat_write_line_canary_runs() {
    let canary = pass_canary("text/runtime_mutable_string_parameter_concat_write_line");
    let main_path = canary.join("main.omg");
    let checked = compile_to_checked(&main_path, None)
        .expect("runtime mutable carrier parameter concat/write canary should check");
    let interpreted = interpret(&checked, &[]);
    assert_eq!(interpreted.error, None);
    assert_eq!(interpreted.exit_code, 77);
    assert_eq!(interpreted.stdout, b"prefix omega\n".to_vec());
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-mutable-string-parameter-concat-write-line-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone()).expect(
        "runtime mutable carrier parameter concat write_line canary should compile from its authored root",
    );

    let executable = compilation.checked_native_executable_path().expect(
        "runtime mutable carrier parameter concat write_line canary should retain its executable receipt",
    );
    let output = Command::new(executable)
        .output()
        .expect("runtime mutable carrier parameter concat write_line canary should run");

    assert_eq!(
        output.status.code(),
        Some(77),
        "expected runtime mutable carrier parameter concat write_line canary to print generated pointee text and exit 77, got {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"prefix omega\n");

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_mutable_string_parameter_wrapped_concat_write_line_canary_runs() {
    let canary = pass_canary("text/runtime_mutable_string_parameter_wrapped_concat_write_line");
    let main_path = canary.join("main.omg");
    let checked = compile_to_checked(&main_path, None)
        .expect("runtime wrapped mutable carrier concat/write canary should check");
    let interpreted = interpret(&checked, &[]);
    assert_eq!(interpreted.error, None);
    assert_eq!(interpreted.exit_code, 77);
    assert_eq!(interpreted.stdout, b"prefix omega done\n".to_vec());
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-mutable-string-parameter-wrapped-concat-write-line-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone()).expect(
        "runtime wrapped mutable carrier concat write_line canary should compile from its authored root",
    );

    let executable = compilation.checked_native_executable_path().expect(
        "runtime wrapped mutable carrier concat write_line canary should retain its executable receipt",
    );
    let output = Command::new(executable)
        .output()
        .expect("runtime wrapped mutable carrier concat write_line canary should run");

    assert_eq!(
        output.status.code(),
        Some(77),
        "expected wrapped mutable carrier concat write_line canary to print generated pointee text and exit 77, got {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"prefix omega done\n");

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_mutable_struct_carrier_field_copy_concat_exit_canary_runs() {
    let canary = pass_canary("text/runtime_mutable_struct_string_field_copy_concat_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-mutable-struct-carrier-field-copy-concat-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone()).expect(
        "runtime mutable struct carrier field copy concat canary should compile from its authored root",
    );

    let executable = compilation.checked_native_executable_path().expect(
        "runtime mutable struct carrier field copy concat canary should retain its executable receipt",
    );
    let output = Command::new(executable)
        .output()
        .expect("runtime mutable struct carrier field copy concat canary should run");

    assert_eq!(
        output.status.code(),
        Some(77),
        "expected runtime mutable struct carrier field copy concat canary to preserve copied carrier fields and exit 77, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_local_struct_string_field_concat_exit_canary_runs() {
    let canary = pass_canary("text/runtime_local_struct_string_field_concat_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-local-struct-string-field-concat-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime local struct string field concat canary should compile");

    let executable = compilation.checked_native_executable_path().expect(
        "runtime local struct string field concat canary should retain its executable receipt",
    );
    let output = Command::new(executable)
        .output()
        .expect("runtime local struct string field concat canary should run");

    assert_eq!(
        output.status.code(),
        Some(188),
        "expected generated string concat to append a copied local struct string field and exit 188, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_string_stored_suffix_exit_canary_runs() {
    let canary = pass_canary("text/runtime_string_stored_suffix_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-string-stored-suffix-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime string stored-suffix canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("runtime string stored-suffix canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("runtime string stored-suffix canary should run");

    assert_eq!(
        output.status.code(),
        Some(193),
        "expected segmented stored-suffix text assembly to exit 193, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_lookup_struct_field_concat_exit_canary_runs() {
    let canary = pass_canary("text/runtime_lookup_struct_field_concat_exit");
    let main_path = canary.join("main.omg");
    let checked = compile_to_checked(&main_path, None)
        .expect("runtime lookup carrier concat should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(outcome.exit_code, 190, "interpreter lookup carrier concat");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-lookup-struct-field-concat-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime lookup struct field concat canary should compile from its authored root");

    let executable = compilation
        .checked_native_executable_path()
        .expect("runtime lookup struct field concat canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("runtime lookup struct field concat canary should run");

    assert_eq!(
        output.status.code(),
        Some(190),
        "expected lookup-filled local struct field to feed generated string concat and exit 190, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_large_lookup_struct_field_concat_exit_canary_runs() {
    let canary = pass_canary("text/runtime_large_lookup_struct_field_concat_exit");
    let main_path = canary.join("main.omg");
    let checked = compile_to_checked(&main_path, None)
        .expect("large lookup carrier concat should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 192,
        "interpreter large lookup carrier concat"
    );
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-large-lookup-struct-field-concat-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone()).expect(
        "runtime large lookup struct field concat canary should compile from its authored root",
    );

    let executable = compilation.checked_native_executable_path().expect(
        "runtime large lookup struct field concat canary should retain its executable receipt",
    );
    let output = Command::new(executable)
        .output()
        .expect("runtime large lookup struct field concat canary should run");

    assert_eq!(
        output.status.code(),
        Some(192),
        "expected large-frame lookup-filled local struct field concat to exit 192, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_large_room_lookup_struct_field_concat_exit_canary_runs() {
    let canary = pass_canary("text/runtime_large_room_lookup_struct_field_concat_exit");
    let main_path = canary.join("main.omg");
    let checked = compile_to_checked(&main_path, None)
        .expect("large room lookup carrier concat should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 200,
        "interpreter large-room lookup carrier concat"
    );
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-large-room-lookup-struct-field-concat-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone()).expect(
        "runtime large room lookup struct field concat canary should compile from its authored root",
    );

    let executable = compilation.checked_native_executable_path().expect(
        "runtime large room lookup struct field concat canary should retain its executable receipt",
    );
    let output = Command::new(executable)
        .output()
        .expect("runtime large room lookup struct field concat canary should run");

    assert_eq!(
        output.status.code(),
        Some(200),
        "expected large indexed room copy to preserve label for generated concat and exit 200, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_call_argument_struct_string_field_slice_alias_exit_canary_runs() {
    let canary = pass_canary("text/runtime_call_argument_struct_string_field_slice_alias_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-call-argument-struct-string-slice-alias-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone()).expect(
        "runtime call argument struct string slice alias canary should compile from its authored root",
    );

    let executable = compilation.checked_native_executable_path().expect(
        "runtime call argument struct string slice alias canary should retain its executable receipt",
    );
    let output = Command::new(executable)
        .output()
        .expect("runtime call argument struct string slice alias canary should run");

    assert_eq!(
        output.status.code(),
        Some(77),
        "expected runtime call argument string copied through slice-alias struct field to exit 77, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn bounded_carrier_regressions_compile_on_aarch64() {
    for (index, canary_name) in [
        "text/runtime_mutable_string_parameter_concat_exit",
        "text/runtime_mutable_struct_string_field_copy_concat_exit",
        "text/runtime_mutable_string_parameter_concat_write_line",
        "text/runtime_mutable_string_parameter_wrapped_concat_write_line",
        "text/runtime_mutable_struct_string_field_copy_concat_write_line",
        "dungeon/runtime_clear_carve_render_string_fields_exit",
        "dungeon/runtime_full_level_wrapper_lookup_string_field_exit",
        "calls/mutable_output_host_call",
        "text/runtime_text_storage",
        "text/runtime_stdin_line_buffering_exit",
        "text/runtime_stdin_command_branch_exit",
        "dungeon/runtime_ordered_room_dispatch_loop_exit",
        "dungeon/runtime_ordered_room_dispatch_large_machine_exit",
        "dungeon/runtime_ordered_room_dispatch_real_show_states_exit",
        "text/runtime_chained_string_append_exit",
        "text/runtime_machine_string_append_in_place_exit",
    ]
    .into_iter()
    .enumerate()
    {
        let canary = pass_canary(canary_name);
        let scratch = std::env::temp_dir().join(format!(
            "omega-carrier-place-arm64-{}-{index}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        let source = scratch.join("src");
        fs::create_dir_all(&source).expect("AArch64 carrier scratch source directory");
        fs::copy(canary.join("main.omg"), source.join("main.omg"))
            .expect("copy carrier canary into AArch64 scratch source");
        fs::write(source.join("build.omg"), application_build())
            .expect("write AArch64 carrier build source");
        compile(CanaryCompileSpec {
            root_path: source.join("main.omg"),
            build_dir: Some(scratch.join("out")),
            target_name: Some("linux_arm64".into()),
            product: CanaryCompileProduct::Check,
        })
        .unwrap_or_else(|diagnostics| {
            panic!(
                "AArch64 carrier place/view/return lowering should compile for {canary_name}:\n{}",
                diagnostics
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        });
        let _ = fs::remove_dir_all(&scratch);
    }
}

#[test]
fn runtime_mutable_struct_string_field_copy_concat_write_line_canary_runs() {
    let canary = pass_canary("text/runtime_mutable_struct_string_field_copy_concat_write_line");
    let main_path = canary.join("main.omg");
    let checked = compile_to_checked(&main_path, None)
        .expect("runtime mutable struct carrier field copy/write canary should check");
    let interpreted = interpret(&checked, &[]);
    assert_eq!(interpreted.error, None);
    assert_eq!(interpreted.exit_code, 77);
    assert_eq!(interpreted.stdout, b"prefix omega done\n".to_vec());
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-mutable-struct-string-field-copy-concat-write-line-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone()).expect(
        "runtime mutable struct carrier field copy concat write_line canary should compile from its authored root",
    );

    let executable = compilation.checked_native_executable_path().expect(
        "runtime mutable struct carrier field copy concat write_line canary should retain its executable receipt",
    );
    let output = Command::new(executable)
        .output()
        .expect("runtime mutable struct carrier field copy concat write_line canary should run");

    assert_eq!(
        output.status.code(),
        Some(77),
        "expected runtime mutable struct carrier field copy concat write_line canary to print copied-field text and exit 77, got {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"prefix omega done\n");

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_machine_owned_indexed_integer_write_exit_canary_runs() {
    let canary = pass_canary("storage/runtime_machine_owned_indexed_integer_write_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-machine-owned-indexed-integer-write-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone()).expect(
        "runtime machine-owned indexed integer write canary should compile from its authored root",
    );

    let executable = compilation.checked_native_executable_path().expect(
        "runtime machine-owned indexed integer write canary should retain its executable receipt",
    );
    let output = Command::new(executable)
        .output()
        .expect("runtime machine-owned indexed integer write canary should run");

    assert_eq!(
        output.status.code(),
        Some(79),
        "expected runtime machine-owned indexed integer write canary to preserve direct machine-owned indexed writes and exit 79, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_machine_owned_fixed_indexed_struct_copy_exit_canary_runs() {
    let canary = pass_canary("storage/runtime_machine_owned_fixed_indexed_struct_copy_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-machine-owned-fixed-indexed-struct-copy-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone()).expect(
        "runtime machine-owned fixed indexed struct copy canary should compile from its authored root",
    );

    let executable = compilation.checked_native_executable_path().expect(
        "runtime machine-owned fixed indexed struct copy canary should retain its executable receipt",
    );
    let output = Command::new(executable)
        .output()
        .expect("runtime machine-owned fixed indexed struct copy canary should run");

    assert_eq!(
        output.status.code(),
        Some(83),
        "expected runtime machine-owned fixed indexed struct copy canary to preserve direct fixed-index machine-owned copies and exit 83, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_machine_owned_indexed_struct_copy_exit_canary_runs() {
    let canary = pass_canary("storage/runtime_machine_owned_indexed_struct_copy_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-machine-owned-indexed-struct-copy-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone()).expect(
        "runtime machine-owned indexed struct copy canary should compile from its authored root",
    );

    let executable = compilation.checked_native_executable_path().expect(
        "runtime machine-owned indexed struct copy canary should retain its executable receipt",
    );
    let output = Command::new(executable)
        .output()
        .expect("runtime machine-owned indexed struct copy canary should run");

    assert_eq!(
        output.status.code(),
        Some(85),
        "expected runtime machine-owned indexed struct copy canary to preserve direct indexed machine-owned copies and exit 85, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_machine_owned_indexed_nested_exit_write_exit_canary_runs() {
    let canary = pass_canary("storage/runtime_machine_owned_indexed_nested_exit_write_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-machine-owned-indexed-nested-exit-write-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone()).expect(
        "runtime machine-owned indexed nested exit write canary should compile from its authored root",
    );

    let executable = compilation.checked_native_executable_path().expect(
        "runtime machine-owned indexed nested exit write canary should retain its executable receipt",
    );
    let output = Command::new(executable)
        .output()
        .expect("runtime machine-owned indexed nested exit write canary should run");

    assert_eq!(
        output.status.code(),
        Some(89),
        "expected runtime machine-owned indexed nested exit write canary to preserve nested fixed-array writes and exit 89, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_ordered_room_dispatch_exit_canary_runs() {
    let canary = pass_canary("dungeon/runtime_ordered_room_dispatch_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-ordered-room-dispatch-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime ordered room dispatch canary should compile from its authored root");

    let executable = compilation
        .checked_native_executable_path()
        .expect("runtime ordered room dispatch canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("runtime ordered room dispatch canary should run");

    assert_eq!(
        output.status.code(),
        Some(73),
        "expected runtime ordered room dispatch canary to route to ambush_clear exit code 73, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_ordered_room_dispatch_after_call_exit_canary_runs() {
    let canary = pass_canary("dungeon/runtime_ordered_room_dispatch_after_call_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-ordered-room-dispatch-after-call-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone()).expect(
        "runtime ordered room dispatch after call canary should compile from its authored root",
    );

    let executable = compilation.checked_native_executable_path().expect(
        "runtime ordered room dispatch after call canary should retain its executable receipt",
    );
    let output = Command::new(executable)
        .output()
        .expect("runtime ordered room dispatch after call canary should run");

    assert_eq!(
        output.status.code(),
        Some(83),
        "expected runtime ordered room dispatch after call canary to route to ambush_clear exit code 83, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_ordered_room_dispatch_game_shape_exit_canary_runs() {
    let canary = pass_canary("dungeon/runtime_ordered_room_dispatch_game_shape_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-ordered-room-dispatch-game-shape-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone()).expect(
        "runtime ordered room dispatch game-shape canary should compile from its authored root",
    );

    let executable = compilation.checked_native_executable_path().expect(
        "runtime ordered room dispatch game-shape canary should retain its executable receipt",
    );
    let output = Command::new(executable)
        .output()
        .expect("runtime ordered room dispatch game-shape canary should run");

    assert_eq!(
        output.status.code(),
        Some(93),
        "expected runtime ordered room dispatch game-shape canary to route to show_ambush_room exit code 93, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_ordered_room_dispatch_large_machine_exit_canary_runs() {
    let canary = pass_canary("dungeon/runtime_ordered_room_dispatch_large_machine_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-ordered-room-dispatch-large-machine-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone()).expect(
        "runtime ordered room dispatch large-machine canary should compile from its authored root",
    );

    let executable = compilation.checked_native_executable_path().expect(
        "runtime ordered room dispatch large-machine canary should retain its executable receipt",
    );
    let output = Command::new(executable)
        .output()
        .expect("runtime ordered room dispatch large-machine canary should run");

    assert_eq!(
        output.status.code(),
        Some(103),
        "expected runtime ordered room dispatch large-machine canary to route to show_ambush_room exit code 103, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_ordered_room_dispatch_loop_exit_canary_runs() {
    let canary = pass_canary("dungeon/runtime_ordered_room_dispatch_loop_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-ordered-room-dispatch-loop-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime ordered room dispatch loop canary should compile from its authored root");

    let executable = compilation
        .checked_native_executable_path()
        .expect("runtime ordered room dispatch loop canary should retain its executable receipt");
    let mut child = Command::new(executable)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("runtime ordered room dispatch loop canary should start");
    child
        .stdin
        .as_mut()
        .expect("stdin should be piped")
        .write_all(b"east\n")
        .expect("loop canary input should be written");
    let output = child
        .wait_with_output()
        .expect("runtime ordered room dispatch loop canary should finish");

    assert_eq!(
        output.status.code(),
        Some(135),
        "expected runtime ordered room dispatch loop canary to route to show_ambush_encounter exit code 135, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_guarded_inline_leaf_arm_skip_exit_canary_runs() {
    let canary = pass_canary("dungeon/runtime_guarded_inline_leaf_arm_skip_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-guarded-inline-leaf-arm-skip-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone()).expect(
        "runtime guarded inline leaf arm skip canary should compile from its authored root",
    );

    let executable = compilation
        .checked_native_executable_path()
        .expect("runtime guarded inline leaf arm skip canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("runtime guarded inline leaf arm skip canary should run");

    // The matched value-switch arm (`1 -> store(20)`) must skip its sibling arms;
    // exit 71 would mean the `_ -> store(30)` fallback clobbered the result to 30.
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected guarded inline leaf arm to survive sibling clobber (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_ordered_room_dispatch_real_show_states_exit_canary_runs() {
    let canary = pass_canary("dungeon/runtime_ordered_room_dispatch_real_show_states_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-ordered-room-dispatch-real-show-states-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone()).expect(
        "runtime ordered room dispatch real-show-states canary should compile from its authored root",
    );

    let executable = compilation.checked_native_executable_path().expect(
        "runtime ordered room dispatch real-show-states canary should retain its executable receipt",
    );
    let mut child = Command::new(executable)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("runtime ordered room dispatch real-show-states canary should start");
    child
        .stdin
        .as_mut()
        .expect("stdin should be piped")
        .write_all(b"east\n")
        .expect("real show states canary input should be written");
    let output = child
        .wait_with_output()
        .expect("runtime ordered room dispatch real-show-states canary should finish");

    assert_eq!(
        output.status.code(),
        Some(145),
        "expected runtime ordered room dispatch real-show-states canary to route to show_ambush_encounter exit code 145, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_threaded_mut_arg_interrupt_soak_exit_canary_runs() {
    // Soak net for interrupt-clobbered scratch registers in frame-slot copies:
    // the encoder once parked slot copies in x18, which the Darwin arm64 kernel
    // zeroes on every kernel->user return, so threaded `&mut` args corrupted
    // whenever a timer tick landed inside a copy pair (the dungeon hot-potato
    // segfault). Fifty million dispatched pointer-threaded increments span many
    // ticks; a lost copy shows up as exit 71 (dropped count) or a crash.
    let canary = pass_canary("dungeon/runtime_threaded_mut_arg_interrupt_soak_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-threaded-mut-arg-interrupt-soak-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile(CanaryCompileSpec {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("threaded mut-arg interrupt soak canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("threaded mut-arg interrupt soak canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("threaded mut-arg interrupt soak canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected all fifty million pointer-threaded increments to land (exit 70), got {:?} (71 = increments lost to a clobbered scratch register)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn named_integer_conversion_prng_cohort_reaches_checked_trees() {
    for relative in [
        "tests/omega/pass/arithmetic/runtime_contained_range_write",
        "tests/omega/pass/arithmetic/runtime_unsigned_modulo_call_argument_exit",
        "tests/omega/pass/calls/runtime_call_enum_sequence",
        "tests/omega/pass/calls/runtime_nested_named_conversion_alias_exit",
        "tests/omega/pass/control_flow/runtime_branching_helper_local_guard_value",
        "tests/omega/pass/dungeon/runtime_nested_value_call_caller_local_guard_exit",
        "tests/omega/pass/rewards/runtime_contained_reward_table_roll_item",
        "tests/omega/pass/rewards/runtime_reward_table_roll_item_shape",
    ] {
        let main_path = repo_root().join(relative).join("main.omg");
        compile_to_checked(&main_path, None).unwrap_or_else(|diagnostics| {
            panic!(
                "named integer-conversion PRNG canary {relative} should reach checked trees: \
                 {diagnostics:#?}"
            )
        });
    }
}

#[test]
fn named_integer_conversion_filesystem_decode_cohort_reaches_checked_trees() {
    for relative in [
        "tests/omega/pass/calls/runtime_value_call_struct_payload_cast_field_exit",
        "tests/omega/pass/filesystem/native_copy_preserve",
        "tests/omega/pass/filesystem/native_filetype",
        "tests/omega/pass/filesystem/native_fs_workflow",
        "tests/omega/pass/filesystem/native_fstat",
        "tests/omega/pass/filesystem/native_metadata_blocks",
        "tests/omega/pass/filesystem/native_metadata_ctime_dev",
        "tests/omega/pass/filesystem/native_metadata_ino",
        "tests/omega/pass/filesystem/native_metadata_modified",
        "tests/omega/pass/filesystem/native_metadata_nlink",
        "tests/omega/pass/filesystem/native_metadata_readonly",
        "tests/omega/pass/filesystem/native_metadata_times",
        "tests/omega/pass/filesystem/native_open_create",
        "tests/omega/pass/filesystem/native_set_times",
        "tests/omega/pass/filesystem/native_stat",
        "tests/omega/pass/filesystem/native_symlink_metadata",
        "tests/omega/pass/time/runtime_fs_mtime_interop_windows_exit",
        "tests/omega/pass/time/runtime_fs_mtime_system_time_interop_exit",
    ] {
        let main_path = repo_root().join(relative).join("main.omg");
        compile_to_checked(&main_path, None).unwrap_or_else(|diagnostics| {
            panic!(
                "named integer-conversion filesystem decode canary {relative} should reach \
                 checked trees: {diagnostics:#?}"
            )
        });
    }

    let windows_relative = "tests/omega/pass/filesystem/windows_set_file_time_exit";
    let windows_main = repo_root().join(windows_relative).join("main.omg");
    compile_to_checked(&windows_main, None).unwrap_or_else(|diagnostics| {
        panic!(
            "named integer-conversion filesystem decode canary {windows_relative} should reach \
             checked trees: {diagnostics:#?}"
        )
    });
}

#[test]
fn named_integer_conversion_filesystem_cross_targets_reach_checked_trees() {
    let canary = pass_canary("filesystem/windows_positioned_io_exit");
    let targets = ["linux_x86_64", "linux_arm64", "windows_x86_64"];
    let results = run_bounded_canary_jobs(&targets, |target| {
        compile_to_checked(&canary.join("main.omg"), Some(target))
            .map(|_| ())
            .map_err(|diagnostics| format!("{diagnostics:#?}"))
    });
    for (target, result) in targets.into_iter().zip(results) {
        result.unwrap_or_else(|diagnostic| {
            panic!(
                "named integer-conversion filesystem cohort should reach checked trees for \
                 {target}: {diagnostic}"
            )
        });
    }
}

#[test]
fn runtime_nested_value_call_caller_local_guard_exit_canary_runs() {
    // A guarded transition on a value call whose NESTED inline value call
    // returns a comparison against the CALLER's fold-only local (`chance`'s
    // `roll < numerator` with `numerator` bound to should_carve's slot-less
    // local `chance`). The leaf context could not resolve the name as a place,
    // so the call-result write was silently dropped: the guard byte stayed 0
    // and the TRUE arm never dispatched -- the dungeon's side rooms R05/R06
    // were never carved, rendering empty description lines.
    let canary = pass_canary("dungeon/runtime_nested_value_call_caller_local_guard_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-nested-value-call-caller-local-guard-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("nested value-call caller-local guard canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("nested value-call caller-local guard canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("nested value-call caller-local guard canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the nested chance comparison to reach its call-result slot so \
         the TRUE transition arm dispatches (exit 70, interpreter semantics; \
         exit 71 = the result write was dropped and the guard byte read 0), \
         got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[cfg(not(windows))]
#[test]
fn native_dungeon_crawler_runs_stable_scripted_loop() {
    let sample = sample_project("cli/games/dungeon_crawler_cli");
    let main_path = sample.join("main.omg");
    let build_dir =
        std::env::temp_dir().join(format!("omega-native-dungeon-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CanaryCompileSpec {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("native dungeon crawler should compile to a runnable executable");

    let mut child = Command::new(build_dir.join(executable_name()))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("native dungeon crawler executable should start");
    child
        .stdin
        .as_mut()
        .expect("stdin should be piped")
        .write_all(
            // The detour at R02 (`west` into R06, `east` back) visits the second
            // side room so its data-driven description line is exercised too.
            b"look\nnorth\nnorth\nuse\nlook\nnorth\nfight\nlook\nnorth\nuse\nlook\nsouth\nsouth\nwest\neast\nsouth\neast\nuse\nlook\ninv\nwest\nsouth\nhelp\nexit\n",
        )
        .expect("scripted dungeon input should be written");
    let output = child
        .wait_with_output()
        .expect("native dungeon crawler executable should finish");

    assert!(
        output.status.success(),
        "generated native dungeon executable exited with {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Dungeon Crawler"));
    assert!(stdout.contains("== Gate =="));
    // Canonical strings come from the sample's data-driven room view (commit
    // 3971b22f replaced the hardcoded "A stone gate opens..." text with the maze
    // builder's depth-derived descriptions); every string below is produced
    // identically by the interpreter oracle on this exact script.
    assert!(stdout.contains("A bottomless dark room near the dungeon heart."));
    assert!(stdout.contains("== Branch Room =="));
    assert!(stdout.contains("A winding branch room where the walls sweat mineral dust."));
    assert!(stdout.contains("[Paths] north"));
    assert!(stdout.contains("[Paths] south | north | east"));
    assert!(stdout.contains("You take the treasure."));
    assert!(stdout.contains("The enemy collapses. You find a little gold."));
    assert!(stdout.contains("The fountain heals your wounds."));
    assert!(stdout.contains("You collect the loose gold."));
    // The side rooms' data-driven DESCRIPTIONS are asserted with their adjacent
    // unique lines so the match pins the right room view. These were the last
    // native/interpreter divergence: the side rooms were never CARVED natively
    // because `should_carve`'s nested `chance` value (`roll < numerator`, with
    // `numerator` bound to the caller's slot-less local `chance`) lost its
    // call-result write, so the carve transition's TRUE arm never fired --
    // fixed by resolving caller-local initializer names in leaf terminal value
    // writes, pinned by dungeon/runtime_nested_value_call_caller_local_guard_exit.
    // With this the scripted tour is byte-for-byte the interpreter's output.
    assert!(stdout.contains(
        "A shallow limestone room with fresh claw marks.\nLoose gold glitters in the dust."
    ));
    assert!(stdout.contains("Loose gold glitters in the dust."));
    assert!(stdout.contains("[Paths] west"));
    // R06 (the west side chamber off R02): depth-3 description, quiet event,
    // and its unique single east exit.
    assert!(stdout.contains(
        "A winding branch room where the walls sweat mineral dust.\nThe room is quiet.\n[Paths] east"
    ));
    assert!(stdout.contains("Inv: 30 gold. Purse heavy, charm secured."));

    let _ = fs::remove_dir_all(&build_dir);
}

#[cfg(not(windows))]
#[test]
fn native_dungeon_direct_movement_dispatch_runs() {
    let source = sample_project("cli/games/dungeon_crawler_cli");
    let package_dir = std::env::temp_dir().join(format!(
        "omega-dungeon-direct-movement-{}",
        std::process::id()
    ));
    let build_dir = package_dir.join("build");
    let _ = fs::remove_dir_all(&package_dir);
    copy_dir_recursive(&source, &package_dir).expect("sample package should copy into temp repro");

    compile(CanaryCompileSpec {
        root_path: package_dir.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("patched dungeon direct-dispatch repro should compile");

    let mut child = Command::new(build_dir.join(executable_name()))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("patched dungeon direct-dispatch repro should start");
    child
        .stdin
        .as_mut()
        .expect("stdin should be piped")
        .write_all(b"north\neast\nexit\n")
        .expect("repro input should be written");
    let output = child
        .wait_with_output()
        .expect("patched dungeon direct-dispatch repro should finish");

    assert!(
        output.status.success(),
        "patched dungeon direct-dispatch repro exited with {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    // R05 (the east side chamber) is identified by its data-driven description
    // followed by its gold-cache event line and its unique "[Paths] west" exit
    // list -- all rendered identically by the interpreter oracle on this script.
    assert!(
        stdout.contains(
            "A shallow limestone room with fresh claw marks.\nLoose gold glitters in the dust."
        ),
        "expected the side chamber's depth-derived description right before its gold-cache event line; stdout was:\n{}",
        stdout
    );
    assert!(
        stdout.contains("Loose gold glitters in the dust."),
        "expected direct movement dispatch sample to reach the side chamber after 'north' then 'east'; stdout was:\n{}",
        stdout
    );
    assert!(
        stdout.contains("[Paths] west"),
        "expected the side chamber's exit list after 'north' then 'east'; stdout was:\n{}",
        stdout
    );
    assert!(
        !stdout.contains("[Input] That action is not available right now."),
        "expected direct movement dispatch sample not to reject 'north' then 'east'; stdout was:\n{}",
        stdout
    );

    let _ = fs::remove_dir_all(&package_dir);
}

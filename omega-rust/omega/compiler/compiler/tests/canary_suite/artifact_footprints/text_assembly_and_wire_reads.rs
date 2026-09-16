use super::assert_native_exit_code;
use super::fixture_roster;
use crate::{
    CanaryCompileProduct, CanaryCompileSpec, compile, compile_rooted_canary_for_native_host,
    compile_rooted_canary_for_target, fs, hosted_main_program_entry_build,
    hosted_main_program_entry_build_for, pass_canary,
};

#[test]
fn compiler_body_frame_base_indexed_text_assembly_footprints_reach_aarch64_artifact() {
    let canary = pass_canary(fixture_roster::RUNTIME_LOCAL_ARRAY_INDEXED_STRING_FIELD_CONCAT_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-compiler-body-frame-base-indexed-text-assembly-footprint-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let output = scratch.join("out");
    compile_rooted_canary_for_target(
        &canary,
        output.clone(),
        "linux_arm64",
    )
    .unwrap_or_else(|diagnostics| {
        panic!(
            "compiler-body frame-base-indexed text assembly should compile for linux_arm64: {diagnostics:?}"
        )
    });
    let footprints = fs::read_to_string(output.join("08_boundary_footprints.json"))
        .expect("compiler-body frame-base-indexed text-assembly evidence should be written");
    assert!(
        footprints.contains("\"origin\": \"compiler_body_text_assembly_write\"")
            && footprints.contains("\"Aarch64X(19)\"")
            && footprints.contains("\"Aarch64X(24)\"")
            && footprints.contains("\"enumeration_complete\": false"),
        "linux_arm64 artifact must retain frame-base-indexed stored/literal text-assembly evidence without claiming completeness"
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn compiler_body_text_literal_append_footprints_reach_artifacts() {
    let canary = pass_canary(fixture_roster::RUNTIME_LOCAL_STRUCT_STRING_FIELD_CONCAT_EXIT);
    for (target, expected_register) in [
        ("linux_x86_64", "\"X86Rcx\""),
        ("linux_arm64", "\"Aarch64X(26)\""),
    ] {
        let scratch = std::env::temp_dir().join(format!(
            "omega-compiler-body-text-literal-append-footprint-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        let source = scratch.join("src");
        let output = scratch.join("out");
        fs::create_dir_all(&source)
            .expect("create compiler-body text literal-append source directory");
        fs::copy(canary.join("main.omg"), source.join("main.omg"))
            .expect("copy compiler-body text literal-append canary");
        fs::write(
            source.join("build.omg"),
            hosted_main_program_entry_build_for(&canary, target),
        )
        .expect("write compiler-body text literal-append target");
        compile(CanaryCompileSpec {
            root_path: source.join("main.omg"),
            build_dir: Some(output.clone()),
            target_name: Some(target.into()),
            product: CanaryCompileProduct::NativeArtifactAndPublish,
        })
        .unwrap_or_else(|diagnostics| {
            panic!(
                "compiler-body text literal appends should compile for {target}: {diagnostics:?}"
            )
        });
        let footprints = fs::read_to_string(output.join("08_boundary_footprints.json"))
            .expect("compiler-body text literal-append evidence should be written");
        assert!(
            footprints.contains("\"origin\": \"compiler_body_text_assembly_write\"")
                && footprints.contains(expected_register)
                && footprints.contains("\"enumeration_complete\": false"),
            "{target} artifact must retain text literal-append evidence without claiming completeness"
        );
        let _ = fs::remove_dir_all(&scratch);
    }
}

#[test]
fn compiler_body_text_stored_append_footprints_reach_artifacts() {
    let canary = pass_canary(fixture_roster::RUNTIME_SLICE_ALIAS_INDEXED_STRING_FIELD_CONCAT_EXIT);
    for (target, expected_register) in [
        ("linux_x86_64", "\"X86Rsi\""),
        ("linux_arm64", "\"Aarch64X(24)\""),
    ] {
        let scratch = std::env::temp_dir().join(format!(
            "omega-compiler-body-text-stored-append-footprint-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        let output = scratch.join("out");
        compile_rooted_canary_for_target(&canary, output.clone(), target).unwrap_or_else(
            |diagnostics| {
                panic!(
                    "compiler-body stored-text appends should compile for {target}: {diagnostics:?}"
                )
            },
        );
        let footprints = fs::read_to_string(output.join("08_boundary_footprints.json"))
            .expect("compiler-body stored-text append evidence should be written");
        assert!(
            footprints.contains("\"origin\": \"compiler_body_text_assembly_write\"")
                && footprints.contains(expected_register)
                && footprints.contains("\"enumeration_complete\": false"),
            "{target} artifact must retain stored-text append evidence without claiming completeness"
        );
        let _ = fs::remove_dir_all(&scratch);
    }
}

#[test]
fn compiler_body_text_stored_suffix_footprints_reach_artifacts() {
    let canary = pass_canary(fixture_roster::RUNTIME_STRING_STORED_SUFFIX_EXIT);
    for (target, expected_register) in [
        ("linux_x86_64", "\"X86R11\""),
        ("linux_arm64", "\"Aarch64X(23)\""),
    ] {
        let scratch = std::env::temp_dir().join(format!(
            "omega-compiler-body-text-stored-suffix-footprint-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        let source = scratch.join("src");
        let output = scratch.join("out");
        fs::create_dir_all(&source)
            .expect("create compiler-body stored-text suffix source directory");
        fs::copy(canary.join("main.omg"), source.join("main.omg"))
            .expect("copy compiler-body stored-text suffix canary");
        fs::write(
            source.join("build.omg"),
            hosted_main_program_entry_build_for(&canary, target),
        )
        .expect("write compiler-body stored-text suffix target");
        compile(CanaryCompileSpec {
            root_path: source.join("main.omg"),
            build_dir: Some(output.clone()),
            target_name: Some(target.into()),
            product: CanaryCompileProduct::NativeArtifactAndPublish,
        })
        .unwrap_or_else(|diagnostics| {
            panic!("compiler-body stored-text suffix should compile for {target}: {diagnostics:?}")
        });
        let footprints = fs::read_to_string(output.join("08_boundary_footprints.json"))
            .expect("compiler-body stored-text suffix evidence should be written");
        let target_operations = fs::read_to_string(output.join("09_target_operations.html"))
            .expect("compiler-body stored-text suffix target operations should be written");
        assert!(
            target_operations.contains("AppendRuntimeTextStoredSuffix"),
            "{target} canary must exercise the segmented stored-suffix operation"
        );
        assert!(
            footprints.contains("\"origin\": \"compiler_body_text_assembly_write\"")
                && footprints.contains(expected_register)
                && footprints.contains("\"enumeration_complete\": false"),
            "{target} artifact must retain segmented stored-suffix evidence without claiming completeness"
        );
        let _ = fs::remove_dir_all(&scratch);
    }
}

#[test]
fn compiler_body_place_address_footprints_reach_artifacts() {
    let canary = pass_canary(fixture_roster::RUNTIME_RECORD_VIEW_EXIT);
    for (target, expected_register) in [
        ("linux_x86_64", "\"X86R14\""),
        ("linux_arm64", "\"Aarch64X(21)\""),
    ] {
        let scratch = std::env::temp_dir().join(format!(
            "omega-compiler-body-place-address-footprint-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        let source = scratch.join("src");
        let output = scratch.join("out");
        fs::create_dir_all(&source).expect("create compiler-body place-address source directory");
        fs::copy(canary.join("main.omg"), source.join("main.omg"))
            .expect("copy compiler-body place-address canary");
        fs::write(
            source.join("build.omg"),
            hosted_main_program_entry_build_for(&canary, target),
        )
        .expect("write compiler-body place-address target");
        compile(CanaryCompileSpec {
            root_path: source.join("main.omg"),
            build_dir: Some(output.clone()),
            target_name: Some(target.into()),
            product: CanaryCompileProduct::NativeArtifactAndPublish,
        })
        .unwrap_or_else(|diagnostics| {
            panic!("compiler-body place-address write should compile for {target}: {diagnostics:?}")
        });
        let footprints = fs::read_to_string(output.join("08_boundary_footprints.json"))
            .expect("compiler-body place-address evidence should be written");
        let target_operations = fs::read_to_string(output.join("09_target_operations.html"))
            .expect("compiler-body place-address target operations should be written");
        assert!(
            target_operations.contains("WritePlaceAddress"),
            "{target} canary must exercise a place-address write"
        );
        assert!(
            footprints.contains("\"origin\": \"compiler_body_place_address_write\"")
                && footprints.contains(expected_register)
                && footprints.contains("\"enumeration_complete\": false"),
            "{target} artifact must retain place-address evidence without claiming completeness"
        );
        let _ = fs::remove_dir_all(&scratch);
    }
}

#[test]
fn runtime_record_view_place_address_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_RECORD_VIEW_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-record-view-place-address-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime record-view place-address canary should compile");
    assert_native_exit_code(
        &compilation,
        70,
        "record-view place-address canary",
        "the record view should retain its exact place address",
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn compiler_body_bounded_buffer_literal_append_footprints_reach_artifacts() {
    let canary = pass_canary(
        fixture_roster::RUNTIME_MACHINE_OWNED_DOUBLE_INDEXED_BOUNDED_CARRIER_LITERAL_EXIT,
    );
    for (target, expected_register) in [
        ("linux_x86_64", "\"X86Rdi\""),
        ("linux_arm64", "\"Aarch64X(14)\""),
    ] {
        let scratch = std::env::temp_dir().join(format!(
            "omega-compiler-body-bounded-buffer-literal-append-footprint-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        let output = scratch.join("out");
        compile_rooted_canary_for_target(
            &canary,
            output.clone(),
            target,
        )
        .unwrap_or_else(
            |diagnostics| {
                panic!(
                    "compiler-body bounded-buffer literal appends should compile for {target}: {diagnostics:?}"
                )
            },
        );
        let footprints = fs::read_to_string(output.join("08_boundary_footprints.json"))
            .expect("compiler-body bounded-buffer literal-append evidence should be written");
        assert!(
            footprints.contains("\"origin\": \"compiler_body_place_bounded_buffer_write\"")
                && footprints.contains(expected_register)
                && footprints.contains("\"enumeration_complete\": false"),
            "{target} artifact must retain the bounded-buffer literal-append footprint without claiming completeness"
        );
        let _ = fs::remove_dir_all(&scratch);
    }
}

#[test]
fn compiler_body_string_write_footprints_reach_x86_and_aarch64_artifacts() {
    let canary =
        pass_canary(fixture_roster::RUNTIME_MACHINE_OWNED_DOUBLE_INDEXED_STRING_FIELD_CONCAT_EXIT);
    for (target, expected_register) in [
        ("linux_x86_64", "\"X86R14\""),
        ("linux_arm64", "\"Aarch64X(17)\""),
    ] {
        let scratch = std::env::temp_dir().join(format!(
            "omega-compiler-body-string-write-footprint-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        let output = scratch.join("out");
        compile_rooted_canary_for_target(&canary, output.clone(), target).unwrap_or_else(
            |diagnostics| {
                panic!("compiler-body string writes should compile for {target}: {diagnostics:?}")
            },
        );
        let footprints = fs::read_to_string(output.join("08_boundary_footprints.json"))
            .expect("compiler-body string-write footprint evidence should be written");
        assert!(
            footprints.contains("\"origin\": \"compiler_body_place_string_write\"")
                && footprints.contains(expected_register)
                && footprints.contains("\"enumeration_complete\": false"),
            "{target} artifact must retain the string-write footprint without claiming completeness"
        );
        let _ = fs::remove_dir_all(&scratch);
    }
}

#[test]
fn compiler_body_general_x86_text_assembly_reaches_the_final_artifact() {
    let canary =
        pass_canary(fixture_roster::RUNTIME_X86_GENERAL_DOUBLE_INDEXED_STRING_CONCAT_COMPILE);
    let scratch = std::env::temp_dir().join(format!(
        "omega-general-x86-text-assembly-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let output = scratch.join("out");
    compile(CanaryCompileSpec {
        root_path: canary.join("main.omg"),
        build_dir: Some(output.clone()),
        target_name: Some("linux_x86_64".into()),
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("general double-indexed x86 text assembly should compile");

    let footprints = fs::read_to_string(output.join("08_boundary_footprints.json"))
        .expect("general x86 text-assembly footprint evidence should be written");
    assert!(
        footprints.contains("\"origin\": \"compiler_body_text_assembly_write\"")
            && footprints.contains("\"X86R10\"")
            && footprints.contains("\"X86R15\""),
        "general x86 text assembly must retain its two-index materializer footprint"
    );
    let regions = fs::read_to_string(output.join("13_executable_regions.json"))
        .expect("general x86 final executable-region evidence should be written");
    assert!(
        regions.contains("\"certificate_marker\": \"omega.final-footprint-certificate.current\"")
            && regions.contains("\"compiler_function_body_specification\""),
        "general x86 text assembly must reach final-image validation"
    );
    let elf = fs::read(output.join("omega-program")).expect("linux_x64 ELF emitted");
    assert_eq!(&elf[..4], b"\x7fELF");
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn compiler_body_wire_scalar_appends_reach_x86_and_aarch64_artifacts() {
    let canary = pass_canary(fixture_roster::RUNTIME_WIRE_ENCODE_PRIMITIVE_EXIT);
    for (target, expected_scalar_register) in [
        ("linux_x86_64", "\"X86Rax\""),
        ("linux_arm64", "\"Aarch64X(26)\""),
    ] {
        let scratch = std::env::temp_dir().join(format!(
            "omega-compiler-body-wire-scalar-appends-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        let source = scratch.join("src");
        let output = scratch.join("out");
        fs::create_dir_all(&source)
            .expect("create compiler-body wire scalar-append source directory");
        fs::copy(canary.join("main.omg"), source.join("main.omg"))
            .expect("copy compiler-body wire scalar-append canary");
        fs::write(
            source.join("build.omg"),
            hosted_main_program_entry_build_for(&canary, target),
        )
        .expect("write compiler-body wire scalar-append target");

        compile(CanaryCompileSpec {
            root_path: source.join("main.omg"),
            build_dir: Some(output.clone()),
            target_name: Some(target.into()),
            product: CanaryCompileProduct::NativeArtifactAndPublish,
        })
        .unwrap_or_else(|diagnostics| {
            panic!("compiler-body wire scalar appends should compile for {target}: {diagnostics:?}")
        });
        let target_operations = fs::read_to_string(output.join("09_target_operations.html"))
            .expect("compiler-body wire scalar-append target operations should be written");
        let footprints = fs::read_to_string(output.join("08_boundary_footprints.json"))
            .expect("compiler-body wire scalar-append footprint evidence should be written");
        let regions = fs::read_to_string(output.join("13_executable_regions.json"))
            .expect("compiler-body wire scalar-append final-region evidence should be written");
        assert!(
            target_operations.contains("AppendWireLiteralByte")
                && target_operations.contains("AppendWireScalarVarint"),
            "{target} canary must exercise wire literal-byte and scalar-varint appends"
        );
        assert!(
            footprints.contains("\"origin\": \"compiler_body_wire_literal_byte_append\"")
                && footprints.contains("\"origin\": \"compiler_body_wire_scalar_varint_append\"")
                && footprints.contains(expected_scalar_register)
                && footprints.contains("\"enumeration_complete\": false"),
            "{target} artifact must retain both wire append footprints without claiming completeness"
        );
        assert!(
            regions
                .contains("\"certificate_marker\": \"omega.final-footprint-certificate.current\"")
                && regions.contains("\"compiler_function_body_specification\""),
            "{target} wire appends must reach final-byte validation"
        );
        let elf = fs::read(output.join("omega-program"))
            .unwrap_or_else(|error| panic!("read {target} ELF: {error}"));
        assert_eq!(&elf[..4], b"\x7fELF");
        let _ = fs::remove_dir_all(&scratch);
    }
}

#[test]
fn compiler_body_wire_text_appends_reach_x86_and_aarch64_artifacts() {
    let canary = pass_canary(fixture_roster::RUNTIME_WIRE_ENCODE_STRING_EXIT);
    for (target, expected_registers) in [
        (
            "linux_x86_64",
            "[\"X86Rax\", \"X86Rcx\", \"X86R9\", \"X86R10\", \"X86R11\", \"X86R14\", \"X86R15\"]",
        ),
        (
            "linux_arm64",
            "[\"Aarch64X(16)\", \"Aarch64X(17)\", \"Aarch64X(19)\", \"Aarch64X(20)\", \"Aarch64X(22)\", \"Aarch64X(24)\", \"Aarch64X(25)\", \"Aarch64X(26)\"]",
        ),
    ] {
        let scratch = std::env::temp_dir().join(format!(
            "omega-compiler-body-wire-text-append-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        let source = scratch.join("src");
        let output = scratch.join("out");
        fs::create_dir_all(&source)
            .expect("create compiler-body wire text-append source directory");
        fs::copy(canary.join("main.omg"), source.join("main.omg"))
            .expect("copy compiler-body wire text-append canary");
        fs::write(
            source.join("build.omg"),
            hosted_main_program_entry_build_for(&canary, target),
        )
        .expect("write compiler-body wire text-append target");

        compile(CanaryCompileSpec {
            root_path: source.join("main.omg"),
            build_dir: Some(output.clone()),
            target_name: Some(target.into()),
            product: CanaryCompileProduct::NativeArtifactAndPublish,
        })
        .unwrap_or_else(|diagnostics| {
            panic!("compiler-body wire text appends should compile for {target}: {diagnostics:?}")
        });
        let target_operations = fs::read_to_string(output.join("09_target_operations.html"))
            .expect("compiler-body wire text-append target operations should be written");
        let footprints = fs::read_to_string(output.join("08_boundary_footprints.json"))
            .expect("compiler-body wire text-append footprint evidence should be written");
        let regions = fs::read_to_string(output.join("13_executable_regions.json"))
            .expect("compiler-body wire text-append final-region evidence should be written");
        assert!(
            target_operations.contains("AppendWireTextBytes"),
            "{target} canary must exercise a wire text append"
        );
        let text_fragment = footprints
            .lines()
            .find(|line| line.contains("\"origin\": \"compiler_body_wire_text_bytes_append\""))
            .unwrap_or_else(|| panic!("{target} wire text-append footprint fragment must exist"));
        assert!(
            text_fragment.contains("\"machine_state_bits\": 5")
                && text_fragment.contains(expected_registers)
                && footprints.contains("\"enumeration_complete\": false"),
            "{target} artifact must retain the exact wire text-append footprint without claiming completeness"
        );
        assert!(
            regions
                .contains("\"certificate_marker\": \"omega.final-footprint-certificate.current\"")
                && regions.contains("\"compiler_function_body_specification\""),
            "{target} wire text append must reach final-byte validation"
        );
        let elf = fs::read(output.join("omega-program"))
            .unwrap_or_else(|error| panic!("read {target} ELF: {error}"));
        assert_eq!(&elf[..4], b"\x7fELF");
        let _ = fs::remove_dir_all(&scratch);
    }
}

#[test]
fn compiler_body_wire_scalar_slice_appends_reach_x86_and_aarch64_artifacts() {
    let canary = pass_canary(fixture_roster::RUNTIME_WIRE_ENCODE_BORROWED_SCALAR_SLICE_EXIT);
    for (target, expected_registers) in [
        (
            "linux_x86_64",
            "[\"X86Rax\", \"X86Rcx\", \"X86Rdx\", \"X86Rsi\", \"X86Rdi\", \"X86R8\", \"X86R9\", \"X86R10\", \"X86R11\", \"X86R14\", \"X86R15\"]",
        ),
        (
            "linux_arm64",
            "[\"Aarch64X(16)\", \"Aarch64X(17)\", \"Aarch64X(19)\", \"Aarch64X(20)\", \"Aarch64X(21)\", \"Aarch64X(22)\", \"Aarch64X(23)\", \"Aarch64X(24)\", \"Aarch64X(25)\", \"Aarch64X(26)\", \"Aarch64X(27)\", \"Aarch64X(28)\"]",
        ),
    ] {
        let scratch = std::env::temp_dir().join(format!(
            "omega-compiler-body-wire-scalar-slice-append-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        let source = scratch.join("src");
        let output = scratch.join("out");
        fs::create_dir_all(&source)
            .expect("create compiler-body wire scalar-slice source directory");
        fs::copy(canary.join("main.omg"), source.join("main.omg"))
            .expect("copy compiler-body wire scalar-slice canary");
        fs::write(
            source.join("build.omg"),
            hosted_main_program_entry_build_for(&canary, target),
        )
        .expect("write compiler-body wire scalar-slice target");

        compile(CanaryCompileSpec {
            root_path: source.join("main.omg"),
            build_dir: Some(output.clone()),
            target_name: Some(target.into()),
            product: CanaryCompileProduct::NativeArtifactAndPublish,
        })
        .unwrap_or_else(|diagnostics| {
            panic!(
                "compiler-body wire scalar-slice append should compile for {target}: {diagnostics:?}"
            )
        });
        let target_operations = fs::read_to_string(output.join("09_target_operations.html"))
            .expect("compiler-body wire scalar-slice target operations should be written");
        let footprints = fs::read_to_string(output.join("08_boundary_footprints.json"))
            .expect("compiler-body wire scalar-slice footprint evidence should be written");
        let regions = fs::read_to_string(output.join("13_executable_regions.json"))
            .expect("compiler-body wire scalar-slice final-region evidence should be written");
        assert!(
            target_operations.contains("AppendWireScalarSlice"),
            "{target} canary must exercise a wire scalar-slice append"
        );
        let slice_fragment = footprints
            .lines()
            .find(|line| line.contains("\"origin\": \"compiler_body_wire_scalar_slice_append\""))
            .unwrap_or_else(|| panic!("{target} wire scalar-slice footprint fragment must exist"));
        assert!(
            slice_fragment.contains("\"machine_state_bits\": 5")
                && slice_fragment.contains(expected_registers)
                && footprints.contains("\"enumeration_complete\": false"),
            "{target} artifact must retain the exact wire scalar-slice footprint without claiming completeness"
        );
        assert!(
            regions
                .contains("\"certificate_marker\": \"omega.final-footprint-certificate.current\"")
                && regions.contains("\"compiler_function_body_specification\""),
            "{target} wire scalar-slice append must reach final-byte validation"
        );
        let elf = fs::read(output.join("omega-program"))
            .unwrap_or_else(|error| panic!("read {target} ELF: {error}"));
        assert_eq!(&elf[..4], b"\x7fELF");
        let _ = fs::remove_dir_all(&scratch);
    }
}

#[test]
fn compiler_body_wire_repeated_scalar_appends_reach_x86_and_aarch64_artifacts() {
    let canary = pass_canary(fixture_roster::RUNTIME_WIRE_ENCODE_REPEATED_THEN_STRING_EXIT);
    for (target, expected_registers) in [
        (
            "linux_x86_64",
            "[\"X86Rax\", \"X86R9\", \"X86R10\", \"X86R11\", \"X86R14\", \"X86R15\"]",
        ),
        (
            "linux_arm64",
            "[\"Aarch64X(16)\", \"Aarch64X(17)\", \"Aarch64X(19)\", \"Aarch64X(20)\", \"Aarch64X(26)\"]",
        ),
    ] {
        let scratch = std::env::temp_dir().join(format!(
            "omega-compiler-body-wire-repeated-scalar-append-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        let source = scratch.join("src");
        let output = scratch.join("out");
        fs::create_dir_all(&source)
            .expect("create compiler-body wire repeated-scalar source directory");
        fs::copy(canary.join("main.omg"), source.join("main.omg"))
            .expect("copy compiler-body wire repeated-scalar canary");
        fs::write(
            source.join("build.omg"),
            hosted_main_program_entry_build_for(&canary, target),
        )
        .expect("write compiler-body wire repeated-scalar target");

        compile(CanaryCompileSpec {
            root_path: source.join("main.omg"),
            build_dir: Some(output.clone()),
            target_name: Some(target.into()),
            product: CanaryCompileProduct::NativeArtifactAndPublish,
        })
        .unwrap_or_else(|diagnostics| {
            panic!(
                "compiler-body wire repeated scalar appends should compile for {target}: {diagnostics:?}"
            )
        });
        let target_operations = fs::read_to_string(output.join("09_target_operations.html"))
            .expect("compiler-body wire repeated-scalar target operations should be written");
        let footprints = fs::read_to_string(output.join("08_boundary_footprints.json"))
            .expect("compiler-body wire repeated-scalar footprint evidence should be written");
        let regions = fs::read_to_string(output.join("13_executable_regions.json"))
            .expect("compiler-body wire repeated-scalar final-region evidence should be written");
        assert!(
            target_operations.contains("AppendWireRepeatedScalarVarint"),
            "{target} canary must exercise guarded wire repeated-scalar appends"
        );
        let repeated_fragment = footprints
            .lines()
            .find(|line| {
                line.contains("\"origin\": \"compiler_body_wire_repeated_scalar_varint_append\"")
            })
            .unwrap_or_else(|| {
                panic!("{target} wire repeated-scalar footprint fragment must exist")
            });
        assert!(
            repeated_fragment.contains("\"machine_state_bits\": 5")
                && repeated_fragment.contains(expected_registers)
                && footprints.contains("\"enumeration_complete\": false"),
            "{target} artifact must retain the exact guarded repeated-scalar footprint without claiming completeness"
        );
        assert!(
            regions
                .contains("\"certificate_marker\": \"omega.final-footprint-certificate.current\"")
                && regions.contains("\"compiler_function_body_specification\""),
            "{target} wire repeated-scalar appends must reach final-byte validation"
        );
        let elf = fs::read(output.join("omega-program"))
            .unwrap_or_else(|error| panic!("read {target} ELF: {error}"));
        assert_eq!(&elf[..4], b"\x7fELF");
        let _ = fs::remove_dir_all(&scratch);
    }
}

#[test]
fn compiler_body_wire_byte_slice_reads_reach_x86_and_aarch64_artifacts() {
    let canary = pass_canary(fixture_roster::RUNTIME_WIRE_DECODE_BYTE_SLICE_EXIT);
    for (target, expected_registers) in [
        (
            "linux_x86_64",
            "[\"X86Rax\", \"X86Rcx\", \"X86R8\", \"X86R9\", \"X86R10\", \"X86R11\", \"X86R13\", \"X86R14\", \"X86R15\"]",
        ),
        (
            "linux_arm64",
            "[\"Aarch64X(16)\", \"Aarch64X(17)\", \"Aarch64X(19)\", \"Aarch64X(20)\", \"Aarch64X(21)\", \"Aarch64X(22)\", \"Aarch64X(23)\", \"Aarch64X(24)\", \"Aarch64X(25)\", \"Aarch64X(26)\"]",
        ),
    ] {
        let scratch = std::env::temp_dir().join(format!(
            "omega-compiler-body-wire-byte-slice-read-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        let source = scratch.join("src");
        let output = scratch.join("out");
        fs::create_dir_all(&source).expect("create compiler-body wire byte-slice source directory");
        fs::copy(canary.join("main.omg"), source.join("main.omg"))
            .expect("copy compiler-body wire byte-slice canary");
        fs::write(
            source.join("build.omg"),
            hosted_main_program_entry_build_for(&canary, target),
        )
        .expect("write compiler-body wire byte-slice target");

        compile(CanaryCompileSpec {
            root_path: source.join("main.omg"),
            build_dir: Some(output.clone()),
            target_name: Some(target.into()),
            product: CanaryCompileProduct::NativeArtifactAndPublish,
        })
        .unwrap_or_else(|diagnostics| {
            panic!(
                "compiler-body wire byte-slice reads should compile for {target}: {diagnostics:?}"
            )
        });
        let target_operations = fs::read_to_string(output.join("09_target_operations.html"))
            .expect("compiler-body wire byte-slice target operations should be written");
        let footprints = fs::read_to_string(output.join("08_boundary_footprints.json"))
            .expect("compiler-body wire byte-slice footprint evidence should be written");
        let regions = fs::read_to_string(output.join("13_executable_regions.json"))
            .expect("compiler-body wire byte-slice final-region evidence should be written");
        assert!(
            target_operations.contains("ReadWireByteSlice"),
            "{target} canary must exercise a zero-copy wire byte-slice read"
        );
        let slice_fragment = footprints
            .lines()
            .find(|line| line.contains("\"origin\": \"compiler_body_wire_byte_slice_read\""))
            .unwrap_or_else(|| panic!("{target} wire byte-slice footprint fragment must exist"));
        assert!(
            slice_fragment.contains("\"machine_state_bits\": 5")
                && slice_fragment.contains(expected_registers)
                && footprints.contains("\"enumeration_complete\": false"),
            "{target} artifact must retain the exact zero-copy byte-slice footprint without claiming completeness"
        );
        assert!(
            regions
                .contains("\"certificate_marker\": \"omega.final-footprint-certificate.current\"")
                && regions.contains("\"compiler_function_body_specification\""),
            "{target} wire byte-slice read must reach final-byte validation"
        );
        let elf = fs::read(output.join("omega-program"))
            .unwrap_or_else(|error| panic!("read {target} ELF: {error}"));
        assert_eq!(&elf[..4], b"\x7fELF");
        let _ = fs::remove_dir_all(&scratch);
    }
}

#[test]
fn compiler_body_wire_nested_bounds_reach_x86_and_aarch64_artifacts() {
    let canary = pass_canary(fixture_roster::RUNTIME_WIRE_ROUNDTRIP_NESTED_AND_REPEATED_EXIT);
    for (target, expected_open_registers, expected_close_registers) in [
        (
            "linux_x86_64",
            "[\"X86Rax\", \"X86R8\", \"X86R9\", \"X86R10\", \"X86R11\", \"X86R13\", \"X86R14\", \"X86R15\"]",
            "[\"X86Rax\", \"X86R8\", \"X86R9\", \"X86R10\", \"X86R11\", \"X86R13\", \"X86R14\", \"X86R15\"]",
        ),
        (
            "linux_arm64",
            "[\"Aarch64X(16)\", \"Aarch64X(17)\", \"Aarch64X(19)\", \"Aarch64X(20)\", \"Aarch64X(21)\", \"Aarch64X(23)\", \"Aarch64X(24)\", \"Aarch64X(25)\", \"Aarch64X(26)\"]",
            "[\"Aarch64X(16)\", \"Aarch64X(17)\", \"Aarch64X(19)\", \"Aarch64X(20)\", \"Aarch64X(21)\", \"Aarch64X(23)\", \"Aarch64X(25)\", \"Aarch64X(26)\"]",
        ),
    ] {
        let scratch = std::env::temp_dir().join(format!(
            "omega-compiler-body-wire-nested-bounds-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        let source = scratch.join("src");
        let output = scratch.join("out");
        fs::create_dir_all(&source)
            .expect("create compiler-body wire nested-bounds source directory");
        fs::copy(canary.join("main.omg"), source.join("main.omg"))
            .expect("copy compiler-body wire nested-bounds canary");
        fs::write(
            source.join("build.omg"),
            hosted_main_program_entry_build_for(&canary, target),
        )
        .expect("write compiler-body wire nested-bounds target");

        compile(CanaryCompileSpec {
            root_path: source.join("main.omg"),
            build_dir: Some(output.clone()),
            target_name: Some(target.into()),
            product: CanaryCompileProduct::NativeArtifactAndPublish,
        })
        .unwrap_or_else(|diagnostics| {
            panic!("compiler-body wire nested bounds should compile for {target}: {diagnostics:?}")
        });
        let target_operations = fs::read_to_string(output.join("09_target_operations.html"))
            .expect("compiler-body wire nested-bounds target operations should be written");
        let footprints = fs::read_to_string(output.join("08_boundary_footprints.json"))
            .expect("compiler-body wire nested-bounds footprint evidence should be written");
        let regions = fs::read_to_string(output.join("13_executable_regions.json"))
            .expect("compiler-body wire nested-bounds final-region evidence should be written");
        assert!(
            target_operations.contains("ReadWireNestedOpen")
                && target_operations.contains("ReadWireNestedClose"),
            "{target} canary must exercise both nested-boundary operations"
        );
        let open_fragment = footprints
            .lines()
            .find(|line| line.contains("\"origin\": \"compiler_body_wire_nested_open\""))
            .unwrap_or_else(|| panic!("{target} wire nested-open footprint fragment must exist"));
        let close_fragment = footprints
            .lines()
            .find(|line| line.contains("\"origin\": \"compiler_body_wire_nested_close\""))
            .unwrap_or_else(|| panic!("{target} wire nested-close footprint fragment must exist"));
        assert!(
            open_fragment.contains("\"machine_state_bits\": 5")
                && open_fragment.contains(expected_open_registers)
                && close_fragment.contains("\"machine_state_bits\": 5")
                && close_fragment.contains(expected_close_registers)
                && footprints.contains("\"enumeration_complete\": false"),
            "{target} artifact must retain exact nested-open and nested-close footprints without claiming completeness"
        );
        assert!(
            regions
                .contains("\"certificate_marker\": \"omega.final-footprint-certificate.current\"")
                && regions.contains("\"compiler_function_body_specification\""),
            "{target} nested-boundary checks must reach final-byte validation"
        );
        let elf = fs::read(output.join("omega-program"))
            .unwrap_or_else(|error| panic!("read {target} ELF: {error}"));
        assert_eq!(&elf[..4], b"\x7fELF");
        let _ = fs::remove_dir_all(&scratch);
    }
}

#[test]
fn compiler_body_wire_repeated_scalar_reads_reach_x86_and_aarch64_artifacts() {
    let canary = pass_canary(fixture_roster::RUNTIME_WIRE_ROUNDTRIP_NESTED_AND_REPEATED_EXIT);
    for (target, expected_registers) in [
        (
            "linux_x86_64",
            "[\"X86Rax\", \"X86Rcx\", \"X86R8\", \"X86R9\", \"X86R10\", \"X86R11\", \"X86R13\", \"X86R14\", \"X86R15\"]",
        ),
        (
            "linux_arm64",
            "[\"Aarch64X(16)\", \"Aarch64X(17)\", \"Aarch64X(19)\", \"Aarch64X(20)\", \"Aarch64X(21)\", \"Aarch64X(22)\", \"Aarch64X(23)\", \"Aarch64X(24)\", \"Aarch64X(25)\", \"Aarch64X(26)\"]",
        ),
    ] {
        let scratch = std::env::temp_dir().join(format!(
            "omega-compiler-body-wire-repeated-scalar-read-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        let source = scratch.join("src");
        let output = scratch.join("out");
        fs::create_dir_all(&source)
            .expect("create compiler-body wire repeated-scalar-read source directory");
        fs::copy(canary.join("main.omg"), source.join("main.omg"))
            .expect("copy compiler-body wire repeated-scalar-read canary");
        fs::write(
            source.join("build.omg"),
            hosted_main_program_entry_build_for(&canary, target),
        )
        .expect("write compiler-body wire repeated-scalar-read target");

        compile(CanaryCompileSpec {
            root_path: source.join("main.omg"),
            build_dir: Some(output.clone()),
            target_name: Some(target.into()),
            product: CanaryCompileProduct::NativeArtifactAndPublish,
        })
        .unwrap_or_else(|diagnostics| {
            panic!(
                "compiler-body wire repeated-scalar reads should compile for {target}: {diagnostics:?}"
            )
        });
        let target_operations = fs::read_to_string(output.join("09_target_operations.html"))
            .expect("compiler-body wire repeated-scalar-read target operations should be written");
        let footprints = fs::read_to_string(output.join("08_boundary_footprints.json"))
            .expect("compiler-body wire repeated-scalar-read footprint evidence should be written");
        let regions = fs::read_to_string(output.join("13_executable_regions.json")).expect(
            "compiler-body wire repeated-scalar-read final-region evidence should be written",
        );
        assert!(
            target_operations.contains("ReadWireRepeatedScalarVarint"),
            "{target} canary must exercise a guarded repeated-scalar read"
        );
        let repeated_fragment = footprints
            .lines()
            .find(|line| {
                line.contains("\"origin\": \"compiler_body_wire_repeated_scalar_varint_read\"")
            })
            .unwrap_or_else(|| {
                panic!("{target} repeated-scalar-read footprint fragment must exist")
            });
        assert!(
            repeated_fragment.contains("\"machine_state_bits\": 5")
                && repeated_fragment.contains(expected_registers)
                && footprints.contains("\"enumeration_complete\": false"),
            "{target} artifact must retain the exact repeated-scalar-read footprint without claiming completeness"
        );
        assert!(
            regions
                .contains("\"certificate_marker\": \"omega.final-footprint-certificate.current\"")
                && regions.contains("\"compiler_function_body_specification\""),
            "{target} guarded repeated-scalar read must reach final-byte validation"
        );
        let elf = fs::read(output.join("omega-program"))
            .unwrap_or_else(|error| panic!("read {target} ELF: {error}"));
        assert_eq!(&elf[..4], b"\x7fELF");
        let _ = fs::remove_dir_all(&scratch);
    }
}

#[test]
fn compiler_body_wire_expected_byte_reads_reach_x86_and_aarch64_artifacts() {
    let canary = pass_canary(fixture_roster::RUNTIME_WIRE_DECODE_LET_COMPARE_EXIT);
    for (target, expected_register) in [
        ("linux_x86_64", "\"X86R13\""),
        ("linux_arm64", "\"Aarch64X(21)\""),
    ] {
        let scratch = std::env::temp_dir().join(format!(
            "omega-compiler-body-wire-expected-byte-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        let source = scratch.join("src");
        let output = scratch.join("out");
        fs::create_dir_all(&source)
            .expect("create compiler-body wire expected-byte source directory");
        fs::copy(canary.join("main.omg"), source.join("main.omg"))
            .expect("copy compiler-body wire expected-byte canary");
        fs::write(
            source.join("build.omg"),
            hosted_main_program_entry_build_for(&canary, target),
        )
        .expect("write compiler-body wire expected-byte target");

        compile(CanaryCompileSpec {
            root_path: source.join("main.omg"),
            build_dir: Some(output.clone()),
            target_name: Some(target.into()),
            product: CanaryCompileProduct::NativeArtifactAndPublish,
        })
        .unwrap_or_else(|diagnostics| {
            panic!(
                "compiler-body wire expected-byte reads should compile for {target}: {diagnostics:?}"
            )
        });
        let target_operations = fs::read_to_string(output.join("09_target_operations.html"))
            .expect("compiler-body wire expected-byte target operations should be written");
        let footprints = fs::read_to_string(output.join("08_boundary_footprints.json"))
            .expect("compiler-body wire expected-byte footprint evidence should be written");
        let regions = fs::read_to_string(output.join("13_executable_regions.json"))
            .expect("compiler-body wire expected-byte final-region evidence should be written");
        assert!(
            target_operations.contains("ReadWireExpectedByte"),
            "{target} canary must exercise a wire expected-byte read"
        );
        assert!(
            footprints.contains("\"origin\": \"compiler_body_wire_expected_byte_read\"")
                && footprints.contains(expected_register)
                && footprints.contains("\"enumeration_complete\": false"),
            "{target} artifact must retain the expected-byte read footprint without claiming completeness"
        );
        assert!(
            regions
                .contains("\"certificate_marker\": \"omega.final-footprint-certificate.current\"")
                && regions.contains("\"compiler_function_body_specification\""),
            "{target} wire expected-byte read must reach final-byte validation"
        );
        let elf = fs::read(output.join("omega-program"))
            .unwrap_or_else(|error| panic!("read {target} ELF: {error}"));
        assert_eq!(&elf[..4], b"\x7fELF");
        let _ = fs::remove_dir_all(&scratch);
    }
}

#[test]
fn compiler_body_wire_ranged_scalar_reads_reach_x86_and_aarch64_artifacts() {
    let canary = pass_canary(fixture_roster::RUNTIME_WIRE_DECODE_RANGED_FIELD_EXIT);
    for (target, expected_registers) in [
        (
            "linux_x86_64",
            "[\"X86Rax\", \"X86Rcx\", \"X86R8\", \"X86R9\", \"X86R10\", \"X86R11\", \"X86R13\", \"X86R14\", \"X86R15\"]",
        ),
        (
            "linux_arm64",
            "[\"Aarch64X(16)\", \"Aarch64X(17)\", \"Aarch64X(19)\", \"Aarch64X(20)\", \"Aarch64X(21)\", \"Aarch64X(22)\", \"Aarch64X(23)\", \"Aarch64X(24)\", \"Aarch64X(25)\", \"Aarch64X(26)\"]",
        ),
    ] {
        let scratch = std::env::temp_dir().join(format!(
            "omega-compiler-body-wire-ranged-scalar-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        let source = scratch.join("src");
        let output = scratch.join("out");
        fs::create_dir_all(&source)
            .expect("create compiler-body wire ranged-scalar source directory");
        fs::copy(canary.join("main.omg"), source.join("main.omg"))
            .expect("copy compiler-body wire ranged-scalar canary");
        fs::write(
            source.join("build.omg"),
            hosted_main_program_entry_build_for(&canary, target),
        )
        .expect("write compiler-body wire ranged-scalar target");

        compile(CanaryCompileSpec {
            root_path: source.join("main.omg"),
            build_dir: Some(output.clone()),
            target_name: Some(target.into()),
            product: CanaryCompileProduct::NativeArtifactAndPublish,
        })
        .unwrap_or_else(|diagnostics| {
            panic!(
                "compiler-body wire ranged-scalar reads should compile for {target}: {diagnostics:?}"
            )
        });
        let target_operations = fs::read_to_string(output.join("09_target_operations.html"))
            .expect("compiler-body wire ranged-scalar target operations should be written");
        let footprints = fs::read_to_string(output.join("08_boundary_footprints.json"))
            .expect("compiler-body wire ranged-scalar footprint evidence should be written");
        let regions = fs::read_to_string(output.join("13_executable_regions.json"))
            .expect("compiler-body wire ranged-scalar final-region evidence should be written");
        assert!(
            target_operations.contains("ReadWireScalarVarint"),
            "{target} canary must exercise unsigned-ranged and signed scalar-varint reads"
        );
        let scalar_fragment = footprints
            .lines()
            .find(|line| line.contains("\"origin\": \"compiler_body_wire_scalar_varint_read\""))
            .unwrap_or_else(|| panic!("{target} scalar-varint footprint fragment must exist"));
        assert!(
            scalar_fragment.contains("\"machine_state_bits\": 5")
                && scalar_fragment.contains(expected_registers)
                && footprints.contains("\"enumeration_complete\": false"),
            "{target} artifact must retain the scalar-varint read footprint without claiming completeness"
        );
        assert!(
            regions
                .contains("\"certificate_marker\": \"omega.final-footprint-certificate.current\"")
                && regions.contains("\"compiler_function_body_specification\""),
            "{target} wire scalar-varint reads must reach final-byte validation"
        );
        let elf = fs::read(output.join("omega-program"))
            .unwrap_or_else(|error| panic!("read {target} ELF: {error}"));
        assert_eq!(&elf[..4], b"\x7fELF");
        let _ = fs::remove_dir_all(&scratch);
    }
}

#[test]
fn aarch64_frame_descriptor_ops_with_machine_index_reach_the_final_artifact() {
    let canary =
        pass_canary(fixture_roster::RUNTIME_AARCH64_CROSS_REGION_FRAME_INDEXED_RMW_COMPILE);
    let scratch = std::env::temp_dir().join(format!(
        "omega-aarch64-cross-region-frame-indexed-rmw-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let output = scratch.join("out");
    compile(CanaryCompileSpec {
        root_path: canary.join("main.omg"),
        build_dir: Some(output.clone()),
        target_name: Some("linux_arm64".into()),
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("cross-region AArch64 frame-descriptor RMW should compile");

    let footprints = fs::read_to_string(output.join("08_boundary_footprints.json"))
        .expect("cross-region AArch64 footprint evidence should be written");
    assert!(
        footprints.contains("\"origin\": \"compiler_body_place_binary_write\"")
            && footprints.contains("\"origin\": \"compiler_body_place_copy\"")
            && footprints.contains("\"Aarch64X(15)\"")
            && footprints.contains("\"Aarch64X(21)\""),
        "cross-region frame-indexed writes and reads must retain their address scratch footprints"
    );
    let regions = fs::read_to_string(output.join("13_executable_regions.json"))
        .expect("cross-region AArch64 final executable-region evidence should be written");
    assert!(
        regions.contains("\"certificate_marker\": \"omega.final-footprint-certificate.current\"")
            && regions.contains("\"compiler_function_body_specification\""),
        "cross-region AArch64 frame-descriptor operations must reach final-image validation"
    );
    let elf = fs::read(output.join("omega-program")).expect("linux_arm64 ELF emitted");
    assert_eq!(&elf[..4], b"\x7fELF");
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn compiler_body_bounded_buffer_write_footprints_reach_x86_and_aarch64_artifacts() {
    let canary = pass_canary(
        fixture_roster::RUNTIME_MACHINE_OWNED_DOUBLE_INDEXED_BOUNDED_CARRIER_LITERAL_EXIT,
    );
    for (target, expected_register) in [
        ("linux_x86_64", "\"X86Rax\""),
        ("linux_arm64", "\"Aarch64X(17)\""),
    ] {
        let scratch = std::env::temp_dir().join(format!(
            "omega-compiler-body-bounded-buffer-write-footprint-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        let output = scratch.join("out");
        compile_rooted_canary_for_target(
            &canary,
            output.clone(),
            target,
        )
        .unwrap_or_else(
            |diagnostics| {
                panic!(
                    "compiler-body bounded-buffer writes should compile for {target}: {diagnostics:?}"
                )
            },
        );
        let footprints = fs::read_to_string(output.join("08_boundary_footprints.json"))
            .expect("compiler-body bounded-buffer-write footprint evidence should be written");
        assert!(
            footprints.contains("\"origin\": \"compiler_body_place_bounded_buffer_write\"")
                && footprints.contains(expected_register)
                && footprints.contains("\"enumeration_complete\": false"),
            "{target} artifact must retain the bounded-buffer-write footprint without claiming completeness"
        );
        let _ = fs::remove_dir_all(&scratch);
    }
}

#[test]
fn compiler_body_storage_bit_field_write_footprints_reach_x86_and_aarch64_artifacts() {
    let canary = pass_canary(fixture_roster::RUNTIME_PLAN_LAID_COMPACT_BITS_EXIT);
    for (target, expected_register) in [
        ("linux_x86_64", "\"X86R11\""),
        ("linux_arm64", "\"Aarch64X(20)\""),
    ] {
        let scratch = std::env::temp_dir().join(format!(
            "omega-compiler-body-storage-bit-field-write-footprint-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        let source = scratch.join("src");
        let output = scratch.join("out");
        fs::create_dir_all(&source)
            .expect("create compiler-body storage-bit-field-write source directory");
        fs::copy(canary.join("main.omg"), source.join("main.omg"))
            .expect("copy compiler-body storage-bit-field-write canary");
        fs::write(
            source.join("build.omg"),
            hosted_main_program_entry_build_for(&canary, target),
        )
        .expect("write compiler-body storage-bit-field-write target");
        compile(CanaryCompileSpec {
            root_path: source.join("main.omg"),
            build_dir: Some(output.clone()),
            target_name: Some(target.into()),
            product: CanaryCompileProduct::NativeArtifactAndPublish,
        })
        .unwrap_or_else(|diagnostics| {
            panic!(
                "compiler-body storage bit-field writes should compile for {target}: {diagnostics:?}"
            )
        });
        let footprints = fs::read_to_string(output.join("08_boundary_footprints.json"))
            .expect("compiler-body storage-bit-field-write footprint evidence should be written");
        assert!(
            footprints.contains("\"origin\": \"compiler_body_storage_bit_field_write\"")
                && footprints.contains(expected_register)
                && footprints.contains("\"enumeration_complete\": false"),
            "{target} artifact must retain the storage-bit-field-write footprint without claiming completeness"
        );
        let _ = fs::remove_dir_all(&scratch);
    }
}

#[test]
fn compiler_body_storage_convert_write_footprints_reach_x86_and_aarch64_artifacts() {
    let canary = pass_canary(fixture_roster::RUNTIME_ENTRY_CAST_RESULT_EXIT);
    for (target, expected_register) in [
        ("linux_x86_64", "\"X86R14\""),
        ("linux_arm64", "\"Aarch64X(16)\""),
    ] {
        let scratch = std::env::temp_dir().join(format!(
            "omega-compiler-body-storage-convert-write-footprint-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        let source = scratch.join("src");
        let output = scratch.join("out");
        fs::create_dir_all(&source)
            .expect("create compiler-body storage-convert-write source directory");
        fs::copy(canary.join("main.omg"), source.join("main.omg"))
            .expect("copy compiler-body storage-convert-write canary");
        fs::write(
            source.join("build.omg"),
            hosted_main_program_entry_build_for(&canary, target),
        )
        .expect("write compiler-body storage-convert-write target");
        compile(CanaryCompileSpec {
            root_path: source.join("main.omg"),
            build_dir: Some(output.clone()),
            target_name: Some(target.into()),
            product: CanaryCompileProduct::NativeArtifactAndPublish,
        })
        .unwrap_or_else(|diagnostics| {
            panic!(
                "compiler-body storage conversion writes should compile for {target}: {diagnostics:?}"
            )
        });
        let footprints = fs::read_to_string(output.join("08_boundary_footprints.json"))
            .expect("compiler-body storage-convert-write footprint evidence should be written");
        assert!(
            footprints.contains("\"origin\": \"compiler_body_storage_convert_write\"")
                && footprints.contains(expected_register)
                && footprints.contains("\"enumeration_complete\": false"),
            "{target} artifact must retain the storage-convert-write footprint without claiming completeness"
        );
        let _ = fs::remove_dir_all(&scratch);
    }
}

#[test]
fn compiler_body_machine_indexed_convert_write_footprints_reach_x86_and_aarch64_artifacts() {
    let canary = pass_canary(fixture_roster::RUNTIME_NUMBER_TO_DECIMAL_EXIT);
    for (target, expected_register) in [
        ("linux_x86_64", "\"X86R14\""),
        ("linux_arm64", "\"Aarch64X(16)\""),
    ] {
        let scratch = std::env::temp_dir().join(format!(
            "omega-compiler-body-machine-indexed-convert-write-footprint-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        let source = scratch.join("src");
        let output = scratch.join("out");
        fs::create_dir_all(&source)
            .expect("create compiler-body machine-indexed convert-write source directory");
        fs::copy(canary.join("main.omg"), source.join("main.omg"))
            .expect("copy compiler-body machine-indexed convert-write canary");
        fs::write(
            source.join("build.omg"),
            hosted_main_program_entry_build_for(&canary, target),
        )
        .expect("write compiler-body machine-indexed convert-write target");
        compile(CanaryCompileSpec {
            root_path: source.join("main.omg"),
            build_dir: Some(output.clone()),
            target_name: Some(target.into()),
            product: CanaryCompileProduct::NativeArtifactAndPublish,
        })
        .unwrap_or_else(|diagnostics| {
            panic!(
                "compiler-body machine-indexed conversion writes should compile for {target}: {diagnostics:?}"
            )
        });
        let footprints = fs::read_to_string(output.join("08_boundary_footprints.json")).expect(
            "compiler-body machine-indexed convert-write footprint evidence should be written",
        );
        assert!(
            footprints.contains("\"origin\": \"compiler_body_storage_convert_write\"")
                && footprints.contains(expected_register)
                && footprints.contains("\"enumeration_complete\": false"),
            "{target} artifact must retain the machine-indexed convert-write footprint without claiming completeness"
        );
        let _ = fs::remove_dir_all(&scratch);
    }
}

#[test]
fn compiler_body_pointee_integer_write_footprints_reach_x86_and_aarch64_artifacts() {
    let canary = pass_canary(fixture_roster::RUNTIME_VIEW_OF_VIEW_CHAIN_EXIT);
    for (target, expected_register) in [
        ("linux_x86_64", "\"X86R15\""),
        ("linux_arm64", "\"Aarch64X(17)\""),
    ] {
        let scratch = std::env::temp_dir().join(format!(
            "omega-compiler-body-pointee-integer-write-footprint-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        let source = scratch.join("src");
        let output = scratch.join("out");
        fs::create_dir_all(&source)
            .expect("create compiler-body pointee integer-write source directory");
        fs::copy(canary.join("main.omg"), source.join("main.omg"))
            .expect("copy compiler-body pointee integer-write canary");
        fs::write(
            source.join("build.omg"),
            hosted_main_program_entry_build_for(&canary, target),
        )
        .expect("write compiler-body pointee integer-write target");
        compile(CanaryCompileSpec {
            root_path: source.join("main.omg"),
            build_dir: Some(output.clone()),
            target_name: Some(target.into()),
            product: CanaryCompileProduct::NativeArtifactAndPublish,
        })
        .unwrap_or_else(|diagnostics| {
            panic!(
                "compiler-body pointee integer writes should compile for {target}: {diagnostics:?}"
            )
        });
        let footprints = fs::read_to_string(output.join("08_boundary_footprints.json"))
            .expect("compiler-body pointee integer-write footprint evidence should be written");
        assert!(
            footprints.contains("\"origin\": \"compiler_body_place_integer_write\"")
                && footprints.contains(expected_register)
                && footprints.contains("\"enumeration_complete\": false"),
            "{target} artifact must retain the pointee integer-write footprint without claiming completeness"
        );
        let _ = fs::remove_dir_all(&scratch);
    }
}

#[test]
fn compiler_body_cross_region_frame_indexed_integer_write_footprints_reach_artifacts() {
    for (target, expected_register) in [
        ("linux_x86_64", "\"X86R11\""),
        ("linux_arm64", "\"Aarch64X(15)\""),
    ] {
        let scratch = std::env::temp_dir().join(format!(
            "omega-compiler-body-frame-indexed-integer-write-footprint-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        let source = scratch.join("src");
        let output = scratch.join("out");
        fs::create_dir_all(&source)
            .expect("create compiler-body frame-indexed integer-write source directory");
        fs::write(
            source.join("main.omg"),
            r#"use omega::language::std::console;

data Entry { value: i32; }
data Main {
    console: Console;
    entries: [Entry; 4];
    index: u64;
}

machine Main::main(&mut self) reaches Console {
    self.index = 2;
    let view: &mut [Entry] = self.entries.as_mut_slice();
    view[self.index].value = 7;
    transition self.entries[2].value == 7 {
        true -> good()
        false -> bad()
    }
    state good(&mut self) { self.console.exit_process(70); }
    state bad(&mut self) { self.console.exit_process(71); }
}
"#,
        )
        .expect("write compiler-body cross-region frame-indexed integer-write canary");
        fs::write(
            source.join("build.omg"),
            hosted_main_program_entry_build(target),
        )
        .expect("write compiler-body frame-indexed integer-write target");
        compile(CanaryCompileSpec {
            root_path: source.join("main.omg"),
            build_dir: Some(output.clone()),
            target_name: Some(target.into()),
            product: CanaryCompileProduct::NativeArtifactAndPublish,
        })
        .unwrap_or_else(|diagnostics| {
            panic!(
                "compiler-body frame-indexed integer writes should compile for {target}: {diagnostics:?}"
            )
        });
        let footprints = fs::read_to_string(output.join("08_boundary_footprints.json")).expect(
            "compiler-body frame-indexed integer-write footprint evidence should be written",
        );
        assert!(
            footprints.contains("\"origin\": \"compiler_body_place_integer_write\"")
                && footprints.contains(expected_register)
                && footprints.contains("\"enumeration_complete\": false"),
            "{target} artifact must retain the frame-indexed integer-write footprint without claiming completeness"
        );
        let _ = fs::remove_dir_all(&scratch);
    }
}

#[test]
fn compiler_body_frame_base_indexed_integer_write_footprints_reach_artifacts() {
    for (target, expected_register) in [
        ("linux_x86_64", "\"X86R11\""),
        ("linux_arm64", "\"Aarch64X(26)\""),
    ] {
        let scratch = std::env::temp_dir().join(format!(
            "omega-compiler-body-frame-base-indexed-integer-write-footprint-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        let source = scratch.join("src");
        let output = scratch.join("out");
        fs::create_dir_all(&source)
            .expect("create compiler-body frame-base-indexed integer-write source directory");
        fs::write(
            source.join("main.omg"),
            r#"use omega::language::std::console;

data Entry { value: i32; }
data Room { entries: [Entry; 4]; }
data Main { console: Console; }

machine Main::main(&mut self) reaches Console {
    let room: Room = Room {
        entries: [
            Entry { value: 0 },
            Entry { value: 0 },
            Entry { value: 0 },
            Entry { value: 0 }
        ]
    };
    let index: u64 = 2;
    room.entries[index].value = 7;
    transition room.entries[2].value == 7 {
        true -> good()
        false -> bad()
    }
    state good(&mut self) { self.console.exit_process(70); }
    state bad(&mut self) { self.console.exit_process(71); }
}

"#,
        )
        .expect("write compiler-body frame-base-indexed integer-write canary");
        fs::write(
            source.join("build.omg"),
            hosted_main_program_entry_build(target),
        )
        .expect("write compiler-body frame-base-indexed integer-write target");
        compile(CanaryCompileSpec {
            root_path: source.join("main.omg"),
            build_dir: Some(output.clone()),
            target_name: Some(target.into()),
            product: CanaryCompileProduct::NativeArtifactAndPublish,
        })
        .unwrap_or_else(|diagnostics| {
            panic!(
                "compiler-body frame-base-indexed integer writes should compile for {target}: {diagnostics:?}"
            )
        });
        let footprints = fs::read_to_string(output.join("08_boundary_footprints.json")).expect(
            "compiler-body frame-base-indexed integer-write footprint evidence should be written",
        );
        assert!(
            footprints.contains("\"origin\": \"compiler_body_place_integer_write\"")
                && footprints.contains(expected_register)
                && footprints.contains("\"enumeration_complete\": false"),
            "{target} artifact must retain the frame-base-indexed integer-write footprint without claiming completeness"
        );
        let _ = fs::remove_dir_all(&scratch);
    }
}

#[test]
fn compiler_body_machine_indexed_integer_write_footprints_reach_artifacts() {
    let canary = pass_canary(fixture_roster::RUNTIME_MACHINE_OWNED_INDEXED_INTEGER_WRITE_EXIT);
    for (target, expected_register) in [
        ("linux_x86_64", "\"X86R11\""),
        ("linux_arm64", "\"Aarch64X(26)\""),
    ] {
        let scratch = std::env::temp_dir().join(format!(
            "omega-compiler-body-machine-indexed-integer-write-footprint-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        let output = scratch.join("out");
        compile_rooted_canary_for_target(
            &canary,
            output.clone(),
            target,
        )
        .unwrap_or_else(|diagnostics| {
            panic!(
                "compiler-body machine-indexed integer writes should compile for {target}: {diagnostics:?}"
            )
        });
        let footprints = fs::read_to_string(output.join("08_boundary_footprints.json")).expect(
            "compiler-body machine-indexed integer-write footprint evidence should be written",
        );
        assert!(
            footprints.contains("\"origin\": \"compiler_body_place_integer_write\"")
                && footprints.contains(expected_register)
                && footprints.contains("\"enumeration_complete\": false"),
            "{target} artifact must retain the machine-indexed integer-write footprint without claiming completeness"
        );
        let _ = fs::remove_dir_all(&scratch);
    }
}

#[test]
fn compiler_body_double_indexed_integer_write_footprints_reach_artifacts() {
    for (target, expected_register) in [
        ("linux_x86_64", "\"X86R10\""),
        ("linux_arm64", "\"Aarch64X(15)\""),
    ] {
        let scratch = std::env::temp_dir().join(format!(
            "omega-compiler-body-double-indexed-integer-write-footprint-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        let source = scratch.join("src");
        let output = scratch.join("out");
        fs::create_dir_all(&source)
            .expect("create compiler-body double-indexed integer-write source directory");
        fs::write(
            source.join("main.omg"),
            r#"use omega::language::std::console;

data Main {
    console: Console;
    grid: [[i32; 4]; 3];
}

machine Main::main(&mut self) reaches Console {
    let i: u64 [0..=2] = 1;
    let j: u64 [0..=3] = 2;
    self.grid[i][j] = 70;
    transition self.grid[1][2] == 70 {
        true -> good()
        false -> bad()
    }
    state good(&mut self) { self.console.exit_process(70); }
    state bad(&mut self) { self.console.exit_process(71); }
}
"#,
        )
        .expect("write compiler-body double-indexed integer-write canary");
        fs::write(
            source.join("build.omg"),
            hosted_main_program_entry_build(target),
        )
        .expect("write compiler-body double-indexed integer-write target");
        compile(CanaryCompileSpec {
            root_path: source.join("main.omg"),
            build_dir: Some(output.clone()),
            target_name: Some(target.into()),
            product: CanaryCompileProduct::NativeArtifactAndPublish,
        })
        .unwrap_or_else(|diagnostics| {
            panic!(
                "compiler-body double-indexed integer writes should compile for {target}: {diagnostics:?}"
            )
        });
        let footprints = fs::read_to_string(output.join("08_boundary_footprints.json")).expect(
            "compiler-body double-indexed integer-write footprint evidence should be written",
        );
        assert!(
            footprints.contains("\"origin\": \"compiler_body_place_integer_write\"")
                && footprints.contains(expected_register)
                && footprints.contains("\"enumeration_complete\": false"),
            "{target} artifact must retain the double-indexed integer-write footprint without claiming completeness"
        );
        let _ = fs::remove_dir_all(&scratch);
    }
}

#[test]
fn runtime_value_guard_footprints_reach_x86_and_aarch64_artifacts() {
    let canary = pass_canary(fixture_roster::RUNTIME_STRING_FIELD_LITERAL_GUARD_EXIT);
    for (target, expected_registers) in [
        (
            "linux_x86_64",
            "[\"X86Rax\", \"X86Rcx\", \"X86Rdx\", \"X86R8\", \"X86R9\", \"X86R10\", \"X86R11\", \"X86R15\", \"X86Xmm(0)\", \"X86Xmm(1)\"]",
        ),
        (
            "linux_arm64",
            "[\"Aarch64X(9)\", \"Aarch64X(10)\", \"Aarch64X(11)\", \"Aarch64X(12)\", \"Aarch64X(13)\", \"Aarch64X(14)\", \"Aarch64X(15)\", \"Aarch64X(17)\", \"Aarch64X(19)\", \"Aarch64X(20)\", \"Aarch64X(21)\", \"Aarch64X(26)\", \"Aarch64V(0)\", \"Aarch64V(1)\"]",
        ),
    ] {
        let scratch = std::env::temp_dir().join(format!(
            "omega-runtime-value-guard-footprint-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        let output = scratch.join("out");
        compile_rooted_canary_for_target(&canary, output.clone(), target).unwrap_or_else(
            |diagnostics| {
                panic!("runtime-value guard should compile for {target}: {diagnostics:?}")
            },
        );
        let abstract_operations = fs::read_to_string(output.join("08_abstract_operations.html"))
            .expect("runtime-value guard abstract operations should be written");
        let footprints = fs::read_to_string(output.join("08_boundary_footprints.json"))
            .expect("runtime-value guard footprint evidence should be written");
        assert!(
            abstract_operations.contains("CompareRuntimeValues"),
            "{target} canary must exercise the recursive runtime-value guard encoder"
        );
        assert!(
            footprints.contains("\"origin\": \"runtime_value_guard_comparison\"")
                && footprints.contains(expected_registers)
                && footprints.contains("\"enumeration_complete\": false"),
            "{target} artifact must retain the runtime-value guard ceiling without claiming completeness"
        );
        let _ = fs::remove_dir_all(&scratch);
    }
}

use super::*;

fn assert_native_exit_code(
    report: &CompileReport,
    expected: i32,
    fixture: &str,
    expectation: &str,
) {
    let executable = report
        .checked_native_executable_path()
        .unwrap_or_else(|| panic!("{fixture} lost its exact executable publication receipt"));
    let output = Command::new(executable)
        .output()
        .unwrap_or_else(|error| panic!("{fixture} should run: {error}"));
    assert_eq!(
        output.status.code(),
        Some(expected),
        "{expectation}; expected exit {expected}, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn static_guard_footprints_reach_x86_and_aarch64_artifacts() {
    let canary = pass_canary("control_flow/runtime_integer_literal_dispatch_exit");
    let host_scratch = std::env::temp_dir().join(format!(
        "omega-static-guard-footprint-host-{}",
        std::process::id()
    ));
    compile_single_file_hosted_main(&canary, &host_scratch, native_hosted_target())
        .expect("integer literal dispatch should compile for the host");
    let host_dir = host_scratch.join("out");
    let host_footprint = fs::read_to_string(host_dir.join("08_boundary_footprints.json"))
        .expect("host static-guard footprint evidence should be written");
    assert!(
        host_footprint.contains("\"origin\": \"dispatch_scaffold\"")
            && host_footprint.contains("\"origin\": \"static_guard_comparison\"")
            && host_footprint.contains("\"enumeration_complete\": false"),
        "x86-64 dispatch must retain its static guard evidence without claiming completeness"
    );
    let _ = fs::remove_dir_all(&host_scratch);

    let arm_scratch = std::env::temp_dir().join(format!(
        "omega-static-guard-footprint-arm-{}",
        std::process::id()
    ));
    let arm_output = arm_scratch.join("out");
    compile_single_file_hosted_main(&canary, &arm_scratch, "linux_arm64")
        .expect("integer literal dispatch should cross-compile for AArch64");
    let arm_footprint = fs::read_to_string(arm_output.join("08_boundary_footprints.json"))
        .expect("AArch64 static-guard footprint evidence should be written");
    assert!(
        arm_footprint.contains("\"origin\": \"dispatch_scaffold\"")
            && arm_footprint.contains("\"origin\": \"static_guard_comparison\"")
            && arm_footprint.contains("\"enumeration_complete\": false"),
        "AArch64 dispatch must retain its static guard evidence without claiming completeness"
    );
    let _ = fs::remove_dir_all(&arm_scratch);
}

#[test]
fn runtime_text_guard_footprints_reach_x86_and_aarch64_artifacts() {
    let canary = pass_canary("text/runtime_local_struct_string_field_concat_exit");
    for (target, expected_registers) in [
        ("linux_x86_64", "[\"X86Rax\", \"X86R15\"]"),
        ("linux_arm64", "[\"Aarch64X(16)\", \"Aarch64X(17)\"]"),
    ] {
        let scratch = std::env::temp_dir().join(format!(
            "omega-runtime-text-guard-footprint-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        let source = scratch.join("src");
        let output = scratch.join("out");
        fs::create_dir_all(&source).expect("create runtime-text guard source directory");
        fs::copy(canary.join("main.omg"), source.join("main.omg"))
            .expect("copy runtime-text guard canary");
        fs::write(
            source.join("build.omg"),
            hosted_main_program_entry_build(target),
        )
        .expect("write runtime-text guard target");

        compile_with_auxiliary_artifacts(CanaryCompileSpec {
            root_path: source.join("main.omg"),
            build_dir: Some(output.clone()),
            target_name: Some(target.into()),
            product: CanaryCompileProduct::NativeArtifactAndPublish,
        })
        .unwrap_or_else(|diagnostics| {
            panic!("runtime-text guard should compile for {target}: {diagnostics:?}")
        });
        let abstract_operations = fs::read_to_string(output.join("08_abstract_operations.html"))
            .expect("runtime-text guard abstract operations should be written");
        let footprints = fs::read_to_string(output.join("08_boundary_footprints.json"))
            .expect("runtime-text guard footprint evidence should be written");
        assert!(
            abstract_operations.contains("CompareRuntimeTextLiteral"),
            "{target} canary must exercise the dedicated runtime-text literal guard encoder"
        );
        assert!(
            footprints.contains("\"origin\": \"runtime_text_guard_comparison\"")
                && footprints.contains(expected_registers)
                && footprints.contains("\"enumeration_complete\": false"),
            "{target} artifact must retain exact runtime-text guard evidence without claiming completeness"
        );
        let _ = fs::remove_dir_all(&scratch);
    }
}

#[test]
fn place_guard_footprints_reach_x86_and_aarch64_artifacts() {
    let canary = pass_canary("control_flow/termination_index_distance_compile");
    for (target, expected_registers) in [
        (
            "linux_x86_64",
            "[\"X86R10\", \"X86R11\", \"X86R14\", \"X86R15\"]",
        ),
        (
            "linux_arm64",
            "[\"Aarch64X(16)\", \"Aarch64X(17)\", \"Aarch64X(19)\", \"Aarch64X(26)\"]",
        ),
    ] {
        let scratch = std::env::temp_dir().join(format!(
            "omega-place-guard-footprint-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        let source = scratch.join("src");
        let output = scratch.join("out");
        fs::create_dir_all(&source).expect("create place-guard source directory");
        fs::copy(canary.join("main.omg"), source.join("main.omg"))
            .expect("copy place-guard canary");
        fs::write(
            source.join("build.omg"),
            hosted_main_program_entry_build(target),
        )
        .expect("write place-guard target");

        compile_with_auxiliary_artifacts(CanaryCompileSpec {
            root_path: source.join("main.omg"),
            build_dir: Some(output.clone()),
            target_name: Some(target.into()),
            product: CanaryCompileProduct::NativeArtifactAndPublish,
        })
        .unwrap_or_else(|diagnostics| {
            panic!("place guard should compile for {target}: {diagnostics:?}")
        });
        let abstract_operations = fs::read_to_string(output.join("08_abstract_operations.html"))
            .expect("place-guard abstract operations should be written");
        let footprints = fs::read_to_string(output.join("08_boundary_footprints.json"))
            .expect("place-guard footprint evidence should be written");
        assert!(
            abstract_operations.contains("ComparePlaces"),
            "{target} canary must exercise the place-pair guard encoder"
        );
        assert!(
            footprints.contains("\"origin\": \"place_guard_comparison\"")
                && footprints.contains(expected_registers)
                && footprints.contains("\"enumeration_complete\": false"),
            "{target} artifact must retain exact place-guard evidence without claiming completeness"
        );
        let _ = fs::remove_dir_all(&scratch);
    }
}

#[test]
fn compiler_body_place_copy_footprints_reach_x86_and_aarch64_artifacts() {
    let canary = pass_canary("calls/runtime_value_call_through_alias_in_dispatch_exit");
    for (target, expected_registers) in [
        ("linux_x86_64", "[\"X86Rax\", \"X86R14\", \"X86R15\"]"),
        (
            "linux_arm64",
            "[\"Aarch64X(16)\", \"Aarch64X(17)\", \"Aarch64X(20)\", \"Aarch64X(26)\"]",
        ),
    ] {
        let scratch = std::env::temp_dir().join(format!(
            "omega-compiler-body-place-copy-footprint-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        let output = scratch.join("out");
        compile_rooted_canary_for_target_with_auxiliary_artifacts(&canary, output.clone(), target)
            .unwrap_or_else(|diagnostics| {
                panic!("compiler-body place copy should compile for {target}: {diagnostics:?}")
            });
        let abstract_operations = fs::read_to_string(output.join("08_abstract_operations.html"))
            .expect("compiler-body place-copy abstract operations should be written");
        let footprints = fs::read_to_string(output.join("08_boundary_footprints.json"))
            .expect("compiler-body place-copy footprint evidence should be written");
        assert!(
            abstract_operations.contains("CopyPlaces"),
            "{target} canary must exercise a CopyPlaces operation"
        );
        assert!(
            footprints.contains("\"origin\": \"compiler_body_place_copy\"")
                && footprints.contains(expected_registers)
                && footprints.contains("\"enumeration_complete\": false"),
            "{target} artifact must retain the ordinary direct/pointee-copy footprint without claiming completeness"
        );
        let _ = fs::remove_dir_all(&scratch);
    }
}

#[test]
fn compiler_body_from_pointee_copy_footprints_reach_x86_and_aarch64_artifacts() {
    let canary = pass_canary("calls/runtime_shared_ref_param_copy_exit");
    for (target, expected_registers) in [
        ("linux_x86_64", "[\"X86Rax\", \"X86R14\", \"X86R15\"]"),
        (
            "linux_arm64",
            "[\"Aarch64X(16)\", \"Aarch64X(17)\", \"Aarch64X(20)\", \"Aarch64X(26)\"]",
        ),
    ] {
        let scratch = std::env::temp_dir().join(format!(
            "omega-compiler-body-from-pointee-footprint-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        let source = scratch.join("src");
        let output = scratch.join("out");
        fs::create_dir_all(&source).expect("create compiler-body from-pointee source directory");
        fs::copy(canary.join("main.omg"), source.join("main.omg"))
            .expect("copy compiler-body from-pointee canary");
        fs::write(
            source.join("build.omg"),
            hosted_main_program_entry_build(target),
        )
        .expect("write compiler-body from-pointee target");

        compile_with_auxiliary_artifacts(CanaryCompileSpec {
            root_path: source.join("main.omg"),
            build_dir: Some(output.clone()),
            target_name: Some(target.into()),
            product: CanaryCompileProduct::NativeArtifactAndPublish,
        })
        .unwrap_or_else(|diagnostics| {
            panic!("compiler-body from-pointee copy should compile for {target}: {diagnostics:?}")
        });
        let abstract_operations = fs::read_to_string(output.join("08_abstract_operations.html"))
            .expect("compiler-body from-pointee abstract operations should be written");
        let footprints = fs::read_to_string(output.join("08_boundary_footprints.json"))
            .expect("compiler-body from-pointee footprint evidence should be written");
        assert!(
            abstract_operations.contains("CopyPlaces"),
            "{target} canary must exercise a dereferenced-source CopyPlaces operation"
        );
        assert!(
            footprints.contains("\"origin\": \"compiler_body_place_copy\"")
                && footprints.contains(expected_registers)
                && footprints.contains("\"enumeration_complete\": false"),
            "{target} artifact must retain the ordinary from-pointee footprint without claiming completeness"
        );
        let _ = fs::remove_dir_all(&scratch);
    }
}

#[test]
fn runtime_shared_ref_param_copy_exit_canary_runs() {
    let canary = pass_canary("calls/runtime_shared_ref_param_copy_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-shared-ref-param-copy-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("shared ref-param direct-copy canary should compile");
    assert_native_exit_code(
        &compilation,
        42,
        "shared reference-parameter copy canary",
        "the dereferenced source field should copy into the receiver",
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn compiler_body_pointee_pair_copy_footprints_reach_x86_and_aarch64_artifacts() {
    let canary = pass_canary("calls/runtime_pointee_pair_copy_exit");
    for (target, expected_registers) in [
        ("linux_x86_64", "[\"X86Rax\", \"X86R14\", \"X86R15\"]"),
        (
            "linux_arm64",
            "[\"Aarch64X(16)\", \"Aarch64X(17)\", \"Aarch64X(20)\", \"Aarch64X(26)\"]",
        ),
    ] {
        let scratch = std::env::temp_dir().join(format!(
            "omega-compiler-body-pointee-pair-footprint-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        let source = scratch.join("src");
        let output = scratch.join("out");
        fs::create_dir_all(&source).expect("create compiler-body pointee-pair source directory");
        fs::copy(canary.join("main.omg"), source.join("main.omg"))
            .expect("copy compiler-body pointee-pair canary");
        fs::write(
            source.join("build.omg"),
            hosted_main_program_entry_build(target),
        )
        .expect("write compiler-body pointee-pair target");

        compile_with_auxiliary_artifacts(CanaryCompileSpec {
            root_path: source.join("main.omg"),
            build_dir: Some(output.clone()),
            target_name: Some(target.into()),
            product: CanaryCompileProduct::NativeArtifactAndPublish,
        })
        .unwrap_or_else(|diagnostics| {
            panic!("compiler-body pointee-pair copy should compile for {target}: {diagnostics:?}")
        });
        let abstract_operations = fs::read_to_string(output.join("08_abstract_operations.html"))
            .expect("compiler-body pointee-pair abstract operations should be written");
        let footprints = fs::read_to_string(output.join("08_boundary_footprints.json"))
            .expect("compiler-body pointee-pair footprint evidence should be written");
        assert!(
            abstract_operations.contains("CopyPlaces"),
            "{target} canary must exercise a pointee-pair CopyPlaces operation"
        );
        assert!(
            footprints.contains("\"origin\": \"compiler_body_place_copy\"")
                && footprints.contains(expected_registers)
                && footprints.contains("\"enumeration_complete\": false"),
            "{target} artifact must retain the ordinary pointee-pair footprint without claiming completeness"
        );
        let _ = fs::remove_dir_all(&scratch);
    }
}

#[test]
fn runtime_pointee_pair_copy_exit_canary_runs() {
    let canary = pass_canary("calls/runtime_pointee_pair_copy_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-pointee-pair-copy-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("pointee-pair copy canary should compile");
    assert_native_exit_code(
        &compilation,
        42,
        "pointee-pair copy canary",
        "the field value should copy through the source and target references",
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn compiler_body_from_indexed_copy_footprints_reach_x86_and_aarch64_artifacts() {
    let canary = pass_canary("slices/runtime_slice_element_runtime_index_read_exit");
    for (target, expected_registers) in [
        (
            "linux_x86_64",
            "[\"X86Rax\", \"X86R11\", \"X86R14\", \"X86R15\"]",
        ),
        (
            "linux_arm64",
            "[\"Aarch64X(16)\", \"Aarch64X(17)\", \"Aarch64X(19)\", \"Aarch64X(20)\", \"Aarch64X(21)\", \"Aarch64X(26)\"]",
        ),
    ] {
        let scratch = std::env::temp_dir().join(format!(
            "omega-compiler-body-from-indexed-footprint-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        let source = scratch.join("src");
        let output = scratch.join("out");
        fs::create_dir_all(&source).expect("create compiler-body from-indexed source directory");
        fs::copy(canary.join("main.omg"), source.join("main.omg"))
            .expect("copy compiler-body from-indexed canary");
        fs::write(
            source.join("build.omg"),
            hosted_main_program_entry_build(target),
        )
        .expect("write compiler-body from-indexed target");

        compile_with_auxiliary_artifacts(CanaryCompileSpec {
            root_path: source.join("main.omg"),
            build_dir: Some(output.clone()),
            target_name: Some(target.into()),
            product: CanaryCompileProduct::NativeArtifactAndPublish,
        })
        .unwrap_or_else(|diagnostics| {
            panic!("compiler-body from-indexed copy should compile for {target}: {diagnostics:?}")
        });
        let abstract_operations = fs::read_to_string(output.join("08_abstract_operations.html"))
            .expect("compiler-body from-indexed abstract operations should be written");
        let footprints = fs::read_to_string(output.join("08_boundary_footprints.json"))
            .expect("compiler-body from-indexed footprint evidence should be written");
        assert!(
            abstract_operations.contains("CopyPlaces"),
            "{target} canary must exercise a runtime-indexed-source CopyPlaces operation"
        );
        assert!(
            footprints.contains("\"origin\": \"compiler_body_place_copy\"")
                && footprints.contains(expected_registers)
                && footprints.contains("\"enumeration_complete\": false"),
            "{target} artifact must retain the ordinary from-indexed footprint without claiming completeness"
        );
        let _ = fs::remove_dir_all(&scratch);
    }
}

#[test]
fn compiler_body_to_indexed_copy_footprints_reach_x86_and_aarch64_artifacts() {
    let canary = pass_canary("collections/runtime_fixed_vec_round_trip_exit");
    for (target, expected_registers) in [
        (
            "linux_x86_64",
            "[\"X86Rax\", \"X86R11\", \"X86R14\", \"X86R15\"]",
        ),
        (
            "linux_arm64",
            "[\"Aarch64X(16)\", \"Aarch64X(17)\", \"Aarch64X(19)\", \"Aarch64X(20)\", \"Aarch64X(21)\", \"Aarch64X(26)\"]",
        ),
    ] {
        let scratch = std::env::temp_dir().join(format!(
            "omega-compiler-body-to-indexed-footprint-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        let source = scratch.join("src");
        let output = scratch.join("out");
        fs::create_dir_all(&source).expect("create compiler-body to-indexed source directory");
        fs::copy(canary.join("main.omg"), source.join("main.omg"))
            .expect("copy compiler-body to-indexed canary");
        fs::create_dir_all(source.join("platform"))
            .expect("create compiler-body to-indexed platform directory");
        fs::copy(
            canary.join("platform/console.omg"),
            source.join("platform/console.omg"),
        )
        .expect("copy compiler-body to-indexed platform binding");
        fs::write(
            source.join("build.omg"),
            hosted_main_program_entry_build(target),
        )
        .expect("write compiler-body to-indexed target");

        compile_with_auxiliary_artifacts(CanaryCompileSpec {
            root_path: source.join("main.omg"),
            build_dir: Some(output.clone()),
            target_name: Some(target.into()),
            product: CanaryCompileProduct::NativeArtifactAndPublish,
        })
        .unwrap_or_else(|diagnostics| {
            panic!("compiler-body to-indexed copy should compile for {target}: {diagnostics:?}")
        });
        let abstract_operations = fs::read_to_string(output.join("08_abstract_operations.html"))
            .expect("compiler-body to-indexed abstract operations should be written");
        let footprints = fs::read_to_string(output.join("08_boundary_footprints.json"))
            .expect("compiler-body to-indexed footprint evidence should be written");
        assert!(
            abstract_operations.contains("CopyPlaces"),
            "{target} canary must exercise a runtime-indexed-target CopyPlaces operation"
        );
        assert!(
            footprints.contains("\"origin\": \"compiler_body_place_copy\"")
                && footprints.contains(expected_registers)
                && footprints.contains("\"enumeration_complete\": false"),
            "{target} artifact must retain the ordinary to-indexed footprint without claiming completeness"
        );
        let _ = fs::remove_dir_all(&scratch);
    }
}

#[test]
fn compiler_body_indexed_to_pointee_copy_footprints_reach_x86_and_aarch64_artifacts() {
    let canary = pass_canary("calls/runtime_alias_indexed_read_through_transition_exit");
    for (target, expected_registers) in [
        (
            "linux_x86_64",
            "[\"X86Rax\", \"X86R11\", \"X86R14\", \"X86R15\"]",
        ),
        (
            "linux_arm64",
            "[\"Aarch64X(16)\", \"Aarch64X(17)\", \"Aarch64X(19)\", \"Aarch64X(20)\", \"Aarch64X(21)\", \"Aarch64X(26)\"]",
        ),
    ] {
        let scratch = std::env::temp_dir().join(format!(
            "omega-compiler-body-indexed-to-pointee-footprint-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        let output = scratch.join("out");
        compile_rooted_canary_for_target_with_auxiliary_artifacts(
            &canary,
            output.clone(),
            target,
        )
        .unwrap_or_else(|diagnostics| {
            panic!(
                "compiler-body indexed-to-pointee copy should compile for {target}: {diagnostics:?}"
            )
        });
        let abstract_operations = fs::read_to_string(output.join("08_abstract_operations.html"))
            .expect("compiler-body indexed-to-pointee abstract operations should be written");
        let footprints = fs::read_to_string(output.join("08_boundary_footprints.json"))
            .expect("compiler-body indexed-to-pointee footprint evidence should be written");
        assert!(
            abstract_operations.contains("CopyPlaces"),
            "{target} canary must exercise an indexed-to-pointee CopyPlaces operation"
        );
        assert!(
            footprints.contains("\"origin\": \"compiler_body_place_copy\"")
                && footprints.contains(expected_registers)
                && footprints.contains("\"enumeration_complete\": false"),
            "{target} artifact must retain the ordinary indexed-to-pointee footprint without claiming completeness"
        );
        let _ = fs::remove_dir_all(&scratch);
    }
}

#[test]
fn compiler_body_cross_region_frame_base_indexed_write_footprints_reach_artifacts() {
    let canary = pass_canary("collections/runtime_frame_indexed_local_read_exit");
    for (target, expected_register) in [
        ("linux_x86_64", "\"X86R10\""),
        ("linux_arm64", "\"Aarch64X(15)\""),
    ] {
        let scratch = std::env::temp_dir().join(format!(
            "omega-compiler-body-frame-base-indexed-footprint-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        let source = scratch.join("src");
        let output = scratch.join("out");
        fs::create_dir_all(&source)
            .expect("create compiler-body frame-base-indexed source directory");
        fs::copy(canary.join("main.omg"), source.join("main.omg"))
            .expect("copy compiler-body frame-base-indexed canary");
        fs::write(
            source.join("build.omg"),
            hosted_main_program_entry_build(target),
        )
        .expect("write compiler-body frame-base-indexed target");

        compile_with_auxiliary_artifacts(CanaryCompileSpec {
            root_path: source.join("main.omg"),
            build_dir: Some(output.clone()),
            target_name: Some(target.into()),
            product: CanaryCompileProduct::NativeArtifactAndPublish,
        })
        .unwrap_or_else(|diagnostics| {
            panic!(
                "compiler-body cross-region frame-base-indexed writes should compile for {target}: {diagnostics:?}"
            )
        });
        let abstract_operations = fs::read_to_string(output.join("08_abstract_operations.html"))
            .expect("compiler-body frame-base-indexed abstract operations should be written");
        let footprints = fs::read_to_string(output.join("08_boundary_footprints.json"))
            .expect("compiler-body frame-base-indexed footprint evidence should be written");
        assert!(
            abstract_operations.contains("CopyPlaces")
                && abstract_operations.contains("WritePlaceInteger")
                && abstract_operations.contains("WritePlaceBinary")
                && abstract_operations.contains("WritePlaceConvert")
                && abstract_operations.contains("WritePlaceString")
                && abstract_operations.contains("WritePlaceBoundedBuffer")
                && abstract_operations.contains("AppendPlaceBoundedBufferSource")
                && abstract_operations.contains("AppendPlaceBoundedBufferLiteral")
                && abstract_operations.contains("WritePlaceAddress"),
            "{target} canary must exercise frame-base-indexed copy, immediate, binary, conversion, string, carrier mutation, and place-address operations"
        );
        assert!(
            footprints.contains("\"origin\": \"compiler_body_place_copy\"")
                && footprints.contains("\"origin\": \"compiler_body_place_integer_write\"")
                && footprints.contains("\"origin\": \"compiler_body_place_binary_write\"")
                && footprints.contains("\"origin\": \"compiler_body_storage_convert_write\"")
                && footprints.contains("\"origin\": \"compiler_body_place_string_write\"")
                && footprints.contains("\"origin\": \"compiler_body_place_bounded_buffer_write\"")
                && footprints.contains("\"origin\": \"compiler_body_place_address_write\"")
                && footprints.contains(expected_register)
                && footprints.contains("\"enumeration_complete\": false"),
            "{target} artifact must retain the ordinary frame-base-indexed footprints without claiming completeness"
        );
        let _ = fs::remove_dir_all(&scratch);
    }
}

#[test]
fn compiler_body_machine_indexed_copy_footprints_reach_x86_and_aarch64_artifacts() {
    let canary = pass_canary("calls/runtime_machine_indexed_struct_field_arg_exit");
    for (target, expected_registers) in [
        (
            "linux_x86_64",
            "[\"X86Rax\", \"X86R11\", \"X86R14\", \"X86R15\"]",
        ),
        (
            "linux_arm64",
            "[\"Aarch64X(16)\", \"Aarch64X(17)\", \"Aarch64X(19)\", \"Aarch64X(20)\", \"Aarch64X(26)\"]",
        ),
    ] {
        let scratch = std::env::temp_dir().join(format!(
            "omega-compiler-body-machine-indexed-footprint-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        let source = scratch.join("src");
        let output = scratch.join("out");
        fs::create_dir_all(&source).expect("create compiler-body machine-indexed source directory");
        fs::copy(canary.join("main.omg"), source.join("main.omg"))
            .expect("copy compiler-body machine-indexed canary");
        fs::write(
            source.join("build.omg"),
            hosted_main_program_entry_build(target),
        )
        .expect("write compiler-body machine-indexed target");

        compile_with_auxiliary_artifacts(CanaryCompileSpec {
            root_path: source.join("main.omg"),
            build_dir: Some(output.clone()),
            target_name: Some(target.into()),
            product: CanaryCompileProduct::NativeArtifactAndPublish,
        })
        .unwrap_or_else(|diagnostics| {
            panic!(
                "compiler-body machine-indexed copy should compile for {target}: {diagnostics:?}"
            )
        });
        let abstract_operations = fs::read_to_string(output.join("08_abstract_operations.html"))
            .expect("compiler-body machine-indexed abstract operations should be written");
        let footprints = fs::read_to_string(output.join("08_boundary_footprints.json"))
            .expect("compiler-body machine-indexed footprint evidence should be written");
        assert!(
            abstract_operations.contains("CopyPlaces"),
            "{target} canary must exercise a machine-indexed CopyPlaces operation"
        );
        assert!(
            footprints.contains("\"origin\": \"compiler_body_place_copy\"")
                && footprints.contains(expected_registers)
                && footprints.contains("\"enumeration_complete\": false"),
            "{target} artifact must retain the ordinary machine-indexed footprint without claiming completeness"
        );
        let _ = fs::remove_dir_all(&scratch);
    }
}

#[test]
fn compiler_body_to_machine_indexed_copy_footprints_reach_x86_and_aarch64_artifacts() {
    let canary = pass_canary("collections/runtime_machine_frame_index_write_exit");
    for (target, expected_registers) in [
        (
            "linux_x86_64",
            "[\"X86Rax\", \"X86R11\", \"X86R14\", \"X86R15\"]",
        ),
        (
            "linux_arm64",
            "[\"Aarch64X(16)\", \"Aarch64X(17)\", \"Aarch64X(19)\", \"Aarch64X(20)\", \"Aarch64X(26)\"]",
        ),
    ] {
        let scratch = std::env::temp_dir().join(format!(
            "omega-compiler-body-to-machine-indexed-footprint-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        let source = scratch.join("src");
        let output = scratch.join("out");
        fs::create_dir_all(&source)
            .expect("create compiler-body to-machine-indexed source directory");
        fs::copy(canary.join("main.omg"), source.join("main.omg"))
            .expect("copy compiler-body to-machine-indexed canary");
        fs::write(
            source.join("build.omg"),
            hosted_main_program_entry_build(target),
        )
        .expect("write compiler-body to-machine-indexed target");

        compile_with_auxiliary_artifacts(CanaryCompileSpec {
            root_path: source.join("main.omg"),
            build_dir: Some(output.clone()),
            target_name: Some(target.into()),
            product: CanaryCompileProduct::NativeArtifactAndPublish,
        })
        .unwrap_or_else(|diagnostics| {
            panic!(
                "compiler-body to-machine-indexed copy should compile for {target}: {diagnostics:?}"
            )
        });
        let abstract_operations = fs::read_to_string(output.join("08_abstract_operations.html"))
            .expect("compiler-body to-machine-indexed abstract operations should be written");
        let footprints = fs::read_to_string(output.join("08_boundary_footprints.json"))
            .expect("compiler-body to-machine-indexed footprint evidence should be written");
        assert!(
            abstract_operations.contains("CopyPlaces"),
            "{target} canary must exercise a to-machine-indexed CopyPlaces operation"
        );
        assert!(
            footprints.contains("\"origin\": \"compiler_body_place_copy\"")
                && footprints.contains(expected_registers)
                && footprints.contains("\"enumeration_complete\": false"),
            "{target} artifact must retain the ordinary to-machine-indexed footprint without claiming completeness"
        );
        let _ = fs::remove_dir_all(&scratch);
    }
}

#[test]
fn compiler_body_frame_double_indexed_write_footprints_reach_both_artifacts() {
    let canary = pass_canary("collections/runtime_frame_double_indexed_read_exit");
    for (target, expected_register) in [
        ("linux_x86_64", "\"X86R11\""),
        ("linux_arm64", "\"Aarch64X(15)\""),
    ] {
        let scratch = std::env::temp_dir().join(format!(
            "omega-compiler-body-frame-double-indexed-footprint-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        let source = scratch.join("src");
        let output = scratch.join("out");
        fs::create_dir_all(&source)
            .expect("create compiler-body frame-double-indexed source directory");
        fs::copy(canary.join("main.omg"), source.join("main.omg"))
            .expect("copy compiler-body frame-double-indexed canary");
        fs::write(
            source.join("build.omg"),
            hosted_main_program_entry_build(target),
        )
        .expect("write compiler-body frame-double-indexed target");
        compile_with_auxiliary_artifacts(CanaryCompileSpec {
            root_path: source.join("main.omg"),
            build_dir: Some(output.clone()),
            target_name: Some(target.into()),
            product: CanaryCompileProduct::NativeArtifactAndPublish,
        })
        .unwrap_or_else(|diagnostics| {
            panic!("compiler-body frame-double-indexed operations should compile for {target}: {diagnostics:?}")
        });
        let footprints = fs::read_to_string(output.join("08_boundary_footprints.json"))
            .expect("compiler-body frame-double-indexed footprint evidence should be written");
        let abstract_operations = fs::read_to_string(output.join("08_abstract_operations.html"))
            .expect("compiler-body frame-double-indexed operations should be written");
        assert!(
            footprints.contains("\"origin\": \"compiler_body_place_copy\"")
                && footprints.contains("\"origin\": \"compiler_body_place_integer_write\"")
                && footprints.contains("\"origin\": \"compiler_body_place_binary_write\"")
                && footprints.contains("\"origin\": \"compiler_body_storage_convert_write\"")
                && footprints.contains("\"origin\": \"compiler_body_place_address_write\"")
                && footprints.contains("\"origin\": \"compiler_body_place_string_write\"")
                && abstract_operations.contains("AppendPlaceBoundedBufferLiteral")
                && abstract_operations.contains("AppendPlaceBoundedBufferSource")
                && abstract_operations.contains("AppendTextLiteralToPlace")
                && abstract_operations.contains("AppendTextStoredToPlace")
                && abstract_operations.contains("WritePlaceAddress")
                && footprints.contains(expected_register)
                && footprints.contains("\"enumeration_complete\": false"),
            "{target} artifact must retain the frame-double-indexed read/write copy, integer, binary, conversion, address, string/text assembly, and bounded-buffer write/append footprints without claiming completeness"
        );
        let _ = fs::remove_dir_all(&scratch);
    }
}

#[test]
fn compiler_body_machine_double_indexed_copy_footprints_reach_x86_and_aarch64_artifacts() {
    let canary = pass_canary("collections/runtime_double_indexed_read_exit");
    for (target, expected_register) in [
        ("linux_x86_64", "\"X86R10\""),
        ("linux_arm64", "\"Aarch64X(15)\""),
    ] {
        let scratch = std::env::temp_dir().join(format!(
            "omega-compiler-body-machine-double-indexed-footprint-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        let source = scratch.join("src");
        let output = scratch.join("out");
        fs::create_dir_all(&source)
            .expect("create compiler-body machine-double-indexed source directory");
        fs::copy(canary.join("main.omg"), source.join("main.omg"))
            .expect("copy compiler-body machine-double-indexed canary");
        fs::write(
            source.join("build.omg"),
            hosted_main_program_entry_build(target),
        )
        .expect("write compiler-body machine-double-indexed target");
        compile_with_auxiliary_artifacts(CanaryCompileSpec {
            root_path: source.join("main.omg"),
            build_dir: Some(output.clone()),
            target_name: Some(target.into()),
            product: CanaryCompileProduct::NativeArtifactAndPublish,
        })
        .unwrap_or_else(|diagnostics| {
            panic!("compiler-body machine-double-indexed copy should compile for {target}: {diagnostics:?}")
        });
        let footprints = fs::read_to_string(output.join("08_boundary_footprints.json"))
            .expect("compiler-body machine-double-indexed footprint evidence should be written");
        assert!(
            footprints.contains("\"origin\": \"compiler_body_place_copy\"")
                && footprints.contains(expected_register)
                && footprints.contains("\"enumeration_complete\": false"),
            "{target} artifact must retain the ordinary machine-double-indexed footprint without claiming completeness"
        );
        let _ = fs::remove_dir_all(&scratch);
    }
}

#[test]
fn compiler_body_to_machine_double_indexed_copy_footprints_reach_x86_and_aarch64_artifacts() {
    let canary = pass_canary("collections/runtime_double_indexed_write_exit");
    for (target, expected_register) in [
        ("linux_x86_64", "\"X86R10\""),
        ("linux_arm64", "\"Aarch64X(24)\""),
    ] {
        let scratch = std::env::temp_dir().join(format!(
            "omega-compiler-body-to-machine-double-indexed-footprint-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        let source = scratch.join("src");
        let output = scratch.join("out");
        fs::create_dir_all(&source)
            .expect("create compiler-body to-machine-double-indexed source directory");
        fs::copy(canary.join("main.omg"), source.join("main.omg"))
            .expect("copy compiler-body to-machine-double-indexed canary");
        fs::write(
            source.join("build.omg"),
            hosted_main_program_entry_build(target),
        )
        .expect("write compiler-body to-machine-double-indexed target");
        compile_with_auxiliary_artifacts(CanaryCompileSpec {
            root_path: source.join("main.omg"),
            build_dir: Some(output.clone()),
            target_name: Some(target.into()),
            product: CanaryCompileProduct::NativeArtifactAndPublish,
        })
        .unwrap_or_else(|diagnostics| {
            panic!("compiler-body to-machine-double-indexed copy should compile for {target}: {diagnostics:?}")
        });
        let footprints = fs::read_to_string(output.join("08_boundary_footprints.json"))
            .expect("compiler-body to-machine-double-indexed footprint evidence should be written");
        let abstract_operations = fs::read_to_string(output.join("08_abstract_operations.html"))
            .expect("compiler-body to-machine-double-indexed operations should be written");
        assert!(
            footprints.contains("\"origin\": \"compiler_body_place_copy\"")
                && footprints.contains("\"origin\": \"compiler_body_place_address_write\"")
                && abstract_operations.contains("WritePlaceAddress")
                && footprints.contains(expected_register)
                && footprints.contains("\"enumeration_complete\": false"),
            "{target} artifact must retain the ordinary to-machine-double-indexed copy and address footprints without claiming completeness"
        );
        let _ = fs::remove_dir_all(&scratch);
    }
}

#[test]
fn compiler_body_machine_indexed_pair_copy_footprints_reach_x86_and_aarch64_artifacts() {
    let canary = pass_canary("collections/runtime_dual_indexed_copy_exit");
    for (target, expected_register) in [
        ("linux_x86_64", "\"X86R11\""),
        ("linux_arm64", "\"Aarch64X(24)\""),
    ] {
        let scratch = std::env::temp_dir().join(format!(
            "omega-compiler-body-machine-indexed-pair-footprint-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        let source = scratch.join("src");
        let output = scratch.join("out");
        fs::create_dir_all(&source)
            .expect("create compiler-body machine-indexed-pair source directory");
        fs::copy(canary.join("main.omg"), source.join("main.omg"))
            .expect("copy compiler-body machine-indexed-pair canary");
        fs::write(
            source.join("build.omg"),
            hosted_main_program_entry_build(target),
        )
        .expect("write compiler-body machine-indexed-pair target");
        compile_with_auxiliary_artifacts(CanaryCompileSpec {
            root_path: source.join("main.omg"),
            build_dir: Some(output.clone()),
            target_name: Some(target.into()),
            product: CanaryCompileProduct::NativeArtifactAndPublish,
        })
        .unwrap_or_else(|diagnostics| {
            panic!("compiler-body machine-indexed-pair copy should compile for {target}: {diagnostics:?}")
        });
        let footprints = fs::read_to_string(output.join("08_boundary_footprints.json"))
            .expect("compiler-body machine-indexed-pair footprint evidence should be written");
        assert!(
            footprints.contains("\"origin\": \"compiler_body_place_copy\"")
                && footprints.contains(expected_register)
                && footprints.contains("\"enumeration_complete\": false"),
            "{target} artifact must retain the ordinary machine-indexed-pair footprint without claiming completeness"
        );
        let _ = fs::remove_dir_all(&scratch);
    }
}

#[test]
fn compiler_body_mixed_index_frame_pair_copy_footprints_reach_x86_and_aarch64_artifacts() {
    let canary = pass_canary("collections/runtime_frame_mixed_index_pair_copy_exit");
    for (target, expected_register) in [
        ("linux_x86_64", "\"X86R11\""),
        ("linux_arm64", "\"Aarch64X(15)\""),
    ] {
        let scratch = std::env::temp_dir().join(format!(
            "omega-compiler-body-mixed-index-frame-pair-footprint-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        let source = scratch.join("src");
        let output = scratch.join("out");
        fs::create_dir_all(&source)
            .expect("create compiler-body mixed-index frame-pair source directory");
        fs::copy(canary.join("main.omg"), source.join("main.omg"))
            .expect("copy compiler-body mixed-index frame-pair canary");
        fs::write(
            source.join("build.omg"),
            hosted_main_program_entry_build(target),
        )
        .expect("write compiler-body mixed-index frame-pair target");
        compile_with_auxiliary_artifacts(CanaryCompileSpec {
            root_path: source.join("main.omg"),
            build_dir: Some(output.clone()),
            target_name: Some(target.into()),
            product: CanaryCompileProduct::NativeArtifactAndPublish,
        })
        .unwrap_or_else(|diagnostics| {
            panic!(
                "compiler-body mixed-index frame-pair copy should compile for {target}: {diagnostics:?}"
            )
        });
        let footprints = fs::read_to_string(output.join("08_boundary_footprints.json"))
            .expect("compiler-body mixed-index frame-pair footprint evidence should be written");
        assert!(
            footprints.contains("\"origin\": \"compiler_body_place_copy\"")
                && footprints.contains(expected_register)
                && footprints.contains("\"enumeration_complete\": false"),
            "{target} artifact must retain the ordinary mixed-index frame-pair footprint without claiming completeness"
        );
        let _ = fs::remove_dir_all(&scratch);
    }
}

#[test]
fn compiler_body_cross_region_indexed_pair_footprints_reach_x86_and_aarch64_artifacts() {
    let canary = pass_canary("collections/runtime_cross_region_indexed_pair_copy_exit");
    for (target, expected_register) in [
        ("linux_x86_64", "\"X86R11\""),
        ("linux_arm64", "\"Aarch64X(15)\""),
    ] {
        let scratch = std::env::temp_dir().join(format!(
            "omega-compiler-body-cross-region-indexed-pair-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        let source = scratch.join("src");
        let output = scratch.join("out");
        fs::create_dir_all(&source)
            .expect("create compiler-body cross-region indexed-pair source directory");
        fs::copy(canary.join("main.omg"), source.join("main.omg"))
            .expect("copy compiler-body cross-region indexed-pair canary");
        fs::write(
            source.join("build.omg"),
            hosted_main_program_entry_build(target),
        )
        .expect("write compiler-body cross-region indexed-pair target");
        compile_with_auxiliary_artifacts(CanaryCompileSpec {
            root_path: source.join("main.omg"),
            build_dir: Some(output.clone()),
            target_name: Some(target.into()),
            product: CanaryCompileProduct::NativeArtifactAndPublish,
        })
        .unwrap_or_else(|diagnostics| {
            panic!(
                "compiler-body cross-region indexed-pair copy should compile for {target}: {diagnostics:?}"
            )
        });
        let footprints = fs::read_to_string(output.join("08_boundary_footprints.json"))
            .expect("compiler-body cross-region indexed-pair footprint evidence should be written");
        assert!(
            footprints.contains("\"origin\": \"compiler_body_place_copy\"")
                && footprints.contains(expected_register)
                && footprints.contains("\"enumeration_complete\": false"),
            "{target} artifact must retain the cross-region indexed-pair footprint without claiming completeness"
        );
        let _ = fs::remove_dir_all(&scratch);
    }
}

#[test]
fn compiler_body_cross_region_double_indexed_pair_footprints_reach_x86_and_aarch64_artifacts() {
    let canary = pass_canary("collections/runtime_cross_region_double_indexed_pair_copy_exit");
    for (target, expected_register) in [
        ("linux_x86_64", "\"X86R10\""),
        ("linux_arm64", "\"Aarch64X(20)\""),
    ] {
        let scratch = std::env::temp_dir().join(format!(
            "omega-compiler-body-cross-region-double-indexed-pair-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        let source = scratch.join("src");
        let output = scratch.join("out");
        fs::create_dir_all(&source)
            .expect("create compiler-body cross-region double-indexed-pair source directory");
        fs::copy(canary.join("main.omg"), source.join("main.omg"))
            .expect("copy compiler-body cross-region double-indexed-pair canary");
        fs::write(
            source.join("build.omg"),
            hosted_main_program_entry_build(target),
        )
        .expect("write compiler-body cross-region double-indexed-pair target");
        compile_with_auxiliary_artifacts(CanaryCompileSpec {
            root_path: source.join("main.omg"),
            build_dir: Some(output.clone()),
            target_name: Some(target.into()),
            product: CanaryCompileProduct::NativeArtifactAndPublish,
        })
        .unwrap_or_else(|diagnostics| {
            panic!(
                "compiler-body cross-region double-indexed-pair copy should compile for {target}: {diagnostics:?}"
            )
        });
        let footprints = fs::read_to_string(output.join("08_boundary_footprints.json")).expect(
            "compiler-body cross-region double-indexed-pair footprint evidence should be written",
        );
        assert!(
            footprints.contains("\"origin\": \"compiler_body_place_copy\"")
                && footprints.contains(expected_register)
                && footprints.contains("\"enumeration_complete\": false"),
            "{target} artifact must retain the cross-region double-indexed-pair footprint without claiming completeness"
        );
        let _ = fs::remove_dir_all(&scratch);
    }
}

#[test]
fn compiler_body_direct_integer_write_footprints_reach_x86_and_aarch64_artifacts() {
    let canary = pass_canary("collections/runtime_dual_indexed_copy_exit");
    for (target, expected_register) in [
        ("linux_x86_64", "\"X86R15\""),
        ("linux_arm64", "\"Aarch64X(17)\""),
    ] {
        let scratch = std::env::temp_dir().join(format!(
            "omega-compiler-body-direct-integer-write-footprint-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        let source = scratch.join("src");
        let output = scratch.join("out");
        fs::create_dir_all(&source)
            .expect("create compiler-body direct integer-write source directory");
        fs::copy(canary.join("main.omg"), source.join("main.omg"))
            .expect("copy compiler-body direct integer-write canary");
        fs::write(
            source.join("build.omg"),
            hosted_main_program_entry_build(target),
        )
        .expect("write compiler-body direct integer-write target");
        compile_with_auxiliary_artifacts(CanaryCompileSpec {
            root_path: source.join("main.omg"),
            build_dir: Some(output.clone()),
            target_name: Some(target.into()),
            product: CanaryCompileProduct::NativeArtifactAndPublish,
        })
        .unwrap_or_else(|diagnostics| {
            panic!(
                "compiler-body direct integer writes should compile for {target}: {diagnostics:?}"
            )
        });
        let footprints = fs::read_to_string(output.join("08_boundary_footprints.json"))
            .expect("compiler-body direct integer-write footprint evidence should be written");
        assert!(
            footprints.contains("\"origin\": \"compiler_body_place_integer_write\"")
                && footprints.contains(expected_register)
                && footprints.contains("\"enumeration_complete\": false"),
            "{target} artifact must retain the direct integer-write footprint without claiming completeness"
        );
        let _ = fs::remove_dir_all(&scratch);
    }
}

#[test]
fn compiler_body_direct_binary_write_footprints_reach_x86_and_aarch64_artifacts() {
    // The left-associative f32 chain retains nested Binary operand roots, so
    // this covers both the outer target relocation and recursive evaluator
    // relocation/footprint replay on each architecture.
    let canary = pass_canary("expressions/f32_deep_chain_binary");
    for (target, expected_register) in [
        ("linux_x86_64", "\"X86R14\""),
        ("linux_arm64", "\"Aarch64X(16)\""),
    ] {
        let scratch = std::env::temp_dir().join(format!(
            "omega-compiler-body-direct-binary-write-footprint-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        let source = scratch.join("src");
        let output = scratch.join("out");
        fs::create_dir_all(&source)
            .expect("create compiler-body direct binary-write source directory");
        fs::copy(canary.join("main.omg"), source.join("main.omg"))
            .expect("copy compiler-body direct binary-write canary");
        fs::write(
            source.join("build.omg"),
            hosted_main_program_entry_build(target),
        )
        .expect("write compiler-body direct binary-write target");
        compile_with_auxiliary_artifacts(CanaryCompileSpec {
            root_path: source.join("main.omg"),
            build_dir: Some(output.clone()),
            target_name: Some(target.into()),
            product: CanaryCompileProduct::NativeArtifactAndPublish,
        })
        .unwrap_or_else(|diagnostics| {
            panic!(
                "compiler-body direct binary writes should compile for {target}: {diagnostics:?}"
            )
        });
        let footprints = fs::read_to_string(output.join("08_boundary_footprints.json"))
            .expect("compiler-body direct binary-write footprint evidence should be written");
        assert!(
            footprints.contains("\"origin\": \"compiler_body_place_binary_write\"")
                && footprints.contains(expected_register)
                && footprints.contains("\"enumeration_complete\": false"),
            "{target} artifact must retain the direct binary-write footprint without claiming completeness"
        );
        let _ = fs::remove_dir_all(&scratch);
    }
}

#[test]
fn compiler_body_pointee_binary_write_footprints_reach_x86_and_aarch64_artifacts() {
    let canary = pass_canary("control_flow/runtime_statement_call_single_execution_exit");
    for (target, expected_register) in [
        ("linux_x86_64", "\"X86R14\""),
        ("linux_arm64", "\"Aarch64X(16)\""),
    ] {
        let scratch = std::env::temp_dir().join(format!(
            "omega-compiler-body-pointee-binary-write-footprint-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        let output = scratch.join("out");
        compile_rooted_canary_for_target_with_auxiliary_artifacts(&canary, output.clone(), target)
            .unwrap_or_else(|diagnostics| {
                panic!(
                "compiler-body pointee binary writes should compile for {target}: {diagnostics:?}"
            )
            });
        let footprints = fs::read_to_string(output.join("08_boundary_footprints.json"))
            .expect("compiler-body pointee binary-write footprint evidence should be written");
        assert!(
            footprints.contains("\"origin\": \"compiler_body_place_binary_write\"")
                && footprints.contains(expected_register)
                && footprints.contains("\"enumeration_complete\": false"),
            "{target} artifact must retain the pointee binary-write footprint without claiming completeness"
        );
        let _ = fs::remove_dir_all(&scratch);
    }
}

#[test]
fn compiler_body_frame_indexed_binary_write_footprints_reach_x86_and_aarch64_artifacts() {
    let canary = pass_canary("storage/runtime_slice_indexed_binary_rmw_exit");
    for (target, expected_register) in [
        ("linux_x86_64", "\"X86R14\""),
        ("linux_arm64", "\"Aarch64X(16)\""),
    ] {
        let scratch = std::env::temp_dir().join(format!(
            "omega-compiler-body-frame-indexed-binary-write-footprint-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        let source = scratch.join("src");
        let output = scratch.join("out");
        fs::create_dir_all(&source)
            .expect("create compiler-body frame-indexed binary-write source directory");
        fs::copy(canary.join("main.omg"), source.join("main.omg"))
            .expect("copy compiler-body frame-indexed binary-write canary");
        fs::write(
            source.join("build.omg"),
            hosted_main_program_entry_build(target),
        )
        .expect("write compiler-body frame-indexed binary-write target");
        compile_with_auxiliary_artifacts(CanaryCompileSpec {
            root_path: source.join("main.omg"),
            build_dir: Some(output.clone()),
            target_name: Some(target.into()),
            product: CanaryCompileProduct::NativeArtifactAndPublish,
        })
        .unwrap_or_else(|diagnostics| {
            panic!(
                "compiler-body frame-indexed binary writes should compile for {target}: {diagnostics:?}"
            )
        });
        let footprints = fs::read_to_string(output.join("08_boundary_footprints.json")).expect(
            "compiler-body frame-indexed binary-write footprint evidence should be written",
        );
        assert!(
            footprints.contains("\"origin\": \"compiler_body_place_binary_write\"")
                && footprints.contains(expected_register)
                && footprints.contains("\"enumeration_complete\": false"),
            "{target} artifact must retain the frame-indexed binary-write footprint without claiming completeness"
        );
        let _ = fs::remove_dir_all(&scratch);
    }
}

#[test]
fn compiler_body_frame_base_indexed_binary_write_footprints_reach_x86_and_aarch64_artifacts() {
    let canary = pass_canary("storage/runtime_dispatch_local_index_binary_write_exit");
    for (target, expected_register) in [
        ("linux_x86_64", "\"X86R14\""),
        ("linux_arm64", "\"Aarch64X(16)\""),
    ] {
        let scratch = std::env::temp_dir().join(format!(
            "omega-compiler-body-frame-base-indexed-binary-write-footprint-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        let output = scratch.join("out");
        compile_rooted_canary_for_target_with_auxiliary_artifacts(
            &canary,
            output.clone(),
            target,
        )
        .unwrap_or_else(|diagnostics| {
            panic!(
                "compiler-body frame-base-indexed binary writes should compile for {target}: {diagnostics:?}"
            )
        });
        let footprints = fs::read_to_string(output.join("08_boundary_footprints.json")).expect(
            "compiler-body frame-base-indexed binary-write footprint evidence should be written",
        );
        assert!(
            footprints.contains("\"origin\": \"compiler_body_place_binary_write\"")
                && footprints.contains(expected_register)
                && footprints.contains("\"enumeration_complete\": false"),
            "{target} artifact must retain the frame-base-indexed binary-write footprint without claiming completeness"
        );
        let _ = fs::remove_dir_all(&scratch);
    }
}

#[test]
fn compiler_body_machine_indexed_binary_write_footprints_reach_x86_and_aarch64_artifacts() {
    let canary = pass_canary("collections/runtime_indexed_rmw_loop_exit");
    for (target, expected_register) in [
        ("linux_x86_64", "\"X86R14\""),
        ("linux_arm64", "\"Aarch64X(16)\""),
    ] {
        let scratch = std::env::temp_dir().join(format!(
            "omega-compiler-body-machine-indexed-binary-write-footprint-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        let output = scratch.join("out");
        compile_rooted_canary_for_target_with_auxiliary_artifacts(
            &canary,
            output.clone(),
            target,
        )
        .unwrap_or_else(|diagnostics| {
            panic!(
                "compiler-body machine-indexed binary writes should compile for {target}: {diagnostics:?}"
            )
        });
        let footprints = fs::read_to_string(output.join("08_boundary_footprints.json")).expect(
            "compiler-body machine-indexed binary-write footprint evidence should be written",
        );
        assert!(
            footprints.contains("\"origin\": \"compiler_body_place_binary_write\"")
                && footprints.contains(expected_register)
                && footprints.contains("\"enumeration_complete\": false"),
            "{target} artifact must retain the machine-indexed binary-write footprint without claiming completeness"
        );
        let _ = fs::remove_dir_all(&scratch);
    }
}

#[test]
fn compiler_body_machine_double_indexed_binary_write_footprints_reach_x86_and_aarch64_artifacts() {
    let canary = pass_canary("collections/runtime_double_indexed_rmw_exit");
    for (target, expected_register) in [
        ("linux_x86_64", "\"X86R14\""),
        ("linux_arm64", "\"Aarch64X(16)\""),
    ] {
        let scratch = std::env::temp_dir().join(format!(
            "omega-compiler-body-machine-double-indexed-binary-write-footprint-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        let source = scratch.join("src");
        let output = scratch.join("out");
        fs::create_dir_all(&source)
            .expect("create compiler-body machine-double-indexed binary-write source directory");
        fs::copy(canary.join("main.omg"), source.join("main.omg"))
            .expect("copy compiler-body machine-double-indexed binary-write canary");
        fs::write(
            source.join("build.omg"),
            hosted_main_program_entry_build(target),
        )
        .expect("write compiler-body machine-double-indexed binary-write target");
        compile_with_auxiliary_artifacts(CanaryCompileSpec {
            root_path: source.join("main.omg"),
            build_dir: Some(output.clone()),
            target_name: Some(target.into()),
            product: CanaryCompileProduct::NativeArtifactAndPublish,
        })
        .unwrap_or_else(|diagnostics| {
            panic!(
                "compiler-body machine-double-indexed binary writes should compile for {target}: {diagnostics:?}"
            )
        });
        let footprints = fs::read_to_string(output.join("08_boundary_footprints.json")).expect(
            "compiler-body machine-double-indexed binary-write footprint evidence should be written",
        );
        assert!(
            footprints.contains("\"origin\": \"compiler_body_place_binary_write\"")
                && footprints.contains(expected_register)
                && footprints.contains("\"enumeration_complete\": false"),
            "{target} artifact must retain the machine-double-indexed binary-write footprint without claiming completeness"
        );
        let _ = fs::remove_dir_all(&scratch);
    }
}

#[test]
fn compiler_body_general_x86_binary_write_footprints_reach_artifacts() {
    {
        let (case_name, source_text) = (
            "frame-indexed-by-region",
            r#"use omega::language::std::console;
data Counter { n: i32 in Wrapping; }
data Room { exits: [Counter; 3]; }
data Main { console: Console; index: u64 [0..=2]; }
machine Main::main(&mut self) {
    self.index = 1;
    let room: Room = Room { exits: [Counter { n: 10 }, Counter { n: 20 }, Counter { n: 30 }] };
    let exits: &mut [Counter] = room.exits.as_mut_slice();
    exits[self.index].n = exits[self.index].n + 1;
    transition room.exits[1].n == 21 { true -> good() _ -> bad() }
    state good(&mut self) { self.console.exit_process(70); }
    state bad(&mut self) { self.console.exit_process(71); }
}

"#,
        );
        let scratch = std::env::temp_dir().join(format!(
            "omega-compiler-body-general-x86-binary-write-footprint-{case_name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        let source = scratch.join("src");
        let output = scratch.join("out");
        fs::create_dir_all(&source)
            .expect("create compiler-body general x86 binary-write source directory");
        fs::write(source.join("main.omg"), source_text)
            .expect("write compiler-body general x86 binary-write source");
        fs::write(
            source.join("build.omg"),
            hosted_main_program_entry_build("linux_x86_64"),
        )
        .expect("write exact compiler-body general x86 binary-write target entry");
        production_compile(CanaryCompileSpec {
            root_path: source.join("main.omg"),
            build_dir: Some(output.clone()),
            target_name: Some("linux_x86_64".into()),
            product: CanaryCompileProduct::NativeArtifactAndPublish,
        })
        .unwrap_or_else(|diagnostics| {
            panic!(
                "compiler-body general x86 binary writes in {case_name} should compile: {diagnostics:?}"
            )
        });
        let footprints = fs::read_to_string(output.join("08_boundary_footprints.json"))
            .expect("compiler-body general x86 binary-write footprint evidence should be written");
        assert!(
            footprints.contains("\"origin\": \"compiler_body_place_binary_write\"")
                && footprints.contains("\"X86R14\"")
                && footprints.contains("\"enumeration_complete\": false"),
            "linux_x64 artifact must retain the general binary-write footprint for {case_name} without claiming completeness"
        );
        let _ = fs::remove_dir_all(&scratch);
    }
}

#[test]
fn compiler_body_bounded_buffer_source_append_footprints_reach_artifacts() {
    let canary = pass_canary("text/runtime_bounded_carrier_local_source_concat_exit");
    for (target, expected_register) in [
        ("linux_x86_64", "\"X86Rsi\""),
        ("linux_arm64", "\"Aarch64X(12)\""),
    ] {
        let scratch = std::env::temp_dir().join(format!(
            "omega-compiler-body-bounded-buffer-source-append-footprint-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        let source = scratch.join("src");
        let output = scratch.join("out");
        fs::create_dir_all(&source)
            .expect("create compiler-body bounded-buffer source-append source directory");
        fs::copy(canary.join("main.omg"), source.join("main.omg"))
            .expect("copy compiler-body bounded-buffer source-append canary");
        fs::write(
            source.join("build.omg"),
            hosted_main_program_entry_build(target),
        )
        .expect("write compiler-body bounded-buffer source-append target");
        compile_with_auxiliary_artifacts(CanaryCompileSpec {
            root_path: source.join("main.omg"),
            build_dir: Some(output.clone()),
            target_name: Some(target.into()),
            product: CanaryCompileProduct::NativeArtifactAndPublish,
        })
        .unwrap_or_else(|diagnostics| {
            panic!(
                "compiler-body bounded-buffer source appends should compile for {target}: {diagnostics:?}"
            )
        });
        let footprints = fs::read_to_string(output.join("08_boundary_footprints.json"))
            .expect("compiler-body bounded-buffer source-append evidence should be written");
        assert!(
            footprints.contains("\"origin\": \"compiler_body_place_bounded_buffer_write\"")
                && footprints.contains(expected_register)
                && footprints.contains("\"enumeration_complete\": false"),
            "{target} artifact must retain the bounded-buffer source-append footprint without claiming completeness"
        );
        let _ = fs::remove_dir_all(&scratch);
    }
}

#[test]
fn compiler_body_text_buffer_materialize_footprints_reach_artifacts() {
    let canary = pass_canary("text/runtime_string_append_in_place_exit");
    for (target, expected_register) in [
        ("linux_x86_64", "\"X86Rsi\""),
        ("linux_arm64", "\"Aarch64X(21)\""),
    ] {
        let scratch = std::env::temp_dir().join(format!(
            "omega-compiler-body-text-buffer-materialize-footprint-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        let source = scratch.join("src");
        let output = scratch.join("out");
        fs::create_dir_all(&source)
            .expect("create compiler-body text-buffer materialize source directory");
        fs::copy(canary.join("main.omg"), source.join("main.omg"))
            .expect("copy compiler-body text-buffer materialize canary");
        fs::write(
            source.join("build.omg"),
            hosted_main_program_entry_build(target),
        )
        .expect("write compiler-body text-buffer materialize target");
        compile_with_auxiliary_artifacts(CanaryCompileSpec {
            root_path: source.join("main.omg"),
            build_dir: Some(output.clone()),
            target_name: Some(target.into()),
            product: CanaryCompileProduct::NativeArtifactAndPublish,
        })
        .unwrap_or_else(|diagnostics| {
            panic!(
                "compiler-body text-buffer materialization should compile for {target}: {diagnostics:?}"
            )
        });
        let footprints = fs::read_to_string(output.join("08_boundary_footprints.json"))
            .expect("compiler-body text-buffer materialize evidence should be written");
        assert!(
            footprints.contains("\"origin\": \"compiler_body_text_assembly_write\"")
                && footprints.contains(expected_register)
                && footprints.contains("\"enumeration_complete\": false"),
            "{target} artifact must retain text-buffer materialization evidence without claiming completeness"
        );
        let _ = fs::remove_dir_all(&scratch);
    }
}

#[test]
fn compiler_body_frame_base_indexed_text_assembly_footprints_reach_aarch64_artifact() {
    let canary = pass_canary("text/runtime_local_array_indexed_string_field_concat_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-compiler-body-frame-base-indexed-text-assembly-footprint-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let output = scratch.join("out");
    compile_rooted_canary_for_target_with_auxiliary_artifacts(
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
    let canary = pass_canary("text/runtime_local_struct_string_field_concat_exit");
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
            hosted_main_program_entry_build(target),
        )
        .expect("write compiler-body text literal-append target");
        compile_with_auxiliary_artifacts(CanaryCompileSpec {
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
    let canary = pass_canary("text/runtime_slice_alias_indexed_string_field_concat_exit");
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
        compile_rooted_canary_for_target_with_auxiliary_artifacts(&canary, output.clone(), target)
            .unwrap_or_else(|diagnostics| {
                panic!(
                    "compiler-body stored-text appends should compile for {target}: {diagnostics:?}"
                )
            });
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
    let canary = pass_canary("text/runtime_string_stored_suffix_exit");
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
            hosted_main_program_entry_build(target),
        )
        .expect("write compiler-body stored-text suffix target");
        compile_with_auxiliary_artifacts(CanaryCompileSpec {
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
    let canary = pass_canary("recast/runtime_record_view_exit");
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
            hosted_main_program_entry_build(target),
        )
        .expect("write compiler-body place-address target");
        compile_with_auxiliary_artifacts(CanaryCompileSpec {
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
    let canary = pass_canary("recast/runtime_record_view_exit");
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
    let canary =
        pass_canary("text/runtime_machine_owned_double_indexed_bounded_carrier_literal_exit");
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
        compile_rooted_canary_for_target_with_auxiliary_artifacts(
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
    let canary = pass_canary("text/runtime_machine_owned_double_indexed_string_field_concat_exit");
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
        compile_rooted_canary_for_target_with_auxiliary_artifacts(&canary, output.clone(), target)
            .unwrap_or_else(|diagnostics| {
                panic!("compiler-body string writes should compile for {target}: {diagnostics:?}")
            });
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
    let canary = pass_canary("text/runtime_x86_general_double_indexed_string_concat_compile");
    let scratch = std::env::temp_dir().join(format!(
        "omega-general-x86-text-assembly-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let output = scratch.join("out");
    compile_with_auxiliary_artifacts(CanaryCompileSpec {
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
    let canary = pass_canary("wire/runtime_wire_encode_primitive_exit");
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
            hosted_main_program_entry_build(target),
        )
        .expect("write compiler-body wire scalar-append target");

        compile_with_auxiliary_artifacts(CanaryCompileSpec {
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
    let canary = pass_canary("wire/runtime_wire_encode_string_exit");
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
            hosted_main_program_entry_build(target),
        )
        .expect("write compiler-body wire text-append target");

        compile_with_auxiliary_artifacts(CanaryCompileSpec {
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
    let canary = pass_canary("wire/runtime_wire_encode_borrowed_scalar_slice_exit");
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
            hosted_main_program_entry_build(target),
        )
        .expect("write compiler-body wire scalar-slice target");

        compile_with_auxiliary_artifacts(CanaryCompileSpec {
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
    let canary = pass_canary("wire/runtime_wire_encode_repeated_then_string_exit");
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
            hosted_main_program_entry_build(target),
        )
        .expect("write compiler-body wire repeated-scalar target");

        compile_with_auxiliary_artifacts(CanaryCompileSpec {
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
    let canary = pass_canary("wire/runtime_wire_decode_byte_slice_exit");
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
            hosted_main_program_entry_build(target),
        )
        .expect("write compiler-body wire byte-slice target");

        compile_with_auxiliary_artifacts(CanaryCompileSpec {
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
    let canary = pass_canary("wire/runtime_wire_roundtrip_nested_and_repeated_exit");
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
            hosted_main_program_entry_build(target),
        )
        .expect("write compiler-body wire nested-bounds target");

        compile_with_auxiliary_artifacts(CanaryCompileSpec {
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
    let canary = pass_canary("wire/runtime_wire_roundtrip_nested_and_repeated_exit");
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
            hosted_main_program_entry_build(target),
        )
        .expect("write compiler-body wire repeated-scalar-read target");

        compile_with_auxiliary_artifacts(CanaryCompileSpec {
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
    let canary = pass_canary("wire/runtime_wire_decode_let_compare_exit");
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
            hosted_main_program_entry_build(target),
        )
        .expect("write compiler-body wire expected-byte target");

        compile_with_auxiliary_artifacts(CanaryCompileSpec {
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
    let canary = pass_canary("wire/runtime_wire_decode_ranged_field_exit");
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
            hosted_main_program_entry_build(target),
        )
        .expect("write compiler-body wire ranged-scalar target");

        compile_with_auxiliary_artifacts(CanaryCompileSpec {
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
    let canary = pass_canary("slices/runtime_aarch64_cross_region_frame_indexed_rmw_compile");
    let scratch = std::env::temp_dir().join(format!(
        "omega-aarch64-cross-region-frame-indexed-rmw-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let output = scratch.join("out");
    compile_with_auxiliary_artifacts(CanaryCompileSpec {
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
    let canary =
        pass_canary("text/runtime_machine_owned_double_indexed_bounded_carrier_literal_exit");
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
        compile_rooted_canary_for_target_with_auxiliary_artifacts(
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
    let canary = pass_canary("layouts/runtime_plan_laid_compact_bits_exit");
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
            hosted_main_program_entry_build(target),
        )
        .expect("write compiler-body storage-bit-field-write target");
        compile_with_auxiliary_artifacts(CanaryCompileSpec {
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
    let canary = pass_canary("control_flow/runtime_entry_cast_result_exit");
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
            hosted_main_program_entry_build(target),
        )
        .expect("write compiler-body storage-convert-write target");
        compile_with_auxiliary_artifacts(CanaryCompileSpec {
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
    let canary = pass_canary("text/runtime_number_to_decimal_exit");
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
            hosted_main_program_entry_build(target),
        )
        .expect("write compiler-body machine-indexed convert-write target");
        compile_with_auxiliary_artifacts(CanaryCompileSpec {
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
    let canary = pass_canary("borrow/runtime_view_of_view_chain_exit");
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
            hosted_main_program_entry_build(target),
        )
        .expect("write compiler-body pointee integer-write target");
        compile_with_auxiliary_artifacts(CanaryCompileSpec {
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

machine Main::main(&mut self) {
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
        compile_with_auxiliary_artifacts(CanaryCompileSpec {
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

machine Main::main(&mut self) {
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
        compile_with_auxiliary_artifacts(CanaryCompileSpec {
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
    let canary = pass_canary("storage/runtime_machine_owned_indexed_integer_write_exit");
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
        compile_rooted_canary_for_target_with_auxiliary_artifacts(
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

machine Main::main(&mut self) {
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
        compile_with_auxiliary_artifacts(CanaryCompileSpec {
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
    let canary = pass_canary("text/runtime_string_field_literal_guard_exit");
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
        compile_rooted_canary_for_target_with_auxiliary_artifacts(&canary, output.clone(), target)
            .unwrap_or_else(|diagnostics| {
                panic!("runtime-value guard should compile for {target}: {diagnostics:?}")
            });
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

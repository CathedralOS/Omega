use super::assert_native_exit_code;
use super::fixture_roster;
use crate::{
    CanaryCompileProduct, CanaryCompileSpec, compile, compile_rooted_canary_for_native_host,
    compile_rooted_canary_for_target, compile_single_file_hosted_main, fs,
    hosted_main_program_entry_build, hosted_main_program_entry_build_for, native_hosted_target,
    pass_canary, production_compile,
};

#[test]
fn static_guard_footprints_reach_x86_and_aarch64_artifacts() {
    let canary = pass_canary(fixture_roster::RUNTIME_INTEGER_LITERAL_DISPATCH_EXIT);
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
    let canary = pass_canary(fixture_roster::RUNTIME_LOCAL_STRUCT_STRING_FIELD_CONCAT_EXIT);
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
            hosted_main_program_entry_build_for(&canary, target),
        )
        .expect("write runtime-text guard target");

        compile(CanaryCompileSpec {
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
    let canary = pass_canary(fixture_roster::TERMINATION_INDEX_DISTANCE_COMPILE);
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
            hosted_main_program_entry_build_for(&canary, target),
        )
        .expect("write place-guard target");

        compile(CanaryCompileSpec {
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
    let canary = pass_canary(fixture_roster::RUNTIME_VALUE_CALL_THROUGH_ALIAS_IN_DISPATCH_EXIT);
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
        compile_rooted_canary_for_target(&canary, output.clone(), target).unwrap_or_else(
            |diagnostics| {
                panic!("compiler-body place copy should compile for {target}: {diagnostics:?}")
            },
        );
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
    let canary = pass_canary(fixture_roster::RUNTIME_SHARED_REF_PARAM_COPY_EXIT);
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
            hosted_main_program_entry_build_for(&canary, target),
        )
        .expect("write compiler-body from-pointee target");

        compile(CanaryCompileSpec {
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
    let canary = pass_canary(fixture_roster::RUNTIME_SHARED_REF_PARAM_COPY_EXIT);
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
    let canary = pass_canary(fixture_roster::RUNTIME_POINTEE_PAIR_COPY_EXIT);
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
            hosted_main_program_entry_build_for(&canary, target),
        )
        .expect("write compiler-body pointee-pair target");

        compile(CanaryCompileSpec {
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
    let canary = pass_canary(fixture_roster::RUNTIME_POINTEE_PAIR_COPY_EXIT);
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
    let canary = pass_canary(fixture_roster::RUNTIME_SLICE_ELEMENT_RUNTIME_INDEX_READ_EXIT);
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
            hosted_main_program_entry_build_for(&canary, target),
        )
        .expect("write compiler-body from-indexed target");

        compile(CanaryCompileSpec {
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
    let canary = pass_canary(fixture_roster::RUNTIME_FIXED_VEC_ROUND_TRIP_EXIT);
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
            hosted_main_program_entry_build_for(&canary, target),
        )
        .expect("write compiler-body to-indexed target");

        compile(CanaryCompileSpec {
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
    let canary = pass_canary(fixture_roster::RUNTIME_ALIAS_INDEXED_READ_THROUGH_TRANSITION_EXIT);
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
        compile_rooted_canary_for_target(
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
    let canary = pass_canary(fixture_roster::RUNTIME_FRAME_INDEXED_LOCAL_READ_EXIT);
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
            hosted_main_program_entry_build_for(&canary, target),
        )
        .expect("write compiler-body frame-base-indexed target");

        compile(CanaryCompileSpec {
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
    let canary = pass_canary(fixture_roster::RUNTIME_MACHINE_INDEXED_STRUCT_FIELD_ARG_EXIT);
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
            hosted_main_program_entry_build_for(&canary, target),
        )
        .expect("write compiler-body machine-indexed target");

        compile(CanaryCompileSpec {
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
    let canary = pass_canary(fixture_roster::RUNTIME_MACHINE_FRAME_INDEX_WRITE_EXIT);
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
            hosted_main_program_entry_build_for(&canary, target),
        )
        .expect("write compiler-body to-machine-indexed target");

        compile(CanaryCompileSpec {
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
    let canary = pass_canary(fixture_roster::RUNTIME_FRAME_DOUBLE_INDEXED_READ_EXIT);
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
            hosted_main_program_entry_build_for(&canary, target),
        )
        .expect("write compiler-body frame-double-indexed target");
        compile(CanaryCompileSpec {
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
    let canary = pass_canary(fixture_roster::RUNTIME_DOUBLE_INDEXED_READ_EXIT);
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
            hosted_main_program_entry_build_for(&canary, target),
        )
        .expect("write compiler-body machine-double-indexed target");
        compile(CanaryCompileSpec {
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
    let canary = pass_canary(fixture_roster::RUNTIME_DOUBLE_INDEXED_WRITE_EXIT);
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
            hosted_main_program_entry_build_for(&canary, target),
        )
        .expect("write compiler-body to-machine-double-indexed target");
        compile(CanaryCompileSpec {
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
    let canary = pass_canary(fixture_roster::RUNTIME_DUAL_INDEXED_COPY_EXIT);
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
            hosted_main_program_entry_build_for(&canary, target),
        )
        .expect("write compiler-body machine-indexed-pair target");
        compile(CanaryCompileSpec {
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
    let canary = pass_canary(fixture_roster::RUNTIME_FRAME_MIXED_INDEX_PAIR_COPY_EXIT);
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
            hosted_main_program_entry_build_for(&canary, target),
        )
        .expect("write compiler-body mixed-index frame-pair target");
        compile(CanaryCompileSpec {
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
    let canary = pass_canary(fixture_roster::RUNTIME_CROSS_REGION_INDEXED_PAIR_COPY_EXIT);
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
            hosted_main_program_entry_build_for(&canary, target),
        )
        .expect("write compiler-body cross-region indexed-pair target");
        compile(CanaryCompileSpec {
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
    let canary = pass_canary(fixture_roster::RUNTIME_CROSS_REGION_DOUBLE_INDEXED_PAIR_COPY_EXIT);
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
            hosted_main_program_entry_build_for(&canary, target),
        )
        .expect("write compiler-body cross-region double-indexed-pair target");
        compile(CanaryCompileSpec {
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
    let canary = pass_canary(fixture_roster::RUNTIME_DUAL_INDEXED_COPY_EXIT);
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
            hosted_main_program_entry_build_for(&canary, target),
        )
        .expect("write compiler-body direct integer-write target");
        compile(CanaryCompileSpec {
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
    let canary = pass_canary(fixture_roster::F32_DEEP_CHAIN_BINARY);
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
            hosted_main_program_entry_build_for(&canary, target),
        )
        .expect("write compiler-body direct binary-write target");
        compile(CanaryCompileSpec {
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
    let canary = pass_canary(fixture_roster::RUNTIME_STATEMENT_CALL_SINGLE_EXECUTION_EXIT);
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
        compile_rooted_canary_for_target(&canary, output.clone(), target)
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
    let canary = pass_canary(fixture_roster::RUNTIME_SLICE_INDEXED_BINARY_RMW_EXIT);
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
            hosted_main_program_entry_build_for(&canary, target),
        )
        .expect("write compiler-body frame-indexed binary-write target");
        compile(CanaryCompileSpec {
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
    let canary = pass_canary(fixture_roster::RUNTIME_DISPATCH_LOCAL_INDEX_BINARY_WRITE_EXIT);
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
        compile_rooted_canary_for_target(
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
    let canary = pass_canary(fixture_roster::RUNTIME_INDEXED_RMW_LOOP_EXIT);
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
        compile_rooted_canary_for_target(
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
    let canary = pass_canary(fixture_roster::RUNTIME_DOUBLE_INDEXED_RMW_EXIT);
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
            hosted_main_program_entry_build_for(&canary, target),
        )
        .expect("write compiler-body machine-double-indexed binary-write target");
        compile(CanaryCompileSpec {
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
use omega::language::core::binding;
data Counter { n: i32 in Wrapping; }
data Room { exits: [Counter; 3]; }
data Main { console: Binding<Console>; index: u64 [0..=2]; }
machine Main::main(&mut self) reaches Console {
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
    let canary = pass_canary(fixture_roster::RUNTIME_BOUNDED_CARRIER_LOCAL_SOURCE_CONCAT_EXIT);
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
            hosted_main_program_entry_build_for(&canary, target),
        )
        .expect("write compiler-body bounded-buffer source-append target");
        compile(CanaryCompileSpec {
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
    let canary = pass_canary(fixture_roster::RUNTIME_STRING_APPEND_IN_PLACE_EXIT);
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
            hosted_main_program_entry_build_for(&canary, target),
        )
        .expect("write compiler-body text-buffer materialize target");
        compile(CanaryCompileSpec {
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

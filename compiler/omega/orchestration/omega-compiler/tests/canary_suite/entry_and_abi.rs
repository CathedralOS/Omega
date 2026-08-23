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
fn explicit_program_entry_binding_owns_capability_manifest_identity() {
    let canary = pass_canary("build/explicit_program_entry_binding");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-explicit-entry-manifest-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile_with_auxiliary_artifacts(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("windows_x64".into()),
        write_output: true,
    })
    .expect("explicit entry canary should emit audit artifacts");
    let manifest = fs::read_to_string(build_dir.join("05_capability_manifest.json"))
        .expect("capability manifest should be written");

    assert!(
        manifest.contains("\"entry_machine\": \"launch\"")
            && manifest.contains("\"entry_state\": \"entry\""),
        "capability manifest must consume the exact Build-selected entry\n{manifest}"
    );
    let _ = fs::remove_dir_all(build_dir);
}

#[test]
fn checked_compilation_retains_the_exact_selected_program_entry() {
    let canary = pass_canary("build/explicit_program_entry_binding");
    let checked = compile_to_checked(&canary.join("main.omg"), Some("windows_x64"))
        .expect("explicit entry canary should reach checked semantics");

    assert_eq!(checked.selected_program_entry_machine(), Some("launch"));
    let outcome = psi_checked_interpreter::interpret_entry(
        &checked,
        checked
            .selected_program_entry_machine()
            .expect("target build selected an exact entry"),
        &[],
    );
    assert_eq!(outcome.error, None);
}

#[test]
fn checked_compilation_does_not_infer_an_entry_for_legacy_semantic_corpus() {
    let canary = pass_canary("arithmetic/runtime_chained_field_mutation_exit");
    let checked = compile_to_checked(&canary.join("main.omg"), None)
        .expect("legacy Main entry canary should reach checked semantics");

    assert_eq!(checked.selected_program_entry_machine(), None);
    let outcome = psi_checked_interpreter::interpret_entry(&checked, "Main::main", &[]);
    assert_eq!(outcome.error, None);
    assert_eq!(outcome.exit_code, 70);
}

#[test]
fn production_compile_rejects_an_unrooted_legacy_entry() {
    let canary = pass_canary("arithmetic/runtime_chained_field_mutation_exit");
    let build_dir = unique_no_output_build_dir();
    let diagnostics = production_compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect_err("production compilation must not discover `Main::main` by name");
    let _ = fs::remove_dir_all(build_dir);

    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("no runtime entry point was selected")),
        "missing ProgramEntry selection should fail explicitly: {diagnostics:#?}"
    );
}

#[test]
fn production_check_accepts_entry_agnostic_semantic_corpus() {
    let canary = pass_canary("arithmetic/runtime_chained_field_mutation_exit");
    let build_dir = unique_no_output_build_dir();
    let report = production_compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: false,
    })
    .expect("check-only compilation must not require or infer a runtime entry");

    assert!(!report.wrote_output());
    assert!(report.program_storage_entry().is_none());
    assert!(build_dir.join("04_typed_trees.json").is_file());
    assert!(build_dir.join("05_machine_contracts.json").is_file());
    let _ = fs::remove_dir_all(build_dir);
}

#[test]
fn migrated_main_entries_are_selected_only_through_their_target_root_bindings() {
    for (canary_name, target) in [
        (
            "capabilities/win64_scalar_float_import_compile",
            "windows_x64",
        ),
        (
            "capabilities/win64_large_aggregate_import_compile",
            "windows_x64",
        ),
        (
            "capabilities/win64_direct_aggregate_import_compile",
            "windows_x64",
        ),
        (
            "capabilities/win64_direct_aggregate_result_import_compile",
            "windows_x64",
        ),
        (
            "capabilities/win64_large_aggregate_result_import_compile",
            "windows_x64",
        ),
        (
            "capabilities/sysv_small_aggregate_import_compile",
            "linux_x64",
        ),
        (
            "build/static_machine_parameter_config_compile",
            "windows_x64",
        ),
        ("inline_asm/asm_port_out_final_validation", "linux_x64"),
        (
            "inline_asm/asm_runtime_port_msr_final_validation",
            "linux_x64",
        ),
        (
            "text/runtime_x86_general_double_indexed_string_concat_compile",
            "linux_x64",
        ),
        (
            "slices/runtime_aarch64_cross_region_frame_indexed_rmw_compile",
            "linux_arm64",
        ),
    ] {
        let canary = pass_canary(canary_name);
        let checked = compile_to_checked(&canary.join("main.omg"), Some(target))
            .unwrap_or_else(|diagnostics| {
                panic!(
                    "{canary_name} should retain its explicit {target} ProgramEntry binding: {diagnostics:?}"
                )
            });

        assert_eq!(
            checked.selected_program_entry_machine(),
            Some("Main::main"),
            "{canary_name} must select Main::main through its target-owned ProgramEntry slot"
        );
    }
}

#[test]
fn uefi_program_entry_retains_exact_storage_root_binding() {
    let canary = pass_canary("build/uefi_program_entry_storage_roots");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-uefi-program-storage-entry-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    let report = compile_with_auxiliary_artifacts(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("uefi_x64".into()),
        write_output: true,
    })
    .expect("UEFI program-storage entry should bind its generated captures");
    let binding = report
        .program_storage_entry()
        .expect("UEFI entry should retain a program-storage binding");
    let bridge = report
        .program_storage_entry_bridge()
        .expect("UEFI entry should retain its emitted native-bridge handoff");

    assert!(
        binding
            .requirement_identity()
            .contains("ProgramStorageEntry")
    );
    assert_eq!(binding.image().parameter_index(), 0);
    assert_eq!(binding.initial_storage().parameter_index(), 1);
    assert_ne!(binding.boundary_contract_fingerprint(), 0);
    let receiver = binding
        .receiver()
        .expect("receiver-bound entry should retain its checked storage demand");
    assert!(receiver.type_identity().contains("Boot"));
    assert_eq!(receiver.byte_size(), 8);
    assert_eq!(receiver.byte_alignment(), 8);
    assert_eq!(bridge.binding(), binding);
    assert_eq!(bridge.target_profile(), "uefi_x64");
    assert!(!bridge.entry_symbol().is_empty());
    assert!(bridge.entry_text_size() > 0);
    assert!(bridge.continuation_key().is_valid());
    assert_eq!(bridge.continuation_machine(), "Boot::launch");
    assert!(!bridge.continuation_state().is_empty());
    let source_signature = bridge
        .source_signature()
        .expect("production bridge must retain its checked typed source signature");
    assert_eq!(
        source_signature.machine_symbol(),
        bridge.continuation_key().machine
    );
    assert_eq!(
        source_signature.state_symbol(),
        bridge.continuation_key().state
    );
    assert_eq!(
        source_signature.machine_name(),
        bridge.continuation_machine()
    );
    assert_eq!(source_signature.state_name(), bridge.continuation_state());
    assert_eq!(
        source_signature.result(),
        omega_compiler::ProgramEntrySourceResultSignature::Unit
    );
    assert!(!source_signature.normalized_callable_identity().is_empty());
    assert_eq!(
        source_signature.receiver().normalized_type_identity(),
        Some(receiver.type_identity())
    );
    let [image, initial_storage] = source_signature.visible_parameters() else {
        panic!("UEFI source signature must retain two visible declaration rows")
    };
    assert_eq!(image.visible_parameter_index(), 0);
    assert_eq!(
        image.role(),
        omega_compiler::ProgramStorageEntryRootRole::Image
    );
    assert_eq!(
        image.normalized_type_identity(),
        binding.image().parameter_type_identity()
    );
    assert_eq!(initial_storage.visible_parameter_index(), 1);
    assert_eq!(
        initial_storage.role(),
        omega_compiler::ProgramStorageEntryRootRole::InitialStorage
    );
    assert_eq!(
        initial_storage.normalized_type_identity(),
        binding.initial_storage().parameter_type_identity()
    );
    let continuation_abi = bridge
        .continuation_abi()
        .expect("production bridge must retain its complete outbound ABI");
    assert_eq!(
        continuation_abi.normalized_callable_identity(),
        source_signature.normalized_callable_identity()
    );
    assert_eq!(
        continuation_abi.call().policy,
        omega_calling_conventions::CallingPolicy::MicrosoftX64
    );
    assert!(continuation_abi.call().result.is_none());
    assert_eq!(continuation_abi.call().parameters.len(), 3);
    assert!(matches!(
        continuation_abi.receiver(),
        omega_compiler::ProgramStorageEntryContinuationReceiverAbiPlan::BorrowedActivationLoan {
            parameter_index: 0,
            ..
        }
    ));
    let [image_abi, initial_storage_abi] = continuation_abi.visible_arguments() else {
        panic!("UEFI continuation ABI must place both visible root declarations")
    };
    assert_eq!(image_abi.call_parameter_index(), 1);
    assert_eq!(initial_storage_abi.call_parameter_index(), 2);
    assert_eq!(image_abi.shape(), image.value_shape());
    assert_eq!(initial_storage_abi.shape(), initial_storage.value_shape());
    assert_eq!(
        image_abi.placement(),
        &continuation_abi.call().parameters[1]
    );
    assert_eq!(
        initial_storage_abi.placement(),
        &continuation_abi.call().parameters[2]
    );
    assert!(
        bridge.selected_provider().is_none(),
        "the current UEFI profile has no physical provider selection to claim as installed"
    );

    let manifest = fs::read_to_string(build_dir.join("10_program_storage_entry.json"))
        .expect("program-storage entry manifest should be emitted");
    assert!(manifest.contains("\"role\": \"image\""));
    assert!(manifest.contains("\"role\": \"initial_storage\""));
    assert!(manifest.contains("\"domain\": \"Extent::Granted\""));
    assert!(manifest.contains("\"copy_stack_byte_offset\": 32"));
    assert!(manifest.contains("\"copy_stack_byte_offset\": 48"));
    assert!(manifest.contains("\"status\": \"reservation_required\""));
    assert!(manifest.contains("\"byte_size\": 8"));
    assert!(manifest.contains("\"native_bridge\""));
    assert!(manifest.contains("\"status\": \"pending_runtime_installation\""));
    assert!(manifest.contains("\"target_profile\": \"uefi_x64\""));
    assert!(manifest.contains("\"selected_physical_provider_plan\": null"));
    assert!(manifest.contains("\"status\": \"required\""));
    assert!(
        manifest
            .contains("validate_geometry_and_receiver_reservation_before_consuming_either_grant")
    );

    fs::write(
        build_dir.join(PROGRAM_STORAGE_INSTALLATION_ARTIFACT),
        "stale completed installation",
    )
    .expect("seed stale completed-installation artifact");

    let hosted = pass_canary("build/explicit_program_entry_binding");
    compile_with_auxiliary_artifacts(CompileOptions {
        root_path: hosted.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("windows_x64".into()),
        write_output: true,
    })
    .expect("hosted entry should compile into the reused artifact directory");
    assert!(
        !build_dir.join("10_program_storage_entry.json").exists(),
        "a hosted build must remove a stale program-storage entry artifact"
    );
    assert!(
        !build_dir
            .join(PROGRAM_STORAGE_INSTALLATION_ARTIFACT)
            .exists(),
        "a new compilation must remove a stale completed-installation claim"
    );
    let _ = fs::remove_dir_all(build_dir);
}

#[test]
fn catalog_checked_assembly_is_validated_against_final_image_bytes() {
    let canary = pass_canary("inline_asm/asm_fences_compile");
    let build_dir =
        std::env::temp_dir().join(format!("omega-final-asm-evidence-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile_with_auxiliary_artifacts(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("linux_x64".into()),
        write_output: true,
    })
    .expect("fixed checked assembly should cross-compile with final-byte evidence");

    let executable_regions = fs::read_to_string(build_dir.join("13_executable_regions.json"))
        .expect("final executable-region inventory should be written");
    assert!(
        executable_regions.contains("\"checked_instruction_validation_count\": 3")
            && executable_regions.contains("\"checked_instruction_validation_fingerprint\": \"0x")
            && executable_regions.contains("\"checked_instruction_footprint_fingerprint\": \"0x")
            && executable_regions.contains("\"catalog_checked_assembly\"")
            && executable_regions.contains("\"enumeration_complete\": true")
            && executable_regions.contains("\"missing_classes\": []"),
        "final image evidence should cover all three fixed fence instructions with complete body validation:\n{executable_regions}"
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn immediate_port_io_is_bound_in_final_image_validation() {
    let canary = pass_canary("inline_asm/asm_port_out_final_validation");
    let build_dir =
        std::env::temp_dir().join(format!("omega-final-port-evidence-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile_with_auxiliary_artifacts(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("linux_x64".into()),
        write_output: true,
    })
    .expect("immediate-port checked assembly should emit final-byte evidence");

    let executable_regions = fs::read_to_string(build_dir.join("13_executable_regions.json"))
        .expect("final executable-region inventory should be written");
    assert!(
        executable_regions.contains("\"checked_instruction_validation_count\": 1")
            && executable_regions.contains("\"checked_instruction_validation_fingerprint\": \"0x")
            && executable_regions.contains("\"checked_instruction_footprint_fingerprint\": \"0x"),
        "final image evidence should bind the immediate port instruction:\n{executable_regions}"
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn structured_machine_control_envelopes_are_bound_in_final_image_validation() {
    for (canary_name, expected_count) in [
        ("inline_asm/asm_msr_compile", 2),
        ("inline_asm/asm_control_registers_compile", 7),
        ("inline_asm/asm_flags_compile", 3),
        ("inline_asm/asm_runtime_port_msr_final_validation", 4),
    ] {
        let canary = pass_canary(canary_name);
        let build_dir = std::env::temp_dir().join(format!(
            "omega-final-machine-control-evidence-{}-{}",
            std::process::id(),
            expected_count
        ));
        let _ = fs::remove_dir_all(&build_dir);

        compile_with_auxiliary_artifacts(CompileOptions {
            root_path: canary.join("main.omg"),
            build_dir: Some(build_dir.clone()),
            target_name: Some("linux_x64".into()),
            write_output: true,
        })
        .expect("structured machine-control assembly should emit final-byte evidence");

        let executable_regions = fs::read_to_string(build_dir.join("13_executable_regions.json"))
            .expect("final executable-region inventory should be written");
        assert!(
            executable_regions.contains(&format!(
                "\"checked_instruction_validation_count\": {expected_count}"
            )),
            "{canary_name} should publish evidence for every structured machine-control instruction:\n{executable_regions}"
        );

        let _ = fs::remove_dir_all(&build_dir);
    }
}

#[test]
fn aarch64_hfa_entry_argument_spreads_vector_registers() {
    let canary = pass_canary("targets/aarch64_hfa_entry_argument");
    let build_dir =
        std::env::temp_dir().join(format!("omega-aarch64-hfa-entry-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("linux_arm64".into()),
        write_output: true,
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
    let canary = pass_canary("targets/aarch64_small_aggregate_entry");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-aarch64-small-aggregate-entry-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("linux_arm64".into()),
        write_output: true,
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
    let canary = pass_canary("targets/aarch64_small_aggregate_stack_entry");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-aarch64-small-aggregate-stack-entry-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("linux_arm64".into()),
        write_output: true,
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
    let canary = pass_canary("targets/aarch64_large_aggregate_entry");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-aarch64-large-aggregate-entry-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("linux_arm64".into()),
        write_output: true,
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
    let canary = pass_canary("targets/aarch64_large_aggregate_stack_entry");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-aarch64-large-aggregate-stack-entry-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("linux_arm64".into()),
        write_output: true,
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
    let canary = pass_canary("targets/aarch64_wide_aggregate_entry");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-aarch64-wide-aggregate-entry-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("linux_arm64".into()),
        write_output: true,
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
    let canary = pass_canary("targets/aarch64_small_result_entry");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-aarch64-small-result-entry-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("linux_arm64".into()),
        write_output: true,
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
    let canary = pass_canary("targets/aggregate_literal_result_entry");
    for target in ["linux_x64", "linux_arm64", "uefi_x64"] {
        let scratch = std::env::temp_dir().join(format!(
            "omega-{target}-aggregate-literal-result-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        let src_dir = scratch.join("src");
        let out_dir = scratch.join("out");
        fs::create_dir_all(&src_dir).expect("scratch source directory");
        fs::copy(canary.join("main.omg"), src_dir.join("main.omg")).expect("copy canary");
        fs::write(
            src_dir.join("build.omg"),
            format!("target {target} {{\n}}\n"),
        )
        .expect("write target manifest");

        compile(CompileOptions {
            root_path: src_dir.join("main.omg"),
            build_dir: Some(out_dir.clone()),
            target_name: Some(target.into()),
            write_output: true,
        })
        .expect("aggregate-literal entry result should cross-compile");

        let output_name = if target == "uefi_x64" {
            "omega-program.exe"
        } else {
            "omega-program"
        };
        let image = fs::read(out_dir.join(output_name)).expect("read emitted image");
        if target == "linux_x64" {
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
    let canary = pass_canary("targets/indexed_scalar_result_entry");
    for target in ["linux_x64", "linux_arm64"] {
        let scratch = std::env::temp_dir().join(format!(
            "omega-{target}-indexed-scalar-result-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        let src_dir = scratch.join("src");
        let out_dir = scratch.join("out");
        fs::create_dir_all(&src_dir).expect("scratch source directory");
        fs::copy(canary.join("main.omg"), src_dir.join("main.omg")).expect("copy canary");
        fs::write(
            src_dir.join("build.omg"),
            format!("target {target} {{\n}}\n"),
        )
        .expect("write target manifest");

        compile(CompileOptions {
            root_path: src_dir.join("main.omg"),
            build_dir: Some(out_dir.clone()),
            target_name: Some(target.into()),
            write_output: true,
        })
        .expect("indexed scalar entry result should cross-compile");

        let image = fs::read(out_dir.join("omega-program")).expect("read emitted ELF");
        if target == "linux_x64" {
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
    let canary = pass_canary("targets/aarch64_hfa_result_entry");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-aarch64-hfa-result-entry-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("linux_arm64".into()),
        write_output: true,
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
    let canary = pass_canary("targets/aarch64_large_result_entry");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-aarch64-large-result-entry-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile_with_auxiliary_artifacts(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("linux_arm64".into()),
        write_output: true,
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

#[test]
fn sysv_small_aggregate_entry_spreads_consecutive_gprs() {
    let canary = pass_canary("targets/sysv_small_aggregate_entry");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-sysv-small-aggregate-entry-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("linux_x64".into()),
        write_output: true,
    })
    .expect("SysV two-eightbyte record should cross-compile through rsi/rdx");

    let image = fs::read(build_dir.join("omega-program")).expect("read emitted x86-64 ELF");
    assert!(
        image.windows(30).any(|window| {
            window[10..13] == [0x49, 0x89, 0xb7] && window[27..30] == [0x49, 0x89, 0x97]
        }),
        "expected consecutive runtime-frame stores from rsi and rdx"
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn sysv_erased_small_aggregate_entry_spreads_only_relevant_fields() {
    let canary = pass_canary("targets/sysv_erased_small_aggregate_entry");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-sysv-erased-small-aggregate-entry-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("linux_x64".into()),
        write_output: true,
    })
    .expect("erased-stripped SysV record should cross-compile through rsi/rdx");

    let image = fs::read(build_dir.join("omega-program")).expect("read emitted x86-64 ELF");
    assert!(
        image.windows(30).any(|window| {
            window[10..13] == [0x49, 0x89, 0xb7] && window[27..30] == [0x49, 0x89, 0x97]
        }),
        "erased evidence must not interrupt consecutive rsi/rdx frame stores"
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn sysv_hfa_entry_argument_packs_eightbytes_into_xmm_registers() {
    let canary = pass_canary("targets/sysv_hfa_entry_argument");
    let build_dir =
        std::env::temp_dir().join(format!("omega-sysv-hfa-entry-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("linux_x64".into()),
        write_output: true,
    })
    .expect("SysV f64 pair entry should cross-compile through xmm0/xmm1");

    let image = fs::read(build_dir.join("omega-program")).expect("read emitted x86-64 ELF");
    assert!(
        image.windows(38).any(|window| {
            window[10..15] == [0xf2, 0x41, 0x0f, 0x11, 0x87]
                && window[29..34] == [0xf2, 0x41, 0x0f, 0x11, 0x8f]
        }),
        "expected packed entry stores from xmm0 and xmm1"
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn sysv_mixed_aggregate_entry_uses_independent_register_banks() {
    let canary = pass_canary("targets/sysv_mixed_aggregate_entry");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-sysv-mixed-aggregate-entry-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("linux_x64".into()),
        write_output: true,
    })
    .expect("SysV INTEGER/SSE entry record should cross-compile through rsi/xmm0");

    let image = fs::read(build_dir.join("omega-program")).expect("read emitted x86-64 ELF");
    assert!(
        image.windows(55).any(|window| {
            window[10..13] == [0x49, 0x89, 0xb7]
                && window[27..32] == [0xf2, 0x41, 0x0f, 0x11, 0x87]
                && window[46..51] == [0xf2, 0x41, 0x0f, 0x11, 0x8f]
        }),
        "expected mixed entry stores from rsi/xmm0 followed by scalar xmm1"
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn sysv_mixed_aggregate_entry_rolls_wholly_to_stack() {
    let canary = pass_canary("targets/sysv_mixed_aggregate_stack_entry");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-sysv-mixed-aggregate-stack-entry-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("linux_x64".into()),
        write_output: true,
    })
    .expect("register-exhausted SysV mixed record should cross-compile wholly from the stack");

    let image = fs::read(build_dir.join("omega-program")).expect("read emitted x86-64 ELF");
    let first_load = [0x4c, 0x8b, 0x94, 0x24, 72, 0, 0, 0];
    let second_load = [0x4c, 0x8b, 0x94, 0x24, 80, 0, 0, 0];
    assert!(
        image
            .windows(43)
            .any(|window| { window[10..18] == first_load && window[35..43] == second_load }),
        "expected both mixed-record fragments loaded from the incoming stack"
    );
    assert!(
        image
            .windows(5)
            .any(|window| window == [0xf2, 0x41, 0x0f, 0x11, 0x87]),
        "expected the trailing float to retain xmm0 after aggregate rollback"
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn sysv_small_aggregate_entry_rolls_wholly_to_stack() {
    let canary = pass_canary("targets/sysv_small_aggregate_stack_entry");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-sysv-small-aggregate-stack-entry-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("linux_x64".into()),
        write_output: true,
    })
    .expect("register-exhausted SysV record should cross-compile wholly from the stack");

    let image = fs::read(build_dir.join("omega-program")).expect("read emitted x86-64 ELF");
    let first_load = [0x4c, 0x8b, 0x94, 0x24, 72, 0, 0, 0];
    let second_load = [0x4c, 0x8b, 0x94, 0x24, 80, 0, 0, 0];
    assert!(
        image
            .windows(43)
            .any(|window| { window[10..18] == first_load && window[35..43] == second_load }),
        "expected both aggregate fragments loaded from the incoming stack"
    );
    assert!(
        image.windows(3).any(|window| window == [0x4d, 0x89, 0x8f]),
        "expected the trailing scalar to retain the rolled-back r9 slot"
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn sysv_large_aggregate_entry_copies_the_memory_class_stack_value() {
    let canary = pass_canary("targets/sysv_large_aggregate_entry");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-sysv-large-aggregate-entry-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("linux_x64".into()),
        write_output: true,
    })
    .expect("SysV MEMORY-class entry record should copy from the incoming stack");

    let image = fs::read(build_dir.join("omega-program")).expect("read emitted x86-64 ELF");
    for source_offset in [72u8, 80, 88] {
        let load = [0x4c, 0x8b, 0x94, 0x24, source_offset, 0, 0, 0];
        assert!(
            image.windows(load.len()).any(|window| window == load),
            "expected MEMORY-class fragment load from incoming rsp+{source_offset}"
        );
    }
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn sysv_wide_aggregate_entry_uses_general_memory_classification() {
    let canary = pass_canary("targets/sysv_wide_aggregate_entry");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-sysv-wide-aggregate-entry-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("linux_x64".into()),
        write_output: true,
    })
    .expect("SysV record beyond 32 bytes should use general MEMORY stack passing");

    let image = fs::read(build_dir.join("omega-program")).expect("read emitted x86-64 ELF");
    for source_offset in [72u8, 80, 88, 96, 104] {
        let load = [0x4c, 0x8b, 0x94, 0x24, source_offset, 0, 0, 0];
        assert!(
            image.windows(load.len()).any(|window| window == load),
            "expected wide MEMORY-class fragment load from incoming rsp+{source_offset}"
        );
    }
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn sysv_large_result_entry_saves_and_uses_the_hidden_pointer() {
    let canary = pass_canary("targets/sysv_large_result_entry");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-sysv-large-result-entry-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile_with_auxiliary_artifacts(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("linux_x64".into()),
        write_output: true,
    })
    .expect("SysV MEMORY-result entry should preserve and populate its hidden pointer");

    let footprint_artifact = fs::read_to_string(build_dir.join("08_boundary_footprints.json"))
        .expect("SysV indirect-result footprint evidence should be written");
    assert!(
        footprint_artifact.contains("\"boundary_contract_fingerprint\": \"0x")
            && footprint_artifact.contains("\"origin\": \"call_return_mechanics\"")
            && footprint_artifact.contains("\"origin\": \"exit_indirect_result_copy\"")
            && footprint_artifact.contains("\"enumeration_complete\": false"),
        "SysV hidden-result copy must bind evidence to its contract without claiming final completeness"
    );

    let image = fs::read(build_dir.join("omega-program")).expect("read emitted x86-64 ELF");
    assert!(
        image.windows(34).any(|window| {
            window[10..13] == [0x49, 0x89, 0xbf] && window[27..30] == [0x49, 0x89, 0xb7]
        }),
        "expected hidden rdi capture before the declared rsi parameter store"
    );
    assert!(
        image.windows(3).any(|window| window == [0x4d, 0x8b, 0xbf]),
        "expected terminal record copy through the saved result pointer"
    );
    assert!(
        image.windows(3).any(|window| window == [0x49, 0x8b, 0x87]),
        "expected the saved result pointer returned in rax"
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn sysv_large_hfa_result_entry_remains_memory_class() {
    let canary = pass_canary("targets/sysv_large_hfa_result_entry");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-sysv-large-hfa-result-entry-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("linux_x64".into()),
        write_output: true,
    })
    .expect("SysV HFA above 16 bytes should remain a MEMORY-class entry result");

    let image = fs::read(build_dir.join("omega-program")).expect("read emitted x86-64 ELF");
    assert!(
        image.windows(3).any(|window| window == [0x49, 0x89, 0xbf]),
        "expected incoming rdi hidden-result pointer capture"
    );
    assert!(
        image.windows(3).any(|window| window == [0x4d, 0x8b, 0xbf]),
        "expected the 24-byte terminal copied through the saved pointer"
    );
    assert!(
        image.windows(3).any(|window| window == [0x49, 0x8b, 0x87]),
        "expected the saved result pointer returned in rax"
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn sysv_small_result_entry_loads_rax_and_rdx() {
    let canary = pass_canary("targets/sysv_small_result_entry");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-sysv-small-result-entry-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("linux_x64".into()),
        write_output: true,
    })
    .expect("SysV INTEGER/INTEGER entry result should load rax/rdx");

    let image = fs::read(build_dir.join("omega-program")).expect("read emitted x86-64 ELF");
    assert!(
        image.windows(34).any(|window| {
            window[10..13] == [0x49, 0x8b, 0x87] && window[27..30] == [0x49, 0x8b, 0x97]
        }),
        "expected terminal record fragments loaded into rax and rdx"
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn sysv_hfa_result_entry_loads_xmm0_and_xmm1() {
    let canary = pass_canary("targets/sysv_hfa_result_entry");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-sysv-hfa-result-entry-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("linux_x64".into()),
        write_output: true,
    })
    .expect("SysV SSE/SSE entry result should load xmm0/xmm1");

    let image = fs::read(build_dir.join("omega-program")).expect("read emitted x86-64 ELF");
    assert!(
        image.windows(38).any(|window| {
            window[10..15] == [0xf2, 0x41, 0x0f, 0x10, 0x87]
                && window[29..34] == [0xf2, 0x41, 0x0f, 0x10, 0x8f]
        }),
        "expected terminal record fragments loaded into xmm0 and xmm1"
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn sysv_mixed_result_entry_loads_rax_and_xmm0() {
    let canary = pass_canary("targets/sysv_mixed_result_entry");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-sysv-mixed-result-entry-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("linux_x64".into()),
        write_output: true,
    })
    .expect("SysV INTEGER/SSE entry result should load rax/xmm0");

    let image = fs::read(build_dir.join("omega-program")).expect("read emitted x86-64 ELF");
    assert!(
        image.windows(36).any(|window| {
            window[10..13] == [0x49, 0x8b, 0x87] && window[27..32] == [0xf2, 0x41, 0x0f, 0x10, 0x87]
        }),
        "expected terminal record fragments loaded into rax and xmm0"
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn sysv_wrapped_float_entry_uses_xmm0_in_both_directions() {
    let canary = pass_canary("targets/sysv_wrapped_float_entry");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-sysv-wrapped-float-entry-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("linux_x64".into()),
        write_output: true,
    })
    .expect("one-eightbyte nested SSE record should use xmm0 for entry and result");

    let image = fs::read(build_dir.join("omega-program")).expect("read emitted x86-64 ELF");
    assert!(
        image
            .windows(5)
            .any(|window| window == [0xf2, 0x41, 0x0f, 0x11, 0x87]),
        "expected incoming wrapped f64 stored from xmm0"
    );
    assert!(
        image
            .windows(5)
            .any(|window| window == [0xf2, 0x41, 0x0f, 0x10, 0x87]),
        "expected terminal wrapped f64 loaded into xmm0"
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn pass_canaries_compile() {
    // COLLECT-ALL, not first-panic: a serial panic at the first failing
    // member masked every member ordered after it (this is the same
    // umbrella-masking pattern that hid a real interpreter bug behind the
    // differential's tick_count stop). One host-blocked cluster (e.g. the
    // efi members on a non-EFI-lowering host) must not exempt the rest of
    // the corpus from its compile check.
    let _umbrella = CANARY_UMBRELLA_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let mut failures: Vec<String> = Vec::new();
    let filter = std::env::var("OMEGA_PASS_CANARY_FILTER").ok();
    let selected = |canary_name: &&str| {
        filter.as_deref().is_none_or(|filter| {
            filter
                .split(',')
                .map(str::trim)
                .any(|candidate| !candidate.is_empty() && canary_name.contains(candidate))
        })
    };
    let mut selected_count = 0usize;

    let checked_only = CHECKED_ONLY_PASS_CANARIES
        .iter()
        .copied()
        .filter(selected)
        .collect::<Vec<_>>();
    selected_count += checked_only.len();
    failures.extend(
        run_bounded_canary_jobs(&checked_only, |canary_name| {
            let canary = pass_canary(canary_name);
            check_canary(&canary).err().map(|diagnostics| {
                format!(
                    "checked-only {}:\n{}",
                    canary.display(),
                    diagnostics
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join("\n")
                )
            })
        })
        .into_iter()
        .flatten(),
    );

    let coverage_started = std::time::Instant::now();
    let exact_native_coverage = exact_native_coverage::ExactNativeCanaryCoverageIndex::discover()
        .unwrap_or_else(|diagnostic| {
            panic!("cannot audit dedicated exact-native canary coverage: {diagnostic}")
        });
    let coverage_elapsed = coverage_started.elapsed();

    let selected_cross_target = CROSS_TARGET_PASS_CANARIES
        .iter()
        .copied()
        .filter(|(canary_name, _)| selected(canary_name))
        .collect::<Vec<_>>();
    selected_count += selected_cross_target.len();
    let cross_target_elided_count = selected_cross_target
        .iter()
        .filter(|(canary_name, target)| {
            exact_native_coverage
                .unique_cross_target_owner(canary_name, target)
                .is_some()
        })
        .count();
    let cross_target = selected_cross_target
        .into_iter()
        .filter(|(canary_name, target)| {
            exact_native_coverage
                .unique_cross_target_owner(canary_name, target)
                .is_none()
        })
        .collect::<Vec<_>>();
    failures.extend(
        run_bounded_canary_jobs(&cross_target, |(canary_name, target)| {
            let canary = pass_canary(canary_name);
            compile_canary_without_output_for_target(&canary, target)
                .err()
                .map(|diagnostics| {
                    format!(
                        "cross-target {target} {}:\n{}",
                        canary.display(),
                        diagnostics
                            .iter()
                            .map(ToString::to_string)
                            .collect::<Vec<_>>()
                            .join("\n")
                    )
                })
        })
        .into_iter()
        .flatten(),
    );

    let selected_rooted_target = ROOTED_TARGET_BACKEND_PASS_CANARIES
        .iter()
        .copied()
        .filter(|(canary_name, _)| selected(canary_name))
        .collect::<Vec<_>>();
    selected_count += selected_rooted_target.len();
    let rooted_target_elided_count = selected_rooted_target
        .iter()
        .filter(|(canary_name, target)| {
            exact_native_coverage
                .unique_rooted_target_owner(canary_name, target)
                .is_some()
        })
        .count();
    let rooted_target = selected_rooted_target
        .into_iter()
        .filter(|(canary_name, target)| {
            exact_native_coverage
                .unique_rooted_target_owner(canary_name, target)
                .is_none()
        })
        .collect::<Vec<_>>();
    failures.extend(
        run_bounded_canary_jobs(&rooted_target, |(canary_name, target)| {
            let canary = pass_canary(canary_name);
            compile_rooted_backend_canary_without_output_for_target(&canary, target)
                .err()
                .map(|diagnostics| {
                    format!(
                        "rooted target {target} {}:\n{}",
                        canary.display(),
                        diagnostics
                            .iter()
                            .map(ToString::to_string)
                            .collect::<Vec<_>>()
                            .join("\n")
                    )
                })
        })
        .into_iter()
        .flatten(),
    );

    #[cfg(windows)]
    {
        let windows_host = WINDOWS_HOST_PASS_CANARIES
            .iter()
            .copied()
            .filter(selected)
            .collect::<Vec<_>>();
        selected_count += windows_host.len();
        failures.extend(
            run_bounded_canary_jobs(&windows_host, |canary_name| {
                let canary = pass_canary(canary_name);
                compile_legacy_backend_canary_without_output(&canary)
                    .err()
                    .map(|diagnostics| {
                        format!(
                            "windows-host {}:\n{}",
                            canary.display(),
                            diagnostics
                                .iter()
                                .map(ToString::to_string)
                                .collect::<Vec<_>>()
                                .join("\n")
                        )
                    })
            })
            .into_iter()
            .flatten(),
        );
    }
    let selected_active = ACTIVE_PASS_CANARIES
        .iter()
        .copied()
        .filter(selected)
        .collect::<Vec<_>>();
    selected_count += selected_active.len();
    let rooted_elided_count = selected_active
        .iter()
        .filter(|canary_name| {
            ROOTED_BACKEND_PASS_CANARIES.contains(canary_name)
                && exact_native_coverage
                    .unique_rooted_owner(canary_name)
                    .is_some()
        })
        .count();
    let legacy_elided_count = selected_active
        .iter()
        .filter(|canary_name| {
            !ROOTED_BACKEND_PASS_CANARIES.contains(canary_name)
                && exact_native_coverage
                    .unique_legacy_owner(canary_name)
                    .is_some()
        })
        .count();
    let active = selected_active
        .into_iter()
        .filter(|canary_name| {
            if ROOTED_BACKEND_PASS_CANARIES.contains(canary_name) {
                exact_native_coverage
                    .unique_rooted_owner(canary_name)
                    .is_none()
            } else {
                exact_native_coverage
                    .unique_legacy_owner(canary_name)
                    .is_none()
            }
        })
        .collect::<Vec<_>>();
    if std::env::var_os("OMEGA_PASS_CANARY_REPORT_COUNTS").is_some() {
        eprintln!(
            "pass-canary coverage: selected-active={} rooted-exact-native-elided={} legacy-exact-native-elided={} active-compiled={} cross-target-elided={} cross-target-compiled={} rooted-target-elided={} rooted-target-compiled={} source-files={} source-bytes={} scan-micros={}",
            active.len() + rooted_elided_count + legacy_elided_count,
            rooted_elided_count,
            legacy_elided_count,
            active.len(),
            cross_target_elided_count,
            cross_target.len(),
            rooted_target_elided_count,
            rooted_target.len(),
            exact_native_coverage.source_file_count(),
            exact_native_coverage.source_byte_count(),
            coverage_elapsed.as_micros(),
        );
    }
    failures.extend(
        run_bounded_canary_jobs(&active, |canary_name| {
            let canary = pass_canary(canary_name);
            let result = if ROOTED_BACKEND_PASS_CANARIES.contains(canary_name) {
                compile_rooted_backend_canary_without_output(&canary)
            } else {
                compile_legacy_backend_canary_without_output(&canary)
            };
            result.err().map(|diagnostics| {
                format!(
                    "{}:\n{}",
                    canary.display(),
                    diagnostics
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join("\n")
                )
            })
        })
        .into_iter()
        .flatten(),
    );

    assert!(
        filter.is_none() || selected_count > 0,
        "OMEGA_PASS_CANARY_FILTER matched no active pass canaries"
    );
    assert!(
        failures.is_empty(),
        "{} pass canary(ies) failed to compile:\n\n{}",
        failures.len(),
        failures.join("\n\n")
    );
}

#[test]
fn discovered_exact_native_coverage_is_consistent() {
    let started = std::time::Instant::now();
    let coverage = exact_native_coverage::ExactNativeCanaryCoverageIndex::discover()
        .expect("canary test sources should form one exact-native coverage index");
    let elapsed = started.elapsed();
    let rooted_active = ROOTED_BACKEND_PASS_CANARIES
        .iter()
        .copied()
        .filter(|canary| ACTIVE_PASS_CANARIES.contains(canary))
        .collect::<Vec<_>>();
    let uniquely_rooted = rooted_active
        .iter()
        .filter(|canary| coverage.unique_rooted_owner(canary).is_some())
        .count();
    let legacy_active = ACTIVE_PASS_CANARIES
        .iter()
        .copied()
        .filter(|canary| !ROOTED_BACKEND_PASS_CANARIES.contains(canary))
        .collect::<Vec<_>>();
    let uniquely_legacy = legacy_active
        .iter()
        .filter(|canary| coverage.unique_legacy_owner(canary).is_some())
        .count();
    let uniquely_cross_target = CROSS_TARGET_PASS_CANARIES
        .iter()
        .filter(|(canary, target)| coverage.unique_cross_target_owner(canary, target).is_some())
        .count();
    let uniquely_rooted_target = ROOTED_TARGET_BACKEND_PASS_CANARIES
        .iter()
        .filter(|(canary, target)| {
            coverage
                .unique_rooted_target_owner(canary, target)
                .is_some()
        })
        .count();
    assert_eq!(
        uniquely_rooted,
        exact_native_coverage::EXPECTED_UNIQUE_ROOTED_ACTIVE_COVERAGE,
        "the discovered rooted duplicate-elision cohort must change deliberately after auditing every added or removed owner"
    );
    assert_eq!(
        uniquely_legacy,
        exact_native_coverage::EXPECTED_UNIQUE_LEGACY_ACTIVE_COVERAGE,
        "the discovered legacy duplicate-elision cohort must change deliberately after auditing every added or removed owner"
    );
    assert_eq!(
        uniquely_cross_target,
        exact_native_coverage::EXPECTED_UNIQUE_CROSS_TARGET_COVERAGE,
        "the discovered cross-target duplicate-elision cohort must change deliberately after auditing every exact fixture/target owner"
    );
    assert_eq!(
        uniquely_rooted_target,
        exact_native_coverage::EXPECTED_UNIQUE_ROOTED_TARGET_COVERAGE,
        "the discovered rooted-target duplicate-elision cohort must change deliberately after auditing every exact fixture/target owner"
    );
    assert!(coverage.source_file_count() >= 25);
    assert!(coverage.source_byte_count() > 1_000_000);
    assert!(coverage.test_body_count() > coverage.qualifying_test_count());
    for canary in rooted_active
        .iter()
        .filter(|canary| coverage.unique_rooted_owner(canary).is_some())
    {
        let fixture = pass_canary(canary);
        assert!(
            fixture.join("main.omg").is_file() && fixture.join("build.omg").is_file(),
            "{} must retain both its source and authored root",
            canary
        );
    }
    for canary in legacy_active
        .iter()
        .filter(|canary| coverage.unique_legacy_owner(canary).is_some())
    {
        assert!(
            pass_canary(canary).join("main.omg").is_file(),
            "{} must retain its compiled source fixture",
            canary
        );
    }
    let positive = coverage
        .unique_rooted_owner("arithmetic/runtime_unsigned_modulo_call_argument_exit")
        .expect("known rooted exact-native owner should be discovered");
    assert_eq!(positive.expected_status, 70);
    assert_eq!(
        positive.test_name,
        "runtime_unsigned_modulo_call_argument_exit_canary_runs"
    );
    assert!(
        positive
            .source_path
            .ends_with("canary_suite/arithmetic_and_data.rs")
    );
    assert_eq!(
        coverage.rooted_owner_count("ownership/linear_transfer_and_consume"),
        2,
        "the repeated linear-transfer fixture must remain ambiguous and unelided",
    );
    assert!(
        coverage
            .unique_rooted_owner("ownership/linear_transfer_and_consume")
            .is_none()
    );
    #[cfg(windows)]
    {
        let rooted_windows_positive = coverage
            .unique_rooted_owner("host/runtime_user32_key_state_exit")
            .expect("known rooted Windows exact-native owner should be discovered");
        assert_eq!(rooted_windows_positive.expected_status, 70);
        assert_eq!(
            rooted_windows_positive.test_name,
            "runtime_user32_key_state_exit_canary_runs"
        );
    }
    #[cfg(not(windows))]
    assert_eq!(
        coverage.rooted_owner_count("host/runtime_user32_key_state_exit"),
        0
    );
    assert_eq!(
        coverage.legacy_owner_count("host/runtime_user32_key_state_exit"),
        0
    );
    assert_eq!(
        coverage.legacy_owner_count("traits/boundary_trait_effects_host_call"),
        0
    );
    let cross_target_positive = coverage
        .unique_cross_target_owner("targets/sysv_small_result_entry", "linux_x64")
        .expect("known exact cross-target owner should be discovered");
    assert_eq!(
        cross_target_positive.test_name,
        "sysv_small_result_entry_loads_rax_and_rdx"
    );
    assert!(
        cross_target_positive
            .source_path
            .ends_with("canary_suite/entry_and_abi.rs")
    );
    assert_eq!(
        coverage.cross_target_owner_count("build/receiver_bound_program_entry", "windows_x64"),
        0,
        "known cross-target control without a dedicated exact owner must remain in the umbrella"
    );
    let rooted_target_positive = coverage
        .unique_rooted_target_owner("providers/external_leaf_syscall_compile", "linux_arm64")
        .expect("known exact rooted-target owner should be discovered");
    assert_eq!(
        rooted_target_positive.test_name,
        "external_leaf_syscall_reaches_linux_x64_backend"
    );
    assert_eq!(
        coverage.rooted_target_owner_count("time/runtime_time_host_native_exit", "windows_x64"),
        0,
        "known rooted-target control without a dedicated exact owner must remain in the umbrella"
    );
    eprintln!(
        "exact-native coverage index: files={} bytes={} test-bodies={} qualifying-tests={} qualifying-target-compiles={} unique-rooted-active={} unique-legacy-active={} unique-cross-target={} unique-rooted-target={} scan-micros={}",
        coverage.source_file_count(),
        coverage.source_byte_count(),
        coverage.test_body_count(),
        coverage.qualifying_test_count(),
        coverage.qualifying_target_compile_count(),
        uniquely_rooted,
        uniquely_legacy,
        uniquely_cross_target,
        uniquely_rooted_target,
        elapsed.as_micros(),
    );
}

#[test]
fn efi_freestanding_skeleton_emits_importless_subsystem_10_pe() {
    // The first-boot milestone-1 skeleton (BOOTED under QEMU/OVMF 2026-07-03:
    // "Image Return Status = Success"; returning 5 printed "Warning Stale
    // Data"). This build independently selects subsystem 10 and a freestanding
    // empty host ABI baseline, so the emitted PE32+ must have no import directory/IAT
    // (services arrive via the entry's parameters, never imports). This pins
    // the emitted HEADER FACTS so a regression that re-populates host bindings
    // for the EFI target (or loses the empty-import path) fails here, without
    // needing QEMU in CI.
    let canary = pass_canary("targets/efi_freestanding_skeleton");
    let build_dir = std::env::temp_dir().join(format!("omega-efi-skeleton-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("uefi_x64".into()),
        write_output: true,
    })
    .expect("EFI freestanding skeleton should compile");

    let image = fs::read(build_dir.join("omega-program.exe")).expect("emitted image should exist");
    let e_lfanew = u32::from_le_bytes(image[0x3c..0x40].try_into().unwrap()) as usize;
    let optional_header = e_lfanew + 4 + 20;
    let magic = u16::from_le_bytes(
        image[optional_header..optional_header + 2]
            .try_into()
            .unwrap(),
    );
    assert_eq!(magic, 0x20b, "expected a PE32+ optional header");
    let subsystem = u16::from_le_bytes(
        image[optional_header + 68..optional_header + 70]
            .try_into()
            .unwrap(),
    );
    assert_eq!(subsystem, 10, "expected subsystem EFI_APPLICATION (10)");
    // Data directories: import = index 1, IAT = index 12 (each 8 bytes at
    // optional_header + 112 + index*8 for PE32+).
    let directory = |index: usize| -> (u32, u32) {
        let offset = optional_header + 112 + index * 8;
        (
            u32::from_le_bytes(image[offset..offset + 4].try_into().unwrap()),
            u32::from_le_bytes(image[offset + 4..offset + 8].try_into().unwrap()),
        )
    };
    assert_eq!(directory(1), (0, 0), "expected NO import directory");
    assert_eq!(directory(12), (0, 0), "expected NO import address table");

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn efi_entry_arguments_prologue_unmarshals_rcx_rdx() {
    // The entry prologue's argument unmarshal (BOOT-VERIFIED under QEMU/OVMF:
    // the same program returns 4 without the stub and 5 with it -- firmware's
    // ImageHandle arrives through RCX). Pin the emitted OPCODE SKELETON at the
    // very start of .text so CI needs no QEMU: for each of the two declared
    // parameters, `mov r15, imm64` (49 BF <8-byte frame base, relocated>) then
    // `mov [r15+disp32], rcx/rdx` (49 89 8F / 49 89 97 + disp32 0 / 8).
    let canary = pass_canary("targets/efi_entry_arguments");
    let build_dir = std::env::temp_dir().join(format!("omega-efi-args-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("uefi_x64".into()),
        write_output: true,
    })
    .expect("EFI entry-arguments canary should compile");

    let image = fs::read(build_dir.join("omega-program.exe")).expect("emitted image should exist");
    // Locate .text's raw offset from the section table.
    let e_lfanew = u32::from_le_bytes(image[0x3c..0x40].try_into().unwrap()) as usize;
    let optional_size =
        u16::from_le_bytes(image[e_lfanew + 20..e_lfanew + 22].try_into().unwrap()) as usize;
    let section_count = u16::from_le_bytes(image[e_lfanew + 6..e_lfanew + 8].try_into().unwrap());
    let sections = e_lfanew + 4 + 20 + optional_size;
    let text_raw = (0..section_count as usize)
        .map(|index| sections + index * 40)
        .find(|offset| &image[*offset..*offset + 6] == b".text\0")
        .map(|offset| {
            u32::from_le_bytes(image[offset + 20..offset + 24].try_into().unwrap()) as usize
        })
        .expect(".text section should exist");

    let entry = image[text_raw..]
        .windows(34)
        .find(|window| {
            window[0..2] == [0x49, 0xbf]
                && window[10..13] == [0x49, 0x89, 0x8f]
                && window[17..19] == [0x49, 0xbf]
                && window[27..30] == [0x49, 0x89, 0x97]
        })
        .expect("entry argument unmarshal sequence should follow the target prologue");
    // Store 0: mov r15, imm64 ; mov [r15+0], rcx
    assert_eq!(&entry[0..2], &[0x49, 0xbf], "store 0: mov r15, imm64");
    assert_eq!(
        &entry[10..13],
        &[0x49, 0x89, 0x8f],
        "store 0: mov [r15+disp32], rcx"
    );
    assert_eq!(
        &entry[13..17],
        &0u32.to_le_bytes(),
        "param 0 frame offset 0"
    );
    // Store 1: mov r15, imm64 ; mov [r15+8], rdx
    assert_eq!(&entry[17..19], &[0x49, 0xbf], "store 1: mov r15, imm64");
    assert_eq!(
        &entry[27..30],
        &[0x49, 0x89, 0x97],
        "store 1: mov [r15+disp32], rdx"
    );
    assert_eq!(
        &entry[30..34],
        &8u32.to_le_bytes(),
        "param 1 frame offset 8"
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn efi_float_entry_argument_unmarshals_xmm0() {
    let canary = pass_canary("targets/efi_float_entry_argument");
    let build_dir =
        std::env::temp_dir().join(format!("omega-efi-float-arg-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("uefi_x64".into()),
        write_output: true,
    })
    .expect("EFI float entry-argument canary should compile");

    let image = fs::read(build_dir.join("omega-program.exe")).expect("emitted image should exist");
    let e_lfanew = u32::from_le_bytes(image[0x3c..0x40].try_into().unwrap()) as usize;
    let optional_size =
        u16::from_le_bytes(image[e_lfanew + 20..e_lfanew + 22].try_into().unwrap()) as usize;
    let section_count = u16::from_le_bytes(image[e_lfanew + 6..e_lfanew + 8].try_into().unwrap());
    let sections = e_lfanew + 4 + 20 + optional_size;
    let text_raw = (0..section_count as usize)
        .map(|index| sections + index * 40)
        .find(|offset| &image[*offset..*offset + 6] == b".text\0")
        .map(|offset| {
            u32::from_le_bytes(image[offset + 20..offset + 24].try_into().unwrap()) as usize
        })
        .expect(".text section should exist");

    let entry = image[text_raw..]
        .windows(19)
        .find(|window| {
            window[0..2] == [0x49, 0xbf] && window[10..15] == [0xf2, 0x41, 0x0f, 0x11, 0x87]
        })
        .expect("floating entry argument unmarshal should follow the target prologue");
    assert_eq!(&entry[0..2], &[0x49, 0xbf], "mov r15, frame base");
    assert_eq!(
        &entry[10..15],
        &[0xf2, 0x41, 0x0f, 0x11, 0x87],
        "movsd [r15+disp32], xmm0"
    );
    assert_eq!(&entry[15..19], &0u32.to_le_bytes(), "frame offset 0");

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn efi_float_entry_result_round_trips_through_xmm0() {
    let canary = pass_canary("targets/efi_float_result_entry");
    let build_dir =
        std::env::temp_dir().join(format!("omega-efi-float-result-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("uefi_x64".into()),
        write_output: true,
    })
    .expect("EFI float arithmetic entry-result canary should compile");

    let image = fs::read(build_dir.join("omega-program.exe")).expect("emitted image should exist");
    assert!(
        image
            .windows(5)
            .any(|window| window == [0xf2, 0x41, 0x0f, 0x11, 0x87]),
        "expected incoming f64 stored from XMM0"
    );
    assert!(
        image
            .windows(5)
            .any(|window| window == [0xf2, 0x41, 0x0f, 0x10, 0x87]),
        "expected computed terminal f64 loaded into XMM0"
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn float_literal_entry_result_uses_native_vector_registers() {
    let canary = pass_canary("targets/efi_float_literal_result_entry");
    for target in ["uefi_x64", "linux_arm64"] {
        let scratch = std::env::temp_dir().join(format!(
            "omega-{target}-float-literal-result-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        let src_dir = scratch.join("src");
        let out_dir = scratch.join("out");
        fs::create_dir_all(&src_dir).expect("scratch source directory");
        fs::copy(canary.join("main.omg"), src_dir.join("main.omg")).expect("copy canary");
        fs::write(
            src_dir.join("build.omg"),
            format!("target {target} {{\n}}\n"),
        )
        .expect("write target manifest");

        compile(CompileOptions {
            root_path: src_dir.join("main.omg"),
            build_dir: Some(out_dir.clone()),
            target_name: Some(target.into()),
            write_output: true,
        })
        .expect("float-literal entry result should cross-compile");

        let output_name = if target == "uefi_x64" {
            "omega-program.exe"
        } else {
            "omega-program"
        };
        let image = fs::read(out_dir.join(output_name)).expect("read emitted image");
        if target == "uefi_x64" {
            assert!(
                image
                    .windows(5)
                    .any(|window| window == [0xf2, 0x41, 0x0f, 0x10, 0x87]),
                "UEFI x64 float-literal entry missing terminal XMM0 scratch load"
            );
            assert!(
                image
                    .windows(8)
                    .any(|window| { window == 1.5f64.to_bits().to_le_bytes() }),
                "UEFI x64 float-literal entry missing the f64 bit pattern"
            );
        } else {
            assert!(
                image.windows(4).any(|window| {
                    let instruction = u32::from_le_bytes(window.try_into().expect("word"));
                    instruction & !0x003f_fc00 == 0xfd40_0200
                }),
                "Linux ARM64 float-literal entry missing terminal d0 scratch load"
            );
        }
        let _ = fs::remove_dir_all(&scratch);
    }
}

#[test]
fn scalar_float_entry_result_uses_native_vector_registers_on_linux() {
    let canary = pass_canary("targets/efi_float_result_entry");
    for target in ["linux_x64", "linux_arm64"] {
        let scratch = std::env::temp_dir().join(format!(
            "omega-{target}-float-entry-result-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        let src_dir = scratch.join("src");
        let out_dir = scratch.join("out");
        fs::create_dir_all(&src_dir).expect("scratch source directory");
        fs::copy(canary.join("main.omg"), src_dir.join("main.omg")).expect("copy canary");
        fs::write(
            src_dir.join("build.omg"),
            format!("target {target} {{\n}}\n"),
        )
        .expect("write target manifest");

        compile(CompileOptions {
            root_path: src_dir.join("main.omg"),
            build_dir: Some(out_dir.clone()),
            target_name: Some(target.into()),
            write_output: true,
        })
        .expect("scalar-float arithmetic entry result should cross-compile");

        let image = fs::read(out_dir.join("omega-program")).expect("read emitted ELF");
        if target == "linux_x64" {
            for (name, opcode) in [
                ("incoming XMM0 store", [0xf2, 0x41, 0x0f, 0x11, 0x87]),
                ("terminal XMM0 load", [0xf2, 0x41, 0x0f, 0x10, 0x87]),
            ] {
                assert!(
                    image.windows(opcode.len()).any(|window| window == opcode),
                    "Linux x64 scalar-float entry missing {name}"
                );
            }
        } else {
            assert!(
                image
                    .windows(4)
                    .any(|window| window == 0xfd00_0200u32.to_le_bytes()),
                "Linux ARM64 scalar-float entry missing incoming d0 store"
            );
            assert!(
                image.windows(4).any(|window| {
                    let instruction = u32::from_le_bytes(window.try_into().expect("word"));
                    instruction & !0x003f_fc00 == 0xfd40_0200
                }),
                "Linux ARM64 scalar-float entry missing terminal d0 scratch load"
            );
        }
        let _ = fs::remove_dir_all(&scratch);
    }
}

#[test]
fn constant_u64_entry_result_uses_the_declared_native_width() {
    let canary = pass_canary("targets/efi_u64_constant_result_entry");
    let build_dir =
        std::env::temp_dir().join(format!("omega-efi-u64-result-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("uefi_x64".into()),
        write_output: true,
    })
    .expect("EFI u64 constant result should compile");
    let image = fs::read(build_dir.join("omega-program.exe")).expect("read emitted PE");
    assert!(
        image
            .windows(10)
            .any(|window| window == [0x48, 0xb8, 7, 0, 0, 0, 0, 0, 0, 0]),
        "u64 constant terminal must write the full RAX register"
    );
    let _ = fs::remove_dir_all(&build_dir);

    let scratch = std::env::temp_dir().join(format!(
        "omega-linux-arm64-u64-result-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let src_dir = scratch.join("src");
    let out_dir = scratch.join("out");
    fs::create_dir_all(&src_dir).expect("scratch source directory");
    fs::copy(canary.join("main.omg"), src_dir.join("main.omg")).expect("copy canary");
    fs::write(src_dir.join("build.omg"), "target linux_arm64 {\n}\n")
        .expect("write target manifest");

    compile(CompileOptions {
        root_path: src_dir.join("main.omg"),
        build_dir: Some(out_dir.clone()),
        target_name: Some("linux_arm64".into()),
        write_output: true,
    })
    .expect("Linux ARM64 u64 constant result should compile");
    let image = fs::read(out_dir.join("omega-program")).expect("read emitted ELF");
    assert!(
        image
            .windows(4)
            .any(|window| window == 0xd280_00e0u32.to_le_bytes()),
        "u64 constant terminal must write the full X0 register"
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn efi_small_aggregate_entry_uses_rcx_and_rax() {
    let canary = pass_canary("targets/efi_small_aggregate_entry");
    let build_dir =
        std::env::temp_dir().join(format!("omega-efi-small-record-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("uefi_x64".into()),
        write_output: true,
    })
    .expect("Microsoft x64 direct record entry should compile");

    let image = fs::read(build_dir.join("omega-program.exe")).expect("emitted image should exist");
    assert!(
        image
            .windows(17)
            .any(|window| { window[0..2] == [0x49, 0xbf] && window[10..13] == [0x49, 0x89, 0x8f] }),
        "expected the incoming eight-byte record stored from rcx"
    );
    assert!(
        image
            .windows(17)
            .any(|window| { window[0..2] == [0x49, 0xbf] && window[10..13] == [0x49, 0x8b, 0x87] }),
        "expected the terminal eight-byte record loaded into rax"
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn efi_large_result_entry_saves_rcx_shifts_argument_and_returns_pointer() {
    let canary = pass_canary("targets/efi_large_result_entry");
    let build_dir =
        std::env::temp_dir().join(format!("omega-efi-large-result-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("uefi_x64".into()),
        write_output: true,
    })
    .expect("Microsoft x64 indirect-result entry should compile");

    let image = fs::read(build_dir.join("omega-program.exe")).expect("emitted image should exist");
    assert!(
        image.windows(34).any(|window| {
            window[10..13] == [0x49, 0x89, 0x8f] && window[27..30] == [0x49, 0x89, 0x97]
        }),
        "expected hidden rcx capture before the shifted declared rdx parameter"
    );
    assert!(
        image.windows(3).any(|window| window == [0x4d, 0x8b, 0xbf]),
        "expected terminal record copy through the saved result pointer"
    );
    assert!(
        image.windows(3).any(|window| window == [0x49, 0x8b, 0x87]),
        "expected the saved result pointer returned in rax"
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn efi_large_aggregate_entry_copies_from_rdx_pointer() {
    let canary = pass_canary("targets/efi_large_aggregate_entry");
    let build_dir =
        std::env::temp_dir().join(format!("omega-efi-large-arg-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("uefi_x64".into()),
        write_output: true,
    })
    .expect("Microsoft x64 register-indirect entry record should compile");

    let image = fs::read(build_dir.join("omega-program.exe")).expect("emitted image should exist");
    assert!(
        image
            .windows(5)
            .any(|window| window == [0x4c, 0x8b, 0xda, 0x49, 0xbf]),
        "expected the RDX record pointer preserved in r11 before frame materialization"
    );
    for source_offset in [0u32, 8, 16] {
        let mut load = vec![0x4d, 0x8b, 0x93];
        load.extend(source_offset.to_le_bytes());
        assert!(
            image.windows(load.len()).any(|window| window == load),
            "expected record fragment load through r11+{source_offset}"
        );
    }
    assert!(
        image.windows(3).any(|window| window == [0x4d, 0x89, 0x87]),
        "expected the following scalar stored from positional R8"
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn efi_large_aggregate_stack_entry_loads_pointer_after_shadow_space() {
    let canary = pass_canary("targets/efi_large_aggregate_stack_entry");
    let build_dir =
        std::env::temp_dir().join(format!("omega-efi-large-stack-arg-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("uefi_x64".into()),
        write_output: true,
    })
    .expect("Microsoft x64 stack-indirect entry record should compile");

    let image = fs::read(build_dir.join("omega-program.exe")).expect("emitted image should exist");
    assert!(
        image
            .windows(10)
            .any(|window| { window == [0x4c, 0x8b, 0x9c, 0x24, 120, 0, 0, 0, 0x49, 0xbf] }),
        "expected fifth-slot pointer loaded after saved frame, return address, and shadow space"
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn efi_fifth_entry_argument_unmarshals_from_the_ms_x64_stack_area() {
    let canary = pass_canary("targets/efi_stack_entry_argument");
    let build_dir =
        std::env::temp_dir().join(format!("omega-efi-stack-arg-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("uefi_x64".into()),
        write_output: true,
    })
    .expect("EFI stack entry-argument canary should compile");

    let image = fs::read(build_dir.join("omega-program.exe")).expect("emitted image should exist");
    let e_lfanew = u32::from_le_bytes(image[0x3c..0x40].try_into().unwrap()) as usize;
    let optional_size =
        u16::from_le_bytes(image[e_lfanew + 20..e_lfanew + 22].try_into().unwrap()) as usize;
    let section_count = u16::from_le_bytes(image[e_lfanew + 6..e_lfanew + 8].try_into().unwrap());
    let sections = e_lfanew + 4 + 20 + optional_size;
    let text_raw = (0..section_count as usize)
        .map(|index| sections + index * 40)
        .find(|offset| &image[*offset..*offset + 6] == b".text\0")
        .map(|offset| {
            u32::from_le_bytes(image[offset + 20..offset + 24].try_into().unwrap()) as usize
        })
        .expect(".text section should exist");

    let prologue = [
        0x53, 0x55, 0x56, 0x57, 0x41, 0x54, 0x41, 0x55, 0x41, 0x56, 0x41, 0x57,
    ];
    assert_eq!(&image[text_raw..text_raw + prologue.len()], &prologue);

    // Four 17-byte register stores precede the fifth parameter's 25-byte stack
    // copy. Locate the exact copy shape rather than baking in the independently
    // evolving fixed-prologue width. Its source displacement is saved frame
    // (80, including MXCSR control-state preservation) + return address (8) +
    // shadow space (32).
    let stack_copy = image[text_raw..]
        .windows(25)
        .find(|window| {
            window[0..2] == [0x49, 0xbf]
                && window[10..18] == [0x4c, 0x8b, 0x94, 0x24, 120, 0, 0, 0]
                && window[18..21] == [0x4d, 0x89, 0x97]
        })
        .expect("fifth stack-argument copy should be present");
    assert_eq!(&stack_copy[0..2], &[0x49, 0xbf], "mov r15, frame base");
    assert_eq!(
        &stack_copy[10..18],
        &[0x4c, 0x8b, 0x94, 0x24, 120, 0, 0, 0],
        "mov r10, [rsp + saved-frame + return-address + shadow-space]"
    );
    assert_eq!(
        &stack_copy[18..25],
        &[0x4d, 0x89, 0x97, 32, 0, 0, 0],
        "mov [r15 + fifth-slot], r10"
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn entry_run_args_bytes_canary_runs() {
    // The canonical entry `Main::run(&self, args: &[u8])`: the prologue binds
    // `args` as a 32-byte view over the spilled argument registers, so
    // `args.len == 32` holds deterministically (exit 5) regardless of what the
    // OS passed in the registers. NATIVE-ONLY (the interpreter has no entry-
    // argument notion yet, so this is not a differential canary). The
    // efi_application twin of this program was boot-verified under QEMU/OVMF
    // ("Warning Stale Data" = the same 5).
    let canary = pass_canary("targets/entry_run_args_bytes");
    let build_dir = std::env::temp_dir().join(format!("omega-run-args-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_with_auxiliary_artifacts(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("entry run-args canary should compile");
    let footprint_artifact = fs::read_to_string(build_dir.join("08_boundary_footprints.json"))
        .expect("entry run-args footprint evidence should be written");
    assert!(
        footprint_artifact.contains("\"origin\": \"entry_storage\"")
            && footprint_artifact.contains("\"origin\": \"entry_slice_descriptor\"")
            && footprint_artifact.contains("\"origin\": \"exit_result_registers\"")
            && footprint_artifact.contains("\"enumeration_complete\": false"),
        "bytes handoff must retain entry-storage, descriptor, and exit-register evidence without claiming final completeness"
    );
    assert_native_exit_code(
        &compilation,
        5,
        "entry run-args canary",
        "the canonical byte-view argument should retain its 32-byte handoff bound",
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_utf16_literal_exit_canary_runs() {
    // `utf16"Hello from Omega"` (CR LF NUL escaped) desugars at parse to the integer array
    // literal of its UTF-16 code units: 'H'=72 at [0], newline=10 at [17], NUL at
    // [18] (exit 70). Native must match the interpreter (both see plain
    // integers -- the sugar is gone before resolution).
    let canary = pass_canary("text/runtime_utf16_literal_exit");
    let build_dir = std::env::temp_dir().join(format!("omega-utf16-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("utf16 literal canary should compile");
    assert_native_exit_code(
        &compilation,
        70,
        "Utf16 literal canary",
        "the greeting's exact Utf16 code units should verify",
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_case_array_element_write_exit_canary_runs() {
    // Array-of-CASE element writes (const + runtime index) with payload
    // read-back -- the case-vocabulary Plan's foundation shape. At{8,4}+At{16,8}
    // matched back = 12 + 24 = exit 36; native must match the interpreter (the
    // interpreter is the L0 build-time engine).
    let canary = pass_canary("collections/runtime_case_array_element_write_exit");
    let build_dir = std::env::temp_dir().join(format!("omega-case-array-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("case-array element write canary should compile");
    assert_native_exit_code(
        &compilation,
        36,
        "case-array element-write canary",
        "both constant- and runtime-indexed case payloads should read back",
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_wire_policy_authored_plan_exit_canary_runs() {
    // RUNG 2b: an inline `CompactBinary::plan` grammar policy AUTHORS the wire
    // plan (L0-evaluated against materialized schema facts incl. FieldKind);
    // the codec's tag bytes come from it, and the hand-computed roundtrip
    // bytes still hold exactly (exit 70). The fail twin proves divergence is
    // a compile error.
    let canary = pass_canary("wire/runtime_wire_policy_authored_plan_exit");
    let build_dir = std::env::temp_dir().join(format!("omega-wire-policy-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("policy-authored wire plan canary should compile");
    assert_native_exit_code(
        &compilation,
        70,
        "policy-authored wire-plan canary",
        "the authored plan should roundtrip its exact bytes",
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_wire_policy_authored_nested_exit_canary_runs() {
    // RUNG 2c: nested CHILD tags come from the child schema's own authored
    // plan -- the byte-pinned nested roundtrip holds exactly with the inline
    // `CompactBinary::plan` policy evaluated for both parent and child.
    let canary = pass_canary("wire/runtime_wire_policy_authored_nested_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-wire-policy-nested-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("policy-authored nested wire plan canary should compile");
    assert_native_exit_code(
        &compilation,
        70,
        "policy-authored nested wire-plan canary",
        "the parent and child authored plans should roundtrip their exact nested bytes",
    );
    let _ = fs::remove_dir_all(&build_dir);

    for target in ["windows_x64", "linux_arm64"] {
        let cross_dir = std::env::temp_dir().join(format!(
            "omega-wire-policy-nested-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&cross_dir);
        let source_dir = cross_dir.join("src");
        let cross_build_dir = cross_dir.join("build");
        fs::create_dir_all(&source_dir).expect("create wire-policy cross-target source");
        fs::copy(canary.join("main.omg"), source_dir.join("main.omg"))
            .expect("copy nested wire-policy canary");
        fs::write(
            source_dir.join("build.omg"),
            hosted_main_program_entry_build(target),
        )
        .expect("write wire-policy cross-target manifest");
        compile(CompileOptions {
            root_path: source_dir.join("main.omg"),
            build_dir: Some(cross_build_dir),
            target_name: Some(target.into()),
            write_output: true,
        })
        .unwrap_or_else(|diagnostics| {
            panic!("nested wire policy should cross-compile for {target}: {diagnostics:?}")
        });
        let _ = fs::remove_dir_all(&cross_dir);
    }
}

#[cfg(windows)]
#[test]
fn efi_struct_handoff_prologue_spreads_registers() {
    // Ladder step 3: the boundary entry's sole struct parameter receives the
    // argument registers spread across its 8-byte chunks. Pins the prologue:
    // store #0 = mov r15,imm64 + mov [r15+0],rcx (49 89 8F disp 0); store #1 =
    // mov r15,imm64 + mov [r15+8],rdx (49 89 97 disp 8).
    let canary = pass_canary("targets/efi_struct_handoff");
    let build_dir =
        std::env::temp_dir().join(format!("omega-struct-handoff-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("struct-handoff canary should compile");
    let bytes = fs::read(build_dir.join("omega-program.exe")).expect("read emitted PE");
    let lfanew = u32::from_le_bytes([bytes[0x3c], bytes[0x3d], bytes[0x3e], bytes[0x3f]]) as usize;
    let opt = lfanew + 4 + 20;
    let opt_size = u16::from_le_bytes([bytes[lfanew + 4 + 16], bytes[lfanew + 4 + 17]]) as usize;
    let section_count = u16::from_le_bytes([bytes[lfanew + 6], bytes[lfanew + 7]]) as usize;
    let mut text_raw = None;
    for section in 0..section_count {
        let header = opt + opt_size + section * 40;
        if &bytes[header..header + 5] == b".text" {
            text_raw = Some(u32::from_le_bytes([
                bytes[header + 20],
                bytes[header + 21],
                bytes[header + 22],
                bytes[header + 23],
            ]) as usize);
        }
    }
    let text = text_raw.expect(".text section");
    // store #0: [10-byte mov r15,imm64] 49 89 8F <disp32 0>
    assert_eq!(&bytes[text..text + 2], &[0x49, 0xbf], "frame-base mov #0");
    assert_eq!(
        &bytes[text + 10..text + 17],
        &[0x49, 0x89, 0x8f, 0, 0, 0, 0],
        "rcx -> handoff.handle @ +0"
    );
    // store #1 immediately follows: 49 BF ... 49 89 97 08 00 00 00
    let second = text + 17;
    assert_eq!(
        &bytes[second..second + 2],
        &[0x49, 0xbf],
        "frame-base mov #1"
    );
    assert_eq!(
        &bytes[second + 10..second + 17],
        &[0x49, 0x89, 0x97, 8, 0, 0, 0],
        "rdx -> handoff.table @ +8"
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[cfg(windows)]
#[test]
fn efi_vtable_call_emits_indirect_dispatch() {
    // The external-leaf VtableSlot(1) call lowers to `mov rax, [rcx+8];
    // call rax` -- read OutputString from the con_out protocol struct and
    // dispatch. Pins those bytes in .text (the whole selection->encode chain;
    // the live boot awaits the reference-param projection routing fix).
    let canary = pass_canary("targets/efi_vtable_call");
    let build_dir = std::env::temp_dir().join(format!("omega-vtable-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile_with_auxiliary_artifacts(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("vtable-call canary should compile");
    let bytes = fs::read(build_dir.join("omega-program.exe")).expect("read emitted PE");
    let needle = [0x48u8, 0x8b, 0x81, 0x08, 0x00, 0x00, 0x00, 0xff, 0xd0];
    assert!(
        bytes.windows(needle.len()).any(|window| window == needle),
        "expected `mov rax, [rcx+8]; call rax` (VtableSlot(1) dispatch) in .text"
    );
    let footprints = fs::read_to_string(build_dir.join("08_boundary_footprints.json"))
        .expect("vtable-call boundary footprints should be emitted");
    assert!(
        footprints.contains("\"origin\": \"compiler_body_outbound_indirect_call\""),
        "vtable dispatch must retain its independently derived call footprint"
    );
    let regions = fs::read_to_string(build_dir.join("13_executable_regions.json"))
        .expect("vtable-call executable-region evidence should be emitted");
    assert!(
        regions.contains("\"certificate_marker\": \"omega.final-footprint-certificate.current\"")
            && regions.contains("\"compiler_function_body_specification\""),
        "vtable dispatch must reach final-byte replay"
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn efi_ref_param_direct_faces_deref_not_flat() {
    // Task #37: the DIRECT guard-subject and machine-target reads through an
    // entry ref-param must DEREFERENCE the pointer slot (pointee copies in the
    // report), never fold flat (`frame_storage@72` = slot 8 + con_out 64).
    let canary = pass_canary("targets/efi_ref_param_direct_faces");
    let build_dir = std::env::temp_dir().join(format!("omega-refparam-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile_with_auxiliary_artifacts(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("ref-param direct-faces canary should compile");
    let report = fs::read_to_string(build_dir.join("backend_report.txt"))
        .expect("backend report should be written");
    assert!(
        report.contains("omega_runtime_frame_storage[ConstOffset(8), Deref, ConstOffset(64)]"),
        "expected the con_out DEREF (place frame[8].deref+64) in the report"
    );
    assert!(
        report.contains("omega_runtime_frame_storage[ConstOffset(8), Deref, ConstOffset(32)]"),
        "expected the firmware_revision DEREF (place frame[8].deref+32) in the report"
    );
    assert!(
        report.contains("omega_runtime_frame_storage[ConstOffset(8), Deref, ConstOffset(48)]"),
        "expected the con_in DEREF (place frame[8].deref+48) feeding the transition arg"
    );
    assert!(
        !report.contains("omega_runtime_frame_storage[ConstOffset(72)]"),
        "flat slot+field read (frame place ConstOffset(72) = con_out) regressed -- an entry-ref-param member folded flat instead of dereferencing"
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn efi_ref_param_call_arg_derefs_and_dispatches() {
    // The direct host-call arg `output_string(table.con_out, ..)` must deref
    // (pointee frame@8 +64), never fold flat (frame_storage@72 fed firmware
    // poison into the vtable dispatch), and the `mov rax,[rcx+8]; call rax`
    // dispatch bytes must survive the hoist.
    let canary = pass_canary("targets/efi_ref_param_call_arg");
    let build_dir = std::env::temp_dir().join(format!("omega-refarg-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile_with_auxiliary_artifacts(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("uefi_x64".into()),
        write_output: true,
    })
    .expect("ref-param call-arg canary should compile");
    let report = fs::read_to_string(build_dir.join("backend_report.txt"))
        .expect("backend report should be written");
    assert!(
        report.contains("omega_runtime_frame_storage[ConstOffset(8), Deref, ConstOffset(64)]"),
        "expected the con_out DEREF (place frame[8].deref+64) feeding the call arg"
    );
    assert!(
        !report.contains("omega_runtime_frame_storage[ConstOffset(72)]"),
        "flat slot+field read (frame place ConstOffset(72)) regressed for the call-arg face"
    );
    let bytes = fs::read(build_dir.join("omega-program.exe")).expect("read emitted PE");
    let needle = [0x48u8, 0x8b, 0x81, 0x08, 0x00, 0x00, 0x00, 0xff, 0xd0];
    assert!(
        bytes.windows(needle.len()).any(|window| window == needle),
        "expected `mov rax, [rcx+8]; call rax` (VtableSlot(1) dispatch) in .text"
    );
    let _ = fs::remove_dir_all(&build_dir);
}

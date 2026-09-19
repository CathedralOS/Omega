use super::fixture_roster;
use crate::{
    CanaryCompileProduct, CanaryCompileSpec, compile, compile_reviewed_repository_fixture,
    entry_free_fixture_build, fail_canary, fs, pass_canary, production_compile,
    unique_no_output_build_dir,
};
use checked_interpreter::InterpretOptions;
use compiler::CheckedCompileRequest;

#[test]
fn checked_compilation_retains_the_exact_selected_program_entry() {
    let canary = pass_canary(fixture_roster::BUILD_EXPLICIT_PROGRAM_ENTRY_BINDING);
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &canary.join("main.omg"),
        Some("windows_x86_64"),
    ))
    .expect("explicit entry canary should reach checked semantics");

    assert_eq!(checked.selected_program_entry_machine(), Some("launch"));
    let selected = checked
        .selected_program_entry()
        .expect("checked compilation must retain the complete selected entry");
    assert_eq!(selected.source_signature().machine_name(), "launch");
    assert_eq!(
        selected.source_signature().state_name(),
        "entry",
        "the Build-selected free machine enters through its implicit `entry` state"
    );
    let plans = selected
        .calling_plans()
        .expect("Windows x86-64 entry must retain semantic and physical calling plans");
    let physical_contract = plans
        .storage_entry
        .physical_contract()
        .expect("the retained semantic storage plan must keep its distinct physical contract");
    assert_eq!(
        physical_contract.requirement_identity(),
        program_entry_plan::WINDOWS_X86_64_PHYSICAL_REQUIREMENT_IDENTITY,
    );
    assert!(
        physical_contract.parameter_type_identities().is_empty(),
        "the Windows loader arrival carries no contractual parameters"
    );
    assert_eq!(
        physical_contract.result_type_identity(),
        program_entry_plan::WINDOWS_X86_64_U32_TYPE_IDENTITY,
    );
    assert_eq!(
        physical_contract.target_slot(),
        target::TargetProfile::WindowsX64.program_entry_slot(),
    );
    assert_eq!(
        physical_contract.target_package(),
        target::ProgramEntryPhysicalContractPackage::WindowsX64,
    );
    let expected_physical_plan =
        program_entry_plan::exact_windows_x86_64_physical_boundary_entry_plan();
    assert_eq!(
        physical_contract.boundary_entry_plan(),
        expected_physical_plan.plan(),
    );
    assert_eq!(
        physical_contract.calling_plan_report_fingerprint(),
        expected_physical_plan.contract_report_fingerprint(),
    );
    assert!(
        physical_contract.matches_exact_windows_x86_64_physical_contract(),
        "build evaluation must produce the exact canonical normalized Windows x86-64 physical contract: {physical_contract:#?}"
    );
    assert_eq!(
        plans
            .semantic_calling_application
            .boundary_entry_plan
            .call
            .policy,
        calling_conventions::CallingPolicy::MicrosoftX64
    );
    native_realization::NativeProgramEntrySettlement::new(
        selected.source_signature(),
        Some((
            &plans.semantic_calling_application,
            &plans.physical_calling_application,
            &plans.storage_entry,
        )),
        selected.fused_service_establishments(),
    )
    .validate_for_target(target::NativeTarget::windows_x64())
    .expect("native settlement must replay the actual authored two-surface applications");
    let outcome = checked_interpreter::interpret_entry(
        &checked,
        checked
            .selected_program_entry_machine()
            .expect("target build selected an exact entry"),
        &[],
        InterpretOptions::default(),
    );
    assert_eq!(outcome.error, None);
}

#[test]
fn repeated_root_binding_execution_rejects() {
    let canary = fail_canary(fixture_roster::BUILD_REPEATED_EVALUATED_ROOT_BINDING);
    let diagnostics = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &canary.join("main.omg"),
        Some("windows_x86_64"),
    ))
    .expect_err("executing the same slot binding twice must reject");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("already bound")),
        "{diagnostics:?}"
    );
}

#[test]
fn evaluated_root_bindings_retain_the_executed_entry() {
    for fixture in fixture_roster::BUILD_EVALUATED_ROOT_BINDINGS {
        let canary = pass_canary(fixture);
        let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
            &canary.join("main.omg"),
            Some("windows_x86_64"),
        ))
        .unwrap_or_else(|error| {
            panic!("{fixture} must evaluate borrowed root authority: {error:?}")
        });
        assert_eq!(
            checked.selected_program_entry_machine(),
            Some("launch"),
            "{fixture}"
        );
    }
}

#[test]
fn uefi_entry_machine_plan_produces_terminal_artifact() {
    let canary = pass_canary(fixture_roster::BUILD_UEFI_PROGRAM_ENTRY_STORAGE_ROOTS);
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &canary.join("main.omg"),
        Some("uefi_x86_64"),
    ))
    .expect("UEFI entry canary should retain its complete typed settlement");
    let selected = checked
        .selected_program_entry()
        .expect("UEFI checked compilation must retain its selected entry");
    let source = selected.source_signature();
    let trees = checked.terminal_production_trees();
    // `Boot::launch` retains both whole-root linear entry claims across its
    // `retain` self-loop, so the checked transitive Unit machine plan exists
    // and lowering emits the claim-aliased two-state graph.
    let plan = trees
        .facts
        .flow
        .terminal_unit_effects
        .composed_for_machine(source.machine_symbol())
        .expect("Boot::launch must retain its checked transitive Unit machine plan");
    assert_eq!(plan.states.len(), 2);
    let produced = terminal_production::TerminalProductionRequest::for_machine_symbol(
        trees,
        source.machine_symbol(),
    )
    .produce_program_entry(source.identity().bytes());
    // Both claims stay pinned on their owned entry roots for the loop's whole
    // cyclic lifetime — nothing in the emitted graph rebinds, consumes, or
    // transfers them — so the verifier's claim-custody fence admits the
    // two-state machine and the canonical Terminal artifact publishes. The
    // physical entry adapter remains downstream of this product.
    let produced = produced
        .expect("the claim-pinned retain cycle must validate and produce the Terminal artifact");
    assert_eq!(produced.receipt().source_machine_name(), "Boot::launch");
}

#[test]
fn checked_uefi_compilation_retains_source_and_two_surface_entry_custody() {
    let canary = pass_canary(fixture_roster::BUILD_UEFI_PROGRAM_ENTRY_STORAGE_ROOTS);
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &canary.join("main.omg"),
        Some("uefi_x86_64"),
    ))
    .expect("UEFI entry canary should retain its complete typed settlement");
    let selected = checked
        .selected_program_entry()
        .expect("UEFI checked compilation must retain its selected entry");
    let source = selected.source_signature();
    let plans = selected
        .calling_plans()
        .expect("UEFI entry must retain semantic and physical calling plans");

    assert_eq!(source.target_slot().owner, target::TargetProfile::UefiX64);
    assert_eq!(source.machine_name(), "Boot::launch");
    assert_eq!(source.visible_parameters().len(), 2);
    assert_eq!(
        source.visible_parameters()[0].role(),
        program_entry_plan::ProgramStorageEntryRootRole::Image
    );
    assert_eq!(
        source.visible_parameters()[1].role(),
        program_entry_plan::ProgramStorageEntryRootRole::InitialStorage
    );
    let physical_contract = plans
        .storage_entry
        .physical_contract()
        .expect("the retained semantic storage plan must keep its distinct physical contract");
    assert_eq!(
        physical_contract.requirement_identity(),
        program_entry_plan::UEFI_X64_PHYSICAL_REQUIREMENT_IDENTITY,
    );
    assert_eq!(
        physical_contract.parameter_type_identities(),
        [
            program_entry_plan::UEFI_X64_IMAGE_HANDLE_TYPE_IDENTITY,
            program_entry_plan::UEFI_X64_SYSTEM_TABLE_REFERENCE_TYPE_IDENTITY,
        ],
    );
    assert_eq!(
        physical_contract.result_type_identity(),
        program_entry_plan::UEFI_X64_STATUS_TYPE_IDENTITY,
    );
    assert_eq!(
        physical_contract.target_slot(),
        target::TargetProfile::UefiX64.program_entry_slot(),
    );
    assert_eq!(
        physical_contract.target_package(),
        target::ProgramEntryPhysicalContractPackage::UefiX64,
    );
    let expected_physical_plan = program_entry_plan::exact_uefi_x64_physical_boundary_entry_plan();
    assert_eq!(
        physical_contract.boundary_entry_plan(),
        expected_physical_plan.plan(),
    );
    assert_eq!(
        physical_contract.calling_plan_report_fingerprint(),
        expected_physical_plan.contract_report_fingerprint(),
    );
    assert!(
        physical_contract.matches_exact_uefi_x64_physical_contract(),
        "build evaluation must produce the exact canonical normalized UEFI physical contract: {physical_contract:#?}"
    );
    assert_eq!(
        plans
            .semantic_calling_application
            .boundary_entry_plan
            .call
            .policy,
        calling_conventions::CallingPolicy::MicrosoftX64
    );
    native_realization::NativeProgramEntrySettlement::new(
        source,
        Some((
            &plans.semantic_calling_application,
            &plans.physical_calling_application,
            &plans.storage_entry,
        )),
        selected.fused_service_establishments(),
    )
    .validate_for_target(target::NativeTarget::uefi_x64())
    .expect("native settlement must replay the actual authored two-surface applications");

    let rejects =
        |semantic: &provider_planning::calling_policy_plans::BoundaryCallingPlanRealization,
         physical: &provider_planning::calling_policy_plans::BoundaryCallingPlanRealization,
         storage: &program_entry_plan::SelectedProgramStorageEntryPlan| {
            assert!(
                native_realization::NativeProgramEntrySettlement::new(
                    source,
                    Some((semantic, physical, storage)),
                    selected.fused_service_establishments(),
                )
                .validate_for_target(target::NativeTarget::uefi_x64())
                .is_err()
            );
        };
    let semantic = &plans.semantic_calling_application;
    let physical = &plans.physical_calling_application;
    rejects(physical, semantic, &plans.storage_entry);
    rejects(semantic, semantic, &plans.storage_entry);
    assert!(
        native_realization::NativeProgramEntrySettlement::new(source, None, &[])
            .validate_for_target(target::NativeTarget::uefi_x64())
            .is_err()
    );

    for physical_role in [false, true] {
        let original = if physical_role { physical } else { semantic };
        let mut wrong_report = original.clone();
        wrong_report.report_fingerprint ^= 1;
        let mut raw_plan_identity = original.clone();
        let plan = raw_plan_identity.replayed_validated_plan().unwrap();
        raw_plan_identity.commitment =
            effects::provider_plan::BoundaryCallingPlanCommitment::from_digest(
                plan.contract_commitment_digest(),
            );
        for changed in [&wrong_report, &raw_plan_identity] {
            if physical_role {
                rejects(semantic, changed, &plans.storage_entry);
            } else {
                rejects(changed, physical, &plans.storage_entry);
            }
        }
    }

    // Recompute even the public application digest and schema row after a
    // structurally valid placement change. The sealed authored plan must still
    // reject it; self-consistent public metadata is not source custody.
    let mut moved = semantic.clone();
    moved.boundary_entry_plan.call.parameters.swap(0, 1);
    let (_, report, commitment) = moved
        .replayed_validated_application()
        .expect("swapped equal-size parameters still form a structurally valid ABI plan");
    moved.report_fingerprint = report;
    moved.commitment = commitment;
    let mut schema = plans.storage_entry.schema().clone();
    let method = schema
        .methods
        .iter_mut()
        .find(|method| method.requirement_identity == plans.storage_entry.requirement_identity())
        .unwrap();
    method.calling_plan_report_fingerprint = Some(report);
    method.calling_plan_commitment = Some(commitment);
    let forged_storage = program_entry_plan::SelectedProgramStorageEntryPlan::from_target_slot(
        source.target_slot(),
        schema,
        plans.storage_entry.requirement_identity().to_owned(),
    )
    .unwrap()
    .with_physical_contract(physical_contract.clone())
    .unwrap();
    rejects(&moved, physical, &forged_storage);

    let mut missing_physical = plans.storage_entry.schema().clone();
    missing_physical
        .methods
        .retain(|method| method.requirement_owner != "UefiPhysicalEntry");
    let missing_physical = program_entry_plan::SelectedProgramStorageEntryPlan::from_target_slot(
        source.target_slot(),
        missing_physical,
        plans.storage_entry.requirement_identity().to_owned(),
    )
    .unwrap()
    .with_physical_contract(physical_contract.clone())
    .unwrap();
    rejects(semantic, physical, &missing_physical);
}

#[test]
fn checked_uefi_os_handoff_invocation_retains_edge_binding() {
    let canary = pass_canary(fixture_roster::BUILD_UEFI_OS_HANDOFF_INVOCATION);
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &canary.join("main.omg"),
        Some("uefi_x86_64"),
    ))
    .expect("UEFI OS-handoff invocation canary should compile");
    assert_eq!(
        checked.selected_program_entry_machine(),
        Some("Loader::run")
    );

    let machine_path = |symbol| checked.typed.symbols.display_path(symbol, "::").to_owned();
    let loader_entry = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine_path(machine.symbol) == "Loader::run")
        .expect("the loader entry machine must be retained");

    // The selected loader entry reaches and invokes both authored boundary
    // surfaces through the target's `UefiOsHandoffCycle::run`: the Boot
    // Services leaves and the compiler-owned terminal edges.
    let invokes: std::collections::BTreeSet<String> = checked
        .typed
        .machine_invokes(loader_entry)
        .iter()
        .map(|invocation| invocation.as_str().to_owned())
        .collect();
    for service in ["UefiBootServices", "UefiOsHandoffTermination"] {
        assert!(
            invokes.contains(service),
            "Loader::run must invoke the {service} boundary, got {invokes:?}"
        );
    }

    // The target package's boundary realizations supply the requirements: the
    // native provider's bodyless machines satisfy `UefiOsHandoffTermination`'s
    // two compiler-owned edges, and the table leaves satisfy the four
    // `UefiBootServices` requirements the authored cycle drives.
    for (provider_machine, requirement) in [
        ("transfer_to_entry", "transfer"),
        ("firmware_return", "firmware_return"),
    ] {
        let provider = checked
            .typed
            .machines()
            .iter()
            .find(|machine| {
                machine_path(machine.symbol)
                    == format!("UefiOsHandoffNativeProvider::{provider_machine}")
            })
            .expect("the target's termination realization must be retained");
        let conformance = checked
            .typed
            .machine_trait_conformances(provider)
            .iter()
            .find(|conformance| conformance.requirement.as_deref() == Some(requirement))
            .expect("the provider realization must satisfy the termination requirement");
        assert_eq!(
            machine_path(conformance.requirement_symbol),
            format!("UefiOsHandoffTermination::{requirement}")
        );
    }
    for (leaf_machine, requirement) in [
        ("allocate_pages_leaf", "allocate_pages"),
        ("free_pages_leaf", "free_pages"),
        ("get_memory_map_leaf", "get_memory_map"),
        ("exit_boot_services_leaf", "exit_boot_services"),
    ] {
        let leaf = checked
            .typed
            .machines()
            .iter()
            .find(|machine| {
                machine_path(machine.symbol) == format!("EfiBootServicesTable::{leaf_machine}")
            })
            .expect("the target's Boot Services leaf must be retained");
        let conformance = checked
            .typed
            .machine_trait_conformances(leaf)
            .iter()
            .find(|conformance| conformance.requirement.as_deref() == Some(requirement))
            .expect("the leaf must satisfy the Boot Services requirement");
        assert_eq!(
            machine_path(conformance.requirement_symbol),
            format!("UefiBootServices::{requirement}")
        );
    }
}

#[test]
fn native_uefi_os_handoff_invocation_reports_missing_boundary_plan() {
    // The checked canary pins the invocation surface; this pins the authored
    // handoff route's first refusing emission stage. The cycle's Boot Services
    // calls live on `UefiOsHandoffLegs` `boundary machine`s — scalar-returning
    // boundary calls carry no state-graph custody admission in the checked
    // unit lane, so each leg owns one firmware call and lands its status on
    // the legs record — and attached-Unit closure refuses them today: a
    // bodied boundary machine carries no boundary plan for a unit caller.
    // Once bodied boundary machines lower as callees, this canary becomes the
    // PE32+ emission assertion.
    let canary = pass_canary(fixture_roster::BUILD_UEFI_OS_HANDOFF_INVOCATION);
    let build_dir = std::env::temp_dir().join(format!("omega-uefi-handoff-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let diagnostics = compile(CanaryCompileSpec {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("uefi_x86_64".into()),
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect_err("the authored cycle must stay pinned at the missing boundary plan");
    let messages: Vec<&str> = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect();
    assert!(
        messages.iter().any(|message| {
            message
                .contains("calls boundary `UefiOsHandoffLegs::acquire`, which has no boundary plan")
        }),
        "expected the pinned boundary-plan refusal, got {messages:?}"
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn checked_compilation_does_not_infer_an_entry_for_legacy_semantic_corpus() {
    let canary = pass_canary(fixture_roster::ARITHMETIC_RUNTIME_CHAINED_FIELD_MUTATION_EXIT);
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &canary.join("main.omg"),
        None,
    ))
    .expect("direct Main entry canary should reach checked semantics");

    assert_eq!(checked.selected_program_entry_machine(), None);
    let outcome = checked_interpreter::interpret_entry(
        &checked,
        "Main::main",
        &[],
        InterpretOptions::default(),
    );
    assert_eq!(outcome.error, None);
    assert_eq!(outcome.exit_code, 70);
}

#[test]
fn production_compile_rejects_an_unrooted_legacy_entry() {
    let scratch = unique_no_output_build_dir();
    let source_dir = scratch.join("source");
    let build_dir = scratch.join("output");
    fs::create_dir_all(&source_dir).expect("create entry-agnostic source directory");
    // A `Main::main` machine that reaches no services: with no provider
    // bindings to review, native production must stop at exact entry
    // admission rather than failing an earlier gate.
    fs::write(
        source_dir.join("main.omg"),
        r#"data Main { v: i32; }
machine Main::main(&mut self) {
    self.v = 24;
}
"#,
    )
    .expect("write service-free source carrying a legacy Main::main name");
    // The companion binds no ProgramEntry, so production reaches entry
    // selection rather than failing module resolution.
    fs::write(
        source_dir.join("build.omg"),
        "machine build(builder: &mut Build) {\n    builder.application(\"entry-free-fixture\");\n}\n",
    )
    .expect("write entry-free build companion");
    let diagnostics = production_compile(CanaryCompileSpec {
        root_path: source_dir.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect_err("production compilation must not discover `Main::main` by name");
    let _ = fs::remove_dir_all(scratch);

    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("requires one exact selected program entry")),
        "missing ProgramEntry selection should fail explicitly: {diagnostics:#?}"
    );
}

#[test]
fn production_check_accepts_entry_agnostic_semantic_corpus() {
    let canary = pass_canary(fixture_roster::ARITHMETIC_RUNTIME_CHAINED_FIELD_MUTATION_EXIT);
    let scratch = unique_no_output_build_dir();
    let source_dir = scratch.join("source");
    let build_dir = scratch.join("output");
    fs::create_dir_all(&source_dir).expect("create entry-agnostic source directory");
    fs::copy(canary.join("main.omg"), source_dir.join("main.omg"))
        .expect("copy source without its entry-selecting build companion");
    // Same entry-free companion: the declared standard-library dependency
    // keeps module resolution working while no ProgramEntry is selected.
    fs::write(
        source_dir.join("build.omg"),
        entry_free_fixture_build(&canary),
    )
    .expect("write entry-free build companion");
    let report = production_compile(CanaryCompileSpec {
        root_path: source_dir.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        product: CanaryCompileProduct::Check,
    })
    .expect("check-only compilation must not require or infer a runtime entry");

    assert!(!report.wrote_output());
    assert!(build_dir.join("05_capability_manifest.json").is_file());
    assert!(build_dir.join("05_machine_contracts.json").is_file());
    let _ = fs::remove_dir_all(scratch);
}

#[test]
fn migrated_main_entries_are_selected_only_through_their_target_root_bindings() {
    for &(canary_name, target) in fixture_roster::MIGRATED_ENTRY_PASS_CANARIES {
        let canary = pass_canary(canary_name);
        let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&canary.join("main.omg"), Some(target)))
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
fn catalog_checked_assembly_is_validated_against_final_image_bytes() {
    let canary = pass_canary(fixture_roster::INLINE_ASM_ASM_FENCES_COMPILE);
    let build_dir =
        std::env::temp_dir().join(format!("omega-final-asm-evidence-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CanaryCompileSpec {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("linux_x86_64".into()),
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("fixed checked assembly should cross-compile with final-byte evidence");

    let executable_regions = fs::read_to_string(build_dir.join("13_executable_regions.json"))
        .expect("final executable-region inventory should be written");
    assert!(
        executable_regions.contains("\"checked_instruction_validation_count\": 3")
            && executable_regions
                .contains("\"checked_instruction_validation_report_fingerprint\": \"0x")
            && executable_regions
                .contains("\"checked_instruction_footprint_report_fingerprint\": \"0x")
            && executable_regions.contains("\"catalog_checked_assembly\"")
            && executable_regions.contains("\"enumeration_complete\": true")
            && executable_regions.contains("\"missing_classes\": []"),
        "final image evidence should cover all three fixed fence instructions with complete body validation:\n{executable_regions}"
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn immediate_port_io_is_bound_in_final_image_validation() {
    let canary = pass_canary(fixture_roster::INLINE_ASM_ASM_PORT_OUT_FINAL_VALIDATION);
    let build_dir =
        std::env::temp_dir().join(format!("omega-final-port-evidence-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CanaryCompileSpec {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("linux_x86_64".into()),
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("immediate-port checked assembly should emit final-byte evidence");

    let executable_regions = fs::read_to_string(build_dir.join("13_executable_regions.json"))
        .expect("final executable-region inventory should be written");
    assert!(
        executable_regions.contains("\"checked_instruction_validation_count\": 1")
            && executable_regions
                .contains("\"checked_instruction_validation_report_fingerprint\": \"0x")
            && executable_regions
                .contains("\"checked_instruction_footprint_report_fingerprint\": \"0x"),
        "final image evidence should bind the immediate port instruction:\n{executable_regions}"
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn structured_machine_control_envelopes_are_bound_in_final_image_validation() {
    for &(canary_name, expected_count) in fixture_roster::MACHINE_CONTROL_PASS_CANARIES {
        let canary = pass_canary(canary_name);
        let build_dir = std::env::temp_dir().join(format!(
            "omega-final-machine-control-evidence-{}-{}",
            std::process::id(),
            expected_count
        ));
        let _ = fs::remove_dir_all(&build_dir);

        compile(CanaryCompileSpec {
            root_path: canary.join("main.omg"),
            build_dir: Some(build_dir.clone()),
            target_name: Some("linux_x86_64".into()),
            product: CanaryCompileProduct::NativeArtifactAndPublish,
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

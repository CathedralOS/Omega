use crate::tests::fixtures::checked_source::checked;
use crate::{NativeProgramEntrySettlement, NativeRealizationRequest};

const RECEIVER_STORE: &str = r#"
    data Main { value: i32; }
    machine Main::launch(&mut self) {
        self.value = 17;
    }
"#;

fn entry_fixture(
    source_text: &str,
    receiver: program_entry_plan::ProgramEntrySourceReceiverSignature,
    target_profile: target::TargetProfile,
) -> (
    terminal_production::ProducedProgramEntryTerminalArtifact,
    program_entry_plan::SelectedProgramEntrySourceSignature,
) {
    let checked = checked(source_text);
    let selection = checked
        .facts
        .flow
        .terminal_machines
        .machines
        .iter()
        .find(|machine| machine.name == "Main::launch")
        .expect("source-selected entry");
    let signature =
        program_entry_plan::SelectedProgramEntrySourceSignature::from_checked_typed_entry(
            target_profile.program_entry_slot(),
            selection.machine,
            selection.machine,
            selection.name.clone(),
            "entry".into(),
            if matches!(
                receiver,
                program_entry_plan::ProgramEntrySourceReceiverSignature::Free
            ) {
                "test::Main::launch() -> Unit"
            } else {
                "test::Main::launch(&mut self) -> Unit"
            }
            .into(),
            receiver,
            Vec::new(),
        )
        .expect("selected source signature");
    let produced = terminal_production::produce_program_entry_terminal_artifact(
        &checked,
        "Main::launch",
        signature.identity().bytes(),
    )
    .expect("source receiver store produces a receipt-coupled Terminal artifact");
    (produced, signature)
}

fn request<'request>(
    signature: &'request program_entry_plan::SelectedProgramEntrySourceSignature,
    profile: &'request proof_admission::AdmissionProfile,
    optimizations: &'request optimization_core::PostTerminalOptimizationSelections,
    providers: &'request effects::SelectedProviderPlanFacts,
) -> NativeRealizationRequest<'request> {
    NativeRealizationRequest {
        target: signature.target_slot().owner.native_target(),
        subsystem: 3,
        profile,
        terminal_authority_policy: crate::current_compiler_intrinsic_terminal_authority_policy(),
        terminal_authority_permission_policy: crate::current_terminal_authority_permission_policy(),
        program_entry: NativeProgramEntrySettlement::new(signature, None, &[]),
        optimization_selections: optimizations,
        selected_provider_plans: providers,
        external_binding_rows: &[],
        settlements: &[],
        compiler_builtins: &[],
        boundary_application_coverage: None,
        ieee_float_fma: &[],
        native_callbacks: &[],
        callback_thunks: &[],
    }
}

#[test]
fn unprovisioned_receiver_entry_rejects_fresh_and_prepared_executable_realization() {
    let profile = proof_admission::AdmissionProfile::default();
    let optimizations = optimization_core::PostTerminalOptimizationSelections::default();
    let providers = effects::SelectedProviderPlanFacts::default();
    for target_profile in [
        target::TargetProfile::LinuxX64,
        target::TargetProfile::LinuxArm64,
    ] {
        let (produced, signature) = entry_fixture(
            RECEIVER_STORE,
            program_entry_plan::ProgramEntrySourceReceiverSignature::ProvisionedMutable {
                normalized_type_identity: "test::Main".into(),
            },
            target_profile,
        );
        let (artifact, receipt, scope, _) = produced.into_parts();
        crate::validate_native_program_entry_settlement(
            &artifact,
            &receipt,
            NativeProgramEntrySettlement::new(&signature, None, &[]),
            target_profile.native_target(),
        )
        .expect(
            "source-entry declaration settlement remains valid without executable provisioning",
        );
        let prepared = crate::prepare_native_realization_input(&artifact, &profile, &optimizations)
            .expect("verified callable input remains preparable");
        let fresh = crate::realize_native_artifact_with_checked_boundary_operator_scope(
            terminal_codec::CanonicalTerminalArtifact::from_bytes(&artifact.to_bytes())
                .expect("replay the same canonical artifact for fresh realization"),
            &scope,
            request(&signature, &profile, &optimizations, &providers),
        )
        .expect_err("direct executable must not use an unprovisioned receiver pointer");
        let reopened =
            crate::realize_native_artifact_with_checked_boundary_operator_scope_and_prepared_input(
                artifact,
                &scope,
                request(&signature, &profile, &optimizations, &providers),
                &prepared,
            )
            .expect_err("prepared input must not bypass executable receiver provisioning");
        for diagnostics in [fresh, reopened] {
            let [diagnostic] = diagnostics.as_slice() else {
                panic!("one explicit missing provisioning diagnostic")
            };
            assert!(
                diagnostic
                    .message
                    .contains("ProgramEntry receiver provisioning"),
                "unexpected diagnostic: {}",
                diagnostic.message
            );
            assert!(
                diagnostic
                    .message
                    .contains("no root-backed bridge constructs and lends its receiver")
            );
        }
    }
}

#[test]
fn receiver_store_remains_callable_through_machine_and_object_custody() {
    assert_callable_receiver_store(RECEIVER_STORE, 0);
}

#[test]
fn indexed_receiver_store_remains_callable_through_machine_and_object_custody() {
    assert_callable_receiver_store(
        r#"
        data Cell [copy] { value: i32; }
        data Main { cells: [Cell; 3]; }
        machine Main::launch(&mut self) { self.cells[1].value = 17; }
        "#,
        4,
    );
}

fn assert_callable_receiver_store(source: &str, field_byte_offset: u32) {
    let checked = checked(source);
    let artifact = terminal_production::produce_terminal_artifact(&checked, "Main::launch")
        .expect("source receiver store reaches canonical Terminal");
    let plan = terminal_psi_to_abstract_operations::lower_artifact_sections(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("verified receiver store remains a callable abstract function");
    let optimization_input =
        terminal_psi_to_abstract_operations::lower_artifact_sections_for_optimization(
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            &proof_admission::AdmissionProfile::default(),
        )
        .unwrap();
    let optimizations = optimization_core::PostTerminalOptimizationSelections::default();
    crate::optimize_verified_abstract_input(
        optimization_input,
        crate::compiler_baseline_request_v1(optimizations.selections()),
    )
    .expect("the ordinary optimization pipeline retains the same callable receiver store");
    let entry = plan
        .functions
        .iter()
        .find(|function| function.machine == plan.entry)
        .unwrap();
    let [receiver] = entry.structural_parameters.as_slice() else {
        panic!("one actual source receiver")
    };
    assert!(receiver.is_self);
    assert_eq!(receiver.position, 0);
    assert_eq!(
        receiver.access,
        terminal_psi::StructuralAccess::MutableBorrow
    );
    for target in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
    ] {
        let target_plan =
            abstract_operations_to_target_operations::lower_to_target_operations(&plan, target)
                .expect("callable target lowering accepts the receiver");
        let assigned =
            target_operations_to_assigned_target_operations::assign_registers(&target_plan)
                .expect("callable receiver store reaches physical assignment");
        let emitted = machine_emission::emit_machine_code(&assigned)
            .expect("callable receiver store reaches machine code");
        let function = emitted
            .functions
            .iter()
            .find(|function| function.machine == emitted.entry)
            .unwrap();
        let [store] = function.unit_structural_scalar_field_stores.as_slice() else {
            panic!("one receiver store survives emission")
        };
        assert_eq!(&store.destination, receiver);
        assert!(store.parameter_home_indirect);
        assert_eq!(store.field_byte_offset, field_byte_offset);
        image_emission::build_object_artifact(&emitted)
            .expect("callable object replay accepts the receiver store");
    }
}

#[test]
fn namespace_attachment_without_receiver_still_realizes_an_executable() {
    let (produced, signature) = entry_fixture(
        "data Main {} machine Main::launch() {}",
        program_entry_plan::ProgramEntrySourceReceiverSignature::Free,
        target::TargetProfile::WindowsX64,
    );
    let module = terminal_codec::decode_module(produced.artifact().semantic_bytes()).unwrap();
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    assert!(entry.attachment.is_some());
    assert!(entry.structural_parameters.is_empty());
    let profile = proof_admission::AdmissionProfile::default();
    let optimizations = optimization_core::PostTerminalOptimizationSelections::default();
    let providers = effects::SelectedProviderPlanFacts::default();
    let native = crate::realize_program_entry_native_artifact(
        produced,
        request(&signature, &profile, &optimizations, &providers),
    )
    .expect("namespace attachment does not require receiver provisioning");
    native
        .artifact()
        .validate()
        .expect("namespace-only executable replays");
}

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
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.symbol == selection.machine)
        .expect("checked source entry");
    let state = checked
        .machine_states(machine)
        .first()
        .expect("entry state");
    let receiver = checked
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.is_self)
        .map_or(
            program_entry_plan::ProgramEntrySourceReceiverSignature::Free,
            |parameter| {
                program_entry_plan::ProgramEntrySourceReceiverSignature::ProvisionedMutable {
                    normalized_type_identity: checked
                        .normalized_type_identity(parameter.type_reference)
                        .into_string(),
                }
            },
        );
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
    let produced = terminal_production::TerminalProductionRequest::new(&checked, "Main::launch")
        .produce_program_entry(signature.identity().bytes())
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
        checked_scope: None,
        prepared_input: None,
        target: signature.target_slot().owner.native_target(),
        image_request: image_emission::ExecutableImageEmissionRequest::direct(3),
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
fn executable_entry_rejects_lost_source_receiver_projection() {
    let (produced, signature) = entry_fixture(RECEIVER_STORE, target::TargetProfile::MacosArm64);
    let profile = proof_admission::AdmissionProfile::default();
    let artifact = produced.artifact();
    let input =
        super::lower_realization_input(artifact.semantic_bytes(), artifact.proof_bytes(), &profile)
            .expect("checked receiver input");
    let mut plan = input.plan().clone();
    let entry = plan
        .functions
        .iter_mut()
        .find(|function| function.machine == plan.entry)
        .expect("selected entry");
    assert!(
        entry
            .structural_parameters
            .iter()
            .any(|parameter| parameter.is_self)
    );
    for parameter in &mut entry.structural_parameters {
        parameter.is_self = false;
    }
    let diagnostics = super::validate_executable_entry_receiver(&plan, &signature)
        .expect_err("lowering cannot change the source-selected receiver mode");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("source-selected receiver mode"))
    );
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
        let (produced, signature) = entry_fixture(RECEIVER_STORE, target_profile);
        let (artifact, receipt, scope, _, _) = produced.into_parts();
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
        let fresh = crate::realize_native_artifact(
            terminal_codec::CanonicalTerminalArtifact::from_bytes(&artifact.to_bytes())
                .expect("replay the same canonical artifact for fresh realization"),
            NativeRealizationRequest {
                checked_scope: Some(&scope),
                prepared_input: None,
                ..request(&signature, &profile, &optimizations, &providers)
            },
        )
        .expect_err("direct executable must not use an unprovisioned receiver pointer");
        let reopened = crate::realize_native_artifact(
            artifact,
            NativeRealizationRequest {
                checked_scope: Some(&scope),
                prepared_input: Some(&prepared),
                ..request(&signature, &profile, &optimizations, &providers)
            },
        )
        .expect_err("prepared input must not bypass executable receiver provisioning");
        for diagnostics in [fresh, reopened] {
            let [diagnostic] = diagnostics.diagnostics() else {
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
fn namespace_attachment_without_receiver_still_realizes_an_executable() {
    let (produced, signature) = entry_fixture(
        "data Main {} machine Main::launch() {}",
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
        .as_direct()
        .expect("direct image requested")
        .validate()
        .expect("namespace-only executable replays");
}

#[test]
fn native_request_scope_and_reuse_preserve_direct_image_bytes() {
    let (produced, signature) = entry_fixture(
        "data Main {} machine Main::launch() {}",
        target::TargetProfile::WindowsX64,
    );
    let (artifact, _, scope, _, _) = produced.into_parts();
    let profile = proof_admission::AdmissionProfile::default();
    let optimizations = optimization_core::PostTerminalOptimizationSelections::default();
    let providers = effects::SelectedProviderPlanFacts::default();
    let prepared =
        crate::prepare_native_realization_input(&artifact, &profile, &optimizations).unwrap();
    let mut expected_bytes = None;
    for checked_scope in [None, Some(&scope)] {
        for prepared_input in [None, Some(&prepared)] {
            let mut request = request(&signature, &profile, &optimizations, &providers);
            request.checked_scope = checked_scope;
            request.prepared_input = prepared_input;
            let native = crate::realize_native_artifact(
                terminal_codec::CanonicalTerminalArtifact::from_bytes(&artifact.to_bytes())
                    .unwrap(),
                request,
            )
            .unwrap()
            .into_direct()
            .expect("direct image request");
            native.validate().unwrap();
            let bytes = &native.image().output().bytes;
            if let Some(expected) = &expected_bytes {
                assert_eq!(expected, bytes);
            } else {
                expected_bytes = Some(bytes.clone());
            }
        }
    }
}

#[test]
fn native_request_rejects_substituted_scope_or_prepared_input_and_returns_image_request() {
    let (produced, signature) = entry_fixture(
        "data Main {} machine Main::launch() {}",
        target::TargetProfile::WindowsX64,
    );
    let (artifact, _, scope, _, _) = produced.into_parts();
    let (other, _) = entry_fixture(
        "data Main {} machine Main::launch() { Main::work(); } machine Main::work() {}",
        target::TargetProfile::WindowsX64,
    );
    let (other_artifact, _, other_scope, _, _) = other.into_parts();
    assert_ne!(
        artifact.manifest().identity(),
        other_artifact.manifest().identity()
    );
    let profile = proof_admission::AdmissionProfile::default();
    let optimizations = optimization_core::PostTerminalOptimizationSelections::default();
    let providers = effects::SelectedProviderPlanFacts::default();
    let other_prepared =
        crate::prepare_native_realization_input(&other_artifact, &profile, &optimizations).unwrap();
    for (checked_scope, prepared_input, expected) in [
        (Some(&other_scope), None, "checked boundary-operator scope"),
        (Some(&scope), Some(&other_prepared), "prepared native input"),
    ] {
        let mut request = request(&signature, &profile, &optimizations, &providers);
        request.checked_scope = checked_scope;
        request.prepared_input = prepared_input;
        request.image_request = image_emission::ExecutableImageEmissionRequest::direct(19);
        let error = crate::realize_native_artifact(
            terminal_codec::CanonicalTerminalArtifact::from_bytes(&artifact.to_bytes()).unwrap(),
            request,
        )
        .expect_err("substituted evidence cannot select another realization route");
        let (image_request, diagnostics) = error.into_parts();
        assert!(matches!(
            image_request,
            image_emission::ExecutableImageEmissionRequest::Direct { subsystem: 19 }
        ));
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(expected)),
            "{diagnostics:?}"
        );
    }
}

#[test]
fn program_entry_adapter_does_not_ignore_supplied_scope() {
    let (produced, signature) = entry_fixture(
        "data Main {} machine Main::launch() {}",
        target::TargetProfile::WindowsX64,
    );
    let (other, _) = entry_fixture(
        "data Main {} machine Main::launch() { Main::work(); } machine Main::work() {}",
        target::TargetProfile::WindowsX64,
    );
    let (_, _, other_scope, _, _) = other.into_parts();
    let profile = proof_admission::AdmissionProfile::default();
    let optimizations = optimization_core::PostTerminalOptimizationSelections::default();
    let providers = effects::SelectedProviderPlanFacts::default();
    let mut request = request(&signature, &profile, &optimizations, &providers);
    request.checked_scope = Some(&other_scope);
    let error = crate::realize_program_entry_native_artifact(produced, request)
        .expect_err("owned entry custody cannot hide substituted request custody");
    let (image, diagnostics) = error.into_parts();
    assert!(matches!(
        image,
        image_emission::ExecutableImageEmissionRequest::Direct { subsystem: 3 }
    ));
    assert!(
        diagnostics[0]
            .message
            .contains("checked boundary-operator scope")
    );
}

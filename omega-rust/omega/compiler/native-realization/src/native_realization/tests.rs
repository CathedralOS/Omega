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
    Option<build_evaluation::SelectedProgramEntryCallingPlans>,
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
    // Slots that declare their two-surface calling custody need the evaluated
    // paired plans; a hosted free entry supplies them for the profile.
    let slot = signature.target_slot();
    let plans = (matches!(slot.schema, target::ProgramEntrySchema::HostedApplication)
        && slot.boundary_schema.is_some())
    .then(|| crate::tests::fixtures::hosted::hosted_calling_plans(target_profile));
    (produced, signature, plans)
}

fn request<'request>(
    signature: &'request program_entry_plan::SelectedProgramEntrySourceSignature,
    plans: Option<&'request build_evaluation::SelectedProgramEntryCallingPlans>,
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
        terminal_authority_permission_policy: Some(
            crate::current_terminal_authority_permission_policy(),
        ),
        program_entry: NativeProgramEntrySettlement::new(
            signature,
            plans.map(crate::tests::fixtures::hosted::paired_calling_plan_parts),
            &[],
        ),
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
    let (produced, signature, plans) =
        entry_fixture(RECEIVER_STORE, target::TargetProfile::MacosArm64);
    let profile = proof_admission::AdmissionProfile::default();
    let optimizations = optimization_core::PostTerminalOptimizationSelections::default();
    let providers = effects::SelectedProviderPlanFacts::default();
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
    let diagnostics = super::validate_executable_entry_receiver(
        &plan,
        input.context().module(),
        artifact,
        &request(
            &signature,
            plans.as_ref(),
            &profile,
            &optimizations,
            &providers,
        ),
    )
    .expect_err("lowering cannot change the source-selected receiver mode");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("source-selected receiver mode"))
    );
}

const ERASED_RECEIVER: &str = "data Main {} machine Main::launch(&mut self) {}";

#[test]
fn erased_receiver_cannot_discard_nominal_cleanup() {
    let (produced, signature, plans) = entry_fixture(
        "data Helper {}
         machine Helper::finish() {}
         data Main { value: i32; }
         machine Main::drop(&mut self) { Helper::finish(); }
         machine Main::launch(&mut self) {}",
        target::TargetProfile::WindowsX64,
    );
    let module = terminal_codec::decode_module(produced.artifact().semantic_bytes()).unwrap();
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    assert!(
        entry
            .structural_parameters
            .iter()
            .all(|parameter| !parameter.is_self)
    );
    let profile = proof_admission::AdmissionProfile::default();
    let optimizations = optimization_core::PostTerminalOptimizationSelections::default();
    let providers = effects::SelectedProviderPlanFacts::default();
    let error = crate::realize_program_entry_native_artifact(
        produced,
        request(
            &signature,
            plans.as_ref(),
            &profile,
            &optimizations,
            &providers,
        ),
    )
    .expect_err("erasing an unused borrow cannot erase the provisioned owner's cleanup");
    assert!(
        error
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.message.contains("receiver"))
    );
}

#[test]
fn erased_receiver_eligibility_is_required_for_fresh_and_prepared_inputs() {
    let profile = proof_admission::AdmissionProfile::default();
    let optimizations = optimization_core::PostTerminalOptimizationSelections::default();
    let providers = effects::SelectedProviderPlanFacts::default();
    for source in [
        "data Main { value: i32 [1..=9]; } machine Main::launch(&mut self) {}",
        "data Child {} machine Child::drop(&mut self) {} data Main { child: Child; } machine Main::launch(&mut self) {}",
    ] {
        let (produced, signature, plans) = entry_fixture(source, target::TargetProfile::WindowsX64);
        let (artifact, receipt, scope, _, _, _) = produced.into_parts();
        assert!(receipt.receiver_eligibility().is_none());
        let prepared = crate::prepare_native_realization_input(&artifact, &profile, &optimizations)
            .expect("callable preparation does not provision an entry receiver");
        for prepared_input in [None, Some(&prepared)] {
            let result = crate::realize_native_artifact(
                terminal_codec::CanonicalTerminalArtifact::from_bytes(&artifact.to_bytes())
                    .unwrap(),
                NativeRealizationRequest {
                    checked_scope: Some(&scope),
                    prepared_input,
                    program_entry: NativeProgramEntrySettlement::new(
                        &signature,
                        plans
                            .as_ref()
                            .map(crate::tests::fixtures::hosted::paired_calling_plan_parts),
                        &[],
                    )
                    .with_checked_entry(&receipt),
                    ..request(
                        &signature,
                        plans.as_ref(),
                        &profile,
                        &optimizations,
                        &providers,
                    )
                },
            );
            let Err(error) = result else {
                panic!(
                    "an erased entry still requires source initialization and cleanup eligibility"
                );
            };
            assert!(error.diagnostics().iter().any(|diagnostic| {
                diagnostic
                    .message
                    .contains("checked ZII-valid value with no executable nominal cleanup")
            }));
        }
    }
}

#[test]
fn provisioned_receiver_erased_before_realization_still_realizes_an_executable() {
    let (produced, signature, plans) =
        entry_fixture(ERASED_RECEIVER, target::TargetProfile::WindowsX64);
    let module = terminal_codec::decode_module(produced.artifact().semantic_bytes()).unwrap();
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    // Checked production erases the unused `&mut self` place entirely; the
    // attached type survives so checked source eligibility still has its owner.
    assert!(
        entry
            .structural_parameters
            .iter()
            .all(|parameter| !parameter.is_self)
    );
    assert!(entry.attachment.is_some());
    let profile = proof_admission::AdmissionProfile::default();
    let optimizations = optimization_core::PostTerminalOptimizationSelections::default();
    let providers = effects::SelectedProviderPlanFacts::default();
    let native = crate::realize_program_entry_native_artifact(
        produced,
        request(
            &signature,
            plans.as_ref(),
            &profile,
            &optimizations,
            &providers,
        ),
    )
    .expect("checked erasure preserves the source receiver mode without storage");
    native
        .artifact()
        .as_direct()
        .expect("direct image requested")
        .validate()
        .expect("erased-receiver executable replays");
}

#[test]
fn erased_provisioned_receiver_must_retain_its_attached_type() {
    let (produced, signature, plans) =
        entry_fixture(ERASED_RECEIVER, target::TargetProfile::WindowsX64);
    let profile = proof_admission::AdmissionProfile::default();
    let optimizations = optimization_core::PostTerminalOptimizationSelections::default();
    let providers = effects::SelectedProviderPlanFacts::default();
    let input = super::lower_realization_input(
        produced.artifact().semantic_bytes(),
        produced.artifact().proof_bytes(),
        &profile,
    )
    .expect("checked erased-receiver input");
    let mut plan = input.plan().clone();
    for function in &mut plan.functions {
        if function.machine == plan.entry {
            function.attachment = None;
        }
    }
    let diagnostics = super::validate_executable_entry_receiver(
        &plan,
        input.context().module(),
        produced.artifact(),
        &request(
            &signature,
            plans.as_ref(),
            &profile,
            &optimizations,
            &providers,
        ),
    )
    .expect_err("an erased provisioned receiver cannot lose its checked attachment owner");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("source-selected receiver mode"))
    );
}

#[test]
fn retained_receiver_entry_must_preserve_its_checked_receiver_identity() {
    let (produced, signature, plans) =
        entry_fixture(RECEIVER_STORE, target::TargetProfile::MacosArm64);
    let (artifact, receipt, _, _, _, _) = produced.into_parts();
    let profile = proof_admission::AdmissionProfile::default();
    let optimizations = optimization_core::PostTerminalOptimizationSelections::default();
    let providers = effects::SelectedProviderPlanFacts::default();
    let input =
        super::lower_realization_input(artifact.semantic_bytes(), artifact.proof_bytes(), &profile)
            .expect("checked receiver input");
    let request = NativeRealizationRequest {
        program_entry: NativeProgramEntrySettlement::new(
            &signature,
            plans
                .as_ref()
                .map(crate::tests::fixtures::hosted::paired_calling_plan_parts),
            &[],
        )
        .with_checked_entry(&receipt),
        ..request(
            &signature,
            plans.as_ref(),
            &profile,
            &optimizations,
            &providers,
        )
    };
    // The unmutated retained receiver clears the identity join: the paired
    // calling plans settle first, so a later rejection is a separate
    // settlement requirement, never a receiver-identity drift.
    if let Err(diagnostics) = super::validate_executable_entry_receiver(
        input.plan(),
        input.context().module(),
        &artifact,
        &request,
    ) {
        assert!(
            diagnostics.iter().all(|diagnostic| {
                !diagnostic
                    .message
                    .contains("source-selected receiver identity")
            }),
            "unexpected diagnostics: {diagnostics:?}"
        );
    }
    // The bridge sizes and lends the storage this lowered declaration spells
    // out, and no later stage rejoins it to the checked Terminal receiver: a
    // drifted owner attachment or self declaration must reject at admission.
    let drifts: [fn(&mut abstract_operations::AbstractFunction); 4] = [
        |function| function.attachment = None,
        |function| {
            function.attachment =
                Some(semantic_vocabulary::StructuralTypeId::new(u64::MAX).unwrap());
        },
        |function| {
            function
                .structural_parameters
                .iter_mut()
                .find(|parameter| parameter.is_self)
                .expect("retained receiver")
                .access = terminal_psi::StructuralAccess::SharedBorrow;
        },
        |function| {
            function
                .structural_parameters
                .iter_mut()
                .find(|parameter| parameter.is_self)
                .expect("retained receiver")
                .structural_type = semantic_vocabulary::StructuralTypeId::new(u64::MAX).unwrap();
        },
    ];
    for drift in drifts {
        let mut plan = input.plan().clone();
        drift(
            plan.functions
                .iter_mut()
                .find(|function| function.machine == plan.entry)
                .expect("selected entry"),
        );
        let diagnostics = super::validate_executable_entry_receiver(
            &plan,
            input.context().module(),
            &artifact,
            &request,
        )
        .expect_err("a drifted receiver identity must reject");
        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic
                    .message
                    .contains("source-selected receiver identity")
            }),
            "unexpected diagnostics: {diagnostics:?}"
        );
    }
}

#[test]
fn free_source_entry_cannot_acquire_a_receiver() {
    let (produced, signature, plans) =
        entry_fixture(RECEIVER_STORE, target::TargetProfile::MacosArm64);
    let artifact = produced.artifact();
    let profile = proof_admission::AdmissionProfile::default();
    let optimizations = optimization_core::PostTerminalOptimizationSelections::default();
    let providers = effects::SelectedProviderPlanFacts::default();
    let input =
        super::lower_realization_input(artifact.semantic_bytes(), artifact.proof_bytes(), &profile)
            .expect("checked receiver input");
    let free_signature =
        program_entry_plan::SelectedProgramEntrySourceSignature::from_checked_typed_entry(
            signature.target_slot(),
            signature.machine_symbol(),
            signature.state_symbol(),
            signature.machine_name().into(),
            signature.state_name().into(),
            signature.normalized_callable_identity().into(),
            program_entry_plan::ProgramEntrySourceReceiverSignature::Free,
            signature.visible_parameters().to_vec(),
        )
        .expect("free source signature");
    // Both frontiers retain the receiver param; the source mode alone forbids it.
    let diagnostics = super::validate_executable_entry_receiver(
        input.plan(),
        input.context().module(),
        artifact,
        &request(
            &free_signature,
            plans.as_ref(),
            &profile,
            &optimizations,
            &providers,
        ),
    )
    .expect_err("a free source entry cannot acquire a receiver through lowering");
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
        target::TargetProfile::WindowsX64,
        target::TargetProfile::UefiX64,
    ] {
        let (produced, signature, plans) = entry_fixture(RECEIVER_STORE, target_profile);
        let (artifact, receipt, scope, _, _, _) = produced.into_parts();
        let settlement = crate::validate_native_program_entry_settlement(
            &artifact,
            &receipt,
            NativeProgramEntrySettlement::new(
                &signature,
                plans
                    .as_ref()
                    .map(crate::tests::fixtures::hosted::paired_calling_plan_parts),
                &[],
            ),
            target_profile.native_target(),
        );
        if matches!(target_profile, target::TargetProfile::UefiX64) {
            // The freestanding UEFI slot declares its two-surface contract but
            // its authored storage roots cannot come from this hosted fixture,
            // so a declaration-only settlement still fails closed before
            // receiver provisioning is examined.
            assert!(matches!(
                settlement,
                Err(crate::NativeProgramEntrySettlementError::CallingPlanPairingDrift)
            ));
            continue;
        }
        settlement.expect(
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
                ..request(
                    &signature,
                    plans.as_ref(),
                    &profile,
                    &optimizations,
                    &providers,
                )
            },
        )
        .expect_err("direct executable must not use an unprovisioned receiver pointer");
        let reopened = crate::realize_native_artifact(
            artifact,
            NativeRealizationRequest {
                checked_scope: Some(&scope),
                prepared_input: Some(&prepared),
                ..request(
                    &signature,
                    plans.as_ref(),
                    &profile,
                    &optimizations,
                    &providers,
                )
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
            // The hosted bridges exist; a declaration-only settlement without
            // the checked entry custody cannot authorize receiver storage.
            assert!(
                diagnostic
                    .message
                    .contains("requires exact checked initialization and cleanup custody"),
                "unexpected diagnostic: {}",
                diagnostic.message
            );
        }
    }
}

#[test]
fn admitted_receiver_provisioning_must_reach_the_emitted_object() {
    let (produced, signature, plans) =
        entry_fixture(RECEIVER_STORE, target::TargetProfile::MacosArm64);
    let (artifact, receipt, scope, _, _, _) = produced.into_parts();
    let profile = proof_admission::AdmissionProfile::default();
    let optimizations = optimization_core::PostTerminalOptimizationSelections::default();
    let providers = effects::SelectedProviderPlanFacts::default();
    let request = NativeRealizationRequest {
        checked_scope: Some(&scope),
        program_entry: NativeProgramEntrySettlement::new(
            &signature,
            plans
                .as_ref()
                .map(crate::tests::fixtures::hosted::paired_calling_plan_parts),
            &[],
        )
        .with_checked_entry(&receipt),
        ..request(
            &signature,
            plans.as_ref(),
            &profile,
            &optimizations,
            &providers,
        )
    };
    // Emitting without the admitted settlement is the bypass shape: the entry
    // code keeps its self parameter while no bridge constructs its receiver.
    let input =
        super::lower_realization_input(artifact.semantic_bytes(), artifact.proof_bytes(), &profile)
            .expect("checked receiver input");
    let admitted_providers = super::providers::admit_native_providers(
        &input,
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        *artifact.manifest().identity().as_bytes(),
        &request,
    )
    .expect("provider admission");
    let emitted = super::emit_realization_object(
        input,
        admitted_providers.installation,
        &admitted_providers.settlements,
        None,
        None,
        &request,
    )
    .expect("emission alone never provisions the receiver");
    assert!(emitted.object.hosted_receiver_binding().is_none());
    // The bypass verdict needs only the admitted custody, not a complete
    // paired-contract settlement: the binding is already absent.
    let native_target = request.target;
    let settlement = crate::ValidatedNativeProgramEntrySettlement {
        checked_entry: receipt,
        target: native_target,
        source: signature,
        semantic_calling_application: None,
        physical_calling_application: None,
        storage_entry: None,
        fused_service_establishments: Vec::new(),
    };
    let diagnostics = super::validate_emitted_receiver_binding(&emitted.object, Some(&settlement))
        .expect_err("an admitted receiver that never reached the object must reject");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("did not reach the emitted object")),
        "unexpected diagnostics: {diagnostics:?}"
    );
    super::validate_emitted_receiver_binding(&emitted.object, None)
        .expect("no admitted receiver and no binding stays consistent");
}

#[test]
fn emitted_receiver_binding_rejects_unadmitted_and_substituted_identities() {
    let (produced, signature, plans) =
        entry_fixture(RECEIVER_STORE, target::TargetProfile::LinuxX64);
    let (artifact, receipt, scope, _, _, _) = produced.into_parts();
    let profile = proof_admission::AdmissionProfile::default();
    let optimizations = optimization_core::PostTerminalOptimizationSelections::default();
    let providers = effects::SelectedProviderPlanFacts::default();
    let request = NativeRealizationRequest {
        checked_scope: Some(&scope),
        program_entry: NativeProgramEntrySettlement::new(
            &signature,
            plans
                .as_ref()
                .map(crate::tests::fixtures::hosted::paired_calling_plan_parts),
            &[],
        )
        .with_checked_entry(&receipt),
        ..request(
            &signature,
            plans.as_ref(),
            &profile,
            &optimizations,
            &providers,
        )
    };
    let input =
        super::lower_realization_input(artifact.semantic_bytes(), artifact.proof_bytes(), &profile)
            .expect("checked receiver input");
    let admitted_providers = super::providers::admit_native_providers(
        &input,
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        *artifact.manifest().identity().as_bytes(),
        &request,
    )
    .expect("provider admission");
    let emitted = super::emit_realization_object(
        input,
        admitted_providers.installation,
        &admitted_providers.settlements,
        None,
        None,
        &request,
    )
    .expect("emission alone never provisions the receiver");
    let mut object = emitted.object;
    assert!(object.hosted_receiver_binding().is_none());

    // Bind the exact Linux x86-64 bridge the way the admitted emission stage
    // does once provisioning is granted: same source signature, same closed
    // target-package contract, same derived stack demand.
    let demand = image_emission::derive_stack_demand(&object, object.entry())
        .expect("emitted entry stack demand");
    let calling_plan = program_entry_plan::exact_linux_x86_64_physical_boundary_entry_plan();
    let physical = program_entry_plan::ProgramEntryPhysicalContractPlan::new(
        target::TargetProfile::LinuxX64.program_entry_slot(),
        program_entry_plan::LINUX_X86_64_PHYSICAL_REQUIREMENT_IDENTITY.into(),
        target::ProgramEntryPhysicalContractPackage::LinuxX86_64,
        program_entry_plan::exact_linux_x86_64_physical_contract_package_source_digest(),
        // The package-source report fingerprint is retained for diagnostics
        // only; contract custody replays the strong source digest instead.
        0,
        vec![program_entry_plan::LINUX_X86_64_ADDRESS_TYPE_IDENTITY.into()],
        program_entry_plan::LINUX_X86_64_I32_TYPE_IDENTITY.into(),
        calling_plan.contract_report_fingerprint(),
        calling_plan.plan().clone(),
    )
    .expect("exact Linux x86-64 physical contract");
    image_emission::bind_hosted_receiver(&mut object, &signature, &physical, &[], &demand)
        .expect("the exact bridge binds the emitted object");
    assert!(object.hosted_receiver_binding().is_some());

    // Bypassed provisioning: an emitted binding no admission ever granted.
    let diagnostics = super::validate_emitted_receiver_binding(&object, None)
        .expect_err("an emitted binding without admission must reject");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("admission never granted")),
        "unexpected diagnostics: {diagnostics:?}"
    );

    // Redirected contract identity: an admitted settlement that lost its
    // physical-contract custody cannot be satisfied by the emitted binding.
    let settlement = crate::ValidatedNativeProgramEntrySettlement {
        checked_entry: receipt.clone(),
        target: request.target,
        source: signature.clone(),
        semantic_calling_application: None,
        physical_calling_application: None,
        storage_entry: None,
        fused_service_establishments: Vec::new(),
    };
    let diagnostics = super::validate_emitted_receiver_binding(&object, Some(&settlement))
        .expect_err("a settlement without the emitted contract must reject");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("does not carry the admitted source signature and physical contract")),
        "unexpected diagnostics: {diagnostics:?}"
    );

    // Redirected receiver identity: a substituted source signature — here the
    // same entry declaration with its receiver provision removed — cannot
    // stand in for the emitted binding's exact receiver source.
    let redirected_source =
        program_entry_plan::SelectedProgramEntrySourceSignature::from_checked_typed_entry(
            signature.target_slot(),
            signature.machine_symbol(),
            signature.state_symbol(),
            signature.machine_name().into(),
            signature.state_name().into(),
            signature.normalized_callable_identity().into(),
            program_entry_plan::ProgramEntrySourceReceiverSignature::Free,
            signature.visible_parameters().to_vec(),
        )
        .expect("free declaration is valid alone");
    let settlement = crate::ValidatedNativeProgramEntrySettlement {
        source: redirected_source,
        ..settlement
    };
    let diagnostics = super::validate_emitted_receiver_binding(&object, Some(&settlement))
        .expect_err("a substituted receiver source signature must reject");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("does not carry the admitted source signature and physical contract")),
        "unexpected diagnostics: {diagnostics:?}"
    );
}

#[test]
fn namespace_attachment_without_receiver_still_realizes_an_executable() {
    let (produced, signature, plans) = entry_fixture(
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
        request(
            &signature,
            plans.as_ref(),
            &profile,
            &optimizations,
            &providers,
        ),
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
    let (produced, signature, plans) = entry_fixture(
        "data Main {} machine Main::launch() {}",
        target::TargetProfile::WindowsX64,
    );
    let (artifact, _, scope, _, _, _) = produced.into_parts();
    let profile = proof_admission::AdmissionProfile::default();
    let optimizations = optimization_core::PostTerminalOptimizationSelections::default();
    let providers = effects::SelectedProviderPlanFacts::default();
    let prepared =
        crate::prepare_native_realization_input(&artifact, &profile, &optimizations).unwrap();
    let mut expected_bytes = None;
    for checked_scope in [None, Some(&scope)] {
        for prepared_input in [None, Some(&prepared)] {
            let mut request = request(
                &signature,
                plans.as_ref(),
                &profile,
                &optimizations,
                &providers,
            );
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
    let (produced, signature, plans) = entry_fixture(
        "data Main {} machine Main::launch() {}",
        target::TargetProfile::WindowsX64,
    );
    let (artifact, _, scope, _, _, _) = produced.into_parts();
    let (other, ..) = entry_fixture(
        "data Main {} machine Main::launch() { Main::work(); } machine Main::work() {}",
        target::TargetProfile::WindowsX64,
    );
    let (other_artifact, _, other_scope, _, _, _) = other.into_parts();
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
        let mut request = request(
            &signature,
            plans.as_ref(),
            &profile,
            &optimizations,
            &providers,
        );
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
            image_emission::ExecutableImageEmissionRequest::Direct { subsystem: 19, .. }
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
    let (produced, signature, plans) = entry_fixture(
        "data Main {} machine Main::launch() {}",
        target::TargetProfile::WindowsX64,
    );
    let (other, ..) = entry_fixture(
        "data Main {} machine Main::launch() { Main::work(); } machine Main::work() {}",
        target::TargetProfile::WindowsX64,
    );
    let (_, _, other_scope, _, _, _) = other.into_parts();
    let profile = proof_admission::AdmissionProfile::default();
    let optimizations = optimization_core::PostTerminalOptimizationSelections::default();
    let providers = effects::SelectedProviderPlanFacts::default();
    let mut request = request(
        &signature,
        plans.as_ref(),
        &profile,
        &optimizations,
        &providers,
    );
    request.checked_scope = Some(&other_scope);
    let error = crate::realize_program_entry_native_artifact(produced, request)
        .expect_err("owned entry custody cannot hide substituted request custody");
    let (image, diagnostics) = error.into_parts();
    assert!(matches!(
        image,
        image_emission::ExecutableImageEmissionRequest::Direct { subsystem: 3, .. }
    ));
    assert!(
        diagnostics[0]
            .message
            .contains("checked boundary-operator scope")
    );
}

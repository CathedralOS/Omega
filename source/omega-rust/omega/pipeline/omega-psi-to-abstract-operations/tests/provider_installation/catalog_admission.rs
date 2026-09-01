use super::builders::{artifact, provider_module, selected};
use super::ids::{boundary_id, machine_id, operation_id, structural_type_id};
use omega_psi_to_abstract_operations::{
    ProviderInstallationError, admit_provider_installation,
    admit_provider_installation_for_optimization, lower_artifact_sections,
    lower_replay_artifact_sections, lower_replay_artifact_sections_for_optimization,
};
use psi_proof_admission::AdmissionProfile;
use psi_terminal::{
    Operation, OperationKind, OperationResult, ProviderSignatureParameter, StructuralMultiplicity,
};
use psi_terminal_codec::{
    build_terminal_obligation_ledger, current_terminal_trust_graph, encode_module,
    encode_terminal_obligation_ledger, semantic_fingerprint,
};
use psi_terminal_fuel::TerminalFuelMeter;
use psi_terminal_interpreter::{
    TerminalEffect, TerminalEffectHandler, TerminalEffectRejection, TerminalExecution,
    TerminalExecutionResult, TerminalExecutionStatus, TerminalInterpretError,
};
use psi_terminal_verifier::{ModuleError, validate_module};

#[test]
fn omega_installs_only_the_checked_adapter_selected_by_provider_plan_facts() {
    let module = provider_module();
    let (semantic, proof) = artifact(&module);
    let profile = AdmissionProfile::default();
    let plan = lower_artifact_sections(&semantic, &proof, &profile).expect("verified lowering");
    let trust_graph = current_terminal_trust_graph().expect("current trust graph");
    let obligation_ledger = build_terminal_obligation_ledger(&module, &trust_graph)
        .and_then(|ledger| encode_terminal_obligation_ledger(&ledger))
        .expect("canonical obligation ledger");
    assert_eq!(
        lower_replay_artifact_sections(&semantic, &obligation_ledger, &proof, &profile)
            .expect("locally replayed artifact lowering"),
        plan
    );
    let replayed_optimizer_input = lower_replay_artifact_sections_for_optimization(
        &semantic,
        &obligation_ledger,
        &proof,
        &profile,
    )
    .expect("locally replayed optimizer input");
    assert_eq!(replayed_optimizer_input.plan(), &plan);
    assert_eq!(replayed_optimizer_input.context().module(), &module);

    let mut substituted_module = module.clone();
    let OperationKind::PortWrite { value, .. } =
        &mut substituted_module.machines[1].blocks[0].operations[0].kind
    else {
        panic!("fixture provider writes a port")
    };
    *value = 67;
    let substituted_ledger = build_terminal_obligation_ledger(&substituted_module, &trust_graph)
        .and_then(|ledger| encode_terminal_obligation_ledger(&ledger))
        .expect("substituted obligation ledger");
    assert!(matches!(
        lower_replay_artifact_sections(&semantic, &substituted_ledger, &proof, &profile),
        Err(omega_psi_to_abstract_operations::ArtifactLoweringError::ObligationReplay(_))
    ));
    assert_eq!(plan.provider_candidates, module.provider_candidates);
    assert!(matches!(
        admit_provider_installation(
            &plan,
            &semantic,
            &proof,
            &profile,
            &[],
        ),
        Err(ProviderInstallationError::MissingSelectedProvider { boundary })
            if boundary == boundary_id(1)
    ));

    let selected_facts = selected("second-plan", "SecondProvider", "SecondProvider::emit");
    let installation =
        admit_provider_installation(&plan, &semantic, &proof, &profile, &selected_facts)
            .expect("Omega derives the exact selected terminal row");
    let optimized_installation = admit_provider_installation_for_optimization(
        replayed_optimizer_input.plan(),
        &semantic,
        &proof,
        &profile,
        &selected_facts,
    )
    .expect("explicit optimizer lowering replays the same selected terminal row");
    assert_eq!(installation.psi(), plan.psi);
    assert_eq!(
        installation.installed_candidates(),
        &plan.provider_candidates[1..]
    );
    assert_eq!(installation.installed_unit_calls().len(), 1);
    assert_eq!(
        optimized_installation.installed_candidates(),
        installation.installed_candidates()
    );
    assert_eq!(
        optimized_installation.installed_unit_calls(),
        installation.installed_unit_calls()
    );
    let installed_call = &installation.installed_unit_calls()[0];
    assert_eq!(installed_call.caller(), machine_id(1));
    assert_eq!(installed_call.psi_operation(), operation_id(1));
    assert_eq!(installed_call.boundary(), boundary_id(1));
    assert_eq!(installed_call.provider(), &plan.provider_candidates[1]);
    let mut execution = TerminalExecution::start_artifact_with_provider_installation(
        &semantic,
        &proof,
        &profile,
        &[],
        &[],
        installation.psi_installation(),
    )
    .expect("selected installation starts");
    assert_eq!(
        execution.resume(&mut TerminalFuelMeter::default()).unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert!(matches!(
        execution.effects(),
        [TerminalEffect::PortWrite { value: 66, .. }]
    ));

    let mut uninstalled = TerminalExecution::start_artifact(&semantic, &proof, &profile, &[])
        .expect("artifact starts without an installation");
    let mut handler = CountingEffects::default();
    assert!(matches!(
        uninstalled.resume_with_effect_handler(&mut TerminalFuelMeter::default(), &mut handler),
        Err(TerminalInterpretError::ProviderInstallationMissing(boundary))
            if boundary == boundary_id(1)
    ));
    assert_eq!(handler.calls, 0);
    assert!(uninstalled.effects().is_empty());

    let mismatched = selected("bad-plan", "FirstProvider", "SecondProvider::emit");
    assert!(matches!(
        admit_provider_installation(&plan, &semantic, &proof, &profile, &mismatched),
        Err(ProviderInstallationError::SelectedProviderMismatch { boundary })
            if boundary == boundary_id(1)
    ));
}

#[derive(Default)]
struct CountingEffects {
    calls: usize,
}

impl TerminalEffectHandler for CountingEffects {
    fn handle_effect(&mut self, _effect: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        self.calls += 1;
        Ok(())
    }
}

#[test]
fn provider_catalog_identity_and_admission_fail_closed_on_tamper_or_reorder() {
    let module = provider_module();
    let original = semantic_fingerprint(&module).expect("canonical fingerprint");

    let mut identity_tamper = module.clone();
    identity_tamper.provider_candidates[1].candidate_identity = "SecondProvider::other".into();
    assert_ne!(
        semantic_fingerprint(&identity_tamper).expect("identity tamper remains representable"),
        original
    );
    let (identity_semantic, identity_proof) = artifact(&identity_tamper);
    let identity_plan = lower_artifact_sections(
        &identity_semantic,
        &identity_proof,
        &AdmissionProfile::default(),
    )
    .expect("identity-tampered artifact remains valid");
    let formerly_selected = selected("second-plan", "SecondProvider", "SecondProvider::emit");
    assert!(matches!(
        admit_provider_installation(
            &identity_plan,
            &identity_semantic,
            &identity_proof,
            &AdmissionProfile::default(),
            &formerly_selected,
        ),
        Err(ProviderInstallationError::SelectedProviderMismatch { .. })
    ));

    let mut invalid = module.clone();
    invalid.provider_candidates[1]
        .signature
        .parameters
        .push(ProviderSignatureParameter {
            position: 0,
            is_self: false,
            structural_type: structural_type_id(2),
            multiplicity: StructuralMultiplicity::Unrestricted,
            access: psi_terminal::StructuralAccess::Owned,
            qualifications: Vec::new(),
        });
    assert!(matches!(
        validate_module(&invalid),
        Err(ModuleError::InvalidProviderCandidate { .. })
    ));
    assert!(semantic_fingerprint(&invalid).is_err());

    let mut reordered = module.clone();
    reordered.provider_candidates.swap(0, 1);
    assert!(matches!(
        validate_module(&reordered),
        Err(ModuleError::InvalidProviderCandidate { .. })
    ));
    assert!(encode_module(&reordered).is_err());

    let (semantic, proof) = artifact(&module);
    let profile = AdmissionProfile::default();
    let plan = lower_artifact_sections(&semantic, &proof, &profile).expect("verified lowering");
    let selected = selected("second-plan", "SecondProvider", "SecondProvider::emit");
    let installation = admit_provider_installation(&plan, &semantic, &proof, &profile, &selected)
        .expect("installation for original artifact");
    let mut other = module.clone();
    let OperationKind::PortWrite { value, .. } =
        &mut other.machines[1].blocks[0].operations[0].kind
    else {
        panic!("fixture candidate writes a port")
    };
    *value = 67;
    let (other_semantic, other_proof) = artifact(&other);
    assert!(matches!(
        TerminalExecution::start_artifact_with_provider_installation(
            &other_semantic,
            &other_proof,
            &profile,
            &[],
            &[],
            installation.psi_installation(),
        ),
        Err(
            psi_terminal_interpreter::TerminalArtifactInterpretError::Execution(
                TerminalInterpretError::ProviderInstallationIdentityMismatch
            )
        )
    ));
}

#[test]
fn provider_catalog_union_rejects_a_candidate_that_reenters_its_boundary() {
    let mut module = provider_module();
    module.provider_candidates.remove(0);
    module.machines.remove(1);
    module.machines[1].blocks[0].operations[0] = Operation {
        id: operation_id(3),
        result: OperationResult::Unit,
        kind: OperationKind::BoundaryCall {
            boundary: boundary_id(1),
            arguments: Vec::new(),
            structural_arguments: Vec::new(),
            completion_receipts: Vec::new(),
        },
    };

    assert_eq!(
        validate_module(&module).unwrap_err(),
        ModuleError::RecursiveCallSliceNotYetSupported(machine_id(3))
    );
}

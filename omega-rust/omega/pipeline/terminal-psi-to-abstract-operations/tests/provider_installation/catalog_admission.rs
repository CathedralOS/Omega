use super::builders::{artifact, provider_module, selected};
use super::ids::{boundary_id, machine_id, operation_id, structural_type_id};
use proof_admission::AdmissionProfile;
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use terminal_codec::encode_proof_bundle;
use terminal_codec::{
    build_terminal_obligation_ledger, current_terminal_trust_graph, encode_module,
    encode_terminal_obligation_ledger, semantic_fingerprint,
};
use terminal_fuel::TerminalFuelMeter;
use terminal_interpreter::{
    TerminalEffect, TerminalEffectHandler, TerminalEffectRejection, TerminalExecution,
    TerminalExecutionResult, TerminalExecutionStatus, TerminalInterpretError,
};
use terminal_psi::{
    Operation, OperationKind, OperationResult, ProviderSignatureParameter, StructuralMultiplicity,
};
use terminal_psi_to_abstract_operations::SelectedProviderAdapter;
use terminal_psi_to_abstract_operations::{
    ProviderInstallationError, admit_provider_installation, lower_artifact_sections,
    lower_replay_artifact_sections, lower_replay_artifact_sections_for_optimization,
};
use terminal_verifier::{ModuleError, validate_module};
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees_to_checked_trees::lower_typed_trees;

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
        Err(terminal_psi_to_abstract_operations::ArtifactLoweringError::ObligationReplay(_))
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
    let optimized_installation = admit_provider_installation(
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
    assert_eq!(installation.installed_calls().len(), 1);
    assert_eq!(
        optimized_installation.installed_candidates(),
        installation.installed_candidates()
    );
    assert_eq!(
        optimized_installation.installed_calls(),
        installation.installed_calls()
    );
    let installed_call = &installation.installed_calls()[0];
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
            access: terminal_psi::StructuralAccess::Owned,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
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
            &[],
            installation.psi_installation(),
        ),
        Err(
            terminal_interpreter::TerminalArtifactInterpretError::Execution(
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
        static_reach_binding: None,
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

#[test]
fn unit_provider_installation_joins_independently_selected_overload_identity() {
    check_selected_overload_identity(
        r#"
boundary trait Sink { machine take(value: i32) reaches Sink; }
data Provider {}
machine Provider::take(value: i32) satisfies Sink::take {}
machine enter(value: i32) reaches Sink { Sink::take(value); }
"#,
        "Provider::take",
        "data Provider {} machine Provider::take(value: u64) {}",
    );
}

#[test]
fn provider_installation_accepts_a_verified_local_byte_view() {
    check_selected_overload_identity(
        r#"
boundary trait Sink { machine take(value: &[u8]) reaches Sink; }
data Provider {}
machine Provider::take(value: &[u8]) satisfies Sink::take {}
machine enter() reaches Sink { Sink::take("local bytes"); }
"#,
        "Provider::take",
        "data Provider {} machine Provider::take(value: u64) {}",
    );
}

#[test]
fn provider_installation_accepts_a_verified_projected_array_loan_without_claims() {
    check_selected_overload_identity(
        r#"
boundary trait Sink { machine take(value: &mut [u8]) reaches Sink; }
data Provider {}
machine Provider::take(value: &mut [u8]) satisfies Sink::take {}
pub data Buffer { bytes: [u8; 8]; }
machine enter(buffer: &mut Buffer) reaches Sink { Sink::take(&mut buffer.bytes); }
"#,
        "Provider::take",
        "data Provider {} machine Provider::take(value: u64) {}",
    );
}

fn check_selected_overload_identity(source: &str, provider_machine: &str, wrong_overload: &str) {
    let tokens = Lexer::new(source).tokenize().unwrap();
    let syntax = parse_syntax_trees(&tokens).unwrap();
    let resolved = lower_syntax_trees(&syntax).unwrap();
    let typed = lower_symbol_resolved_trees(&resolved).unwrap();
    let checked = lower_typed_trees(typed).unwrap();
    let machine = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == provider_machine)
        .expect("the independently selected source provider");
    let machine_identity = checked
        .typed
        .normalized_machine_overload_identity(machine)
        .expect("selected source overload identity")
        .identity();
    assert_ne!(machine_identity, provider_machine);
    let boundary = checked
        .typed
        .traits()
        .iter()
        .find(|definition| definition.is_boundary)
        .unwrap();
    let requirement = &checked.typed.trait_machine_signatures(boundary)[0];
    let selected = SelectedProviderAdapter {
        requirement_identity: checked
            .typed
            .normalized_trait_requirement_overload_identity(boundary, requirement)
            .identity(),
        provider_identity: "Provider".into(),
        machine_identity,
    };
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "enter").unwrap();
    let semantic = encode_module(&lowered.semantic_module).unwrap();
    let proof = encode_proof_bundle(&lowered.proof_bundle).unwrap();
    let profile = AdmissionProfile::default();
    let plan = lower_artifact_sections(&semantic, &proof, &profile).unwrap();
    admit_provider_installation(&plan, &semantic, &proof, &profile, &[selected.clone()])
        .expect("source-selected overload rejoins its exact emitted candidate");
    let candidate = plan
        .provider_candidates
        .iter()
        .find(|candidate| candidate.provider_identity == "Provider")
        .unwrap();
    assert_eq!(candidate.candidate_identity, selected.machine_identity);

    let mut changed_module = lowered.semantic_module.clone();
    let borrowed_argument = changed_module
        .machines
        .iter_mut()
        .flat_map(|machine| {
            machine
                .blocks
                .iter_mut()
                .flat_map(|block| &mut block.operations)
        })
        .find_map(|operation| match &mut operation.kind {
            OperationKind::BoundaryCall {
                structural_arguments,
                ..
            } => structural_arguments.first_mut(),
            _ => None,
        });
    if let Some(argument) = borrowed_argument {
        argument.access = terminal_psi::StructuralAccess::Owned;
        assert!(
            validate_module(&changed_module).is_err(),
            "independent Terminal checking must reject turning a loan into ownership"
        );
    }

    let tokens = Lexer::new(wrong_overload).tokenize().unwrap();
    let syntax = parse_syntax_trees(&tokens).unwrap();
    let resolved = lower_syntax_trees(&syntax).unwrap();
    let typed = lower_symbol_resolved_trees(&resolved).unwrap();
    let wrong = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == provider_machine)
        .unwrap();
    let wrong_identity = typed
        .normalized_machine_overload_identity(wrong)
        .unwrap()
        .identity();
    assert_ne!(wrong_identity, selected.machine_identity);
    for replacement in [wrong_identity, provider_machine.to_owned()] {
        let mut corrupted = selected.clone();
        corrupted.machine_identity = replacement;
        assert!(matches!(
            admit_provider_installation(&plan, &semantic, &proof, &profile, &[corrupted]),
            Err(terminal_psi_to_abstract_operations::ProviderInstallationError::SelectedProviderMismatch { .. })
        ));
    }
    let mut wrong_provider = selected.clone();
    wrong_provider.provider_identity = "UnselectedProvider".into();
    assert!(matches!(
        admit_provider_installation(&plan, &semantic, &proof, &profile, &[wrong_provider]),
        Err(terminal_psi_to_abstract_operations::ProviderInstallationError::SelectedProviderMismatch { .. })
    ));
    let mut wrong_requirement = selected;
    wrong_requirement.requirement_identity = "unselected-requirement".into();
    assert!(matches!(
        admit_provider_installation(&plan, &semantic, &proof, &profile, &[wrong_requirement]),
        Err(terminal_psi_to_abstract_operations::ProviderInstallationError::MissingSelectedProvider { .. })
    ));
}

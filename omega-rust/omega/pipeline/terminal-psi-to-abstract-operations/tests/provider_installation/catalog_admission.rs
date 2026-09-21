use super::builders::{artifact, provider_module, selected};
use super::ids::{boundary_id, machine_id, operation_id, structural_type_id};
use proof_admission::AdmissionProfile;
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
use terminal_codec::{
    build_terminal_obligation_ledger, current_terminal_trust_graph, encode_module,
    encode_proof_section, encode_terminal_obligation_ledger, semantic_fingerprint,
};
use terminal_fuel::TerminalFuelMeter;
use terminal_interpreter::AcceptTerminalEffects;
use terminal_interpreter::TerminalStructuralInputs;
use terminal_interpreter::{
    TerminalEffect, TerminalEffectHandler, TerminalEffectRejection, TerminalExecution,
    TerminalExecutionResult, TerminalExecutionStatus, TerminalInterpretError,
};
use terminal_psi::{
    Operation, OperationKind, OperationResult, ProviderSignatureParameter, StructuralMultiplicity,
};
use terminal_psi_to_abstract_operations::{
    ProviderInstallationError, SelectedProviderAdapter, admit_provider_installation, lower_artifact,
};
use terminal_verifier::{ModuleError, validate_module};
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees_to_checked_trees::CheckingRequest;
use typed_trees_to_checked_trees::lower_typed_trees;

#[test]
fn artifact_admission_preserves_decode_and_replay_diagnostic_order() {
    use terminal_psi_to_abstract_operations::{ArtifactLoweringError, ArtifactSections};
    let module = provider_module();
    let (semantic, _) = artifact(&module);
    let trust = current_terminal_trust_graph().unwrap();
    let ledger = build_terminal_obligation_ledger(&module, &trust)
        .and_then(|ledger| encode_terminal_obligation_ledger(&ledger))
        .unwrap();
    let profile = AdmissionProfile::default();
    let requests = [
        ArtifactSections {
            semantic_bytes: &[],
            proof_bytes: &[],
            obligation_ledger_bytes: Some(&[]),
        },
        ArtifactSections {
            semantic_bytes: &semantic,
            proof_bytes: &[],
            obligation_ledger_bytes: Some(&[]),
        },
        ArtifactSections {
            semantic_bytes: &semantic,
            proof_bytes: &[],
            obligation_ledger_bytes: Some(&ledger),
        },
        ArtifactSections {
            semantic_bytes: &semantic,
            proof_bytes: &[],
            obligation_ledger_bytes: None,
        },
    ];
    for (position, request) in requests.into_iter().enumerate() {
        let result = lower_artifact(request, &profile).map(|_| ());
        match position {
            0 => assert!(matches!(
                result,
                Err(ArtifactLoweringError::SemanticDecode(_))
            )),
            1 => assert!(matches!(
                result,
                Err(ArtifactLoweringError::ObligationLedgerDecode(_))
            )),
            _ => assert!(matches!(result, Err(ArtifactLoweringError::ProofDecode(_)))),
        }
    }
}

#[test]
fn admitted_artifacts_retain_rosters_and_reject_unsupplied_native_custody() {
    use terminal_psi_to_abstract_operations::{ArtifactLoweringError, ArtifactSections};
    let mut module = provider_module();
    let (original_semantic, _) = artifact(&module);
    let trust = current_terminal_trust_graph().unwrap();
    let stale_ledger = build_terminal_obligation_ledger(&module, &trust)
        .and_then(|ledger| encode_terminal_obligation_ledger(&ledger))
        .unwrap();
    let identity = |name: &str| format!("package:{}::{name}", "01".repeat(32));
    let policy_identity = identity("Uart");
    let schema_identity = identity("Registers");
    module
        .placed_view_inputs
        .push(terminal_psi::TerminalPlacedViewInput {
            machine: module.entry,
            position: 0,
            source_machine_identity: identity("inspect"),
            source_state_identity: identity("inspect::entry"),
            source_parameter_identity: identity("inspect::entry::view0"),
            access: terminal_psi::StructuralAccess::MutableBorrow,
            binding_is_const: false,
            binding_is_mutable: true,
            view_identity: terminal_psi::canonical_placed_view_identity(
                &policy_identity,
                &schema_identity,
            ),
            policy_identity,
            policy_plan_machine_identity: identity("Uart::plan"),
            schema_identity,
            placement_report_fingerprint: 41,
            placement_commitment: [0x5a; 32],
        });
    let (semantic, proof) = artifact(&module);
    assert_ne!(semantic, original_semantic);
    let ledger = build_terminal_obligation_ledger(&module, &trust)
        .and_then(|ledger| encode_terminal_obligation_ledger(&ledger))
        .unwrap();
    let profile = AdmissionProfile::default();
    for obligation_ledger_bytes in [None, Some(ledger.as_slice())] {
        let sections = ArtifactSections {
            semantic_bytes: &semantic,
            proof_bytes: &proof,
            obligation_ledger_bytes,
        };
        let native = lower_artifact(sections, &profile).unwrap();
        assert_eq!(native.placed_view_inputs(), module.placed_view_inputs);
        let optimizer = native.clone().into_optimization_artifact();
        assert_eq!(optimizer.placed_view_inputs(), module.placed_view_inputs);
        // The roster stays retained inside the optimizer input's verifier
        // context module: downgrading authority never erases custody rows.
        assert_eq!(
            optimizer
                .into_optimization_input()
                .context()
                .module()
                .placed_view_inputs,
            module.placed_view_inputs
        );
        // A declared entry row with no supplied establishment fails custody
        // at the native input boundary rather than being silently dropped.
        assert!(matches!(
            native.try_into_native_input(&[]),
            Err(ArtifactLoweringError::PlacedViewInputsRequireCustodyLowering)
        ));
    }
    let stale = ArtifactSections {
        semantic_bytes: &semantic,
        proof_bytes: &[],
        obligation_ledger_bytes: Some(&stale_ledger),
    };
    assert!(matches!(
        lower_artifact(stale, &profile).map(|_| ()),
        Err(ArtifactLoweringError::ObligationReplay(_))
    ));
}

#[test]
fn omega_installs_only_the_checked_adapter_selected_by_provider_plan_facts() {
    let module = provider_module();
    let (semantic, proof) = artifact(&module);
    let profile = AdmissionProfile::default();
    let plan = lower_artifact(
        terminal_psi_to_abstract_operations::ArtifactSections {
            semantic_bytes: &semantic,
            proof_bytes: &proof,
            obligation_ledger_bytes: None,
        },
        &profile,
    )
    .map(|admitted| admitted.into_plan())
    .expect("verified lowering");
    let trust_graph = current_terminal_trust_graph().expect("current trust graph");
    let obligation_ledger = build_terminal_obligation_ledger(&module, &trust_graph)
        .and_then(|ledger| encode_terminal_obligation_ledger(&ledger))
        .expect("canonical obligation ledger");
    assert_eq!(
        lower_artifact(
            terminal_psi_to_abstract_operations::ArtifactSections {
                semantic_bytes: &semantic,
                proof_bytes: &proof,
                obligation_ledger_bytes: Some(&obligation_ledger)
            },
            &profile
        )
        .map(|admitted| admitted.into_plan())
        .expect("locally replayed artifact lowering"),
        plan
    );
    let replayed_optimizer_input = lower_artifact(
        terminal_psi_to_abstract_operations::ArtifactSections {
            semantic_bytes: &semantic,
            proof_bytes: &proof,
            obligation_ledger_bytes: Some(&obligation_ledger),
        },
        &profile,
    )
    .map(|admitted| {
        admitted
            .into_optimization_artifact()
            .into_optimization_input()
    })
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
        lower_artifact(
            terminal_psi_to_abstract_operations::ArtifactSections {
                semantic_bytes: &semantic,
                proof_bytes: &proof,
                obligation_ledger_bytes: Some(&substituted_ledger)
            },
            &profile
        )
        .map(|admitted| admitted.into_plan()),
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
    let mut execution = TerminalExecution::start_installed_artifact(
        &semantic,
        &proof,
        &profile,
        &[],
        TerminalStructuralInputs::default(),
        installation.psi_installation(),
    )
    .expect("selected installation starts");
    assert_eq!(
        execution
            .resume(
                &mut TerminalFuelMeter::default(),
                &mut AcceptTerminalEffects
            )
            .unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert!(matches!(
        execution.effects(),
        [TerminalEffect::PortWrite { value: 66, .. }]
    ));

    let mut uninstalled = TerminalExecution::start_artifact(
        &semantic,
        &proof,
        &profile,
        &[],
        TerminalStructuralInputs::default(),
    )
    .expect("artifact starts without an installation");
    let mut handler = CountingEffects::default();
    assert!(matches!(
        uninstalled.resume(&mut TerminalFuelMeter::default(), &mut handler),
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
    let identity_plan = lower_artifact(
        terminal_psi_to_abstract_operations::ArtifactSections {
            semantic_bytes: &identity_semantic,
            proof_bytes: &identity_proof,
            obligation_ledger_bytes: None,
        },
        &AdmissionProfile::default(),
    )
    .map(|admitted| admitted.into_plan())
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
    let plan = lower_artifact(
        terminal_psi_to_abstract_operations::ArtifactSections {
            semantic_bytes: &semantic,
            proof_bytes: &proof,
            obligation_ledger_bytes: None,
        },
        &profile,
    )
    .map(|admitted| admitted.into_plan())
    .expect("verified lowering");
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
        TerminalExecution::start_installed_artifact(
            &other_semantic,
            &other_proof,
            &profile,
            &[],
            TerminalStructuralInputs::default(),
            installation.psi_installation()
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
        suspension_crossing: None,
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
    let resolved = resolve(ResolutionRequest::new(&syntax)).unwrap();
    let typed = lower_symbol_resolved_trees(&resolved).unwrap();
    let checked = lower_typed_trees(typed, &CheckingRequest::settled()).unwrap();
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
    let proof = encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle).unwrap();
    let profile = AdmissionProfile::default();
    let plan = lower_artifact(
        terminal_psi_to_abstract_operations::ArtifactSections {
            semantic_bytes: &semantic,
            proof_bytes: &proof,
            obligation_ledger_bytes: None,
        },
        &profile,
    )
    .map(|admitted| admitted.into_plan())
    .unwrap();
    admit_provider_installation(
        &plan,
        &semantic,
        &proof,
        &profile,
        std::slice::from_ref(&selected),
    )
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
    let resolved = resolve(ResolutionRequest::new(&syntax)).unwrap();
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

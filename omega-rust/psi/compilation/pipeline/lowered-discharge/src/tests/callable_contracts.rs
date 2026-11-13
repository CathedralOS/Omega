//! Construction refinement is independently replayed against the current header.

use super::*;
use terminal_psi::{
    BoundaryMachineResult, ContractClause, CrashRouteBucket, CrashRouteGuard,
    EvidenceArgumentSubject, EvidenceCallableOwner, EvidenceContractLane, EvidenceContractLaneKind,
    EvidenceInterfaceIdentity, EvidenceTermDeclaration, MachineContract,
    OutcomeSpecificCallArgumentSubstitution, PropositionApplicationIdentity,
    PropositionDeclaration, PropositionEvidence, TerminalCallableContract,
    TerminalCallableEnvelope, TerminalCallableEvidenceLane, TerminalCallableTermination,
    TerminalMachineDescriptor, TerminalMachineDescriptorOrigin, TerminalMachineResult,
    TerminalModule,
};
use terminal_semantics::calls::callable_refinement::{
    CallableRefinementGoalKind, callable_contract_refinement_goals, callable_refinement_context,
    descriptor_refinement_goals, validate_callable_evidence_contracts_in,
    validate_descriptor_refinement_evidence,
};

fn subject_bound(result: u64, parameter: u64) -> Proposition {
    Proposition::LessOrEqual(formal(result, u32_type()), formal(parameter, u32_type()))
}

fn refinement_module() -> TerminalModule {
    let mut selected = machine(vec![u32_type()], vec![returns(slot(0, u32_type()))]);
    selected.requires = vec![at_least(1, 3)];
    selected.ensures = vec![subject_bound(2, 1)];
    let (elaboration, verdict) = run(vec![selected]);
    for (_, result) in verdict.machines.get(id(1)).unwrap().verdicts.iter() {
        assert!(
            result.is_ok(),
            "selected body must establish its own contract: {result:?}"
        );
    }
    let mut module = elaboration.module;
    let selected = &module.machines[0];
    assert_eq!(selected.parameters[0].id, id(1));
    assert_eq!(selected.result.scalar().unwrap().id, id(2));
    let parameter = terminal_psi::ValueDeclaration {
        id: id(10),
        ..selected.parameters[0]
    };
    let result = terminal_psi::ValueDeclaration {
        id: id(11),
        ..selected.result.scalar().unwrap()
    };
    let parameter_order = selected.parameter_order.clone();
    let mut identities = module.identities.clone().into_builder();
    let identity = identities.intern("callable-refinement-test-contract");
    let application = identities.intern("callable-refinement-test-selection");
    module.identities = identities.build();
    let mut proof = MachineContract::empty(id(10));
    proof.requires = vec![at_least(10, 5)];
    proof.ensures = vec![ContractClause {
        obligation: id(90),
        proposition: subject_bound(11, 10),
    }];
    module.callables.contracts.push(TerminalCallableContract {
        id: id(1),
        identity,
        scalar_parameters: vec![parameter],
        parameter_order,
        structural_parameters: Vec::new(),
        result: BoundaryMachineResult::Scalar(result.value_type()),
        proof_result: TerminalMachineResult::Scalar(result),
        entry_claim_paths: Vec::new(),
        result_claim_paths: Vec::new(),
        result_reference_regions: Vec::new(),
        proof,
        evidence_contract: Vec::new(),
        envelope: TerminalCallableEnvelope {
            service_reach: Vec::new(),
            invocations: Vec::new(),
            suspends: false,
            blocks: false,
            termination: TerminalCallableTermination::NotGuaranteed,
        },
    });
    let goals = callable_contract_refinement_goals(&module, id(1), id(1)).unwrap();
    let mut next_obligation = 100;
    let obligations = goals
        .iter()
        .map(|_| {
            let obligation = id(next_obligation);
            next_obligation += 1;
            obligation
        })
        .collect();
    module
        .callables
        .descriptors
        .push(TerminalMachineDescriptor {
            id: id(1),
            contract: id(1),
            machine: id(1),
            origin: TerminalMachineDescriptorOrigin {
                application,
                conformance: None,
                requirement: None,
            },
            refinement_obligations: obligations,
            refinement_evidence: Vec::new(),
        });
    module
}

fn derive_refinements(
    module: &TerminalModule,
) -> Result<Vec<terminal_psi::CertificateEnvelope>, &'static str> {
    let context = callable_refinement_context(module, id(1))?;
    let required = &module.callables.contracts[0];
    let mut declarations = required
        .scalar_parameters
        .iter()
        .chain(&required.proof.erased_scalar_formals)
        .chain(required.proof_result.scalar_ref())
        .map(|value| (value.id, value.scalar_type))
        .collect::<Vec<_>>();
    declarations.sort_by_key(|(id, _)| *id);
    descriptor_refinement_goals(module, id(1))?
        .into_iter()
        .map(|(id, goal)| {
            crate::derive_contract_obligation(&context, id, &goal.proposition, &declarations)
        })
        .collect()
}

#[test]
fn callable_refinement_proves_input_weakening_and_remaps_result_formals() {
    let mut module = refinement_module();
    let goals = descriptor_refinement_goals(&module, id(1)).unwrap();
    assert_eq!(
        goals.iter().map(|(_, goal)| &goal.kind).collect::<Vec<_>>(),
        [
            &CallableRefinementGoalKind::Input,
            &CallableRefinementGoalKind::NormalOutput
        ]
    );
    module.callables.descriptors[0].refinement_evidence = derive_refinements(&module).unwrap();
    assert_eq!(
        validate_descriptor_refinement_evidence(&module)
            .unwrap()
            .len(),
        goals.len()
    );
    let mut missing = module.clone();
    missing.callables.descriptors[0].refinement_evidence.pop();
    assert!(validate_descriptor_refinement_evidence(&missing).is_err());
    let mut aliased = module.clone();
    aliased.callables.descriptors[0].refinement_obligations[1] = id(100);
    assert!(validate_descriptor_refinement_evidence(&aliased).is_err());
    let mut changed = module;
    changed.callables.contracts[0].proof.requires = vec![at_least(10, 2)];
    assert!(derive_refinements(&changed).is_err());
    assert!(validate_descriptor_refinement_evidence(&changed).is_err());
}

#[test]
fn checker_finalizes_descriptors_and_refuses_mutated_retained_certificates() {
    let mut module = refinement_module();
    crate::finalize_callable_refinements(&mut module).unwrap();
    assert!(
        !module.callables.descriptors[0]
            .refinement_evidence
            .is_empty()
    );
    let retained = module.callables.descriptors[0].refinement_evidence.clone();
    crate::finalize_callable_refinements(&mut module).unwrap();
    assert_eq!(
        module.callables.descriptors[0].refinement_evidence,
        retained
    );
    module.callables.descriptors[0].refinement_evidence[0]
        .proof
        .conclusion = Proposition::Falsehood;
    assert!(crate::finalize_callable_refinements(&mut module).is_err());
}

#[test]
fn callable_refinement_refuses_weaker_outputs_and_unpermitted_crashes() {
    let mut module = refinement_module();
    module.callables.descriptors[0].refinement_evidence = derive_refinements(&module).unwrap();
    let mut weaker = module.clone();
    weaker.machines[0].contract.ensures[0].proposition = at_most(2, u32::MAX.into());
    assert!(derive_refinements(&weaker).is_err());
    assert!(validate_descriptor_refinement_evidence(&weaker).is_err());
    module.machines[0]
        .contract
        .crash_routes
        .push(CrashRouteBucket {
            cause: semantic_vocabulary::CrashCause::Trap,
            alternatives: vec![CrashRouteGuard::Truth],
        });
    let goals = callable_contract_refinement_goals(&module, id(1), id(1)).unwrap();
    assert!(matches!(
        goals.last().unwrap().kind,
        CallableRefinementGoalKind::Crash { .. }
    ));
    assert!(validate_descriptor_refinement_evidence(&module).is_err());
    // Allocate the new question too: refusal must be logical, not only a stale count.
    module.callables.descriptors[0]
        .refinement_obligations
        .push(id(102));
    assert!(derive_refinements(&module).is_err());
}

fn install_roster(module: &mut TerminalModule) {
    let mut identities = module.identities.clone().into_builder();
    let interface = EvidenceInterfaceIdentity {
        trait_identity: identities.intern("proof-witness"),
        arguments: vec!["u32".into()],
        requirements: Vec::new(),
    };
    module.identities = identities.build();
    let declaration = PropositionDeclaration {
        id: id(1),
        name: "Bound".into(),
        binders: Vec::new(),
        parameter_types: vec!["u32".into()],
        evidence: PropositionEvidence::Witness {
            evidence_type: "proof-witness[u32]".into(),
        },
    };
    module.proposition_declarations.push(declaration.clone());
    for (position, kind) in [
        EvidenceContractLaneKind::Requires,
        EvidenceContractLaneKind::Ensures,
    ]
    .into_iter()
    .enumerate()
    {
        let subjects = match kind {
            EvidenceContractLaneKind::Requires => vec![EvidenceArgumentSubject::Parameter(
                OutcomeSpecificCallArgumentSubstitution {
                    argument_position: 0,
                    parameter_position: 0,
                    structural_path: None,
                },
            )],
            EvidenceContractLaneKind::Ensures => vec![EvidenceArgumentSubject::Result {
                argument_position: 0,
            }],
        };
        let application = PropositionApplicationIdentity {
            id: id(position as u64 + 10),
            declaration: declaration.id,
            binder_arguments: Vec::new(),
            arguments: vec![format!("formal-{position}")],
            evidence_interface: Some(interface.clone()),
        };
        let term = EvidenceTermDeclaration {
            id: id(position as u64 + 1),
            proposition: application.id,
            interface: interface.clone(),
        };
        module.callables.contracts[0]
            .evidence_contract
            .push(TerminalCallableEvidenceLane {
                kind,
                position: 0,
                declaration: declaration.clone(),
                application: application.clone(),
                term: term.clone(),
                argument_subjects: subjects.clone(),
            });
        module.proposition_applications.push(application);
        module.evidence_terms.push(term.clone());
        module.evidence_contract_lanes.push(EvidenceContractLane {
            owner: EvidenceCallableOwner::Runtime(id(1)),
            kind,
            position: 0,
            term: term.id,
            output_field: (kind == EvidenceContractLaneKind::Ensures).then(|| "$0".into()),
            argument_subjects: subjects,
        });
    }
}

fn roster_valid(module: &TerminalModule) -> bool {
    validate_callable_evidence_contracts_in(
        &module.callables,
        &module.proposition_declarations,
        &module.proposition_applications,
        &module.evidence_terms,
        &module.evidence_contract_lanes,
    )
    .is_ok()
}

#[test]
fn runtime_evidence_roster_rejoins_full_meaning_and_exact_subjects() {
    let mut module = refinement_module();
    install_roster(&mut module);
    assert!(roster_valid(&module));
    let mut missing = module.clone();
    missing.evidence_contract_lanes.pop();
    assert!(!roster_valid(&missing));
    let mut duplicate = module.clone();
    duplicate
        .evidence_contract_lanes
        .push(duplicate.evidence_contract_lanes[0].clone());
    assert!(!roster_valid(&duplicate));
    let mut subject = module.clone();
    subject.evidence_contract_lanes[0].argument_subjects[0] =
        EvidenceArgumentSubject::Parameter(OutcomeSpecificCallArgumentSubstitution {
            argument_position: 0,
            parameter_position: 1,
            structural_path: None,
        });
    assert!(!roster_valid(&subject));
    let mut interface = module.clone();
    interface.evidence_terms[0].interface.arguments[0] = "u64".into();
    assert!(!roster_valid(&interface));
    let mut application = module;
    application.proposition_applications[0].arguments[0] = "another-formal".into();
    assert!(!roster_valid(&application));
}

//! Call ceilings may follow entry requirements without rewriting callee routes.

use proof_admission::AdmissionProfile;
use semantic_vocabulary::{
    BlockId, ContractId, EdgeId, MachineId, OperationId, Proposition, ScalarTerm, ScalarType,
    ValueId,
};
use terminal_psi::{
    Block, CrashCause, CrashPredicateTerm, CrashRouteBucket, CrashRouteGuard, MachineContract,
    Operation, OperationKind, OperationResult, TerminalMachine, TerminalMachineResult,
    TerminalModule, Terminator, ValueDeclaration, VocabularyMarker,
};
use terminal_verifier::{ModuleError, ProofBundle, validate_module, verify_module};

fn boolean(position: u64, value: bool) -> Proposition {
    let mut terms = [
        ScalarTerm::value(ValueId::new(position).unwrap(), ScalarType::Boolean),
        ScalarTerm::boolean(value),
    ];
    terms.sort();
    Proposition::Equal(terms[0].clone(), terms[1].clone())
}

fn bucket(guard: CrashRouteGuard) -> CrashRouteBucket {
    CrashRouteBucket {
        cause: CrashCause::Trap,
        alternatives: vec![guard],
    }
}

fn predicate(proposition: Proposition) -> CrashRouteGuard {
    CrashRouteGuard::Predicate(CrashPredicateTerm::new(proposition))
}

fn machine(identity: u64) -> TerminalMachine {
    TerminalMachine {
        id: MachineId::new(identity).unwrap(),
        attachment: None,
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        ranked_scc: None,
        result: TerminalMachineResult::Unit,
        structural_places: Vec::new(),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: BlockId::new(identity).unwrap(),
        blocks: vec![Block {
            id: BlockId::new(identity).unwrap(),
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::ReturnUnit {
                edge: EdgeId::new(identity).unwrap(),
                trivial_affine_discards: Vec::new(),
            },
        }],
        contract: MachineContract {
            id: ContractId::new(identity).unwrap(),
            requires: Vec::new(),
            ensures: Vec::new(),
            crash_routes: Vec::new(),
            outcome_specific_ensures: Vec::new(),
        },
    }
}

fn module(scalar_call: bool) -> TerminalModule {
    let mut caller = machine(1);
    caller.parameters = [1, 2]
        .map(|identity| ValueDeclaration {
            id: ValueId::new(identity).unwrap(),
            scalar_type: ScalarType::Boolean,
        })
        .to_vec();
    caller.contract.requires = vec![boolean(1, true)];
    caller.contract.crash_routes = vec![bucket(predicate(boolean(1, true)))];
    let mut callee = machine(2);
    callee.contract.crash_routes = vec![bucket(CrashRouteGuard::Truth)];
    callee.blocks[0].terminator = Terminator::Crash {
        edge: EdgeId::new(2).unwrap(),
        cause: CrashCause::Trap,
        site_guard: Vec::new(),
        frontier_lower_bound: Vec::new(),
    };
    let call = if scalar_call {
        callee.result = TerminalMachineResult::Scalar(ValueDeclaration {
            id: ValueId::new(4).unwrap(),
            scalar_type: ScalarType::Boolean,
        });
        Operation {
            id: OperationId::new(1).unwrap(),
            result: OperationResult::Scalar(ValueDeclaration {
                id: ValueId::new(3).unwrap(),
                scalar_type: ScalarType::Boolean,
            }),
            kind: OperationKind::Call {
                callee: callee.id,
                arguments: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: callee.contract.crash_routes.clone(),
            },
        }
    } else {
        Operation {
            id: OperationId::new(1).unwrap(),
            result: OperationResult::Unit,
            kind: OperationKind::CallUnit {
                callee: callee.id,
                arguments: Vec::new(),
                structural_arguments: Vec::new(),
                claim_transfers: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: callee.contract.crash_routes.clone(),
            },
        }
    };
    caller.blocks[0].operations.push(call);
    TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: caller.id,
        structural_types: Vec::new(),
        structural_domains: Vec::new(),
        services: Vec::new(),
        root_service_reach: Default::default(),
        placed_view_inputs: Vec::new(),
        reborrow_root_handoffs: Vec::new(),
        reborrow_restored_call_uses: Vec::new(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        float_meaning_projections: Vec::new(),
        float_meaning_equalities: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        evidence_terms: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        proof_output_calls: Vec::new(),
        proof_recursive_components: Vec::new(),
        closed_conformance_applications: Vec::new(),
        dynamic_dispatch: Default::default(),
        suspension_call_plan_count: 0,
        suspension_call_sites: Vec::new(),
        suspension_call_plans: Vec::new(),
        quotient_correspondences: Vec::new(),
        machines: vec![caller, callee],
    }
}

fn rejects_coverage(module: &TerminalModule) {
    assert!(
        matches!(validate_module(module), Err(ModuleError::CallCrashContinuationUncovered { operation, cause: CrashCause::Trap }) if operation == OperationId::new(1).unwrap())
    );
}

#[test]
fn entry_requirement_covers_an_unconditional_scalar_or_unit_callee_route() {
    for scalar_call in [false, true] {
        let module = module(scalar_call);
        verify_module(
            &module,
            &ProofBundle::default(),
            &AdmissionProfile::default(),
        )
        .expect("entry requirement proves the published ceiling without producer evidence");
    }
}

#[test]
fn conjunction_requirements_may_project_the_exact_published_predicate() {
    for scalar_call in [false, true] {
        let mut module = module(scalar_call);
        let mut conjuncts = vec![boolean(1, true), boolean(2, true)];
        conjuncts.sort();
        module.machines[0].contract.requires = vec![Proposition::Conjunction(conjuncts)];
        verify_module(
            &module,
            &ProofBundle::default(),
            &AdmissionProfile::default(),
        )
        .unwrap();
    }
}

#[test]
fn separate_requirements_may_establish_a_conjunctive_ceiling() {
    for scalar_call in [false, true] {
        let mut module = module(scalar_call);
        let mut conjuncts = vec![boolean(1, true), boolean(2, true)];
        conjuncts.sort();
        module.machines[0].contract.requires = conjuncts.clone();
        module.machines[0].contract.crash_routes =
            vec![bucket(predicate(Proposition::Conjunction(conjuncts)))];
        verify_module(
            &module,
            &ProofBundle::default(),
            &AdmissionProfile::default(),
        )
        .unwrap();
    }
}

#[test]
fn every_disjunct_must_establish_the_same_call_ceiling() {
    for scalar_call in [false, true] {
        let mut module = module(scalar_call);
        module.machines[0].parameters.push(ValueDeclaration {
            id: ValueId::new(5).unwrap(),
            scalar_type: ScalarType::Boolean,
        });
        let mut alternatives = [2, 5]
            .map(|identity| {
                let mut children = vec![boolean(1, true), boolean(identity, true)];
                children.sort();
                Proposition::Conjunction(children)
            })
            .to_vec();
        alternatives.sort();
        module.machines[0].contract.requires = vec![Proposition::Disjunction(alternatives.clone())];
        verify_module(
            &module,
            &ProofBundle::default(),
            &AdmissionProfile::default(),
        )
        .expect("each alternative establishes the exact caller ceiling");
        alternatives[1] = boolean(5, true);
        alternatives.sort();
        module.machines[0].contract.requires = vec![Proposition::Disjunction(alternatives)];
        rejects_coverage(&module);
    }
}

#[test]
fn missing_opposite_foreign_or_disjunctive_requirements_do_not_prove_coverage() {
    for scalar_call in [false, true] {
        let mut disjuncts = vec![boolean(1, true), boolean(2, true)];
        disjuncts.sort();
        for requirements in [
            Vec::new(),
            vec![boolean(1, false)],
            vec![boolean(2, true)],
            vec![Proposition::Disjunction(disjuncts)],
        ] {
            let mut module = module(scalar_call);
            module.machines[0].contract.requires = requirements;
            rejects_coverage(&module);
        }
        let mut wrong_cause = module(scalar_call);
        wrong_cause.machines[0].contract.crash_routes[0].cause = CrashCause::Abort;
        rejects_coverage(&wrong_cause);
    }
}

#[test]
fn current_body_values_are_not_entry_requirement_assumptions() {
    let mut module = module(false);
    module.machines[0].contract.requires.clear();
    module.machines[0].blocks[0].operations.insert(
        0,
        Operation {
            id: OperationId::new(2).unwrap(),
            result: OperationResult::Scalar(ValueDeclaration {
                id: ValueId::new(5).unwrap(),
                scalar_type: ScalarType::Boolean,
            }),
            kind: OperationKind::BooleanConstant { value: true },
        },
    );
    rejects_coverage(&module);
}

#[test]
fn coverage_proof_cannot_replace_the_exact_callee_continuation() {
    for scalar_call in [false, true] {
        let mut module = module(scalar_call);
        let kind = &mut module.machines[0].blocks[0].operations[0].kind;
        let (OperationKind::Call {
            crash_continuations,
            ..
        }
        | OperationKind::CallUnit {
            crash_continuations,
            ..
        }) = kind
        else {
            unreachable!()
        };
        crash_continuations[0].alternatives[0] = predicate(boolean(1, true));
        assert!(
            matches!(validate_module(&module), Err(ModuleError::CallCrashContinuationsMismatch { operation, .. }) if operation == OperationId::new(1).unwrap())
        );
    }
}

fn negated_boolean(position: u64) -> Proposition {
    let negated = ScalarTerm::boolean_not(ScalarTerm::value(
        ValueId::new(position).unwrap(),
        ScalarType::Boolean,
    ))
    .unwrap();
    let mut terms = [negated, ScalarTerm::boolean(true)];
    terms.sort();
    Proposition::Equal(terms[0].clone(), terms[1].clone())
}

#[test]
fn negated_entry_requirement_covers_an_equivalently_encoded_crash_predicate() {
    for scalar_call in [false, true] {
        let mut module = module(scalar_call);
        // Requires !flag uses the false polarity of the entry value; the
        // published predicate retains the total BooleanNot expression tree.
        module.machines[0].contract.requires = vec![boolean(1, false)];
        module.machines[0].contract.crash_routes = vec![bucket(predicate(boolean(1, false)))];
        verify_module(
            &module,
            &ProofBundle::default(),
            &AdmissionProfile::default(),
        )
        .expect("the exact spelling remains a valid positive control");
        module.machines[0].contract.crash_routes = vec![bucket(predicate(negated_boolean(1)))];
        verify_module(
            &module,
            &ProofBundle::default(),
            &AdmissionProfile::default(),
        )
        .expect("equivalent Boolean denotations establish caller coverage");
    }
}

fn term_holds(term: ScalarTerm) -> Proposition {
    let mut terms = [term, ScalarTerm::boolean(true)];
    terms.sort();
    Proposition::Equal(terms[0].clone(), terms[1].clone())
}

fn verify_coverage(module: &TerminalModule) {
    verify_module(
        module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .unwrap();
}

#[test]
fn nested_negations_and_boolean_equal_wrappers_preserve_entry_polarity() {
    let flag = ScalarTerm::value(ValueId::new(1).unwrap(), ScalarType::Boolean);
    let not_flag = ScalarTerm::boolean_not(flag.clone()).unwrap();
    let double_not = ScalarTerm::boolean_not(not_flag.clone()).unwrap();
    let wrapped_not =
        ScalarTerm::boolean_equal(not_flag.clone(), ScalarTerm::boolean(true)).unwrap();
    let reversed_not = ScalarTerm::boolean_equal(ScalarTerm::boolean(true), not_flag).unwrap();
    let equals_false = ScalarTerm::boolean_equal(flag, ScalarTerm::boolean(false)).unwrap();
    for (term, entry) in [
        (double_not, true),
        (wrapped_not, false),
        (reversed_not, false),
        (equals_false, false),
    ] {
        for scalar_call in [false, true] {
            let mut checked = module(scalar_call);
            checked.machines[0].contract.requires = vec![boolean(1, entry)];
            checked.machines[0].contract.crash_routes =
                vec![bucket(predicate(term_holds(term.clone())))];
            verify_coverage(&checked);
            // Conversion applies to both premises and goals, not just routes.
            checked.machines[0].contract.requires = vec![term_holds(term.clone())];
            checked.machines[0].contract.crash_routes = vec![bucket(predicate(boolean(1, entry)))];
            verify_coverage(&checked);
        }
    }
}

#[test]
fn equivalent_spelling_does_not_supply_missing_opposite_or_foreign_assumptions() {
    for scalar_call in [false, true] {
        for requirements in [
            Vec::new(),
            vec![boolean(1, true)],
            vec![boolean(2, false)],
            vec![negated_boolean(2)],
        ] {
            let mut checked = module(scalar_call);
            checked.machines[0].contract.requires = requirements;
            checked.machines[0].contract.crash_routes = vec![bucket(predicate(negated_boolean(1)))];
            rejects_coverage(&checked);
        }
    }
}

#[test]
fn entry_disjunction_covers_the_union_of_same_cause_published_alternatives() {
    let mut disjuncts = vec![boolean(1, true), boolean(2, true)];
    disjuncts.sort();
    for scalar_call in [false, true] {
        let mut checked = module(scalar_call);
        checked.machines[0].contract.requires = vec![Proposition::Disjunction(disjuncts.clone())];
        let mut alternatives = disjuncts.iter().cloned().map(predicate).collect::<Vec<_>>();
        alternatives.sort();
        checked.machines[0].contract.crash_routes = vec![CrashRouteBucket {
            cause: CrashCause::Trap,
            alternatives,
        }];
        verify_coverage(&checked);
        checked.machines[0].contract.crash_routes = vec![bucket(predicate(boolean(1, true)))];
        rejects_coverage(&checked);
        // A known disjunct also establishes a single compound alternative.
        checked.machines[0].contract.requires = vec![boolean(1, true)];
        checked.machines[0].contract.crash_routes = vec![bucket(predicate(
            Proposition::Disjunction(disjuncts.clone()),
        ))];
        verify_coverage(&checked);
    }
}

#[test]
fn equivalent_entry_coverage_does_not_rewrite_exact_call_continuations() {
    for scalar_call in [false, true] {
        let mut checked = module(scalar_call);
        checked.machines[0].contract.requires = vec![boolean(1, false)];
        checked.machines[0].contract.crash_routes = vec![bucket(predicate(negated_boolean(1)))];
        verify_coverage(&checked);
        let mut forged = checked.clone();
        let kind = &mut forged.machines[0].blocks[0].operations[0].kind;
        let (OperationKind::Call {
            crash_continuations,
            ..
        }
        | OperationKind::CallUnit {
            crash_continuations,
            ..
        }) = kind
        else {
            unreachable!()
        };
        crash_continuations[0].alternatives = vec![predicate(negated_boolean(1))];
        assert!(
            matches!(validate_module(&forged), Err(ModuleError::CallCrashContinuationsMismatch { operation, .. }) if operation == OperationId::new(1).unwrap())
        );

        // The callee still has a structurally valid unconditional crash, but
        // changing its cause invalidates the caller's retained continuation.
        checked.machines[1].contract.crash_routes[0].cause = CrashCause::Abort;
        let Terminator::Crash { cause, .. } = &mut checked.machines[1].blocks[0].terminator else {
            unreachable!()
        };
        *cause = CrashCause::Abort;
        assert!(
            matches!(validate_module(&checked), Err(ModuleError::CallCrashContinuationsMismatch { operation, .. }) if operation == OperationId::new(1).unwrap())
        );
    }
}

#[test]
fn proving_one_alternative_does_not_reset_the_whole_union_conversion_budget() {
    for scalar_call in [false, true] {
        let mut checked = module(scalar_call);
        checked.machines[0].parameters = (10_u64..1110)
            .map(|identity| ValueDeclaration {
                id: ValueId::new(identity).unwrap(),
                scalar_type: ScalarType::Boolean,
            })
            .collect();
        let mut alternatives = (10_u64..1110)
            .map(|identity| predicate(boolean(identity, true)))
            .collect::<Vec<_>>();
        alternatives.sort();
        let CrashRouteGuard::Predicate(first) = &alternatives[0] else {
            unreachable!()
        };
        checked.machines[0].contract.requires = vec![first.proposition().clone()];
        checked.machines[0].contract.crash_routes = vec![bucket(alternatives[0].clone())];
        verify_coverage(&checked);

        // Every predicate is small and valid by itself, and the first is
        // already proved. Input traversal plus normalization of the complete
        // 1100-alternative union nevertheless exceeds the shared 4096 steps.
        checked.machines[0].contract.crash_routes[0].alternatives = alternatives;
        rejects_coverage(&checked);
    }
}

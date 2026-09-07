//! A serialized crash-site predicate is a claim, not its own proof.

use proof_admission::AdmissionProfile;
use semantic_vocabulary::{
    BlockId, ContractId, EdgeId, IntegerSign, IntegerType, MachineId, OperationId, Proposition,
    ScalarTerm, ScalarType, ValueId,
};
use terminal_psi::{
    Block, CrashCause, CrashPredicateTerm, CrashRouteBucket, CrashRouteGuard, MachineContract,
    Operation, OperationKind, OperationResult, SuccessorEdge, TerminalMachine,
    TerminalMachineResult, TerminalModule, Terminator, ValueDeclaration, VocabularyMarker,
};
use terminal_verifier::{ModuleError, ProofBundle, VerificationError, verify_module};

fn value(identity: u64) -> ValueId {
    ValueId::new(identity).unwrap()
}

fn declaration(identity: u64) -> ValueDeclaration {
    ValueDeclaration {
        id: value(identity),
        scalar_type: ScalarType::Boolean,
    }
}

fn boolean(identity: u64, expected: bool) -> Proposition {
    let mut terms = [
        ScalarTerm::value(value(identity), ScalarType::Boolean),
        ScalarTerm::boolean(expected),
    ];
    terms.sort();
    Proposition::Equal(terms[0].clone(), terms[1].clone())
}

fn crash(edge: u64, guards: Vec<Proposition>) -> Terminator {
    let mut site_guard = guards
        .into_iter()
        .map(CrashPredicateTerm::new)
        .collect::<Vec<_>>();
    site_guard.sort();
    Terminator::Crash {
        edge: EdgeId::new(edge).unwrap(),
        cause: CrashCause::Trap,
        site_guard,
        frontier_lower_bound: Vec::new(),
    }
}

fn block(identity: u64, terminator: Terminator) -> Block {
    Block {
        id: BlockId::new(identity).unwrap(),
        parameters: Vec::new(),
        operations: Vec::new(),
        terminator,
    }
}

fn successor(edge: u64, target: u64, arguments: &[u64]) -> SuccessorEdge {
    SuccessorEdge {
        edge: EdgeId::new(edge).unwrap(),
        target: BlockId::new(target).unwrap(),
        arguments: arguments.iter().copied().map(value).collect(),
        trivial_affine_discards: Vec::new(),
    }
}

fn jump(edge: u64, target: u64, arguments: &[u64]) -> Terminator {
    let successor = successor(edge, target, arguments);
    Terminator::Jump {
        edge: successor.edge,
        target: successor.target,
        arguments: successor.arguments,
        residual_affine_discards: Vec::new(),
        trivial_affine_discards: Vec::new(),
    }
}

fn module(parameter: u64, expected: bool) -> TerminalModule {
    TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: MachineId::new(1).unwrap(),
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
        machines: vec![TerminalMachine {
            id: MachineId::new(1).unwrap(),
            attachment: None,
            parameters: vec![declaration(parameter)],
            structural_parameters: Vec::new(),
            ranked_scc: None,
            result: TerminalMachineResult::Unit,
            structural_places: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: BlockId::new(1).unwrap(),
            blocks: vec![block(1, crash(1, vec![boolean(parameter, expected)]))],
            contract: MachineContract {
                id: ContractId::new(1).unwrap(),
                requires: Vec::new(),
                ensures: Vec::new(),
                crash_routes: vec![CrashRouteBucket {
                    cause: CrashCause::Trap,
                    alternatives: vec![CrashRouteGuard::Predicate(CrashPredicateTerm::new(
                        boolean(parameter, expected),
                    ))],
                }],
                outcome_specific_ensures: Vec::new(),
            },
        }],
    }
}

fn verify(module: &TerminalModule) {
    verify_module(
        module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .unwrap();
}

fn unconditional_ceiling(module: &mut TerminalModule) {
    module.machines[0].contract.crash_routes[0].alternatives = vec![CrashRouteGuard::Truth];
}

fn rejects_guard(module: &TerminalModule) {
    let error = verify_module(
        module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect_err("an asserted site predicate needs independent entry or control-flow evidence");
    assert!(
        matches!(
            error,
            VerificationError::Module(ModuleError::CrashSiteGuardUnproved { .. })
        ),
        "expected a crash-site truth failure, got {error:?}"
    );
}

#[test]
fn matching_published_bucket_does_not_prove_an_unconditional_site_guard() {
    let forged = module(1, true);
    let mut valid_control = forged.clone();
    valid_control.machines[0].contract.requires = vec![boolean(1, true)];
    verify(&valid_control);
    rejects_guard(&forged);
}

#[test]
fn exact_entry_requirements_establish_site_truth_independently_of_value_numbering() {
    for parameter in [1, 41] {
        for expected in [false, true] {
            let mut checked = module(parameter, expected);
            checked.machines[0].contract.requires = vec![boolean(parameter, expected)];
            verify(&checked);
            checked.machines[0].contract.requires = vec![boolean(parameter, !expected)];
            rejects_guard(&checked);
        }
    }
}

fn branch_module(crash_on_true: bool, merge: bool, forwarded: bool) -> TerminalModule {
    let mut checked = module(1, crash_on_true);
    // A Truth ceiling isolates site truth from the separate ceiling-coverage join.
    unconditional_ceiling(&mut checked);
    let machine = &mut checked.machines[0];
    let arguments: &[u64] = if forwarded { &[1] } else { &[] };
    machine.blocks = vec![
        block(
            1,
            Terminator::Conditional {
                condition: value(1),
                when_true: successor(1, 2, arguments),
                when_false: successor(2, 3, arguments),
            },
        ),
        block(
            2,
            Terminator::ReturnUnit {
                edge: EdgeId::new(3).unwrap(),
                trivial_affine_discards: Vec::new(),
            },
        ),
        block(
            3,
            Terminator::ReturnUnit {
                edge: EdgeId::new(4).unwrap(),
                trivial_affine_discards: Vec::new(),
            },
        ),
    ];
    if forwarded {
        machine.blocks[1].parameters = vec![declaration(11)];
        machine.blocks[2].parameters = vec![declaration(12)];
    }
    if merge {
        machine.blocks[1].terminator = jump(3, 4, if forwarded { &[11] } else { &[] });
        machine.blocks[2].terminator = jump(4, 4, if forwarded { &[12] } else { &[] });
        let mut merged = block(
            4,
            crash(
                5,
                vec![boolean(if forwarded { 13 } else { 1 }, crash_on_true)],
            ),
        );
        if forwarded {
            merged.parameters = vec![declaration(13)];
        }
        machine.blocks.push(merged);
    } else {
        let index = if crash_on_true { 1 } else { 2 };
        let parameter = if forwarded {
            if crash_on_true { 11 } else { 12 }
        } else {
            1
        };
        machine.blocks[index].terminator = crash(
            if crash_on_true { 3 } else { 4 },
            vec![boolean(parameter, crash_on_true)],
        );
    }
    checked
}

#[test]
fn actual_true_and_false_successors_prove_exact_forwarded_site_values() {
    for expected in [false, true] {
        for forwarded in [false, true] {
            let checked = branch_module(expected, false, forwarded);
            verify(&checked);
            let mut opposite = checked.clone();
            let index = if expected { 1 } else { 2 };
            let parameter = if forwarded {
                if expected { 11 } else { 12 }
            } else {
                1
            };
            opposite.machines[0].blocks[index].terminator = crash(
                if expected { 3 } else { 4 },
                vec![boolean(parameter, !expected)],
            );
            rejects_guard(&opposite);
        }
    }
}

#[test]
fn merged_predecessors_do_not_retain_one_branchs_predicate() {
    for forwarded in [false, true] {
        for expected in [false, true] {
            let forged = branch_module(expected, true, forwarded);
            let mut control = forged.clone();
            control.machines[0].blocks[3].terminator = crash(5, Vec::new());
            verify(&control);
            rejects_guard(&forged);
        }
    }
}

#[test]
fn updated_scalar_values_do_not_inherit_old_entry_predicates() {
    let mut checked = module(1, true);
    unconditional_ceiling(&mut checked);
    checked.machines[0].contract.requires = vec![boolean(1, true)];
    checked.machines[0].blocks[0].operations.push(Operation {
        id: OperationId::new(1).unwrap(),
        result: OperationResult::Scalar(declaration(2)),
        kind: OperationKind::BooleanConstant { value: false },
    });
    // Terminal SSA gives an updated mutable source binding a distinct ValueId.
    verify(&checked);
    checked.machines[0].blocks[0].terminator = crash(1, vec![boolean(2, false)]);
    verify(&checked);
    checked.machines[0].blocks[0].terminator = crash(1, vec![boolean(2, true)]);
    rejects_guard(&checked);
}

#[test]
fn truth_ceiling_does_not_authorize_an_extra_forged_site_conjunct() {
    let mut checked = module(1, true);
    unconditional_ceiling(&mut checked);
    checked.machines[0].parameters.push(declaration(2));
    checked.machines[0].contract.requires = vec![boolean(1, true)];
    verify(&checked);
    checked.machines[0].blocks[0].terminator = crash(1, vec![boolean(1, true), boolean(2, true)]);
    rejects_guard(&checked);
}

fn term_boolean(term: ScalarTerm, expected: bool) -> Proposition {
    let mut terms = [term, ScalarTerm::boolean(expected)];
    terms.sort();
    Proposition::Equal(terms[0].clone(), terms[1].clone())
}

fn derived_branch(kind: OperationKind, expected: bool) -> TerminalModule {
    let mut checked = branch_module(expected, false, false);
    checked.machines[0].blocks[0].operations = vec![Operation {
        id: OperationId::new(1).unwrap(),
        result: OperationResult::Scalar(declaration(3)),
        kind,
    }];
    let Terminator::Conditional { condition, .. } = &mut checked.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    *condition = value(3);
    checked
}

fn assert_derived_guard(mut checked: TerminalModule, expected: bool, term: ScalarTerm) {
    let index = if expected { 1 } else { 2 };
    let edge = if expected { 3 } else { 4 };
    checked.machines[0].blocks[index].terminator =
        crash(edge, vec![term_boolean(term.clone(), expected)]);
    verify(&checked);
    checked.machines[0].blocks[index].terminator = crash(edge, vec![term_boolean(term, !expected)]);
    rejects_guard(&checked);
}

#[test]
fn negated_boolean_branches_prove_encoded_predicates_not_the_opposite_polarity() {
    for expected in [false, true] {
        let checked = derived_branch(OperationKind::BooleanNot { operand: value(1) }, expected);
        let term =
            ScalarTerm::boolean_not(ScalarTerm::value(value(1), ScalarType::Boolean)).unwrap();
        assert_derived_guard(checked, expected, term);
    }
}

#[test]
fn integer_comparison_branches_prove_source_style_canonical_predicates() {
    let integer = IntegerType::new(IntegerSign::Signed, 32).unwrap();
    let scalar_type = ScalarType::Integer(integer);
    for inclusive in [false, true] {
        for expected in [false, true] {
            let kind = if inclusive {
                OperationKind::IntegerLessOrEqual {
                    left: value(1),
                    right: value(2),
                }
            } else {
                OperationKind::IntegerLessThan {
                    left: value(1),
                    right: value(2),
                }
            };
            let mut checked = derived_branch(kind, expected);
            checked.machines[0].parameters = [1, 2]
                .map(|identity| ValueDeclaration {
                    id: value(identity),
                    scalar_type,
                })
                .to_vec();
            let left = ScalarTerm::value(value(1), scalar_type);
            let right = ScalarTerm::value(value(2), scalar_type);
            let term = if inclusive {
                ScalarTerm::integer_less_or_equal(integer, left, right)
            } else {
                ScalarTerm::integer_less_than(integer, left, right)
            }
            .unwrap();
            assert_derived_guard(checked, expected, term);
        }
    }
}

#[test]
fn computed_condition_join_preserves_only_feasible_entry_predicate_paths() {
    let mut checked = branch_module(false, true, false);
    let machine = &mut checked.machines[0];
    machine.contract.crash_routes[0].alternatives = vec![CrashRouteGuard::Predicate(
        CrashPredicateTerm::new(boolean(1, false)),
    )];
    for (index, constant) in [(1, true), (2, false)] {
        let result = index as u64 + 1;
        machine.blocks[index].operations = vec![Operation {
            id: OperationId::new(index as u64).unwrap(),
            result: OperationResult::Scalar(declaration(result)),
            kind: OperationKind::BooleanConstant { value: constant },
        }];
        machine.blocks[index].terminator = jump(index as u64 + 2, 4, &[result]);
    }
    machine.blocks[3].parameters = vec![declaration(4)];
    machine.blocks[3].terminator = Terminator::Conditional {
        condition: value(4),
        when_true: successor(5, 5, &[]),
        when_false: successor(6, 6, &[]),
    };
    machine.blocks.extend([
        block(
            5,
            Terminator::ReturnUnit {
                edge: EdgeId::new(7).unwrap(),
                trivial_affine_discards: Vec::new(),
            },
        ),
        block(6, crash(8, vec![boolean(1, false)])),
    ]);
    // The true-entry path supplies true to the join, so only false entry can
    // reach the crash. Merely intersecting facts at block 4 loses this relation.
    verify(&checked);

    // Same IDs, edges, and predicate: changing just this actual computation
    // makes the true-entry path reach the crash too. It is no longer infeasible.
    checked.machines[0].blocks[1].operations[0].kind =
        OperationKind::BooleanConstant { value: false };
    rejects_guard(&checked);
}

#[test]
fn repeated_diamonds_fail_closed_at_the_crash_reconstruction_work_limit() {
    let mut checked = module(1, true);
    let machine = &mut checked.machines[0];
    machine.blocks.clear();
    // Forty blocks encode 8192 syntactic paths. Reusing one flag keeps the
    // graph small; the current bounded traversal does not prune infeasible
    // prefixes, and must report exhaustion rather than trust the site guard.
    for diamond in 0_u64..13 {
        let entry = diamond * 3 + 1;
        let edge = diamond * 4 + 1;
        machine.blocks.extend([
            block(
                entry,
                Terminator::Conditional {
                    condition: value(1),
                    when_true: successor(edge, entry + 1, &[]),
                    when_false: successor(edge + 1, entry + 2, &[]),
                },
            ),
            block(entry + 1, jump(edge + 2, entry + 3, &[])),
            block(entry + 2, jump(edge + 3, entry + 3, &[])),
        ]);
    }
    machine
        .blocks
        .push(block(40, crash(53, vec![boolean(1, true)])));
    let error = verify_module(
        &checked,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect_err("bounded crash-path reconstruction must stop before accepting a guard");
    assert_eq!(
        error,
        VerificationError::Module(ModuleError::CrashSiteReconstructionLimitExceeded(
            MachineId::new(1).unwrap()
        ))
    );
}

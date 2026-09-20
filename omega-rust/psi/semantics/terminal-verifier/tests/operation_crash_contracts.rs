//! Operation-level crash contracts substitute the operation's operands and
//! then need caller coverage, exactly like a call's continuations.

use semantic_vocabulary::{
    BlockId, ContractId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId, OperationId,
    Proposition, PropositionError, ScalarTerm, ScalarType, ValueId,
};
use terminal_psi::{
    Block, CrashCause, CrashPredicateTerm, CrashRouteBucket, CrashRouteGuard, MachineContract,
    Operation, OperationKind, OperationResult, TerminalMachine, TerminalMachineResult,
    TerminalModule, TerminalOperationCrashContract, Terminator, ValueDeclaration, VocabularyMarker,
};
use terminal_verifier::{ModuleError, validate_module};

const LEFT: u64 = 10;
const RIGHT: u64 = 20;
const COMPARISON: u64 = 30;

fn id<T>(raw: u64, constructor: impl FnOnce(u64) -> Option<T>) -> T {
    constructor(raw).expect("nonzero fixture identity")
}

fn i32_type() -> IntegerType {
    IntegerType::new(IntegerSign::Signed, 32).expect("i32")
}

fn integer(raw: u64) -> ScalarTerm {
    ScalarTerm::value(id(raw, ValueId::new), ScalarType::Integer(i32_type()))
}

fn negative(value: u64) -> Proposition {
    Proposition::LessThan(
        integer(value),
        ScalarTerm::integer(i32_type(), IntegerValue::Signed(0)).unwrap(),
    )
}

fn nonnegative(value: u64) -> Proposition {
    Proposition::LessOrEqual(
        ScalarTerm::integer(i32_type(), IntegerValue::Signed(0)).unwrap(),
        integer(value),
    )
}

fn guarded(cause: CrashCause, proposition: Proposition) -> CrashRouteBucket {
    CrashRouteBucket {
        cause,
        alternatives: vec![CrashRouteGuard::Predicate(CrashPredicateTerm::new(
            proposition,
        ))],
    }
}

fn unconditional(cause: CrashCause) -> CrashRouteBucket {
    CrashRouteBucket {
        cause,
        alternatives: vec![CrashRouteGuard::Truth],
    }
}

fn declaration(raw: u64, scalar_type: ScalarType) -> ValueDeclaration {
    ValueDeclaration {
        qualifications: Default::default(),
        id: id(raw, ValueId::new),
        scalar_type,
    }
}

/// `compare(left: i32, right: i32) -> bool { left == right }` where the
/// selected equality operator publishes `crashes Trap right < 0` in its own
/// formal namespace (formal 2 is `right`), and the caller publishes the
/// substituted route over its actual `right` parameter.
fn module() -> TerminalModule {
    let integer_type = ScalarType::Integer(i32_type());
    TerminalModule {
        scalar_qualifications: Default::default(),
        scalar_block_invariants: Vec::new(),
        operation_crash_contracts: vec![TerminalOperationCrashContract {
            machine: id(1, MachineId::new),
            operation: id(1, OperationId::new),
            published_routes: vec![guarded(CrashCause::Trap, negative(2))],
            crash_continuations: vec![guarded(CrashCause::Trap, negative(RIGHT))],
        }],
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: id(1, MachineId::new),
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
            closed_reach_application: None,
            declared_service_reach: Vec::new(),
            id: id(1, MachineId::new),
            attachment: None,
            parameters: vec![
                declaration(LEFT, integer_type),
                declaration(RIGHT, integer_type),
            ],
            structural_parameters: Vec::new(),
            ranked_scc: None,
            result: TerminalMachineResult::Scalar(declaration(40, ScalarType::Boolean)),
            structural_places: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: id(1, BlockId::new),
            blocks: vec![Block {
                erased_scalar_formals: Vec::new(),
                erased_proof_formals: Vec::new(),
                structural_parameters: Vec::new(),
                id: id(1, BlockId::new),
                parameters: Vec::new(),
                operations: vec![
                    Operation {
                        static_reach_binding: None,
                        id: id(1, OperationId::new),
                        result: OperationResult::Scalar(declaration(
                            COMPARISON,
                            ScalarType::Boolean,
                        )),
                        kind: OperationKind::IntegerEqual {
                            left: id(LEFT, ValueId::new),
                            right: id(RIGHT, ValueId::new),
                        },
                    },
                    Operation {
                        static_reach_binding: None,
                        id: id(2, OperationId::new),
                        result: OperationResult::Scalar(declaration(50, integer_type)),
                        kind: OperationKind::IntegerConstant {
                            value: IntegerValue::Signed(0),
                        },
                    },
                ],
                terminator: Terminator::Return {
                    cleanup_actions: Vec::new(),
                    edge: id(1, EdgeId::new),
                    value: id(COMPARISON, ValueId::new),
                },
            }],
            contract: MachineContract {
                erased_scalar_formals: Vec::new(),
                erased_proof_formals: Vec::new(),
                id: id(1, ContractId::new),
                crash_routes: vec![guarded(CrashCause::Trap, negative(RIGHT))],
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    }
}

fn contract(module: &mut TerminalModule) -> &mut TerminalOperationCrashContract {
    &mut module.operation_crash_contracts[0]
}

fn row_error(module: &TerminalModule) -> ModuleError {
    validate_module(module).expect_err("the operation crash contract must reject")
}

#[test]
fn operation_contract_substitutes_formals_with_the_operations_operands() {
    validate_module(&module())
        .expect("formal 2 becomes the right operand and the caller publishes that route");
}

#[test]
fn continuations_must_equal_the_exact_operand_substitution() {
    let mismatch = ModuleError::OperationCrashContinuationsMismatch {
        machine: id(1, MachineId::new),
        operation: id(1, OperationId::new),
    };
    // The formal namespace never survives into the machine's value namespace.
    let mut unsubstituted = module();
    contract(&mut unsubstituted).crash_continuations = vec![guarded(CrashCause::Trap, negative(2))];
    assert_eq!(row_error(&unsubstituted), mismatch);
    // Formal 2 is the second operand, not the first.
    let mut swapped = module();
    contract(&mut swapped).crash_continuations = vec![guarded(CrashCause::Trap, negative(LEFT))];
    assert_eq!(row_error(&swapped), mismatch);
    // An empty roster cannot erase a published crash.
    let mut erased = module();
    contract(&mut erased).crash_continuations.clear();
    assert_eq!(row_error(&erased), mismatch);
    // Neither can a widened continuation.
    let mut widened = module();
    contract(&mut widened).crash_continuations = vec![unconditional(CrashCause::Trap)];
    assert_eq!(row_error(&widened), mismatch);
    // Nor a changed cause.
    let mut recaused = module();
    contract(&mut recaused).crash_continuations = vec![guarded(CrashCause::Abort, negative(RIGHT))];
    assert_eq!(row_error(&recaused), mismatch);
}

#[test]
fn substituted_continuations_need_same_cause_caller_coverage() {
    let uncovered = ModuleError::CallCrashContinuationUncovered {
        operation: id(1, OperationId::new),
        cause: CrashCause::Trap,
    };
    let mut silent = module();
    silent.machines[0].contract.crash_routes.clear();
    assert_eq!(row_error(&silent), uncovered);
    let mut other_operand = module();
    other_operand.machines[0].contract.crash_routes =
        vec![guarded(CrashCause::Trap, negative(LEFT))];
    assert_eq!(row_error(&other_operand), uncovered);
    let mut other_cause = module();
    other_cause.machines[0].contract.crash_routes =
        vec![guarded(CrashCause::Abort, negative(RIGHT))];
    assert_eq!(row_error(&other_cause), uncovered);
    let mut ceiling = module();
    ceiling.machines[0].contract.crash_routes = vec![unconditional(CrashCause::Trap)];
    validate_module(&ceiling).expect("an unconditional caller route covers the guarded one");
}

#[test]
fn entry_requirements_disprove_exact_substituted_continuations() {
    let mut safe = module();
    safe.machines[0].contract.crash_routes.clear();
    safe.machines[0].contract.requires = vec![Proposition::LessOrEqual(
        ScalarTerm::integer(i32_type(), IntegerValue::Signed(0)).unwrap(),
        integer(RIGHT),
    )];
    validate_module(&safe).expect("the exact right operand cannot be negative");
    let retained = safe.operation_crash_contracts.clone();
    assert!(!retained[0].published_routes.is_empty());
    assert!(!retained[0].crash_continuations.is_empty());

    let mut wrong_operand = safe.clone();
    wrong_operand.machines[0].contract.requires = vec![Proposition::LessOrEqual(
        ScalarTerm::integer(i32_type(), IntegerValue::Signed(0)).unwrap(),
        integer(LEFT),
    )];
    assert!(matches!(
        row_error(&wrong_operand),
        ModuleError::CallCrashContinuationUncovered { .. }
    ));
    let mut wrong_polarity = safe.clone();
    wrong_polarity.machines[0].contract.requires = vec![negative(RIGHT)];
    assert!(matches!(
        row_error(&wrong_polarity),
        ModuleError::CallCrashContinuationUncovered { .. }
    ));
    let mut missing = safe.clone();
    missing.machines[0].contract.requires.clear();
    assert!(matches!(
        row_error(&missing),
        ModuleError::CallCrashContinuationUncovered { .. }
    ));
    assert_eq!(safe.operation_crash_contracts, retained);
}

#[test]
fn each_alternative_needs_coverage_or_disproof() {
    let mut checked = module();
    let mut published = [negative(1), negative(2)]
        .map(|predicate| CrashRouteGuard::Predicate(CrashPredicateTerm::new(predicate)))
        .to_vec();
    published.sort();
    contract(&mut checked).published_routes[0].alternatives = published;
    let mut actuals = [negative(LEFT), negative(RIGHT)]
        .map(|predicate| CrashRouteGuard::Predicate(CrashPredicateTerm::new(predicate)))
        .to_vec();
    actuals.sort();
    contract(&mut checked).crash_continuations[0].alternatives = actuals;
    checked.machines[0].contract.crash_routes.clear();
    checked.machines[0].contract.requires = vec![nonnegative(LEFT), nonnegative(RIGHT)];
    validate_module(&checked).expect("both alternatives are disproved");
    checked.machines[0].contract.requires = vec![nonnegative(RIGHT)];
    assert!(matches!(
        row_error(&checked),
        ModuleError::CallCrashContinuationUncovered { .. }
    ));
    checked.machines[0].contract.crash_routes = vec![guarded(CrashCause::Trap, negative(LEFT))];
    validate_module(&checked).expect("left is covered and right is disproved");
    checked.machines[0].contract.crash_routes = vec![guarded(CrashCause::Abort, negative(LEFT))];
    assert!(matches!(
        row_error(&checked),
        ModuleError::CallCrashContinuationUncovered { .. }
    ));
}

#[test]
fn current_body_value_cannot_borrow_an_entry_disproof() {
    let mut checked = module();
    checked.machines[0].contract.crash_routes.clear();
    checked.machines[0].contract.requires = vec![nonnegative(RIGHT)];
    let comparison = &mut checked.machines[0].blocks[0].operations[0].kind;
    let OperationKind::IntegerEqual { right, .. } = comparison else {
        panic!("comparison");
    };
    *right = id(50, ValueId::new);
    // The existing constant is an ordinary SSA body value, not formal RIGHT.
    // Even its known zero must not be inferred from unrelated entry facts.
    checked.machines[0].blocks[0].operations.swap(0, 1);
    contract(&mut checked).crash_continuations = vec![guarded(CrashCause::Trap, negative(50))];
    assert!(matches!(
        row_error(&checked),
        ModuleError::CallCrashContinuationUncovered { .. }
    ));
}

#[test]
fn compound_route_disproof_preserves_boolean_connectives() {
    for conjunction in [false, true] {
        let mut checked = module();
        let combine = |mut children: Vec<Proposition>| {
            children.sort();
            if conjunction {
                Proposition::Conjunction(children)
            } else {
                Proposition::Disjunction(children)
            }
        };
        contract(&mut checked).published_routes = vec![guarded(
            CrashCause::Trap,
            combine(vec![negative(1), negative(2)]),
        )];
        contract(&mut checked).crash_continuations = vec![guarded(
            CrashCause::Trap,
            combine(vec![negative(LEFT), negative(RIGHT)]),
        )];
        checked.machines[0].contract.crash_routes.clear();
        checked.machines[0].contract.requires = vec![nonnegative(RIGHT)];
        if conjunction {
            validate_module(&checked).expect("one false conjunct disproves the route");
        } else {
            assert!(matches!(
                row_error(&checked),
                ModuleError::CallCrashContinuationUncovered { .. }
            ));
        }
        checked.machines[0]
            .contract
            .requires
            .push(nonnegative(LEFT));
        validate_module(&checked).expect("both negative operands are impossible");
    }
}

#[test]
fn entry_disproof_requires_the_same_formal_on_every_incoming_edge() {
    let mut checked = module();
    let machine = &mut checked.machines[0];
    machine.contract.crash_routes.clear();
    machine.contract.requires = vec![nonnegative(RIGHT)];
    machine
        .parameters
        .push(declaration(70, ScalarType::Boolean));
    let mut call_block = machine.blocks.remove(0);
    call_block.id = id(2, BlockId::new);
    call_block.parameters = vec![declaration(60, ScalarType::Integer(i32_type()))];
    let OperationKind::IntegerEqual { right, .. } = &mut call_block.operations[0].kind else {
        panic!("comparison");
    };
    *right = id(60, ValueId::new);
    let successor = |edge| terminal_psi::SuccessorEdge {
        edge: id(edge, EdgeId::new),
        target: id(2, BlockId::new),
        arguments: vec![id(RIGHT, ValueId::new)],
        structural_arguments: Vec::new(),
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    machine.blocks = vec![
        Block {
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            structural_parameters: Vec::new(),
            id: id(1, BlockId::new),
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::Conditional {
                condition: id(70, ValueId::new),
                when_true: successor(2),
                when_false: successor(3),
            },
        },
        call_block,
    ];
    contract(&mut checked).crash_continuations = vec![guarded(CrashCause::Trap, negative(60))];
    validate_module(&checked).expect("both arrivals forward the exact right formal");
    let Terminator::Conditional { when_false, .. } = &mut checked.machines[0].blocks[0].terminator
    else {
        panic!("conditional");
    };
    when_false.arguments[0] = id(LEFT, ValueId::new);
    assert!(matches!(
        row_error(&checked),
        ModuleError::CallCrashContinuationUncovered { .. }
    ));
}

#[test]
fn unconditional_published_routes_survive_unconditionally() {
    let mut module = module();
    contract(&mut module).published_routes = vec![unconditional(CrashCause::Trap)];
    contract(&mut module).crash_continuations = vec![unconditional(CrashCause::Trap)];
    assert_eq!(
        row_error(&module),
        ModuleError::CallCrashContinuationUncovered {
            operation: id(1, OperationId::new),
            cause: CrashCause::Trap,
        }
    );
    module.machines[0].contract.crash_routes = vec![unconditional(CrashCause::Trap)];
    validate_module(&module).unwrap();
}

#[test]
fn rows_name_an_existing_operation_with_positional_scalar_operands() {
    let mut unknown_machine = module();
    contract(&mut unknown_machine).machine = id(9, MachineId::new);
    assert_eq!(
        row_error(&unknown_machine),
        ModuleError::InvalidOperationCrashContract {
            machine: id(9, MachineId::new),
            operation: id(1, OperationId::new),
        }
    );
    let mut unknown_operation = module();
    contract(&mut unknown_operation).operation = id(9, OperationId::new);
    assert_eq!(
        row_error(&unknown_operation),
        ModuleError::InvalidOperationCrashContract {
            machine: id(1, MachineId::new),
            operation: id(9, OperationId::new),
        }
    );
    // A constant has no operand roster for the formal telescope to bind.
    let mut constant = module();
    contract(&mut constant).operation = id(2, OperationId::new);
    assert_eq!(
        row_error(&constant),
        ModuleError::UnsupportedOperationCrashContractOperation {
            machine: id(1, MachineId::new),
            operation: id(2, OperationId::new),
        }
    );
}

#[test]
fn published_routes_are_nonempty_canonical_and_scalar_over_the_formal_telescope() {
    let noncanonical = ModuleError::NonCanonicalOperationCrashContractRoutes {
        machine: id(1, MachineId::new),
        operation: id(1, OperationId::new),
    };
    let mut empty = module();
    contract(&mut empty).published_routes.clear();
    contract(&mut empty).crash_continuations.clear();
    assert_eq!(row_error(&empty), noncanonical);
    let mut truth_predicate = module();
    contract(&mut truth_predicate).published_routes =
        vec![guarded(CrashCause::Trap, Proposition::Truth)];
    assert_eq!(row_error(&truth_predicate), noncanonical);
    let mut misordered = module();
    contract(&mut misordered).published_routes = vec![
        unconditional(CrashCause::Abort),
        guarded(CrashCause::Trap, negative(2)),
    ];
    assert_eq!(row_error(&misordered), noncanonical);
    // Formal 3 names no operand: the equality has exactly two.
    let mut out_of_range = module();
    contract(&mut out_of_range).published_routes = vec![guarded(CrashCause::Trap, negative(3))];
    assert!(matches!(
        row_error(&out_of_range),
        ModuleError::MalformedProposition(PropositionError::UnknownValue(value))
            if value == id(3, ValueId::new)
    ));
    // Formals are typed by their operand: a Boolean equation over an i32
    // operand is malformed even though the identity exists.
    let mut mistyped = module();
    contract(&mut mistyped).published_routes = vec![guarded(
        CrashCause::Trap,
        Proposition::Equal(
            ScalarTerm::value(id(2, ValueId::new), ScalarType::Boolean),
            ScalarTerm::boolean(true),
        ),
    )];
    assert!(matches!(
        row_error(&mistyped),
        ModuleError::MalformedProposition(_)
    ));
}

#[test]
fn rows_are_strictly_ordered_by_machine_and_operation() {
    let mut duplicated = module();
    let row = contract(&mut duplicated).clone();
    duplicated.operation_crash_contracts.push(row);
    assert_eq!(
        row_error(&duplicated),
        ModuleError::NonCanonicalOperationCrashContracts
    );
}

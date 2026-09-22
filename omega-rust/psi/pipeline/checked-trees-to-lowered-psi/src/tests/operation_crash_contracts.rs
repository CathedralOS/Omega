//! Checked operator crash sites lower to Terminal operation crash contracts
//! at the exact emitted operation, and every crash-qualified use the closure
//! cannot carry that way fails closed.

use crate::TerminalMachineSelection;
use std::collections::BTreeMap;

use super::checked_source;
use crate::lower_machine;
use crate::lowering_error::LoweringError;
use crate::proofs::crash_routes::lower_formal_crash_routes;
use semantic_vocabulary::{
    IntegerSign, IntegerType, IntegerValue, Proposition, ScalarTerm, ScalarType, ValueId,
};
use terminal_psi::{
    Block, CrashCause, CrashPredicateTerm, CrashRouteBucket, CrashRouteGuard, Operation,
    OperationKind, TerminalOperationCrashContract,
};

/// Omega separately rejoins these opaque commitments to actual selected
/// ProviderPlans; this unit boundary tests only source-to-Terminal custody.
fn checked_with_provider_commitments(source: &str) -> checked_trees::CheckedTrees {
    let mut checked = checked_source(source);
    let handles = checked
        .facts
        .operators
        .uses
        .iter()
        .map(|(handle, _)| handle)
        .collect::<Vec<_>>();
    for handle in handles {
        let selected = checked.facts.operators.uses.get_mut(handle);
        selected.provider_plan_report_fingerprint = 7;
        selected.provider_plan_commitment =
            checked_trees::CheckedProviderPlanCommitment::from_digest([7; 32]);
    }
    checked
}

fn unconditional(cause: CrashCause) -> CrashRouteBucket {
    CrashRouteBucket {
        cause,
        alternatives: vec![CrashRouteGuard::Truth],
    }
}

fn crash_qualified_float_comparison(cause: &str) -> checked_trees::CheckedTrees {
    checked_with_provider_commitments(&format!(
        "boundary operator == Float::equal(left: f64, right: f64) -> bool crashes {cause};
         machine compare(left: f64, right: f64) -> bool crashes {cause} {{ left == right }}"
    ))
}

fn i32_type() -> IntegerType {
    IntegerType::new(IntegerSign::Signed, 32).expect("i32")
}

/// The authored guard `!(right >= 0)` as the checked stage structures it over
/// an i32 `right`: `!(0 <= right) == true`, with `Proposition::Equal` operands
/// in canonical order.
fn guarded_trap_route(right: ScalarTerm) -> CrashRouteBucket {
    let i32_type = i32_type();
    let mut left = ScalarTerm::boolean_not(
        ScalarTerm::integer_less_or_equal(
            i32_type,
            ScalarTerm::integer(i32_type, IntegerValue::Signed(0)).expect("literal 0"),
            right,
        )
        .expect("0 <= right"),
    )
    .expect("!(0 <= right)");
    let mut right = ScalarTerm::boolean(true);
    if left > right {
        std::mem::swap(&mut left, &mut right);
    }
    CrashRouteBucket {
        cause: CrashCause::Trap,
        alternatives: vec![CrashRouteGuard::Predicate(CrashPredicateTerm::new(
            Proposition::Equal(left, right),
        ))],
    }
}

/// One guarded integer `boundary operator` route used by a crash-qualified
/// caller: the shape `operators/crash_routes` authors for its `may_crash`.
const GUARDED_INTEGER_OPERATOR_SOURCE: &str =
    "boundary operator == Comparison::equal(left: i32, right: i32) -> bool
     crashes Trap !(right >= 0);
     pub machine compare(left: i32, right: i32) -> bool crashes Trap { left == right }";

/// The same guarded route on a spelling whose emission reverses the authored
/// operands: `>` emits `IntegerLessThan` over `(right, left)`.
const GUARDED_REORDERED_INTEGER_OPERATOR_SOURCE: &str =
    "boundary operator > Comparison::greater(left: i32, right: i32) -> bool
     crashes Trap !(right >= 0);
     pub machine compare(left: i32, right: i32) -> bool crashes Trap { left > right }";

/// The same guarded route on a spelling whose emission negates the authored
/// comparison: `!=` emits `IntegerEqual` and one `BooleanNot` over its result.
const GUARDED_NEGATED_INTEGER_OPERATOR_SOURCE: &str =
    "boundary operator != Comparison::not_equal(left: i32, right: i32) -> bool
     crashes Trap !(right >= 0);
     pub machine compare(left: i32, right: i32) -> bool crashes Trap { left != right }";

/// The block and operation one crash contract row names.
fn joined_operation<'lowered>(
    lowered: &'lowered lowered_psi::LoweredPsi,
    row: &TerminalOperationCrashContract,
) -> (&'lowered Block, &'lowered Operation) {
    lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == row.machine)
        .expect("the row names the lowered machine")
        .blocks
        .iter()
        .find_map(|block| {
            block
                .operations
                .iter()
                .find(|operation| operation.id == row.operation)
                .map(|operation| (block, operation))
        })
        .expect("the row names an emitted operation")
}

#[test]
fn selected_operator_crash_site_lowers_to_one_row_at_the_emitted_comparison() {
    for cause in [CrashCause::Trap, CrashCause::Abort] {
        let checked = crash_qualified_float_comparison(match cause {
            CrashCause::Trap => "Trap",
            CrashCause::Abort => "Abort",
        });
        let lowered = lower_machine(&checked, TerminalMachineSelection::Name("compare"))
            .expect("a joined selected comparison carries its crash contract");
        let [occurrence] = lowered
            .selected_ieee_float_comparison_occurrences
            .as_slice()
        else {
            panic!("one selected comparison occurrence");
        };
        let [row] = lowered.semantic_module.operation_crash_contracts.as_slice() else {
            panic!("one operation crash contract row");
        };
        assert_eq!(row.machine, occurrence.terminal_machine);
        assert_eq!(row.operation, occurrence.terminal_operation);
        assert_eq!(row.published_routes, vec![unconditional(cause)]);
        assert_eq!(row.crash_continuations, vec![unconditional(cause)]);
        terminal_verifier::validate_module(&lowered.semantic_module)
            .expect("the verifier accepts the produced row");
    }
}

#[test]
fn the_row_carries_the_declaration_local_formal_telescope() {
    // Two f64 operands: formals 1 and 2 typed by the operands. The published
    // roster is unconditional here, so the telescope is exercised through the
    // verifier's own reconstruction of the continuations: a continuation over
    // the formal namespace, or over swapped operands, is a verifier rejection.
    let checked = crash_qualified_float_comparison("Trap");
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("compare")).unwrap();
    let mut forged = lowered.semantic_module.clone();
    let row = &mut forged.operation_crash_contracts[0];
    let formal = |raw| {
        CrashRouteGuard::Predicate(CrashPredicateTerm::new(Proposition::Equal(
            ScalarTerm::value(ValueId::new(raw).unwrap(), ScalarType::Boolean),
            ScalarTerm::boolean(true),
        )))
    };
    row.crash_continuations = vec![CrashRouteBucket {
        cause: CrashCause::Trap,
        alternatives: vec![formal(1)],
    }];
    assert!(matches!(
        terminal_verifier::validate_module(&forged),
        Err(terminal_verifier::ModuleError::OperationCrashContinuationsMismatch { .. })
    ));
}

#[test]
fn a_recaused_site_roster_fails_closed_at_verification() {
    // Operator declarations own no checked machine contract plan of their
    // own, so the site's retained roster is the requirement carrier. A site
    // whose cause no longer matches what the caller publishes produces a row
    // the verifier rejects, and lowering fails closed on that rejection.
    let mut checked = crash_qualified_float_comparison("Trap");
    lower_machine(&checked, TerminalMachineSelection::Name("compare"))
        .expect("the retained site lowers");
    let compare = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "compare")
        .expect("compare machine")
        .symbol;
    let plan = checked
        .facts
        .contract_plans
        .machines
        .iter_mut()
        .find(|plan| plan.machine == compare)
        .expect("compare contract plan");
    let mut sites = plan.crash.checked_operators().to_vec();
    assert_eq!(sites.len(), 1);
    sites[0].published = vec![checked_trees::CrashRouteBucket::unconditional(
        checked_trees::CrashCause::Abort,
    )];
    plan.crash = plan
        .crash
        .clone()
        .with_checked_operators(sites)
        .expect("one site identity");
    // The lowered module stays structurally valid; the producer's own
    // certificate replay finds no supply for the uncovered continuation, so
    // lowering refuses to attach a roster a receiver would reject.
    assert!(matches!(
        lower_machine(&checked, TerminalMachineSelection::Name("compare")),
        Err(LoweringError::UndischargedCrashObligations(owners))
            if owners.iter().any(|owner| matches!(
                owner,
                terminal_psi::CrashObligationOwner::Continuation {
                    cause: CrashCause::Abort,
                    ..
                }
            ))
    ));
}

#[test]
fn a_guarded_operator_route_lowers_to_the_guard_proposition_the_verifier_accepts() {
    // The checked stage attaches the guard's structured scalar form over the
    // operator's own formals, and the producer's formal-telescope lowering
    // turns it into the published proposition (`right` is formal 2). This
    // witness isolates that route lowering from emission: the real checked
    // site's routes lower through the producer's own route lowering, and the
    // row is installed by hand at the `IntegerEqual` operation of an
    // independently lowered builtin integer comparison to show the verifier
    // accepts its recomputed continuation. The sibling below goes through
    // `lower_machine` end to end.
    let checked = checked_source(GUARDED_INTEGER_OPERATOR_SOURCE);
    let compare = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "compare")
        .expect("compare machine")
        .symbol;
    let plan = checked
        .facts
        .contract_plans
        .for_machine(compare)
        .expect("compare contract plan");
    let [site] = plan.crash.checked_operators() else {
        panic!("one checked operator crash site");
    };
    let integer = ScalarType::Integer(i32_type());
    let published = lower_formal_crash_routes(site.published(), &[integer, integer])
        .expect("the structured guard lowers through the formal telescope");
    let formal = |raw| ScalarTerm::value(ValueId::new(raw).expect("formal"), integer);
    assert_eq!(published, vec![guarded_trap_route(formal(2))]);

    let host = checked_source(
        "pub machine compare(left: i32, right: i32) -> bool crashes Trap { left == right }",
    );
    let lowered = lower_machine(&host, TerminalMachineSelection::Name("compare"))
        .expect("an integer comparison lowers");
    let mut module = lowered.semantic_module.clone();
    let (machine, operation, left, right) = module
        .machines
        .iter()
        .flat_map(|machine| {
            machine.blocks.iter().flat_map(move |block| {
                block
                    .operations
                    .iter()
                    .map(move |operation| (machine, operation))
            })
        })
        .find_map(|(machine, operation)| match operation.kind {
            OperationKind::IntegerEqual { left, right } => {
                Some((machine.id, operation.id, left, right))
            }
            _ => None,
        })
        .expect("the host emits one integer equality");
    let substitutions = BTreeMap::from([
        (ValueId::new(1).unwrap(), ScalarTerm::value(left, integer)),
        (ValueId::new(2).unwrap(), ScalarTerm::value(right, integer)),
    ]);
    let crash_continuations =
        terminal_verifier::substitute_crash_routes(&published, &substitutions);
    assert_eq!(
        crash_continuations,
        vec![guarded_trap_route(ScalarTerm::value(right, integer))]
    );
    module.operation_crash_contracts = vec![terminal_psi::TerminalOperationCrashContract {
        machine,
        operation,
        published_routes: published,
        crash_continuations,
    }];
    terminal_verifier::validate_module(&module)
        .expect("the verifier accepts the guarded row and its recomputed continuation");
}

#[test]
fn a_guarded_integer_operator_route_lowers_end_to_end_to_the_row_the_verifier_accepts() {
    // The selected integer comparison now records an emitted-operation join
    // (`selected_integer_comparison_occurrences`, keyed by the same checked
    // `operator_use` the crash site names), so the checked site's guarded
    // route reaches a producer-written row at the `IntegerEqual` operation:
    // published `!(0 <= formal 2)`, continuation over the operation's own
    // right operand, and the verifier accepts the module `lower_machine`
    // produced.
    let checked = checked_with_provider_commitments(GUARDED_INTEGER_OPERATOR_SOURCE);
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("compare"))
        .expect("a joined guarded integer comparison carries its crash contract");
    assert!(
        lowered
            .selected_ieee_float_comparison_occurrences
            .is_empty()
    );
    let [occurrence] = lowered.selected_integer_comparison_occurrences.as_slice() else {
        panic!("one selected integer comparison occurrence");
    };
    assert_eq!(
        occurrence.comparison,
        lowered_psi::LoweredSelectedIntegerComparisonOperation::Equal
    );
    assert_eq!(occurrence.integer_type, i32_type());
    let [row] = lowered.semantic_module.operation_crash_contracts.as_slice() else {
        panic!("one operation crash contract row");
    };
    assert_eq!(row.machine, occurrence.terminal_machine);
    assert_eq!(row.operation, occurrence.terminal_operation);
    let machine = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == row.machine)
        .expect("the row names the lowered machine");
    let (block, operation) = machine
        .blocks
        .iter()
        .find_map(|block| {
            block
                .operations
                .iter()
                .find(|operation| operation.id == row.operation)
                .map(|operation| (block, operation))
        })
        .expect("the row names an emitted operation");
    let OperationKind::IntegerEqual { left, right } = operation.kind else {
        panic!("the joined operation is the emitted integer equality");
    };
    // Scalar-graph sequencing completes `left` then `right` as the trailing
    // parameters of the comparison's own block, so the operation's positional
    // roster is the authored operand order the formal telescope reads.
    let [.., authored_left, authored_right] = block.parameters.as_slice() else {
        panic!("the comparison block carries both completed operands");
    };
    assert_eq!((left, right), (authored_left.id, authored_right.id));
    let integer = ScalarType::Integer(i32_type());
    let formal = |raw| ScalarTerm::value(ValueId::new(raw).expect("formal"), integer);
    assert_eq!(row.published_routes, vec![guarded_trap_route(formal(2))]);
    assert_eq!(
        row.crash_continuations,
        vec![guarded_trap_route(ScalarTerm::value(right, integer))]
    );
    terminal_verifier::validate_module(&lowered.semantic_module)
        .expect("the verifier accepts the produced guarded row");
}

#[test]
fn a_reordered_greater_route_publishes_the_operations_own_formal_telescope() {
    // `>` emits `IntegerLessThan` over the reversed pair, so the operator's
    // authored formal 2 (`right`) is the operation's operand position 0. The
    // producer reindexes the published routes through the mapping the
    // occurrence recorded, so the verifier's positional substitution — which
    // binds formal 1 to operand position 0 — reconstructs the guard over the
    // authored `right` without the positional rule being relaxed.
    let checked = checked_with_provider_commitments(GUARDED_REORDERED_INTEGER_OPERATOR_SOURCE);
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("compare"))
        .expect("a reordered guarded integer comparison carries its crash contract");
    let [occurrence] = lowered.selected_integer_comparison_occurrences.as_slice() else {
        panic!("one selected integer comparison occurrence");
    };
    assert_eq!(
        occurrence.comparison,
        lowered_psi::LoweredSelectedIntegerComparisonOperation::LessThan
    );
    assert_eq!(
        occurrence.operand_order,
        lowered_psi::LoweredSelectedIntegerComparisonOperandOrder::Swapped
    );
    assert!(!occurrence.negated);
    let [row] = lowered.semantic_module.operation_crash_contracts.as_slice() else {
        panic!("one operation crash contract row");
    };
    assert_eq!(row.machine, occurrence.terminal_machine);
    assert_eq!(row.operation, occurrence.terminal_operation);
    let (block, operation) = joined_operation(&lowered, row);
    let OperationKind::IntegerLessThan { left, right } = operation.kind else {
        panic!("the joined operation is the emitted reversed comparison");
    };
    // The authored pair still completes as `left` then `right`; only the
    // emitted operand roster is reversed.
    let [.., authored_left, authored_right] = block.parameters.as_slice() else {
        panic!("the comparison block carries both completed operands");
    };
    assert_eq!((left, right), (authored_right.id, authored_left.id));
    let integer = ScalarType::Integer(i32_type());
    let formal = |raw| ScalarTerm::value(ValueId::new(raw).expect("formal"), integer);
    assert_eq!(row.published_routes, vec![guarded_trap_route(formal(1))]);
    assert_eq!(
        row.crash_continuations,
        vec![guarded_trap_route(ScalarTerm::value(left, integer))]
    );
    terminal_verifier::validate_module(&lowered.semantic_module)
        .expect("the verifier accepts the reindexed guarded row");
}

#[test]
fn a_reordered_row_left_in_the_authored_telescope_fails_verification() {
    // The reindexing is load-bearing, not cosmetic. A `>` row that kept the
    // operator declaration's authored formal 2 would make the verifier
    // reconstruct the guard over the other operand, so it must reject.
    let checked = checked_with_provider_commitments(GUARDED_REORDERED_INTEGER_OPERATOR_SOURCE);
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("compare"))
        .expect("the reordered route lowers");
    let mut forged = lowered.semantic_module.clone();
    let integer = ScalarType::Integer(i32_type());
    forged.operation_crash_contracts[0].published_routes = vec![guarded_trap_route(
        ScalarTerm::value(ValueId::new(2).expect("formal"), integer),
    )];
    assert!(matches!(
        terminal_verifier::validate_module(&forged),
        Err(terminal_verifier::ModuleError::OperationCrashContinuationsMismatch { .. })
    ));
}

#[test]
fn a_negated_route_keeps_its_contract_on_the_emitted_comparison() {
    // `!=` emits `IntegerEqual` over the authored order plus one `BooleanNot`.
    // The crash contract stays on the equality, which is the operation owning
    // the scalar operands the formal telescope binds, so the authored
    // telescope needs no reindexing and the negation only carries the Boolean
    // result forward without taking a row of its own.
    let checked = checked_with_provider_commitments(GUARDED_NEGATED_INTEGER_OPERATOR_SOURCE);
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("compare"))
        .expect("a negated guarded integer comparison carries its crash contract");
    let [occurrence] = lowered.selected_integer_comparison_occurrences.as_slice() else {
        panic!("one selected integer comparison occurrence");
    };
    assert_eq!(
        occurrence.comparison,
        lowered_psi::LoweredSelectedIntegerComparisonOperation::Equal
    );
    assert_eq!(
        occurrence.operand_order,
        lowered_psi::LoweredSelectedIntegerComparisonOperandOrder::Authored
    );
    assert!(occurrence.negated);
    let [row] = lowered.semantic_module.operation_crash_contracts.as_slice() else {
        panic!("one operation crash contract row");
    };
    assert_eq!(row.operation, occurrence.terminal_operation);
    let (block, operation) = joined_operation(&lowered, row);
    let OperationKind::IntegerEqual { left, right } = operation.kind else {
        panic!("the joined operation is the emitted equality");
    };
    let [.., authored_left, authored_right] = block.parameters.as_slice() else {
        panic!("the comparison block carries both completed operands");
    };
    assert_eq!((left, right), (authored_left.id, authored_right.id));
    let result = operation
        .result
        .scalar()
        .expect("the comparison produces a Boolean")
        .id;
    assert_eq!(
        lowered
            .semantic_module
            .machines
            .iter()
            .filter(|machine| machine.id == row.machine)
            .flat_map(|machine| machine.blocks.iter().flat_map(|block| &block.operations))
            .filter(|candidate| matches!(candidate.kind,
                OperationKind::BooleanNot { operand } if operand == result))
            .count(),
        1
    );
    let integer = ScalarType::Integer(i32_type());
    let formal = |raw| ScalarTerm::value(ValueId::new(raw).expect("formal"), integer);
    assert_eq!(row.published_routes, vec![guarded_trap_route(formal(2))]);
    assert_eq!(
        row.crash_continuations,
        vec![guarded_trap_route(ScalarTerm::value(right, integer))]
    );
    terminal_verifier::validate_module(&lowered.semantic_module)
        .expect("the verifier accepts the negated route's row");
}

#[test]
fn a_guarded_operator_route_without_a_structured_scalar_form_fails_closed() {
    // The checked stage structures integer comparisons and Boolean formals
    // over an operator's parameters; an IEEE ordering over scalar float
    // formals has no checked structured form, so this guarded route still
    // carries its predicate identity only, cannot lower into the row's
    // proposition form, and the use must not lower crash-free.
    let checked = checked_with_provider_commitments(
        "boundary operator == Float::equal(left: f64, right: f64) -> bool
         crashes Trap !(right >= 0.0);
         machine compare(left: f64, right: f64) -> bool crashes Trap { left == right }",
    );
    let error = lower_machine(&checked, TerminalMachineSelection::Name("compare"))
        .expect_err("a guarded route cannot lower");
    assert!(
        format!("{error:?}")
            .contains("guarded crash route is outside structured scalar predicate lowering"),
        "{error:?}"
    );
}

#[test]
fn a_crash_qualified_use_without_lowerable_crash_evidence_fails_closed() {
    // Every authored integer comparison spelling now has an admitted
    // emission, so the remaining refusals are about the evidence itself: a
    // published route that normalizes away leaves the site with nothing to
    // carry, and a use without complete provider plan evidence has no exact
    // selected occurrence to join at all.
    let checked = checked_with_provider_commitments(
        "boundary operator > Comparison::greater(left: i32, right: i32) -> bool
         crashes Trap false;
         pub machine compare(left: i32, right: i32) -> bool { left > right }",
    );
    let error = lower_machine(&checked, TerminalMachineSelection::Name("compare"))
        .expect_err("a site with no lowerable route must not lower crash-free");
    assert!(
        format!("{error:?}").contains("operator crash site publishes no lowerable crash route"),
        "{error:?}"
    );
    let checked = checked_source(GUARDED_INTEGER_OPERATOR_SOURCE);
    let error = lower_machine(&checked, TerminalMachineSelection::Name("compare"))
        .expect_err("a use without complete provider plan evidence must not lower");
    assert!(
        format!("{error:?}").contains("selected comparison has no complete provider plan evidence"),
        "{error:?}"
    );
}

#[test]
fn a_crash_qualified_use_whose_site_was_dropped_fails_closed() {
    let mut checked = crash_qualified_float_comparison("Trap");
    let compare = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "compare")
        .expect("compare machine")
        .symbol;
    let plan = checked
        .facts
        .contract_plans
        .machines
        .iter_mut()
        .find(|plan| plan.machine == compare)
        .expect("compare contract plan");
    plan.crash = plan
        .crash
        .clone()
        .with_checked_operators(Vec::new())
        .expect("an empty site roster");
    let error = lower_machine(&checked, TerminalMachineSelection::Name("compare"))
        .expect_err("a dropped site cannot lower");
    assert!(
        format!("{error:?}")
            .contains("selected operator crash invocation has no checked crash site to lower"),
        "{error:?}"
    );
}

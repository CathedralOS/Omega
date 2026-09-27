//! Checked operator crash sites lower to Terminal operation crash contracts
//! at the exact emitted operation, and every crash-qualified use the closure
//! cannot carry that way fails closed.

use crate::TerminalMachineSelection;

use crate::lower_machine;
use crate::lowering_error::LoweringError;
use semantic_vocabulary::{
    IntegerSign, IntegerType, IntegerValue, Proposition, ScalarTerm, ScalarType, ValueId,
};
use terminal_psi::{CrashCause, CrashPredicateTerm, CrashRouteBucket, CrashRouteGuard};

fn unconditional(cause: CrashCause) -> CrashRouteBucket {
    CrashRouteBucket {
        cause,
        alternatives: vec![CrashRouteGuard::Truth],
    }
}

fn crash_qualified_float_comparison(cause: &str) -> checked_trees::CheckedTrees {
    crate::front_end::checked_program(&format!(
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

/// The same guarded route on a spelling whose emission reverses the authored
/// operands: `>` emits `IntegerLessThan` over `(right, left)`.
const GUARDED_REORDERED_INTEGER_OPERATOR_SOURCE: &str =
    "boundary operator > Comparison::greater(left: i32, right: i32) -> bool
     crashes Trap !(right >= 0);
     pub machine compare(left: i32, right: i32) -> bool crashes Trap { left > right }";

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
fn a_reordered_row_left_in_the_authored_telescope_fails_verification() {
    // The reindexing is load-bearing, not cosmetic. A `>` row that kept the
    // operator declaration's authored formal 2 would make the verifier
    // reconstruct the guard over the other operand, so it must reject.
    let checked = crate::front_end::checked_program(GUARDED_REORDERED_INTEGER_OPERATOR_SOURCE);
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
fn a_crash_qualified_use_without_lowerable_crash_evidence_fails_closed() {
    // Every authored integer comparison spelling now has an admitted
    // emission, so the remaining refusal is about the evidence itself: a
    // published route that normalizes away leaves the site with nothing to
    // carry.
    let checked = crate::front_end::checked_program(
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

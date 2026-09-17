//! Checked operator crash sites lower to Terminal operation crash contracts
//! at the exact emitted operation, and every crash-qualified use the closure
//! cannot carry that way fails closed.

use super::checked_source;
use crate::lower_machine;
use crate::lowering_error::LoweringError;
use semantic_vocabulary::{Proposition, ScalarTerm, ScalarType, ValueId};
use terminal_psi::{CrashCause, CrashPredicateTerm, CrashRouteBucket, CrashRouteGuard};

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

#[test]
fn selected_operator_crash_site_lowers_to_one_row_at_the_emitted_comparison() {
    for cause in [CrashCause::Trap, CrashCause::Abort] {
        let checked = crash_qualified_float_comparison(match cause {
            CrashCause::Trap => "Trap",
            CrashCause::Abort => "Abort",
        });
        let lowered = lower_machine(&checked, "compare")
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
    let lowered = lower_machine(&checked, "compare").unwrap();
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
    lower_machine(&checked, "compare").expect("the retained site lowers");
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
    assert!(matches!(
        lower_machine(&checked, "compare"),
        Err(LoweringError::InvalidTerminalModule(
            terminal_verifier::ModuleError::CallCrashContinuationUncovered {
                cause: CrashCause::Abort,
                ..
            }
        ))
    ));
}

#[test]
fn a_guarded_operator_route_without_a_structured_scalar_form_fails_closed() {
    // Operator published rows retain the authored predicate identity only;
    // the checked stage attaches no structured scalar form to them yet, so a
    // guarded route cannot lower into the row's proposition form and the use
    // must not lower crash-free.
    let checked = checked_with_provider_commitments(
        "boundary operator == Float::equal(left: f64, right: f64) -> bool
         crashes Trap !(right >= 0.0);
         machine compare(left: f64, right: f64) -> bool crashes Trap { left == right }",
    );
    let error = lower_machine(&checked, "compare").expect_err("a guarded route cannot lower");
    assert!(
        format!("{error:?}")
            .contains("guarded crash route is outside structured scalar predicate lowering"),
        "{error:?}"
    );
}

#[test]
fn a_crash_qualified_use_without_an_emitted_join_fails_closed() {
    // An integer boundary comparison has no exact selected Terminal meaning
    // yet, so its crash-qualified use cannot lower as a crash-free operation.
    for (operator_contract, caller_contract) in [
        ("crashes Trap", "crashes Trap"),
        ("crashes Abort", "crashes Abort"),
        ("crashes Trap false", ""),
    ] {
        let checked = checked_source(&format!(
            "boundary operator == Comparison::equal(left: i32, right: i32) -> bool {operator_contract};
             pub machine compare(left: i32, right: i32) -> bool {caller_contract} {{ left == right }}"
        ));
        let error = lower_machine(&checked, "compare")
            .expect_err("an unjoined crash-qualified use must not lower crash-free");
        assert!(
            format!("{error:?}").contains("comparison has no exact selected IEEE meaning"),
            "{error:?}"
        );
    }
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
    let error = lower_machine(&checked, "compare").expect_err("a dropped site cannot lower");
    assert!(
        format!("{error:?}")
            .contains("selected operator crash invocation has no checked crash site to lower"),
        "{error:?}"
    );
}

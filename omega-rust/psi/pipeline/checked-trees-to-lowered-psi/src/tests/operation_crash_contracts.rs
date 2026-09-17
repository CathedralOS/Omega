//! Checked operator crash sites lower to Terminal operation crash contracts
//! at the exact emitted operation, and every crash-qualified use the closure
//! cannot carry that way fails closed.

use std::collections::BTreeMap;

use super::checked_source;
use crate::lower_machine;
use crate::lowering_error::LoweringError;
use crate::proofs::crash_routes::lower_formal_crash_routes;
use semantic_vocabulary::{
    IntegerSign, IntegerType, IntegerValue, Proposition, ScalarTerm, ScalarType, ValueId,
};
use terminal_psi::{
    CrashCause, CrashPredicateTerm, CrashRouteBucket, CrashRouteGuard, OperationKind,
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
fn a_guarded_operator_route_lowers_to_the_guard_proposition_the_verifier_accepts() {
    // The checked stage attaches the guard's structured scalar form over the
    // operator's own formals, and the producer's formal-telescope lowering
    // turns it into the published proposition (`right` is formal 2). The only
    // operation join emission records today is the selected IEEE comparison,
    // whose two float formals admit no structured guard, so this witness
    // stops at the row: the real checked site's routes lower through the
    // producer's own route lowering, and the row is installed at the
    // `IntegerEqual` operation of an independently lowered integer comparison
    // to show the verifier accepts its recomputed continuation.
    let checked = checked_source(
        "boundary operator == Comparison::equal(left: i32, right: i32) -> bool
         crashes Trap !(right >= 0);
         pub machine compare(left: i32, right: i32) -> bool crashes Trap { left == right }",
    );
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
    let i32_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32");
    let integer = ScalarType::Integer(i32_type);
    let published = lower_formal_crash_routes(site.published(), &[integer, integer])
        .expect("the structured guard lowers through the formal telescope");
    let guard = |right: ScalarTerm| {
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
    };
    let formal = |raw| ScalarTerm::value(ValueId::new(raw).expect("formal"), integer);
    assert_eq!(published, vec![guard(formal(2))]);

    let host = checked_source(
        "pub machine compare(left: i32, right: i32) -> bool crashes Trap { left == right }",
    );
    let lowered = lower_machine(&host, "compare").expect("an integer comparison lowers");
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
        vec![guard(ScalarTerm::value(right, integer))]
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

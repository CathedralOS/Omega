//! The terminal-stage boundary-operator custody scope replays the selected
//! integer comparison roster the way it replays the IEEE one: a recorded
//! occurrence rejoins its checked use, its checked application and its
//! emitted operation, and a stale, duplicated or foreign row rejects.

use super::checked_source;
use crate::lower_machine;
use lowered_psi::{
    LoweredSelectedIntegerComparisonOperandOrder, LoweredSelectedIntegerComparisonOperation,
};
use semantic_vocabulary::{IntegerSign, IntegerType, OperationId};

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

fn selected_integer_comparison(token: &str, name: &str, primitive: &str) -> String {
    format!(
        "boundary operator {token} Comparison::{name}(left: {primitive}, right: {primitive}) -> bool;
         machine choose(left: {primitive}, right: {primitive}) -> bool {{ left {token} right }}"
    )
}

fn i32_equality() -> checked_trees::CheckedTrees {
    checked_with_provider_commitments(&selected_integer_comparison("==", "equal", "i32"))
}

#[test]
fn a_replayed_integer_occurrence_is_admitted_into_the_custody_scope() {
    use LoweredSelectedIntegerComparisonOperandOrder as Order;
    use LoweredSelectedIntegerComparisonOperation as Operation;
    for (token, name, expected, order, negated) in [
        ("==", "equal", Operation::Equal, Order::Authored, false),
        ("!=", "not_equal", Operation::Equal, Order::Authored, true),
        ("<", "less", Operation::LessThan, Order::Authored, false),
        (
            "<=",
            "less_or_equal",
            Operation::LessOrEqual,
            Order::Authored,
            false,
        ),
        (">", "greater", Operation::LessThan, Order::Swapped, false),
        (
            ">=",
            "greater_or_equal",
            Operation::LessOrEqual,
            Order::Swapped,
            false,
        ),
    ] {
        for (primitive, sign, bits) in [
            ("i32", IntegerSign::Signed, 32),
            ("u64", IntegerSign::Unsigned, 64),
        ] {
            let source = selected_integer_comparison(token, name, primitive);
            let checked = checked_with_provider_commitments(&source);
            let lowered =
                lower_machine(&checked, "choose").expect("selected integer comparison lowers");
            let [occurrence] = lowered.selected_integer_comparison_occurrences.as_slice() else {
                panic!("one selected integer comparison occurrence: {source}");
            };
            assert_eq!(occurrence.comparison, expected, "{source}");
            assert_eq!(occurrence.operand_order, order, "{source}");
            assert_eq!(occurrence.negated, negated, "{source}");
            assert_eq!(
                occurrence.integer_type,
                IntegerType::new(sign, bits).expect("primitive integer type"),
                "{source}"
            );
            let produced = terminal_production::TerminalProductionRequest::new(&checked, "choose")
                .produce_checked_artifact()
                .expect("selected integer comparison custody publishes");
            let scope = produced.boundary_operator_scope();
            let [published] = scope.occurrences() else {
                panic!("one exact published integer comparison occurrence: {source}");
            };
            assert_eq!(
                published.terminal_operation(),
                occurrence.terminal_operation,
                "{source}"
            );
            let application = &scope.applications()[published.application_index()];
            assert_eq!(application.site, occurrence.application_site, "{source}");
            assert_eq!(
                application.requirement_symbol, occurrence.requirement_operator,
                "{source}"
            );
            // The same roster replays directly against the published artifact.
            let replayed = lowered_psi_to_terminal_psi::checked_boundary_operator_scope(
                &checked,
                produced.artifact(),
                &lowered,
            )
            .expect("the recorded roster rejoins its checked application");
            assert_eq!(replayed.occurrences(), scope.occurrences(), "{source}");
        }
    }
}

#[test]
fn a_stale_integer_occurrence_rejects() {
    let checked = i32_equality();
    let lowered = lower_machine(&checked, "choose").unwrap();
    let produced = terminal_production::TerminalProductionRequest::new(&checked, "choose")
        .produce_checked_artifact()
        .unwrap();
    let replay = |corrupted: &lowered_psi::LoweredPsi| {
        lowered_psi_to_terminal_psi::checked_boundary_operator_scope(
            &checked,
            produced.artifact(),
            corrupted,
        )
        .unwrap_err()
    };
    // A row naming an operation the machine no longer carries.
    let mut corrupted = lowered.clone();
    corrupted.selected_integer_comparison_occurrences[0].terminal_operation =
        OperationId::new(u64::MAX).expect("unused operation identity");
    assert_eq!(
        replay(&corrupted),
        "integer comparison does not name one exact Terminal operation"
    );
    // A row naming a machine the module no longer carries.
    let mut corrupted = lowered.clone();
    corrupted.selected_integer_comparison_occurrences[0].terminal_machine =
        semantic_vocabulary::MachineId::new(u64::MAX).expect("unused machine identity");
    assert_eq!(
        replay(&corrupted),
        "integer comparison does not name one exact Terminal operation"
    );
    // A row whose checked use handle no longer resolves.
    let mut corrupted = lowered.clone();
    corrupted.selected_integer_comparison_occurrences[0].operator_use = Default::default();
    assert_eq!(
        replay(&corrupted),
        "integer comparison lost its checked operator occurrence"
    );
}

#[test]
fn a_duplicated_integer_occurrence_rejects() {
    let checked = i32_equality();
    let lowered = lower_machine(&checked, "choose").unwrap();
    let produced = terminal_production::TerminalProductionRequest::new(&checked, "choose")
        .produce_checked_artifact()
        .unwrap();
    let mut corrupted = lowered.clone();
    let duplicate = corrupted.selected_integer_comparison_occurrences[0];
    corrupted
        .selected_integer_comparison_occurrences
        .push(duplicate);
    assert_eq!(
        lowered_psi_to_terminal_psi::checked_boundary_operator_scope(
            &checked,
            produced.artifact(),
            &corrupted,
        )
        .unwrap_err(),
        "checked boundary-operator applications do not map one-to-one onto Terminal operations"
    );
}

#[test]
fn a_foreign_integer_occurrence_rejects() {
    let checked = i32_equality();
    let lowered = lower_machine(&checked, "choose").unwrap();
    let produced = terminal_production::TerminalProductionRequest::new(&checked, "choose")
        .produce_checked_artifact()
        .unwrap();
    let replay = |corrupted: &lowered_psi::LoweredPsi| {
        lowered_psi_to_terminal_psi::checked_boundary_operator_scope(
            &checked,
            produced.artifact(),
            corrupted,
        )
        .unwrap_err()
    };
    let changed = "integer comparison changed its exact selected application";
    // A row claiming a different comparison than the checked use selected.
    let mut corrupted = lowered.clone();
    corrupted.selected_integer_comparison_occurrences[0].comparison =
        LoweredSelectedIntegerComparisonOperation::LessThan;
    assert_eq!(replay(&corrupted), changed);
    // A row claiming a different operand type than the checked operands.
    let mut corrupted = lowered.clone();
    corrupted.selected_integer_comparison_occurrences[0].integer_type =
        IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    assert_eq!(replay(&corrupted), changed);
    // A row whose provider commitment is not the checked use's, or is absent.
    let mut corrupted = lowered.clone();
    corrupted.selected_integer_comparison_occurrences[0].provider_plan_commitment =
        checked_trees::CheckedProviderPlanCommitment::from_digest([9; 32]);
    assert_eq!(replay(&corrupted), changed);
    let mut checked_without_provider = checked.clone();
    let use_handle = lowered.selected_integer_comparison_occurrences[0].operator_use;
    checked_without_provider
        .facts
        .operators
        .uses
        .get_mut(use_handle)
        .provider_plan_commitment = Default::default();
    let mut corrupted = lowered.clone();
    corrupted.selected_integer_comparison_occurrences[0].provider_plan_commitment =
        Default::default();
    assert_eq!(
        lowered_psi_to_terminal_psi::checked_boundary_operator_scope(
            &checked_without_provider,
            produced.artifact(),
            &corrupted,
        )
        .unwrap_err(),
        changed
    );
    // A row whose recorded operand mapping or negation is not the authored
    // spelling's admitted emission. The admitted roster is the producer's
    // only source for both, so neither can be relabelled onto a real
    // operation: `==` emits the authored order and does not negate.
    let mut corrupted = lowered.clone();
    corrupted.selected_integer_comparison_occurrences[0].operand_order =
        LoweredSelectedIntegerComparisonOperandOrder::Swapped;
    assert_eq!(replay(&corrupted), changed);
    let mut corrupted = lowered.clone();
    corrupted.selected_integer_comparison_occurrences[0].negated = true;
    assert_eq!(replay(&corrupted), changed);
    // A row from a program whose checked use has a different admitted
    // emission: the same handle now resolves a `>` use, which emits the
    // reversed `IntegerLessThan` rather than this row's equality.
    let foreign =
        checked_with_provider_commitments(&selected_integer_comparison(">", "greater", "i32"));
    assert_eq!(
        lowered_psi_to_terminal_psi::checked_boundary_operator_scope(
            &foreign,
            produced.artifact(),
            &lowered,
        )
        .unwrap_err(),
        changed
    );
    // A row from a program whose checked use compares floats.
    let float = checked_with_provider_commitments(
        "boundary operator == Float::equal(left: f32, right: f32) -> bool;
         machine choose(left: f32, right: f32) -> bool { left == right }",
    );
    let error = lowered_psi_to_terminal_psi::checked_boundary_operator_scope(
        &float,
        produced.artifact(),
        &lowered,
    )
    .unwrap_err();
    assert_eq!(error, changed);
}

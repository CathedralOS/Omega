//! A condition cannot make its own arithmetic safe. These tests enter the
//! declaration phase directly, including declarations with no executable body.

use super::*;

fn number(value: u128) -> ScalarTerm {
    ScalarTerm::integer(
        integer_type(IntegerSign::Unsigned),
        IntegerValue::Unsigned(value),
    )
    .unwrap()
}

fn subtract(left: ScalarTerm, right: ScalarTerm) -> ScalarTerm {
    ScalarTerm::exact_integer_subtract(integer_type(IntegerSign::Unsigned), left, right).unwrap()
}

fn nonnegative(term: ScalarTerm) -> Proposition {
    Proposition::LessOrEqual(number(0), term)
}

fn form(clauses: Vec<Proposition>) -> Checked<()> {
    let elaboration = lower(vec![Machine {
        requires: clauses,
        ..machine(vec![u32_type(), u32_type()], vec![returns(unsigned(0))])
    }])?;
    crate::formation::module(&elaboration.module)
}

#[test]
fn carrier_bounds_form_maximum_subtraction_without_using_its_clause() {
    form(vec![nonnegative(subtract(
        number(u32::MAX.into()),
        formal(1, u32_type()),
    ))])
    .unwrap();
}

#[test]
fn a_clause_cannot_use_its_own_consequence_to_form_subtraction() {
    let error = form(vec![nonnegative(subtract(
        formal(1, u32_type()),
        number(1),
    ))])
    .unwrap_err();
    assert_eq!(error.reason, RefusalReason::Violation);
}

#[test]
fn independent_bounds_form_terms_in_either_authored_order() {
    let guarded = nonnegative(subtract(formal(1, u32_type()), number(1)));
    let independent = Proposition::LessOrEqual(number(1), formal(1, u32_type()));
    form(vec![guarded.clone(), independent.clone()]).unwrap();
    form(vec![independent, guarded]).unwrap();
}

#[test]
fn mutually_pending_clauses_supply_no_premises() {
    let first = Proposition::LessOrEqual(number(1), subtract(formal(1, u32_type()), number(1)));
    let second = Proposition::LessOrEqual(number(1), subtract(formal(2, u32_type()), number(1)));
    assert_eq!(
        form(vec![first, second]).unwrap_err().reason,
        RefusalReason::Violation
    );
}

#[test]
fn nested_children_form_before_their_results_supply_carrier_bounds() {
    let maximum = number(u32::MAX.into());
    let child = subtract(maximum.clone(), formal(1, u32_type()));
    form(vec![nonnegative(subtract(maximum, child))]).unwrap();
    let bad_child = subtract(formal(1, u32_type()), number(1));
    assert!(
        form(vec![nonnegative(subtract(
            number(u32::MAX.into()),
            bad_child
        ))])
        .is_err()
    );
}

#[test]
fn a_decided_disjunction_does_not_erase_an_unformed_occurrence() {
    let hidden = nonnegative(subtract(formal(1, u32_type()), number(1)));
    assert!(
        form(vec![Proposition::Disjunction(vec![
            Proposition::Truth,
            hidden
        ])])
        .is_err()
    );
}

#[test]
fn a_bodyless_contract_is_not_exempt_from_formation() {
    let mut elaboration =
        lower(vec![machine(vec![u32_type()], vec![returns(unsigned(0))])]).unwrap();
    let mut declared = elaboration.module.machines[0].contract.clone();
    declared.requires = vec![nonnegative(subtract(formal(1, u32_type()), number(1)))];
    elaboration.module.declaration_contracts.push(declared);
    assert_eq!(
        crate::prepare_replay(&mut elaboration)
            .err()
            .unwrap()
            .reason,
        RefusalReason::Violation
    );
}

#[test]
fn guarantees_cannot_form_an_input_precondition() {
    let mut elaboration =
        lower(vec![machine(vec![u32_type()], vec![returns(unsigned(0))])]).unwrap();
    let contract = &mut elaboration.module.machines[0].contract;
    contract.requires = vec![nonnegative(subtract(formal(1, u32_type()), number(1)))];
    contract.ensures.push(terminal_psi::ContractClause {
        obligation: id(1),
        proposition: Proposition::LessOrEqual(number(1), formal(1, u32_type())),
    });
    assert!(crate::formation::module(&elaboration.module).is_err());
}

#[test]
fn replay_formation_is_an_explicit_searching_entrance() {
    let mut elaboration = lower(vec![Machine {
        requires: vec![nonnegative(subtract(
            number(u32::MAX.into()),
            formal(1, u32_type()),
        ))],
        ..machine(vec![u32_type()], vec![returns(unsigned(0))])
    }])
    .unwrap();
    let before = crate::prove::SEARCHES.with(std::cell::Cell::get);
    let _scope = crate::prepare_replay(&mut elaboration).unwrap();
    assert!(crate::prove::SEARCHES.with(std::cell::Cell::get) > before);
}

fn source_result(text: &str) -> RouteVerdict {
    super::build_purpose::judge_build(text).unwrap()
}

fn accepted_source(text: &str) -> Box<crate::AcceptedClosure> {
    let verdict = source_result(text);
    let RouteVerdict::Accepted(accepted) = verdict else {
        panic!("{verdict:?}");
    };
    accepted
}

fn executed_source(text: &str) {
    // A formed contract must still survive publication and execution of the
    // same body. A bodyless boundary is checked separately without a provider.
    let publication = accepted_source(text).publish_invocation().unwrap();
    let program = terminal_interpreter::TerminalProgram::borrow(publication.artifact.view());
    let mut instance = program.instantiate(Default::default()).unwrap();
    let status = instance
        .invoke(&mut terminal_work::WorkMeter::with_budget(10_000))
        .unwrap();
    assert_eq!(
        status,
        terminal_interpreter::TerminalExecutionStatus::Complete(
            terminal_interpreter::TerminalExecutionResult::Unit
        )
    );
}

fn rejected_source(text: &str) {
    let verdict = source_result(text);
    match verdict {
        RouteVerdict::Rejected { .. } => {}
        RouteVerdict::Accepted(_) => panic!("invalid contract arithmetic was accepted"),
        other => panic!("invalid contract arithmetic was not rejected: {other:?}"),
    }
}

#[test]
fn source_maximum_subtraction_forms_but_the_caller_still_owes_the_bound() {
    const SOURCE: &str = r#"
machine append(offset: u64, size: u64) -> u64
requires size <= 18446744073709551615u64 - offset
{ offset + size }
machine build_entry() { let end: u64 = append(17, 23); }
"#;
    executed_source(SOURCE);
    rejected_source(&SOURCE.replace("append(17, 23)", "append(18446744073709551615u64, 1)"));
}

#[test]
fn source_packed_comparison_forms_each_exact_operation_from_independent_bounds() {
    const SOURCE: &str = r#"
machine packed(rest: u8, count: u8) -> u8
requires rest <= 9, count <= 9, rest + count * 10 <= 99
{ rest }
machine build_entry() { let value: u8 = packed(9, 9); }
"#;
    executed_source(SOURCE);

    // 26 * 10 can overflow the child before the sum is formed. With the
    // child bounded to 90, 166 + 90 can still overflow the sum.
    for invalid in [
        SOURCE.replace("count <= 9", "count <= 26"),
        SOURCE.replace("rest <= 9", "rest <= 166"),
    ] {
        let verdict = source_result(&invalid);
        let RouteVerdict::Rejected { violations, .. } = verdict else {
            panic!("an operation can overflow before its comparison forms: {verdict:?}");
        };
        let [violation] = violations.as_slice() else {
            panic!("expected a single formation violation: {violations:?}");
        };
        assert!(
            violation
                .statement
                .contains("contract arithmetic is not proved total before its condition"),
            "{violation:?}"
        );
    }
}

#[test]
fn a_bound_on_one_field_does_not_form_its_neighbors_subtraction() {
    const SOURCE: &str = r#"
data Pair [copy] { first: u32; second: u32; }
machine consume(value: &Pair)
requires value.first >= 1, value.first - 1 <= 4294967295u32
{ }
machine build_entry() { let value: Pair = Pair { first: 7, second: 0 }; consume(&value); }
"#;
    executed_source(SOURCE);
    rejected_source(&SOURCE.replace("value.first - 1", "value.second - 1"));
}

#[test]
fn completion_storage_does_not_inherit_the_inputs_nonzero_bound() {
    const SOURCE: &str = r#"
data Cell { value: u32; }
machine retain(value: &mut Cell)
requires value.value >= 1
ensures old(value.value) - 1 <= 4294967295u32
{ value.value = 0; }
machine build_entry() { let mut value: Cell = Cell { value: 7 }; retain(&mut value); }
"#;
    executed_source(SOURCE);
    rejected_source(&SOURCE.replace("old(value.value) - 1", "value.value - 1"));
}

#[test]
fn independent_result_guarantees_can_form_a_later_guarantee() {
    const SOURCE: &str = r#"
machine positive() -> u32
ensures result >= 1, result - 1 == 0
{ 1 }
machine build_entry() { let value: u32 = positive(); }
"#;
    executed_source(SOURCE);
    rejected_source(&SOURCE.replace("result >= 1, ", ""));
}

#[test]
fn scalar_domains_must_form_before_membership_can_supply_their_predicate() {
    const SOURCE: &str = r#"
domain u32::Nonzero requires self >= 1, self - 1 <= 4294967295u32;
machine build_entry() { let value: u32 in Nonzero = 7 as +Nonzero; }
"#;
    executed_source(SOURCE);
    rejected_source(&SOURCE.replace("self >= 1, ", ""));
}

#[test]
fn data_requirements_keep_each_partial_field_occurrence() {
    const SOURCE: &str = r#"
data Count [copy] requires value >= 1, value - 1 <= 4294967295u32 { value: u32; }
machine build_entry() { let value: Count = Count { value: 7 }; }
"#;
    executed_source(SOURCE);
    rejected_source(&SOURCE.replace("value >= 1, ", ""));
}

#[test]
fn bodyless_crash_guards_need_their_own_formed_input_context() {
    const SOURCE: &str = r#"
boundary trait Device {
    machine read(value: u32) -> u32
    reaches none
    requires value >= 1
    crashes Trap value - 1 == 0;
}
machine build_entry() crashes none { let value: u32 = Device::read(2); }
"#;
    accepted_source(SOURCE);
    rejected_source(&SOURCE.replace("requires value >= 1", "requires true"));
}

#[test]
fn overflow_policies_do_not_remove_contract_divisor_obligations() {
    for policy in ["%", "|"] {
        let source = format!(
            r#"
machine divide(left: u32, right: u32) -> u32
requires right > 0
ensures result == left /{policy} right
{{ left /{policy} right }}
machine build_entry() {{ let value: u32 = divide(31, 3); }}
"#
        );
        executed_source(&source);
        rejected_source(&source.replace("requires right > 0", "requires true"));
    }
}

/// Nested computations retain their policy and every inner formation question,
/// including when an outer multiplication cannot change the final value.
#[test]
fn nested_summary_arithmetic_keeps_each_inner_formation_obligation() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../../tests/fixtures/run/calls/scalar_contract_route_exit/main.omg"
    ))
    .replace("machine main()", "machine build_entry()");
    executed_source(&source);

    let guarded = r#"
machine nested(value: u8) -> u8
requires value <= 254
ensures result == (value + 1u8) * 0u8
{ 0u8 }
machine build_entry() { let value: u8 = nested(0u8); }
"#;
    executed_source(guarded);
    rejected_source(&guarded.replace("requires value <= 254", "requires true"));
}

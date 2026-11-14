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
    assert_eq!(error.reason, RefusalReason::Violation(None));
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
        RefusalReason::Violation(None)
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
        RefusalReason::Violation(None)
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

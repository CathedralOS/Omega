//! Integer entry ranges retain their authored inclusive endpoints on the
//! closed scalar contract roster while the requires tail keeps each range as
//! a `Predicate` conjunction in the same constraint order: the roster is
//! exact endpoint evidence riding beside the clause, never a replacement.
use super::{
    Lexer, ResolutionRequest, SymbolHandle, lower_symbol_resolved_trees, parse_syntax_trees,
    resolve,
};
use crate::lower_typed_trees;
use numerics::literals::IntegerLiteral;
use typed_trees::types::PrimitiveType;

fn checked(source: &str) -> checked_trees::CheckedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    lower_typed_trees(typed).expect("check")
}

fn machine_named(checked: &checked_trees::CheckedTrees, name: &str) -> SymbolHandle {
    checked
        .machines()
        .iter()
        .find(|machine| {
            machine.name.as_str() == name || machine.name.as_str().ends_with(&format!("::{name}"))
        })
        .unwrap_or_else(|| panic!("missing machine `{name}`"))
        .symbol
}

fn contract_plan(
    checked: &checked_trees::CheckedTrees,
    machine: SymbolHandle,
) -> &checked_trees::MachineContractPlan {
    checked
        .facts
        .contract_plans
        .for_machine(machine)
        .unwrap_or_else(|| panic!("missing contract plan"))
}

#[test]
fn inclusive_u64_entry_range_retains_authored_endpoints() {
    let checked = checked(
        r#"
        machine accept(value: u64[0..=3]) -> u64 { value }
    "#,
    );
    let machine = machine_named(&checked, "accept");
    let plan = contract_plan(&checked, machine);
    let [requirement] = plan
        .closed_scalar_values
        .integer_entry_ranges()
        .expect("one complete retained integer range")
    else {
        panic!("one retained integer range")
    };
    assert_eq!(
        *requirement,
        checked_trees::ClosedIntegerRangeRequirement {
            position: 0,
            primitive_type: PrimitiveType::U64,
            minimum: IntegerLiteral::from_value(0),
            maximum: IntegerLiteral::from_value(3),
        }
    );
    // The requires tail keeps the authored range as a supported predicate
    // clause — the roster is its exact endpoint evidence, not a clause
    // replacement like a floating range would be.
    let [clause] = plan.closed_scalar_values.requires() else {
        panic!("one requires clause")
    };
    assert!(
        matches!(
            clause,
            Some(checked_trees::ClosedScalarContractValue::Predicate(
                checked_trees::CheckedBooleanExpression::And { .. }
            ))
        ),
        "the integer range rides the requires tail as a predicate conjunction"
    );
}

#[test]
fn exclusive_u64_entry_range_retains_its_inclusive_predecessor() {
    // `u64[0..4]` excludes 4, so the retained inclusive maximum is 3.
    let checked = checked(
        r#"
        machine accept(value: u64[0..4]) -> u64 { value }
    "#,
    );
    let machine = machine_named(&checked, "accept");
    let [requirement] = contract_plan(&checked, machine)
        .closed_scalar_values
        .integer_entry_ranges()
        .expect("one retained integer range")
    else {
        panic!("one retained integer range")
    };
    assert_eq!(
        *requirement,
        checked_trees::ClosedIntegerRangeRequirement {
            position: 0,
            primitive_type: PrimitiveType::U64,
            minimum: IntegerLiteral::from_value(0),
            maximum: IntegerLiteral::from_value(3),
        }
    );
}

#[test]
fn integer_entry_range_names_its_dense_scalar_position() {
    let checked = checked(
        r#"
        data Carrier { raw: u64; }
        machine accept(flag: bool, value: u64[0..=7], aux: Carrier) -> u64 { value }
    "#,
    );
    let machine = machine_named(&checked, "accept");
    let [requirement] = contract_plan(&checked, machine)
        .closed_scalar_values
        .integer_entry_ranges()
        .expect("one retained integer range")
    else {
        panic!("one retained integer range")
    };
    // `flag` occupies dense scalar position 0, so `value` is position 1;
    // the non-scalar `aux` claims no position.
    assert_eq!(requirement.position, 1);
    assert_eq!(requirement.primitive_type, PrimitiveType::U64);
}

#[test]
fn signed_i64_entry_range_retains_signed_endpoints() {
    let checked = checked(
        r#"
        machine accept(value: i64[-4..=4]) -> i64 { value }
    "#,
    );
    let machine = machine_named(&checked, "accept");
    let [requirement] = contract_plan(&checked, machine)
        .closed_scalar_values
        .integer_entry_ranges()
        .expect("one retained integer range")
    else {
        panic!("one retained integer range")
    };
    assert_eq!(
        *requirement,
        checked_trees::ClosedIntegerRangeRequirement {
            position: 0,
            primitive_type: PrimitiveType::I64,
            minimum: IntegerLiteral::from_value(-4),
            maximum: IntegerLiteral::from_value(4),
        }
    );
}

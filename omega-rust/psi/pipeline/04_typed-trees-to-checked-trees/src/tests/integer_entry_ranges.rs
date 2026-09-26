//! Integer entry ranges retain their authored inclusive endpoints on the
//! closed scalar contract roster while the requires tail keeps each range as
//! a `Predicate` conjunction in the same constraint order: the roster is
//! exact endpoint evidence riding beside the clause, never a replacement.
use super::SymbolHandle;
use crate::tests::front_end::checked_program;
use numerics::literals::IntegerLiteral;
use typed_trees::types::PrimitiveType;

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

/// A domain whose membership is exactly an interval retains the same entry
/// range the bracketed spelling did; an unstated side is the carrier's own
/// extreme. The range suffix is being retired in favor of this spelling.
#[test]
fn interval_domain_parameter_retains_the_bracketed_entry_range() {
    let requirement = |source: &str| {
        let checked = checked_program(source);
        let machine = machine_named(&checked, "accept");
        let [requirement] = contract_plan(&checked, machine)
            .closed_scalar_values
            .integer_entry_ranges()
            .expect("one complete retained integer range")
        else {
            panic!("one retained integer range")
        };
        requirement.clone()
    };
    assert_eq!(
        requirement(
            "domain u64::Small requires self <= 3;
             machine accept(value: u64 in Small) -> u64 { value }"
        ),
        requirement("machine accept(value: u64[0..=3]) -> u64 { value }")
    );
}

#[test]
fn inclusive_u64_entry_range_retains_authored_endpoints() {
    let checked = checked_program(
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
    let checked = checked_program(
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
    let checked = checked_program(
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
    let checked = checked_program(
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

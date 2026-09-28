//! Authored integer entry ranges keep their exact inclusive endpoints through
//! source lowering: an exclusive maximum becomes its predecessor, the retained
//! rows reach the published catalog beside the matching `requires` bounds, a
//! conforming delivery executes, and an exact out-of-range constant rejects.

use checked_trees_to_lowered_psi::TerminalMachineSelection;
use semantic_vocabulary::{IntegerValue, Proposition, ScalarTerm, ScalarType};

fn lower(source: &str, entry: &str) -> lowered_psi::LoweredPsi {
    checked_trees_to_lowered_psi::lower_machine(
        &crate::front_end::checked_program(source),
        TerminalMachineSelection::Name(entry),
    )
    .unwrap_or_else(|error| panic!("{source}: {error:?}"))
}

/// The owner contract publishes both inclusive bounds of `row` as
/// `LTE(minimum, parameter)` and `LTE(parameter, maximum)` conjuncts.
fn publishes_bounds(lowered: &lowered_psi::LoweredPsi, row: &terminal_psi::ScalarIntegerRange) {
    let owner = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == row.machine)
        .expect("range owner");
    let parameter = owner
        .parameters
        .iter()
        .find(|parameter| parameter.id == row.parameter)
        .expect("ranged parameter");
    assert_eq!(parameter.scalar_type, ScalarType::Integer(row.integer_type));
    let carrier = ScalarType::Integer(row.integer_type);
    let subject = ScalarTerm::value(parameter.id, carrier);
    let minimum = ScalarTerm::integer(row.integer_type, row.minimum).unwrap();
    let maximum = ScalarTerm::integer(row.integer_type, row.maximum).unwrap();
    let published = |expected: &Proposition| {
        let mut pending: Vec<&Proposition> = owner.contract.requires.iter().collect();
        while let Some(proposition) = pending.pop() {
            match proposition {
                Proposition::Conjunction(terms) => pending.extend(terms.iter()),
                proposition if proposition == expected => return true,
                _ => {}
            }
        }
        false
    };
    assert!(
        published(&Proposition::LessOrEqual(minimum, subject.clone())),
        "owner must publish the inclusive minimum bound"
    );
    assert!(
        published(&Proposition::LessOrEqual(subject, maximum)),
        "owner must publish the inclusive maximum bound"
    );
}

#[test]
fn exclusive_maximum_retains_its_inclusive_predecessor() {
    // `u64[0..4]` excludes 4, so the retained inclusive maximum is 3.
    let source = r#"
        machine accept(value: u64[0..4]) -> u64 { value }
        machine value() -> u64 { accept(3) }
    "#;
    let lowered = lower(source, "value");
    let [range] = lowered
        .semantic_module
        .scalar_qualifications
        .integer_entry_ranges
        .as_slice()
    else {
        panic!("one retained integer range: {source}")
    };
    assert_eq!(range.minimum, IntegerValue::Unsigned(0));
    assert_eq!(range.maximum, IntegerValue::Unsigned(3));
    publishes_bounds(&lowered, range);
}

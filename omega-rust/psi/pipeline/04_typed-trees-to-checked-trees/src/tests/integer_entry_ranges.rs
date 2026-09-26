//! Integer entry ranges retain their authored inclusive endpoints on the
//! closed scalar contract roster while the requires tail keeps each range as
//! a `Predicate` conjunction in the same constraint order: the roster is
//! exact endpoint evidence riding beside the clause, never a replacement.
use super::SymbolHandle;
use crate::tests::front_end::checked_program;
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

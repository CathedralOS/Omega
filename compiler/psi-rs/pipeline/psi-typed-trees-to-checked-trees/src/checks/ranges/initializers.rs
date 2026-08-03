use super::expressions::expression_integer_value;
use super::facts::RangeFacts;
use psi_typed_trees::machine::Machine;

pub(super) fn seed_field_integer_facts(
    program: &psi_typed_trees::TypedTrees,
    facts: &mut RangeFacts<'_>,
    machine: &Machine,
) {
    for owned in program.machine_owned_data(machine) {
        if !owned.initial_value.is_valid() {
            continue;
        }
        let Some(integer) = expression_integer_value(program, facts, owned.initial_value) else {
            continue;
        };
        facts.define_field_integer(owned.symbol, owned.name.to_string(), integer);
    }
}

use super::expressions::expression_integer_value;
use super::facts::RangeFacts;
use omega_typed_trees::machine::Machine;

pub(super) fn seed_field_integer_facts(
    program: &omega_typed_trees::TypedTrees,
    facts: &mut RangeFacts<'_>,
    machine: &Machine,
) {
    for data in program.data_definitions() {
        for member in program.data_members(data) {
            let omega_typed_trees::data::DataMember::Field(field) = member else {
                continue;
            };
            if !field.initial_value.is_valid() {
                continue;
            }
            let Some(integer) = expression_integer_value(program, facts, field.initial_value)
            else {
                continue;
            };
            facts.define_field_integer(field.symbol, field.name.to_string(), integer);
        }
    }

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

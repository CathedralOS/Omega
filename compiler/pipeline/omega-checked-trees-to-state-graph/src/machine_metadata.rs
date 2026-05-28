use omega_checked_trees::CheckedTrees;
use omega_checked_trees::machine::Machine;
use omega_core::arena::HandleSpan;
use omega_state_graph::{ContainedGraph, MachineOwnedDataGraph, StateGraph};

pub(crate) fn machine_owned_data(
    state_graph: &mut StateGraph,
    program: &CheckedTrees,
    machine: &Machine,
) -> HandleSpan<MachineOwnedDataGraph> {
    state_graph
        .machine_owned_data
        .insert_many(
            program
                .machine_owned_data(machine)
                .iter()
                .map(|data| MachineOwnedDataGraph {
                    symbol: data.symbol,
                    name: data.name.clone(),
                    type_reference: data.type_reference,
                }),
        )
}

pub(crate) fn machine_effect_bits(
    program: &CheckedTrees,
    machine_symbol: omega_core::symbols::SymbolHandle,
) -> (omega_effects::EffectBits, omega_effects::EffectBits) {
    program
        .facts
        .effects
        .machines()
        .iter()
        .find(|effects| effects.symbol == machine_symbol)
        .map(|effects| (effects.direct.bits(), effects.transitive.bits()))
        .unwrap_or_default()
}

pub(crate) fn state_effect_bits(
    program: &CheckedTrees,
    state_symbol: omega_core::symbols::SymbolHandle,
) -> (omega_effects::EffectBits, omega_effects::EffectBits) {
    program
        .facts
        .effects
        .machines()
        .iter()
        .flat_map(|machine| program.facts.effects.states.span_or_empty(machine.states))
        .find(|effects| effects.symbol == state_symbol)
        .map(|effects| (effects.direct.bits(), effects.transitive.bits()))
        .unwrap_or_default()
}

pub(crate) fn machine_contains(
    state_graph: &mut StateGraph,
    program: &CheckedTrees,
    machine: &Machine,
) -> HandleSpan<ContainedGraph> {
    let mut contains = HandleSpan::empty();
    for contained in program.machine_contained_objects(machine) {
        state_graph.contained_machines.append_to_span(
            &mut contains,
            ContainedGraph {
                symbol: contained.symbol,
                name: contained.name.clone(),
                type_symbol: contained.type_symbol,
                type_name: contained.type_name.clone(),
            },
        );
    }

    let Some(data_definition) = program
        .data_definitions()
        .iter()
        .find(|data_definition| Some(&data_definition.name) == machine.attached_data.as_ref())
    else {
        return contains;
    };

    for member in program.data_members(data_definition) {
        let omega_checked_trees::data::DataMember::Field(field) = member else {
            continue;
        };

        let field_type_name = type_reference_name_handle(program, field.type_reference);
        let Some(target_machine) = program
            .machines()
            .iter()
            .find(|candidate| candidate.attached_data.as_ref() == Some(&field_type_name))
        else {
            continue;
        };

        let contained_symbol = program
            .symbols
            .find_child_by_name(machine.symbol, field.name.as_str())
            .unwrap_or(field.symbol);

        if state_graph
            .contained_machines
            .span_or_empty(contains)
            .iter()
            .any(|contained| contained.symbol == contained_symbol)
        {
            continue;
        }

        state_graph.contained_machines.append_to_span(
            &mut contains,
            ContainedGraph {
                symbol: contained_symbol,
                name: field.name.clone(),
                type_symbol: target_machine.symbol,
                type_name: field_type_name,
            },
        );
    }

    contains
}

fn type_reference_name_handle(
    program: &CheckedTrees,
    type_reference: omega_checked_trees::types::TypeReferenceHandle,
) -> omega_checked_trees::name::Identifier {
    match program.type_reference_table.type_reference(type_reference) {
        omega_checked_trees::types::TypeReferenceNode::Reference { referee, .. } => {
            type_reference_name_handle(program, *referee)
        }
        omega_checked_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
            type_reference_name_handle(program, *base_type)
        }
        omega_checked_trees::types::TypeReferenceNode::FixedArray { element_type, .. } => {
            type_reference_name_handle(program, *element_type)
        }
        omega_checked_trees::types::TypeReferenceNode::Slice { element_type } => {
            type_reference_name_handle(program, *element_type)
        }
        omega_checked_trees::types::TypeReferenceNode::Generic { base_name, .. } => {
            base_name.clone()
        }
        omega_checked_trees::types::TypeReferenceNode::Named { name, .. } => name.clone(),
        omega_checked_trees::types::TypeReferenceNode::Unit => {
            omega_checked_trees::name::Identifier::default()
        }
    }
}

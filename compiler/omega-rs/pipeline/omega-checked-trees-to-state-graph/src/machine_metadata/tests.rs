use super::*;
use psi_arena::HandleSpan;
use psi_checked_trees::data::{DataDefinition, DataField, DataMember};
use psi_checked_trees::name::Identifier;
use psi_checked_trees::types::TypeReferenceNode;
use psi_checked_trees::{
    ContainedMachineFieldFact, ContainedMachineTargetFact, MachineCarryTopologyFact,
};
use psi_symbols::SymbolHandle;

#[test]
fn contained_topology_is_derived_only_from_fields_with_attached_machines() {
    let main_data_symbol = SymbolHandle::from_arena_index(1);
    let worker_data_symbol = SymbolHandle::from_arena_index(2);
    let scalar_data_symbol = SymbolHandle::from_arena_index(3);
    let main_machine_symbol = SymbolHandle::from_arena_index(4);
    let worker_machine_symbol = SymbolHandle::from_arena_index(5);
    let worker_field_symbol = SymbolHandle::from_arena_index(6);
    let scalar_field_symbol = SymbolHandle::from_arena_index(7);

    let mut program = CheckedTrees::default();
    let worker_type = program
        .typed
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: worker_data_symbol,
            name: Identifier::generated("Worker"),
        });
    let scalar_type = program
        .typed
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: scalar_data_symbol,
            name: Identifier::generated("Scalar"),
        });

    program.typed.push_data_definition(DataDefinition {
        symbol: worker_data_symbol,
        name: Identifier::generated("Worker"),
        ..Default::default()
    });
    program.typed.push_data_definition(DataDefinition {
        symbol: scalar_data_symbol,
        name: Identifier::generated("Scalar"),
        ..Default::default()
    });
    let mut main_data = DataDefinition {
        symbol: main_data_symbol,
        name: Identifier::generated("Main"),
        ..Default::default()
    };
    program.typed.push_data_member(
        &mut main_data,
        DataMember::Field(DataField {
            identity: None,
            symbol: worker_field_symbol,
            name: Identifier::generated("worker"),
            relevance: Default::default(),
            type_reference: worker_type,
        }),
    );
    program.typed.push_data_member(
        &mut main_data,
        DataMember::Field(DataField {
            identity: None,
            symbol: scalar_field_symbol,
            name: Identifier::generated("count"),
            relevance: Default::default(),
            type_reference: scalar_type,
        }),
    );
    program.typed.push_data_definition(main_data);

    program.typed.push_machine(Machine {
        symbol: worker_machine_symbol,
        name: Identifier::generated("Worker::run"),
        attached_data: Some(Identifier::generated("Worker")),
        ..Default::default()
    });
    let main_machine = Machine {
        symbol: main_machine_symbol,
        name: Identifier::generated("Main::main"),
        attached_data: Some(Identifier::generated("Main")),
        ..Default::default()
    };
    program.typed.push_machine(main_machine.clone());

    let targets = program
        .facts
        .carry
        .contained_targets
        .insert_many([ContainedMachineTargetFact {
            machine: worker_machine_symbol,
        }]);
    let fields = program
        .facts
        .carry
        .contained_fields
        .insert_many([ContainedMachineFieldFact {
            field: worker_field_symbol,
            data: worker_data_symbol,
            type_reference: worker_type,
            targets,
        }]);
    program
        .facts
        .carry
        .machine_topologies
        .insert(MachineCarryTopologyFact {
            machine: worker_machine_symbol,
            fields: HandleSpan::empty(),
        });
    program
        .facts
        .carry
        .machine_topologies
        .insert(MachineCarryTopologyFact {
            machine: main_machine_symbol,
            fields,
        });

    let mut graph = StateGraph::default();
    let contains = machine_contains(&mut graph, &program, &main_machine);
    let contained = graph.contained_machines.span_or_empty(contains);

    assert_eq!(contained.len(), 1);
    assert_eq!(contained[0].symbol, worker_field_symbol);
    assert_eq!(contained[0].name.as_str(), "worker");
    assert_eq!(contained[0].type_symbol, worker_machine_symbol);
    assert_eq!(contained[0].type_name.as_str(), "Worker");
}

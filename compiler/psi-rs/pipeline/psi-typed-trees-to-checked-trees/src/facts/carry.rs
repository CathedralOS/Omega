use psi_arena::{Arena, HandleSpan};
use psi_checked_trees::{
    CarryFacts, ContainedMachineFieldFact, ContainedMachineTargetFact, DataCarryFact,
    MachineCarryTopologyFact,
};
use psi_typed_trees::TypedTrees;

pub(super) fn build_carry_facts(program: &TypedTrees) -> CarryFacts {
    let data = program
        .data_definitions()
        .iter()
        .map(|definition| DataCarryFact {
            data: definition.symbol,
            declared: definition.properties.carry,
            effective: psi_validation::effective_data_carry_policy(program, definition),
        })
        .collect();
    let (machine_topologies, contained_fields, contained_targets) =
        build_contained_machine_topology(program);

    CarryFacts {
        data,
        machine_topologies,
        contained_fields,
        contained_targets,
        suspension_crossings: Vec::new(),
        activation_wide_carry: Vec::new(),
        claim_policies: Vec::new(),
    }
}

fn build_contained_machine_topology(
    program: &TypedTrees,
) -> (
    Arena<MachineCarryTopologyFact>,
    Arena<ContainedMachineFieldFact>,
    Arena<ContainedMachineTargetFact>,
) {
    let mut machine_topologies = Arena::with_capacity(program.machines().len());
    let mut contained_fields = Arena::new();
    let mut contained_targets = Arena::new();

    for machine in program.machines() {
        let mut fields = HandleSpan::empty();
        if let Some(attached_data) = machine.attached_data.as_ref()
            && let Some(definition) = program
                .data_definitions()
                .iter()
                .find(|definition| definition.name == *attached_data)
        {
            for member in program.data_members(definition) {
                let psi_typed_trees::data::DataMember::Field(field) = member else {
                    continue;
                };
                if field.relevance.is_erased() {
                    continue;
                }
                let data_symbol = program.type_reference_symbol(field.type_reference);
                let Some(field_data) = program
                    .data_definitions()
                    .iter()
                    .find(|definition| definition.symbol == data_symbol)
                else {
                    continue;
                };

                let targets = contained_targets.insert_many(
                    program
                        .machines()
                        .iter()
                        .filter(|candidate| {
                            candidate.attached_data.as_ref() == Some(&field_data.name)
                        })
                        .map(|candidate| ContainedMachineTargetFact {
                            machine: candidate.symbol,
                        }),
                );
                if targets.is_empty() {
                    continue;
                }

                contained_fields.append_to_span(
                    &mut fields,
                    ContainedMachineFieldFact {
                        field: field.symbol,
                        data: field_data.symbol,
                        type_reference: field.type_reference,
                        targets,
                    },
                );
            }
        }

        machine_topologies.append(MachineCarryTopologyFact {
            machine: machine.symbol,
            fields,
        });
    }

    (machine_topologies, contained_fields, contained_targets)
}

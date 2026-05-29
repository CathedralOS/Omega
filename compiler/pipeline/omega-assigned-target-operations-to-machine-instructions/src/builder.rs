use crate::shapes::lower_machine_instruction_kind;
use omega_assigned_target_operations::AssignedTargetOperationPlan;
use omega_core::arena::{Arena, HandleSpan};
use omega_core::diagnostics::Diagnostic;
use omega_machine_instructions::{
    MachineInstruction, MachineInstructionFunction, MachineInstructionPlan,
};

pub(crate) fn build_machine_instructions(
    assigned_target_operations: &AssignedTargetOperationPlan,
) -> Result<MachineInstructionPlan, Diagnostic> {
    let mut machine_instructions = MachineInstructionPlan::with_capacity(
        assigned_target_operations.target,
        assigned_target_operations.functions.len(),
        assigned_target_operations.instructions.len(),
    );
    machine_instructions.values = assigned_target_operations.values.clone();
    machine_instructions.boundary_edges = assigned_target_operations.boundary_edges.clone();
    machine_instructions.ownership = assigned_target_operations.ownership.clone();

    for (_, function) in assigned_target_operations.functions.iter() {
        let function_instructions = append_machine_instructions(
            assigned_target_operations,
            function,
            &mut machine_instructions.instructions,
        )?;

        machine_instructions
            .functions
            .insert(MachineInstructionFunction {
                source_key: function.source_key,
                instructions: function_instructions,
            });
    }

    Ok(machine_instructions)
}

fn append_machine_instructions(
    assigned_target_operations: &AssignedTargetOperationPlan,
    function: &omega_assigned_target_operations::AssignedTargetOperationFunction,
    output_instructions: &mut Arena<MachineInstruction>,
) -> Result<HandleSpan<MachineInstruction>, Diagnostic> {
    let Some(selected_instructions) = assigned_target_operations
        .instructions
        .span(function.instructions)
    else {
        return Ok(HandleSpan::empty());
    };

    output_instructions.try_insert_many(selected_instructions.iter().enumerate().map(
        |(selected_offset, selected_instruction)| {
            let selected_instruction_index = function
                .instructions
                .start()
                .arena_index()
                .checked_add(u32::try_from(selected_offset).expect("selected instruction overflow"))
                .expect("selected instruction overflow");
            let selected_instruction_handle =
                omega_core::arena::Handle::from_arena_index(selected_instruction_index);

            Ok(MachineInstruction {
                selected_instruction_index,
                source_kind: selected_instruction.kind.clone(),
                kind: lower_machine_instruction_kind(
                    assigned_target_operations,
                    selected_instruction_handle,
                    &selected_instruction.kind,
                )?,
            })
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_abstract_operations::{
        AbstractMoveEvent, AbstractOwnershipEventSource, AbstractSourceBoundaryEdge,
        AbstractValueFact, AbstractValueOrigin, AbstractValueStatementRole,
    };
    use omega_core::symbols::SymbolHandle;

    #[test]
    fn copies_assigned_value_summary_to_machine_instruction_plan() {
        let mut assigned_operations = AssignedTargetOperationPlan::default();
        let machine_symbol = SymbolHandle::from_arena_index(1);
        let state_symbol = SymbolHandle::from_arena_index(2);

        assigned_operations.values.values.insert(AbstractValueFact {
            source_key: Default::default(),
            machine_symbol,
            state_symbol,
            expression: Default::default(),
            origin: AbstractValueOrigin::Statement {
                statement_index: 11,
                role: AbstractValueStatementRole::TransitionGuard,
            },
        });

        let machine_instructions =
            build_machine_instructions(&assigned_operations).expect("machine instructions");

        assert_eq!(machine_instructions.values.values.len(), 1);
        let value = machine_instructions
            .values
            .values
            .iter()
            .next()
            .map(|(_, value)| value)
            .expect("machine value");
        assert_eq!(
            value.origin,
            AbstractValueOrigin::Statement {
                statement_index: 11,
                role: AbstractValueStatementRole::TransitionGuard,
            }
        );
    }

    #[test]
    fn copies_assigned_boundary_summary_to_machine_instruction_plan() {
        let mut assigned_operations = AssignedTargetOperationPlan::default();
        let trait_symbol = SymbolHandle::from_arena_index(1);
        let signature_symbol = SymbolHandle::from_arena_index(2);

        assigned_operations
            .boundary_edges
            .source_edges
            .insert(AbstractSourceBoundaryEdge {
                source_key: Default::default(),
                statement_index: 12,
                call_ordinal: 1,
                receiver_symbol: Default::default(),
                target_symbol: Default::default(),
                boundary_trait_symbol: trait_symbol,
                boundary_signature_symbol: signature_symbol,
            });

        let machine_instructions =
            build_machine_instructions(&assigned_operations).expect("machine instructions");

        assert_eq!(machine_instructions.boundary_edges.source_edges.len(), 1);
        let edge = machine_instructions
            .boundary_edges
            .source_edges
            .iter()
            .next()
            .map(|(_, edge)| edge)
            .expect("machine boundary edge");
        assert_eq!(edge.statement_index, 12);
        assert_eq!(edge.call_ordinal, 1);
        assert_eq!(edge.boundary_trait_symbol, trait_symbol);
        assert_eq!(edge.boundary_signature_symbol, signature_symbol);
    }

    #[test]
    fn copies_assigned_ownership_summary_to_machine_instruction_plan() {
        let mut assigned_operations = AssignedTargetOperationPlan::default();
        let target_symbol = SymbolHandle::from_arena_index(1);

        assigned_operations
            .ownership
            .moves
            .insert(AbstractMoveEvent {
                source_key: Default::default(),
                source: AbstractOwnershipEventSource::Call {
                    statement_index: 13,
                    call_ordinal: 2,
                    target_symbol,
                },
                root: Default::default(),
                segments: Default::default(),
            });

        let machine_instructions =
            build_machine_instructions(&assigned_operations).expect("machine instructions");

        assert_eq!(machine_instructions.ownership.moves.len(), 1);
        let event = machine_instructions
            .ownership
            .moves
            .iter()
            .next()
            .map(|(_, event)| event)
            .expect("machine ownership event");
        assert_eq!(
            event.source,
            AbstractOwnershipEventSource::Call {
                statement_index: 13,
                call_ordinal: 2,
                target_symbol,
            }
        );
    }
}

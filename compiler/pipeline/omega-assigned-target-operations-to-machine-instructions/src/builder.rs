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
}

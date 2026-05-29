use omega_core::diagnostics::Diagnostic;
use omega_machine_program::MachineProgram;
use omega_target_operations::InstructionPlan;

pub(crate) fn build_machine_program(
    instructions: &InstructionPlan,
) -> Result<MachineProgram, Diagnostic> {
    let assigned_target_operations =
        omega_target_operations_to_assigned_target_operations::build_assigned_target_operations(
            instructions,
        );
    let machine_instructions =
        omega_assigned_target_operations_to_machine_instructions::build_machine_instructions(
            &assigned_target_operations,
        )?;

    Ok(MachineProgram::from(machine_instructions))
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_abstract_operations::{
        AbstractValueFact, AbstractValueOrigin, AbstractValueStatementRole,
    };
    use omega_core::symbols::SymbolHandle;

    #[test]
    fn preserves_target_value_summary_into_machine_program() {
        let mut target_operations = InstructionPlan::default();
        let machine_symbol = SymbolHandle::from_arena_index(1);
        let state_symbol = SymbolHandle::from_arena_index(2);

        target_operations.values.values.insert(AbstractValueFact {
            source_key: Default::default(),
            machine_symbol,
            state_symbol,
            expression: Default::default(),
            origin: AbstractValueOrigin::Statement {
                statement_index: 13,
                role: AbstractValueStatementRole::TransitionTargetValue,
            },
        });

        let machine_program = build_machine_program(&target_operations).expect("machine program");

        assert_eq!(machine_program.values.values.len(), 1);
        let value = machine_program
            .values
            .values
            .iter()
            .next()
            .map(|(_, value)| value)
            .expect("machine-program value");
        assert_eq!(
            value.origin,
            AbstractValueOrigin::Statement {
                statement_index: 13,
                role: AbstractValueStatementRole::TransitionTargetValue,
            }
        );
    }
}

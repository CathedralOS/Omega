//! Keep installation’s refusal result false when allocation copies it into other registers.
//! Allocation may also regenerate zero near a use. These copies still represent
//! one source result, so every register must retain the same Boolean value.
use super::{
    Error, Graph,
    expressions::{Expr, Expressions},
};
use selected_instructions::{SelectedFunction, SelectedInstructionId, VirtualRegisterOrigin};
use semantic_vocabulary::{MachineId, OperationId, ScalarType, ValueId};

pub(super) fn validate(
    machine: MachineId,
    operation: OperationId,
    source_value: ValueId,
    selected: &SelectedFunction,
    validation: SelectedInstructionId,
    graph: &Graph<'_>,
    expressions: &mut Expressions<'_>,
) -> Result<(), Error> {
    let mut initialized = false;
    let mut carriers = 0usize;
    for result in &selected.virtual_registers {
        let VirtualRegisterOrigin::InstructionResult {
            instruction,
            source_value: value,
        } = result.origin
        else {
            continue;
        };
        if value != source_value {
            continue;
        }
        carriers += 1;
        let actual = expressions.defining_instruction(result.id)?;
        let value = expressions.value(result.id)?;
        if actual != instruction
            || result.scalar_type != ScalarType::Boolean
            || value != Expr::constant(0)
        {
            return Err(Error::InstallationResultDefinition {
                machine,
                operation,
                detail: format!(
                    "source={source_value:?}, register={:?}, retained definition={instruction:?}, actual definition={actual:?}, type={:?}, value={value:?}; expected Boolean false",
                    result.id, result.scalar_type
                ),
            });
        }
        // The source result is initialized before checking the table. Later
        // copies/rematerializations must still compute false, but can be placed
        // at their uses on refusal paths. Every carrier is checked above.
        initialized |= graph.precedes(actual, validation)?;
    }
    if carriers == 0 || !initialized {
        return Err(Error::InstallationResultDefinition {
            machine,
            operation,
            detail: format!(
                "source={source_value:?}, physical carriers={carriers}; no actual false definition dominates Validate"
            ),
        });
    }
    Ok(())
}

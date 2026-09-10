//! Rejoin the scalar call result to its current source and exact selected definition.
use crate::function_fragments::{Error, source};
use object_file::StagedOptimizedRelocationFreeObjectContainer;
use selected_instructions::SelectedFunction;

pub(super) fn result(
    source: &StagedOptimizedRelocationFreeObjectContainer,
    selected: &SelectedFunction,
    contract: &selected_instructions::SelectedCallContract,
) -> Result<Option<abstract_operations::AbstractResult>, Error> {
    let Some(placement) = &contract.call.result_placement else {
        return Ok(None);
    };
    let invalid = || Error::Mismatch("structural scalar call result differs from current source");
    let (abstracted, _) = source::function(source, selected.machine)?;
    let mut operations = abstracted
        .operations
        .iter()
        .filter_map(|operation| match operation {
            abstract_operations::AbstractOperation::CallStructuralScalar {
                psi_operation,
                result,
                callee,
                ..
            } if *psi_operation == contract.operation && *callee == contract.call.callee => {
                Some(*result)
            }
            _ => None,
        });
    let result = operations.next().ok_or_else(invalid)?;
    let (callee, target) = source::function(source, contract.call.callee)?;
    let abi = target
        .mixed_structural_scalar_abi
        .as_ref()
        .ok_or_else(invalid)?;
    if operations.next().is_some()
        || callee
            .result
            .scalar()
            .is_none_or(|value| value.scalar_type != result.scalar_type)
        || abi.result.scalar_type != result.scalar_type
        || abi.result.placement != *placement
        || abi.call_plan != contract.call.call_plan
    {
        return Err(invalid());
    }
    let mut instructions = selected
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter(|instruction| instruction.id == contract.instruction);
    let instruction = instructions.next().ok_or_else(invalid)?;
    let output = instruction
        .operands
        .last()
        .ok_or_else(invalid)?
        .virtual_register;
    if instructions.next().is_some()
        || instruction.kind
            != (selected_instructions::SelectedInstructionKind::CallScalar {
                callee: contract.call.callee,
            })
        || !selected.virtual_registers.iter().any(|register| {
            register.id == output && definition_is_exact(register, contract.instruction, result)
        })
    {
        return Err(invalid());
    }
    Ok(Some(result))
}

fn definition_is_exact(
    register: &selected_instructions::VirtualRegister,
    instruction: selected_instructions::SelectedInstructionId,
    result: abstract_operations::AbstractResult,
) -> bool {
    register.scalar_type == result.scalar_type
        && matches!(register.origin,
            selected_instructions::VirtualRegisterOrigin::InstructionResult { instruction: defining, source_value }
                if defining == instruction && source_value == result.value)
}

#[cfg(test)]
mod tests {
    use super::definition_is_exact;
    use selected_instructions::{
        SelectedInstructionId, VirtualRegister, VirtualRegisterId, VirtualRegisterOrigin,
    };
    use semantic_vocabulary::{ScalarType, ValueId};

    #[test]
    fn scalar_call_result_requires_the_exact_call_definition_and_value() {
        let instruction = SelectedInstructionId(7);
        let result = abstract_operations::AbstractResult {
            value: ValueId::new(9).unwrap(),
            scalar_type: ScalarType::Integer(
                semantic_vocabulary::IntegerType::new(
                    semantic_vocabulary::IntegerSign::Unsigned,
                    64,
                )
                .unwrap(),
            ),
        };
        let register = VirtualRegister {
            id: VirtualRegisterId(2),
            scalar_type: result.scalar_type,
            class: register_model::RegisterClassId(0),
            origin: VirtualRegisterOrigin::InstructionResult {
                instruction,
                source_value: result.value,
            },
            definition_site: None,
            entry_fixed_view: None,
        };
        assert!(definition_is_exact(&register, instruction, result));
        for mutation in 0..4 {
            let mut changed = register.clone();
            match mutation {
                0 => changed.scalar_type = ScalarType::Boolean,
                1 => {
                    changed.origin = VirtualRegisterOrigin::InstructionResult {
                        instruction: SelectedInstructionId(8),
                        source_value: result.value,
                    }
                }
                2 => {
                    changed.origin = VirtualRegisterOrigin::InstructionResult {
                        instruction,
                        source_value: ValueId::new(10).unwrap(),
                    }
                }
                _ => {
                    changed.origin = VirtualRegisterOrigin::EntryParameter {
                        source_value: result.value,
                        parameter_index: 0,
                    }
                }
            }
            assert!(
                !definition_is_exact(&changed, instruction, result),
                "mutation {mutation}"
            );
        }
    }
}

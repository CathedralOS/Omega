//! Function parameter, block-flattening, and operation-offset projection.

use omega_abstract_operations::{AbstractBlockEntry, AbstractFunction};
use omega_optimization_unit::{PsiOptimizationFunction, ValueDefinitionSite};

use super::project_parameter;
use crate::OptimizedAbstractProjectionError;

pub(super) fn project(
    unit: &PsiOptimizationFunction,
) -> Result<AbstractFunction, OptimizedAbstractProjectionError> {
    let parameters = unit
        .parameters
        .iter()
        .enumerate()
        .map(|(position, definition)| {
            project_parameter(
                definition,
                ValueDefinitionSite::FunctionParameter(u32::try_from(position).map_err(|_| {
                    OptimizedAbstractProjectionError::InvalidFunctionParameter {
                        machine: unit.machine,
                        position,
                    }
                })?),
                OptimizedAbstractProjectionError::InvalidFunctionParameter {
                    machine: unit.machine,
                    position,
                },
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut operation_offset = 0usize;
    let mut block_entries = Vec::with_capacity(unit.blocks.len());
    let mut operations = Vec::new();
    for block in &unit.blocks {
        let parameters = block
            .parameters
            .iter()
            .enumerate()
            .map(|(position, definition)| {
                project_parameter(
                    definition,
                    ValueDefinitionSite::BlockParameter {
                        block: block.id,
                        position: u32::try_from(position).map_err(|_| {
                            OptimizedAbstractProjectionError::InvalidBlockParameter {
                                machine: unit.machine,
                                position,
                            }
                        })?,
                    },
                    OptimizedAbstractProjectionError::InvalidBlockParameter {
                        machine: unit.machine,
                        position,
                    },
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        block_entries.push(AbstractBlockEntry {
            block: block.id,
            parameters,
            operation_offset,
        });
        operation_offset = operation_offset.checked_add(block.nodes.len()).ok_or(
            OptimizedAbstractProjectionError::OperationOffsetOverflow(unit.machine),
        )?;
        operations.extend(block.nodes.iter().map(|node| node.operation.clone()));
    }
    Ok(AbstractFunction {
        machine: unit.machine,
        attachment: unit.attachment,
        entry: unit.entry,
        parameters,
        structural_parameters: unit.structural_parameters.clone(),
        result: unit.result.clone(),
        entry_claims: unit.entry_claim_declarations.clone(),
        published_service_ceiling: unit.published_service_ceiling.clone(),
        block_entries,
        operations,
    })
}

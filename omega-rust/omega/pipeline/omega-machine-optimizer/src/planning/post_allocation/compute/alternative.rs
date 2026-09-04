//! Exact physical-home applicability and unique alternative selection.

use omega_register_model::ValidatedPhysicalRegisterModel;
use omega_selected_instructions::{MachineAlternative, MachineAlternativeApplicability};

use crate::{PhysicalOperandFootprint, PostAllocationMachineError};

pub(super) fn choose(
    instruction: u32,
    operands: &[PhysicalOperandFootprint],
    alternatives: &[MachineAlternative],
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<MachineAlternative, PostAllocationMachineError> {
    let mut applicable = Vec::new();
    for alternative in alternatives {
        if is_applicable(instruction, operands, alternative.applicability, physical)? {
            applicable.push(alternative.clone());
        }
    }
    match applicable.as_slice() {
        [alternative] => Ok(alternative.clone()),
        [] => Err(PostAllocationMachineError::NoApplicableAlternative { instruction }),
        _ => Err(PostAllocationMachineError::AmbiguousApplicableAlternatives { instruction }),
    }
}

fn is_applicable(
    instruction: u32,
    operands: &[PhysicalOperandFootprint],
    applicability: MachineAlternativeApplicability,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<bool, PostAllocationMachineError> {
    let view = |operand| {
        operands
            .iter()
            .find(|candidate| candidate.operand == operand)
            .map(|operand| operand.view)
            .ok_or(PostAllocationMachineError::MissingApplicabilityOperand {
                instruction,
                operand,
            })
    };
    let aliases = |left, right| physical.model().aliases(left, right);
    Ok(match applicability {
        MachineAlternativeApplicability::Always => true,
        MachineAlternativeApplicability::ResultAliasesOperand { result, operand } => {
            aliases(view(result)?, view(operand)?)
        }
        MachineAlternativeApplicability::ResultAliasesOperandAndDistinctFromOperand {
            result,
            aliased_operand,
            distinct_operand,
        } => {
            let result = view(result)?;
            aliases(result, view(aliased_operand)?) && !aliases(result, view(distinct_operand)?)
        }
        MachineAlternativeApplicability::ResultAliasesOperands {
            result,
            left,
            right,
        } => {
            let result = view(result)?;
            aliases(result, view(left)?) && aliases(result, view(right)?)
        }
        MachineAlternativeApplicability::ResultDistinctFromOperands {
            result,
            left,
            right,
        } => {
            let result = view(result)?;
            !aliases(result, view(left)?) && !aliases(result, view(right)?)
        }
        MachineAlternativeApplicability::AtLeastOneOperandDoesNotAliasView {
            left,
            right,
            excluded_view,
        } => !aliases(view(left)?, excluded_view) || !aliases(view(right)?, excluded_view),
    })
}

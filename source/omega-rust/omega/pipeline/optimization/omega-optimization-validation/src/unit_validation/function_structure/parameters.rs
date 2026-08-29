//! Entry-claim and parameter-definition metadata contracts.

use super::*;

pub(super) fn validate_entry_claim_index(
    function: &PsiOptimizationFunction,
) -> Result<(), OptimizationUnitValidationError> {
    let indexed_entry_claims = function
        .entry_claim_declarations
        .iter()
        .map(|claim| claim.claim)
        .collect::<BTreeSet<_>>();
    if indexed_entry_claims.len() != function.entry_claim_declarations.len()
        || indexed_entry_claims != function.entry_claims
    {
        return Err(OptimizationUnitValidationError::EntryClaimIndexMismatch(
            function.machine,
        ));
    }
    Ok(())
}

pub(crate) fn validate_parameter_metadata(
    function: &PsiOptimizationFunction,
) -> Result<(), OptimizationUnitValidationError> {
    for (position, parameter) in function.parameters.iter().enumerate() {
        let Ok(position) = u32::try_from(position) else {
            return Err(OptimizationUnitValidationError::ParameterMetadataMismatch {
                machine: function.machine,
                block: None,
            });
        };
        if parameter.site != ValueDefinitionSite::FunctionParameter(position) {
            return Err(OptimizationUnitValidationError::ParameterMetadataMismatch {
                machine: function.machine,
                block: None,
            });
        }
    }
    for block in &function.blocks {
        for (position, parameter) in block.parameters.iter().enumerate() {
            let Ok(position) = u32::try_from(position) else {
                return Err(OptimizationUnitValidationError::ParameterMetadataMismatch {
                    machine: function.machine,
                    block: Some(block.id),
                });
            };
            if parameter.site
                != (ValueDefinitionSite::BlockParameter {
                    block: block.id,
                    position,
                })
            {
                return Err(OptimizationUnitValidationError::ParameterMetadataMismatch {
                    machine: function.machine,
                    block: Some(block.id),
                });
            }
        }
    }
    Ok(())
}

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

pub(super) fn validate_dynamic_descriptor_parameters(
    function: &PsiOptimizationFunction,
) -> Result<(), OptimizationUnitValidationError> {
    let entry = function
        .blocks
        .iter()
        .find(|block| block.id == function.entry)
        .ok_or(
            OptimizationUnitValidationError::DynamicDescriptorParameterMismatch(function.machine),
        )?;
    let mut parameters = BTreeMap::new();
    let mut declaration_prefix_open = true;
    for node in &entry.nodes {
        match &node.operation {
            abstract_operations::AbstractOperation::DynamicDescriptorParameter { parameter }
                if declaration_prefix_open =>
            {
                let expected_ordinal = u32::try_from(parameters.len()).map_err(|_| {
                    OptimizationUnitValidationError::DynamicDescriptorParameterMismatch(
                        function.machine,
                    )
                })?;
                if parameter.owner != function.machine
                    || parameter.ordinal != expected_ordinal
                    || parameters.insert(parameter.ordinal, parameter).is_some()
                {
                    return Err(
                        OptimizationUnitValidationError::DynamicDescriptorParameterMismatch(
                            function.machine,
                        ),
                    );
                }
            }
            abstract_operations::AbstractOperation::DynamicDescriptorParameter { .. } => {
                return Err(
                    OptimizationUnitValidationError::DynamicDescriptorParameterMismatch(
                        function.machine,
                    ),
                );
            }
            _ => declaration_prefix_open = false,
        }
    }
    if function.blocks.iter().any(|block| {
        block.id != function.entry
            && block.nodes.iter().any(|node| {
                matches!(
                    node.operation,
                    abstract_operations::AbstractOperation::DynamicDescriptorParameter { .. }
                )
            })
    }) {
        return Err(
            OptimizationUnitValidationError::DynamicDescriptorParameterMismatch(function.machine),
        );
    }
    for node in function.blocks.iter().flat_map(|block| &block.nodes) {
        if let abstract_operations::AbstractOperation::CallDynamicParameterScalar {
            dynamic_dispatch,
            ..
        } = &node.operation
        {
            let parameter = &dynamic_dispatch.parameter;
            if parameters.get(&parameter.ordinal).copied() != Some(parameter) {
                return Err(
                    OptimizationUnitValidationError::DynamicDescriptorParameterMismatch(
                        function.machine,
                    ),
                );
            }
        }
    }
    Ok(())
}

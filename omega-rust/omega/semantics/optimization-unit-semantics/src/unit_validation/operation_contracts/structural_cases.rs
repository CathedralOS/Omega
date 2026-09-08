//! Closed case rosters and edge-produced scalar payload telescopes.

use super::*;

pub(super) fn validate(
    function: &PsiOptimizationFunction,
    source: PlaceId,
    successors: &[abstract_operations::AbstractStructuralCaseSuccessor],
    blocks: &BTreeMap<BlockId, &optimization_unit::OptimizationBlock>,
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
) -> Result<(), OptimizationUnitValidationError> {
    let invalid = || OptimizationUnitValidationError::InvalidStructuralCaseDispatch {
        machine: function.machine,
        source,
    };
    let signature = structural_source_contract(function, source, false).ok_or_else(invalid)?;
    if signature.access == terminal_psi::StructuralAccess::WriteOnlyBorrow {
        return Err(invalid());
    }
    let declaration = types.get(&signature.structural_type).ok_or_else(invalid)?;
    let cases = match &declaration.shape {
        terminal_psi::StructuralTypeShape::Sum { cases }
        | terminal_psi::StructuralTypeShape::Mixed { cases, .. } => cases,
        _ => return Err(invalid()),
    };
    if successors.len() != cases.len() {
        return Err(invalid());
    }
    for (successor, case) in successors.iter().zip(cases) {
        if successor.case != case.id {
            return Err(invalid());
        }
        let target = blocks.get(&successor.target).ok_or_else(invalid)?;
        if successor.payloads.len() != target.parameters.len()
            || !target.structural_parameters.is_empty()
        {
            return Err(invalid());
        }
        for (payload, parameter) in successor.payloads.iter().zip(&target.parameters) {
            let field = case
                .fields
                .iter()
                .find(|field| field.id == payload.field)
                .ok_or_else(invalid)?;
            if payload.parameter != parameter.value
                || payload.scalar_type != parameter.scalar_type
                || field.relevance.is_erased()
                || !matches!(field.field_type, terminal_psi::StructuralFieldType::Scalar(actual)
                    if actual == payload.scalar_type)
            {
                return Err(invalid());
            }
        }
    }
    Ok(())
}

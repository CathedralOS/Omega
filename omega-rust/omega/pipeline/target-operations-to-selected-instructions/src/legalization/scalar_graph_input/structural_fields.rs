use super::*;

pub(in crate::legalization) fn read(
    function: &PsiOptimizationFunction,
    operation: &AbstractOperation,
    types: &[terminal_psi::StructuralTypeDeclaration],
) -> Option<(
    semantic_vocabulary::OperationId,
    abstract_operations::AbstractResult,
    terminal_psi::StructuralArgument,
    semantic_vocabulary::StructuralFieldId,
)> {
    let (operation, result, place, field) = match operation {
        AbstractOperation::IntegerStructuralField {
            psi_operation,
            result,
            source,
            field,
        } => (*psi_operation, *result, *source, *field),
        AbstractOperation::BooleanStructuralField {
            psi_operation,
            result,
            source,
            field,
        } => (
            *psi_operation,
            abstract_operations::AbstractResult {
                value: *result,
                scalar_type: ScalarType::Boolean,
            },
            *source,
            *field,
        ),
        _ => return None,
    };
    if function
        .entry_claim_declarations
        .iter()
        .any(|claim| claim.input == place)
        || function
            .content_entry_claims
            .iter()
            .any(|claim| claim.input.root == place)
    {
        return None;
    }
    // Entry-owned records use inline ABI fragments, not a readable retained
    // home in this load route. Owned block arrivals do have an established home.
    if function.structural_parameters.iter().any(|parameter| {
        parameter.place == place && parameter.access == terminal_psi::StructuralAccess::Owned
    }) {
        return None;
    }
    let (structural_type, access) = if let Some(parameter) = function
        .structural_parameters
        .iter()
        .chain(
            function
                .blocks
                .iter()
                .flat_map(|block| &block.structural_parameters),
        )
        .find(|parameter| parameter.place == place)
    {
        if parameter.access == terminal_psi::StructuralAccess::WriteOnlyBorrow
            || parameter.multiplicity == terminal_psi::StructuralMultiplicity::Linear
            || !parameter.qualifications.is_empty()
            || !parameter.projected_qualifications.is_empty()
        {
            return None;
        }
        (parameter.structural_type, parameter.access)
    } else {
        let (_, result) = super::structural_case::source_result(function, place).ok()?;
        if result.multiplicity == terminal_psi::StructuralMultiplicity::Linear
            || !result.qualifications.is_empty()
            || !result.projected_qualifications.is_empty()
            || !result.claims.is_empty()
        {
            return None;
        }
        (
            result.structural_type,
            terminal_psi::StructuralAccess::Owned,
        )
    };
    let source = terminal_psi::StructuralArgument {
        place,
        access,
        path: Vec::new(),
    };
    crate::structural_reference_input::field_read(
        structural_type,
        field,
        result.scalar_type,
        types,
    )?;
    Some((operation, result, source, field))
}

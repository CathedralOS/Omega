use super::{AbstractOperation, PsiOptimizationFunction, ScalarType};
pub(in crate::legalization) fn replacement(
    function: &PsiOptimizationFunction,
    operation: &AbstractOperation,
    types: &[terminal_psi::StructuralTypeDeclaration],
) -> Option<terminal_psi::StructuralArgument> {
    let (destination, path, field, source) = match operation {
        AbstractOperation::StructuralByteSequenceFieldStore {
            destination,
            path,
            field,
            source,
            ..
        } => (destination, path, field, Some(*source)),
        AbstractOperation::StructuralByteSequenceFieldByteStore {
            destination,
            path,
            field,
            ..
        } => (destination, path, field, None),
        _ => return None,
    };
    let parameter = function
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == *destination)?;
    if !matches!(
        parameter.access,
        terminal_psi::StructuralAccess::MutableBorrow
            | terminal_psi::StructuralAccess::WriteOnlyBorrow
    ) || parameter.multiplicity == terminal_psi::StructuralMultiplicity::Linear
        || !parameter.qualifications.is_empty()
        || !parameter.projected_qualifications.is_empty()
        || source == Some(*destination)
        || function
            .entry_claim_declarations
            .iter()
            .any(|claim| claim.input == *destination || Some(claim.input) == source)
        || function
            .content_entry_claims
            .iter()
            .any(|claim| claim.input.root == *destination || Some(claim.input.root) == source)
        || source.is_some_and(|source| !super::byte_views::contains_view(function, source))
    {
        return None;
    }
    crate::structural_inputs::structural_reference_input::byte_field_storage(
        parameter.structural_type,
        path,
        *field,
        types,
    )?;
    Some(terminal_psi::StructuralArgument {
        place: *destination,
        access: parameter.access,
        path: path.clone(),
    })
}

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
    if let AbstractOperation::StructuralByteSequenceFieldLength {
        psi_operation,
        result,
        source,
        path,
        field,
    } = operation
    {
        let parameter = function
            .structural_parameters
            .iter()
            .find(|parameter| parameter.place == *source)?;
        if !matches!(
            parameter.access,
            terminal_psi::StructuralAccess::SharedBorrow
                | terminal_psi::StructuralAccess::MutableBorrow
                | terminal_psi::StructuralAccess::WriteOnlyBorrow
        ) || parameter.multiplicity == terminal_psi::StructuralMultiplicity::Linear
            || !parameter.qualifications.is_empty()
            || !parameter.projected_qualifications.is_empty()
            || function
                .entry_claim_declarations
                .iter()
                .any(|claim| claim.input == *source)
            || function
                .content_entry_claims
                .iter()
                .any(|claim| claim.input.root == *source)
        {
            return None;
        }
        crate::structural_inputs::structural_reference_input::byte_field_length(
            parameter.structural_type,
            path,
            *field,
            result.scalar_type,
            types,
        )?;
        return Some((
            *psi_operation,
            *result,
            terminal_psi::StructuralArgument {
                place: *source,
                access: parameter.access,
                path: path.clone(),
            },
            *field,
        ));
    }
    let (operation, result, place, path, field) = match operation {
        AbstractOperation::IntegerStructuralField {
            psi_operation,
            result,
            source,
            path,
            field,
        } => (*psi_operation, *result, *source, path, *field),
        AbstractOperation::BooleanStructuralField {
            psi_operation,
            result,
            source,
            path,
            field,
        } => (
            *psi_operation,
            abstract_operations::AbstractResult {
                value: *result,
                scalar_type: ScalarType::Boolean,
            },
            *source,
            path,
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
    // Access belongs to the exact declaration. Owned input observations retain
    // the value ABI; selection must supply captured value storage, not reinterpret
    // an incoming value fragment as the address used by a borrowed parameter.
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
    let mut carrier = structural_type;
    let mut runtime_path = Vec::with_capacity(path.len());
    for (position, segment) in path.iter().enumerate() {
        match segment {
            semantic_vocabulary::CanonicalStructuralPathSegment::Field(field) => {
                let terminal_psi::StructuralTypeShape::Record { fields } = &types
                    .iter()
                    .find(|declaration| declaration.id == carrier)?
                    .shape
                else {
                    return None;
                };
                let selected = fields
                    .iter()
                    .find(|candidate| candidate.id == *field && !candidate.relevance.is_erased())?;
                let child = match &selected.field_type {
                    terminal_psi::StructuralFieldType::Structural(child) => *child,
                    leaf => {
                        let shape = leaf.canonical_leaf_shape()?;
                        types
                            .iter()
                            .find(|declaration| declaration.shape == shape)
                            .map(|declaration| declaration.id)?
                    }
                };
                runtime_path.push(terminal_psi::StructuralPathSegment::Field(
                    selected.identity.clone(),
                ));
                carrier = child;
            }
            // The bounded carrier grammar ends with at most one literal
            // index: the fixed-array element is the record owning the
            // observed field. The bound is rechecked against the declared
            // extent rather than trusted from the producer.
            semantic_vocabulary::CanonicalStructuralPathSegment::FixedIndex(index)
                if position + 1 == path.len() =>
            {
                let terminal_psi::StructuralTypeShape::FixedArray { element, length } = &types
                    .iter()
                    .find(|declaration| declaration.id == carrier)?
                    .shape
                else {
                    return None;
                };
                if index >= length {
                    return None;
                }
                runtime_path.push(terminal_psi::StructuralPathSegment::FixedIndex(*index));
                carrier = *element;
            }
            _ => return None,
        }
    }
    let source = terminal_psi::StructuralArgument {
        place,
        access,
        path: runtime_path,
    };
    crate::structural_inputs::structural_reference_input::field_read(
        structural_type,
        &source.path,
        field,
        result.scalar_type,
        types,
    )?;
    Some((operation, result, source, field))
}

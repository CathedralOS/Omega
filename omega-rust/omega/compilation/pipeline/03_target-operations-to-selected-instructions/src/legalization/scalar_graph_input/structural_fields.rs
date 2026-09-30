use super::{AbstractOperation, PsiOptimizationFunction, ScalarType};
pub(in crate::legalization) fn replacement(
    function: &PsiOptimizationFunction,
    operation: &AbstractOperation,
    types: &[terminal_psi::StructuralTypeDeclaration],
) -> Option<terminal_psi::StructuralArgument> {
    if let AbstractOperation::ByteSequenceStore {
        destination,
        source,
        ..
    } = operation
    {
        let parameter = function
            .structural_parameters
            .iter()
            .find(|parameter| parameter.place == *destination)?;
        if source == destination
            || !matches!(
                parameter.access,
                terminal_psi::StructuralAccess::MutableBorrow
                    | terminal_psi::StructuralAccess::WriteOnlyBorrow
            )
            || parameter.multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted
            || !parameter.projected_qualifications.is_empty()
            || !types.iter().any(|declaration| {
                declaration.id == parameter.structural_type
                    && matches!(
                        declaration.shape,
                        terminal_psi::StructuralTypeShape::ByteSequence(
                            terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
                        )
                    )
            })
            || !super::byte_views::contains_readable_bytes(function, *source)
            || function
                .entry_claim_declarations
                .iter()
                .any(|claim| claim.input == *destination || claim.input == *source)
            || function
                .content_entry_claims
                .iter()
                .any(|claim| claim.input.root == *destination || claim.input.root == *source)
        {
            return None;
        }
        return Some(terminal_psi::StructuralArgument {
            place: *destination,
            access: parameter.access,
            path: Vec::new(),
        });
    }
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
        || source
            .is_some_and(|source| !super::byte_views::contains_readable_bytes(function, source))
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

/// The borrowed parameter a bounded byte field's length or byte observation
/// reads through: any borrow of an unqualified, non-linear parameter that no
/// entry claim holds.
fn readable_byte_field_parameter(
    function: &PsiOptimizationFunction,
    source: semantic_vocabulary::PlaceId,
) -> Option<&terminal_psi::StructuralParameterDeclaration> {
    let parameter = function
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == source)?;
    (matches!(
        parameter.access,
        terminal_psi::StructuralAccess::SharedBorrow
            | terminal_psi::StructuralAccess::MutableBorrow
            | terminal_psi::StructuralAccess::WriteOnlyBorrow
    ) && parameter.multiplicity != terminal_psi::StructuralMultiplicity::Linear
        && parameter.qualifications.is_empty()
        && parameter.projected_qualifications.is_empty()
        && function
            .entry_claim_declarations
            .iter()
            .all(|claim| claim.input != source)
        && function
            .content_entry_claims
            .iter()
            .all(|claim| claim.input.root != source))
    .then_some(parameter)
}

/// The bounded byte field one indexed byte read observes, as the structural
/// argument selection reconstructs its storage from.
pub(in crate::legalization) fn byte_read(
    function: &PsiOptimizationFunction,
    operation: &AbstractOperation,
    types: &[terminal_psi::StructuralTypeDeclaration],
) -> Option<terminal_psi::StructuralArgument> {
    let AbstractOperation::StructuralByteSequenceFieldRead {
        result,
        source,
        path,
        field,
        ..
    } = operation
    else {
        return None;
    };
    let parameter = readable_byte_field_parameter(function, *source)?;
    // A write-only borrow may observe the live length but not the bytes.
    if parameter.access == terminal_psi::StructuralAccess::WriteOnlyBorrow
        || result.scalar_type
            != ScalarType::Integer(
                semantic_vocabulary::IntegerType::new(
                    semantic_vocabulary::IntegerSign::Unsigned,
                    8,
                )
                .ok()?,
            )
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
        place: *source,
        access: parameter.access,
        path: path.clone(),
    })
}

/// One scalar field observation's legal shape: the defining operation and
/// result, the runtime carrier, the declared case whose payload holds the
/// field (absent for a record field), and the observed field.
pub(in crate::legalization) type FieldObservation = (
    semantic_vocabulary::OperationId,
    abstract_operations::AbstractResult,
    terminal_psi::StructuralArgument,
    Option<semantic_vocabulary::StructuralCaseId>,
    semantic_vocabulary::StructuralFieldId,
);

pub(in crate::legalization) fn read(
    function: &PsiOptimizationFunction,
    operation: &AbstractOperation,
    types: &[terminal_psi::StructuralTypeDeclaration],
) -> Option<FieldObservation> {
    if let AbstractOperation::StructuralByteSequenceFieldLength {
        psi_operation,
        result,
        source,
        path,
        field,
    } = operation
    {
        let parameter = readable_byte_field_parameter(function, *source)?;
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
            None,
            *field,
        ));
    }
    let AbstractOperation::StructuralScalarFieldRead {
        psi_operation: operation,
        result,
        source: place,
        path,
        field,
    } = operation
    else {
        return None;
    };
    let (operation, result, place, field) = (*operation, *result, *place, *field);
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
    // A final `Case` step selects which payload overlay holds the field. It
    // names no storage of its own: the payload lives at the sum's place, so
    // the runtime carrier stops at the sum and the case travels beside it.
    let (carrier_path, case) = match path.split_last() {
        Some((semantic_vocabulary::CanonicalStructuralPathSegment::Case(case), prefix)) => {
            (prefix, Some(*case))
        }
        _ => (path.as_slice(), None),
    };
    let runtime_path = runtime_path(structural_type, carrier_path, types)?;
    let source = terminal_psi::StructuralArgument {
        place,
        access,
        path: runtime_path,
    };
    match case {
        None => crate::structural_inputs::structural_reference_input::field_read(
            structural_type,
            &source.path,
            field,
            result.scalar_type,
            types,
        )?,
        Some(case) => crate::structural_inputs::structural_reference_input::case_field_read(
            structural_type,
            &source.path,
            case,
            field,
            result.scalar_type,
            types,
        )?,
    };
    Some((operation, result, source, case, field))
}

/// The runtime spelling of a canonical projection beneath `root`: each field
/// by its declared identity, and at most one literal index as the final
/// segment, bounded by the declared extent.
fn runtime_path(
    root: semantic_vocabulary::StructuralTypeId,
    path: &[semantic_vocabulary::CanonicalStructuralPathSegment],
    types: &[terminal_psi::StructuralTypeDeclaration],
) -> Option<Vec<terminal_psi::StructuralPathSegment>> {
    let mut carrier = root;
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
    Some(runtime_path)
}

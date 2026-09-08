//! Exact bounded field metadata and indexed mutation custody.

use super::*;

mod freshness;
pub(crate) use freshness::replacement_length_equation;

pub(super) fn validate(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
) -> Result<(), ModuleError> {
    let invalid = || ModuleError::InvalidStructuralByteSequenceFieldAccess(operation.id);
    let (root, path, field, writing) = match &operation.kind {
        OperationKind::StructuralByteSequenceFieldLength {
            source,
            path,
            field,
        } => (*source, path, *field, false),
        OperationKind::StructuralByteSequenceFieldByteStore {
            destination,
            path,
            field,
            ..
        } => (*destination, path, *field, true),
        _ => return Err(invalid()),
    };
    let parameter = machine
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == root)
        .ok_or_else(invalid)?;
    if !matches!(
        parameter.access,
        StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
    ) && (writing || parameter.access != StructuralAccess::SharedBorrow)
        || !matches!(
            parameter.multiplicity,
            StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
        )
        || !parameter.qualifications.is_empty()
        || !parameter.projected_qualifications.is_empty()
        || !terminal_psi::is_bounded_structural_scalar_store_path(path)
        || !machine.structural_places.iter().any(|place| {
            place.id == root
                && place.kind
                    == StructuralPlaceKind::Parameter {
                        position: parameter.position,
                        is_self: parameter.is_self,
                    }
        })
        || machine.entry_claims.iter().any(|claim| claim.input == root)
        || machine
            .content_entry_claims
            .iter()
            .any(|claim| claim.input.root == root)
    {
        return Err(invalid());
    }
    field_path(module, machine, root, path, field).ok_or_else(invalid)?;
    if writing {
        if operation.result != OperationResult::Unit
            || !freshness::exact_length_is_current(module, machine, operation)
        {
            return Err(invalid());
        }
    } else if operation
        .result
        .scalar()
        .is_none_or(|result| result.scalar_type != byte_count_type())
    {
        return Err(invalid());
    }
    Ok(())
}

fn byte_count_type() -> ScalarType {
    ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).expect("u64 is valid"))
}

/// Resolve the field identity before comparing a call's projected write path.
fn field_path(
    module: &TerminalModule,
    machine: &TerminalMachine,
    root: PlaceId,
    path: &[StructuralPathSegment],
    field: StructuralFieldId,
) -> Option<Vec<StructuralPathSegment>> {
    let parameter = machine
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == root)?;
    let carrier = resolve_structural_path(module, parameter.structural_type, path)?;
    let declaration = module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == carrier)?;
    let StructuralTypeShape::Record { fields } = &declaration.shape else {
        return None;
    };
    let field = fields
        .iter()
        .find(|candidate| candidate.id == field && !candidate.relevance.is_erased())?;
    if !matches!(
        field.field_type,
        StructuralFieldType::ByteSequence(terminal_psi::ByteSequenceCarrier::BoundedOwned { .. })
    ) {
        return None;
    }
    let mut exact = path.to_vec();
    exact.push(StructuralPathSegment::Field(field.identity.clone()));
    Some(exact)
}

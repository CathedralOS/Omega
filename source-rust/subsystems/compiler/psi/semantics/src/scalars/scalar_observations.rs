#[cfg(test)]
mod tests;

use crate::exactly_one::exactly_one;
use semantic_vocabulary::{
    CanonicalStructuralPathSegment as Static, IntegerSign, IntegerType, Proposition,
    ScalarReadProjection, ScalarTerm, ScalarType, StructuralTypeId,
};
use terminal_psi::{
    ByteSequenceCarrier, Operation, OperationKind, ScalarApplications, StructuralAccess,
    StructuralFieldDeclaration, StructuralFieldType, StructuralTypeDeclaration,
    StructuralTypeShape,
};

/// A read returns the immutable value of its exact catalog application.
/// This supplies no numeric measurement: layout and evaluation still owe
/// their own admission. The declaration fixes the result's carrier.
pub fn scalar_application_read_equation(
    applications: &ScalarApplications,
    operation: &Operation,
) -> Result<Option<Proposition>, &'static str> {
    let OperationKind::ReadScalarApplication { application } = operation.kind else {
        return Ok(None);
    };
    let carrier = applications
        .declaration(application)
        .and_then(|declaration| declaration.scalar_type())
        .ok_or("scalar application read has no catalog declaration")?;
    let result = operation
        .result
        .scalar_ref()
        .filter(|result| result.scalar_type == carrier)
        .ok_or("scalar application read result differs from its declared carrier")?;
    Ok(Some(Proposition::Equal(
        ScalarTerm::value(result.id, carrier),
        ScalarTerm::Application {
            application,
            scalar_type: carrier,
        },
    )))
}

/// Every observation returns a positive guarantee. A fresh inline allocation
/// additionally proves the alignment shared by its base, offset and selected
/// carrier when those measurements are already target independent. An
/// arbitrary borrowed referent or representation view has no such premise.
pub fn storage_alignment_fact(
    types: &[StructuralTypeDeclaration],
    machine: &terminal_psi::TerminalMachine,
    operation: &Operation,
) -> Result<Option<Proposition>, &'static str> {
    let OperationKind::StorageAlignment { source } = &operation.kind else {
        return Ok(None);
    };
    let result = operation
        .result
        .scalar_ref()
        .filter(|result| {
            terminal_psi::StorageLocationQuery::Alignment.admits_result(result.scalar_type)
        })
        .ok_or("storage alignment must return u64")?;
    let ScalarType::Integer(integer) = result.scalar_type else {
        return Err("storage alignment must return u64");
    };
    let declared_minimum = || {
        let (producer, root) = machine.producing_operation(source.place)?;
        if !matches!(
            producer.kind,
            OperationKind::EstablishPrimitiveLocal { .. }
                | OperationKind::EstablishStructuralLocal {}
                | OperationKind::EstablishZeroedStructural {}
                | OperationKind::EstablishScalarArray { .. }
                | OperationKind::EstablishStructuralArray { .. }
                | OperationKind::EstablishRecord { .. }
                | OperationKind::EstablishScalarCase { .. }
                | OperationKind::EstablishStructuralCase { .. }
        ) || !crate::projection_stays_in_inline_storage(types.iter(), root, &source.path)
        {
            return None;
        }
        let mut geometry = crate::ReferenceStorageGeometry::new(types.iter(), |id| {
            Err(crate::StorageGeometryError::UnresolvedExtent(id))
        });
        // A cap is not a floor. Only a real contiguous region inside this
        // fresh, aligned allocation establishes the base-and-offset guarantee.
        let (_, base_alignment) = geometry.layout(root).ok()?;
        let guarantee = geometry
            .region(root, &source.path)?
            .alignment_at(base_alignment)?;
        Some(guarantee.min(geometry.location_alignment_limit(root, &source.path)?))
    };
    Ok(Some(Proposition::LessOrEqual(
        ScalarTerm::Integer {
            scalar_type: integer,
            value: semantic_vocabulary::IntegerValue::Unsigned(u128::from(
                declared_minimum().unwrap_or(1),
            )),
        },
        ScalarTerm::value(result.id, result.scalar_type),
    )))
}

#[derive(Clone, Copy)]
enum Target {
    Declaration(StructuralTypeId),
    Bytes(ByteSequenceCarrier),
    Scalar(ScalarType),
}

pub fn structural_read_scalar_type<'a>(
    types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[ScalarReadProjection],
) -> Option<ScalarType> {
    structural_read_type_with_extents(types, root, path, |_, _| ())
}

/// Report the declared extent at each indexed projection. A view's current
/// length is a storage observation; its capacity cannot establish a bound.
pub fn structural_read_type_with_extents<'a>(
    types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[ScalarReadProjection],
    mut indexed: impl FnMut(usize, Option<terminal_psi::ArrayExtent>),
) -> Option<ScalarType> {
    let declaration = |identity| {
        let mut matches = types.clone().filter(|candidate| candidate.id == identity);
        let found = matches.next()?;
        matches.next().is_none().then_some(found)
    };
    let mut target = Target::Declaration(root);
    let mut case_fields: Option<&[StructuralFieldDeclaration]> = None;
    for (position, projection) in path.iter().enumerate() {
        match projection {
            ScalarReadProjection::Static(Static::Referent) => return None,
            ScalarReadProjection::Extent => {
                if case_fields.is_some() {
                    return None;
                }
                let carrier = match target {
                    Target::Declaration(root) => match declaration(root)?.shape {
                        StructuralTypeShape::ElementView { access, .. } => {
                            if access == Some(StructuralAccess::WriteOnlyBorrow) {
                                return None;
                            }
                            None
                        }
                        StructuralTypeShape::ByteSequence(carrier) => Some(carrier),
                        _ => return None,
                    },
                    Target::Bytes(carrier) => Some(carrier),
                    Target::Scalar(_) => return None,
                };
                if matches!(
                    carrier,
                    Some(
                        ByteSequenceCarrier::BorrowedView {
                            access: Some(StructuralAccess::WriteOnlyBorrow)
                        } | ByteSequenceCarrier::BoundedView {
                            access: Some(StructuralAccess::WriteOnlyBorrow),
                            ..
                        }
                    )
                ) {
                    return None;
                }
                target = Target::Scalar(ScalarType::Integer(
                    IntegerType::new(IntegerSign::Unsigned, 64).ok()?,
                ));
            }
            ScalarReadProjection::Referent => {
                if case_fields.is_some() {
                    return None;
                }
                let Target::Declaration(root) = target else {
                    return None;
                };
                let StructuralTypeShape::Reference { referent, access } =
                    declaration(root)?.shape
                else {
                    return None;
                };
                if !matches!(access, terminal_psi::reading_borrow_pair_pattern!()) {
                    return None;
                }
                target = Target::Declaration(referent);
            }
            ScalarReadProjection::Static(Static::Case(identity)) => {
                if case_fields.is_some() {
                    return None;
                }
                let Target::Declaration(root) = target else {
                    return None;
                };
                let (StructuralTypeShape::Sum { cases }
                | StructuralTypeShape::Mixed { cases, .. }) = &declaration(root)?.shape
                else {
                    return None;
                };
                case_fields =
                    Some(&exactly_one(cases.iter(), |case| case.id == *identity)?.fields);
            }
            ScalarReadProjection::Static(Static::Field(identity)) => {
                let Target::Declaration(root) = target else {
                    return None;
                };
                let fields = match case_fields.take() {
                    Some(fields) => fields,
                    None => match &declaration(root)?.shape {
                        terminal_psi::record_or_mixed_shape_pattern!(fields) => fields,
                        _ => return None,
                    },
                };
                let field = exactly_one(fields.iter(), |field| field.id == *identity)?;
                if field.relevance.is_erased() {
                    let StructuralFieldType::Erased {
                        scalar: Some(scalar),
                        ..
                    } = &field.field_type
                    else {
                        return None;
                    };
                    if position + 1 != path.len() {
                        return None;
                    }
                    target = Target::Scalar(*scalar);
                    continue;
                }
                target = match &field.field_type {
                    StructuralFieldType::Structural(child) => Target::Declaration(*child),
                    StructuralFieldType::ByteSequence(carrier)
                    | StructuralFieldType::PredicateByteSequence { carrier, .. } => {
                        Target::Bytes(*carrier)
                    }
                    StructuralFieldType::Atomic(_)
                    | StructuralFieldType::AtomicBoundedInteger(_)
                    | StructuralFieldType::Erased { .. } => {
                        return None;
                    }
                    other => Target::Scalar(other.scalar_type()?),
                };
            }
            ScalarReadProjection::Static(Static::FixedIndex(_))
            | ScalarReadProjection::Index(_) => {
                if case_fields.is_some() {
                    return None;
                }
                let fixed = match projection {
                    ScalarReadProjection::Static(Static::FixedIndex(index)) => Some(*index),
                    ScalarReadProjection::Index(index) => {
                        index.validate().ok()?;
                        if index.scalar_type().as_integer()?.is_address() {
                            return None;
                        }
                        match index.integer_value() {
                            Some((_, semantic_vocabulary::IntegerValue::Signed(value))) => {
                                Some(u64::try_from(value).ok()?)
                            }
                            Some((_, semantic_vocabulary::IntegerValue::Unsigned(value))) => {
                                Some(u64::try_from(value).ok()?)
                            }
                            None => None,
                        }
                    }
                    _ => unreachable!(),
                };
                if let Target::Declaration(root) = target {
                    target = match &declaration(root)?.shape {
                        StructuralTypeShape::FixedArray { element, length } => {
                            indexed(position, Some(*length));
                            if let terminal_psi::ArrayExtent::Literal(length) = length
                                && (*length == 0 || fixed.is_some_and(|index| index >= *length))
                            {
                                return None;
                            }
                            Target::Declaration(*element)
                        }
                        StructuralTypeShape::ElementView {
                            access: Some(StructuralAccess::WriteOnlyBorrow),
                            ..
                        } => return None,
                        StructuralTypeShape::ElementView { element, .. } => {
                            indexed(position, None);
                            Target::Declaration(*element)
                        }
                        StructuralTypeShape::ByteSequence(carrier) => Target::Bytes(*carrier),
                        _ => return None,
                    };
                    if !matches!(target, Target::Bytes(_)) {
                        continue;
                    }
                }
                let Target::Bytes(carrier) = target else {
                    return None;
                };
                indexed(position, None);
                match carrier {
                    ByteSequenceCarrier::BorrowedView {
                        access: Some(StructuralAccess::WriteOnlyBorrow),
                    }
                    | ByteSequenceCarrier::BoundedView {
                        access: Some(StructuralAccess::WriteOnlyBorrow),
                        ..
                    } => return None,
                    ByteSequenceCarrier::BoundedView { capacity, .. }
                    | ByteSequenceCarrier::BoundedOwned { capacity }
                        if capacity == 0 || fixed.is_some_and(|index| index >= capacity) =>
                    {
                        return None;
                    }
                    _ => {}
                }
                target = Target::Scalar(ScalarType::Integer(
                    IntegerType::new(IntegerSign::Unsigned, 8).ok()?,
                ));
            }
        }
    }
    if case_fields.is_some() {
        return None;
    }
    match target {
        Target::Scalar(scalar) => Some(scalar),
        Target::Declaration(root) => match declaration(root)?.shape {
            StructuralTypeShape::PrimitiveScalar(scalar) => Some(scalar),
            _ => None,
        },
        Target::Bytes(_) => None,
    }
}

//! Declaration geometry of current scalar observations. Runtime selectors are
//! operands, not static coordinates: their bounds and the root's live access
//! are independent obligations. This walker never supplies observation truth.

use semantic_vocabulary::{
    CanonicalStructuralPathSegment as Static, IntegerSign, IntegerType, ScalarReadProjection,
    ScalarType, StructuralTypeId,
};
use terminal_psi::{
    ByteSequenceCarrier, StructuralAccess, StructuralFieldDeclaration, StructuralFieldType,
    StructuralTypeDeclaration, StructuralTypeShape,
};

#[derive(Clone, Copy)]
enum Target {
    Declaration(StructuralTypeId),
    Bytes(ByteSequenceCarrier),
    Scalar(ScalarType),
}

/// Exact scalar carrier selected by a declaration path. Rejects duplicate or
/// erased members, ill-formed case paths, statically impossible indexes, atomic
/// cells and reference crossings. A dynamic index still owes its live bound.
pub fn structural_read_scalar_type<'a>(
    types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[ScalarReadProjection],
) -> Option<ScalarType> {
    if path.is_empty() {
        return None;
    }
    let declaration = |identity| {
        let mut matches = types.clone().filter(|candidate| candidate.id == identity);
        let found = matches.next()?;
        matches.next().is_none().then_some(found)
    };
    let mut target = Target::Declaration(root);
    let mut case_fields: Option<&[StructuralFieldDeclaration]> = None;
    for projection in path {
        match projection {
            ScalarReadProjection::Static(Static::Case(identity)) => {
                if case_fields.is_some() {
                    return None;
                }
                let Target::Declaration(root) = target else {
                    return None;
                };
                let (StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. }) =
                    &declaration(root)?.shape
                else {
                    return None;
                };
                let mut matches = cases.iter().filter(|case| case.id == *identity);
                case_fields = Some(&matches.next()?.fields);
                if matches.next().is_some() {
                    return None;
                }
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
                let mut matches = fields.iter().filter(|field| field.id == *identity);
                let field = matches.next()?;
                if matches.next().is_some() || field.relevance.is_erased() {
                    return None;
                }
                target = match &field.field_type {
                    StructuralFieldType::Structural(child) => Target::Declaration(*child),
                    StructuralFieldType::ByteSequence(carrier)
                    | StructuralFieldType::PredicateByteSequence { carrier, .. } => {
                        Target::Bytes(*carrier)
                    }
                    StructuralFieldType::Atomic(_) | StructuralFieldType::Erased { .. } => {
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
                            if *length == 0 || fixed.is_some_and(|index| index >= *length) {
                                return None;
                            }
                            Target::Declaration(*element)
                        }
                        StructuralTypeShape::ElementView { element, .. } => {
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

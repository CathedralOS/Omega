//! Join an owned record's public ABI graph to its complete semantic field tree.
//!
//! Child extents/alignment come from the already-admitted calling application,
//! not a runtime layout calculation. The root storage codec independently owns
//! the total allocation and every scalar's full typed path, offset and stored
//! carrier. Matching only flattened bytes would lose empty children and permit
//! a different field tree. Neither this join nor its geometry grants custody.

use super::{StorageCodec, rejection, validate_scalar_shape};
use calling_conventions::{BoundarySignatureGeometry, BoundaryValueClass, BoundaryValueShape};
use semantic_vocabulary::{CanonicalStructuralPathSegment as Path, ScalarType, StructuralTypeId};
use terminal_interpreter::TerminalEffectRejection;
use terminal_psi::{
    StructuralFieldDeclaration, StructuralFieldType, StructuralRecordStorage,
    StructuralTypeDeclaration, StructuralTypeShape,
};

enum Visit<'declaration> {
    Record {
        semantic: &'declaration StructuralTypeDeclaration,
        shape: u16,
        offset: usize,
    },
    Field {
        semantic: &'declaration StructuralFieldDeclaration,
        shape: u16,
        offset: usize,
    },
    LeaveRecord,
    LeaveField,
}

pub(super) fn validate<'declaration>(
    signature: &BoundarySignatureGeometry,
    root: u16,
    codec: &StorageCodec,
    declaration: impl Fn(StructuralTypeId) -> Option<&'declaration StructuralTypeDeclaration>,
) -> Result<(), TerminalEffectRejection> {
    let semantic = declaration(codec.structural_type)
        .filter(|semantic| semantic.id == codec.structural_type)
        .ok_or_else(|| rejection("native value ABI lost its semantic record declaration"))?;
    let root_shape = shape(signature, root)?;
    if usize::from(root_shape.byte_size) != codec.extent
        || usize::from(root_shape.alignment) != codec.alignment
    {
        return Err(rejection(
            "native record public shape differs from its exact storage codec",
        ));
    }
    let mut pending = vec![Visit::Record {
        semantic,
        shape: root,
        offset: 0,
    }];
    let mut active_types = Vec::new();
    let mut active_shapes = Vec::new();
    let mut path = Vec::new();
    let mut leaf = 0usize;
    while let Some(visit) = pending.pop() {
        match visit {
            Visit::Record {
                semantic,
                shape: identity,
                offset,
            } => {
                if active_types.contains(&semantic.id) || active_shapes.contains(&identity) {
                    return Err(rejection("native value ABI has cyclic record storage"));
                }
                if semantic.quotient_carrier.is_some() || semantic.scalar_restriction.is_some() {
                    return Err(rejection(
                        "native value ABI needs its exact restricted carrier codec",
                    ));
                }
                let StructuralTypeShape::Record {
                    fields: semantic_fields,
                    storage,
                } = &semantic.shape
                else {
                    return Err(rejection("native value ABI needs a semantic record"));
                };
                if matches!(
                    storage,
                    StructuralRecordStorage::PlacedView(_)
                        | StructuralRecordStorage::PlacedAccessor(_)
                        | StructuralRecordStorage::TargetLayout(_)
                ) {
                    return Err(rejection(
                        "native value ABI needs resolved ordinary record storage",
                    ));
                }
                let record = shape(signature, identity)?;
                let BoundaryValueClass::Record {
                    first_field,
                    field_count,
                } = record.class
                else {
                    return Err(rejection(
                        "native value ABI changes a record's public class",
                    ));
                };
                if offset
                    .checked_add(usize::from(record.byte_size))
                    .is_none_or(|end| end > codec.extent)
                {
                    return Err(rejection(
                        "native public record extent/padding exceeds its containing storage",
                    ));
                }
                let first = usize::from(first_field);
                let end = first
                    .checked_add(usize::from(field_count))
                    .ok_or_else(|| rejection("native public record field span overflows"))?;
                let public_fields = signature
                    .fields
                    .get(first..end)
                    .ok_or_else(|| rejection("native public record field span is absent"))?;
                let relevant = semantic_fields
                    .iter()
                    .filter(|field| !field.relevance.is_erased())
                    .collect::<Vec<_>>();
                if (!semantic_fields.is_empty() && relevant.is_empty())
                    || relevant.len() != public_fields.len()
                {
                    return Err(rejection(
                        "native value ABI changes its complete relevant field roster",
                    ));
                }
                let mut intervals = Vec::with_capacity(public_fields.len());
                for public in public_fields {
                    let child = shape(signature, public.shape)?;
                    let start = usize::from(public.byte_offset);
                    let end = start
                        .checked_add(usize::from(child.byte_size))
                        .ok_or_else(|| rejection("native public child extent overflows"))?;
                    // The admitted calling application owns packing/alignment
                    // policy. Root codec coordinates authenticate actual scalar
                    // placement; natural-alignment guesses would reject an
                    // independently packed field in an aligned parent.
                    if end > usize::from(record.byte_size) {
                        return Err(rejection("native public child exceeds its record extent"));
                    }
                    if start < end {
                        if intervals.iter().any(|(prior_start, prior_end)| {
                            start < *prior_end && *prior_start < end
                        }) {
                            return Err(rejection("native public record children overlap"));
                        }
                        intervals.push((start, end));
                    }
                }
                active_types.push(semantic.id);
                active_shapes.push(identity);
                pending.push(Visit::LeaveRecord);
                for (semantic, public) in relevant.into_iter().zip(public_fields).rev() {
                    let offset = offset
                        .checked_add(usize::from(public.byte_offset))
                        .ok_or_else(|| rejection("native public child offset overflows"))?;
                    pending.push(Visit::Field {
                        semantic,
                        shape: public.shape,
                        offset,
                    });
                }
            }
            Visit::Field {
                semantic,
                shape: identity,
                offset,
            } => {
                path.push(Path::Field(semantic.id));
                pending.push(Visit::LeaveField);
                let logical = match semantic.field_type {
                    StructuralFieldType::Scalar(scalar) => Some(scalar),
                    StructuralFieldType::IeeeFloat(format) => Some(ScalarType::IeeeFloat(format)),
                    StructuralFieldType::Structural(child) => {
                        let child = declaration(child)
                            .filter(|semantic| semantic.id == child)
                            .ok_or_else(|| {
                                rejection("native value ABI lost a semantic child declaration")
                            })?;
                        pending.push(Visit::Record {
                            semantic: child,
                            shape: identity,
                            offset,
                        });
                        None
                    }
                    _ => {
                        return Err(rejection(
                            "native value ABI needs plain scalar or finite record fields",
                        ));
                    }
                };
                if let Some(logical) = logical {
                    let field = codec
                        .fields
                        .get(leaf)
                        .ok_or_else(|| rejection("native value ABI lost a scalar codec field"))?;
                    if field.path != path || field.offset != offset || field.logical != logical {
                        return Err(rejection(
                            "native value ABI changes a scalar's full path, offset or carrier",
                        ));
                    }
                    validate_scalar_shape(*shape(signature, identity)?, field.stored)?;
                    leaf += 1;
                }
            }
            Visit::LeaveRecord => {
                active_types.pop();
                active_shapes.pop();
            }
            Visit::LeaveField => {
                path.pop();
            }
        }
    }
    if leaf != codec.fields.len() {
        return Err(rejection("native value ABI has extra scalar codec fields"));
    }
    Ok(())
}

fn shape(
    signature: &BoundarySignatureGeometry,
    identity: u16,
) -> Result<&BoundaryValueShape, TerminalEffectRejection> {
    let shape = signature
        .shapes
        .get(usize::from(identity))
        .ok_or_else(|| rejection("native public record field shape is absent"))?;
    if !shape.alignment.is_power_of_two() {
        return Err(rejection(
            "native public record shape has invalid alignment",
        ));
    }
    Ok(shape)
}

#[cfg(test)]
mod tests;

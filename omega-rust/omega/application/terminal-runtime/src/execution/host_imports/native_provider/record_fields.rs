//! The complete semantic scalar roster of a finite record/array storage. Paths
//! retain every containing field identity; layout recipes supply placement,
//! never the missing type or authority. References, atomic storage and other
//! carrier families need their own codecs rather than scalar-width guesses.
//! A reference is a typed leaf: its pointee is a separate original allocation.

use super::original_homes::rejection;
use semantic_vocabulary::{CanonicalStructuralPathSegment as Path, ScalarType, StructuralTypeId};
use terminal_interpreter::{TerminalEffectRejection, TerminalProgram};
use terminal_psi::{
    StructuralFieldDeclaration, StructuralFieldType, StructuralRecordStorage, StructuralTypeShape,
};

pub(super) struct SemanticScalarField {
    pub path: Vec<Path>,
    pub logical: ScalarType,
}

enum Visit<'declaration> {
    Type(StructuralTypeId),
    Field(&'declaration StructuralFieldDeclaration),
    Element(StructuralTypeId, u64),
    LeaveType,
    LeaveField,
}

pub(super) struct SemanticReferenceField {
    pub path: Vec<Path>,
    pub referent: StructuralTypeId,
    pub access: terminal_psi::StructuralAccess,
}
pub(super) struct SemanticFields {
    pub scalars: Vec<SemanticScalarField>,
    pub references: Vec<SemanticReferenceField>,
}
pub(super) fn fields(
    program: &TerminalProgram,
    structural_type: StructuralTypeId,
) -> Result<SemanticFields, TerminalEffectRejection> {
    let mut pending = vec![Visit::Type(structural_type)];
    let mut active = Vec::new();
    let mut path = Vec::new();
    let mut output = Vec::new();
    let mut references = Vec::new();
    while let Some(visit) = pending.pop() {
        match visit {
            Visit::Type(identity) => {
                if active.contains(&identity) {
                    return Err(rejection(
                        "native embedded record has cyclic semantic storage",
                    ));
                }
                let declaration = program.structural_type(identity).ok_or_else(|| {
                    rejection("native embedded record has no semantic declaration")
                })?;
                if declaration.quotient_carrier.is_some() {
                    return Err(rejection("native import needs its exact quotient codec"));
                }
                match &declaration.shape {
                    StructuralTypeShape::PrimitiveScalar(logical) => {
                        output.push(SemanticScalarField {
                            path: path.clone(),
                            logical: *logical,
                        });
                    }
                    StructuralTypeShape::Reference { referent, access } => {
                        references.push(SemanticReferenceField {
                            path: path.clone(),
                            referent: *referent,
                            access: *access,
                        });
                    }
                    StructuralTypeShape::Record { fields, storage } => {
                        if matches!(
                            storage,
                            StructuralRecordStorage::PlacedView(_)
                                | StructuralRecordStorage::PlacedAccessor(_)
                                | StructuralRecordStorage::TargetLayout(_)
                        ) {
                            return Err(rejection(
                                "native record codec is not resolved ordinary storage",
                            ));
                        }
                        active.push(identity);
                        pending.push(Visit::LeaveType);
                        pending.extend(
                            fields
                                .iter()
                                .rev()
                                .filter(|field| !field.relevance.is_erased())
                                .map(Visit::Field),
                        );
                    }
                    StructuralTypeShape::FixedArray { element, length } => {
                        active.push(identity);
                        pending.push(Visit::LeaveType);
                        pending.extend(
                            (0..*length)
                                .rev()
                                .map(|index| Visit::Element(*element, index)),
                        );
                    }
                    _ => {
                        return Err(rejection(
                            "native embedded field needs a supported materialized record codec",
                        ));
                    }
                }
            }
            Visit::Field(field) => {
                path.push(Path::Field(field.id));
                pending.push(Visit::LeaveField);
                let logical = match field.field_type {
                    StructuralFieldType::Scalar(scalar) => scalar,
                    StructuralFieldType::BoundedInteger(bounds) => {
                        ScalarType::Integer(bounds.integer_type())
                    }
                    StructuralFieldType::RefinedScalar(ref refinement) => refinement.carrier(),
                    StructuralFieldType::IeeeFloat(format) => ScalarType::IeeeFloat(format),
                    StructuralFieldType::Structural(child) => {
                        pending.push(Visit::Type(child));
                        continue;
                    }
                    _ => {
                        return Err(rejection(
                            "native record field needs its exact non-scalar storage codec",
                        ));
                    }
                };
                output.push(SemanticScalarField {
                    path: path.clone(),
                    logical,
                });
            }
            Visit::Element(element, index) => {
                path.push(Path::FixedIndex(index));
                pending.push(Visit::LeaveField);
                pending.push(Visit::Type(element));
            }
            Visit::LeaveType => {
                active.pop().ok_or_else(|| {
                    rejection("native record lost its containing semantic declaration")
                })?;
            }
            Visit::LeaveField => {
                path.pop()
                    .ok_or_else(|| rejection("native record lost its containing semantic field"))?;
            }
        }
    }
    Ok(SemanticFields {
        scalars: output,
        references,
    })
}

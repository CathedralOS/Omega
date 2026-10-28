//! The complete semantic scalar roster of finite record/array storage. Schema
//! validation is declaration-sized, including children beneath zero dimensions;
//! instance enumeration visits only branches with actual scalar leaves. Paths
//! retain every containing field identity. Layout recipes supply placement,
//! never missing types or authority. References, atomics and other carriers
//! retain their independent codec requirements. A reference is one typed leaf;
//! its pointee belongs to a separate authenticated original allocation.

mod schema;

use super::original_homes::rejection;
use schema::{Element, Schema, Shape};
use semantic_vocabulary::{CanonicalStructuralPathSegment as Path, ScalarType, StructuralTypeId};
use terminal_interpreter::{TerminalEffectRejection, TerminalProgram};

pub(super) struct SemanticScalarField {
    pub path: Vec<Path>,
    pub logical: ScalarType,
}

enum Visit {
    Node(usize),
    Field(Path, usize),
    Scalar(Path, ScalarType),
    Element {
        node: usize,
        index: u64,
        length: u64,
    },
    Pop,
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
    let schema = Schema::derive(program, structural_type)?;
    let mut output = Vec::new();
    let mut references = Vec::new();
    output
        .try_reserve_exact(schema.leaves)
        .map_err(|_| rejection("native semantic scalar roster exceeds available output storage"))?;
    let mut pending = vec![Visit::Node(schema.root)];
    let mut path = Vec::new();
    while let Some(visit) = pending.pop() {
        match visit {
            Visit::Node(index) => {
                let node = &schema.nodes[index];
                if node.is_empty() {
                    continue;
                }
                match node.shape()? {
                    Shape::Scalar(logical) => output.push(SemanticScalarField {
                        path: path.clone(),
                        logical: *logical,
                    }),
                    Shape::Reference { referent, access } => {
                        references.push(SemanticReferenceField {
                            path: path.clone(),
                            referent: *referent,
                            access: *access,
                        })
                    }
                    Shape::Record(fields) => {
                        for (identity, element) in fields.iter().rev() {
                            let segment = Path::Field(*identity);
                            match element {
                                Element::Scalar(logical) => {
                                    pending.push(Visit::Scalar(segment, *logical));
                                }
                                Element::Structural(child) => {
                                    if !schema.nodes[*child].is_empty() {
                                        pending.push(Visit::Field(segment, *child));
                                    }
                                }
                            }
                        }
                    }
                    Shape::Array { element, length } => {
                        pending.push(Visit::Element {
                            node: *element,
                            index: 0,
                            length: *length,
                        });
                    }
                }
            }
            Visit::Field(segment, child) => {
                path.push(segment);
                pending.push(Visit::Pop);
                pending.push(Visit::Node(child));
            }
            Visit::Scalar(segment, logical) => {
                path.push(segment);
                output.push(SemanticScalarField {
                    path: path.clone(),
                    logical,
                });
                path.pop();
            }
            Visit::Element {
                node,
                index,
                length,
            } => {
                if index < length {
                    pending.push(Visit::Element {
                        node,
                        index: index + 1,
                        length,
                    });
                    path.push(Path::FixedIndex(index));
                    pending.push(Visit::Pop);
                    pending.push(Visit::Node(node));
                }
            }
            Visit::Pop => {
                path.pop().ok_or_else(|| {
                    rejection("native scalar roster lost its containing semantic path")
                })?;
            }
        }
    }
    if output.len().checked_add(references.len()) != Some(schema.leaves) {
        return Err(rejection(
            "native scalar roster differs from its complete schema",
        ));
    }
    Ok(SemanticFields {
        scalars: output,
        references,
    })
}

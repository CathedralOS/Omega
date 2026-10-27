//! Declaration-sized planning for a native codec's scalar/reference roster. Array counts
//! multiply leaf counts, not schema nodes. Every element schema is validated,
//! including beneath zero dimensions; only a completed zero count suppresses
//! later instance enumeration. This plan supplies neither layout nor custody.

use super::rejection;
use semantic_vocabulary::{ScalarType, StructuralFieldId, StructuralTypeId};
use terminal_interpreter::{TerminalEffectRejection, TerminalProgram};
use terminal_psi::{
    StructuralFieldType, StructuralRecordStorage, StructuralTypeDeclaration, StructuralTypeShape,
};

pub(super) enum Element {
    Scalar(ScalarType),
    Structural(usize),
}

pub(super) enum Shape {
    Scalar(ScalarType),
    Reference {
        referent: StructuralTypeId,
        access: terminal_psi::StructuralAccess,
    },
    Record(Vec<(StructuralFieldId, Element)>),
    Array {
        element: usize,
        length: u64,
    },
}

pub(super) struct Node {
    identity: StructuralTypeId,
    // None until this declaration's children have completed. Encountering it
    // again during planning is an ancestry cycle, not sibling sharing.
    shape: Option<Shape>,
    // Inner overflow remains deferred: an outer zero still has zero leaves.
    leaves: Option<u64>,
}

impl Node {
    pub(super) fn shape(&self) -> Result<&Shape, TerminalEffectRejection> {
        self.shape
            .as_ref()
            .ok_or_else(|| rejection("native scalar schema has an incomplete declaration"))
    }

    pub(super) fn is_empty(&self) -> bool {
        self.leaves == Some(0)
    }
}

pub(super) struct Schema {
    pub(super) nodes: Vec<Node>,
    pub(super) root: usize,
    pub(super) leaves: usize,
}

enum Visit<'declaration> {
    Type(StructuralTypeId),
    Complete(usize, &'declaration StructuralTypeDeclaration),
}

impl Schema {
    pub(super) fn derive(
        program: &TerminalProgram,
        structural_type: StructuralTypeId,
    ) -> Result<Self, TerminalEffectRejection> {
        let mut nodes: Vec<Node> = Vec::new();
        let mut pending = vec![Visit::Type(structural_type)];
        while let Some(visit) = pending.pop() {
            match visit {
                Visit::Type(identity) => {
                    if let Some(node) = nodes.iter().find(|node| node.identity == identity) {
                        if node.shape.is_none() {
                            return Err(rejection(
                                "native embedded record has cyclic semantic storage",
                            ));
                        }
                        continue;
                    }
                    let declaration = program
                        .structural_type(identity)
                        .filter(|declaration| declaration.id == identity)
                        .ok_or_else(|| {
                            rejection("native embedded record has no semantic declaration")
                        })?;
                    if declaration.quotient_carrier.is_some() {
                        return Err(rejection("native import needs its exact quotient codec"));
                    }
                    let index = nodes.len();
                    nodes.push(Node {
                        identity,
                        shape: None,
                        leaves: None,
                    });
                    pending.push(Visit::Complete(index, declaration));
                    match &declaration.shape {
                        StructuralTypeShape::PrimitiveScalar(_)
                        | StructuralTypeShape::Reference { .. } => {}
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
                            for field in fields
                                .iter()
                                .rev()
                                .filter(|field| !field.relevance.is_erased())
                            {
                                if let StructuralFieldType::Structural(child) = field.field_type {
                                    pending.push(Visit::Type(child));
                                } else {
                                    scalar_type(&field.field_type)?;
                                }
                            }
                        }
                        StructuralTypeShape::FixedArray { element, .. } => {
                            pending.push(Visit::Type(*element));
                        }
                        _ => {
                            return Err(rejection(
                                "native embedded field needs a supported materialized record codec",
                            ));
                        }
                    }
                }
                Visit::Complete(index, declaration) => {
                    let child = |identity| {
                        nodes
                            .iter()
                            .position(|node| node.identity == identity && node.shape.is_some())
                            .ok_or_else(|| {
                                rejection("native scalar schema lost a completed child declaration")
                            })
                    };
                    let (shape, leaves) = match &declaration.shape {
                        StructuralTypeShape::PrimitiveScalar(logical) => {
                            (Shape::Scalar(*logical), Some(1))
                        }
                        StructuralTypeShape::Reference { referent, access } => (
                            Shape::Reference {
                                referent: *referent,
                                access: *access,
                            },
                            Some(1),
                        ),
                        StructuralTypeShape::Record { fields, .. } => {
                            let mut members = Vec::new();
                            let mut leaves = Some(0_u64);
                            for field in fields.iter().filter(|field| !field.relevance.is_erased())
                            {
                                let (element, count) = match field.field_type {
                                    StructuralFieldType::Structural(identity) => {
                                        let child = child(identity)?;
                                        (Element::Structural(child), nodes[child].leaves)
                                    }
                                    _ => {
                                        (Element::Scalar(scalar_type(&field.field_type)?), Some(1))
                                    }
                                };
                                leaves = leaves
                                    .zip(count)
                                    .and_then(|(sum, count)| sum.checked_add(count));
                                members.push((field.id, element));
                            }
                            (Shape::Record(members), leaves)
                        }
                        StructuralTypeShape::FixedArray { element, length } => {
                            let element = child(*element)?;
                            let leaves = if *length == 0 {
                                Some(0)
                            } else {
                                nodes[element]
                                    .leaves
                                    .and_then(|count| count.checked_mul(*length))
                            };
                            (
                                Shape::Array {
                                    element,
                                    length: *length,
                                },
                                leaves,
                            )
                        }
                        _ => {
                            return Err(rejection(
                                "native scalar schema contains an unsupported declaration",
                            ));
                        }
                    };
                    nodes[index].shape = Some(shape);
                    nodes[index].leaves = leaves;
                }
            }
        }
        // Discovery installs the root first; every later node is its dependency.
        let leaves = nodes
            .first()
            .and_then(|root| root.leaves)
            .and_then(|leaves| usize::try_from(leaves).ok())
            .ok_or_else(|| rejection("native semantic scalar roster exceeds host indexing"))?;
        Ok(Self {
            nodes,
            root: 0,
            leaves,
        })
    }
}

fn scalar_type(field: &StructuralFieldType) -> Result<ScalarType, TerminalEffectRejection> {
    match field {
        StructuralFieldType::Scalar(scalar) => Ok(*scalar),
        StructuralFieldType::BoundedInteger(bounds) => {
            Ok(ScalarType::Integer(bounds.integer_type()))
        }
        StructuralFieldType::RefinedScalar(refinement) => Ok(refinement.carrier()),
        StructuralFieldType::IeeeFloat(format) => Ok(ScalarType::IeeeFloat(*format)),
        _ => Err(rejection(
            "native record field needs its exact non-scalar storage codec",
        )),
    }
}

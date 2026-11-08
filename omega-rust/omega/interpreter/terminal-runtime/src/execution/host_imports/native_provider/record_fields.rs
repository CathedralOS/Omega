// We derive the complete semantic scalar and reference leaf roster of finite
// record and array storage. Schema validation is declaration-sized (including
// children beneath zero dimensions), while instance enumeration visits only
// branches with actual leaves and preserves every containing field identity.

use super::original_homes::rejection;
use semantic_vocabulary::{
    CanonicalStructuralPathSegment as Path, ScalarType, StructuralFieldId, StructuralTypeId,
};
use terminal_interpreter::{TerminalEffectRejection, TerminalProgram};
use terminal_psi::{
    StructuralFieldType, StructuralRecordStorage, StructuralTypeDeclaration, StructuralTypeShape,
};

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

enum Element {
    Scalar(ScalarType),
    Structural(usize),
}

enum Shape {
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

struct Node {
    identity: StructuralTypeId,
    shape: Option<Shape>, // None until children have completed; re-encountering None detects ancestry cycles.
    leaves: Option<u64>,  // Deferred inner overflow so an outer zero array still has zero leaves.
}

impl Node {
    fn shape(&self) -> Result<&Shape, TerminalEffectRejection> {
        self.shape
            .as_ref()
            .ok_or_else(|| rejection("native scalar schema has an incomplete declaration"))
    }

    fn is_empty(&self) -> bool {
        self.leaves == Some(0)
    }
}

struct Schema {
    nodes: Vec<Node>,
    root: usize,
    leaves: usize,
}

enum SchemaVisit<'declaration> {
    Type(StructuralTypeId),
    Complete(usize, &'declaration StructuralTypeDeclaration),
}

impl Schema {
    fn derive(
        program: &TerminalProgram,
        structural_type: StructuralTypeId,
    ) -> Result<Self, TerminalEffectRejection> {
        let mut nodes: Vec<Node> = Vec::new();
        let mut pending = vec![SchemaVisit::Type(structural_type)];
        while let Some(visit) = pending.pop() {
            match visit {
                SchemaVisit::Type(identity) => {
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
                    pending.push(SchemaVisit::Complete(index, declaration));
                    match &declaration.shape {
                        StructuralTypeShape::PrimitiveScalar(_)
                        | StructuralTypeShape::Reference { .. } => {}
                        StructuralTypeShape::Record { fields, storage } => {
                            if matches!(
                                storage,
                                StructuralRecordStorage::AccessView(_)
                                    | StructuralRecordStorage::Accessor(_)
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
                                    pending.push(SchemaVisit::Type(child));
                                } else {
                                    scalar_type(&field.field_type)?;
                                }
                            }
                        }
                        StructuralTypeShape::FixedArray { element, .. } => {
                            pending.push(SchemaVisit::Type(*element));
                        }
                        _ => {
                            return Err(rejection(
                                "native embedded field needs a supported materialized record codec",
                            ));
                        }
                    }
                }
                SchemaVisit::Complete(index, declaration) => {
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
                        StructuralTypeShape::FixedArray {
                            element,
                            length: terminal_psi::ArrayExtent::Literal(length),
                        } => {
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

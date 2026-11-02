//! Join an owned aggregate's public ABI graph to its complete semantic tree.
//!
//! Child extents/alignment come from the already-admitted calling application,
//! not a runtime layout calculation. The root storage codec independently owns
//! the total allocation and every scalar's full typed path, offset and stored
//! carrier. Matching only flattened bytes would lose empty children and permit
//! a different field tree. Neither this join nor its geometry grants custody.

use super::{StorageCodec, rejection, validate_scalar_shape};
use calling_conventions::{BoundarySignatureGeometry, BoundaryValueClass, BoundaryValueShape};
use runtime_abi::NativeBorrowedStorageLayout;
use semantic_vocabulary::{
    CanonicalStructuralPathSegment as Path, IntegerSign, IntegerType, ScalarType, StructuralTypeId,
};
use terminal_interpreter::{TerminalEffectRejection, TerminalProgram};
use terminal_psi::{
    StructuralFieldDeclaration, StructuralFieldType, StructuralRecordStorage,
    StructuralTypeDeclaration, StructuralTypeShape,
};

struct Node {
    leaves: usize,
    kind: NodeKind,
}

enum NodeKind {
    Scalar {
        logical: ScalarType,
        shape: u16,
    },
    Record(Vec<(Path, usize, usize)>),
    Array {
        element: usize,
        length: u64,
        stride: usize,
    },
}

enum SchemaVisit<'declaration> {
    Type(&'declaration StructuralTypeDeclaration, u16),
    Field(&'declaration StructuralFieldDeclaration, u16),
    Record {
        first_child: usize,
        children: Vec<(Path, usize)>,
    },
    Array {
        length: u64,
        stride: usize,
    },
}

/// The schema walk visits an array element once, even below a zero dimension.
/// Its leaf counts bound the later instance walk by the actual codec payload.
struct Schema<'input> {
    signature: &'input BoundarySignatureGeometry,
    nodes: Vec<Node>,
    results: Vec<usize>,
    active_types: Vec<StructuralTypeId>,
    active_shapes: Vec<u16>,
    completed: Vec<(StructuralTypeId, u16, usize)>,
}

impl Schema<'_> {
    fn scalar(
        &mut self,
        logical: ScalarType,
        identity: u16,
    ) -> Result<(), TerminalEffectRejection> {
        let public = shape(self.signature, identity)?;
        let expected = if matches!(logical, ScalarType::IeeeFloat(_)) {
            BoundaryValueClass::Float
        } else {
            BoundaryValueClass::Integer
        };
        if public.class != expected {
            return Err(rejection(
                "native value ABI changes a scalar's public class",
            ));
        }
        let index = self.nodes.len();
        self.nodes.push(Node {
            leaves: 1,
            kind: NodeKind::Scalar {
                logical,
                shape: identity,
            },
        });
        self.results.push(index);
        Ok(())
    }

    fn finish(&mut self, node: Node) {
        let index = self.nodes.len();
        self.nodes.push(node);
        self.results.push(index);
        if let (Some(semantic), Some(public)) = (self.active_types.pop(), self.active_shapes.pop())
        {
            self.completed.push((semantic, public, index));
        }
    }
}

/// Match a complete OS record-array contract against semantic shape and the
/// exact selected native recipes. No field or policy spelling supplies geometry.
pub(in crate::execution::host_imports) fn matches_signed_record_array(
    program: &TerminalProgram,
    layouts: &[NativeBorrowedStorageLayout],
    root: StructuralTypeId,
    records: u64,
    words: usize,
    bits: u16,
    alignment: u16,
) -> bool {
    let Ok(integer) = IntegerType::new(IntegerSign::Signed, bits) else {
        return false;
    };
    if bits % 8 != 0 || !alignment.is_power_of_two() {
        return false;
    }
    let expected = ScalarType::Integer(integer);
    let Some(array) = program.structural_type(root) else {
        return false;
    };
    let StructuralTypeShape::FixedArray { element, length } = &array.shape else {
        return false;
    };
    if length.literal() != Some(records) {
        return false;
    }
    let Some(record) = program.structural_type(*element) else {
        return false;
    };
    let StructuralTypeShape::Record { fields, .. } = &record.shape else {
        return false;
    };
    let relevant = fields.iter().filter(|field| !field.relevance.is_erased());
    if relevant.clone().count() != words
        || !relevant.into_iter().all(|field| match &field.field_type {
            StructuralFieldType::Scalar(scalar) => *scalar == expected,
            StructuralFieldType::BoundedInteger(bounds) => {
                ScalarType::Integer(bounds.integer_type()) == expected
            }
            StructuralFieldType::RefinedScalar(refinement) => refinement.carrier() == expected,
            StructuralFieldType::Structural(child) => program.structural_type(*child).is_some_and(
                |child| matches!(&child.shape, StructuralTypeShape::PrimitiveScalar(scalar) if *scalar == expected),
            ),
            _ => false,
        })
    {
        return false;
    }
    let Some(count) = usize::try_from(records)
        .ok()
        .and_then(|count| count.checked_mul(words))
    else {
        return false;
    };
    let word_bytes = usize::from(bits / 8);
    let Some(extent) = count.checked_mul(word_bytes) else {
        return false;
    };
    let Ok(codec) = StorageCodec::admit(program, layouts, root) else {
        return false;
    };
    codec.extent == extent
        && codec.alignment >= usize::from(alignment)
        && codec.references.is_empty()
        && codec.fields.len() == count
        && codec.fields.iter().enumerate().all(|(index, field)| {
            field.logical == expected
                && field.stored == expected
                && field.offset == index * word_bytes
        })
}

pub(super) fn validate<'declaration>(
    signature: &BoundarySignatureGeometry,
    root: u16,
    codec: &StorageCodec,
    declaration: impl Fn(StructuralTypeId) -> Option<&'declaration StructuralTypeDeclaration>,
) -> Result<(), TerminalEffectRejection> {
    let semantic = declaration(codec.structural_type)
        .filter(|semantic| semantic.id == codec.structural_type)
        .ok_or_else(|| rejection("native value ABI lost its semantic declaration"))?;
    let root_shape = shape(signature, root)?;
    if usize::from(root_shape.byte_size) != codec.extent
        || usize::from(root_shape.alignment) != codec.alignment
    {
        return Err(rejection(
            "native public shape differs from its exact storage codec",
        ));
    }
    let mut schema = Schema {
        signature,
        nodes: Vec::new(),
        results: Vec::new(),
        active_types: Vec::new(),
        active_shapes: Vec::new(),
        completed: Vec::new(),
    };
    let mut pending = vec![SchemaVisit::Type(semantic, root)];
    while let Some(visit) = pending.pop() {
        match visit {
            SchemaVisit::Type(semantic, identity) => {
                if schema.active_types.contains(&semantic.id)
                    || schema.active_shapes.contains(&identity)
                {
                    return Err(rejection("native value ABI has cyclic aggregate storage"));
                }
                // A completed pair can recur in siblings. Check active ancestry
                // first so sharing never turns a backedge into a cached proof.
                if let Some((_, _, node)) = schema
                    .completed
                    .iter()
                    .find(|(ty, public, _)| *ty == semantic.id && *public == identity)
                {
                    schema.results.push(*node);
                    continue;
                }
                if semantic.quotient_carrier.is_some()
                    || (semantic.scalar_restriction.is_some()
                        && !matches!(semantic.shape, StructuralTypeShape::PrimitiveScalar(_)))
                {
                    return Err(rejection(
                        "native value ABI needs its exact restricted carrier codec",
                    ));
                }
                let public = shape(signature, identity)?;
                match &semantic.shape {
                    StructuralTypeShape::PrimitiveScalar(logical) => {
                        // Restrictions govern actual values, not this geometry.
                        // A primitive declaration has no authored stored-field override.
                        validate_scalar_shape(*public, *logical)?;
                        schema.scalar(*logical, identity)?;
                        schema
                            .completed
                            .push((semantic.id, identity, schema.nodes.len() - 1));
                    }
                    StructuralTypeShape::Record { fields, storage } => {
                        // Selected placement is already authenticated by the
                        // storage codec and public ABI graph. Keep comparing
                        // the complete semantic tree without inventing geometry.
                        if matches!(
                            storage,
                            StructuralRecordStorage::AccessView(_)
                                | StructuralRecordStorage::Accessor(_)
                        ) {
                            return Err(rejection(
                                "native value ABI needs resolved ordinary record storage",
                            ));
                        }
                        let BoundaryValueClass::Record {
                            first_field,
                            field_count,
                        } = public.class
                        else {
                            return Err(rejection(
                                "native value ABI changes a record's public class",
                            ));
                        };
                        let first = usize::from(first_field);
                        let end = first.checked_add(usize::from(field_count)).ok_or_else(|| {
                            rejection("native public record field span overflows")
                        })?;
                        let public_fields = signature.fields.get(first..end).ok_or_else(|| {
                            rejection("native public record field span is absent")
                        })?;
                        let relevant = fields
                            .iter()
                            .filter(|field| !field.relevance.is_erased())
                            .collect::<Vec<_>>();
                        if (!fields.is_empty() && relevant.is_empty())
                            || relevant.len() != public_fields.len()
                        {
                            return Err(rejection(
                                "native value ABI changes its complete relevant field roster",
                            ));
                        }
                        let mut intervals = Vec::with_capacity(public_fields.len());
                        let mut children = Vec::with_capacity(public_fields.len());
                        for (field, public_field) in relevant.iter().zip(public_fields) {
                            let child = shape(signature, public_field.shape)?;
                            let start = usize::from(public_field.byte_offset);
                            let end = start
                                .checked_add(usize::from(child.byte_size))
                                .ok_or_else(|| rejection("native public child extent overflows"))?;
                            // The admitted application owns packing. Do not impose
                            // natural-alignment guesses on independently packed fields.
                            if end > usize::from(public.byte_size) {
                                return Err(rejection(
                                    "native public child exceeds its record extent",
                                ));
                            }
                            if start < end {
                                if intervals.iter().any(|(prior_start, prior_end)| {
                                    start < *prior_end && *prior_start < end
                                }) {
                                    return Err(rejection("native public record children overlap"));
                                }
                                intervals.push((start, end));
                            }
                            children.push((Path::Field(field.id), start));
                        }
                        schema.active_types.push(semantic.id);
                        schema.active_shapes.push(identity);
                        pending.push(SchemaVisit::Record {
                            first_child: schema.results.len(),
                            children,
                        });
                        for (field, public) in relevant.into_iter().zip(public_fields).rev() {
                            pending.push(SchemaVisit::Field(field, public.shape));
                        }
                    }
                    StructuralTypeShape::FixedArray {
                        element,
                        length: terminal_psi::ArrayExtent::Literal(length),
                    } => {
                        let BoundaryValueClass::FixedArray {
                            element: public_element,
                            length: public_length,
                        } = public.class
                        else {
                            return Err(rejection(
                                "native value ABI changes an array's public class",
                            ));
                        };
                        if *length != u64::from(public_length) {
                            return Err(rejection(
                                "native value ABI changes its fixed-array length",
                            ));
                        }
                        let element = declaration(*element)
                            .filter(|row| row.id == *element)
                            .ok_or_else(|| {
                                rejection("native value ABI lost its array element declaration")
                            })?;
                        let element_shape = shape(signature, public_element)?;
                        let stride = usize::from(element_shape.byte_size);
                        // BoundaryValueLayout constructs exactly this product and
                        // inherits element alignment; classification uses this stride.
                        if stride.checked_mul(usize::from(public_length))
                            != Some(usize::from(public.byte_size))
                            || public.alignment != element_shape.alignment
                        {
                            return Err(rejection(
                                "native public array extent or alignment changes its element geometry",
                            ));
                        }
                        schema.active_types.push(semantic.id);
                        schema.active_shapes.push(identity);
                        pending.push(SchemaVisit::Array {
                            length: *length,
                            stride,
                        });
                        pending.push(SchemaVisit::Type(element, public_element));
                    }
                    _ => {
                        return Err(rejection(
                            "native value ABI needs plain scalar or finite record/array storage",
                        ));
                    }
                }
            }
            SchemaVisit::Field(field, identity) => match field.field_type {
                StructuralFieldType::Scalar(logical) => {
                    let stored = field
                        .storage
                        .as_ref()
                        .and_then(|storage| storage.stored_scalar())
                        .unwrap_or(logical);
                    validate_scalar_shape(*shape(signature, identity)?, stored)?;
                    schema.scalar(logical, identity)?;
                }
                StructuralFieldType::IeeeFloat(format) => {
                    let logical = ScalarType::IeeeFloat(format);
                    let stored = field
                        .storage
                        .as_ref()
                        .and_then(|storage| storage.stored_scalar())
                        .unwrap_or(logical);
                    validate_scalar_shape(*shape(signature, identity)?, stored)?;
                    schema.scalar(logical, identity)?;
                }
                StructuralFieldType::Structural(child) => {
                    let child = declaration(child)
                        .filter(|row| row.id == child)
                        .ok_or_else(|| {
                            rejection("native value ABI lost a semantic child declaration")
                        })?;
                    pending.push(SchemaVisit::Type(child, identity));
                }
                _ => {
                    return Err(rejection(
                        "native value ABI needs plain scalar or finite record/array fields",
                    ));
                }
            },
            SchemaVisit::Record {
                first_child,
                children,
            } => {
                let child_nodes = schema.results.split_off(first_child);
                if child_nodes.len() != children.len() {
                    return Err(rejection(
                        "native value ABI lost its record schema children",
                    ));
                }
                let leaves = child_nodes
                    .iter()
                    .try_fold(0usize, |sum, child| {
                        sum.checked_add(schema.nodes[*child].leaves)
                    })
                    .ok_or_else(|| rejection("native value ABI scalar roster overflows"))?;
                let children = children
                    .into_iter()
                    .zip(child_nodes)
                    .map(|((path, offset), node)| (path, offset, node))
                    .collect();
                schema.finish(Node {
                    leaves,
                    kind: NodeKind::Record(children),
                });
            }
            SchemaVisit::Array { length, stride } => {
                let element = schema
                    .results
                    .pop()
                    .ok_or_else(|| rejection("native value ABI lost its array element schema"))?;
                let leaves = schema.nodes[element]
                    .leaves
                    .checked_mul(usize::try_from(length).map_err(|_| {
                        rejection("native value ABI array length exceeds host indexing")
                    })?)
                    .ok_or_else(|| rejection("native value ABI scalar roster overflows"))?;
                schema.finish(Node {
                    leaves,
                    kind: NodeKind::Array {
                        element,
                        length,
                        stride,
                    },
                });
            }
        }
    }
    let [root] = schema.results.as_slice() else {
        return Err(rejection("native value ABI has no unique aggregate schema"));
    };
    if schema.nodes[*root].leaves != codec.fields.len() {
        return Err(rejection(
            "native value ABI changes its complete scalar codec roster",
        ));
    }
    validate_instances(signature, &schema.nodes, *root, codec)
}

enum InstanceVisit {
    Node(usize, usize),
    Child(Path, usize, usize),
    Leave,
}

fn validate_instances(
    signature: &BoundarySignatureGeometry,
    nodes: &[Node],
    root: usize,
    codec: &StorageCodec,
) -> Result<(), TerminalEffectRejection> {
    let mut pending = vec![InstanceVisit::Node(root, 0)];
    let mut path = Vec::new();
    let mut leaf = 0;
    while let Some(visit) = pending.pop() {
        match visit {
            InstanceVisit::Node(index, offset) => {
                let node = &nodes[index];
                if node.leaves == 0 {
                    continue;
                }
                match &node.kind {
                    NodeKind::Scalar {
                        logical,
                        shape: identity,
                    } => {
                        let field = codec.fields.get(leaf).ok_or_else(|| {
                            rejection("native value ABI lost a scalar codec field")
                        })?;
                        if field.path != path || field.offset != offset || field.logical != *logical
                        {
                            return Err(rejection(
                                "native value ABI changes a scalar's full path, offset or carrier",
                            ));
                        }
                        validate_scalar_shape(*shape(signature, *identity)?, field.stored)?;
                        leaf += 1;
                    }
                    NodeKind::Record(children) => {
                        for (segment, relative, child) in children.iter().rev() {
                            let offset = offset
                                .checked_add(*relative)
                                .ok_or_else(|| rejection("native public child offset overflows"))?;
                            pending.push(InstanceVisit::Child(*segment, *child, offset));
                        }
                    }
                    NodeKind::Array {
                        element,
                        length,
                        stride,
                    } => {
                        for index in (0..*length).rev() {
                            let offset = usize::try_from(index)
                                .ok()
                                .and_then(|index| index.checked_mul(*stride))
                                .and_then(|relative| offset.checked_add(relative))
                                .ok_or_else(|| {
                                    rejection("native public array element offset overflows")
                                })?;
                            pending.push(InstanceVisit::Child(
                                Path::FixedIndex(index),
                                *element,
                                offset,
                            ));
                        }
                    }
                }
            }
            InstanceVisit::Child(segment, node, offset) => {
                path.push(segment);
                pending.push(InstanceVisit::Leave);
                pending.push(InstanceVisit::Node(node, offset));
            }
            InstanceVisit::Leave => {
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
        .ok_or_else(|| rejection("native public aggregate shape is absent"))?;
    if !shape.alignment.is_power_of_two() {
        return Err(rejection(
            "native public aggregate shape has invalid alignment",
        ));
    }
    Ok(shape)
}

#[cfg(test)]
mod tests;

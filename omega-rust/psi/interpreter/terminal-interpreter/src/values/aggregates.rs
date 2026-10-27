//! Complete typed contents of ordinary finite scalar aggregates. Schema planning
//! visits each declaration once, including the elements of empty arrays. Only
//! after that validation do we expand initialized leaves. This keeps huge empty
//! dimensions cheap and prevents zero extent from hiding an unsupported carrier
//! or a cycle. Publication groups pure scalar arrays for the existing constructor
//! storage owner, which owns byte backing versus primitive cells.

use super::TerminalScalarAggregateValue;
use crate::errors::{TerminalInterpretError, malformed};
use semantic_vocabulary::{CanonicalStructuralPathSegment as Path, StructuralTypeId};
use terminal_psi::{
    StructuralFieldDeclaration, StructuralFieldType, StructuralRecordStorage,
    StructuralTypeDeclaration, StructuralTypeShape,
};

#[cfg(test)]
mod tests;

enum Element<'declaration> {
    Scalar(&'declaration StructuralFieldType),
    Structural(usize),
}

enum Shape<'declaration> {
    Scalar(StructuralFieldType),
    Record(
        Vec<(
            &'declaration StructuralFieldDeclaration,
            Element<'declaration>,
        )>,
    ),
    Array {
        element: usize,
        length: u64,
    },
}

struct Node<'declaration> {
    identity: StructuralTypeId,
    shape: Shape<'declaration>,
    // An overflowing inner count can still be suppressed by an outer zero.
    count: Option<u64>,
    scalar_tree: bool,
}

pub(crate) struct ScalarAggregatePlan<'declaration> {
    nodes: Vec<Node<'declaration>>,
    root: usize,
}

pub(crate) enum ScalarAggregatePart<'declaration> {
    Field(&'declaration StructuralFieldType),
    Primitive(&'declaration StructuralFieldType),
    Array {
        structural_type: StructuralTypeId,
        count: usize,
    },
}

enum Visit<'declaration> {
    Node(usize),
    Field(
        &'declaration StructuralFieldDeclaration,
        &'declaration Element<'declaration>,
    ),
    Element {
        node: usize,
        index: u64,
        length: u64,
    },
    Pop,
}

impl<'declaration> ScalarAggregatePlan<'declaration> {
    pub(crate) fn new(
        declaration: &'declaration StructuralTypeDeclaration,
        lookup: impl Fn(StructuralTypeId) -> Option<&'declaration StructuralTypeDeclaration>,
    ) -> Result<Self, TerminalInterpretError> {
        let mut nodes: Vec<Node<'declaration>> = Vec::new();
        let mut active = Vec::new();
        let mut pending = vec![(declaration, false)];
        while let Some((declaration, finish)) = pending.pop() {
            if nodes.iter().any(|node| node.identity == declaration.id) {
                continue;
            }
            if !finish {
                if declaration.quotient_carrier.is_some() || active.contains(&declaration.id) {
                    return Err(malformed());
                }
                active.push(declaration.id);
                pending.push((declaration, true));
                let mut child = |identity| {
                    let declaration = lookup(identity)
                        .filter(|declaration| declaration.id == identity)
                        .ok_or_else(malformed)?;
                    pending.push((declaration, false));
                    Ok::<_, TerminalInterpretError>(())
                };
                match &declaration.shape {
                    StructuralTypeShape::PrimitiveScalar(_) => {}
                    StructuralTypeShape::Record { fields, storage } => {
                        if matches!(
                            storage,
                            StructuralRecordStorage::PlacedView(_)
                                | StructuralRecordStorage::PlacedAccessor(_)
                        ) {
                            return Err(malformed());
                        }
                        for field in fields
                            .iter()
                            .rev()
                            .filter(|field| !field.relevance.is_erased())
                        {
                            if let StructuralFieldType::Structural(identity) = field.field_type {
                                child(identity)?;
                            } else if field.field_type.scalar_type().is_none() {
                                return Err(malformed());
                            }
                        }
                    }
                    StructuralTypeShape::FixedArray { element, .. } => child(*element)?,
                    _ => return Err(malformed()),
                }
                continue;
            }
            if active.pop() != Some(declaration.id) {
                return Err(malformed());
            }
            let child = |identity| {
                nodes
                    .iter()
                    .position(|node| node.identity == identity)
                    .ok_or_else(malformed)
            };
            let (shape, count, scalar_tree) = match &declaration.shape {
                StructuralTypeShape::PrimitiveScalar(scalar) => {
                    let field_type = declaration
                        .scalar_restriction
                        .clone()
                        .unwrap_or(StructuralFieldType::Scalar(*scalar));
                    if field_type.scalar_type() != Some(*scalar) {
                        return Err(malformed());
                    }
                    (Shape::Scalar(field_type), Some(1), true)
                }
                StructuralTypeShape::Record { fields, .. } => {
                    let mut members = Vec::new();
                    let mut count = Some(0_u64);
                    for field in fields.iter().filter(|field| !field.relevance.is_erased()) {
                        let (element, leaves) = match field.field_type {
                            StructuralFieldType::Structural(identity) => {
                                let index = child(identity)?;
                                (Element::Structural(index), nodes[index].count)
                            }
                            _ => (Element::Scalar(&field.field_type), Some(1)),
                        };
                        count = count
                            .zip(leaves)
                            .and_then(|(count, leaves)| count.checked_add(leaves));
                        members.push((field, element));
                    }
                    (Shape::Record(members), count, false)
                }
                StructuralTypeShape::FixedArray { element, length } => {
                    let element = child(*element)?;
                    let count = if *length == 0 {
                        Some(0)
                    } else {
                        nodes[element]
                            .count
                            .and_then(|count| count.checked_mul(*length))
                    };
                    (
                        Shape::Array {
                            element,
                            length: *length,
                        },
                        count,
                        nodes[element].scalar_tree,
                    )
                }
                _ => return Err(malformed()),
            };
            nodes.push(Node {
                identity: declaration.id,
                shape,
                count,
                scalar_tree,
            });
        }
        let root = nodes.len().checked_sub(1).ok_or_else(malformed)?;
        if nodes[root].count.is_none() {
            return Err(malformed());
        }
        Ok(Self { nodes, root })
    }

    pub(crate) fn count(&self) -> Result<usize, TerminalInterpretError> {
        usize::try_from(self.nodes[self.root].count.ok_or_else(malformed)?).map_err(|_| malformed())
    }

    pub(crate) fn scalar_fields(
        &self,
        mut scalar: impl FnMut(&[Path], &StructuralFieldType) -> Result<(), TerminalInterpretError>,
    ) -> Result<(), TerminalInterpretError> {
        self.walk(false, |path, part| match part {
            ScalarAggregatePart::Field(field) | ScalarAggregatePart::Primitive(field) => {
                scalar(path, field)
            }
            ScalarAggregatePart::Array { .. } => Err(malformed()),
        })
    }

    pub(crate) fn storage_parts(
        &self,
        part: impl FnMut(&[Path], ScalarAggregatePart<'_>) -> Result<(), TerminalInterpretError>,
    ) -> Result<(), TerminalInterpretError> {
        self.walk(true, part)
    }

    fn walk(
        &self,
        group_scalar_arrays: bool,
        mut part: impl FnMut(&[Path], ScalarAggregatePart<'_>) -> Result<(), TerminalInterpretError>,
    ) -> Result<(), TerminalInterpretError> {
        let mut pending = vec![Visit::Node(self.root)];
        let mut path = Vec::new();
        while let Some(visit) = pending.pop() {
            match visit {
                Visit::Node(index) => {
                    let node = &self.nodes[index];
                    if group_scalar_arrays
                        && node.scalar_tree
                        && matches!(node.shape, Shape::Array { .. })
                    {
                        part(
                            &path,
                            ScalarAggregatePart::Array {
                                structural_type: node.identity,
                                count: usize::try_from(node.count.ok_or_else(malformed)?)
                                    .map_err(|_| malformed())?,
                            },
                        )?;
                        continue;
                    }
                    if node.count == Some(0)
                        && (!group_scalar_arrays || matches!(node.shape, Shape::Array { .. }))
                    {
                        continue;
                    }
                    match &node.shape {
                        Shape::Scalar(field) => part(&path, ScalarAggregatePart::Primitive(field))?,
                        Shape::Record(fields) => pending.extend(
                            fields
                                .iter()
                                .rev()
                                .map(|(field, element)| Visit::Field(field, element)),
                        ),
                        Shape::Array { element, length } => pending.push(Visit::Element {
                            node: *element,
                            index: 0,
                            length: *length,
                        }),
                    }
                }
                Visit::Field(field, element) => {
                    path.push(Path::Field(field.id));
                    pending.push(Visit::Pop);
                    match element {
                        Element::Scalar(field_type) => {
                            part(&path, ScalarAggregatePart::Field(field_type))?
                        }
                        Element::Structural(index) => pending.push(Visit::Node(*index)),
                    }
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
                    path.pop().ok_or_else(malformed)?;
                }
            }
        }
        Ok(())
    }
}

impl TerminalScalarAggregateValue {
    pub(crate) fn validate_against<'declaration>(
        &self,
        declaration: &'declaration StructuralTypeDeclaration,
        lookup: impl Fn(StructuralTypeId) -> Option<&'declaration StructuralTypeDeclaration>,
    ) -> Result<(), TerminalInterpretError> {
        if declaration.id != self.structural_type || declaration.quotient_carrier.is_some() {
            return Err(malformed());
        }
        let mut actual = self.fields.iter();
        let mut accept = |path: &[Path], field_type: &StructuralFieldType| {
            let (found, value) = actual.next().ok_or_else(malformed)?;
            if found != path
                || field_type.scalar_type() != Some(value.scalar_type())
                || !value.satisfies_field(field_type)
            {
                return Err(malformed());
            }
            Ok(())
        };
        match (&declaration.shape, self.result_case) {
            (
                StructuralTypeShape::Record { .. }
                | StructuralTypeShape::FixedArray { .. }
                | StructuralTypeShape::PrimitiveScalar(_),
                None,
            ) => {
                let plan = ScalarAggregatePlan::new(declaration, lookup)?;
                if plan.count()? != self.fields.len() {
                    return Err(malformed());
                }
                plan.scalar_fields(&mut accept)?;
            }
            (StructuralTypeShape::Sum { cases }, Some(selected)) => {
                let case =
                    terminal_psi::find_structural_case(cases, selected).ok_or_else(malformed)?;
                for field in &case.fields {
                    if field.relevance.is_erased() || field.field_type.scalar_type().is_none() {
                        return Err(malformed());
                    }
                    accept(&[Path::Field(field.id)], &field.field_type)?;
                }
            }
            _ => return Err(malformed()),
        }
        if actual.next().is_some() {
            return Err(malformed());
        }
        Ok(())
    }
}

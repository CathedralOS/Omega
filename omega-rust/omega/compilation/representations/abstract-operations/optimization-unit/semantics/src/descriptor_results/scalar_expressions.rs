//! One immutable scalar namespace for a descriptor-machine adaptation.
//! Keep the kernel's expression and predecessor semantics; prepare only the
//! sparse type, definition, constant and incoming-edge lookups once.

use super::{O, kernel};
use optimization_unit::{OptimizationEdge, OptimizationNode, PsiOptimizationFunction};
use semantic_vocabulary::{BlockId, IntegerValue, ScalarTerm as T, ScalarType, ValueId};
use std::collections::BTreeMap;

pub(super) struct ScalarExpressions<'function> {
    entry: BlockId,
    types: BTreeMap<ValueId, ScalarType>,
    definitions: BTreeMap<ValueId, &'function OptimizationNode>,
    booleans: BTreeMap<ValueId, bool>,
    integers: BTreeMap<ValueId, (ScalarType, IntegerValue)>,
    owners: BTreeMap<ValueId, BlockId>,
    incoming: BTreeMap<BlockId, Vec<&'function OptimizationEdge>>,
}

impl<'function> ScalarExpressions<'function> {
    pub(super) fn new(function: &'function PsiOptimizationFunction) -> Self {
        let types = function
            .parameters
            .iter()
            .chain(function.blocks.iter().flat_map(|block| &block.parameters))
            .chain(function.block_nodes().flat_map(|node| &node.definitions))
            .map(|value| (value.value, value.scalar_type))
            .collect();
        let mut reader = Self {
            entry: function.entry,
            types,
            definitions: BTreeMap::new(),
            booleans: BTreeMap::new(),
            integers: BTreeMap::new(),
            owners: BTreeMap::new(),
            incoming: BTreeMap::new(),
        };
        for block in &function.blocks {
            for parameter in &block.parameters {
                reader.owners.entry(parameter.value).or_insert(block.id);
            }
            for node in &block.nodes {
                for definition in &node.definitions {
                    reader.definitions.entry(definition.value).or_insert(node);
                }
                match node.operation {
                    O::BooleanConstant { result, value, .. } => {
                        reader.booleans.entry(result).or_insert(value);
                    }
                    O::IntegerConstant {
                        result,
                        scalar_type,
                        value,
                        ..
                    } => {
                        reader
                            .integers
                            .entry(result)
                            .or_insert((scalar_type, value));
                    }
                    _ => {}
                }
                for edge in &node.successors {
                    reader.incoming.entry(edge.target).or_default().push(edge);
                }
            }
        }
        reader
    }

    fn literal(&self, value: ValueId) -> Option<T> {
        if let Some(&literal) = self.booleans.get(&value) {
            return Some(T::Boolean(literal));
        }
        let (scalar_type, literal) = terminal_semantics::scalar_constant_value(value, |value| {
            if let Some(&(scalar_type, literal)) = self.integers.get(&value) {
                return Some(terminal_semantics::ScalarConstantOrigin::Literal(
                    scalar_type,
                    literal,
                ));
            }
            let owner = *self.owners.get(&value)?;
            if owner == self.entry {
                return None;
            }
            let incoming = self
                .incoming
                .get(&owner)
                .into_iter()
                .flatten()
                .map(|edge| edge.binding(value).map(|binding| binding.argument))
                .collect::<Option<Vec<_>>>()?;
            Some(terminal_semantics::ScalarConstantOrigin::Incoming(incoming))
        })?;
        let ScalarType::Integer(integer) = scalar_type else {
            return None;
        };
        Some(T::Integer {
            scalar_type: integer,
            value: literal,
        })
    }

    pub(super) fn expression(&self, value: ValueId) -> Option<T> {
        let types = &self.types;
        kernel::scalar_payload_expression(value, &|value| {
            let scalar_type = *types.get(&value)?;
            if let Some(literal) = self.literal(value) {
                return Some((scalar_type, Some(literal)));
            }
            let term = |value| Some(Box::new(T::value(value, *types.get(&value)?)));
            let integer = |value| types.get(&value)?.as_integer();
            let denotation = self.definitions.get(&value).and_then(|node| {
                Some(match &node.operation {
                    O::IntegerArithmetic {
                        arithmetic,
                        scalar_type,
                        left,
                        right,
                        ..
                    } => arithmetic.scalar_term(*scalar_type, *term(*left)?, *term(*right)?),
                    O::BooleanNot { operand, .. } => T::BooleanNot {
                        operand: term(*operand)?,
                    },
                    O::BooleanEqual { left, right, .. } => T::BooleanEqual {
                        left: term(*left)?,
                        right: term(*right)?,
                    },
                    O::IntegerEqual { left, right, .. } => T::IntegerEqual {
                        scalar_type: integer(*left)?,
                        left: term(*left)?,
                        right: term(*right)?,
                    },
                    O::IntegerLessThan { left, right, .. } => T::IntegerLessThan {
                        scalar_type: integer(*left)?,
                        left: term(*left)?,
                        right: term(*right)?,
                    },
                    O::IntegerLessOrEqual { left, right, .. } => T::IntegerLessOrEqual {
                        scalar_type: integer(*left)?,
                        left: term(*left)?,
                        right: term(*right)?,
                    },
                    _ => return None,
                })
            });
            Some((scalar_type, denotation))
        })
    }
}

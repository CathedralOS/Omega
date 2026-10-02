//! Omega reconstructs executable scalar-order inputs; the shared solver owns
//! all-path comparison transport and captured-value stability.
use super::borrowed_regions::Position;
use abstract_operations::{AbstractOperation as O, ValueBinding};
use optimization_unit::PsiOptimizationFunction;
use semantic_vocabulary::BlockId;
#[cfg(test)]
use semantic_vocabulary::{IntegerSign, IntegerType, ScalarType, ValueId};
use terminal_semantics::scalar_ordering::{Binding, Edge, Effect, ScalarOrdering, Step};

pub(super) struct RangeRelations(ScalarOrdering);
impl RangeRelations {
    pub(super) fn new(function: &PsiOptimizationFunction) -> Self {
        let positions = function
            .blocks
            .iter()
            .flat_map(|block| {
                (0..block.nodes.len())
                    .filter_map(|index| u32::try_from(index).ok().map(|index| (block.id, index)))
            })
            .collect::<Vec<_>>();
        let mut steps = Vec::with_capacity(positions.len());
        for position in &positions {
            let block = function
                .blocks
                .iter()
                .find(|block| block.id == position.0)
                .expect("position comes from block");
            let operation = &block.nodes[position.1 as usize].operation;
            let edge = |target: BlockId, bindings: &[ValueBinding], condition| {
                let target_index = positions
                    .iter()
                    .position(|position| *position == (target, 0))?;
                let block = function.blocks.iter().find(|block| block.id == target)?;
                Some(Edge {
                    target: target_index,
                    enters_block: true,
                    parameters: block
                        .parameters
                        .iter()
                        .map(|parameter| (parameter.value, parameter.scalar_type))
                        .collect(),
                    bindings: bindings
                        .iter()
                        .map(|binding| Binding {
                            parameter: binding.parameter,
                            argument: binding.argument,
                            scalar_type: binding.scalar_type,
                        })
                        .collect(),
                    condition,
                })
            };
            let successors = match operation {
                O::Jump {
                    target, bindings, ..
                } => edge(*target, bindings, None).into_iter().collect(),
                O::Conditional {
                    condition,
                    when_true,
                    when_false,
                } => [
                    edge(
                        when_true.target,
                        &when_true.bindings,
                        Some((*condition, true)),
                    ),
                    edge(
                        when_false.target,
                        &when_false.bindings,
                        Some((*condition, false)),
                    ),
                ]
                .into_iter()
                .flatten()
                .collect(),
                O::StructuralCase { cases, .. } => cases
                    .iter()
                    .filter_map(|case| edge(case.target, &[], None))
                    .collect(),
                O::Return { .. }
                | O::ReturnUnit { .. }
                | O::ReturnStructural { .. }
                | O::Crash { .. } => Vec::new(),
                _ => positions
                    .iter()
                    .position(|next| *next == (position.0, position.1 + 1))
                    .map(|target| Edge {
                        target,
                        enters_block: false,
                        parameters: Vec::new(),
                        bindings: Vec::new(),
                        condition: None,
                    })
                    .into_iter()
                    .collect(),
            };
            steps.push(Step {
                position: *position,
                definitions: crate::unit_validation::derived_metadata::expected_definitions(
                    operation, position.0, position.1,
                )
                .iter()
                .map(|definition| (definition.value, definition.scalar_type))
                .collect(),
                effect: match operation {
                    O::IntegerLessThan {
                        result,
                        left,
                        right,
                        ..
                    }
                    | O::IntegerLessOrEqual {
                        result,
                        left,
                        right,
                        ..
                    } => Effect::Compare {
                        result: *result,
                        left: *left,
                        right: *right,
                        strict: matches!(operation, O::IntegerLessThan { .. }),
                    },
                    O::BooleanNot {
                        result, operand, ..
                    } => Effect::Not {
                        result: *result,
                        operand: *operand,
                    },
                    O::IntegerConstant {
                        result,
                        value: semantic_vocabulary::IntegerValue::Unsigned(value),
                        ..
                    } => Effect::UnsignedLiteral {
                        result: *result,
                        value: *value,
                    },
                    _ => Effect::None,
                },
                successors,
            });
        }
        Self(ScalarOrdering::new(
            steps,
            (function.entry, 0),
            function
                .parameters
                .iter()
                .map(|parameter| (parameter.value, parameter.scalar_type))
                .collect(),
        ))
    }
    pub(super) fn ordering(&self) -> &ScalarOrdering {
        &self.0
    }

    #[cfg(test)]
    pub(super) fn proves_less_than(
        &self,
        position: Position,
        left: ValueId,
        right: ValueId,
    ) -> bool {
        self.0.proves_less_than(position, left, right)
    }
    #[cfg(test)]
    pub(super) fn proves_less_or_equal(
        &self,
        position: Position,
        left: ValueId,
        right: ValueId,
    ) -> bool {
        self.0.proves_less_or_equal(position, left, right)
    }
    #[cfg(test)]
    pub(super) fn values_unchanged_since(
        &self,
        birth: Position,
        current: Position,
        values: &[ValueId],
    ) -> bool {
        self.0.values_unchanged_since(birth, current, values)
    }
}
#[cfg(test)]
#[path = "range_relations/tests.rs"]
mod tests;

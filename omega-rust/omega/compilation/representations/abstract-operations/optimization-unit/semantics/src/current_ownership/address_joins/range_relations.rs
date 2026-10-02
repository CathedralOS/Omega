//! Omega reconstructs executable scalar-order inputs; the shared solver owns
//! all-path comparison transport and captured-value stability.
#[cfg(test)]
use super::borrowed_regions::Position;
use abstract_operations::{AbstractOperation as O, ValueBinding};
use optimization_unit::PsiOptimizationFunction;
use semantic_vocabulary::{BlockId, MachineId, Proposition, ScalarType, ValueId};
#[cfg(test)]
use semantic_vocabulary::{IntegerSign, IntegerType};
use std::collections::BTreeMap;
use terminal_semantics::scalar_ordering::{Binding, Edge, Effect, ScalarOrdering, Step};

pub(super) struct RangeRelations(ScalarOrdering);
impl RangeRelations {
    pub(super) fn new(
        function: &PsiOptimizationFunction,
        functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
    ) -> Self {
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
        // Contract custody is attached from the verified Terminal product and
        // independently checked against the immutable seed. Executable call
        // occurrences come from this current graph: no stale source operation
        // coordinate is used to import a guarantee after a rewrite.
        let scalar_types = function
            .parameters
            .iter()
            .chain(function.blocks.iter().flat_map(|block| &block.parameters))
            .map(|parameter| (parameter.value, parameter.scalar_type))
            .chain(
                steps
                    .iter()
                    .flat_map(|step| step.definitions.iter().copied()),
            )
            .collect::<Vec<_>>();
        let normal_returns = function
            .blocks
            .iter()
            .flat_map(|block| {
                let scalar_types = &scalar_types;
                block
                    .nodes
                    .iter()
                    .enumerate()
                    .filter_map(move |(index, node)| {
                        let propositions =
                            call_guarantees(&node.operation, functions, scalar_types)?;
                        Some(((block.id, u32::try_from(index).ok()?), propositions))
                    })
            })
            .collect();
        Self(ScalarOrdering::with_contracts(
            steps,
            (function.entry, 0),
            function
                .parameters
                .iter()
                .map(|parameter| (parameter.value, parameter.scalar_type))
                .collect(),
            function
                .verified_contract
                .as_ref()
                .map_or(&[], |contract| contract.requires.as_slice()),
            normal_returns,
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
/// Instantiate only current, statically named scalar return contracts. The
/// common contract projection rejects unbound leaves and unsupported clauses;
/// result/formal namespaces never fall back to coincident caller identities.
fn call_guarantees(
    operation: &O,
    functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
    scalar_types: &[(ValueId, ScalarType)],
) -> Option<Vec<Proposition>> {
    let (callee, arguments, result) = match operation {
        O::Call {
            callee,
            arguments,
            result,
            scalar_type,
            ..
        } => (
            *callee,
            arguments,
            abstract_operations::AbstractResult {
                value: *result,
                scalar_type: *scalar_type,
            },
        ),
        O::CallStructural {
            callee,
            arguments,
            result,
            ..
        } => (*callee, arguments, result.scalar()?),
        _ => return None,
    };
    let callee = functions.get(&callee)?;
    let contract = callee.verified_contract.as_ref()?;
    let signature = callee.result.scalar()?;
    // A formal denotes its pre-call argument, while the result denotes the
    // completed value. Reusing one identifier would erase that distinction.
    if arguments.contains(&result.value)
        || signature.scalar_type != result.scalar_type
        || arguments.len() != callee.parameters.len()
        || !arguments
            .iter()
            .zip(&callee.parameters)
            .all(|(actual, formal)| {
                scalar_types.contains(&(*actual, formal.scalar_type))
                    && scalar_types.iter().all(|(value, scalar_type)| {
                        value != actual || *scalar_type == formal.scalar_type
                    })
            })
        || !contract.erased_scalar_formals.is_empty()
        || !contract.erased_proof_formals.is_empty()
    {
        return None;
    }
    let mut bindings = callee
        .parameters
        .iter()
        .zip(arguments)
        .map(|(formal, actual)| {
            (
                formal.value,
                semantic_vocabulary::ScalarTerm::value(*actual, formal.scalar_type),
            )
        })
        .collect::<Vec<_>>();
    bindings.push((
        signature.value,
        semantic_vocabulary::ScalarTerm::value(result.value, signature.scalar_type),
    ));
    if bindings
        .iter()
        .enumerate()
        .any(|(index, row)| bindings[..index].iter().any(|other| other.0 == row.0))
    {
        return None;
    }
    Some(
        contract
            .ensures
            .iter()
            .flat_map(|clause| {
                terminal_semantics::scalar_ordering::instantiate_contract_ordering(
                    &clause.proposition,
                    &bindings,
                )
            })
            .collect(),
    )
}

#[cfg(test)]
#[path = "range_relations/tests.rs"]
mod tests;

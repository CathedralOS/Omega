//! Transient inference state. Active body guards and frozen local origin evidence
//! have separate lifetimes; neither is a source of borrow permission.

use super::StoredLocalOrigins;
use super::call_targets::machine_state_by_symbol;
use crate::value_custody::places::binding_observations::{
    BindingObservation, binding_has_write_effects, binding_is_exclusively_exposed,
    binding_is_written,
};
use symbols::SymbolHandle;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator};
use typed_trees::state::State;
use typed_trees::statement::StatementNode;

#[derive(Clone, Default)]
pub(super) struct FrameInference {
    pub(super) active_states: Vec<SymbolHandle>,
    local_shapes: Vec<StoredLocalOrigins>,
    parameter_values: Option<ParameterValues>,
}

impl FrameInference {
    pub(super) fn for_state(symbol: SymbolHandle) -> Self {
        Self {
            active_states: vec![symbol],
            ..Self::default()
        }
    }
    pub(super) fn record_local(&mut self, local: &StoredLocalOrigins) {
        self.local_shapes.push(local.clone());
    }

    pub(super) fn local_origins(&self, symbol: SymbolHandle) -> Option<&StoredLocalOrigins> {
        self.local_shapes
            .iter()
            .rev()
            .find(|local| local.local_symbol == symbol)
    }

    pub(super) fn with_local_scope<T>(&mut self, action: impl FnOnce(&mut Self) -> T) -> T {
        let start = self.local_shapes.len();
        let result = action(self);
        self.local_shapes.truncate(start);
        result
    }

    pub(super) fn argument_values(
        &self,
        program: &dyn crate::CheckingInput,
        arguments: &[ExpressionHandle],
    ) -> Vec<BoolTruth> {
        if let Some(parameter_values) = &self.parameter_values {
            return parameter_values.argument_values(program, arguments);
        }
        arguments
            .iter()
            .map(
                |argument| match program.expression_table.expression(*argument) {
                    ExpressionNode::Boolean(true) => BoolTruth::True,
                    ExpressionNode::Boolean(false) => BoolTruth::False,
                    _ => BoolTruth::Unknown,
                },
            )
            .collect()
    }

    /// Only a proven fixed scalar value refines selector geometry. Each
    /// invocation supplies already completed actuals; unresolved values remain
    /// the ordinary possible-element relation.
    pub(super) fn argument_indices(
        &self,
        program: &dyn crate::CheckingInput,
        arguments: &[ExpressionHandle],
    ) -> Vec<Option<usize>> {
        arguments
            .iter()
            .map(|argument| self.index_value(program, *argument))
            .collect()
    }

    pub(super) fn index_value(
        &self,
        program: &dyn crate::CheckingInput,
        expression: ExpressionHandle,
    ) -> Option<usize> {
        constant_index(program, expression).or_else(|| {
            let ExpressionNode::Name(name) = program.expression_table.expression(expression) else {
                return None;
            };
            self.parameter_values
                .as_ref()?
                .indices
                .iter()
                .find(|(symbol, _)| *symbol == name.symbol)
                .and_then(|(_, value)| *value)
        })
    }

    pub(super) fn specialize_segments(
        &self,
        program: &dyn crate::CheckingInput,
        segments: &mut [facts::PlaceSegment],
    ) {
        for segment in segments {
            if let facts::PlaceSegment::Index { expression } = *segment
                && expression.is_valid()
            {
                let index = self.index_value(program, expression);
                if let Some(index) = index {
                    *segment = facts::PlaceSegment::FixedIndex { index };
                } else if std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_some() {
                    // Metadata only: the existing value query has already refused.
                    // Do not unwrap a capture, inspect current storage or retry it.
                    let (kind, symbol, capture) =
                        match program.expression_table.expression(expression) {
                            ExpressionNode::Name(name) => ("name", name.symbol, Default::default()),
                            ExpressionNode::SavedSubject(saved) => {
                                ("saved-subject", SymbolHandle::invalid(), saved.capture)
                            }
                            ExpressionNode::Integer(_) => {
                                ("integer", SymbolHandle::invalid(), Default::default())
                            }
                            ExpressionNode::Cast(_) => {
                                ("cast", SymbolHandle::invalid(), Default::default())
                            }
                            ExpressionNode::Range(_) => {
                                ("range", SymbolHandle::invalid(), Default::default())
                            }
                            _ => ("other", SymbolHandle::invalid(), Default::default()),
                        };
                    eprintln!(
                        "CHECKSITE selector-specialization-unavailable expression={:?} kind={} name_symbol={:?} capture={:?} scope={:?} active_count={} active_prefix={:?}",
                        expression,
                        kind,
                        symbol,
                        capture,
                        self.parameter_values.as_ref().map(|values| (
                            values.state,
                            values.entry_indices.len(),
                            &values.entry_indices[..values.entry_indices.len().min(8)],
                            values.indices.len(),
                            &values.indices[..values.indices.len().min(8)],
                        )),
                        self.active_states.len(),
                        &self.active_states[..self.active_states.len().min(4)],
                    );
                }
            }
        }
    }

    pub(super) fn specialize_source(
        &self,
        program: &dyn crate::CheckingInput,
        source: &mut super::FrameSourcePlace,
    ) {
        if !source.builtin_coordinates {
            return;
        }
        self.specialize_segments(program, &mut source.segments);
        if let Some((_, path)) = &mut source.representation {
            self.specialize_segments(program, path);
        }
        for recast in &mut source.recasts {
            self.specialize_segments(program, &mut recast.segments);
            if let Some((_, path)) = &mut recast.representation {
                self.specialize_segments(program, path);
            }
        }
    }

    pub(super) fn selects_state(&self, state: &State) -> bool {
        self.parameter_values
            .as_ref()
            .is_some_and(|parameter_values| parameter_values.state == state.symbol)
    }

    /// A body consumes its own entry snapshots, never another state's prefix.
    /// Restore the caller even when inference refuses partway through the body.
    pub(super) fn with_invocation<T>(
        &mut self,
        program: &dyn crate::CheckingInput,
        state: &State,
        values: &[BoolTruth],
        indices: &[Option<usize>],
        action: impl FnOnce(&mut Self) -> T,
    ) -> T {
        let indices = program
            .machine_holding_state(state.symbol)
            .filter(|machine| {
                super::transition_topology::named_state_transition_subgraph_is_acyclic(
                    program, machine, state,
                )
            })
            .map_or(&[][..], |_| indices);
        let parameter_values = ParameterValues::new(program, state, values, indices);
        // With no known eligible input the ordinary full-union summary is
        // sufficient and remains shareable. Never specialize a cache entry.
        let previous = std::mem::replace(
            &mut self.parameter_values,
            parameter_values
                .has_known_values()
                .then_some(parameter_values),
        );
        let result = action(self);
        self.parameter_values = previous;
        result
    }

    /// A nested relation walk cannot advance the enclosing call's live values.
    pub(super) fn with_state_values<T>(
        &mut self,
        program: &dyn crate::CheckingInput,
        state: &State,
        action: impl FnOnce(&mut Self) -> T,
    ) -> T {
        let parameter_values = if self.selects_state(state) {
            self.parameter_values
                .as_ref()
                .map(|parameter_values| parameter_values.restart(program, state))
        } else {
            Some(ParameterValues::new(program, state, &[], &[]))
        };
        let previous = std::mem::replace(&mut self.parameter_values, parameter_values);
        let result = action(self);
        self.parameter_values = previous;
        result
    }

    /// Cyclic state equations remain context-independent and keep their full
    /// union. Invocation snapshots must not enter their state-keyed fixed point.
    pub(super) fn without_values<T>(&mut self, action: impl FnOnce(&mut Self) -> T) -> T {
        let previous = self.parameter_values.take();
        let result = action(self);
        self.parameter_values = previous;
        result
    }

    pub(super) fn finish_statement(
        &mut self,
        program: &dyn crate::CheckingInput,
        state: &State,
        statement: &StatementNode,
    ) {
        if let Some(parameter_values) = &mut self.parameter_values {
            parameter_values.finish_statement(program, state, statement);
        }
    }
}

/// Invocation entry snapshots and live private-home values for one state walk.
/// Calls consume values at their program point, never replay outer expressions.
#[derive(Clone)]
pub(super) struct ParameterValues {
    pub(super) state: SymbolHandle,
    entry_values: Vec<BoolTruth>,
    values: Vec<(SymbolHandle, Option<BoolTruth>)>,
    entry_indices: Vec<Option<usize>>,
    indices: Vec<(SymbolHandle, Option<usize>)>,
}

impl ParameterValues {
    pub(super) fn new(
        program: &dyn crate::CheckingInput,
        state: &State,
        actual_values: &[BoolTruth],
        actual_indices: &[Option<usize>],
    ) -> Self {
        // Actuals are entry snapshots. Only a private owned Bool home can be
        // advanced by the direct-store transfer below. Exposure and indirect
        // writes retain the original unspecialized union throughout the body.
        let values = program
            .state_parameters(state)
            .iter()
            .enumerate()
            .map(|(position, parameter)| {
                let reference = program
                    .type_reference_table
                    .unconstrained_type_reference(parameter.type_reference);
                let private_boolean =
                    crate::value_custody::recasts::exact_primitive_type(program, reference)
                        == Some(typed_trees::types::PrimitiveType::Bool)
                        && !binding_is_exclusively_exposed(program, state, parameter.symbol)
                        && !binding_has_write_effects(
                            program,
                            state,
                            parameter.symbol,
                            BindingObservation::WholeValue,
                        );
                (
                    parameter.symbol,
                    private_boolean.then(|| {
                        actual_values
                            .get(position)
                            .copied()
                            .unwrap_or(BoolTruth::Unknown)
                    }),
                )
            })
            .collect();
        let indices = program
            .state_parameters(state)
            .iter()
            .enumerate()
            .map(|(position, parameter)| {
                let value = actual_indices.get(position).copied().flatten().filter(|_| {
                    !super::type_reference_is_reference(program, parameter.type_reference)
                        && crate::value_custody::recasts::exact_primitive_type(
                            program,
                            parameter.type_reference,
                        )
                        .is_some_and(|primitive| primitive.is_fixed_integer())
                        && !binding_is_exclusively_exposed(program, state, parameter.symbol)
                        && !binding_is_written(
                            program,
                            state,
                            parameter.symbol,
                            BindingObservation::WholeValue,
                        )
                });
                (parameter.symbol, value)
            })
            .collect();
        Self {
            state: state.symbol,
            entry_values: actual_values.to_vec(),
            values,
            entry_indices: actual_indices.to_vec(),
            indices,
        }
    }

    pub(super) fn has_known_values(&self) -> bool {
        self.values
            .iter()
            .any(|(_, value)| matches!(value, Some(BoolTruth::True | BoolTruth::False)))
            || self.indices.iter().any(|(_, value)| value.is_some())
    }

    pub(super) fn restart(&self, program: &dyn crate::CheckingInput, state: &State) -> Self {
        Self::new(program, state, &self.entry_values, &self.entry_indices)
    }

    pub(super) fn argument_values(
        &self,
        program: &dyn crate::CheckingInput,
        arguments: &[ExpressionHandle],
    ) -> Vec<BoolTruth> {
        let Some((_, state)) = machine_state_by_symbol(program, self.state) else {
            return vec![BoolTruth::Unknown; arguments.len()];
        };
        arguments
            .iter()
            .map(|argument| bool_truth(program, state, self, *argument))
            .collect()
    }

    pub(super) fn truth(
        &self,
        program: &dyn crate::CheckingInput,
        state: &State,
        expression: ExpressionHandle,
    ) -> BoolTruth {
        bool_truth(program, state, self, expression)
    }

    /// Commit a direct store only after its operands consumed the old value.
    /// The same transfer serves return selection and chronological write frames.
    pub(super) fn finish_statement(
        &mut self,
        program: &dyn crate::CheckingInput,
        state: &State,
        statement: &StatementNode,
    ) {
        let StatementNode::Assignment(assignment) = statement else {
            return;
        };
        let ExpressionNode::Name(target) = program.expression_table.expression(assignment.target)
        else {
            return;
        };
        if let Some(position) = self
            .values
            .iter()
            .position(|(symbol, value)| *symbol == target.symbol && value.is_some())
        {
            let value = bool_truth(program, state, self, assignment.value);
            self.values[position].1 = Some(value);
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum BoolTruth {
    True,
    False,
    Unknown,
}

fn bool_truth(
    program: &dyn crate::CheckingInput,
    state: &State,
    current: &ParameterValues,
    expression: ExpressionHandle,
) -> BoolTruth {
    match program.expression_table.expression(expression) {
        ExpressionNode::Boolean(value) => {
            if *value {
                BoolTruth::True
            } else {
                BoolTruth::False
            }
        }
        ExpressionNode::Unary(unary) if unary.operator == UnaryOperator::LogicalNot => {
            bool_truth(program, state, current, unary.operand).negate()
        }
        ExpressionNode::Binary(binary) => {
            if !crate::has_builtin_parameter_bound_expression_meaning(
                program,
                state.symbol,
                program.state_parameters(state),
                expression,
            ) {
                return BoolTruth::Unknown;
            }
            let left = bool_truth(program, state, current, binary.left);
            let right = bool_truth(program, state, current, binary.right);
            match binary.operator {
                BinaryOperator::Equal => left.equal(right),
                BinaryOperator::NotEqual => left.equal(right).negate(),
                BinaryOperator::And => left.and(right),
                BinaryOperator::Or => left.or(right),
                _ => BoolTruth::Unknown,
            }
        }
        ExpressionNode::Name(path) => current
            .values
            .iter()
            .find(|(symbol, _)| *symbol == path.symbol)
            .and_then(|(_, value)| *value)
            .unwrap_or(BoolTruth::Unknown),
        _ => BoolTruth::Unknown,
    }
}

impl BoolTruth {
    fn negate(self) -> Self {
        match self {
            Self::True => Self::False,
            Self::False => Self::True,
            Self::Unknown => Self::Unknown,
        }
    }

    fn equal(self, other: Self) -> Self {
        match (self, other) {
            (Self::Unknown, _) | (_, Self::Unknown) => Self::Unknown,
            (Self::True, Self::True) | (Self::False, Self::False) => Self::True,
            _ => Self::False,
        }
    }

    fn and(self, other: Self) -> Self {
        match (self, other) {
            (Self::False, _) | (_, Self::False) => Self::False,
            (Self::True, Self::True) => Self::True,
            _ => Self::Unknown,
        }
    }

    fn or(self, other: Self) -> Self {
        match (self, other) {
            (Self::True, _) | (_, Self::True) => Self::True,
            (Self::False, Self::False) => Self::False,
            _ => Self::Unknown,
        }
    }
}

fn constant_index(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> Option<usize> {
    if !expression.is_valid() {
        return None;
    }
    let ExpressionNode::Integer(integer) = program.expression_table.expression(expression) else {
        return None;
    };
    usize::try_from(integer.value_i64()?).ok()
}

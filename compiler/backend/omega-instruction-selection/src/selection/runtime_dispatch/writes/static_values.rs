use crate::InstructionSelectionInput;
use omega_checked_trees::expression::{
    Expression, ExpressionHandle, ExpressionNode, ExpressionTable,
};
use omega_control_flow::StateKey;

use super::super::super::bindings::{
    RuntimeAliasBinding, resolve_runtime_alias_expression, strip_mutable_expression,
};
use super::super::super::storage_places::{enum_variant_value, enum_variant_value_in_table};
use omega_platform_interface::PlaceKey;

const INLINE_RUNTIME_STATIC_VALUE_COUNT: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeStaticValues {
    inline: [Option<(PlaceKey, i64)>; INLINE_RUNTIME_STATIC_VALUE_COUNT],
    len: usize,
    overflow: Vec<(PlaceKey, i64)>,
}

impl RuntimeStaticValues {
    pub(crate) fn new() -> Self {
        Self::with_capacity(0)
    }

    pub(crate) fn with_capacity(capacity: usize) -> Self {
        Self {
            inline: std::array::from_fn(|_| None),
            len: 0,
            overflow: Vec::with_capacity(
                capacity.saturating_sub(INLINE_RUNTIME_STATIC_VALUE_COUNT),
            ),
        }
    }

    pub(crate) fn clear(&mut self) {
        let inline_len = self.len.min(INLINE_RUNTIME_STATIC_VALUE_COUNT);
        for slot in self.inline.iter_mut().take(inline_len) {
            *slot = None;
        }

        self.len = 0;
        self.overflow.clear();
    }

    fn get(&self, target: &PlaceKey) -> Option<i64> {
        self.iter()
            .find(|(existing_target, _)| existing_target == target)
            .map(|(_, value)| *value)
    }

    /// Forget any recorded constant for `target`. Used after a write whose new
    /// value is not a compile-time constant (binary read-modify-write, copies,
    /// call results): subsequent reads must come from live storage, not the
    /// stale entry-value constant. Inline slots become `None` holes (skipped by
    /// `iter`); overflow entries are dropped.
    fn invalidate(&mut self, target: &PlaceKey) {
        for slot in self
            .inline
            .iter_mut()
            .take(self.len.min(INLINE_RUNTIME_STATIC_VALUE_COUNT))
        {
            if matches!(slot, Some((existing, _)) if existing == target) {
                *slot = None;
            }
        }
        self.overflow.retain(|(existing, _)| existing != target);
    }

    fn set(&mut self, target: PlaceKey, value: i64) {
        if let Some((_, existing_value)) = self
            .iter_mut()
            .find(|(existing_target, _)| existing_target == &target)
        {
            *existing_value = value;
            return;
        }

        if self.len < INLINE_RUNTIME_STATIC_VALUE_COUNT {
            self.inline[self.len] = Some((target, value));
        } else {
            self.overflow.push((target, value));
        }

        self.len += 1;
    }

    fn iter(&self) -> impl Iterator<Item = &(PlaceKey, i64)> {
        self.inline
            .iter()
            .take(self.len.min(INLINE_RUNTIME_STATIC_VALUE_COUNT))
            .filter_map(Option::as_ref)
            .chain(self.overflow.iter())
    }

    fn iter_mut(&mut self) -> impl Iterator<Item = &mut (PlaceKey, i64)> {
        self.inline
            .iter_mut()
            .take(self.len.min(INLINE_RUNTIME_STATIC_VALUE_COUNT))
            .filter_map(Option::as_mut)
            .chain(self.overflow.iter_mut())
    }
}

impl Default for RuntimeStaticValues {
    fn default() -> Self {
        Self::new()
    }
}

pub(super) fn resolve_runtime_static_integer_value(
    input: &InstructionSelectionInput<'_>,
    source_key: StateKey,
    expression: &Expression,
    aliases: &[RuntimeAliasBinding],
    alias_expressions: &ExpressionTable,
    static_values: &RuntimeStaticValues,
) -> Option<i64> {
    match expression {
        Expression::Integer(value) => Some(*value),
        Expression::Name(_) => enum_variant_value(&input.layouts, expression).or_else(|| {
            resolve_runtime_resolved_static_integer_value(
                input,
                resolve_runtime_alias_expression(
                    expression,
                    source_key,
                    aliases,
                    alias_expressions,
                ),
                static_values,
            )
        }),
        Expression::Indexed(_) | Expression::Member(_) | Expression::Mutable(_) => {
            resolve_runtime_resolved_static_integer_value(
                input,
                resolve_runtime_alias_expression(
                    expression,
                    source_key,
                    aliases,
                    alias_expressions,
                ),
                static_values,
            )
        }
        Expression::Range(_) => None,
        Expression::Unary(unary) => {
            let value = resolve_runtime_static_integer_value(
                input,
                source_key,
                &unary.operand,
                aliases,
                alias_expressions,
                static_values,
            )?;
            Some(i64::from(value == 0))
        }
        Expression::Boolean(value) => Some(i64::from(*value)),
        Expression::ArrayLiteral(_)
        | Expression::Binary(_)
        | Expression::Call(_)
        | Expression::Cast(_)
        | Expression::Float(_)
        | Expression::String(_)
        | Expression::StructLiteral(_) => None,
    }
}

pub(super) fn resolve_runtime_static_integer_value_in_table(
    input: &InstructionSelectionInput<'_>,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
    static_values: &RuntimeStaticValues,
) -> Option<i64> {
    match expressions.expression(expression) {
        ExpressionNode::Integer(value) => Some(*value),
        ExpressionNode::Boolean(value) => Some(i64::from(*value)),
        ExpressionNode::Name(_) => {
            enum_variant_value_in_table(&input.layouts, expressions, expression).or_else(|| {
                let key = PlaceKey::from_expression_handle(expressions, expression)?;
                static_values.get(&key)
            })
        }
        ExpressionNode::Indexed(_) | ExpressionNode::Member(_) | ExpressionNode::Mutable(_) => {
            let key = PlaceKey::from_expression_handle(expressions, expression)?;
            static_values.get(&key)
        }
        ExpressionNode::Unary(unary) => {
            let value = resolve_runtime_static_integer_value_in_table(
                input,
                expressions,
                unary.operand,
                static_values,
            )?;
            Some(i64::from(value == 0))
        }
        ExpressionNode::Range(_) => None,
        ExpressionNode::ArrayLiteral(_)
        | ExpressionNode::Binary(_)
        | ExpressionNode::Call(_)
        | ExpressionNode::Cast(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::String(_)
        | ExpressionNode::StructLiteral(_) => None,
    }
}

fn resolve_runtime_resolved_static_integer_value(
    input: &InstructionSelectionInput<'_>,
    expression: Expression,
    static_values: &RuntimeStaticValues,
) -> Option<i64> {
    let expression = strip_mutable_expression(expression);
    match expression {
        Expression::Integer(value) => Some(value),
        Expression::Boolean(value) => Some(i64::from(value)),
        Expression::Name(_) | Expression::Indexed(_) | Expression::Member(_) => {
            enum_variant_value(&input.layouts, &expression).or_else(|| {
                let key = PlaceKey::from_expression(&expression)?;
                static_values.get(&key)
            })
        }
        Expression::Unary(unary) => {
            let value =
                resolve_runtime_resolved_static_integer_value(input, unary.operand, static_values)?;
            Some(i64::from(value == 0))
        }
        Expression::Range(_) => None,
        Expression::Mutable(_)
        | Expression::ArrayLiteral(_)
        | Expression::Binary(_)
        | Expression::Call(_)
        | Expression::Cast(_)
        | Expression::Float(_)
        | Expression::String(_)
        | Expression::StructLiteral(_) => None,
    }
}

pub(super) fn set_runtime_static_value(
    static_values: &mut RuntimeStaticValues,
    target: Expression,
    value: i64,
) {
    let Some(target) = PlaceKey::from_expression(&strip_mutable_expression(target)) else {
        return;
    };

    static_values.set(target, value);
}

pub(super) fn set_runtime_static_value_in_table(
    static_values: &mut RuntimeStaticValues,
    expressions: &ExpressionTable,
    target: ExpressionHandle,
    value: i64,
) {
    let Some(target) = PlaceKey::from_expression_handle(expressions, target) else {
        return;
    };

    static_values.set(target, value);
}

/// Drop any recorded constant for the place written by `target`. Call this after
/// emitting a write whose value is not a tracked compile-time constant so later
/// reads of the same place resolve against live storage instead of a stale fold.
pub(super) fn invalidate_runtime_static_value_in_table(
    static_values: &mut RuntimeStaticValues,
    expressions: &ExpressionTable,
    target: ExpressionHandle,
) {
    let Some(target) = PlaceKey::from_expression_handle(expressions, target) else {
        return;
    };

    static_values.invalidate(&target);
}

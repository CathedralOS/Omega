use crate::place_keys::PlaceKey;
use psi_checked_trees::CheckedTrees;
use psi_checked_trees::expression::{ExpressionHandle, ExpressionNode, ExpressionTable};
use psi_checked_trees::machine::Machine;
use psi_checked_trees::statement::{TableAssignment, TableCall};
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum StaticValue {
    Integer(i64),
    Expression(ExpressionHandle),
    Text(Arc<[u8]>),
}

const INLINE_STATIC_VALUE_COUNT: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StaticValues {
    inline: [Option<(PlaceKey, StaticValue)>; INLINE_STATIC_VALUE_COUNT],
    len: usize,
    overflow: Vec<(PlaceKey, StaticValue)>,
}

impl StaticValues {
    pub(crate) fn new() -> Self {
        Self::with_capacity(0)
    }

    pub(crate) fn with_capacity(capacity: usize) -> Self {
        Self {
            inline: std::array::from_fn(|_| None),
            len: 0,
            overflow: Vec::with_capacity(capacity.saturating_sub(INLINE_STATIC_VALUE_COUNT)),
        }
    }

    fn len(&self) -> usize {
        self.len
    }

    fn get(&self, target_key: &PlaceKey) -> Option<StaticValue> {
        self.iter()
            .find(|(existing_key, _)| existing_key == target_key)
            .map(|(_, value)| value.clone())
    }

    fn get_index(&self, index: usize) -> Option<&(PlaceKey, StaticValue)> {
        if index >= self.len {
            return None;
        }

        if index < INLINE_STATIC_VALUE_COUNT {
            return self.inline[index].as_ref();
        }

        self.overflow.get(index - INLINE_STATIC_VALUE_COUNT)
    }

    fn set(&mut self, target_key: PlaceKey, value: StaticValue) {
        if let Some((_, existing_value)) = self
            .iter_mut()
            .find(|(existing_key, _)| existing_key == &target_key)
        {
            *existing_value = value;
            return;
        }

        if self.len < INLINE_STATIC_VALUE_COUNT {
            self.inline[self.len] = Some((target_key, value));
        } else {
            self.overflow.push((target_key, value));
        }

        self.len += 1;
    }

    fn retain_not_prefixed_by(&mut self, target_key: &PlaceKey) {
        let mut index = 0;
        while index < self.len {
            let should_remove = self
                .get_index(index)
                .is_some_and(|(existing_key, _)| existing_key.starts_with(target_key));
            if should_remove {
                self.remove(index);
            } else {
                index += 1;
            }
        }
    }

    fn remove(&mut self, index: usize) {
        debug_assert!(index < self.len);
        if index >= self.len {
            return;
        }

        if index >= INLINE_STATIC_VALUE_COUNT {
            self.overflow.remove(index - INLINE_STATIC_VALUE_COUNT);
            self.len -= 1;
            return;
        }

        if self.len <= INLINE_STATIC_VALUE_COUNT {
            for shift_index in index..self.len.saturating_sub(1) {
                self.inline[shift_index] = self.inline[shift_index + 1].take();
            }
            self.inline[self.len - 1] = None;
            self.len -= 1;
            return;
        }

        for shift_index in index..INLINE_STATIC_VALUE_COUNT - 1 {
            self.inline[shift_index] = self.inline[shift_index + 1].take();
        }
        self.inline[INLINE_STATIC_VALUE_COUNT - 1] = Some(self.overflow.remove(0));
        self.len -= 1;
    }

    fn iter(&self) -> impl Iterator<Item = &(PlaceKey, StaticValue)> {
        self.inline
            .iter()
            .take(self.len.min(INLINE_STATIC_VALUE_COUNT))
            .filter_map(Option::as_ref)
            .chain(self.overflow.iter())
    }

    fn iter_mut(&mut self) -> impl Iterator<Item = &mut (PlaceKey, StaticValue)> {
        self.inline
            .iter_mut()
            .take(self.len.min(INLINE_STATIC_VALUE_COUNT))
            .filter_map(Option::as_mut)
            .chain(self.overflow.iter_mut())
    }
}

impl Default for StaticValues {
    fn default() -> Self {
        Self::new()
    }
}

pub(crate) fn initial_static_values(
    program: &CheckedTrees,
    machine: &Machine,
    expressions: &mut ExpressionTable,
) -> StaticValues {
    let mut static_values = StaticValues::with_capacity(program.machine_owned_data(machine).len());

    for owned_data in program.machine_owned_data(machine) {
        if !owned_data.initial_value.is_valid() {
            continue;
        }

        let value = match program
            .expression_table
            .expression(owned_data.initial_value)
        {
            ExpressionNode::Integer(value) => match value.value_i64() {
                Some(value) => StaticValue::Integer(value),
                None => continue,
            },
            ExpressionNode::String(value) => StaticValue::Text(value.clone()),
            ExpressionNode::Name(path) if path.symbol.is_valid() => StaticValue::Expression(
                expressions.copy_from(&program.expression_table, owned_data.initial_value),
            ),
            _ => continue,
        };

        static_values.set(
            PlaceKey::from_symbol_name(owned_data.symbol, owned_data.name.clone()),
            value,
        );
    }

    static_values
}

pub(crate) fn apply_static_assignment(
    static_values: &mut StaticValues,
    program: &CheckedTrees,
    expressions: &mut ExpressionTable,
    assignment: TableAssignment,
) {
    let Some(target_key) = static_place_key_handle(program, assignment.target) else {
        return;
    };

    if let ExpressionNode::StructLiteral(struct_literal) =
        program.expression_table.expression(assignment.value)
    {
        for field in program
            .expression_table
            .struct_fields(struct_literal.fields)
        {
            if let Some(field_value) =
                resolve_static_value_handle(program, expressions, field.value, static_values)
            {
                set_static_value(
                    static_values,
                    target_key.append_member(field.name.clone()),
                    field_value,
                );
            }
        }
        return;
    }

    if let Some(source_key) = static_place_key_handle(program, assignment.value) {
        copy_static_prefix(static_values, &source_key, &target_key);
    }

    let Some(value) =
        resolve_static_value_handle(program, expressions, assignment.value, static_values)
    else {
        return;
    };

    set_static_value(static_values, target_key, value);
}

pub(crate) fn apply_call_static_effects(
    static_values: &mut StaticValues,
    program: &CheckedTrees,
    call: &TableCall,
) {
    for argument in program.statement_table.expression_handles(call.arguments) {
        let ExpressionNode::Mutable(target) = program.expression_table.expression(*argument) else {
            continue;
        };

        let Some(target_key) = static_place_key_handle(program, *target) else {
            continue;
        };

        invalidate_static_prefix(static_values, &target_key);
    }
}

pub(crate) fn resolve_static_value_handle(
    program: &CheckedTrees,
    expressions: &mut ExpressionTable,
    expression: ExpressionHandle,
    static_values: &StaticValues,
) -> Option<StaticValue> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Integer(value) => value.value_i64().map(StaticValue::Integer),
        ExpressionNode::String(value) => Some(StaticValue::Text(value.clone())),
        ExpressionNode::Name(path) => {
            let key = static_place_key_handle(program, expression)?;
            static_values.get(&key).or_else(|| {
                if path.symbol.is_valid() {
                    Some(StaticValue::Expression(
                        expressions.copy_from(&program.expression_table, expression),
                    ))
                } else {
                    None
                }
            })
        }
        _ => None,
    }
}

fn static_place_key_handle(
    program: &CheckedTrees,
    expression: ExpressionHandle,
) -> Option<PlaceKey> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path)
            if !program
                .expression_table
                .name_path_members(path.members)
                .is_empty() =>
        {
            PlaceKey::from_expression_handle(&program.expression_table, expression)
        }
        ExpressionNode::Indexed(_) => {
            PlaceKey::from_expression_handle(&program.expression_table, expression)
        }
        _ => None,
    }
}

fn set_static_value(static_values: &mut StaticValues, target_key: PlaceKey, value: StaticValue) {
    static_values.set(target_key, value);
}

fn copy_static_prefix(
    static_values: &mut StaticValues,
    source_key: &PlaceKey,
    target_key: &PlaceKey,
) {
    let initial_value_count = static_values.len();
    for index in 0..initial_value_count {
        let Some((existing_key, value)) = static_values.get_index(index).cloned() else {
            continue;
        };
        if !existing_key.starts_with(source_key) {
            continue;
        }

        let copied_key = existing_key.replace_prefix(source_key, target_key);
        set_static_value(static_values, copied_key, value);
    }
}

fn invalidate_static_prefix(static_values: &mut StaticValues, target_key: &PlaceKey) {
    static_values.retain_not_prefixed_by(target_key);
}

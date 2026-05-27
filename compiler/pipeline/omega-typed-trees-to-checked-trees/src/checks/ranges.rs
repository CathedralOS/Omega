use omega_core::diagnostics::Diagnostic;
use omega_core::symbols::SymbolHandle;
use omega_typed_trees::expression::{ExpressionHandle, ExpressionNode, TableRangeExpression};
use omega_typed_trees::statement::{
    StatementNode, TransitionGuardNode, TransitionTargetNode,
};
use omega_typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

pub(crate) fn check_subslice_ranges(
    program: &omega_typed_trees::TypedTrees,
) -> Result<(), Vec<Diagnostic>> {
    let field_lengths = fixed_array_field_lengths(program);
    let mut diagnostics = Vec::new();

    for machine in program.machines() {
        for state in program.machine_states(machine) {
            let mut facts = SliceLengthFacts::new(&field_lengths);
            for statement in program.statement_table.statements(state.statement_nodes) {
                check_statement(program, &mut facts, statement, &mut diagnostics);
            }
        }
    }

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

fn check_statement(
    program: &omega_typed_trees::TypedTrees,
    facts: &mut SliceLengthFacts<'_>,
    statement: &StatementNode,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match statement {
        StatementNode::Assignment(assignment) => {
            check_expression(program, facts, assignment.target, diagnostics);
            check_expression(program, facts, assignment.value, diagnostics);
        }
        StatementNode::Call(call) => {
            for argument in program.statement_table.expression_handles(call.arguments) {
                check_expression(program, facts, *argument, diagnostics);
            }
        }
        StatementNode::Expression(expression) => {
            check_expression(program, facts, *expression, diagnostics);
        }
        StatementNode::LocalData(local) => {
            check_expression(program, facts, local.initial_value, diagnostics);
            if let Some(length) = expression_slice_length(program, facts, local.initial_value) {
                facts.locals.push((local.symbol, local.name.to_string(), length));
            }
        }
        StatementNode::Transition(transition) => {
            if let TransitionGuardNode::When(guard) = transition.guard {
                check_expression(program, facts, guard, diagnostics);
            }
            check_transition_target(program, facts, transition.target, diagnostics);
            check_transition_target(program, facts, transition.continuation, diagnostics);
        }
    }
}

fn check_transition_target(
    program: &omega_typed_trees::TypedTrees,
    facts: &SliceLengthFacts<'_>,
    target: omega_typed_trees::statement::TransitionTargetHandle,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !target.is_valid() {
        return;
    }

    match program.statement_table.transition_target(target) {
        TransitionTargetNode::Named { arguments, .. } => {
            for argument in program.statement_table.expression_handles(*arguments) {
                check_expression(program, facts, *argument, diagnostics);
            }
        }
        TransitionTargetNode::Value(value) => check_expression(program, facts, *value, diagnostics),
        TransitionTargetNode::SelfTarget | TransitionTargetNode::Terminal => {}
    }
}

fn check_expression(
    program: &omega_typed_trees::TypedTrees,
    facts: &SliceLengthFacts<'_>,
    expression: ExpressionHandle,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !expression.is_valid() {
        return;
    }

    match program.expression_table.expression(expression) {
        ExpressionNode::ArrayLiteral(values) => {
            for value in program.expression_table.expression_handles(*values) {
                check_expression(program, facts, *value, diagnostics);
            }
        }
        ExpressionNode::Binary(binary) => {
            check_expression(program, facts, binary.left, diagnostics);
            check_expression(program, facts, binary.right, diagnostics);
        }
        ExpressionNode::Call(call) => {
            check_expression(program, facts, call.receiver, diagnostics);
            for argument in program.expression_table.expression_handles(call.arguments) {
                check_expression(program, facts, *argument, diagnostics);
            }
        }
        ExpressionNode::Cast(cast) => check_expression(program, facts, cast.value, diagnostics),
        ExpressionNode::Indexed(indexed) => {
            if let Some(length) = expression_slice_length(program, facts, indexed.collection) {
                check_range_index(program, indexed.index, length, diagnostics);
            }
            check_expression(program, facts, indexed.collection, diagnostics);
            check_expression(program, facts, indexed.index, diagnostics);
        }
        ExpressionNode::Member(member) => {
            check_expression(program, facts, member.receiver, diagnostics);
        }
        ExpressionNode::Mutable(inner) => check_expression(program, facts, *inner, diagnostics),
        ExpressionNode::Range(range) => {
            if range.start.is_valid() {
                check_expression(program, facts, range.start, diagnostics);
            }
            if range.end.is_valid() {
                check_expression(program, facts, range.end, diagnostics);
            }
        }
        ExpressionNode::StructLiteral(struct_literal) => {
            for field in program.expression_table.struct_fields(struct_literal.fields) {
                check_expression(program, facts, field.value, diagnostics);
            }
        }
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_) => {}
    }
}

fn check_range_index(
    program: &omega_typed_trees::TypedTrees,
    index: ExpressionHandle,
    length: usize,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let ExpressionNode::Range(range) = program.expression_table.expression(index) else {
        return;
    };
    let Some((start, end)) = literal_range_bounds(program, range) else {
        return;
    };

    let invalid = start < 0
        || end.is_some_and(|end| end < 0 || start > end)
        || usize::try_from(start).map_or(true, |start| start > length)
        || end.and_then(|end| usize::try_from(end).ok()).is_some_and(|end| end > length);

    if invalid {
        diagnostics.push(Diagnostic::error(format!(
            "cannot prove subslice range `{}` is within slice length {}",
            program.expression_table.display_name(index),
            length
        )));
    }
}

fn literal_range_bounds(
    program: &omega_typed_trees::TypedTrees,
    range: &TableRangeExpression,
) -> Option<(i64, Option<i64>)> {
    let start = if range.start.is_valid() {
        let ExpressionNode::Integer(start) = program.expression_table.expression(range.start) else {
            return None;
        };
        *start
    } else {
        0
    };
    let end = if range.end.is_valid() {
        let ExpressionNode::Integer(end) = program.expression_table.expression(range.end) else {
            return None;
        };
        Some(*end)
    } else {
        None
    };
    Some((start, end))
}

fn expression_slice_length(
    program: &omega_typed_trees::TypedTrees,
    facts: &SliceLengthFacts<'_>,
    expression: ExpressionHandle,
) -> Option<usize> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Call(call)
            if matches!(call.target.as_str(), "as_slice" | "as_mut_slice") =>
        {
            fixed_array_expression_length(program, facts, call.receiver)
        }
        ExpressionNode::Indexed(indexed) => {
            let length = expression_slice_length(program, facts, indexed.collection)?;
            range_result_length(program, indexed.index, length)
        }
        ExpressionNode::Name(path) => facts.local_length(
            path.symbol,
            program
                .expression_table
                .name_path_members(path.members)
                .last()
                .map(|name| name.as_str()),
        ),
        _ => None,
    }
}

fn range_result_length(
    program: &omega_typed_trees::TypedTrees,
    index: ExpressionHandle,
    length: usize,
) -> Option<usize> {
    let ExpressionNode::Range(range) = program.expression_table.expression(index) else {
        return None;
    };
    let (start, end) = literal_range_bounds(program, range)?;
    let start = usize::try_from(start).ok()?;
    let end = end
        .map(usize::try_from)
        .transpose()
        .ok()?
        .unwrap_or(length);
    if start > end || end > length {
        return None;
    }
    Some(end.saturating_sub(start))
}

fn fixed_array_expression_length(
    program: &omega_typed_trees::TypedTrees,
    facts: &SliceLengthFacts<'_>,
    expression: ExpressionHandle,
) -> Option<usize> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Member(member) => {
            facts.field_length(member.member_symbol, Some(member.member.as_str()))
        }
        ExpressionNode::Name(path) => facts.local_length(
            path.symbol,
            program
                .expression_table
                .name_path_members(path.members)
                .last()
                .map(|name| name.as_str()),
        ),
        _ => None,
    }
}

fn fixed_array_field_lengths(
    program: &omega_typed_trees::TypedTrees,
) -> Vec<(SymbolHandle, String, usize)> {
    let mut fields = Vec::new();
    for data in program.data_definitions() {
        for member in program.data_members(data) {
            let omega_typed_trees::data::DataMember::Field(field) = member else {
                continue;
            };
            let Some(length) = fixed_array_type_length(program, field.type_reference) else {
                continue;
            };
            fields.push((field.symbol, field.name.to_string(), length));
        }
    }
    fields
}

fn fixed_array_type_length(
    program: &omega_typed_trees::TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<usize> {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::FixedArray { length, .. } => Some(*length),
        TypeReferenceNode::Reference { referee, .. }
        | TypeReferenceNode::Constrained {
            base_type: referee, ..
        } => fixed_array_type_length(program, *referee),
        TypeReferenceNode::Generic { .. }
        | TypeReferenceNode::Named { .. }
        | TypeReferenceNode::Slice { .. }
        | TypeReferenceNode::Unit => None,
    }
}

struct SliceLengthFacts<'field> {
    fields: &'field [(SymbolHandle, String, usize)],
    locals: Vec<(SymbolHandle, String, usize)>,
}

impl<'field> SliceLengthFacts<'field> {
    fn new(fields: &'field [(SymbolHandle, String, usize)]) -> Self {
        Self {
            fields,
            locals: Vec::new(),
        }
    }

    fn field_length(&self, symbol: SymbolHandle, name: Option<&str>) -> Option<usize> {
        if let Some(length) = self
            .fields
            .iter()
            .find_map(|(field, _, length)| (*field == symbol).then_some(*length))
        {
            return Some(length);
        }

        self.fields.iter().find_map(|(_, field_name, length)| {
            name.is_some_and(|name| name == field_name)
                .then_some(*length)
        })
    }

    fn local_length(&self, symbol: SymbolHandle, name: Option<&str>) -> Option<usize> {
        if let Some(length) = self
            .locals
            .iter()
            .rev()
            .find_map(|(local, _, length)| (*local == symbol).then_some(*length))
        {
            return Some(length);
        }

        self.locals.iter().rev().find_map(|(_, local_name, length)| {
            name.is_some_and(|name| name == local_name)
                .then_some(*length)
        })
    }
}

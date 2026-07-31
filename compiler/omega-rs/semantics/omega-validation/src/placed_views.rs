//! Validation for compiler-derived placed accessors.
//!
//! Atomic source syntax lowers to the ordinary atomic carrier before symbol
//! resolution. The generated accessor remains a unique nominal type, and the
//! typed-tree placed plan retains its exact `AtomicPermissions`. This pass
//! joins those two facts: a generated accessor may participate only in an
//! atomic carrier for an operation admitted by its placement plan. Ordinary
//! assignment never becomes an atomic store implicitly.

use omega_access_plans::{AtomicPermissions, FieldAccess};
use omega_core::atomic::AtomicOrderingPlan;
use omega_core::diagnostics::Diagnostic;
use omega_typed_trees::TypedTrees;
use omega_typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use omega_typed_trees::machine::Machine;
use omega_typed_trees::state::State;
use omega_typed_trees::statement::{
    StatementNode, TableAssignment, TransitionGuardNode, TransitionTargetNode,
};

pub(crate) fn validate_statement(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    statement: &StatementNode,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let StatementNode::Assignment(assignment) = statement {
        validate_assignment(program, machine, state, assignment, diagnostics);
    }
    for expression in statement_expression_roots(program, statement) {
        validate_expression(program, machine, state, expression, false, diagnostics);
    }
}

/// Atomic mutation is authorized by the admitted atomic rule, not by an
/// ordinary exclusive-write path. The parser represents store/RMW/swap/CAS as
/// assignments, so the generic assignment validator must not demand a mutable
/// root for these exact placed operations. Permission validation remains in
/// [`validate_assignment`]; direct ordinary assignment is deliberately not
/// exempt.
pub(crate) fn assignment_is_placed_atomic_operation(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    assignment: &TableAssignment,
) -> bool {
    placed_atomic_field_for_place(program, machine, state, assignment.target).is_some()
        && matches!(
            program.expression_table.expression(assignment.value),
            ExpressionNode::Atomic(atomic)
                if !matches!(atomic.ordering, AtomicOrderingPlan::Load(_))
        )
}

fn validate_assignment(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    assignment: &TableAssignment,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(field) = placed_atomic_field_for_place(program, machine, state, assignment.target)
    else {
        return;
    };
    let ExpressionNode::Atomic(atomic) = program.expression_table.expression(assignment.value)
    else {
        diagnostics.push(Diagnostic::error(format!(
            "placed atomic field `{}` cannot be assigned directly; use one admitted atomic operation",
            field.field_name
        )));
        return;
    };
    let (operation, admitted) = match atomic.ordering {
        AtomicOrderingPlan::Store(_) => ("store", permissions(field).store),
        AtomicOrderingPlan::ReadModifyWrite(_) => {
            let operation = fetch_operation(program, atomic.value);
            let admitted = operation.is_some_and(|operation| match operation {
                "fetch_add" => permissions(field).fetch_add,
                "fetch_sub" => permissions(field).fetch_sub,
                "fetch_xor" => permissions(field).fetch_xor,
                "fetch_or" => permissions(field).fetch_or,
                "fetch_and" => permissions(field).fetch_and,
                _ => false,
            });
            (operation.unwrap_or("unknown fetch operation"), admitted)
        }
        AtomicOrderingPlan::Swap(_) => ("swap", permissions(field).swap),
        AtomicOrderingPlan::CompareExchange { .. } => {
            ("compare_exchange", permissions(field).compare_exchange)
        }
        AtomicOrderingPlan::Load(_) => ("load", false),
    };
    if !admitted {
        report_unauthorized(field, operation, diagnostics);
    }
}

fn validate_expression(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    atomic_place_allowed: bool,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !expression.is_valid() {
        return;
    }
    if !atomic_place_allowed
        && let Some(field) = placed_atomic_field_for_place(program, machine, state, expression)
    {
        diagnostics.push(Diagnostic::error(format!(
            "placed atomic field `{}` is an accessor, not an ordinary value; use one admitted atomic operation",
            field.field_name
        )));
        return;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Atomic(atomic) => {
            if matches!(atomic.ordering, AtomicOrderingPlan::Load(_)) {
                if let Some(field) =
                    placed_atomic_field_for_place(program, machine, state, atomic.value)
                    && !permissions(field).load
                {
                    report_unauthorized(field, "load", diagnostics);
                }
                validate_expression(program, machine, state, atomic.value, true, diagnostics);
            } else {
                validate_expression(program, machine, state, atomic.value, false, diagnostics);
            }
            if atomic.result.is_valid() {
                validate_expression(program, machine, state, atomic.result, false, diagnostics);
            }
        }
        ExpressionNode::ArrayLiteral(elements) => {
            for element in program.expression_table.expression_handles(*elements) {
                validate_expression(program, machine, state, *element, false, diagnostics);
            }
        }
        ExpressionNode::Binary(binary) => {
            validate_expression(program, machine, state, binary.left, false, diagnostics);
            validate_expression(program, machine, state, binary.right, false, diagnostics);
        }
        ExpressionNode::Cast(cast) => {
            validate_expression(program, machine, state, cast.value, false, diagnostics);
        }
        ExpressionNode::Call(call) => {
            if call.receiver.is_valid() {
                validate_expression(program, machine, state, call.receiver, false, diagnostics);
            }
            for argument in program.expression_table.expression_handles(call.arguments) {
                validate_expression(program, machine, state, *argument, false, diagnostics);
            }
        }
        ExpressionNode::Indexed(indexed) => {
            validate_expression(
                program,
                machine,
                state,
                indexed.collection,
                false,
                diagnostics,
            );
            validate_expression(program, machine, state, indexed.index, false, diagnostics);
        }
        ExpressionNode::Member(member) => {
            validate_expression(program, machine, state, member.receiver, false, diagnostics);
        }
        ExpressionNode::Mutable(inner) => {
            validate_expression(program, machine, state, *inner, false, diagnostics);
        }
        ExpressionNode::Unary(unary) => {
            validate_expression(program, machine, state, unary.operand, false, diagnostics);
        }
        ExpressionNode::Range(range) => {
            validate_expression(program, machine, state, range.start, false, diagnostics);
            validate_expression(program, machine, state, range.end, false, diagnostics);
        }
        ExpressionNode::StructLiteral(literal) => {
            for field in program.expression_table.struct_fields(literal.fields) {
                validate_expression(program, machine, state, field.value, false, diagnostics);
            }
        }
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => {}
    }
}

fn placed_atomic_field_for_place<'program>(
    program: &'program TypedTrees,
    machine: &Machine,
    state: &State,
    place: ExpressionHandle,
) -> Option<&'program omega_typed_trees::typed_trees::PlacedFieldPlan> {
    let type_reference = crate::places::declared_place_type(program, machine, Some(state), place)?;
    let field = program.placed_field_plan_for_type_reference(type_reference)?;
    matches!(&field.access, FieldAccess::Atomic { .. }).then_some(field)
}

fn permissions(field: &omega_typed_trees::typed_trees::PlacedFieldPlan) -> AtomicPermissions {
    match &field.access {
        FieldAccess::Atomic { operations, .. } => *operations,
        _ => AtomicPermissions::default(),
    }
}

fn fetch_operation(program: &TypedTrees, expression: ExpressionHandle) -> Option<&'static str> {
    let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) else {
        return None;
    };
    match binary.operator {
        BinaryOperator::Add => Some("fetch_add"),
        BinaryOperator::Subtract => Some("fetch_sub"),
        BinaryOperator::BitwiseXor => Some("fetch_xor"),
        BinaryOperator::BitwiseOr => Some("fetch_or"),
        BinaryOperator::BitwiseAnd => Some("fetch_and"),
        _ => None,
    }
}

fn report_unauthorized(
    field: &omega_typed_trees::typed_trees::PlacedFieldPlan,
    operation: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    diagnostics.push(Diagnostic::error(format!(
        "placed atomic field `{}` does not admit `{operation}`",
        field.field_name
    )));
}

fn statement_expression_roots(
    program: &TypedTrees,
    statement: &StatementNode,
) -> Vec<ExpressionHandle> {
    match statement {
        StatementNode::AssemblyFact(fact) => vec![fact.expression],
        StatementNode::Assignment(assignment) => vec![assignment.value],
        StatementNode::Call(call) => program
            .statement_table
            .expression_handles(call.arguments)
            .to_vec(),
        StatementNode::Expression(expression) => vec![*expression],
        StatementNode::LocalData(local) => vec![local.initial_value],
        StatementNode::Transition(transition) => {
            let mut roots = Vec::new();
            if let TransitionGuardNode::When(guard) = transition.guard {
                roots.push(guard);
            }
            for target in [transition.target, transition.continuation] {
                if !target.is_valid() {
                    continue;
                }
                match program.statement_table.transition_target(target) {
                    TransitionTargetNode::Named { arguments, .. } => roots.extend(
                        program
                            .statement_table
                            .expression_handles(*arguments)
                            .iter()
                            .copied(),
                    ),
                    TransitionTargetNode::Value(value) => roots.push(*value),
                    TransitionTargetNode::SelfTarget | TransitionTargetNode::Terminal => {}
                }
            }
            roots
        }
    }
}

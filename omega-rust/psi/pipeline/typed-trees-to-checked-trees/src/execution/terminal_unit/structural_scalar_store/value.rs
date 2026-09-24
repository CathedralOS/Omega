//! Resolve what one store writes: an already-checked scalar source, selected
//! independently of where it is stored.
//!
//! The store vocabulary asks for an "already-defined, exactly typed SSA value"
//! (wiki/spec/terminal-psi/structural_access.md, Store vocabulary). Checking
//! records that value in exactly one of three places: a scalar computation
//! rooted at the value's own role coordinate, a bound pure scalar expression,
//! or -- when the right-hand side is the call this same statement performs --
//! the call's result in the dense scalar namespace.
use super::super::{
    CheckFacts, CheckedScalarExpression, CheckedScalarExpressionRole, ExpressionNode,
    PrimitiveType, TypedTrees,
};
use super::destination::Root;
use crate::execution::terminal_unit::control::LocalConstructionTrace;

/// Where the assignment's right-hand side was evaluated.
#[derive(Clone, Copy)]
pub(super) enum AssignmentSource {
    /// An authored expression with its own checked value facts.
    Authored,
    /// The SSA result of the scalar call this same statement performs, by its
    /// dense scalar-namespace position and declared result type.
    CallResult {
        position: u32,
        primitive_type: PrimitiveType,
    },
}

/// Select the scalar value an authored assignment stores into a leaf of
/// `primitive_type`, then apply the root lane's value policy.
pub(super) fn assignment_value(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    root: &Root<'_>,
    statement_index: u32,
    assignment: &typed_trees::statement::TableAssignment,
    source: AssignmentSource,
    primitive_type: PrimitiveType,
    trace: &LocalConstructionTrace,
) -> Option<checked_trees::CheckedStructuralScalarFieldStoreValue> {
    let value = match source {
        AssignmentSource::CallResult {
            position,
            primitive_type: result_type,
        } => {
            trace.phase("structural field store: pure source: call result: result type");
            if result_type != primitive_type {
                return None;
            }
            trace.phase("structural field store: pure source: call result: authored call");
            if !matches!(
                program.expression_table.expression(assignment.value),
                ExpressionNode::Call(_)
            ) {
                return None;
            }
            checked_trees::CheckedStructuralScalarFieldStoreValue::ScalarResult { position }
        }
        AssignmentSource::Authored => authored_value(
            facts,
            machine,
            state,
            statement_index,
            CheckedScalarExpressionRole::AssignmentValue,
            assignment.value,
            primitive_type,
            trace,
        )?,
    };
    trace.phase("structural field store: pure source: value shape");
    admits(root, primitive_type, &value).then_some(value)
}

/// The checked value at one role coordinate: a computation rooted there, or
/// otherwise the bound pure expression. A computation that exists but belongs
/// to a different machine, authored expression or carrier is a custody
/// mismatch, not permission to fall back to another source.
pub(super) fn authored_value(
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: u32,
    role: CheckedScalarExpressionRole,
    authored: typed_trees::expression::ExpressionHandle,
    primitive_type: PrimitiveType,
    trace: &LocalConstructionTrace,
) -> Option<checked_trees::CheckedStructuralScalarFieldStoreValue> {
    let computations = &facts.values.scalar_computations;
    if let Some(root) = computations.root_at(state.symbol, statement_index, role) {
        trace.phase("structural field store: computation source");
        if root.machine != machine.symbol
            || !computations.nodes.is_valid(root.root)
            || computations.nodes.get(root.root).authored_root != authored
            || computations.nodes.get(root.root).primitive_type != primitive_type
            || facts
                .values
                .scalar_expressions
                .expression_at(state.symbol, statement_index, role)
                .is_some()
        {
            return None;
        }
        return Some(checked_trees::CheckedStructuralScalarFieldStoreValue::Computation(root.root));
    }
    trace.phase("structural field store: pure source: scalar expression row");
    let (binding, value) =
        facts
            .values
            .scalar_expressions
            .bound_expression_at(state.symbol, statement_index, role)?;
    trace.phase("structural field store: pure source: value shape");
    if binding.expression != authored
        || crate::values::scalar_expression_type(value) != Some(primitive_type)
    {
        return None;
    }
    Some(checked_trees::CheckedStructuralScalarFieldStoreValue::Pure(
        value.clone(),
    ))
}

/// One literal record member's value: the computation rooted at the
/// assignment's own `RecordField` coordinate. A root borrowed from another
/// role would let the store consume a computation the assignment never
/// authored.
pub(super) fn record_field_value(
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: u32,
    literal: typed_trees::expression::ExpressionHandle,
    field_ordinal: u32,
    field: &checked_trees::CheckedStructuralRecordField,
    primitive_type: PrimitiveType,
) -> Option<checked_trees::CheckedStructuralScalarFieldStoreValue> {
    let checked_trees::CheckedStructuralRecordFieldValue::Scalar(root) = field.value else {
        return None;
    };
    facts
        .values
        .scalar_computations
        .roots
        .iter()
        .map(|(_, row)| row)
        .find(|row| {
            row.machine == machine.symbol
                && row.state == state.symbol
                && row.statement_ordinal == statement_index
                && row.root == root
                && matches!(row.role,
                    CheckedScalarExpressionRole::RecordField { expression, field_ordinal: ordinal }
                        if expression == literal && ordinal == field_ordinal)
        })?;
    let node = facts.values.scalar_computations.nodes.get(root);
    (node.authored_root == field.expression && node.primitive_type == primitive_type)
        .then_some(checked_trees::CheckedStructuralScalarFieldStoreValue::Computation(root))
}

/// Floating stores forward existing bits: an authored IEEE literal or an
/// already-defined scalar (parameter or local). A floating operation in a
/// pure expression has no selected provider, so it is refused. The
/// scalar-graph lane evaluates only those already-defined sources, so it also
/// refuses a floating computation; the attached lane's computation retains
/// its own selected operations and call correspondence.
fn admits(
    root: &Root<'_>,
    primitive_type: PrimitiveType,
    value: &checked_trees::CheckedStructuralScalarFieldStoreValue,
) -> bool {
    if !matches!(primitive_type, PrimitiveType::F32 | PrimitiveType::F64) {
        return true;
    }
    match value {
        checked_trees::CheckedStructuralScalarFieldStoreValue::Pure(expression) => matches!(
            expression,
            CheckedScalarExpression::IeeeFloatLiteral { .. }
                | CheckedScalarExpression::Parameter { .. }
                | CheckedScalarExpression::Local { .. }
        ),
        checked_trees::CheckedStructuralScalarFieldStoreValue::Computation(_) => {
            matches!(root, Root::Parameter { .. })
        }
        checked_trees::CheckedStructuralScalarFieldStoreValue::ScalarResult { .. } => true,
    }
}

/// The retained runtime selector of an indexed store: the bound
/// `AssignmentIndex` expression of exactly this authored index, of an integer
/// carrier.
pub(super) fn runtime_index(
    facts: &CheckFacts,
    state: &typed_trees::state::State,
    statement_index: u32,
    index: typed_trees::expression::ExpressionHandle,
) -> Option<CheckedScalarExpression> {
    let (binding, value) = facts.values.scalar_expressions.bound_expression_at(
        state.symbol,
        statement_index,
        CheckedScalarExpressionRole::AssignmentIndex,
    )?;
    (binding.expression == index
        && matches!(
            crate::values::scalar_expression_type(value),
            Some(
                PrimitiveType::I8
                    | PrimitiveType::I16
                    | PrimitiveType::I32
                    | PrimitiveType::I64
                    | PrimitiveType::U8
                    | PrimitiveType::U16
                    | PrimitiveType::U32
                    | PrimitiveType::U64
            )
        ))
    .then(|| value.clone())
}

/// The `u8` a byte store writes: the same scalar source selection, without a
/// computation (byte stores evaluate only pure values and call results).
pub(super) fn byte_value(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    root: &Root<'_>,
    statement_index: u32,
    assignment: &typed_trees::statement::TableAssignment,
    source: AssignmentSource,
    trace: &LocalConstructionTrace,
) -> Option<checked_trees::CheckedByteSequenceStoreValue> {
    match assignment_value(
        program,
        facts,
        machine,
        state,
        root,
        statement_index,
        assignment,
        source,
        PrimitiveType::U8,
        trace,
    )? {
        checked_trees::CheckedStructuralScalarFieldStoreValue::Pure(value) => {
            Some(checked_trees::CheckedByteSequenceStoreValue::Pure(value))
        }
        checked_trees::CheckedStructuralScalarFieldStoreValue::ScalarResult { position } => {
            Some(checked_trees::CheckedByteSequenceStoreValue::ScalarResult { position })
        }
        checked_trees::CheckedStructuralScalarFieldStoreValue::Computation(_) => None,
    }
}

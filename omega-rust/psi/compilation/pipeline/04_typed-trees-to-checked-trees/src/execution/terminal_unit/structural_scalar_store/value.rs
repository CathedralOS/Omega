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
    CheckFacts, CheckedScalarExpression, CheckedScalarExpressionRole, ExpressionNode, PrimitiveType,
};
use crate::execution::terminal_unit::control::LocalConstructionTrace;

/// Where the assignment's right-hand side was evaluated.
#[derive(Clone, Copy)]
pub(in crate::execution::terminal_unit) enum AssignmentSource {
    /// An authored expression with its own checked value facts.
    Authored,
    /// The completed, exactly typed structural value of this assignment.
    StructuralValue {
        binding_ordinal: u32,
        result_type: typed_trees::types::TypeReferenceHandle,
    },
    /// The SSA result of the scalar call this same statement performs, by its
    /// dense scalar-namespace position and declared result type.
    CallResult {
        position: u32,
        primitive_type: PrimitiveType,
    },
}

/// Select the scalar value an authored assignment stores into a leaf of
/// `primitive_type`, then apply the value policy.
pub(super) fn assignment_value(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: u32,
    assignment: &typed_trees::statement::TableAssignment,
    source: AssignmentSource,
    primitive_type: PrimitiveType,
    trace: &LocalConstructionTrace,
) -> Option<checked_trees::CheckedScalarStoreValue> {
    let value = match source {
        AssignmentSource::StructuralValue { .. } => return None,
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
            checked_trees::CheckedScalarStoreValue::ScalarResult { position }
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
    admits(primitive_type, &value).then_some(value)
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
) -> Option<checked_trees::CheckedScalarStoreValue> {
    let computations = &facts.values.scalar_computations;
    if let Some(root) = computations.root_at(state.symbol, statement_index, role) {
        trace.phase("structural field store: computation source");
        if !computations.is_authored_root(root, machine.symbol, authored, primitive_type)
            || facts
                .values
                .scalar_expressions
                .expression_at(state.symbol, statement_index, role)
                .is_some()
        {
            return None;
        }
        return Some(checked_trees::CheckedScalarStoreValue::Computation(
            root.root,
        ));
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
    Some(checked_trees::CheckedScalarStoreValue::Pure(value.clone()))
}

/// Floating stores forward existing bits: an authored IEEE literal or an
/// already-defined scalar (parameter, local, or a leaf read out of existing
/// structural storage). A floating operation in a pure expression has no
/// selected provider, so it is refused. A checked computation retains its own
/// selected operations and call correspondence regardless of whether the
/// destination is borrowed parameter storage or an owned local. Destination
/// authority is checked independently by the store planner.
fn admits(primitive_type: PrimitiveType, value: &checked_trees::CheckedScalarStoreValue) -> bool {
    if !matches!(primitive_type, PrimitiveType::F32 | PrimitiveType::F64) {
        return true;
    }
    match value {
        checked_trees::CheckedScalarStoreValue::Pure(expression) => matches!(
            expression,
            CheckedScalarExpression::IeeeFloatLiteral { .. }
                | CheckedScalarExpression::Parameter { .. }
                | CheckedScalarExpression::Local { .. }
                | CheckedScalarExpression::StructuralParameterField { .. }
                | CheckedScalarExpression::StructuralParameterIndexedRead { .. }
        ),
        checked_trees::CheckedScalarStoreValue::Computation(_) => true,
        checked_trees::CheckedScalarStoreValue::ScalarResult { .. } => true,
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
        CheckedScalarExpressionRole::AssignmentIndex { depth: 0 },
    )?;
    (binding.expression == index
        && crate::values::scalar_expression_type(value)
            .is_some_and(|primitive| primitive.is_integer()))
    .then(|| value.clone())
}

/// The `u8` a byte store writes: the same scalar source selection as every
/// other store, including a computation that owns the value's calls.
pub(super) fn byte_value(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: u32,
    assignment: &typed_trees::statement::TableAssignment,
    source: AssignmentSource,
    trace: &LocalConstructionTrace,
) -> Option<checked_trees::CheckedScalarStoreValue> {
    assignment_value(
        program,
        facts,
        machine,
        state,
        statement_index,
        assignment,
        source,
        PrimitiveType::U8,
        trace,
    )
}

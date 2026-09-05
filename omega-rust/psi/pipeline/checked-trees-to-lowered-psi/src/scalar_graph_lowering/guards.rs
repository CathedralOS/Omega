//! Evaluate the selected guard once before dispatching either branch.

use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn lower(
    checked: &CheckedTrees,
    state: symbols::SymbolHandle,
    statement: u32,
    bindings: &storage::ScalarBindings,
    source_types: &[ScalarType],
    when_true: (usize, Vec<LoweredDirectExpression>),
    when_false: (usize, Vec<LoweredDirectExpression>),
    fallback: &CheckedScalarBranchDestination,
    computations: &mut computations::Expansion<'_>,
) -> Result<LoweredScalarBranchTerminator, LoweringError> {
    let computed = checked
        .facts
        .values
        .scalar_computations
        .roots
        .iter()
        .any(|(_, root)| {
            root.state == state
                && root.statement_ordinal == statement
                && root.role == CheckedScalarExpressionRole::Guard
        });
    if computed || matches!(fallback, CheckedScalarBranchDestination::Crash { .. }) {
        validate_fallback(checked, state, statement, fallback)?;
    }
    let condition = if computed {
        LoweredBooleanReturnExpression::Parameter {
            position: source_types.len(),
        }
    } else {
        let LoweredDirectExpression::Boolean { expression } = bindings.expression_at(
            checked,
            state,
            statement,
            CheckedScalarExpressionRole::Guard,
        )?
        else {
            return unsupported("checked scalar graph guard must be Boolean");
        };
        validate_short_circuit_expression(&expression)?;
        validate_boolean_parameter_types(&expression, source_types)?;
        *expression
    };
    let branch = LoweredScalarBranchTerminator::Conditional {
        condition,
        when_true_target: when_true.0,
        when_true_arguments: when_true.1,
        when_false_target: when_false.0,
        when_false_arguments: when_false.1,
    };
    if !computed {
        return Ok(branch);
    }
    // Branch arguments still refer to the unchanged source prefix. Only the
    // condition consumes the appended result; neither arm starts before it.
    let mut parameter_types = source_types.to_vec();
    parameter_types.push(ScalarType::Boolean);
    let target = computations.push(LoweredScalarBranchState {
        parameter_types,
        bindings: Vec::new(),
        terminator: branch,
    });
    let target = computations.retained_value(
        state,
        statement,
        CheckedScalarExpressionRole::Guard,
        symbols::SymbolHandle::invalid(),
        bindings,
        source_types,
        ScalarType::Boolean,
        target,
    )?;
    Ok(LoweredScalarBranchTerminator::Jump {
        target,
        arguments: computations::parameters(source_types),
    })
}

fn validate_fallback(
    checked: &CheckedTrees,
    state: symbols::SymbolHandle,
    statement: u32,
    fallback: &CheckedScalarBranchDestination,
) -> Result<(), LoweringError> {
    use checked_trees::statement::{StatementNode, TransitionExit, TransitionGuardNode};

    let program = &checked.typed;
    let state = program
        .machines()
        .iter()
        .flat_map(|machine| program.machine_states(machine))
        .find(|candidate| candidate.symbol == state)
        .ok_or(LoweringError::Unsupported(
            "scalar guard has no authored state",
        ))?;
    let tail = program
        .statement_table
        .statements(state.statement_nodes)
        .get(statement as usize..)
        .ok_or(LoweringError::Unsupported(
            "scalar guard has no authored dispatch",
        ))?;
    if !matches!(
        tail.first(),
        Some(StatementNode::Transition(guard))
            if matches!(guard.guard, TransitionGuardNode::When(_))
    ) {
        return unsupported("scalar conditional must retain its authored When guard");
    }
    let continuation = match fallback {
        CheckedScalarBranchDestination::Jump(successor) => successor.is_continuation,
        CheckedScalarBranchDestination::Return {
            is_continuation, ..
        } => *is_continuation,
        CheckedScalarBranchDestination::Crash { .. } => false,
    };
    let crash_fallback = matches!(fallback, CheckedScalarBranchDestination::Crash { .. });
    let valid = match tail {
        [StatementNode::Transition(guard)] if continuation => {
            !crash_fallback
                && guard.continuation.is_valid()
                && guard.exit == TransitionExit::Ordinary
        }
        [
            StatementNode::Transition(guard),
            StatementNode::Transition(fallback),
        ] if !continuation => {
            !guard.continuation.is_valid()
                && guard.exit == TransitionExit::Ordinary
                && !fallback.continuation.is_valid()
                && (if crash_fallback {
                    matches!(fallback.exit, TransitionExit::Crash(_))
                } else {
                    fallback.exit == TransitionExit::Ordinary
                })
                && fallback.guard == TransitionGuardNode::Always
        }
        [
            StatementNode::Transition(guard),
            StatementNode::Expression(_),
        ] if !continuation => {
            !crash_fallback
                && !guard.continuation.is_valid()
                && guard.exit == TransitionExit::Ordinary
        }
        _ => false,
    };
    if !valid {
        return unsupported("scalar guard fallback disagrees with its authored dispatch");
    }
    Ok(())
}

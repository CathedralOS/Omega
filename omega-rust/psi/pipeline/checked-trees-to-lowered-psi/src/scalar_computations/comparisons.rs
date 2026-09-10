//! One selected comparison emitter for authored binary uses and saved Match operands.
//!
//! Both callers supply completed operands in the scalar prefix. Match keeps its
//! subject across failed arms; neither path re-evaluates source expressions here.
//! Exact selected requirement/application custody remains separate from Omega's
//! provider authority. Proof-only float equality never supplies executable meaning.
use super::*;

#[cfg(test)]
mod tests;

/// Checked occurrence before emission assigns real Terminal identities.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SelectedComparison {
    pub operator_use: checked_trees::CheckedOperatorUseHandle,
    pub application_site: checked_trees::CheckedBoundaryOperatorApplicationUseSite,
    pub requirement_operator: symbols::SymbolHandle,
    pub provider_plan_report_fingerprint: u64,
    pub provider_plan_commitment: checked_trees::CheckedProviderPlanCommitment,
    pub comparison: semantic_vocabulary::IeeeFloatComparisonOperation,
    pub format: semantic_vocabulary::IeeeFloatFormat,
}

pub(crate) fn occurrence(
    checked: &CheckedTrees,
    handle: checked_trees::CheckedOperatorUseHandle,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    statement: u32,
) -> Result<SelectedComparison, LoweringError> {
    let (comparison, primitive) = checked
        .facts
        .operators
        .selected_float_comparison(&checked.typed, handle)
        .ok_or(LoweringError::Unsupported(
            "comparison has no exact selected IEEE meaning",
        ))?;
    let selected = checked.facts.operators.uses.get(handle);
    validate_origin(checked, selected)?;
    if !matches!(selected.origin, checked_trees::CheckedValueOrigin::StateStatement { machine_symbol, state_symbol, statement_index, .. }
        if machine_symbol == machine && state_symbol == state && usize::try_from(statement).ok() == Some(statement_index))
        || selected.provider_plan_commitment.is_empty()
        || selected.provider_plan_report_fingerprint == 0
        || checked
            .facts
            .operators
            .boundary_applications
            .iter()
            .filter(|application| {
                application.site == selected.application_site()
                    && application.requirement_symbol == selected.selected_operator_symbol
            })
            .count()
            != 1
    {
        return unsupported(
            "selected comparison lost its exact source owner or provider application",
        );
    }
    Ok(SelectedComparison {
        operator_use: handle,
        application_site: selected.application_site(),
        requirement_operator: selected.selected_operator_symbol,
        provider_plan_report_fingerprint: selected.provider_plan_report_fingerprint,
        provider_plan_commitment: selected.provider_plan_commitment,
        comparison,
        format: match primitive {
            PrimitiveType::F32 => semantic_vocabulary::IeeeFloatFormat::Binary32,
            PrimitiveType::F64 => semantic_vocabulary::IeeeFloatFormat::Binary64,
            _ => return unsupported("selected comparison has a non-float operand"),
        },
    })
}

fn validate_origin(
    checked: &CheckedTrees,
    selected: &checked_trees::CheckedOperatorUseFact,
) -> Result<(), LoweringError> {
    use checked_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};
    use checked_trees::{CheckedValueOrigin, CheckedValueStatementRole as Role};
    let CheckedValueOrigin::StateStatement {
        machine_symbol,
        state_symbol,
        statement_index,
        role,
    } = selected.origin
    else {
        return unsupported("comparison has no executable statement origin");
    };
    let program = &checked.typed;
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol)
        .ok_or(LoweringError::Unsupported(
            "comparison owner machine is absent",
        ))?;
    let state = program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == state_symbol)
        .ok_or(LoweringError::Unsupported(
            "comparison owner state is absent",
        ))?;
    let statement = program
        .statement_table
        .statements(state.statement_nodes)
        .get(statement_index)
        .ok_or(LoweringError::Unsupported(
            "comparison owner statement is absent",
        ))?;
    let mut roots = Vec::new();
    match (statement, role) {
        (StatementNode::Expression(expression), Role::Expression) => roots.push(*expression),
        (StatementNode::LocalData(local), Role::LocalInitializer) => {
            roots.push(local.initial_value)
        }
        (StatementNode::Assignment(assignment), Role::AssignmentValue) => {
            roots.push(assignment.value)
        }
        (StatementNode::Assignment(assignment), Role::AssignmentTargetSubexpression) => {
            roots.push(assignment.target)
        }
        (StatementNode::Call(call), Role::CallArgument) => {
            roots.extend_from_slice(program.statement_table.expression_handles(call.arguments))
        }
        (StatementNode::Transition(transition), Role::TransitionGuard) => {
            if let TransitionGuardNode::When(expression) = transition.guard {
                roots.push(expression);
            }
        }
        (
            StatementNode::Transition(transition),
            Role::TransitionTargetArgument | Role::TransitionTargetValue,
        ) => {
            for target in [transition.target, transition.continuation] {
                match (program.statement_table.transition_target(target), role) {
                    (TransitionTargetNode::Value(expression), Role::TransitionTargetValue) => {
                        roots.push(*expression)
                    }
                    (
                        TransitionTargetNode::Named { arguments, .. },
                        Role::TransitionTargetArgument,
                    ) => roots
                        .extend_from_slice(program.statement_table.expression_handles(*arguments)),
                    _ => {}
                }
            }
        }
        _ => {}
    }
    for root in roots {
        if crate::scalar_source_custody::computation_calls::authored_expressions(checked, root)?
            .contains(&selected.expression)
        {
            return Ok(());
        }
    }
    unsupported("comparison expression escaped its exact authored origin role")
}

impl Expansion<'_> {
    pub(super) fn comparison_binding(
        &self,
        operator_use: checked_trees::CheckedOperatorUseHandle,
        left: LoweredDirectExpression,
        right: LoweredDirectExpression,
        site: &Site<'_>,
    ) -> Result<LoweredScalarBinding, LoweringError> {
        let occurrence = occurrence(
            self.checked,
            operator_use,
            self.machine,
            site.state,
            site.statement,
        )?;
        let expected = ScalarType::IeeeFloat(occurrence.format);
        if left.scalar_type() != expected || right.scalar_type() != expected {
            return unsupported("selected comparison operand format changed");
        }
        Ok(LoweredScalarBinding::SelectedComparison {
            occurrence,
            source_machine: self.machine,
            left,
            right,
        })
    }
}

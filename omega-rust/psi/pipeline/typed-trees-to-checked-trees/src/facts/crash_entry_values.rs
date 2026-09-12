//! Relate saved crash operands to immutable invocation-entry values.
//!
//! Ordinary calls and selected operators share this substitution boundary.
//! A stable binding is insufficient when its contents contain mutable loans or
//! interior authority. Reuse stable-observation validation before projecting
//! fields; shared loans may retain immutable contents without owning them.
//! Mutable bindings, unknown contents and state re-entry retain no entry identity.
//! Substitution transports a proven origin, never re-reads an initializer after
//! later operands execute. This is source provenance, not a Terminal certificate.

use checked_trees::CrashPredicateExpression;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use validation::has_stable_observable_contents;

#[cfg(test)]
mod tests;

pub(super) fn entry_operand(
    program: &TypedTrees,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    before_statement: usize,
    expression: ExpressionHandle,
) -> Option<CrashPredicateExpression> {
    if !program.expression_table.expression_is_valid(expression) {
        return None;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Boolean(value) => Some(CrashPredicateExpression::Boolean(*value)),
        ExpressionNode::Integer(value) => {
            Some(CrashPredicateExpression::Integer(value.text().to_owned()))
        }
        ExpressionNode::Unary(unary)
            if unary.operator == typed_trees::expression::UnaryOperator::LogicalNot =>
        {
            Some(CrashPredicateExpression::Unary {
                operator: unary.operator as u8,
                operand: Box::new(entry_operand(
                    program,
                    machine_symbol,
                    state_symbol,
                    before_statement,
                    unary.operand,
                )?),
            })
        }
        ExpressionNode::Member(member)
            if member.member_symbol.is_valid() && member.case_variant.is_none() =>
        {
            Some(CrashPredicateExpression::Member {
                receiver: Box::new(entry_operand(
                    program,
                    machine_symbol,
                    state_symbol,
                    before_statement,
                    member.receiver,
                )?),
                member: member.member.as_str().to_owned(),
            })
        }
        ExpressionNode::Name(path) => {
            if program
                .expression_table
                .name_path_members(path.members)
                .len()
                != 1
                || !path.symbol.is_valid()
                || path.head_symbol != path.symbol
            {
                return None;
            }
            let machine = program
                .machines()
                .iter()
                .find(|machine| machine.symbol == machine_symbol)?;
            let state = program
                .machine_states(machine)
                .iter()
                .find(|state| state.symbol == state_symbol)?;
            let preceding = program
                .statement_table
                .statements(state.statement_nodes)
                .get(..before_statement)?;
            for (ordinal, statement) in preceding.iter().enumerate() {
                if let typed_trees::statement::StatementNode::LocalData(local) = statement
                    && local.symbol == path.symbol
                {
                    if local.is_mutable
                        || !has_stable_observable_contents(program, local.type_reference)
                    {
                        return None;
                    }
                    // This transports a fixed value, not a current read
                    // of its initializer. Every dependency must independently
                    // be immutable and entry-relative; mutable initializers
                    // are rejected even if their storage now has useful facts.
                    // Decreasing the prefix also prevents recursive aliases.
                    return entry_operand(
                        program,
                        machine_symbol,
                        state_symbol,
                        ordinal,
                        local.initial_value,
                    );
                }
            }
            let entry_index = crate::checks::termination::named_transition_target_state_index(
                program,
                machine,
                machine.symbol,
            )?;
            let entry = program.machine_states(machine).get(entry_index)?;
            if state_symbol != entry.symbol
                || entry_has_incoming_transition(program, machine, entry_index)
            {
                return None;
            }
            let (ordinal, _) =
                program
                    .state_parameters(entry)
                    .iter()
                    .enumerate()
                    .find(|(_, parameter)| {
                        parameter.symbol == path.symbol
                            && !parameter.is_mutable
                            && !parameter.is_self
                            && has_stable_observable_contents(program, parameter.type_reference)
                    })?;
            Some(CrashPredicateExpression::Parameter(
                u32::try_from(ordinal).ok()?,
            ))
        }
        _ => None,
    }
}

fn entry_has_incoming_transition(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    entry_index: usize,
) -> bool {
    use typed_trees::statement::{StatementNode, TransitionTargetNode};

    // Typed statement lowering flattens nested transition forms into states.
    // Inspect ordinary targets and continuation targets, just as the existing
    // termination graph does. A repeated immutable declaration is a fresh
    // arrival value, not necessarily the original invocation-entry value.
    program.machine_states(machine).iter().any(|state| {
        program
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .any(|statement| {
                let StatementNode::Transition(transition) = statement else {
                    return false;
                };
                [transition.target, transition.continuation]
                    .into_iter()
                    .filter(|target| target.is_valid())
                    .any(|target| {
                        let symbol = match program.statement_table.transition_target(target) {
                            TransitionTargetNode::Named { path, .. } => path.symbol,
                            TransitionTargetNode::SelfTarget => state.symbol,
                            TransitionTargetNode::Value(_) | TransitionTargetNode::Terminal => {
                                return false;
                            }
                        };
                        crate::checks::termination::named_transition_target_state_index(
                            program, machine, symbol,
                        ) == Some(entry_index)
                    })
            })
    })
}

pub(super) fn substitute_entry(
    expression: &CrashPredicateExpression,
    operands: &[Option<CrashPredicateExpression>],
) -> Option<CrashPredicateExpression> {
    Some(match expression {
        CrashPredicateExpression::Parameter(ordinal) => operands.get(*ordinal as usize)?.clone()?,
        CrashPredicateExpression::Boolean(_) | CrashPredicateExpression::Integer(_) => {
            expression.clone()
        }
        CrashPredicateExpression::Binary {
            operator,
            left,
            right,
        } => CrashPredicateExpression::Binary {
            operator: *operator,
            left: Box::new(substitute_entry(left, operands)?),
            right: Box::new(substitute_entry(right, operands)?),
        },
        CrashPredicateExpression::Unary { operator, operand } => CrashPredicateExpression::Unary {
            operator: *operator,
            operand: Box::new(substitute_entry(operand, operands)?),
        },
        CrashPredicateExpression::Member { receiver, member } => CrashPredicateExpression::Member {
            receiver: Box::new(substitute_entry(receiver, operands)?),
            member: member.clone(),
        },
        _ => return None,
    })
}

//! Relate saved crash operands to immutable invocation-entry values.
//!
//! Ordinary calls and selected operators share this substitution boundary.
//! A stable binding is insufficient when its contents contain mutable loans or
//! interior authority. Reuse stable-observation validation before projecting
//! fields; shared loans may retain immutable contents without owning them.
//! Immutable state parameters resolve through every arrival that binds them:
//! the invocation itself for the entry state, plus each named transition edge
//! into the state, which must all produce the same entry-relative operand.
//! Mutable bindings, unknown contents, divergent arrivals and unresolvable
//! cycles retain no entry identity. Substitution transports a proven origin,
//! never re-reads an initializer after later operands execute. This is source
//! provenance, not a Terminal certificate.

use checked_trees::CrashPredicateExpression;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::statement::{StatementNode, TransitionTargetNode};
use validation::has_stable_observable_contents;

/// Arrival provenance can revisit a state parameter through a transition
/// cycle, so the fold carries a depth bound. Exhaustion is unproven
/// provenance, never an affirmed origin.
const MAX_ENTRY_PROVENANCE_DEPTH: u32 = 64;

#[cfg(test)]
mod tests;

pub(super) fn entry_operand(
    program: &TypedTrees,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    before_statement: usize,
    expression: ExpressionHandle,
) -> Option<CrashPredicateExpression> {
    entry_operand_at(
        program,
        machine_symbol,
        state_symbol,
        before_statement,
        expression,
        0,
    )
}

fn entry_operand_at(
    program: &TypedTrees,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    before_statement: usize,
    expression: ExpressionHandle,
    depth: u32,
) -> Option<CrashPredicateExpression> {
    if depth >= MAX_ENTRY_PROVENANCE_DEPTH
        || !program.expression_table.expression_is_valid(expression)
    {
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
                operand: Box::new(entry_operand_at(
                    program,
                    machine_symbol,
                    state_symbol,
                    before_statement,
                    unary.operand,
                    depth + 1,
                )?),
            })
        }
        ExpressionNode::Member(member)
            if member.member_symbol.is_valid() && member.case_variant.is_none() =>
        {
            Some(CrashPredicateExpression::Member {
                receiver: Box::new(entry_operand_at(
                    program,
                    machine_symbol,
                    state_symbol,
                    before_statement,
                    member.receiver,
                    depth + 1,
                )?),
                member: member.member.as_str().to_owned(),
            })
        }
        ExpressionNode::Binary(binary)
            if matches!(
                binary.operator,
                typed_trees::expression::BinaryOperator::Add
                    | typed_trees::expression::BinaryOperator::Subtract
                    | typed_trees::expression::BinaryOperator::Multiply
                    | typed_trees::expression::BinaryOperator::Divide
                    | typed_trees::expression::BinaryOperator::Modulo
                    | typed_trees::expression::BinaryOperator::BitwiseAnd
                    | typed_trees::expression::BinaryOperator::BitwiseOr
                    | typed_trees::expression::BinaryOperator::BitwiseXor
                    | typed_trees::expression::BinaryOperator::ShiftLeft
                    | typed_trees::expression::BinaryOperator::ShiftRight
            ) || (matches!(
                binary.operator,
                typed_trees::expression::BinaryOperator::And
                    | typed_trees::expression::BinaryOperator::Or
                    | typed_trees::expression::BinaryOperator::Equal
                    | typed_trees::expression::BinaryOperator::NotEqual
                    | typed_trees::expression::BinaryOperator::Less
                    | typed_trees::expression::BinaryOperator::LessOrEqual
                    | typed_trees::expression::BinaryOperator::Greater
                    | typed_trees::expression::BinaryOperator::GreaterOrEqual
            ) && builtin_binary_meaning(
                program,
                machine_symbol,
                state_symbol,
                expression,
            )) =>
        {
            // Only value-producing arithmetic crosses this boundary
            // unconditionally. The domain-free predicate reducers can never
            // fold these operators, so transporting them cannot substitute
            // builtin meaning for a caller-authored one; the checked scalar
            // channel remains the only evaluation authority over the
            // substituted expression. Comparisons and logical connectives are
            // decidable by those same reducers, so they transport only when
            // this occurrence already selected builtin meaning — a custom
            // operator spelled like one would otherwise be folded under laws
            // its selection rejected.
            Some(CrashPredicateExpression::Binary {
                operator: binary.operator as u8,
                left: Box::new(entry_operand_at(
                    program,
                    machine_symbol,
                    state_symbol,
                    before_statement,
                    binary.left,
                    depth + 1,
                )?),
                right: Box::new(entry_operand_at(
                    program,
                    machine_symbol,
                    state_symbol,
                    before_statement,
                    binary.right,
                    depth + 1,
                )?),
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
                    return entry_operand_at(
                        program,
                        machine_symbol,
                        state_symbol,
                        ordinal,
                        local.initial_value,
                        depth + 1,
                    );
                }
            }
            state_parameter_entry_operand(
                program,
                machine,
                machine_symbol,
                state_symbol,
                path.symbol,
                depth,
            )
        }
        _ => None,
    }
}

fn builtin_binary_meaning(
    program: &TypedTrees,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    expression: ExpressionHandle,
) -> bool {
    let Some(machine) = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol)
    else {
        return false;
    };
    let state = program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == state_symbol);
    validation::has_builtin_binary_expression_meaning(program, machine, state, expression)
}

/// A state parameter's saved actual is whatever every arrival binds to it:
/// the invocation for the entry state, and each named transition edge into
/// the state positionally. `-> self` forwards the current values and adds no
/// new arrival; a by-name edge forwarding this same parameter back to its own
/// state is tautological for the same reason. Every remaining edge must
/// resolve to one identical entry-relative operand, or provenance stays
/// unknown rather than picking a winner.
fn state_parameter_entry_operand(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    parameter_symbol: SymbolHandle,
    depth: u32,
) -> Option<CrashPredicateExpression> {
    let states = program.machine_states(machine);
    let state_index = states
        .iter()
        .position(|state| state.symbol == state_symbol)?;
    let parameters = program.state_parameters(&states[state_index]);
    let (parameter_ordinal, parameter) = parameters
        .iter()
        .enumerate()
        .find(|(_, parameter)| parameter.symbol == parameter_symbol)?;
    if parameter.is_mutable
        || parameter.is_self
        || !has_stable_observable_contents(program, parameter.type_reference)
    {
        return None;
    }
    // Transition arguments bind only the non-self parameters, in order.
    let argument_index = parameters[..parameter_ordinal]
        .iter()
        .filter(|parameter| !parameter.is_self)
        .count();
    let entry_index = crate::checks::termination::named_transition_target_state_index(
        program,
        machine,
        machine.symbol,
    )?;
    // The invocation itself is the entry state's one non-transition arrival.
    let mut provenance = if state_index == entry_index {
        Some(CrashPredicateExpression::Parameter(
            u32::try_from(parameter_ordinal).ok()?,
        ))
    } else {
        None
    };
    for (source_symbol, statement_ordinal, argument) in
        named_transition_arguments(program, machine, state_index, argument_index)
    {
        if source_symbol == state_symbol
            && program.expression_table.expression_is_valid(argument)
            && let ExpressionNode::Name(forwarded) = program.expression_table.expression(argument)
            && forwarded.symbol == parameter_symbol
            && forwarded.head_symbol == parameter_symbol
            && program
                .expression_table
                .name_path_members(forwarded.members)
                .len()
                == 1
        {
            continue;
        }
        let resolved = entry_operand_at(
            program,
            machine_symbol,
            source_symbol,
            statement_ordinal,
            argument,
            depth + 1,
        )?;
        if let Some(existing) = provenance.as_ref() {
            if *existing != resolved {
                return None;
            }
        } else {
            provenance = Some(resolved);
        }
    }
    provenance
}

/// Positional arguments of every named transition edge into `state_index`,
/// paired with the source state and the transition's own statement ordinal.
/// Typed lowering flattens nested transition forms into statements; inspect
/// ordinary targets and continuation targets, just as the termination graph
/// does. `-> self` and exits are not named arrivals; an edge short an
/// argument contributes an invalid handle that fails resolution above.
fn named_transition_arguments(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    state_index: usize,
    argument_index: usize,
) -> Vec<(SymbolHandle, usize, ExpressionHandle)> {
    let mut incoming = Vec::new();
    for source in program.machine_states(machine) {
        for (ordinal, statement) in program
            .statement_table
            .statements(source.statement_nodes)
            .iter()
            .enumerate()
        {
            let StatementNode::Transition(transition) = statement else {
                continue;
            };
            for target in [transition.target, transition.continuation] {
                if !target.is_valid() {
                    continue;
                }
                let TransitionTargetNode::Named {
                    path, arguments, ..
                } = program.statement_table.transition_target(target)
                else {
                    continue;
                };
                if crate::checks::termination::named_transition_target_state_index(
                    program,
                    machine,
                    path.symbol,
                ) != Some(state_index)
                {
                    continue;
                }
                incoming.push((
                    source.symbol,
                    ordinal,
                    program
                        .statement_table
                        .expression_handles(*arguments)
                        .get(argument_index)
                        .copied()
                        .unwrap_or_else(ExpressionHandle::invalid),
                ));
            }
        }
    }
    incoming
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

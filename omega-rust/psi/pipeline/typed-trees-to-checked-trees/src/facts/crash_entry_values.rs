//! Relate saved crash operands to invocation-entry values.
//!
//! Ordinary calls and selected operators share this substitution boundary.
//! A stable binding is insufficient when its contents contain mutable loans or
//! interior authority. Reuse stable-observation validation before projecting
//! fields; shared loans may retain immutable contents without owning them.
//! State parameters resolve through every arrival that binds them: the
//! invocation itself for the entry state, plus each named transition edge
//! into the state, which must all produce the same entry-relative operand.
//! A mutable binding additionally keeps that bound snapshot only while its
//! storage is pristine: a write, an exclusive borrow, or a mutable receiver
//! call before the read (or before a `-> self`/same-state forwarding edge
//! that carries the storage into the next arrival) ends provenance. Field
//! projections version the storage below the binding root, so a field read
//! survives writes confined to disjoint siblings. Mutable bindings with
//! unstable contents, divergent arrivals and unresolvable cycles retain no
//! entry identity. Substitution transports a proven origin, never re-reads an
//! initializer after later operands execute. This is source provenance, not a
//! Terminal certificate.

use checked_trees::CrashPredicateExpression;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableNamePath};
use typed_trees::statement::{StatementNode, TransitionTargetNode};
use validation::has_stable_observable_contents;

mod mutable;
pub(super) use mutable::statement_may_overwrite_place;
use mutable::{PlaceSegment, storage_holds_bound_value};

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
        ExpressionNode::Borrow(borrow)
            if borrow.access == language_core::ReferenceAccess::Shared =>
        {
            // A shared borrow supplies access, not storage: every place a
            // guard observes through the reference is a place of the
            // referent, so the operand's entry identity is the referent's.
            // The referent's own pristine-storage window already covers the
            // containing statement — including this borrow expression — so a
            // mutable referent keeps provenance only while nothing in that
            // window could have disturbed it. Exclusive and write-only
            // loans keep no entry identity: the loan itself ends the
            // referent's bound-snapshot window, and an immutable referent
            // cannot form one at all.
            entry_operand_at(
                program,
                machine_symbol,
                state_symbol,
                before_statement,
                borrow.target,
                depth + 1,
            )
        }
        ExpressionNode::Member(member)
            if member.member_symbol.is_valid() && member.case_variant.is_none() =>
        {
            // Walk the contiguous field projection down to its base so a
            // mutable root's pristine-storage check can version the bound
            // snapshot per field: a sibling write does not overwrite this
            // projection. Anything below a case payload or an unresolvable
            // member is not a plain field path and keeps its own resolution.
            let mut segments = vec![(member.member_symbol, member.member.as_str().to_owned())];
            let mut base = member.receiver;
            loop {
                if !program.expression_table.expression_is_valid(base) {
                    return None;
                }
                match program.expression_table.expression(base) {
                    ExpressionNode::Member(inner)
                        if inner.member_symbol.is_valid() && inner.case_variant.is_none() =>
                    {
                        segments.push((inner.member_symbol, inner.member.as_str().to_owned()));
                        base = inner.receiver;
                    }
                    _ => break,
                }
            }
            segments.reverse();
            let root = match program.expression_table.expression(base) {
                ExpressionNode::Name(path) => entry_operand_name_at(
                    program,
                    machine_symbol,
                    state_symbol,
                    before_statement,
                    path,
                    &segments
                        .iter()
                        .map(|(symbol, _)| PlaceSegment::Field(*symbol))
                        .collect::<Vec<_>>(),
                    depth + 1,
                )?,
                _ => entry_operand_at(
                    program,
                    machine_symbol,
                    state_symbol,
                    before_statement,
                    base,
                    depth + 1,
                )?,
            };
            Some(segments.iter().fold(root, |receiver, (_, member)| {
                CrashPredicateExpression::Member {
                    receiver: Box::new(receiver),
                    member: member.clone(),
                }
            }))
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
        ExpressionNode::Name(path) => entry_operand_name_at(
            program,
            machine_symbol,
            state_symbol,
            before_statement,
            path,
            &[],
            depth,
        ),
        _ => None,
    }
}

/// A single-member name's entry operand. `field_path` is the field projection
/// below this binding that the enclosing `Member` chain reads: for a mutable
/// local or parameter the pristine-storage window only has to keep that
/// projection unwritten, since sibling fields version independently.
fn entry_operand_name_at(
    program: &TypedTrees,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    before_statement: usize,
    path: &TableNamePath,
    field_path: &[PlaceSegment],
    depth: u32,
) -> Option<CrashPredicateExpression> {
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
            if !has_stable_observable_contents(program, local.type_reference)
                || (local.is_mutable
                    && !storage_holds_bound_value(
                        program,
                        machine_symbol,
                        state,
                        ordinal + 1,
                        before_statement.saturating_add(1),
                        local.symbol,
                        field_path,
                    ))
            {
                return None;
            }
            // This transports the bound value, not a current read of its
            // storage. A mutable local is admitted only while no statement
            // between its initializer and this read could have overwritten
            // the read projection or lent it exclusive access; the containing
            // statement itself stays in the window because its earlier
            // operands may already have run a call that writes through an
            // exclusive borrow. Every initializer dependency must
            // independently be entry-relative. Decreasing the prefix also
            // prevents recursive aliases.
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
        before_statement,
        field_path,
        depth,
    )
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
/// the state positionally. `-> self` forwards the current values; for an
/// immutable parameter that is always the bound snapshot, while a mutable
/// parameter's storage must still be pristine at the edge. A by-name edge
/// forwarding this same parameter back to its own state is tautological under
/// the same rule. Every remaining edge must resolve to one identical
/// entry-relative operand, or provenance stays unknown rather than picking a
/// winner. For a mutable parameter the read itself and every self-referential
/// edge must keep the projected `field_path` pristine — the produced `Member`
/// operand asserts only that projection is uniform across arrivals, never
/// that the whole bound record is.
fn state_parameter_entry_operand(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    parameter_symbol: SymbolHandle,
    before_statement: usize,
    field_path: &[PlaceSegment],
    depth: u32,
) -> Option<CrashPredicateExpression> {
    let states = program.machine_states(machine);
    let state_index = states
        .iter()
        .position(|state| state.symbol == state_symbol)?;
    let state = &states[state_index];
    let parameters = program.state_parameters(state);
    let (parameter_ordinal, parameter) = parameters
        .iter()
        .enumerate()
        .find(|(_, parameter)| parameter.symbol == parameter_symbol)?;
    if parameter.is_self
        || !has_stable_observable_contents(program, parameter.type_reference)
        || (parameter.is_mutable
            && !storage_holds_bound_value(
                program,
                machine_symbol,
                state,
                0,
                before_statement.saturating_add(1),
                parameter_symbol,
                field_path,
            ))
    {
        return None;
    }
    let mutable = parameter.is_mutable;
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
            // The edge forwards the parameter's current storage back into its
            // own arrival slot. That is tautological only while the read
            // projection still holds the bound value at the edge — a sibling
            // field may change between arrivals without moving this operand.
            // The edge's own argument expressions count because they evaluate
            // at this point.
            if mutable
                && !storage_holds_bound_value(
                    program,
                    machine_symbol,
                    state,
                    0,
                    statement_ordinal.saturating_add(1),
                    parameter_symbol,
                    field_path,
                )
            {
                return None;
            }
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
    if mutable {
        // `-> self` carries the current storage into the next arrival, so the
        // bound snapshot survives only while the read projection is still
        // pristine at every self edge — including the edge's own evaluated
        // arguments. A sibling field may drift between arrivals; the produced
        // `Member` operand only asserts the projected field is uniform.
        for ordinal in mutable::self_target_ordinals(program, state) {
            if !storage_holds_bound_value(
                program,
                machine_symbol,
                state,
                0,
                ordinal.saturating_add(1),
                parameter_symbol,
                field_path,
            ) {
                return None;
            }
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

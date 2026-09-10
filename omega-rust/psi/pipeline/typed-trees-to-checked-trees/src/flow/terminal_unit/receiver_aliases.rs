//! Erase nonescaping reference carriers, not their captured referent or access.

use super::*;
use checked_trees::{
    BorrowAccessKind, BorrowLoanLineage, FlowBorrowWeakeningReason, FlowInvalidationSource,
};
use typed_trees::expression::ExpressionHandle;

mod nested;
mod projection;
#[cfg(test)]
mod tests;

pub(super) struct ReceiverAlias {
    pub(super) owner: SymbolHandle,
    pub(super) root: SymbolHandle,
    pub(super) segments: Vec<facts::PlaceSegment>,
}

/// This is source correspondence for direct calls, not restored-use authority.
/// Every erased local is an exactly captured exclusive loan with no escaping use.
pub(super) fn prefix(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
) -> Option<Vec<ReceiverAlias>> {
    let statements = program.statement_table.statements(state.statement_nodes);
    let count = statements
        .iter()
        .take_while(|statement| matches!(statement, StatementNode::LocalData(_)))
        .count();
    if count == 0 {
        return None;
    }
    let parameters = program.state_parameters(state);
    let flow = state_flow(facts, machine.symbol, state.symbol)?;
    let mut borrow_states = facts.borrow.states.iter().filter(|(_, candidate)| {
        candidate.machine_symbol == machine.symbol && candidate.state_symbol == state.symbol
    });
    let (_, borrow_state) = borrow_states.next()?;
    if borrow_states.next().is_some() {
        return None;
    }
    let mut aliases = Vec::new();
    let mut loans = Vec::new();
    let mut parents = Vec::new();
    for (statement_index, statement) in statements[..count].iter().enumerate() {
        let StatementNode::LocalData(local) = statement else {
            return None;
        };
        if local.is_mutable
            || !local.symbol.is_valid()
            || aliases
                .iter()
                .any(|alias: &ReceiverAlias| alias.owner == local.symbol)
        {
            return None;
        }
        let access = exclusive_access(program, local.type_reference)?;
        let ExpressionNode::Borrow(borrow) =
            program.expression_table.expression(local.initial_value)
        else {
            return None;
        };
        if !matches!(
            (&access, borrow.access),
            (
                BorrowAccessKind::Mutable,
                language_semantics::ReferenceAccess::Mutable
            ) | (
                BorrowAccessKind::WriteOnly,
                language_semantics::ReferenceAccess::WriteOnly
            )
        ) {
            return None;
        }
        let (place, source) =
            projection::formation_place(program, machine, state, statement_index, borrow.target)?;
        let facts::PlaceRoot::Symbol(source_root) = place.root else {
            return None;
        };
        if let Some(parent_position) = aliases.iter().position(|alias| alias.owner == source_root) {
            let (projected_type, _) =
                calls::projected_argument_path(program, state.symbol, statement_index, &place)?;
            if parents.contains(&Some(parent_position))
                || source.root_symbol != source_root
                || source.segments != place.segments
                || base_type_identity(program, projected_type, &[])?
                    != base_type_identity(program, local.type_reference, &[])?
            {
                return None;
            }
            let parent: &(arena::Handle<checked_trees::BorrowLoanFact>, usize) =
                loans.get(parent_position)?;
            let loan = nested::formation(
                facts,
                flow,
                borrow_state,
                statement_index,
                local.symbol,
                source_root,
                &source.segments,
                parent.0,
                &access,
                statements.len(),
            )?;
            let mut segments = aliases[parent_position].segments.clone();
            segments.extend_from_slice(&place.segments);
            aliases.push(ReceiverAlias {
                owner: local.symbol,
                root: aliases[parent_position].root,
                segments,
            });
            loans.push(loan);
            parents.push(Some(parent_position));
            continue;
        }
        let mut roots = parameters.iter().filter(|parameter| {
            parameter.symbol == source_root || (parameter.is_self && source_root == machine.symbol)
        });
        let root = roots.next()?;
        if roots.next().is_some()
            || root.is_const
            || exclusive_access(program, root.type_reference)?.direct_reborrow_effect(&access)
                != Some(checked_trees::CheckedReborrowAccessEffect::ExclusiveSuspension)
        {
            return None;
        }
        // Runtime self's reference names the machine/Self namespace. Its
        // referent is the exact attachment, as in the ordinary Unit signature
        // collector; no other parameter may borrow that substitution.
        let root_type = if root.is_self {
            let reference = validation::unwrapped_type_reference(program, root.type_reference)?;
            let TypeReferenceNode::Named { symbol, .. } =
                program.type_reference_table.type_reference(reference)
            else {
                return None;
            };
            if *symbol != machine.symbol && *symbol != machine.attached_data_symbol {
                return None;
            }
            if !machine.attached_data_symbol.is_valid()
                || !program
                    .data_definitions()
                    .iter()
                    .any(|data| data.symbol == machine.attached_data_symbol)
            {
                return None;
            }
            if place.segments.is_empty() {
                program
                    .type_reference_table
                    .find_named_type_reference(machine.attached_data_symbol)?
            } else {
                // Projected self reaches its type through the exact attachment
                // field below; no separately authored root type is required.
                root.type_reference
            }
        } else {
            root.type_reference
        };
        let projected_type = if place.segments.is_empty() {
            root_type
        } else {
            calls::projected_argument_path(program, state.symbol, statement_index, &place)?.0
        };
        if base_type_identity(program, projected_type, &[])?
            != base_type_identity(program, local.type_reference, &[])?
        {
            return None;
        }
        // Reconstruct the loan's capture with its original owner. Flow's
        // contextual place spelling normalizes runtime self to its formal,
        // while the borrowing owner retains the exact machine/Self root.
        let captured_root = source.root_symbol;
        let mut candidates = facts.borrow.loans.iter().filter(|(handle, loan)| {
            facts.borrow.state_owns_loan(borrow_state, *handle) && loan.owner_symbol == local.symbol
        });
        let (loan_handle, loan) = candidates.next()?;
        if candidates.next().is_some()
            || loan.statement_index != statement_index
            || loan.lineage != BorrowLoanLineage::DirectRoot
            || loan.source_owner_symbol.is_valid()
            || loan.kind != access
            || loan.root_symbol != captured_root
            || facts.borrow.loan_segments(loan) != source.segments
            || !facts.borrow.loan_owner_path(loan).is_empty()
        {
            return None;
        }
        let mut resources = facts
            .borrow
            .direct_loan_resources
            .iter()
            .filter(|(_, resource)| resource.loan == loan_handle);
        let (_, resource) = resources.next()?;
        let activation = FlowInvalidationSource::Statement { statement_index };
        if resources.next().is_some()
            || resource.machine_symbol != machine.symbol
            || resource.state_symbol != state.symbol
            || resource.owner_symbol != local.symbol
            || !resource.owner_path.is_empty()
            || resource.captured_place.root_symbol != captured_root
            || resource.captured_place != source
            || resource.access != access
            || resource.activation_source != activation
            || resource.parent_lifetime.machine_symbol != machine.symbol
            || resource.parent_lifetime.state_symbol != state.symbol
            || resource.parent_lifetime.root_symbol != captured_root
            || resource.restoration.parent != resource.parent_lifetime
            || resource.restoration.weakening_source != resource.weakening_source
            || resource.restoration.weakening_reason != resource.weakening_reason
        {
            return None;
        }
        let activations = facts
            .flow
            .borrow_lifetimes
            .activations
            .span_or_empty(flow.borrow_activations)
            .iter()
            .filter(|row| row.loan == loan_handle)
            .collect::<Vec<_>>();
        if !matches!(activations.as_slice(), [row] if row.source == activation) {
            return None;
        }
        let weakenings = facts
            .flow
            .borrow_lifetimes
            .weakenings
            .span_or_empty(flow.borrow_weakenings)
            .iter()
            .filter(|row| row.loan == loan_handle)
            .collect::<Vec<_>>();
        let [weakening] = weakenings.as_slice() else {
            return None;
        };
        let boundary = loan.last_use_statement_index.checked_add(1)?;
        let reason = if boundary == statements.len() {
            FlowBorrowWeakeningReason::StateExit
        } else {
            FlowBorrowWeakeningReason::LastUseExpired
        };
        if boundary > statements.len()
            || weakening.source
                != (FlowInvalidationSource::Statement {
                    statement_index: boundary,
                })
            || weakening.reason != reason
            || resource.weakening_source != weakening.source
            || resource.weakening_reason != reason
        {
            return None;
        }
        aliases.push(ReceiverAlias {
            owner: local.symbol,
            root: if root.is_self {
                machine.symbol
            } else {
                root.symbol
            },
            segments: place.segments,
        });
        loans.push((loan_handle, loan.last_use_statement_index));
        parents.push(None);
    }
    let mut last_uses = vec![None; aliases.len()];
    for (child_position, parent) in parents.iter().enumerate() {
        if let Some(parent_position) = parent {
            last_uses[*parent_position] = Some(child_position);
        }
    }
    for (statement_index, statement) in statements.iter().enumerate().skip(count) {
        if !matches!(
            statement,
            StatementNode::Call(_) | StatementNode::Expression(_)
        ) {
            return None;
        }
        let site =
            crate::find_call_site(program, machine.symbol, state.symbol, statement_index, 0)?;
        for argument in crate::call_site_argument_expressions(program, &site) {
            if !without_alias(program, *argument, &aliases) {
                return None;
            }
        }
        let receiver = crate::flow::canonical_receiver_place_for_call_site(
            program,
            machine.symbol,
            state.symbol,
            &site,
        );
        if let Some(receiver) = receiver
            && let Some(position) = aliases
                .iter()
                .position(|alias| receiver.root == facts::PlaceRoot::Symbol(alias.owner))
        {
            // Using a suspended ancestor would need separate restored-use
            // evidence; this prefix only erases leaves and retired parents.
            if parents.contains(&Some(position)) {
                return None;
            }
            let target = match &site {
                crate::CallSite::Statement(call) => call.target_symbol,
                crate::CallSite::Expression { call, .. } => call.target_symbol,
                crate::CallSite::TransitionNamed { .. } => return None,
            };
            let mut receivers = crate::call_target_parameters(program, target)?
                .iter()
                .filter(|parameter| parameter.is_self);
            let receiver_parameter = receivers.next()?;
            if receivers.next().is_some()
                || structural_access_for_type_reference(program, receiver_parameter.type_reference)?
                    != CheckedStructuralAccess::WriteOnlyBorrow
            {
                return None;
            }
            if !receiver.segments.iter().all(|segment| {
                matches!(
                    segment,
                    facts::PlaceSegment::Field { .. } | facts::PlaceSegment::FixedIndex { .. }
                )
            }) {
                return None;
            }
            let statement_flow = facts
                .flow
                .control
                .statements
                .span_or_empty(flow.statements)
                .iter()
                .find(|row| row.statement_index == statement_index)?;
            let available = facts
                .flow
                .contexts
                .constraint_refs
                .span_or_empty(statement_flow.entry_constraints)
                .iter()
                .any(|row| {
                    matches!(row.kind,
                        checked_trees::FlowConstraintKind::BorrowLoan { loan }
                            if loan == loans[position].0)
                });
            if !available {
                return None;
            }
            last_uses[position] = Some(statement_index);
        }
    }
    if last_uses
        .iter()
        .zip(&loans)
        .any(|(actual, (_, expected))| *actual != Some(*expected))
    {
        return None;
    }
    nested::closures(facts, flow, &loans, &parents)?;
    Some(aliases)
}

fn exclusive_access(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
) -> Option<BorrowAccessKind> {
    match structural_access_for_type_reference(program, reference)? {
        CheckedStructuralAccess::MutableBorrow => Some(BorrowAccessKind::Mutable),
        CheckedStructuralAccess::WriteOnlyBorrow => Some(BorrowAccessKind::WriteOnly),
        _ => None,
    }
}

// All arguments retain their existing scalar/structural planner. This walk only
// rules out a second, escaping occurrence of an erased carrier in those trees.
fn without_alias(
    program: &TypedTrees,
    expression: ExpressionHandle,
    aliases: &[ReceiverAlias],
) -> bool {
    let mut pending = vec![expression];
    let mut visited = Vec::new();
    while let Some(expression) = pending.pop() {
        if !program.expression_table.expression_is_valid(expression) {
            return false;
        }
        if visited.contains(&expression) {
            continue;
        }
        visited.push(expression);
        match program.expression_table.expression(expression) {
            ExpressionNode::Match(dispatch) => {
                pending.push(dispatch.subject);
                for arm in program.expression_table.match_arms(dispatch.arms) {
                    if let typed_trees::expression::MatchPattern::Value(pattern) = arm.pattern {
                        pending.push(pattern);
                    }
                    pending.push(arm.value);
                }
            }
            ExpressionNode::Name(name) => {
                if aliases.iter().any(|alias| {
                    name.symbol == alias.owner
                        || name.head_symbol == alias.owner
                        || program
                            .expression_table
                            .name_path_member_symbols(name.member_symbols)
                            .contains(&alias.owner)
                }) {
                    return false;
                }
            }
            ExpressionNode::Binary(binary) => pending.extend([binary.left, binary.right]),
            ExpressionNode::Unary(unary) => pending.push(unary.operand),
            ExpressionNode::Cast(cast) => pending.push(cast.value),
            ExpressionNode::Borrow(borrow) => pending.push(borrow.target),
            ExpressionNode::Member(member) => pending.push(member.receiver),
            ExpressionNode::Indexed(indexed) => pending.extend([indexed.collection, indexed.index]),
            ExpressionNode::Range(range) => pending.extend(
                [range.start, range.end]
                    .into_iter()
                    .filter(|value| value.is_valid()),
            ),
            ExpressionNode::Call(call) => {
                if call.receiver.is_valid() {
                    pending.push(call.receiver);
                }
                pending
                    .extend_from_slice(program.expression_table.expression_handles(call.arguments));
            }
            ExpressionNode::ArrayLiteral(values) => {
                pending.extend_from_slice(program.expression_table.expression_handles(*values))
            }
            ExpressionNode::StructLiteral(literal) => pending.extend(
                program
                    .expression_table
                    .struct_fields(literal.fields)
                    .iter()
                    .map(|field| field.value),
            ),
            ExpressionNode::Atomic(atomic) => pending.extend([atomic.value, atomic.result]),
            ExpressionNode::Boolean(_)
            | ExpressionNode::Integer(_)
            | ExpressionNode::Float(_)
            | ExpressionNode::String(_)
            | ExpressionNode::ZeroValue(_) => {}
        }
    }
    true
}

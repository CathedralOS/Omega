//! Source replay for erased whole-referent receiver alias chains.
//!
//! This binds erased names to their exact loans. The separate root-handoff
//! receiving pass retains complete lineage; neither pass grants restored use.

use crate::{CheckedTrees, LoweringError, unsupported};
use arena::Handle;
use checked_trees::expression::{ExpressionHandle, ExpressionNode};
use checked_trees::statement::StatementNode;
use checked_trees::types::TypeReferenceNode;
use checked_trees::{
    BorrowAccessKind, BorrowLoanFact, BorrowLoanLineage, CheckedParentBorrowResource,
    FlowBorrowWeakeningReason, FlowConstraintKind, FlowInvalidationSource,
};
use language_semantics::ReferenceAccess;
use symbols::SymbolHandle;

mod reborrow;
use reborrow::reborrow_resource;

pub(super) fn parameter_root(
    checked: &CheckedTrees,
    machine: SymbolHandle,
    state: SymbolHandle,
    statement_index: usize,
    owner: SymbolHandle,
) -> Result<Option<SymbolHandle>, LoweringError> {
    replay_alias(checked, machine, state, statement_index, owner, false)
}

fn replay_alias(
    checked: &CheckedTrees,
    machine: SymbolHandle,
    state: SymbolHandle,
    statement_index: usize,
    owner: SymbolHandle,
    formation: bool,
) -> Result<Option<SymbolHandle>, LoweringError> {
    let (authored_machine, authored_state) =
        crate::scalar_source_custody::authored_state(checked, state)?;
    if authored_machine.symbol != machine {
        return unsupported("receiver alias state belongs to another machine");
    }
    let statements = checked
        .statement_table
        .statements(authored_state.statement_nodes);
    let prefix = statements
        .iter()
        .take_while(|statement| matches!(statement, StatementNode::LocalData(_)))
        .count();
    let declarations = statements[..prefix]
        .iter()
        .enumerate()
        .filter_map(|(position, statement)| {
            let StatementNode::LocalData(local) = statement else {
                return None;
            };
            (local.symbol == owner).then_some((position, local))
        })
        .collect::<Vec<_>>();
    let [(declaration_index, local)] = declarations.as_slice() else {
        return if declarations.is_empty() {
            Ok(None)
        } else {
            unsupported("receiver alias has duplicate declarations")
        };
    };
    if local.is_mutable
        || !owner.is_valid()
        || statement_index <= *declaration_index
        || (!formation && statement_index < prefix)
    {
        return unsupported("receiver alias is mutable or used before its prefix ends");
    }
    let ExpressionNode::Borrow(initializer) =
        checked.expression_table.expression(local.initial_value)
    else {
        return unsupported("receiver alias has no explicit borrow initializer");
    };
    let ExpressionNode::Name(name) = checked.expression_table.expression(initializer.target) else {
        return unsupported("receiver alias initializer is not a whole parameter");
    };
    if !checked
        .expression_table
        .expression_is_valid(local.initial_value)
        || !checked
            .expression_table
            .expression_is_valid(initializer.target)
        || initializer.access != ReferenceAccess::WriteOnly
        || !name.symbol.is_valid()
        || name.symbol != name.head_symbol
        || checked
            .expression_table
            .name_path_members(name.members)
            .len()
            != 1
        || checked
            .expression_table
            .name_path_member_symbols(name.member_symbols)
            .first()
            .is_some_and(|symbol| *symbol != name.symbol)
    {
        return unsupported("receiver alias initializer identity or access changed");
    }
    let parent_declaration = statements[..*declaration_index].iter().any(|statement| {
        matches!(statement, StatementNode::LocalData(parent) if parent.symbol == name.symbol)
    });
    let root_symbol = if parent_declaration {
        replay_alias(
            checked,
            machine,
            state,
            *declaration_index,
            name.symbol,
            true,
        )?
        .ok_or(LoweringError::Unsupported(
            "receiver alias lost its immediate parent",
        ))?
    } else {
        name.symbol
    };
    let roots = checked
        .state_parameters(authored_state)
        .iter()
        .filter(|parameter| {
            parameter.symbol == root_symbol || (parameter.is_self && root_symbol == machine)
        })
        .collect::<Vec<_>>();
    let [root] = roots.as_slice() else {
        return unsupported("receiver alias has no unique parameter root");
    };
    let TypeReferenceNode::Reference {
        referee: local_type,
        access: ReferenceAccess::WriteOnly,
        ..
    } = checked
        .type_reference_table
        .type_reference(local.type_reference)
    else {
        return unsupported("receiver alias is not a write-only reference");
    };
    let TypeReferenceNode::Reference {
        referee: root_type,
        access: ReferenceAccess::Mutable | ReferenceAccess::WriteOnly,
        ..
    } = checked
        .type_reference_table
        .type_reference(root.type_reference)
    else {
        return unsupported("receiver alias root cannot grant write-only access");
    };
    // Attached self carries machine/Self identity in the typed reference. Its
    // actual referent is the independently resolved attachment, as in signatures.
    let root_type = if root.is_self {
        let TypeReferenceNode::Named { symbol, .. } =
            checked.type_reference_table.type_reference(*root_type)
        else {
            return unsupported("receiver alias self has no exact nominal referent");
        };
        if (*symbol != machine && *symbol != authored_machine.attached_data_symbol)
            || !authored_machine.attached_data_symbol.is_valid()
            || !checked
                .data_definitions()
                .iter()
                .any(|data| data.symbol == authored_machine.attached_data_symbol)
        {
            return unsupported("receiver alias self disagrees with its attachment");
        }
        checked
            .type_reference_table
            .find_named_type_reference(authored_machine.attached_data_symbol)
            .ok_or(LoweringError::Unsupported(
                "receiver alias self lost its attachment",
            ))?
    } else {
        *root_type
    };
    if root.is_const
        || checked.typed.normalized_type_identity(*local_type)
            != checked.typed.normalized_type_identity(root_type)
    {
        return unsupported("receiver alias changes its referent type");
    }
    let borrow = &checked.facts.borrow;
    let states = borrow
        .states
        .iter()
        .filter(|(_, row)| row.machine_symbol == machine && row.state_symbol == state)
        .collect::<Vec<_>>();
    let [(_, borrow_state)] = states.as_slice() else {
        return unsupported("receiver alias has no unique borrow state");
    };
    let loans = borrow
        .loans
        .iter()
        .filter(|(handle, loan)| {
            borrow.state_owns_loan(borrow_state, *handle) && loan.owner_symbol == owner
        })
        .collect::<Vec<_>>();
    let [(loan_handle, loan)] = loans.as_slice() else {
        return unsupported("receiver alias has no unique loan occurrence");
    };
    let parent_loans = borrow
        .loans
        .iter()
        .filter(|(handle, candidate)| {
            borrow.state_owns_loan(borrow_state, *handle) && candidate.owner_symbol == name.symbol
        })
        .collect::<Vec<_>>();
    let exact_lineage = if parent_declaration {
        matches!(parent_loans.as_slice(), [(parent_handle, parent)]
            if loan.lineage == BorrowLoanLineage::Reborrow { parent_loan: *parent_handle }
                && loan.source_owner_symbol == name.symbol
                && loan.root_symbol == parent.root_symbol)
    } else {
        loan.lineage == BorrowLoanLineage::DirectRoot
            && !loan.source_owner_symbol.is_valid()
            && loan.root_symbol == name.symbol
    };
    if loan.statement_index != *declaration_index
        || loan.kind != BorrowAccessKind::WriteOnly
        || !exact_lineage
        || !borrow.loan_segments(loan).is_empty()
        || !borrow.loan_owner_path(loan).is_empty()
    {
        return unsupported("receiver alias loan disagrees with its direct initializer");
    }
    let flow_states = checked
        .facts
        .flow
        .control
        .states
        .iter()
        .filter(|(_, row)| row.machine_symbol == machine && row.state_symbol == state)
        .collect::<Vec<_>>();
    let [(_, flow)] = flow_states.as_slice() else {
        return unsupported("receiver alias has no unique flow state");
    };
    let mut last_use = None;
    let mut current_use = false;
    for (position, statement) in statements.iter().enumerate().skip(*declaration_index + 1) {
        let (receiver, arguments) = match statement {
            StatementNode::LocalData(child) if position < prefix => {
                if !contains_owner(checked, child.initial_value, owner) {
                    continue;
                }
                let ExpressionNode::Borrow(initializer) =
                    checked.expression_table.expression(child.initial_value)
                else {
                    return unsupported("receiver alias escapes through a local initializer");
                };
                let ExpressionNode::Name(name) =
                    checked.expression_table.expression(initializer.target)
                else {
                    return unsupported("receiver alias child is not a whole referent");
                };
                if child.is_mutable
                    || initializer.access != ReferenceAccess::WriteOnly
                    || name.symbol != owner
                    || name.head_symbol != owner
                {
                    return unsupported("receiver alias child formation changed");
                }
                (owner, &[][..])
            }
            StatementNode::Call(call) => (
                call.receiver_root_symbol,
                checked.statement_table.expression_handles(call.arguments),
            ),
            StatementNode::Expression(expression) => {
                let ExpressionNode::Call(call) = checked.expression_table.expression(*expression)
                else {
                    return unsupported("receiver alias suffix contains a non-call expression");
                };
                (
                    receiver_root(checked, call.receiver)?,
                    checked.expression_table.expression_handles(call.arguments),
                )
            }
            _ => return unsupported("receiver alias suffix contains a write, local, or escape"),
        };
        if arguments
            .iter()
            .any(|argument| contains_owner(checked, *argument, owner))
        {
            return unsupported("receiver alias escapes through an explicit argument");
        }
        if receiver != owner {
            continue;
        }
        let entries = checked
            .facts
            .flow
            .control
            .statements
            .span_or_empty(flow.statements)
            .iter()
            .filter(|row| row.statement_index == position)
            .collect::<Vec<_>>();
        let [entry] = entries.as_slice() else {
            return unsupported("receiver alias use has no unique statement entry");
        };
        let available = checked.facts.flow.contexts.constraint_refs
            .span_or_empty(entry.entry_constraints)
            .iter()
            .any(|row| matches!(row.kind, FlowConstraintKind::BorrowLoan { loan } if loan == *loan_handle));
        if !available {
            return unsupported("receiver alias loan is unavailable at its use");
        }
        last_use = Some(position);
        current_use |= position == statement_index;
    }
    if !current_use || last_use != Some(loan.last_use_statement_index) {
        return unsupported("receiver alias use interval disagrees with its source");
    }
    let activation = FlowInvalidationSource::Statement {
        statement_index: *declaration_index,
    };
    let boundary =
        loan.last_use_statement_index
            .checked_add(1)
            .ok_or(LoweringError::Unsupported(
                "receiver alias lifetime overflows",
            ))?;
    let weakening = FlowInvalidationSource::Statement {
        statement_index: boundary,
    };
    let reason = if boundary == statements.len() {
        FlowBorrowWeakeningReason::StateExit
    } else {
        FlowBorrowWeakeningReason::LastUseExpired
    };
    let lifetimes = &checked.facts.flow.borrow_lifetimes;
    let activations = lifetimes
        .activations
        .span_or_empty(flow.borrow_activations)
        .iter()
        .filter(|row| row.loan == *loan_handle)
        .collect::<Vec<_>>();
    let weakenings = lifetimes
        .weakenings
        .span_or_empty(flow.borrow_weakenings)
        .iter()
        .filter(|row| row.loan == *loan_handle)
        .collect::<Vec<_>>();
    if !matches!(activations.as_slice(), [row] if row.source == activation)
        || !matches!(weakenings.as_slice(), [row] if row.source == weakening && row.reason == reason)
    {
        return unsupported("receiver alias activation or weakening changed");
    }
    if parent_declaration {
        if !formation && reason != FlowBorrowWeakeningReason::StateExit {
            return unsupported("receiver alias chain has no state-exit root handoff");
        }
        let [(parent_handle, _)] = parent_loans.as_slice() else {
            return unsupported("receiver alias has no exact parent loan");
        };
        reborrow_resource(
            checked,
            machine,
            state,
            *loan_handle,
            *parent_handle,
            owner,
            activation,
            weakening,
            reason,
        )?;
        return Ok(Some(root.symbol));
    }
    let resources = borrow
        .direct_loan_resources
        .iter()
        .filter(|(_, row)| row.loan == *loan_handle)
        .collect::<Vec<_>>();
    let [(_, resource)] = resources.as_slice() else {
        return unsupported("receiver alias has no unique direct loan resource");
    };
    if resource.machine_symbol != machine
        || resource.state_symbol != state
        || resource.owner_symbol != owner
        || !resource.owner_path.is_empty()
        || resource.captured_place.root_symbol != loan.root_symbol
        || !resource.captured_place.segments.is_empty()
        || resource.access != BorrowAccessKind::WriteOnly
        || resource.activation_source != activation
        || resource.weakening_source != weakening
        || resource.weakening_reason != reason
        || resource.parent_lifetime.machine_symbol != machine
        || resource.parent_lifetime.state_symbol != state
        || resource.parent_lifetime.root_symbol != loan.root_symbol
        || resource.restoration.parent != resource.parent_lifetime
        || resource.restoration.weakening_source != weakening
        || resource.restoration.weakening_reason != reason
    {
        return unsupported("receiver alias resource disagrees with its source lifetime");
    }
    Ok(Some(root.symbol))
}

fn receiver_root(
    checked: &CheckedTrees,
    expression: ExpressionHandle,
) -> Result<SymbolHandle, LoweringError> {
    if !expression.is_valid() {
        return Ok(SymbolHandle::invalid());
    }
    let mut cursor = expression;
    let mut visited = Vec::new();
    loop {
        if !checked.expression_table.expression_is_valid(cursor) || visited.contains(&cursor) {
            return unsupported("receiver alias source is stale or cyclic");
        }
        visited.push(cursor);
        cursor = match checked.expression_table.expression(cursor) {
            ExpressionNode::Name(name) => return Ok(name.head_symbol),
            ExpressionNode::Member(member) => member.receiver,
            ExpressionNode::Indexed(indexed) => indexed.collection,
            _ => return Ok(SymbolHandle::invalid()),
        };
    }
}

fn contains_owner(
    checked: &CheckedTrees,
    expression: ExpressionHandle,
    owner: SymbolHandle,
) -> bool {
    let mut pending = vec![expression];
    let mut visited = Vec::new();
    while let Some(expression) = pending.pop() {
        if !checked.expression_table.expression_is_valid(expression) {
            return true;
        }
        if visited.contains(&expression) {
            continue;
        }
        visited.push(expression);
        match checked.expression_table.expression(expression) {
            ExpressionNode::Name(name) => {
                if name.symbol == owner
                    || name.head_symbol == owner
                    || checked
                        .expression_table
                        .name_path_member_symbols(name.member_symbols)
                        .contains(&owner)
                {
                    return true;
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
                    .extend_from_slice(checked.expression_table.expression_handles(call.arguments));
            }
            ExpressionNode::ArrayLiteral(values) => {
                pending.extend_from_slice(checked.expression_table.expression_handles(*values))
            }
            ExpressionNode::StructLiteral(literal) => pending.extend(
                checked
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
    false
}

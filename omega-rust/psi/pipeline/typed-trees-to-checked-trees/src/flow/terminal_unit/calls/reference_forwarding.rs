//! A bare reference parameter reads its carrier without weakening its referent.

use super::*;

pub(super) fn preserves_mutable_referent(
    program: &TypedTrees,
    borrow: &checked_trees::BorrowFacts,
    borrow_state: &checked_trees::StateBorrowFact,
    borrow_call: &checked_trees::BorrowCallFact,
    call: &checked_trees::FlowCallFact,
    source_symbol: SymbolHandle,
    returned_loan: arena::Handle<checked_trees::BorrowLoanFact>,
) -> bool {
    exact_mutable_referent(
        program,
        borrow,
        borrow_state,
        borrow_call,
        call,
        source_symbol,
        returned_loan,
    )
    .is_some()
}

fn exact_mutable_referent(
    program: &TypedTrees,
    borrow: &checked_trees::BorrowFacts,
    borrow_state: &checked_trees::StateBorrowFact,
    borrow_call: &checked_trees::BorrowCallFact,
    call: &checked_trees::FlowCallFact,
    source_symbol: SymbolHandle,
    returned_loan: arena::Handle<checked_trees::BorrowLoanFact>,
) -> Option<()> {
    let state = crate::find_state_in_machine(
        program,
        borrow_state.machine_symbol,
        borrow_state.state_symbol,
    )?;
    let mut sources = program
        .state_parameters(state)
        .iter()
        .filter(|parameter| parameter.symbol == source_symbol);
    let source = sources.next()?;
    if sources.next().is_some()
        || !source_symbol.is_valid()
        || source.is_self
        || source.is_const
        || structural_access_for_type_reference(program, source.type_reference)?
            != CheckedStructuralAccess::MutableBorrow
    {
        return None;
    }
    // The parser also marks `record: &mut Record` mutable to describe referent
    // access. That bit is not evidence that the carrier was rebound. Rejoin
    // actual preceding writes instead; unknown or overlapping storage changes
    // stay outside this direct entry-parameter forwarding path.
    prefix_preserves_parameter(program, borrow, borrow_state, call, source_symbol)?;
    let site = crate::find_call_site(
        program,
        borrow_state.machine_symbol,
        borrow_state.state_symbol,
        call.statement_index,
        call.call_ordinal,
    )?;
    let target_symbol = match &site {
        crate::CallSite::Statement(authored) => authored.target_symbol,
        crate::CallSite::Expression {
            expression,
            call: authored,
        } if *expression == call.authored_expression => authored.target_symbol,
        _ => return None,
    };
    if !target_symbol.is_valid() || target_symbol != call.target_symbol {
        return None;
    }
    let parameters = crate::call_target_parameters(program, target_symbol)?;
    let arguments = crate::call_site_argument_expressions(program, &site);
    if parameters.len() != arguments.len()
        || parameters
            .iter()
            .any(|parameter| parameter.is_self || parameter.is_const)
    {
        return None;
    }
    let mut matched = false;
    for (parameter, expression) in parameters.iter().zip(arguments) {
        let ExpressionNode::Name(path) = program.expression_table.expression(*expression) else {
            continue;
        };
        if path.symbol != source_symbol {
            continue;
        }
        let members = program.expression_table.name_path_members(path.members);
        if matched
            || path.head_symbol != source_symbol
            || members.len() != 1
            || members[0].as_str() != source.name.as_str()
            || !parameter.symbol.is_valid()
            || structural_access_for_type_reference(program, parameter.type_reference)?
                != CheckedStructuralAccess::MutableBorrow
        {
            return None;
        }
        matched = true;
    }
    if !matched {
        return None;
    }
    // The existing borrow call owns the exact occurrence and all argument
    // observations. A second observation of this root may overlap the mutable
    // referent, even when it names a projected field rather than the carrier.
    let mut accesses = borrow
        .argument_accesses
        .span_or_empty(borrow_call.accesses)
        .iter()
        .filter(|access| access.root_symbol == source_symbol);
    let access = accesses.next()?;
    if accesses.next().is_some()
        || !borrow.access_segments(access).is_empty()
        || access.kind != checked_trees::BorrowAccessKind::Read
    {
        return None;
    }
    // No restoration is inferred from the carrier type. Live descendant loans
    // still require the existing exact restoration certificates; those paths
    // are handled separately by the call-plan owner.
    if borrow
        .loans
        .iter()
        .filter(|(handle, _)| borrow.state_owns_loan(borrow_state, *handle))
        .any(|(handle, loan)| {
            // The exact reconstructed result loan begins with this call's
            // successful return. It is not a preexisting descendant blocking
            // the ingress borrow that produces it. Every other live loan is.
            handle != returned_loan
                && loan.statement_index <= call.statement_index
                && loan.last_use_statement_index >= call.statement_index
                && (loan.root_symbol == source_symbol
                    || loan.owner_symbol == source_symbol
                    || loan.source_owner_symbol == source_symbol)
        })
    {
        return None;
    }
    Some(())
}

fn prefix_preserves_parameter(
    program: &TypedTrees,
    borrow: &checked_trees::BorrowFacts,
    borrow_state: &checked_trees::StateBorrowFact,
    call: &checked_trees::FlowCallFact,
    source_symbol: SymbolHandle,
) -> Option<()> {
    let state = crate::find_state_in_machine(
        program,
        borrow_state.machine_symbol,
        borrow_state.state_symbol,
    )?;
    let statements = program.statement_table.statements(state.statement_nodes);
    let call_frames = validation::CallFrameResolver::new(program);
    for (statement_index, statement) in statements.get(..call.statement_index)?.iter().enumerate() {
        let writes = crate::flow::statement_storage_writes(
            program,
            borrow_state.machine_symbol,
            borrow_state.state_symbol,
            statement_index,
            statement,
            call_frames.as_ref(),
        )?;
        if writes
            .iter()
            .any(|place| place.root == facts::PlaceRoot::Symbol(source_symbol))
        {
            return None;
        }
    }
    let summaries = crate::flow::StateMutationSummaryCache::default();
    for preceding in borrow.calls.span_or_empty(borrow_state.calls) {
        if preceding.statement_index > call.statement_index
            || (preceding.statement_index == call.statement_index
                && preceding.call_ordinal == call.call_ordinal)
        {
            continue;
        }
        // Nested operands can evaluate before their enclosing call regardless
        // of traversal ordinal, so retain every other call in this statement.
        let writes = crate::flow::call_mutated_places(
            program,
            borrow_state.machine_symbol,
            borrow_state.state_symbol,
            borrow,
            preceding,
            &summaries,
            call_frames.as_ref(),
        )?;
        let writes = crate::flow::close_storage_places_over_aliases_with_resolver(
            program,
            borrow_state.machine_symbol,
            borrow_state.state_symbol,
            preceding.statement_index,
            writes,
            call_frames.as_ref(),
        )?;
        if writes
            .iter()
            .any(|place| place.root == facts::PlaceRoot::Symbol(source_symbol))
            && !(preceding.statement_index < call.statement_index
                && preceding_byte_loan_preserves_carrier(
                    program,
                    borrow,
                    borrow_state,
                    preceding,
                    source_symbol,
                ))
        {
            return None;
        }
    }
    Some(())
}

/// A completed call through a byte view can change elements, not the caller's
/// reference carrier. This does not admit assignments or restore live loans.
fn preceding_byte_loan_preserves_carrier(
    program: &TypedTrees,
    borrow: &checked_trees::BorrowFacts,
    state: &checked_trees::StateBorrowFact,
    call: &checked_trees::BorrowCallFact,
    source_symbol: SymbolHandle,
) -> bool {
    let Some(source_state) =
        crate::find_state_in_machine(program, state.machine_symbol, state.state_symbol)
    else {
        return false;
    };
    let Some(source) = program
        .state_parameters(source_state)
        .iter()
        .find(|parameter| parameter.symbol == source_symbol)
    else {
        return false;
    };
    let Some(crate::CallSite::Statement(authored)) = crate::find_call_site(
        program,
        state.machine_symbol,
        state.state_symbol,
        call.statement_index,
        call.call_ordinal,
    ) else {
        return false;
    };
    if authored.target_symbol != call.target_symbol {
        return false;
    }
    let Some(parameters) = crate::call_target_parameters(program, call.target_symbol) else {
        return false;
    };
    let arguments = program
        .statement_table
        .expression_handles(authored.arguments);
    if parameters.len() != arguments.len() {
        return false;
    }
    let mut matching = parameters.iter().zip(arguments).filter(|(_, expression)| {
        matches!(program.expression_table.expression(**expression), ExpressionNode::Name(name) if name.symbol == source_symbol)
    });
    let Some((parameter, expression)) = matching.next() else {
        return false;
    };
    if matching.next().is_some()
        || parameter.is_self
        || parameter.is_const
        || !(super::fixed_byte_array_mutable_view_is_admitted(
            program,
            source.type_reference,
            parameter.type_reference,
        ) || [source.type_reference, parameter.type_reference]
            .into_iter()
            .all(|reference| {
                structural_access_for_type_reference(program, reference)
                    == Some(CheckedStructuralAccess::MutableBorrow)
                    && byte_sequence_carrier(program, reference, &[])
                        == Some(checked_trees::CheckedByteSequenceCarrier::BorrowedView)
            }))
    {
        return false;
    }
    let ExpressionNode::Name(name) = program.expression_table.expression(*expression) else {
        return false;
    };
    if name.head_symbol != source_symbol
        || program
            .expression_table
            .name_path_members(name.members)
            .len()
            != 1
    {
        return false;
    }
    let mut accesses = borrow
        .argument_accesses
        .span_or_empty(call.accesses)
        .iter()
        .filter(|access| access.root_symbol == source_symbol);
    let Some(access) = accesses.next() else {
        return false;
    };
    accesses.next().is_none()
        && borrow.access_segments(access).is_empty()
        && matches!(
            access.kind,
            checked_trees::BorrowAccessKind::Read | checked_trees::BorrowAccessKind::Mutable
        )
}

#[cfg(test)]
mod tests;

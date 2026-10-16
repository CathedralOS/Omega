//! A bare reference parameter reads its carrier without weakening its referent.
use crate::execution::terminal_unit::CheckedStructuralAccess;
use crate::execution::terminal_unit::ExpressionNode;
use crate::execution::terminal_unit::SymbolHandle;
use crate::execution::terminal_unit::types::structural_access_for_type_reference;
use typed_trees::statement::StatementNode;

use crate::execution::terminal_unit::types::byte_sequence_carrier;

pub(super) fn preserves_mutable_referent(
    program: &dyn ::validation::CheckingInput,
    borrow: &checked_trees::BorrowFacts,
    borrow_state: &checked_trees::StateBorrowFact,
    borrow_call: &checked_trees::BorrowCallFact,
    call: &checked_trees::FlowCallFact,
    source_symbol: SymbolHandle,
    returned_loans: &[arena::Handle<checked_trees::BorrowLoanFact>],
) -> bool {
    exact_mutable_referent(
        program,
        borrow,
        borrow_state,
        borrow_call,
        call,
        source_symbol,
        returned_loans,
    )
    .is_some()
}

fn exact_mutable_referent(
    program: &dyn ::validation::CheckingInput,
    borrow: &checked_trees::BorrowFacts,
    borrow_state: &checked_trees::StateBorrowFact,
    borrow_call: &checked_trees::BorrowCallFact,
    call: &checked_trees::FlowCallFact,
    source_symbol: SymbolHandle,
    returned_loans: &[arena::Handle<checked_trees::BorrowLoanFact>],
) -> Option<()> {
    let state = validation::find_state_in_machine(
        program,
        borrow_state.machine_symbol,
        borrow_state.state_symbol,
    )?;
    let order = program
        .statement_table
        .admitted_execution_order(state.statement_nodes);
    let mut sources = program
        .state_parameters(state)
        .iter()
        .filter(|parameter| parameter.symbol == source_symbol);
    let source = sources.next()?;
    if (sources.next().is_some() || !source_symbol.is_valid())
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
    let site = validation::find_call_site(
        program,
        borrow_state.machine_symbol,
        borrow_state.state_symbol,
        call.statement_index,
        call.call_ordinal,
    )?;
    let target_symbol = match &site {
        validation::CallSite::Statement(authored) => authored.target_symbol,
        validation::CallSite::Expression {
            expression,
            call: authored,
        } if *expression == call.authored_expression => {
            validation::call_target_symbol(program, authored)
        }
        _ => return None,
    };
    if !target_symbol.is_valid() || target_symbol != call.target_symbol {
        return None;
    }
    let parameters = validation::call_target_parameters(program, target_symbol)?;
    let arguments = validation::call_site_argument_expressions(program, &site);
    // Method calls pass no expression for the receiver: align the formal list
    // with the authored argument list by dropping `self`, then compare kinds.
    let formals: Vec<&typed_trees::signature::StateParameter> = parameters.iter().collect();
    if formals.len() != arguments.len() || formals.iter().any(|parameter| parameter.is_const) {
        return None;
    }
    let mut matched = false;
    for (parameter, expression) in formals.iter().zip(arguments) {
        let Some(path) = program.expression_table.name_path_node(*expression) else {
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
        .filter(|access| access.root == facts::PlaceRoot::Symbol(source_symbol));
    let Some(access) = accesses.next() else {
        return None;
    };
    if accesses.next().is_some()
        || !borrow.access_segments(access).is_empty()
        || access.kind != checked_trees::BorrowAccessKind::Read
    {
        return None;
    }
    // No restoration is inferred from the carrier type. Live descendant loans
    // still require the existing exact restoration certificates; those paths
    // are handled separately by the call-plan owner.
    //
    // The result binding's own loan — minted by this call on the statement's
    // local — begins with this call's successful return. It is not a
    // preexisting descendant blocking the ingress borrow that produces it,
    // whether or not the checking-side result-loan custody reconstruction
    // admits its release shape. Every other live loan is.
    let result_owner = program
        .statement_table
        .statement_at(state, call.statement_index)
        .and_then(|statement| match statement {
            StatementNode::LocalData(local) => Some(local.symbol),
            _ => None,
        });
    if borrow
        .loans
        .iter()
        .filter(|(handle, _)| borrow.state_owns_loan(borrow_state, *handle))
        .any(|(handle, loan)| {
            !returned_loans.contains(&handle)
                && order.at_or_before(loan.statement_index, call.statement_index)
                && order.at_or_before(call.statement_index, loan.last_use_statement_index)
                && (loan.root == facts::PlaceRoot::Symbol(source_symbol)
                    || loan.owner_symbol == source_symbol
                    || loan.source_owner_symbol == source_symbol)
                && !(loan.statement_index == call.statement_index
                    && Some(loan.owner_symbol) == result_owner)
        })
    {
        return None;
    }
    Some(())
}

/// Whether the carrier still names the referent it arrived with at this call.
///
/// A `record: &mut Record` parameter is a reference value; forwarding it bare
/// reborrows whatever referent that value names now. Contents changing is not
/// the hazard: a projected store (`record.field = ..`) or a completed call
/// that reborrowed the bare carrier writes through the referent and leaves the
/// carrier naming the same storage. Only a write to the whole parameter place
/// can replace the carrier, so only such a write refuses. The static-machine
/// contract spells every helper call this way (`Record::step(record)` then
/// `Record::step(record)` again), so refusing referent writes would refuse
/// the second call of every such sequence.
fn prefix_preserves_parameter(
    program: &dyn ::validation::CheckingInput,
    borrow: &checked_trees::BorrowFacts,
    borrow_state: &checked_trees::StateBorrowFact,
    call: &checked_trees::FlowCallFact,
    source_symbol: SymbolHandle,
) -> Option<()> {
    let state = validation::find_state_in_machine(
        program,
        borrow_state.machine_symbol,
        borrow_state.state_symbol,
    )?;
    let order = program
        .statement_table
        .admitted_execution_order(state.statement_nodes);
    let call_frames = validation::CallFrameResolver::new(program);
    for (statement_index, statement) in program
        .statement_table
        .admitted_execution_statements(state.statement_nodes)
        .take(order.position(call.statement_index)?)
    {
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
            .any(|place| replaces_carrier(place, source_symbol))
        {
            return None;
        }
    }
    let summaries = crate::flow::StateMutationSummaryCache::default();
    for preceding in borrow.calls.span_or_empty(borrow_state.calls) {
        if order.precedes(call.statement_index, preceding.statement_index)
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
        let writes = crate::flow::close_storage_places_over_aliases(
            program,
            borrow_state.machine_symbol,
            borrow_state.state_symbol,
            preceding.statement_index,
            writes,
            call_frames.as_ref(),
        )?;
        if writes
            .iter()
            .any(|place| replaces_carrier(place, source_symbol))
            && !(order.precedes(preceding.statement_index, call.statement_index)
                && preceding_reborrow_preserves_carrier(
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

/// A write rooted at the parameter replaces its carrier only when it names
/// the whole parameter place; a projected write lands inside the referent.
fn replaces_carrier(place: &validation::CanonicalPlace, source_symbol: SymbolHandle) -> bool {
    place.root == facts::PlaceRoot::Symbol(source_symbol) && place.segments.is_empty()
}

/// A completed call that received the bare carrier through a borrowed formal
/// held only a reborrow of the referent: it can change the referent, whole or
/// in part, but has no route to the caller's carrier binding. Byte views are
/// the same case through their view formals. This does not admit assignments
/// or restore live loans.
fn preceding_reborrow_preserves_carrier(
    program: &dyn ::validation::CheckingInput,
    borrow: &checked_trees::BorrowFacts,
    state: &checked_trees::StateBorrowFact,
    call: &checked_trees::BorrowCallFact,
    source_symbol: SymbolHandle,
) -> bool {
    let Some(source_state) =
        validation::find_state_in_machine(program, state.machine_symbol, state.state_symbol)
    else {
        return false;
    };
    let Some(source) = program.state_parameter_by_symbol(source_state, source_symbol) else {
        return false;
    };
    let Some(validation::CallSite::Statement(authored)) = validation::find_call_site(
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
    let Some(parameters) = validation::call_target_parameters(program, call.target_symbol) else {
        return false;
    };
    let arguments = program
        .statement_table
        .expression_handles(authored.arguments);
    // Runtime receivers supply `self` separately; static carrier calls may
    // spell it as the first argument. Rejoin the same explicit telescope as
    // ordinary call planning before judging a completed operand's reborrow.
    let parameters = parameters.iter();
    if parameters.clone().count() != arguments.len() {
        return false;
    }
    let mut matching = parameters.zip(arguments).filter(|(_, expression)| {
        matches!(program.expression_table.expression(**expression), ExpressionNode::Name(name) if name.symbol == source_symbol)
    });
    let Some((parameter, expression)) = matching.next() else {
        return false;
    };
    if (matching.next().is_some())
        || parameter.is_const
        || !(crate::execution::terminal_unit::calls::boundary_admission::fixed_byte_array_view_is_admitted(
            program,
            source.type_reference,
            parameter.type_reference,
        ) || [source.type_reference, parameter.type_reference]
            .into_iter()
            .all(|reference| {
                structural_access_for_type_reference(program, reference)
                    == Some(CheckedStructuralAccess::MutableBorrow)
                    && byte_sequence_carrier(program, reference, &[])
                        .is_some_and(checked_trees::CheckedByteSequenceCarrier::is_borrowed)
            })
            || (structural_access_for_type_reference(program, source.type_reference)
                == Some(CheckedStructuralAccess::MutableBorrow)
                && matches!(
                    structural_access_for_type_reference(program, parameter.type_reference),
                    Some(
                        CheckedStructuralAccess::MutableBorrow
                            | CheckedStructuralAccess::SharedBorrow
                            | CheckedStructuralAccess::WriteOnlyBorrow
                    )
                )))
    {
        return false;
    }
    let Some(name) = program.expression_table.name_path_node(*expression) else {
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
        .filter(|access| access.root == facts::PlaceRoot::Symbol(source_symbol));
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

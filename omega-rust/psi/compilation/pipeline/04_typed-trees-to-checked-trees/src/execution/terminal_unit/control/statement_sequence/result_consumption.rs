//! Result and source consumption: structural result bindings are appended
//! per operation, retained results feed a call argument, and value-call
//! sources place the operation their operand evaluates into.

use super::super::super::{CheckedUnitStructuralArgumentSourcePlan, PermissionProvenance};
use super::super::{
    CheckFacts, CheckedStructuralAccess, CheckedUnitEffectOperationPlan,
    CheckedUnitStructuralResultBindingPlan, Multiplicity, PermissionEventSource, StatementNode,
    SymbolHandle,
};
use checked_trees::CheckedUnitStructuralReturnPlan;

/// A materialized carrier owns the same captured loan group whether formed
/// locally or returned by a call. Statement and exit scheduling must retire
/// that exact holder, not restrict retirement to one producer family.
/// The latest release boundary any loan in the group reaches: a descriptor
/// retires after every alternative source and every descendant can end.
fn latest_loan_release_boundary(
    facts: &CheckFacts,
    loans: arena::HandleSpan<checked_trees::BorrowLoanFact>,
    release: impl Fn(arena::Handle<checked_trees::BorrowLoanFact>) -> Option<u32>,
) -> Option<u32> {
    facts
        .borrow
        .loan_group(loans)?
        .try_fold(0u32, |boundary, (loan, _)| {
            Some(boundary.max(release(loan)?))
        })
}

/// A prior constructor or call transfer already moved this descriptor's
/// custody to another holder.
fn custody_transferred(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    results: &[(CheckedUnitStructuralResultBindingPlan, facts::PlaceRoot)],
    binding_ordinal: u32,
    operations: &[CheckedUnitEffectOperationPlan],
) -> Option<bool> {
    Some(
        super::reference_retirement::transferred_paths(
            program,
            facts,
            results,
            binding_ordinal,
            operations,
        )?
        .iter()
        .any(|path| path.is_empty()),
    )
}

fn captured_reference_result<'operation>(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    results: &[(CheckedUnitStructuralResultBindingPlan, facts::PlaceRoot)],
    operation: &'operation CheckedUnitEffectOperationPlan,
) -> Option<(
    &'operation CheckedUnitStructuralResultBindingPlan,
    arena::HandleSpan<checked_trees::BorrowLoanFact>,
)> {
    match operation {
        CheckedUnitEffectOperationPlan::EstablishReference { result, .. } => {
            super::reference_retirement::local_reference_loans(
                facts, machine, state, result, results,
            )
            .map(|loans| (result, loans))
        }
        CheckedUnitEffectOperationPlan::EstablishStructuralValue { result, value, .. }
            if facts.values.structural_values.nodes.is_valid(*value)
                && validation::reference_result_custody::stored_loan_parts(
                    program,
                    facts
                        .values
                        .structural_values
                        .nodes
                        .get(*value)
                        .type_reference,
                )
                .is_some() =>
        {
            // A transported bare descriptor has its own captured occurrence,
            // just like an explicitly formed one. Releasing it cannot consume
            // the supplying holder or wait for a copy as though it reborrowed.
            super::reference_retirement::local_reference_loans(
                facts, machine, state, result, results,
            )
            .map(|loans| (result, loans))
        }
        _ => operation.call().and_then(|call| {
            let custody = call.structural_custody()?;
            let result = call.structural_result()?;
            (!custody.reference_loans.is_empty()).then_some((result, custody.reference_loans))
        }),
    }
}

/// End captured reference custody at entry to the next authored statement.
/// Whole carriers use ordinary affine edge disposal, which retires every leaf;
/// reference results retain one release of their complete group. Reverse production order
/// closes descendants before earlier parent carriers at a shared boundary.
#[allow(clippy::too_many_arguments)]
pub(in crate::execution::terminal_unit) fn append_reference_releases(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    statement_index: u32,
    results: &[(CheckedUnitStructuralResultBindingPlan, facts::PlaceRoot)],
    operations: &mut Vec<CheckedUnitEffectOperationPlan>,
) -> Option<()> {
    let authored_state = program.state_by_symbol(state)?;
    let transported_loans = super::reference_retirement::transported_loans(
        program,
        facts,
        machine,
        authored_state,
        results,
        operations,
    )?;
    let mut pending = Vec::new();
    let mut retired = Vec::new();
    for operation in operations.iter().rev() {
        let reference_result =
            captured_reference_result(program, facts, machine, state, results, operation);
        if let Some((result, loans)) = reference_result {
            if custody_transferred(program, facts, results, result.binding_ordinal, operations)? {
                // The receiving constructor or call owns the descriptor now.
                // Its former holder cannot release that transferred custody.
                continue;
            }

            // A descriptor retires once, after every alternative source and
            // every descendant can end. Never release an arbitrary candidate
            // early or emit one destructive operation per possible referent.
            let boundary = latest_loan_release_boundary(facts, loans, |loan| {
                super::super::super::reference_results::release_boundary(
                    program, facts, machine, state, loan,
                )
            })?;
            let retirement = super::reference_retirement::Retirement {
                program,
                facts,
                machine,
                state: authored_state,
                statement_index,
                results,
                operations,
                pending: &pending,
                transported_loans: &transported_loans,
            };
            if boundary <= statement_index && facts.borrow.loan_group(loans)?.all(|(loan, _)| retirement.children_retired(loan)) && !operations.iter().any(|operation| {
                matches!(operation, CheckedUnitEffectOperationPlan::ReleaseReference { loans: released, .. } if *released == loans)
            }) {
                pending.push(CheckedUnitEffectOperationPlan::ReleaseReference {
                    statement_index, binding_ordinal: result.binding_ordinal,
                    path: Vec::new(), defer_to_exit: false, loans,
                });
                // Explicit retirement consumes the descriptor now. Its
                // producer cannot also discard that carrier at state exit.
                if operation.discardable_structural_result().is_some() {
                    retired.push(result.binding_ordinal);
                }
            }
            continue;
        }
        let Some((result, true)) = operation.discardable_structural_result() else {
            continue;
        };
        if result.multiplicity != Multiplicity::Affine || result.statement_index >= statement_index
        {
            continue;
        }
        let Some(StatementNode::LocalData(local)) = program
            .statement_table
            .statements(authored_state.statement_nodes)
            .get(result.statement_index as usize)
        else {
            continue;
        };
        // Anonymous operands can share their enclosing local's statement and
        // type. Only the binding registered to that local owns its loan roster.
        if !results.iter().any(|(binding, root)| {
            binding == result && *root == facts::PlaceRoot::Symbol(local.symbol)
        }) {
            continue;
        }
        // Transfers remove only their selected subtree. Other stored leaves
        // may still end at their own checked boundary; a whole-value transfer
        // leaves no initializer custody here at all.
        let transferred_paths = super::reference_retirement::transferred_paths(
            program,
            facts,
            results,
            result.binding_ordinal,
            operations,
        )?;
        if transferred_paths.iter().any(|path| path.is_empty()) {
            continue;
        }
        // A replacement creates a different captured occurrence. This roster
        // describes the initializer, so leave a replaced home to its existing
        // replacement/exit custody until that occurrence is represented here.
        if facts.borrow.loans.iter().any(|(_, loan)| {
            loan.owner_symbol == local.symbol
                && loan.statement_index != result.statement_index as usize
                && loan.statement_index < statement_index as usize
        }) {
            continue;
        }
        let Some(boundary) = validation::reference_result_custody::whole_local_release_boundary(
            program,
            facts,
            machine,
            authored_state,
            result.statement_index,
        ) else {
            continue;
        };
        let retains_siblings = boundary > statement_index || !transferred_paths.is_empty();
        let retirement = super::reference_retirement::Retirement {
            program,
            facts,
            machine,
            state: authored_state,
            statement_index,
            results,
            operations,
            pending: &pending,
            transported_loans: &transported_loans,
        };
        let local_loans = validation::reference_result_custody::local_record_loans(
            program,
            facts,
            machine,
            authored_state,
            result.statement_index,
        )?;
        if !retains_siblings
            && local_loans.iter().any(|(_, group)| {
                !facts.borrow.loan_group(*group).is_some_and(|mut loans| {
                    loans.all(|(loan, _)| retirement.children_retired(loan))
                })
            })
        {
            continue;
        }
        let leaf_releases = if retains_siblings {
            local_loans
                .into_iter()
                .rev()
                .filter(|(source, group)| {
                    facts.borrow.loan_group(*group).is_some_and(|mut loans| {
                        loans.all(|(loan, _)| retirement.children_retired(loan))
                    }) && !transferred_paths
                        .iter()
                        .any(|path| path.starts_with(&source.path) || source.path.starts_with(path))
                        && latest_loan_release_boundary(facts, *group, |loan| {
                            validation::reference_result_custody::release_statement(
                                facts, machine, state, loan,
                            )
                        })
                        .is_some_and(|boundary| boundary <= statement_index)
                        && !operations.iter().chain(&pending).any(|operation| {
                            matches!(operation, CheckedUnitEffectOperationPlan::ReleaseReference {
                            loans: released, ..
                        } if *released == *group)
                        })
                })
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        if retains_siblings && leaf_releases.is_empty() {
            continue;
        }
        // Lexically expired leaves can remain stored in a live aggregate.
        // Retire their actual holders before the storage they borrow. The
        // reverse producer walk makes pending cleanups available to older
        // owners here, so the same join also orders transitive dependencies.
        // A deferred owner is retried after its earliest permitted boundary.
        let storage_loans = validation::reference_result_custody::local_storage_loans(
            facts,
            machine,
            state,
            local.symbol,
        )?;
        if storage_loans
            .into_iter()
            .any(|handle| !retirement.holder_retired(handle))
        {
            continue;
        }
        // A leaf's checked weakening retires only that descriptor. Its scalar
        // siblings remain established storage; the later whole death schedule
        // is completed from the exact released-path complement during lowering.
        if retains_siblings {
            for (source, group) in leaf_releases {
                pending.push(CheckedUnitEffectOperationPlan::ReleaseReference {
                    statement_index,
                    binding_ordinal: result.binding_ordinal,
                    path: source.path,
                    defer_to_exit: false,
                    loans: group,
                });
            }
            continue;
        }
        retired.push(result.binding_ordinal);
        pending.push(CheckedUnitEffectOperationPlan::ContinuationCleanup {
            boundary: checked_trees::CheckedUnitCleanupBoundary::StatementEntry { statement_index },
            cleanup_actions: vec![checked_trees::CheckedNormalCleanupAction::Discard(
                checked_trees::CheckedUnitPartialAffineDiscardPlan {
                    source:
                        checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                            binding_ordinal: result.binding_ordinal,
                        },
                    path: Vec::new(),
                    type_identity: result.type_identity.clone(),
                    case_conditions: Default::default(),
                },
            )],
        });
    }
    for binding in retired {
        let (_, discard_on_return) = operations.iter_mut().find_map(|operation| {
            operation
                .discardable_structural_result_mut()
                .filter(|(result, _)| result.binding_ordinal == binding)
        })?;
        *discard_on_return = false;
    }
    operations.extend(pending);
    Some(())
}

// Loans released by the state's natural exit carry their weakening at a
// boundary the per-statement loop never visits (the transition statement, or
// one past the last ordinary statement). The completion still needs the
// `ReleaseReference` op so the result-binding cleanup sees the disposal.
pub(in crate::execution::terminal_unit) fn append_exit_reference_releases(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    returned: Option<&CheckedUnitStructuralReturnPlan>,
    results: &[(CheckedUnitStructuralResultBindingPlan, facts::PlaceRoot)],
    operations: &mut Vec<CheckedUnitEffectOperationPlan>,
) -> Option<()> {
    let mut pending = Vec::new();
    for operation in operations.iter() {
        if let Some((result, loans)) =
            captured_reference_result(program, facts, machine, state, results, operation)
        {
            // The exit boundary consumes the same current custody as an
            // ordinary statement. A prior constructor/call transfer already
            // moved this descriptor to another holder.
            if custody_transferred(program, facts, results, result.binding_ordinal, operations)? {
                continue;
            }
            // Completion transfers this exact carrier and its retained ingress
            // rather than releasing it before the return. Earlier weakening
            // still belongs to statement scheduling; only a loan live through
            // this state's final statement can cross its result boundary.
            if returned.is_some_and(|returned| {
                matches!(returned.source,
                    CheckedUnitStructuralArgumentSourcePlan::StructuralResult { binding_ordinal }
                        if binding_ordinal == result.binding_ordinal)
                    && returned.type_identity == result.type_identity
                    && returned.multiplicity == result.multiplicity
                    && !returned.reference_sources.is_empty()
            }) {
                let authored = program.state_by_symbol(state)?;
                let statement_count = program
                    .statement_table
                    .statements(authored.statement_nodes)
                    .len();
                if !facts.borrow.loan_group(loans)?.all(|(loan, _)| {
                    super::super::super::reference_results::release_boundary(
                        program, facts, machine, state, loan,
                    )
                    .is_some_and(|boundary| boundary as usize >= statement_count)
                }) {
                    return None;
                }
                continue;
            }
            let boundary = latest_loan_release_boundary(facts, loans, |loan| {
                super::super::super::reference_results::release_statement(
                    facts, machine, state, loan,
                )
            })?;
            if !operations.iter().any(|operation| {
                matches!(operation, CheckedUnitEffectOperationPlan::ReleaseReference { loans: released, .. } if *released == loans)
            }) && !pending.iter().any(|operation: &(u32, CheckedUnitEffectOperationPlan)| {
                matches!(operation, (_, CheckedUnitEffectOperationPlan::ReleaseReference { loans: released, .. }) if *released == loans)
            }) {
                pending.push((boundary, CheckedUnitEffectOperationPlan::ReleaseReference {
                    statement_index: boundary,
                    binding_ordinal: result.binding_ordinal,
                    path: Vec::new(),
                    defer_to_exit: matches!(
                        program.statement_table.statements(
                            program.state_by_symbol(state)?.statement_nodes,
                        ).get(boundary.checked_sub(1)? as usize)?,
                        StatementNode::Transition(_)
                    ),
                    loans,
                }));
            }
        }
    }
    pending.sort_by_key(|(boundary, _)| *boundary);
    for (_, release) in &pending {
        if let CheckedUnitEffectOperationPlan::ReleaseReference {
            binding_ordinal,
            defer_to_exit: false,
            ..
        } = release
            && let Some((_, discard)) = operations.iter_mut().find_map(|operation| {
                operation
                    .discardable_structural_result_mut()
                    .filter(|(result, _)| result.binding_ordinal == *binding_ordinal)
            })
        {
            // Calls also carry an automatic discard flag. EstablishReference
            // has none: its explicit release is already the sole disposition.
            *discard = false;
        }
    }
    operations.extend(pending.into_iter().map(|(_, operation)| operation));
    Some(())
}

/// A selected continuation owns both the chosen result and the conditional
/// complement until the actual death edge. Original producers must not also
/// publish unconditional return drops for those same owners.
pub(in crate::execution::terminal_unit) fn retain_selected_sources(
    facts: &CheckFacts,
    state: SymbolHandle,
    statement: u32,
    sources: &[(CheckedUnitStructuralResultBindingPlan, facts::PlaceRoot)],
    operations: &mut [CheckedUnitEffectOperationPlan],
) -> Option<()> {
    let Some((_, receipt)) = facts.flow.ownership.owned_selection_at(state, statement) else {
        return Some(());
    };
    for source in facts
        .flow
        .ownership
        .selection_sources
        .span(receipt.sources)?
    {
        // A state-entry source (an owned parameter) has no producing
        // operation to unmark: its residual custody already rides the
        // receipt's selected edges instead of an unconditional return drop.
        if matches!(
            source.provenance,
            PermissionProvenance::Established {
                source: PermissionEventSource::StateEntry,
                ..
            }
        ) {
            continue;
        }
        let (binding, _) = sources
            .iter()
            .find(|(_, root)| *root == facts::PlaceRoot::Symbol(source.symbol))?;
        let mut producers = operations.iter_mut().filter_map(|operation| {
            operation
                .discardable_structural_result_mut()
                .filter(|(result, _)| result.binding_ordinal == binding.binding_ordinal)
        });
        let (result, discard) = producers.next()?;
        if producers.next().is_some()
            || result.multiplicity != Multiplicity::Affine
            || result.statement_index != source.statement_ordinal
            || !*discard
        {
            return None;
        }
        *discard = false;
    }
    Some(())
}

/// Discharge only producers owned by this sequence. Call-owned operand
/// preparation has its own result namespace segment; its enclosing sequence
/// owns earlier locals. Each owner updates its own disposal debt, while source
/// custody independently rejoins every operand across that boundary.
pub(in crate::execution::terminal_unit) fn consume_retained_results(
    operations: &mut [CheckedUnitEffectOperationPlan],
    consumer: &CheckedUnitEffectOperationPlan,
) -> Option<()> {
    let Some(call) = consumer.call() else {
        return Some(());
    };
    for argument in &call.structural_arguments {
        let Some(binding) = argument.source_structural_result_binding_ordinal() else {
            continue;
        };
        if operations.iter().any(|operation| {
            operation
                .structural_result_binding()
                .is_some_and(|result| result.binding_ordinal == binding)
        }) {
            consume_result(
                operations,
                binding,
                argument.access,
                !argument.path.is_empty(),
            )?;
        }
    }
    Some(())
}

pub(in crate::execution::terminal_unit) fn consume_value_call_sources(
    facts: &CheckFacts,
    results: &[(CheckedUnitStructuralResultBindingPlan, facts::PlaceRoot)],
    operations: &mut [CheckedUnitEffectOperationPlan],
    calls: &[checked_trees::CheckedStructuralValueCall],
) -> Option<()> {
    for call in calls {
        for operation in call
            .operands()
            .iter()
            .chain(std::iter::once(call.operation()))
            .flat_map(CheckedUnitEffectOperationPlan::with_value_calls)
        {
            if let CheckedUnitEffectOperationPlan::EstablishStructuralValue { value, .. } =
                operation
            {
                consume_value_places(facts, *value, results, operations)?;
            }
            consume_retained_results(operations, operation)?;
        }
    }
    Some(())
}

pub(in crate::execution::terminal_unit) fn consume_results(
    operations: &mut [CheckedUnitEffectOperationPlan],
    consumer: &CheckedUnitEffectOperationPlan,
) -> Option<()> {
    if let Some(call) = consumer.call() {
        for (binding_ordinal, access, projected) in
            call.structural_arguments.iter().filter_map(|argument| {
                argument
                    .source_structural_result_binding_ordinal()
                    .map(|ordinal| (ordinal, argument.access, !argument.path.is_empty()))
            })
        {
            consume_result(operations, binding_ordinal, access, projected)?;
        }
    }
    // A whole-result field store consumes the minted binding the same way a
    // call argument does: Owned access retires the producer's disposal debt.
    if let Some(value) = consumer.installed_value()
        && let Some(binding_ordinal) = value.source_structural_result_binding_ordinal()
    {
        consume_result(
            operations,
            binding_ordinal,
            value.access,
            !value.path.is_empty(),
        )?;
    }
    Some(())
}

pub(in crate::execution::terminal_unit) fn consume_result(
    operations: &mut [CheckedUnitEffectOperationPlan],
    binding_ordinal: u32,
    access: CheckedStructuralAccess,
    projected: bool,
) -> Option<()> {
    let mut producers = operations.iter_mut().filter(|operation| {
        operation
            .structural_result_binding()
            .is_some_and(|result| result.binding_ordinal == binding_ordinal)
    });
    let producer = producers.next()?;
    if producers.next().is_some() {
        return None;
    }
    // A detached field owns its moved custody, not a newly minted automatic
    // return discard. The ordinary call/store may consume that whole value;
    // source ownership and independent replay still account for every use.
    // Its original place's restoration debt remains in OpenWindows.
    if matches!(
        producer,
        CheckedUnitEffectOperationPlan::MoveStructuralField { .. }
    ) {
        return (access != CheckedStructuralAccess::Owned || !projected).then_some(());
    }
    let reference_carrier = matches!(
        producer,
        CheckedUnitEffectOperationPlan::EstablishReference { .. }
    ) || producer
        .call()
        .and_then(checked_trees::CheckedUnitCallPlan::structural_custody)
        .is_some_and(|custody| !custody.reference_loans.is_empty());
    if reference_carrier {
        // Formation and call results own the same permission carrier. Lending
        // its referent leaves disposal with the producer; it does not require
        // an owned-result discard flag or transfer the borrowed backing.
        // Argument admission retains exact access, path and loan lifetime.
        // A whole owned operand transfers the descriptor itself, including
        // its loan and later retirement, to the receiving value. Lending the
        // referent keeps that descriptor in this holder instead.
        return ((!projected && access == CheckedStructuralAccess::Owned)
            || access == CheckedStructuralAccess::SharedBorrow
            || (projected && access == CheckedStructuralAccess::MutableBorrow))
            .then_some(());
    }
    // Unrestricted whole-value arguments copy their payload. They retain
    // a live producer without acquiring an affine disposal obligation. A
    // boundary structural result is not such a copy.
    let unrestricted_copy = match &*producer {
        CheckedUnitEffectOperationPlan::Call(call) => {
            call.is_module()
                && call
                    .structural_result()
                    .is_some_and(|result| result.multiplicity == Multiplicity::Unrestricted)
        }
        CheckedUnitEffectOperationPlan::EstablishScalarArray { result, .. }
        | CheckedUnitEffectOperationPlan::EstablishStructuralValue { result, .. }
        | CheckedUnitEffectOperationPlan::EstablishByteSequence { result, .. }
        | CheckedUnitEffectOperationPlan::EstablishViewSubslice { result, .. } => {
            result.multiplicity == Multiplicity::Unrestricted
        }
        _ => false,
    };
    if unrestricted_copy {
        if access == CheckedStructuralAccess::Owned && projected {
            return None;
        }
        return Some(());
    }
    if matches!(
        access,
        CheckedStructuralAccess::SharedBorrow
            | CheckedStructuralAccess::MutableBorrow
            | CheckedStructuralAccess::WriteOnlyBorrow
    ) {
        // A borrow argument rejoins the producer's loan, not its disposal
        // debt: borrow custody rides the selected edge's claim partition, so
        // a result whose return-discard the edge already owns, or a producer
        // that never mints one (an exclusive view subslice), borrows the same
        // as one still carrying the flag.
        return Some(());
    }
    let (result, discard_result_on_return) = producer.discardable_structural_result_mut()?;
    if result.multiplicity != Multiplicity::Affine {
        // Linear results never acquire automatic disposal debt. Moving a
        // whole result continues its checked call claim frontier; source flow
        // and independent Terminal replay reject a second use of that claim.
        if result.multiplicity == Multiplicity::Linear
            && !*discard_result_on_return
            && access == CheckedStructuralAccess::Owned
            && !projected
        {
            return Some(());
        }
        return None;
    }
    // This flag schedules disposal; it is not the owner's liveness bit.
    // Composed states clear it before planning their selected edges because
    // those edges own the exact cleanup/transfer partition. An affine move
    // still consumes the same producer there. Source permission events reject
    // duplicate moves and uses after consumption, regardless of this flag.
    // A projected transfer leaves its root owner alive until the exact
    // complement is committed on the consumer continuation.
    if !projected {
        *discard_result_on_return = false;
    }
    Some(())
}

pub(in crate::execution::terminal_unit) fn consume_value_places(
    facts: &CheckFacts,
    root: checked_trees::CheckedStructuralValueHandle,
    results: &[(CheckedUnitStructuralResultBindingPlan, facts::PlaceRoot)],
    operations: &mut [CheckedUnitEffectOperationPlan],
) -> Option<()> {
    let plans = &facts.values.structural_values;
    let mut pending = vec![root];
    let mut visited = Vec::new();
    while let Some(value) = pending.pop() {
        if !plans.nodes.is_valid(value) || visited.contains(&value) {
            return None;
        }
        visited.push(value);
        match &plans.nodes.get(value).kind {
            checked_trees::CheckedStructuralValueKind::Place(argument) => {
                if let checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
                    symbol,
                } = argument.source
                {
                    let mut matching = results
                        .iter()
                        .filter(|(_, source)| *source == facts::PlaceRoot::Symbol(symbol));
                    let (result, _) = matching.next()?;
                    if matching.next().is_some() || result.type_identity != argument.type_identity {
                        return None;
                    }
                    consume_result(
                        operations,
                        result.binding_ordinal,
                        argument.access,
                        !argument.path.is_empty(),
                    )?;
                }
            }
            checked_trees::CheckedStructuralValueKind::Record { fields, .. }
            | checked_trees::CheckedStructuralValueKind::StructuralCase { fields, .. } => {
                for field in plans.record_fields.span(*fields)? {
                    if let checked_trees::CheckedStructuralRecordFieldValue::Structural(value) =
                        field.value
                    {
                        pending.push(value);
                    }
                }
            }
            checked_trees::CheckedStructuralValueKind::Dispatch { arms, .. } => {
                pending.extend(plans.dispatch_arms.span(*arms)?.iter().map(|arm| arm.value));
            }
            checked_trees::CheckedStructuralValueKind::Projection { source, .. } => {
                // The selected edge moves a projected child: its root owner
                // remains live for the residual complement that edge commits.
                if let checked_trees::CheckedStructuralValueKind::Place(argument) =
                    &plans.nodes.get(*source).kind
                    && let checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
                        symbol,
                    } = argument.source
                {
                    let mut matching = results
                        .iter()
                        .filter(|(_, source)| *source == facts::PlaceRoot::Symbol(symbol));
                    let (result, _) = matching.next()?;
                    if matching.next().is_some() || result.type_identity != argument.type_identity {
                        return None;
                    }
                    consume_result(operations, result.binding_ordinal, argument.access, true)?;
                }
            }
            checked_trees::CheckedStructuralValueKind::FixedArray { elements } => {
                pending.extend(elements.iter().copied());
            }
            checked_trees::CheckedStructuralValueKind::Reference { .. }
            | checked_trees::CheckedStructuralValueKind::BorrowedSliceView { .. }
            | checked_trees::CheckedStructuralValueKind::MutableSliceView { .. }
            | checked_trees::CheckedStructuralValueKind::Call { .. }
            | checked_trees::CheckedStructuralValueKind::CopiedStructuralPlace { .. }
            | checked_trees::CheckedStructuralValueKind::ViewElementCopy { .. }
            | checked_trees::CheckedStructuralValueKind::ZeroedScalarArray { .. }
            | checked_trees::CheckedStructuralValueKind::ScalarArray { .. }
            | checked_trees::CheckedStructuralValueKind::ZeroedStructural { .. }
            | checked_trees::CheckedStructuralValueKind::ByteLiteralView { .. }
            | checked_trees::CheckedStructuralValueKind::Case(_) => {}
        }
    }
    Some(())
}

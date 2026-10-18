//! Stored descriptors retain the receiving occurrence, not the RHS producer's
//! original slot. The checked store supplies exact storage geometry; the loan
//! ledger supplies the complete formation group and its weakening boundary.

use super::{
    BorrowLoanFact, CheckedUnitEffectOperationPlan, CheckedUnitStructuralArgumentSourcePlan,
    CheckedUnitStructuralPathSegment, Retirement, SymbolHandle, transfers,
};

/// Resolve a current named binding, keeping authored formal positions separate
/// from the dense structural argument lane. This names storage, not proof that
/// a descriptor was installed there; `named_place_loans` requires its activation.
pub(super) fn source_owner(
    program: &dyn validation::CheckingInput,
    state: &typed_trees::state::State,
    source: &CheckedUnitStructuralArgumentSourcePlan,
) -> Option<SymbolHandle> {
    match source {
        CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol }
        | CheckedUnitStructuralArgumentSourcePlan::StructuralLocalBorrow { symbol, .. } => {
            Some(*symbol)
        }
        CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } => program
            .state_parameters(state)
            .iter()
            .filter(|parameter| {
                crate::execution::terminal_unit::types::structural_parameter_candidate(
                    program, parameter,
                )
            })
            .nth(*parameter_index as usize)
            .map(|parameter| parameter.symbol),
        _ => None,
    }
}

fn captured_destination(
    program: &dyn validation::CheckingInput,
    facts: &checked_trees::CheckFacts,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    statement_index: u32,
    selection: &[CheckedUnitStructuralPathSegment],
) -> Option<validation::CanonicalPlace> {
    let flow = facts.flow.control.state(machine, state.symbol)?;
    let statement = facts.flow.state_statement(flow, statement_index as usize)?;
    let control = &facts.flow.control;
    if !control.operands.is_valid(statement.store_target) {
        return None;
    }
    let captured = control.operands.get(statement.store_target);
    let [destination] = control.operand_referents.span(captured.referents)? else {
        // Several possible destinations do not identify one replaced holder.
        return None;
    };
    let captured_segments = control
        .operand_referent_segments
        .span(destination.segments)?;
    let selection = validation::reference_result_custody::loan_place_segments(
        program,
        program
            .statement_table
            .statement_at(state, statement_index as usize)?
            .as_assignment()?
            .receiving_type,
        selection,
    )?;
    let segments = facts::project_captured_view_segments(captured_segments, &selection)?;
    Some(validation::CanonicalPlace {
        root: destination.root,
        segments,
    })
}

pub(super) fn destination(
    program: &dyn validation::CheckingInput,
    facts: &checked_trees::CheckFacts,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    statement_index: u32,
    selection: &[CheckedUnitStructuralPathSegment],
) -> Option<(SymbolHandle, Vec<CheckedUnitStructuralPathSegment>)> {
    let destination =
        captured_destination(program, facts, machine, state, statement_index, selection)?;
    let owner = destination.root.as_symbol()?;
    let path = crate::borrow::owner_path_from_place_segments(program, &destination.segments);
    let reference = validation::reference_result_custody::storage_owner_type_reference(
        program,
        state,
        owner,
        statement_index,
    )?;
    Some((
        owner,
        super::transport::carrier_path(program, reference, &path)?,
    ))
}

pub(super) fn receiver(
    program: &dyn validation::CheckingInput,
    facts: &checked_trees::CheckFacts,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    operation: &CheckedUnitEffectOperationPlan,
) -> Option<(SymbolHandle, u32, Vec<CheckedUnitStructuralPathSegment>)> {
    let CheckedUnitEffectOperationPlan::Store(store) = operation else {
        return None;
    };
    store.installed_value()?;
    let (symbol, path) = destination(
        program,
        facts,
        machine,
        state,
        store.statement_index,
        &store.captured_selection,
    )?;
    Some((symbol, store.statement_index, path))
}

pub(super) fn loans(
    retirement: &Retirement<'_>,
    operation: &CheckedUnitEffectOperationPlan,
) -> Option<Vec<validation::reference_result_custody::RecordLeafLoans>> {
    let (owner, formation, path) = receiver(
        retirement.program,
        retirement.facts,
        retirement.machine,
        retirement.state,
        operation,
    )?;
    validation::reference_result_custody::named_place_loans(
        retirement.program,
        retirement.facts,
        retirement.machine,
        retirement.state,
        owner,
        formation,
        &path,
    )
}

/// A no-displacement replacement ends its resident descriptors in the store
/// itself. There need not be a cleanup operation for an intrinsically
/// discardable reference. Join the exact weakening to the captured destination;
/// the executable root may be a pointer into a different storage owner.
pub(super) fn replaces(
    retirement: &Retirement<'_>,
    store: &checked_trees::CheckedStorePlan,
    handle: arena::Handle<BorrowLoanFact>,
) -> bool {
    let replaced = (|| {
        match store.destination.leaf {
            checked_trees::CheckedStoreLeaf::StructuralStore {
                opening_binding: None,
                ..
            }
            | checked_trees::CheckedStoreLeaf::OwnedRebind { .. } => {}
            _ => return None,
        }
        store.installed_value()?;
        let loan = retirement.facts.borrow.loans.get(handle);
        let statement = store.statement_index as usize;
        if !retirement.order.precedes(loan.statement_index, statement)
            || !retirement
                .order
                .precedes(statement, retirement.statement_index as usize)
        {
            return None;
        }
        let weakening = validation::reference_result_custody::loan_weakening(
            retirement.facts,
            retirement.machine,
            retirement.state.symbol,
            handle,
        )?;
        if weakening.reason != checked_trees::FlowBorrowWeakeningReason::LocalReassigned
            || weakening.source
                != (checked_trees::FlowInvalidationSource::Statement {
                    statement_index: statement,
                })
        {
            return None;
        }
        let destination = captured_destination(
            retirement.program,
            retirement.facts,
            retirement.machine,
            retirement.state,
            store.statement_index,
            &store.captured_selection,
        )?;
        Some(
            destination.root == loan.storage_owner
                && crate::borrow::place_path_covers_owner(
                    retirement.program,
                    &destination.segments,
                    retirement.facts.borrow.loan_owner_path(loan),
                ),
        )
    })();
    replaced == Some(true)
}

/// A store's installed result retains its actual producer graph. Anonymous
/// construction is not another named holder, but fresh borrows in that graph
/// still pin their immediate supplying descriptor until the stored leaf ends.
pub(super) fn construction<'a>(
    retirement: &Retirement<'a>,
    child: arena::Handle<BorrowLoanFact>,
    operation: &CheckedUnitEffectOperationPlan,
) -> Option<(
    &'a CheckedUnitEffectOperationPlan,
    Vec<CheckedUnitStructuralPathSegment>,
)> {
    let (owner, formation, destination) = receiver(
        retirement.program,
        retirement.facts,
        retirement.machine,
        retirement.state,
        operation,
    )?;
    let loan = retirement.facts.borrow.loans.get(child);
    if loan.owner_symbol != owner
        || loan.statement_index != formation as usize
        || !loans(retirement, operation)?
            .iter()
            .any(|(_, group)| retirement.facts.borrow.loans_contain(*group, child))
    {
        return None;
    }
    let reference = validation::reference_result_custody::storage_owner_type_reference(
        retirement.program,
        retirement.state,
        owner,
        formation,
    )?;
    let path = super::transport::carrier_path(
        retirement.program,
        reference,
        retirement.facts.borrow.loan_owner_path(loan),
    )?;
    let installed = operation.installed_value()?;
    let binding = installed.source_structural_result_binding_ordinal()?;
    let mut producers = retirement.operations.iter().filter(|producer| {
        producer
            .structural_result_binding()
            .is_some_and(|result| result.binding_ordinal == binding)
    });
    let producer = producers.next()?;
    if producers.next().is_some() {
        return None;
    }
    let mut selected = installed.path.clone();
    selected.extend_from_slice(path.strip_prefix(destination.as_slice())?);
    Some((producer, selected))
}

/// Retire exact leaf groups without destroying scalar siblings. Replacement
/// disposal and later transfers already end or relocate an earlier occurrence;
/// neither authorizes releasing a new descriptor now occupying that same slot.
pub(in crate::execution::terminal_unit::control::statement_sequence) fn releases(
    retirement: &Retirement<'_>,
    operation: &CheckedUnitEffectOperationPlan,
) -> Option<Vec<CheckedUnitEffectOperationPlan>> {
    let Some((owner, formation, _)) = receiver(
        retirement.program,
        retirement.facts,
        retirement.machine,
        retirement.state,
        operation,
    ) else {
        return Some(Vec::new());
    };
    if !retirement
        .order
        .precedes(formation as usize, retirement.statement_index as usize)
    {
        return Some(Vec::new());
    }
    let reference = validation::reference_result_custody::storage_owner_type_reference(
        retirement.program,
        retirement.state,
        owner,
        formation,
    )?;
    // Installed fields keep the receiving owner's disposition contract. A
    // replacement does not turn a nominal/linear owner into cleanup-free
    // descriptor storage; its selected cleanup still borrows those fields.
    if !validation::has_contents(
        retirement.program,
        reference,
        &[],
        validation::ContentsRequirement::DescriptorCleanup,
    ) {
        return Some(Vec::new());
    }
    let Some(groups) = loans(retirement, operation) else {
        return Some(Vec::new());
    };
    let mut output = Vec::new();
    for (source, group) in groups.into_iter().rev() {
        let moved = transfers::from_storage(
            retirement.program,
            retirement.facts,
            retirement.results,
            CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol: owner },
            formation,
            &source.path,
            retirement.operations,
        )?;
        if moved.possible.iter().any(|transfer| {
            transfer.source.starts_with(&source.path) || source.path.starts_with(&transfer.source)
        }) {
            continue;
        }
        let mut captured = retirement.facts.borrow.loan_group(group)?;
        if !captured.all(|(loan, _)| {
            !retirement.disposed(loan)
                && retirement.children_retired(loan)
                && validation::reference_result_custody::release_statement(
                    retirement.program,
                    retirement.facts,
                    retirement.machine,
                    retirement.state.symbol,
                    loan,
                )
                .is_some_and(|boundary| {
                    retirement
                        .order
                        .at_or_before(boundary as usize, retirement.statement_index as usize)
                })
        }) {
            continue;
        }
        output.push(CheckedUnitEffectOperationPlan::ReleaseReference {
            statement_index: retirement.statement_index,
            source: CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol: owner },
            path: source.path,
            select_evaluated_storage: false,
            defer_to_exit: false,
            loans: group,
            selection_complements: Vec::new(),
            case_conditions: Default::default(),
        });
    }
    Some(output)
}

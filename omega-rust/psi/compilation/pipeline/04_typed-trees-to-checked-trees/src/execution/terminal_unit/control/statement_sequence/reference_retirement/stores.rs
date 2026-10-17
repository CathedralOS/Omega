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

pub(super) fn receiver<'operation>(
    program: &dyn validation::CheckingInput,
    state: &typed_trees::state::State,
    operation: &'operation CheckedUnitEffectOperationPlan,
) -> Option<(
    SymbolHandle,
    u32,
    &'operation [CheckedUnitStructuralPathSegment],
)> {
    let CheckedUnitEffectOperationPlan::Store(store) = operation else {
        return None;
    };
    store.installed_value()?;
    let symbol = match store.destination.root {
        checked_trees::CheckedStoreRoot::Local { symbol } => symbol,
        checked_trees::CheckedStoreRoot::Parameter { position } => {
            program
                .state_parameters(state)
                .get(position as usize)?
                .symbol
        }
    };
    Some((symbol, store.statement_index, &store.destination.path))
}

pub(super) fn loans(
    retirement: &Retirement<'_>,
    operation: &CheckedUnitEffectOperationPlan,
) -> Option<Vec<validation::reference_result_custody::RecordLeafLoans>> {
    let (owner, formation, path) = receiver(retirement.program, retirement.state, operation)?;
    validation::reference_result_custody::named_place_loans(
        retirement.program,
        retirement.facts,
        retirement.machine,
        retirement.state,
        owner,
        formation,
        path,
    )
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
    let (owner, formation, destination) =
        receiver(retirement.program, retirement.state, operation)?;
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
    selected.extend_from_slice(path.strip_prefix(destination)?);
    Some((producer, selected))
}

/// Retire exact leaf groups without destroying scalar siblings. Replacement
/// disposal and later transfers already end or relocate an earlier occurrence;
/// neither authorizes releasing a new descriptor now occupying that same slot.
pub(in crate::execution::terminal_unit::control::statement_sequence) fn releases(
    retirement: &Retirement<'_>,
    operation: &CheckedUnitEffectOperationPlan,
) -> Option<Vec<CheckedUnitEffectOperationPlan>> {
    let Some((owner, formation, _)) = receiver(retirement.program, retirement.state, operation)
    else {
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

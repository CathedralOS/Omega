//! Exact holder retirement and owned operands used by statement scheduling.
//! The checked value graph supplies constructor transfers; loan lineage supplies
//! child dependencies. Neither is inferred from the spelling of an initializer.

use super::{
    CheckFacts, CheckedStructuralAccess, CheckedUnitEffectOperationPlan,
    CheckedUnitStructuralArgumentPlan, CheckedUnitStructuralArgumentSourcePlan,
    CheckedUnitStructuralResultBindingPlan, SymbolHandle, TypedTrees,
};
use checked_trees::{BorrowLoanFact, CheckedStructuralValueKind, CheckedUnitStructuralPathSegment};

type Results = [(CheckedUnitStructuralResultBindingPlan, facts::PlaceRoot)];

pub(super) fn transferred_paths(
    facts: &CheckFacts,
    results: &Results,
    binding_ordinal: u32,
    operations: &[CheckedUnitEffectOperationPlan],
) -> Option<Vec<Vec<CheckedUnitStructuralPathSegment>>> {
    let belongs = |argument: &CheckedUnitStructuralArgumentPlan| {
        argument.access == CheckedStructuralAccess::Owned
            && match argument.source {
                CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                    binding_ordinal: source,
                } => source == binding_ordinal,
                CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol } => {
                    results.iter().any(|(result, root)| {
                        result.binding_ordinal == binding_ordinal
                            && *root == facts::PlaceRoot::Symbol(symbol)
                    })
                }
                _ => false,
            }
    };
    let mut paths = Vec::new();
    let values = &facts.values.structural_values;
    for operation in operations
        .iter()
        .flat_map(|operation| operation.with_value_calls())
    {
        for argument in operation.call_structural_arguments() {
            if belongs(argument) {
                paths.push(argument.path.clone());
            }
        }
        match operation {
            CheckedUnitEffectOperationPlan::StoreStructuralField { value, .. }
                if belongs(value) =>
            {
                paths.push(value.path.clone())
            }
            CheckedUnitEffectOperationPlan::EstablishStructuralValue { value, .. } => {
                let mut pending = vec![(*value, Vec::new())];
                let mut visited = Vec::new();
                while let Some((value, suffix)) = pending.pop() {
                    if !values.nodes.is_valid(value) {
                        return None;
                    }
                    if visited
                        .iter()
                        .any(|(prior, path)| *prior == value && *path == suffix)
                    {
                        continue;
                    }
                    visited.push((value, suffix.clone()));
                    match &values.nodes.get(value).kind {
                        CheckedStructuralValueKind::Place(argument) if belongs(argument) => {
                            let mut path = argument.path.clone();
                            path.extend(suffix);
                            paths.push(path);
                        }
                        CheckedStructuralValueKind::Projection { source, path, .. } => {
                            let mut projected = path.clone();
                            projected.extend(suffix);
                            pending.push((*source, projected));
                        }
                        CheckedStructuralValueKind::Record { fields, .. }
                        | CheckedStructuralValueKind::StructuralCase { fields, .. } => {
                            for field in values.record_fields.span(*fields)? {
                                if let checked_trees::CheckedStructuralRecordFieldValue::Structural(value) = field.value {
                                    pending.push((value, Vec::new()));
                                }
                            }
                        }
                        CheckedStructuralValueKind::FixedArray { elements } => {
                            pending.extend(elements.iter().map(|value| (*value, Vec::new())));
                        }
                        CheckedStructuralValueKind::Dispatch { arms, .. } => {
                            pending.extend(
                                values
                                    .dispatch_arms
                                    .span(*arms)?
                                    .iter()
                                    .map(|arm| (arm.value, suffix.clone())),
                            );
                        }
                        // Calls' arguments occur in the retained call schedule
                        // above. Captures and copies do not transfer an owner.
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    Some(paths)
}

pub(super) struct Retirement<'a> {
    pub(super) program: &'a TypedTrees,
    pub(super) facts: &'a CheckFacts,
    pub(super) machine: SymbolHandle,
    pub(super) state: &'a typed_trees::state::State,
    pub(super) statement_index: u32,
    pub(super) results: &'a Results,
    pub(super) operations: &'a [CheckedUnitEffectOperationPlan],
    pub(super) pending: &'a [CheckedUnitEffectOperationPlan],
}

impl Retirement<'_> {
    /// An immediate child must have ended its actual carrier, not merely
    /// reached its parent's lexical last use. Each child's own schedule obeys
    /// this same rule, so reverse production closes a chain deepest first.
    pub(super) fn children_retired(&self, parent: arena::Handle<BorrowLoanFact>) -> bool {
        let mut states = self.facts.borrow.states.iter().filter_map(|(_, state)| {
            (state.machine_symbol == self.machine && state.state_symbol == self.state.symbol)
                .then_some(state)
        });
        let Some(state) = states.next() else {
            return false;
        };
        if states.next().is_some() {
            return false;
        }
        let mut pending = vec![parent];
        let mut visited = Vec::new();
        while let Some(parent) = pending.pop() {
            if visited.contains(&parent) {
                return false;
            }
            visited.push(parent);
            for (handle, loan) in self.facts.borrow.loans.iter() {
                if self.facts.borrow.state_owns_loan(state, handle)
                    && loan.statement_index < self.statement_index as usize
                    && loan.lineage
                        == (checked_trees::BorrowLoanLineage::Reborrow {
                            parent_loan: parent,
                        })
                {
                    if !self.holder_retired(handle) {
                        return false;
                    }
                    // An unmaterialized alias may have weakened before its
                    // own child. Trace that child as well as stored holders.
                    pending.push(handle);
                }
            }
        }
        true
    }

    pub(super) fn holder_retired(&self, handle: arena::Handle<BorrowLoanFact>) -> bool {
        let loan = self.facts.borrow.loans.get(handle);
        if loan.statement_index >= self.statement_index as usize {
            return true;
        }
        let mut holders = self.results.iter().filter(|(holder, root)| {
            *root == facts::PlaceRoot::Symbol(loan.owner_symbol)
                && holder.statement_index as usize == loan.statement_index
        });
        let Some((holder, _)) = holders.next() else {
            // A captured place alias without an emitted descriptor ends at
            // checked weakening. Unknown and replaced holders remain live.
            let Some((ordinal, alias)) = self
                .program
                .state_local_declaration(self.state, loan.owner_symbol)
            else {
                return false;
            };
            let unmaterialized_alias = ordinal == loan.statement_index
                && self.facts.borrow.loan_owner_path(loan).is_empty()
                && validation::reference_result_custody::shared_place_borrowed_parts(
                    self.program,
                    alias.type_reference,
                )
                .is_some()
                && !self
                    .results
                    .iter()
                    .any(|(_, root)| *root == facts::PlaceRoot::Symbol(loan.owner_symbol))
                && u32::try_from(ordinal).ok().is_some_and(|ordinal| {
                    self.facts
                        .values
                        .structural_values
                        .root_for_expression(self.state.symbol, ordinal, alias.initial_value)
                        .is_none()
                });
            return unmaterialized_alias
                && validation::reference_result_custody::release_boundary(
                    self.program,
                    self.facts,
                    self.machine,
                    self.state.symbol,
                    handle,
                )
                .is_some_and(|boundary| boundary <= self.statement_index);
        };
        if holders.next().is_some() {
            return false;
        }
        self.operations.iter().chain(self.pending).any(|operation| match operation {
            CheckedUnitEffectOperationPlan::ReleaseReference {
                binding_ordinal, loan: released, defer_to_exit: false, ..
            } => *binding_ordinal == holder.binding_ordinal && *released == handle,
            CheckedUnitEffectOperationPlan::ContinuationCleanup { affine_discards, .. } => {
                affine_discards.iter().any(|discard| {
                    discard.path.is_empty()
                        && matches!(discard.source,
                            CheckedUnitStructuralArgumentSourcePlan::StructuralResult { binding_ordinal }
                                if binding_ordinal == holder.binding_ordinal)
                })
            }
            _ => false,
        })
    }
}

/// An explicitly established reference has one whole-owner loan occurrence.
/// Match its registered local binding, rather than re-reading the initializer.
pub(super) fn local_reference_loan(
    facts: &CheckFacts,
    result: &CheckedUnitStructuralResultBindingPlan,
    results: &Results,
) -> Option<arena::Handle<BorrowLoanFact>> {
    let mut roots = results.iter().filter(|(binding, _)| binding == result);
    let (_, facts::PlaceRoot::Symbol(owner)) = roots.next()? else {
        return None;
    };
    if roots.next().is_some() {
        return None;
    }
    let mut loans = facts.borrow.loans.iter().filter_map(|(handle, loan)| {
        (loan.owner_symbol == *owner
            && loan.statement_index == result.statement_index as usize
            && facts.borrow.loan_owner_path(loan).is_empty())
        .then_some(handle)
    });
    let loan = loans.next()?;
    loans.next().is_none().then_some(loan)
}

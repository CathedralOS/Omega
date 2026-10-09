//! Exact holder retirement and owned operands used by statement scheduling.
//! The checked value graph supplies constructor transfers and copied-carrier
//! correspondence; loan formation supplies immediate parent dependencies.
//! Transport preserves those parents without creating a new reborrow, and
//! retirement never infers custody from the spelling of an initializer.

use super::{
    CheckFacts, CheckedStructuralAccess, CheckedUnitEffectOperationPlan,
    CheckedUnitStructuralArgumentPlan, CheckedUnitStructuralArgumentSourcePlan,
    CheckedUnitStructuralResultBindingPlan, SymbolHandle,
};
use checked_trees::{BorrowLoanFact, CheckedStructuralValueKind, CheckedUnitStructuralPathSegment};

type Results = [(CheckedUnitStructuralResultBindingPlan, facts::PlaceRoot)];

mod transfers;
use transfers::transfers;
pub(super) mod transport;

pub(super) fn transferred_paths(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    results: &Results,
    binding_ordinal: u32,
    operations: &[CheckedUnitEffectOperationPlan],
) -> Option<Vec<Vec<CheckedUnitStructuralPathSegment>>> {
    Some(
        transfers(program, facts, results, binding_ordinal, operations)?
            .possible
            .into_iter()
            .map(|transfer| transfer.source)
            .collect(),
    )
}

pub(super) struct Retirement<'a> {
    pub(super) program: &'a dyn ::validation::CheckingInput,
    pub(super) facts: &'a CheckFacts,
    pub(super) machine: SymbolHandle,
    pub(super) state: &'a typed_trees::state::State,
    pub(super) statement_index: u32,
    pub(super) results: &'a Results,
    pub(super) operations: &'a [CheckedUnitEffectOperationPlan],
    pub(super) pending: &'a [CheckedUnitEffectOperationPlan],
    pub(super) transported_loans: &'a transport::TransportedLoans,
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
                    && self.retains_parent(handle, parent, &mut Vec::new())
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

    fn retains_parent(
        &self,
        child: arena::Handle<BorrowLoanFact>,
        parent: arena::Handle<BorrowLoanFact>,
        active: &mut Vec<arena::Handle<BorrowLoanFact>>,
    ) -> bool {
        if child == parent || active.contains(&child) {
            return false;
        }
        let loan = self.facts.borrow.loans.get(child);
        if loan.lineage
            == (checked_trees::BorrowLoanLineage::Reborrow {
                parent_loan: parent,
            })
            || self.captures_holder(child, parent)
        {
            return true;
        }
        active.push(child);
        let retained = self.transported_loans.iter().any(|(receiver, supplier)| {
            *receiver == child && self.retains_parent(*supplier, parent, active)
        });
        active.pop();
        retained
    }

    /// Materialized descriptors retain their supplying carrier even when the
    /// source resource model deliberately leaves aggregate lineage unretained.
    /// This is a disposal dependency from the checked operation's actual input,
    /// not a new reborrow permission or an initializer reconstruction.
    fn captures_holder(
        &self,
        child: arena::Handle<BorrowLoanFact>,
        parent: arena::Handle<BorrowLoanFact>,
    ) -> bool {
        let parent = self.facts.borrow.loans.get(parent);
        self.operations.iter().any(|operation| {
            if let Some(call) = operation.call() {
                // The call's retained loan group names each captured source.
                // This is a disposal dependency, not permission to skip an
                // immediate parent in the source resource judgment.
                return call.structural_custody().is_some_and(|custody| {
                    self.facts
                        .borrow
                        .loans_contain(custody.reference_loans, child)
                        && self.facts.borrow.loans.get(child).source_owner_symbol
                            == parent.owner_symbol
                        && call
                            .structural_arguments
                            .iter()
                            .any(|source| self.source_captures_holder(source, parent))
                });
            }
            let CheckedUnitEffectOperationPlan::EstablishReference { result, source, .. } =
                operation
            else {
                return false;
            };
            if !local_reference_loans(
                self.facts,
                self.machine,
                self.state.symbol,
                result,
                self.results,
            )
            .is_some_and(|loans| self.facts.borrow.loans_contain(loans, child))
            {
                return false;
            }
            self.source_captures_holder(source, parent)
        })
    }

    fn source_captures_holder(
        &self,
        source: &CheckedUnitStructuralArgumentPlan,
        parent: &BorrowLoanFact,
    ) -> bool {
        let source_holder = match source.source {
            CheckedUnitStructuralArgumentSourcePlan::StructuralResult { binding_ordinal } => {
                self.results.iter().find(|(result, root)| {
                    result.binding_ordinal == binding_ordinal
                        && result.statement_index as usize == parent.statement_index
                        && *root == facts::PlaceRoot::Symbol(parent.owner_symbol)
                })
            }
            _ => source.structural_storage_root().and_then(|source_root| {
                self.results.iter().find(|(result, root)| {
                    *root == source_root
                        && result.statement_index as usize == parent.statement_index
                        && *root == facts::PlaceRoot::Symbol(parent.owner_symbol)
                })
            }),
        };
        if source_holder.is_none() {
            return false;
        }
        let owner_path = self.facts.borrow.loan_owner_path(parent);
        // Only storage before the first reference crossing belongs to the
        // supplying holder. A borrow of an enclosing subtree pins all of
        // its contained descriptor slots; disjoint sibling captures do not.
        let prefix = source
            .path
            .iter()
            .position(|segment| *segment == CheckedUnitStructuralPathSegment::Referent)
            .unwrap_or(source.path.len());
        let compared = owner_path.len().min(prefix);
        let Some((_, local)) = self
            .program
            .state_local_declaration(self.state, parent.owner_symbol)
        else {
            return true; // A registered holder with unresolved geometry stays live.
        };
        let Some(segments) = validation::reference_result_custody::loan_place_segments(
            self.program,
            local.type_reference,
            &source.path[..compared],
        ) else {
            return true;
        };
        owner_path
            .iter()
            .zip(segments)
            .all(|(owner, selected)| match (owner, selected) {
                (
                    checked_trees::BorrowLoanOwnerSegment::Field(owner),
                    facts::PlaceSegment::Field { symbol },
                ) => *owner == symbol,
                (
                    checked_trees::BorrowLoanOwnerSegment::FixedIndex(owner),
                    facts::PlaceSegment::FixedIndex { index },
                ) => *owner == index,
                (
                    checked_trees::BorrowLoanOwnerSegment::Case(owner),
                    facts::PlaceSegment::Case { variant },
                ) => *owner == variant,
                (checked_trees::BorrowLoanOwnerSegment::DynamicIndex, _) => true,
                _ => false,
            })
    }

    pub(super) fn holder_retired(&self, handle: arena::Handle<BorrowLoanFact>) -> bool {
        self.holder_retired_following(handle, &mut Vec::new())
    }

    fn holder_retired_following(
        &self,
        handle: arena::Handle<BorrowLoanFact>,
        active: &mut Vec<arena::Handle<BorrowLoanFact>>,
    ) -> bool {
        if active.contains(&handle) {
            return false;
        }
        active.push(handle);
        let retired = self.holder_retired_inner(handle, active);
        active.pop();
        retired
    }

    fn holder_retired_inner(
        &self,
        handle: arena::Handle<BorrowLoanFact>,
        active: &mut Vec<arena::Handle<BorrowLoanFact>>,
    ) -> bool {
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
            // checked weakening for every borrow access. Unknown and replaced
            // holders remain live; descendants are checked separately.
            let Some((ordinal, alias)) = self
                .program
                .state_local_declaration(self.state, loan.owner_symbol)
            else {
                return false;
            };
            let unmaterialized_alias = ordinal == loan.statement_index
                && self.facts.borrow.loan_owner_path(loan).is_empty()
                && validation::reference_shell(self.program, alias.type_reference).is_some()
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
        if self.operations.iter().chain(self.pending).any(|operation| match operation {
            CheckedUnitEffectOperationPlan::ReleaseReference {
                binding_ordinal, loans: released, defer_to_exit: false, ..
            } => *binding_ordinal == holder.binding_ordinal && self.facts.borrow.loans_contain(*released, handle),
            CheckedUnitEffectOperationPlan::ContinuationCleanup { cleanup_actions, .. } => {
                cleanup_actions.iter().any(|discard| {
                    discard.path().is_empty()
                        && matches!(*discard.source(),
                            CheckedUnitStructuralArgumentSourcePlan::StructuralResult { binding_ordinal }
                                if binding_ordinal == holder.binding_ordinal)
                })
            }
            _ => false,
        }) {
            return true;
        }
        let Some(transfers) = transfers(
            self.program,
            self.facts,
            self.results,
            holder.binding_ordinal,
            self.operations,
        ) else {
            return false;
        };
        let Some((_, local)) = self
            .program
            .state_local_declaration(self.state, loan.owner_symbol)
        else {
            return false;
        };
        let owner_path = self.facts.borrow.loan_owner_path(loan);
        let selected_path = |path: &[CheckedUnitStructuralPathSegment]| {
            validation::reference_result_custody::loan_place_segments(
                self.program,
                local.type_reference,
                path,
            )
            .and_then(|segments| {
                segments
                    .into_iter()
                    .map(|segment| match segment {
                        facts::PlaceSegment::Field { symbol } => {
                            Some(checked_trees::BorrowLoanOwnerSegment::Field(symbol))
                        }
                        facts::PlaceSegment::FixedIndex { index } => {
                            Some(checked_trees::BorrowLoanOwnerSegment::FixedIndex(index))
                        }
                        _ => None,
                    })
                    .collect::<Option<Vec<_>>>()
            })
        };
        if !transfers
            .definite
            .iter()
            .any(|path| selected_path(path).is_some_and(|path| owner_path.starts_with(&path)))
        {
            return false;
        }
        let mut found = false;
        for transfer in transfers.possible {
            let Some(selected) = selected_path(&transfer.source) else {
                return false;
            };
            if !owner_path.starts_with(&selected) {
                continue;
            }
            found = true;
            let Some((destination, mut path)) = transfer.destination else {
                return false;
            };
            // Formation at this boundary establishes live custody; it is not
            // the absent-future-child case handled by holder_retired_inner.
            if destination.statement_index >= self.statement_index {
                return false;
            }
            path.extend_from_slice(&owner_path[selected.len()..]);
            let mut bindings = self
                .results
                .iter()
                .filter(|(binding, _)| binding == &destination);
            let Some((_, facts::PlaceRoot::Symbol(owner))) = bindings.next() else {
                return false;
            };
            if bindings.next().is_some() {
                return false;
            }
            let mut receiving = self.facts.borrow.loans.iter().filter(|(_, candidate)| {
                candidate.owner_symbol == *owner
                    && candidate.statement_index == destination.statement_index as usize
                    && self.facts.borrow.loan_owner_path(candidate) == path
            });
            let mut found_receiver = false;
            if !receiving.all(|(received, _)| {
                found_receiver = true;
                self.holder_retired_following(received, active)
            }) || !found_receiver
            {
                return false;
            }
        }
        found
    }
}

/// An established descriptor retains every whole-owner loan occurrence.
/// Match its registered local binding, rather than re-reading the initializer.
pub(super) fn local_reference_loans(
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    result: &CheckedUnitStructuralResultBindingPlan,
    results: &Results,
) -> Option<arena::HandleSpan<BorrowLoanFact>> {
    let mut roots = results.iter().filter(|(binding, _)| binding == result);
    let (_, facts::PlaceRoot::Symbol(owner)) = roots.next()? else {
        return None;
    };
    if roots.next().is_some() {
        return None;
    }
    let mut states = facts
        .borrow
        .states
        .iter()
        .map(|(_, borrowing)| borrowing)
        .filter(|borrowing| borrowing.machine_symbol == machine && borrowing.state_symbol == state);
    let borrowing = states.next()?;
    if states.next().is_some() {
        return None;
    }
    let loans = facts
        .borrow
        .local_loans(borrowing, *owner, result.statement_index as usize)?;
    (!loans.is_empty()
        && facts
            .borrow
            .loan_group(loans)?
            .all(|(_, loan)| facts.borrow.loan_owner_path(loan).is_empty()))
    .then_some(loans)
}

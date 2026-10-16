//! Exact holder retirement and owned operands used by statement scheduling.
//! The checked value graph supplies constructor transfers and copied-carrier
//! correspondence; loan formation supplies immediate parent dependencies.
//! Transport preserves those parents without creating a new reborrow, and
//! retirement never infers custody from the spelling of an initializer.

use super::{
    CheckFacts, CheckedUnitEffectOperationPlan, CheckedUnitStructuralArgumentPlan,
    CheckedUnitStructuralArgumentSourcePlan, CheckedUnitStructuralResultBindingPlan, SymbolHandle,
};
use checked_trees::{BorrowLoanFact, CheckedUnitStructuralPathSegment};

type Results = [(CheckedUnitStructuralResultBindingPlan, facts::PlaceRoot)];

/// Death timing comes from the loan schedule; payload availability comes from
/// facts at that exact boundary. Do not reread a selected, partially consumed
/// sum merely because a later descriptor in its payload now expires.
pub(super) fn prepare_case_conditions(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    shapes: &mut crate::execution::terminal_unit::types::ShapeCollector<'_>,
    machine: SymbolHandle,
    state: SymbolHandle,
    parameters: &[checked_trees::CheckedUnitStructuralParameterPlan],
    results: &Results,
    operations: &mut [CheckedUnitEffectOperationPlan],
) -> Option<()> {
    use crate::execution::terminal_unit::{cleanup::checked_path_segments, types::machine_binders};
    let authored = program.state_by_symbol(state)?;
    let binders = machine_binders(program, program.machine_by_symbol(machine)?);
    for operation in operations {
        let CheckedUnitEffectOperationPlan::ReleaseReference {
            source,
            path,
            statement_index,
            defer_to_exit,
            case_conditions,
            ..
        } = operation
        else {
            continue;
        };
        if !path
            .iter()
            .any(|segment| matches!(segment, CheckedUnitStructuralPathSegment::Case(_)))
        {
            continue;
        }
        let root = match source {
            CheckedUnitStructuralArgumentSourcePlan::StructuralResult { binding_ordinal } => {
                results
                    .iter()
                    .find(|(result, _)| result.binding_ordinal == *binding_ordinal)?
                    .1
            }
            CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol } => {
                facts::PlaceRoot::Symbol(*symbol)
            }
            CheckedUnitStructuralArgumentSourcePlan::StructuralExpression { expression } => {
                facts::PlaceRoot::Expression(*expression)
            }
            CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } => {
                let parameter = parameters.get(*parameter_index as usize)?;
                facts::PlaceRoot::Symbol(
                    program
                        .state_parameters(authored)
                        .get(parameter.position as usize)?
                        .symbol,
                )
            }
            _ => return None,
        };
        let boundary = *statement_index as usize;
        let reference = validation::canonical_place_type_reference(
            program,
            state,
            boundary,
            &validation::CanonicalPlace {
                root,
                segments: Vec::new(),
            },
        )?;
        let flow = facts.flow.control.state(machine, state)?;
        let points: Vec<(usize, Vec<facts::FactContextHandle>)> = if *defer_to_exit {
            facts
                .flow
                .control
                .outgoing
                .span_or_empty(flow.outgoing)
                .iter()
                .map(|exit| {
                    (
                        exit.statement_index,
                        facts
                            .flow
                            .semantic_constraint_contexts(exit.constraints)
                            .collect(),
                    )
                })
                .collect()
        } else {
            facts
                .flow
                .state_statement(flow, boundary)
                .map(|statement| {
                    vec![(
                        boundary,
                        facts
                            .flow
                            .semantic_constraint_contexts(statement.entry_constraints)
                            .collect(),
                    )]
                })
                .unwrap_or_default()
        };
        let mut conditions = Vec::new();
        for (position, segment) in path.iter().enumerate() {
            let CheckedUnitStructuralPathSegment::Case(case_identity) = segment else {
                continue;
            };
            let segments =
                checked_path_segments(program, state, boundary, reference, &path[..=position])?;
            let subject = validation::CanonicalPlace { root, segments };
            if !points.is_empty()
                && points.iter().all(|(statement, contexts)| {
                    crate::flow::place_cases_are_selected(
                        program,
                        &facts.semantic,
                        contexts,
                        machine,
                        state,
                        *statement,
                        &subject,
                    )
                })
            {
                continue;
            }
            let mut carrier = subject;
            carrier.segments.pop();
            let carrier_reference =
                validation::canonical_place_type_reference(program, state, boundary, &carrier)?;
            let type_identity = shapes.add_type(carrier_reference, &binders, &[])?;
            conditions.push(checked_trees::CheckedCleanupCaseCondition {
                path: shapes
                    .cleanup_control
                    .paths
                    .insert_many(path[..position].iter().cloned()),
                type_identity,
                case_identity: case_identity.clone(),
            });
        }
        *case_conditions = shapes.cleanup_control.conditions.insert_many(conditions);
    }
    Some(())
}

/// Missing borrow rows mean no recorded holders; duplicate rows cannot name
/// one retirement schedule. Keep these outcomes distinct while consulting
/// the ledger's existing revision-checked index instead of scanning all states
/// once for every statement and every descendant holder.
fn unique_borrow_state(
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
) -> Option<Option<&checked_trees::StateBorrowFact>> {
    facts
        .borrow
        .with_state_index(|index| match index.bucket(machine, state) {
            [] => Some(None),
            [handle] => Some(Some(facts.borrow.states.get(*handle))),
            _ => None,
        })
}

/// Completion transfers one actual result occurrence, named or anonymous.
/// Its captured origin roster is produced by checking; this join schedules
/// local disposition without inferring a return from a type or source root.
pub(super) fn returned_reference_binding(
    order: &typed_trees::statement::StatementExecutionOrder,
    returned: &checked_trees::CheckedUnitStructuralReturnPlan,
    result: &CheckedUnitStructuralResultBindingPlan,
) -> bool {
    !returned.reference_sources.is_empty()
        && returned.source
            == (CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                binding_ordinal: result.binding_ordinal,
            })
        && returned.type_identity == result.type_identity
        && returned.multiplicity == result.multiplicity
        && match returned.completion {
            checked_trees::CheckedStructuralReturnCompletion::Value {
                statement_index, ..
            } => order.at_or_before(result.statement_index as usize, statement_index as usize),
            checked_trees::CheckedStructuralReturnCompletion::Call { coordinate } => {
                result.statement_index == coordinate.statement_index
            }
        }
}

use crate::execution::terminal_unit::storage_transfers as transfers;
use transfers::{Destination, transfers};
mod carriers;
pub(super) mod selections;
pub(super) mod stores;
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
    pub(super) order: &'a typed_trees::statement::StatementExecutionOrder,
    pub(super) statement_index: u32,
    pub(super) results: &'a Results,
    pub(super) operations: &'a [CheckedUnitEffectOperationPlan],
    pub(super) pending: &'a [CheckedUnitEffectOperationPlan],
    /// Deferred edge releases count only once completion schedules them.
    /// Ordinary statement expiry cannot consume a future edge's custody.
    pub(super) at_exit: bool,
    /// The selected completion transfers this exact holder before its deferred
    /// ancestor dispositions. It never settles a loan at statement entry.
    pub(super) returned: Option<&'a checked_trees::CheckedUnitStructuralReturnPlan>,
    pub(super) transported_loans: &'a transport::TransportedLoans,
}

impl Retirement<'_> {
    /// Computed homes have no independent statement-result binding. Their
    /// stored descriptors nevertheless own the same captured formation and
    /// weakening as named storage. End those leaves only after every holder
    /// borrowing the home has actually retired. This does not dispose the
    /// backing's owned contents or authorize a nominal cleanup.
    /// The state-wide loan roster includes unevaluated alternatives. These are
    /// candidate storage deaths: the existing ownership continuation graph
    /// selects actual births before emitting exact descriptor releases.
    pub(super) fn backing_releases(&self) -> Option<Vec<CheckedUnitEffectOperationPlan>> {
        let state = unique_borrow_state(self.facts, self.machine, self.state.symbol)?;
        let Some(state) = state else {
            return Some(Vec::new());
        };
        if state.loans.is_empty() {
            return Some(Vec::new());
        }
        let mut owners = Vec::new();
        for (handle, loan) in self.facts.borrow.loans.iter() {
            if self.facts.borrow.state_owns_loan(state, handle)
                && matches!(loan.storage_owner, facts::PlaceRoot::Expression(_))
                && self
                    .order
                    .precedes(loan.statement_index, self.statement_index as usize)
                && !owners.contains(&(loan.storage_owner, loan.statement_index))
            {
                owners.push((loan.storage_owner, loan.statement_index));
            }
        }
        let mut releases = Vec::new();
        for (owner, formation) in owners.into_iter().rev() {
            if self.facts.borrow.loans.iter().any(|(handle, loan)| {
                self.facts.borrow.state_owns_loan(state, handle)
                    && loan.root == owner
                    && self
                        .order
                        .precedes(loan.statement_index, self.statement_index as usize)
                    && !self.holder_retired(handle)
            }) {
                continue;
            }
            let reference = validation::canonical_place_type_reference(
                self.program,
                self.state.symbol,
                formation,
                &validation::CanonicalPlace {
                    root: owner,
                    segments: Vec::new(),
                },
            )?;
            // A computed owner may contain references without being a
            // descriptor. Its nominal/linear disposal remains authoritative;
            // lexical leaf expiry cannot retire a hook's receiver contents.
            if !validation::has_contents(
                self.program,
                reference,
                &[],
                validation::ContentsRequirement::DescriptorCleanup,
            ) {
                continue;
            }
            for (source, loans) in validation::reference_result_custody::storage_place_loans(
                self.program,
                self.facts,
                self.machine,
                self.state,
                owner,
                reference,
                u32::try_from(formation).ok()?,
                &[],
            )? {
                let transfers = transfers::from_storage(
                    self.program,
                    self.facts,
                    self.results,
                    source.source.source.clone(),
                    u32::try_from(formation).ok()?,
                    &source.path,
                    self.operations,
                )?;
                // Custody moved to a receiving value is no longer held by
                // this backing leaf, even when its lexical loan has expired.
                // Shared descriptor copies keep their independent original.
                if transfers.possible.iter().any(|transfer| {
                    source.path.starts_with(&transfer.source)
                        || transfer.source.starts_with(&source.path)
                }) {
                    continue;
                }
                let mut ready = true;
                for (handle, _) in self.facts.borrow.loan_group(loans)? {
                    if self.disposed(handle)
                        || !self.children_retired(handle)
                        || !validation::reference_result_custody::release_boundary(
                            self.program,
                            self.facts,
                            self.machine,
                            self.state.symbol,
                            handle,
                        )
                        .is_some_and(|boundary| {
                            self.order
                                .at_or_before(boundary as usize, self.statement_index as usize)
                        })
                    {
                        ready = false;
                        break;
                    }
                }
                if ready {
                    releases.push(CheckedUnitEffectOperationPlan::ReleaseReference {
                        statement_index: self.statement_index,
                        source: source.source.source,
                        path: source.path,
                        select_evaluated_storage: true,
                        // Exit scheduling may depend on carriers retired by
                        // the selected edge. Their backing must follow that
                        // edge too, after its condition and operands run.
                        defer_to_exit: self.at_exit,
                        loans,
                        selection_complements: Vec::new(),
                        case_conditions: Default::default(),
                    });
                }
            }
        }
        Some(releases)
    }

    /// An immediate child must have ended its actual carrier, not merely
    /// reached its parent's lexical last use. Each child's own schedule obeys
    /// this same rule, so reverse production closes a chain deepest first.
    pub(super) fn children_retired(&self, parent: arena::Handle<BorrowLoanFact>) -> bool {
        let Some(Some(state)) = unique_borrow_state(self.facts, self.machine, self.state.symbol)
        else {
            return false;
        };
        let mut pending = vec![parent];
        let mut visited = Vec::new();
        while let Some(parent) = pending.pop() {
            if visited.contains(&parent) {
                return false;
            }
            visited.push(parent);
            for (handle, loan) in self.facts.borrow.loans.iter() {
                if self.facts.borrow.state_owns_loan(state, handle)
                    && self
                        .order
                        .precedes(loan.statement_index, self.statement_index as usize)
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
                // Root-reference custody owns a whole descriptor release;
                // record results instead retire their stored leaf groups.
                // Both pin the actual source holder. Join the registered
                // receiving binding, not the root-only call release roster.
                // This is disposal order, not new reborrow authority.
                return call.structural_result().is_some_and(|result| {
                    carrier_loans(
                        self.facts,
                        self.machine,
                        self.state.symbol,
                        result,
                        self.results,
                    )
                    .is_some_and(|loans| self.facts.borrow.loans_contain(loans, child))
                        && self.facts.borrow.loans.get(child).source_owner_symbol
                            == parent.owner_symbol
                        && call.structural_arguments.iter().any(|source| {
                            source.access.borrow_access_kind().is_some()
                                && self.source_captures_holder(source, parent)
                        })
                });
            }
            let construction = if let CheckedUnitEffectOperationPlan::EstablishLocal(
                checked_trees::CheckedUnitLocalBindingPlan::StructuralValue { result, .. },
            ) = operation
            {
                if !carrier_loans(
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
                let loan = self.facts.borrow.loans.get(child);
                let Some((_, local)) = self
                    .program
                    .state_local_declaration(self.state, loan.owner_symbol)
                else {
                    return false;
                };
                let Some(path) = transport::carrier_path(
                    self.program,
                    local.type_reference,
                    self.facts.borrow.loan_owner_path(loan),
                ) else {
                    return false;
                };
                Some((operation, path))
            } else {
                stores::construction(self, child, operation)
            };
            if let Some((producer, path)) = construction {
                let loan = self.facts.borrow.loans.get(child);
                if let Some(call) = producer.call() {
                    return loan.source_owner_symbol == parent.owner_symbol
                        && call.structural_arguments.iter().any(|source| {
                            source.access.borrow_access_kind().is_some()
                                && self.source_captures_holder(source, parent)
                        });
                }
                let CheckedUnitEffectOperationPlan::EstablishLocal(
                    checked_trees::CheckedUnitLocalBindingPlan::StructuralValue {
                        value,
                        calls,
                        ..
                    },
                ) = producer
                else {
                    return false;
                };
                let Some(sources) = validation::reference_result_custody::stored_carrier_sources(
                    self.program,
                    self.facts,
                    *value,
                    &path,
                ) else {
                    return false;
                };
                // A retained inner call forms a descriptor in the enclosing
                // result's exact leaf slot. Its anonymous call result is not
                // another local holder. Keep operand parents until this
                // receiving occurrence has actually retired.
                return sources.into_iter().any(|source| {
                    if let validation::reference_result_custody::StoredCarrierSource::Borrow {
                        value,
                        source,
                    } = &source
                    {
                        return loan.formation_expression
                            == self
                                .facts
                                .values
                                .structural_values
                                .nodes
                                .get(*value)
                                .expression
                            && source.access.borrow_access_kind().is_some()
                            && self.source_captures_holder(source, parent);
                    }
                    let validation::reference_result_custody::StoredCarrierSource::Call {
                        value,
                        ..
                    } = source
                    else {
                        return false;
                    };
                    if loan.formation_expression
                        != self
                            .facts
                            .values
                            .structural_values
                            .nodes
                            .get(value)
                            .expression
                        || loan.source_owner_symbol != parent.owner_symbol
                    {
                        return false;
                    }
                    calls
                        .iter()
                        .find(|call| call.value == value)
                        .and_then(|call| call.operation().call())
                        .is_some_and(|call| {
                            call.structural_arguments.iter().any(|source| {
                                source.access.borrow_access_kind().is_some()
                                    && self.source_captures_holder(source, parent)
                            })
                        })
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

    /// Calls can publish receiving loans through results or stored effects.
    /// The supplying occurrence, not the syntactic placement of the call,
    /// identifies which receivers must finish before its backing may retire.
    fn call_receivers_retired(
        &self,
        supplier: arena::Handle<BorrowLoanFact>,
        statement_index: u32,
        structural_result: bool,
        active: &mut Vec<arena::Handle<BorrowLoanFact>>,
    ) -> bool {
        if !self
            .order
            .precedes(statement_index as usize, self.statement_index as usize)
        {
            return false;
        }
        let mut found_receiver = false;
        let retired = self
            .transported_loans
            .iter()
            .filter(|(_, candidate)| *candidate == supplier)
            .filter(|(receiver, _)| {
                self.facts.borrow.loans.get(*receiver).statement_index == statement_index as usize
            })
            .all(|(receiver, _)| {
                found_receiver = true;
                self.holder_retired_following(*receiver, active)
            });
        retired && (!structural_result || found_receiver)
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
        if !self
            .order
            .precedes(loan.statement_index, self.statement_index as usize)
        {
            return true;
        }
        if self.disposed(handle) {
            return true;
        }
        if self.at_exit && self.return_transfers_holder(loan) {
            return true;
        }
        let mut holders = self.results.iter().filter(|(holder, root)| {
            *root == facts::PlaceRoot::Symbol(loan.owner_symbol)
                && holder.statement_index as usize == loan.statement_index
        });
        let holder = holders.next();
        let stored = self.operations.iter().any(|operation| {
            stores::receiver(operation).is_some_and(|(owner, formation, _)| {
                owner == loan.owner_symbol && formation as usize == loan.statement_index
            }) && stores::loans(self, operation).is_some_and(|groups| {
                groups
                    .iter()
                    .any(|(_, group)| self.facts.borrow.loans_contain(*group, handle))
            })
        });
        if holder.is_none() && !stored {
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
                .is_some_and(|boundary| {
                    self.order
                        .at_or_before(boundary as usize, self.statement_index as usize)
                });
        }
        if holders.next().is_some() {
            return false;
        }
        let source = holder.map_or(
            CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
                symbol: loan.owner_symbol,
            },
            |(holder, _)| CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                binding_ordinal: holder.binding_ordinal,
            },
        );
        let Ok(formation) = u32::try_from(loan.statement_index) else {
            return false;
        };
        let Some((_, local)) = self
            .program
            .state_local_declaration(self.state, loan.owner_symbol)
        else {
            return false;
        };
        let Some(selection) = transport::carrier_path(
            self.program,
            local.type_reference,
            self.facts.borrow.loan_owner_path(loan),
        ) else {
            return false;
        };
        let Some(transfers) = transfers::from_storage(
            self.program,
            self.facts,
            self.results,
            source,
            formation,
            &selection,
            self.operations,
        ) else {
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
            let (destination, mut path) = match transfer.destination {
                Destination::Unknown => return false,
                Destination::Call {
                    statement_index,
                    structural_result,
                } => {
                    // A structural result may carry the consumed descriptor
                    // onward, so only the producer's complete receiving
                    // occurrences close this holder. A call returning none
                    // ends the carrier inside the callee: every receiving
                    // occurrence it still publishes (a stored reference
                    // effect) must retire, and no receiver means the call
                    // itself retired the holder.
                    if !self.call_receivers_retired(
                        handle,
                        statement_index,
                        structural_result,
                        active,
                    ) {
                        return false;
                    }
                    continue;
                }
                Destination::Storage {
                    symbol,
                    statement_index,
                    path,
                } => {
                    if !self
                        .order
                        .precedes(statement_index as usize, self.statement_index as usize)
                    {
                        return false;
                    }
                    let Some((_, local)) = self.program.state_local_declaration(self.state, symbol)
                    else {
                        return false;
                    };
                    let Some(mut receiving_path) =
                        validation::reference_result_custody::loan_owner_path(
                            self.program,
                            local.type_reference,
                            &path,
                        )
                    else {
                        return false;
                    };
                    receiving_path.extend_from_slice(&owner_path[selected.len()..]);
                    let mut found_receiver = false;
                    let retired = self
                        .facts
                        .borrow
                        .loans
                        .iter()
                        .filter(|(_, receiver)| {
                            receiver.owner_symbol == symbol
                                && receiver.statement_index == statement_index as usize
                                && self.facts.borrow.loan_owner_path(receiver) == receiving_path
                        })
                        .all(|(receiver, _)| {
                            found_receiver = true;
                            self.holder_retired_following(receiver, active)
                        });
                    if !retired || !found_receiver {
                        return false;
                    }
                    continue;
                }
                Destination::Holder(destination, path) => (destination, path),
            };
            // Formation at this boundary establishes live custody; it is not
            // the absent-future-child case handled by holder_retired_inner.
            if !self.order.precedes(
                destination.statement_index as usize,
                self.statement_index as usize,
            ) {
                return false;
            }
            path.extend_from_slice(&owner_path[selected.len()..]);
            if self.return_receives_carrier(&destination, &path, loan) {
                continue;
            }
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

    fn return_transfers_holder(&self, loan: &BorrowLoanFact) -> bool {
        let path = self.facts.borrow.loan_owner_path(loan);
        self.results.iter().any(|(holder, root)| {
            *root == loan.storage_owner
                && *root == facts::PlaceRoot::Symbol(loan.owner_symbol)
                && holder.statement_index as usize == loan.statement_index
                && self.return_receives_carrier(holder, path, loan)
        })
    }

    /// A definite owned transfer can end in the anonymous completion value,
    /// which has no source-local loan holder. Join that receiving occurrence to
    /// the actual captured return, rather than inventing a named local for it.
    fn return_receives_carrier(
        &self,
        receiver: &CheckedUnitStructuralResultBindingPlan,
        path: &[checked_trees::BorrowLoanOwnerSegment],
        loan: &BorrowLoanFact,
    ) -> bool {
        let Some(returned) = self.returned.filter(|_| self.at_exit) else {
            return false;
        };
        if !returned_reference_binding(self.order, returned, receiver) {
            return false;
        }
        let Some(path) = transport::carrier_path(self.program, self.state.return_type, path) else {
            return false;
        };
        let (statement_index, expression) = match returned.completion {
            checked_trees::CheckedStructuralReturnCompletion::Value {
                statement_index,
                expression,
                ..
            } => (statement_index as usize, Some(expression)),
            checked_trees::CheckedStructuralReturnCompletion::Call { coordinate } => {
                (coordinate.statement_index as usize, None)
            }
        };
        if loan.last_use_statement_index != statement_index {
            return false;
        }
        let Some(segments) = self.facts.borrow.access_segments.span(loan.segments) else {
            return false;
        };
        let Some(Some(state)) = unique_borrow_state(self.facts, self.machine, self.state.symbol)
        else {
            return false;
        };
        self.facts
            .borrow
            .returned_loans
            .span(state.returned_loans)
            .is_some_and(|captured| {
                captured.iter().any(|captured| {
                    captured.statement_index == statement_index
                        && expression
                            .is_none_or(|expression| captured.formation_expression == expression)
                        && captured.root == loan.root
                        && captured.incoming_reference == loan.incoming_reference
                        && captured.kind == loan.kind
                        && self.facts.borrow.access_segments.span(captured.segments)
                            == Some(segments)
                        && self
                            .facts
                            .borrow
                            .owner_segments
                            .span(captured.owner_path)
                            .and_then(|owner| {
                                transport::carrier_path(self.program, self.state.return_type, owner)
                            })
                            .is_some_and(|captured_path| captured_path == path)
                })
            })
    }

    /// A source owner in the result roster is provenance, not a statement
    /// that an anonymous result is the owner's carrier. Translate an owned
    /// extraction's selected subtree; borrowed extractions retain their carrier.
    fn disposed_result_path(
        &self,
        binding_ordinal: u32,
        path: &[CheckedUnitStructuralPathSegment],
        loan: &BorrowLoanFact,
    ) -> Option<Vec<CheckedUnitStructuralPathSegment>> {
        let mut holders = self
            .results
            .iter()
            .filter(|(holder, _)| holder.binding_ordinal == binding_ordinal);
        let (holder, _) = holders.next()?;
        if holders.next().is_some() {
            return None;
        }
        if holder.local.symbol == loan.owner_symbol
            && holder.statement_index as usize == loan.statement_index
        {
            return Some(path.to_vec());
        }
        let mut producers = self
            .operations
            .iter()
            .chain(self.pending)
            .flat_map(CheckedUnitEffectOperationPlan::with_value_calls)
            .filter_map(|operation| match operation {
                CheckedUnitEffectOperationPlan::MoveStructuralField {
                    source,
                    result,
                    expression,
                } if result.binding_ordinal == binding_ordinal => {
                    Some((source, result, expression))
                }
                _ => None,
            });
        let (source, result, expression) = producers.next()?;
        // An old detached value cannot settle a later occurrence installed
        // in the same slot. A statement can extract the old value before
        // forming its replacement loan, so equality is insufficient too.
        if !self
            .order
            .precedes(loan.statement_index, result.statement_index as usize)
            || producers.next().is_some()
            || source.source
                != (CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
                    symbol: loan.owner_symbol,
                })
            || !transfers::extraction_owns_source(self.program, source, result, *expression)?
        {
            return None;
        }
        let mut selected = source.path.clone();
        selected.extend_from_slice(path);
        Some(selected)
    }

    /// Disposal ends an occurrence, not all future values in the same local.
    /// Replacement's checked boundary captures the old destination after RHS
    /// evaluation. A release instead names the exact retained loan group.
    pub(super) fn disposed(&self, handle: arena::Handle<BorrowLoanFact>) -> bool {
        let loan = self.facts.borrow.loans.get(handle);
        // Ownership already selected each temporary owner's normal call
        // death. Statement and scalar emission consume those same contextual
        // dispositions. A whole-owner death at formation settles every stored
        // descriptor, including a child lending another backing home; counting
        // only explicit statement releases would dispose those leaves twice.
        // The occurrence and coordinate join also preserve selected evaluation:
        // this owner is established while evaluating that call's operand.
        // Projected deaths and excluded subtrees remain with exact path cleanup.
        if self
            .order
            .precedes(loan.statement_index, self.statement_index as usize)
            && self
                .facts
                .contextual_cleanup
                .dispositions
                .iter()
                .any(|(_, disposition)| {
                    disposition.machine == self.machine
                        && disposition.state == self.state.symbol
                        && disposition.root == loan.storage_owner
                        && disposition.path.is_empty()
                        && disposition.exclusions.is_empty()
                        && matches!(disposition.source,
                        language_semantics::PermissionEventSource::Call { statement_index, .. }
                            if statement_index == loan.statement_index)
                })
        {
            return true;
        }
        self.operations
            .iter()
            .chain(self.pending)
            .any(|operation| match operation {
                CheckedUnitEffectOperationPlan::ReleaseReference {
                    statement_index,
                    loans,
                    selection_complements,
                    defer_to_exit,
                    ..
                } => {
                    (!*defer_to_exit || self.at_exit)
                        && self
                            .order
                            .at_or_before(*statement_index as usize, self.statement_index as usize)
                        && (self.facts.borrow.loans_contain(*loans, handle)
                            || selection_complements.iter().any(|complement| {
                                selections::disposes_candidate(self, handle, complement)
                            }))
                }
                CheckedUnitEffectOperationPlan::ContinuationCleanup {
                    boundary,
                    cleanup_actions,
                    ..
                } => {
                    let statement = match boundary {
                        checked_trees::CheckedUnitCleanupBoundary::Replacement {
                            statement_index,
                            ..
                        }
                        | checked_trees::CheckedUnitCleanupBoundary::StatementEntry {
                            statement_index,
                        } => *statement_index,
                        _ => return false,
                    };
                    if self
                        .order
                        .precedes(self.statement_index as usize, statement as usize)
                        || !self
                            .order
                            .precedes(loan.statement_index, statement as usize)
                    {
                        return false;
                    }
                    let Some((_, local)) = self
                        .program
                        .state_local_declaration(self.state, loan.owner_symbol)
                    else {
                        return false;
                    };
                    if self.operations.iter().any(|operation| {
                        let CheckedUnitEffectOperationPlan::Store(store) = operation else {
                            return false;
                        };
                        self.order
                            .precedes(loan.statement_index, store.statement_index as usize)
                            && self
                                .order
                                .precedes(store.statement_index as usize, statement as usize)
                            && store.installed_value().is_some()
                            && store.destination.root
                                == (checked_trees::CheckedStoreRoot::Local {
                                    symbol: loan.owner_symbol,
                                })
                            && validation::reference_result_custody::loan_owner_path(
                                self.program,
                                local.type_reference,
                                &store.destination.path,
                            )
                            .is_some_and(|path| {
                                self.facts.borrow.loan_owner_path(loan).starts_with(&path)
                            })
                    }) {
                        // A later replacement receipt settles the installed
                        // value, not an older occurrence moved out of this slot.
                        return false;
                    }
                    cleanup_actions.iter().any(|discard| {
                        let path =
                            match *discard.source() {
                                CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
                                    symbol,
                                } if symbol == loan.owner_symbol => discard.path().to_vec(),
                                CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                                    binding_ordinal,
                                } => {
                                    let Some(path) = self.disposed_result_path(
                                        binding_ordinal,
                                        discard.path(),
                                        loan,
                                    ) else {
                                        return false;
                                    };
                                    path
                                }
                                _ => return false,
                            };
                        validation::reference_result_custody::loan_owner_path(
                            self.program,
                            local.type_reference,
                            &path,
                        )
                        .is_some_and(|path| {
                            self.facts.borrow.loan_owner_path(loan).starts_with(&path)
                        })
                    })
                }
                _ => false,
            })
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
    let loans = carrier_loans(facts, machine, state, result, results)?;
    if loans.is_empty() {
        return None;
    }
    facts
        .borrow
        .loan_group(loans)?
        .all(|(_, loan)| facts.borrow.loan_owner_path(loan).is_empty())
        .then_some(loans)
}

/// Retained occurrences of this exact storage establishment, whether stored
/// at the root or beneath record/array paths. The path-specific retirement
/// owner still decides how each descriptor ends. An empty proven roster is
/// distinct from a missing or ambiguous producer relationship.
pub(super) fn carrier_loans(
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    result: &CheckedUnitStructuralResultBindingPlan,
    results: &Results,
) -> Option<arena::HandleSpan<BorrowLoanFact>> {
    let mut roots = results.iter().filter(|(binding, _)| binding == result);
    let (_, owner) = roots.next()?;
    if !matches!(
        owner,
        facts::PlaceRoot::Symbol(_) | facts::PlaceRoot::Expression(_)
    ) {
        return None;
    }
    if roots.next().is_some() {
        return None;
    }
    let borrowing = unique_borrow_state(facts, machine, state)??;
    facts
        .borrow
        .storage_loans(borrowing, *owner, result.statement_index as usize)
}

//! Captured loan resources name the actual storage retained across an arrival,
//! including stored-field crossings and call-returned references. Missing
//! descriptor provenance refuses retention rather than guessing a local home.
//! A selected arrival may retire a descriptor and transport its owned backing
//! only when no other captured value needs that backing's address. Retirement
//! alone says nothing about copied descriptors. Use the checked call's complete
//! operand resources and their stored dependencies, not producer spellings or
//! lexical last-use indices. Unresolved coverage retains the stable-home route.

use arena::{Handle, HandleSpan};
use checked_trees::{
    BorrowLoanFact, CheckFacts, CheckedCallBorrowLoanSource, CheckedUnitEffectOperationPlan,
    CheckedUnitStructuralArgumentPlan, CheckedUnitStructuralPathSegment, FlowConstraintRef,
};
use facts::PlaceRoot;
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionHandle;

fn selected_call(
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    statement: usize,
    target: SymbolHandle,
    arguments: &[ExpressionHandle],
) -> Option<Handle<checked_trees::FlowCallFact>> {
    let flow = facts.flow.control.state(machine, state)?;
    let mut calls = flow.calls.handles().filter(|handle| {
        let call = facts.flow.control.calls.get(*handle);
        call.statement_index == statement
            && call.authored_site == checked_trees::FlowCallSite::NamedTransition
            && call.authored_target == target
            && facts
                .flow
                .control
                .call_arguments
                .span(call.authored_arguments)
                == Some(arguments)
    });
    let call = calls.next()?;
    calls.next().is_none().then_some(call)
}

/// Rejoin captured permission to its state-local storage occurrence. The
/// checker has already composed stored-field crossings and call-result
/// provenance. Replaying descriptor constructors here would lose those edges.
pub(super) fn retained_backing(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    statement: usize,
    target: SymbolHandle,
    arguments: &[ExpressionHandle],
    argument_position: usize,
    operations: &[CheckedUnitEffectOperationPlan],
) -> Option<Option<u32>> {
    let call_handle = selected_call(facts, machine, state, statement, target, arguments)?;
    let operand = facts
        .flow
        .control
        .calls
        .get(call_handle)
        .argument_operands()
        .handles()
        .nth(argument_position)?;
    let mut roots = facts
        .borrow
        .call_loan_resources
        .iter()
        .filter(|(_, resource)| {
            resource.call == call_handle
                && resource.machine_symbol == machine
                && resource.state_symbol == state
                && resource.owner_operand == operand
                && resource.owner_path.is_empty()
        })
        .map(|(_, resource)| resource.parent_lifetime.root);
    let Some(root) = roots.next() else {
        let capture = facts.flow.control.operands.get(operand);
        if validation::reference_result_custody::stored_loan_parts(program, capture.type_reference)
            .is_some()
        {
            // A completed descriptor requires its whole invocation resource.
            // Missing resolution is not evidence of caller-owned storage.
            return None;
        }
        return Some(None);
    };
    // One home may have several supplying permissions. Distinct alternatives
    // cannot be collapsed to whichever resource happened to be encountered first.
    if roots.any(|candidate| candidate != root) {
        return None;
    }
    let typed_state = program.state_by_symbol(state)?;
    let root_type = validation::canonical_place_type_reference(
        program,
        state,
        statement,
        &validation::CanonicalPlace {
            root,
            segments: Vec::new(),
        },
    )?;
    if validation::reference_result_custody::stored_loan_parts(program, root_type).is_some() {
        // Owning a descriptor is not owning provider/caller storage it denotes.
        return Some(None);
    }
    if let Some(symbol) = root.as_symbol() {
        if program
            .state_parameter_by_symbol(typed_state, symbol)
            .is_some()
        {
            return Some(None);
        }
        let source = super::super::local_storage::source(program, typed_state, operations, symbol);
        // A named root whose occurrence is absent from the operation prefix is
        // caller-owned storage, not missing evidence. `stored_loan_parts` above
        // already refused the descriptor case, where an absent occurrence would
        // mean a produced reference whose provenance was lost; a scalar local
        // declared before these operations simply has no structural occurrence
        // to find, exactly as the producer search below concludes when it finds
        // no producer for its root.
        return match source {
            Some(source) => source.current_result_binding(operations).map(Some),
            None => Some(None),
        };
    }
    let mut producers = operations.iter().filter(|operation| {
        super::super::cleanup::normal_edges::producer_root(facts, state, operation) == Some(root)
    });
    let Some(producer) = producers.next() else {
        return Some(None);
    };
    if producers.next().is_some() {
        return None;
    }
    Some(Some(producer.structural_result_binding()?.binding_ordinal))
}

pub(super) fn retired_carrier_backing(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    statement: usize,
    target: SymbolHandle,
    arguments: &[ExpressionHandle],
    argument_position: usize,
    operations: &[CheckedUnitEffectOperationPlan],
    subject: &CheckedUnitStructuralArgumentPlan,
) -> Option<u32> {
    if subject.path.as_slice() != [CheckedUnitStructuralPathSegment::Referent] {
        return None;
    }
    let carrier = subject.source.current_result_binding(operations)?;
    let (formation, producer) = operations.iter().enumerate().find(|(_, operation)| {
        operation
            .structural_result_binding()
            .is_some_and(|result| result.binding_ordinal == carrier)
    })?;
    let lent = producer.established_reference_source(&facts.values.structural_values)?;
    if !lent.path.is_empty() {
        return None;
    }
    let backing = lent
        .source
        .current_result_binding(&operations[..formation])?;
    let backing_producer = operations[..formation].iter().find(|operation| {
        operation
            .structural_result_binding()
            .is_some_and(|result| result.binding_ordinal == backing)
    })?;
    let backing_root =
        super::super::cleanup::normal_edges::producer_root(facts, state, backing_producer)?;
    let carrier_root = super::super::cleanup::normal_edges::producer_root(facts, state, producer)?;
    let backing_type = validation::canonical_place_type_reference(
        program,
        state,
        statement,
        &validation::CanonicalPlace {
            root: backing_root,
            segments: Vec::new(),
        },
    )?;
    if validation::reference_shell(program, backing_type).is_some() {
        return None;
    }
    let released =
        operations
            .iter()
            .enumerate()
            .find_map(|(position, operation)| match operation {
                CheckedUnitEffectOperationPlan::ReleaseReference {
                    source,
                    path,
                    case_conditions,
                    loans,
                    select_evaluated_storage: false,
                    defer_to_exit: true,
                    ..
                } if path.is_empty()
                    && case_conditions.is_empty()
                    && source.current_result_binding(&operations[..position]) == Some(carrier) =>
                {
                    Some(*loans)
                }
                _ => None,
            })?;

    let call_handle = selected_call(facts, machine, state, statement, target, arguments)?;
    let call = facts.flow.control.calls.get(call_handle);
    let selected_operand = call.argument_operands().handles().nth(argument_position)?;
    let resources = facts
        .borrow
        .call_loan_resources
        .iter()
        .filter(|(_, resource)| {
            resource.call == call_handle
                && resource.machine_symbol == machine
                && resource.state_symbol == state
        })
        .map(|(_, resource)| resource)
        .collect::<Vec<_>>();
    let borrow_state =
        facts
            .borrow
            .with_state_index(|index| match index.bucket(machine, state) {
                [handle] => Some(facts.borrow.states.get(*handle)),
                _ => None,
            })?;
    let dependencies = Dependencies {
        program,
        facts,
        state,
        statement,
        borrow_state,
        forbidden: [backing_root, carrier_root],
    };
    let mut selected = false;
    for operand in call.argument_operands().handles() {
        let capture = facts.flow.control.operands.get(operand);
        let paths = validation::borrow_contracts::borrow_carrying_owner_paths(
            program,
            capture.type_reference,
        )?;
        let captured = resources
            .iter()
            .copied()
            .filter(|resource| resource.owner_operand == operand)
            .collect::<Vec<_>>();
        if !paths.iter().all(|path| {
            captured.iter().any(|resource| {
                facts.borrow.owner_segments.span(resource.owner_path) == Some(path.as_slice())
            })
        }) || captured.iter().any(|resource| {
            !paths.iter().any(|path| {
                facts.borrow.owner_segments.span(resource.owner_path) == Some(path.as_slice())
            })
        }) {
            return None;
        }
        for resource in captured {
            if operand == selected_operand {
                // Rebinding one whole reference is not permission to retarget
                // another leaf or an alternative captured source.
                if !resource.owner_path.is_empty() || resource.parent_lifetime.root != backing_root
                {
                    return None;
                }
                let CheckedCallBorrowLoanSource::CallAccess { access, parents } = resource.source
                else {
                    return None;
                };
                if facts.borrow.argument_accesses.get(access).expression != capture.expression {
                    return None;
                }
                let parents = facts.borrow.call_loan_parents.span(parents)?;
                let released = facts.borrow.loan_group(released)?.collect::<Vec<_>>();
                if released.is_empty()
                    || !released.iter().all(|(loan, _)| parents.contains(loan))
                    || parents.iter().any(|parent| {
                        !released.iter().any(|(loan, _)| loan == parent)
                            && !dependencies.loan(
                                *parent,
                                capture.constraints,
                                &mut Vec::new(),
                                &mut Vec::new(),
                            )
                    })
                {
                    return None;
                }
                selected = true;
                continue;
            }
            if !dependencies.root(
                resource.parent_lifetime.root,
                call.entry_constraints,
                &mut Vec::new(),
            ) {
                return None;
            }
            let loans = match resource.source {
                CheckedCallBorrowLoanSource::StoredLoan { loan } => vec![loan],
                CheckedCallBorrowLoanSource::CallAccess { parents, .. } => {
                    facts.borrow.call_loan_parents.span(parents)?.to_vec()
                }
            };
            for loan in loans {
                if !dependencies.loan(loan, capture.constraints, &mut Vec::new(), &mut Vec::new()) {
                    return None;
                }
            }
        }
    }
    // The moved home carries its own reference leaves too. Exclude self- or
    // mutually backed storage even when the outgoing formal is just &Home.
    if !selected || !dependencies.contents(backing_root, call.entry_constraints, &mut Vec::new()) {
        return None;
    }
    Some(backing)
}

struct Dependencies<'a> {
    program: &'a dyn validation::CheckingInput,
    facts: &'a CheckFacts,
    state: SymbolHandle,
    statement: usize,
    borrow_state: &'a checked_trees::StateBorrowFact,
    forbidden: [PlaceRoot; 2],
}

impl Dependencies<'_> {
    fn root(
        &self,
        root: PlaceRoot,
        constraints: HandleSpan<FlowConstraintRef>,
        active: &mut Vec<PlaceRoot>,
    ) -> bool {
        !self.forbidden.contains(&root) && self.contents(root, constraints, active)
    }

    fn contents(
        &self,
        root: PlaceRoot,
        constraints: HandleSpan<FlowConstraintRef>,
        active: &mut Vec<PlaceRoot>,
    ) -> bool {
        if active.contains(&root) {
            return false;
        }
        let Some(mut reference) = validation::canonical_place_type_reference(
            self.program,
            self.state,
            self.statement,
            &validation::CanonicalPlace {
                root,
                segments: Vec::new(),
            },
        ) else {
            return false;
        };
        if root.as_symbol().is_some_and(|symbol| {
            self.program
                .state_by_symbol(self.state)
                .is_some_and(|state| {
                    self.program
                        .state_parameter_by_symbol(state, symbol)
                        .is_some()
                })
        }) {
            // A borrowed parameter names its caller-supplied referent. Its
            // root permission is not a stored descriptor requiring a loan row;
            // only references inside that referent join the stored frontier.
            let Some(carrier) =
                validation::reference_result_custody::input_carrier_type(self.program, reference)
            else {
                return false;
            };
            reference = carrier;
        }
        let Some(paths) =
            validation::borrow_contracts::borrow_carrying_owner_paths(self.program, reference)
        else {
            return false;
        };
        let loans = self
            .facts
            .flow
            .borrow_loan_constraints(constraints)
            .filter(|handle| self.facts.borrow.loans.get(*handle).storage_owner == root)
            .collect::<Vec<_>>();
        if !paths.iter().all(|path| {
            loans.iter().any(|handle| {
                self.facts
                    .borrow
                    .loan_owner_path(self.facts.borrow.loans.get(*handle))
                    == path.as_slice()
            })
        }) || loans.iter().any(|handle| {
            !paths.iter().any(|path| {
                self.facts
                    .borrow
                    .loan_owner_path(self.facts.borrow.loans.get(*handle))
                    == path.as_slice()
            })
        }) {
            return false;
        }
        active.push(root);
        let independent = loans
            .into_iter()
            .all(|loan| self.loan(loan, constraints, active, &mut Vec::new()));
        active.pop();
        independent
    }

    fn loan(
        &self,
        handle: Handle<BorrowLoanFact>,
        constraints: HandleSpan<FlowConstraintRef>,
        active_roots: &mut Vec<PlaceRoot>,
        active_loans: &mut Vec<Handle<BorrowLoanFact>>,
    ) -> bool {
        if !self.facts.borrow.state_owns_loan(self.borrow_state, handle)
            || active_loans.contains(&handle)
        {
            return false;
        }
        let loan = self.facts.borrow.loans.get(handle);
        if self.forbidden.contains(&loan.root) {
            return false;
        }
        // Incoming referents predate this state's independent local storage.
        // Their identity is not the descriptor slot named by their root.
        let incoming = self
            .facts
            .borrow
            .incoming_references
            .is_valid(loan.incoming_reference)
            && {
                let identity = self
                    .facts
                    .borrow
                    .incoming_references
                    .get(loan.incoming_reference);
                identity.machine_symbol == self.borrow_state.machine_symbol
                    && identity.state_symbol == self.state
                    && matches!(identity.origin,
                        checked_trees::IncomingReferenceOrigin::Parameter { symbol }
                            if loan.root == PlaceRoot::Symbol(symbol))
            };
        if loan.incoming_reference.is_valid() && !incoming {
            return false;
        }
        if !incoming && !self.root(loan.root, constraints, active_roots) {
            return false;
        }
        active_loans.push(handle);
        let parent_independent = match loan.lineage {
            checked_trees::BorrowLoanLineage::Reborrow { parent_loan } => {
                self.loan(parent_loan, constraints, active_roots, active_loans)
            }
            checked_trees::BorrowLoanLineage::DirectRoot => true,
            // A keepalive obligation does not locate provider-owned storage.
            checked_trees::BorrowLoanLineage::LifetimeDependency => false,
            // Transport has no new reborrow parent. Its exact incoming
            // identity or complete supplier relation still names its backing;
            // this independence query grants no restoration authority.
            checked_trees::BorrowLoanLineage::UnretainedDerived => {
                incoming || !loan.supplying_loans.is_empty()
            }
        };
        let independent = parent_independent
            && self
                .facts
                .borrow
                .loan_suppliers
                .span(loan.supplying_loans)
                .is_some_and(|suppliers| {
                    suppliers.iter().all(|supplier| {
                        self.loan(*supplier, constraints, active_roots, active_loans)
                    })
                });
        active_loans.pop();
        independent
    }
}

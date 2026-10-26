//! One live-loan judgment for every access to storage.
//!
//! Reads, writes, call operands and exclusive operations name a place and an
//! access kind. A place rooted at a local reference that owns a live loan
//! denotes the storage that loan captured, not the local's name, so the
//! access is first rejoined to every backing it may denote. Storage identity
//! and authority are separate judgments: each backing is compared with every
//! live loan, and only the access's own retained lineage is exempt. A child
//! formed through the accessing alias is not part of that lineage, so an
//! exclusive child suspends the branch it captured for any other access until
//! its last use, and a shared child freezes that branch's mutation.

use arena::{Handle, HandleSpan};
use checked_trees::{
    BorrowAccessKind, BorrowFacts, BorrowLoanFact, CapturedPlace, CheckFacts, FlowConstraintRef,
    FlowStateFact,
};

use super::access_sources;
use super::aliases::ResolvedAlias;
use super::overlap::{StatedOrderingPremise, captured_place_compatibility};

/// Every backing an access of `place` may denote, with the retained lineage
/// that grants its authority. A root that owns none of the live loans (a
/// parameter, an owner, a reference initialized from static text) names its
/// own storage. A local reference that owns one is rejoined to the storage
/// its loans captured; `None` when that backing cannot be established.
pub(in crate::checks::borrows) fn place_backings(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    statement_index: usize,
    live_loans: impl Iterator<Item = Handle<BorrowLoanFact>>,
    place: CapturedPlace,
) -> Option<Vec<ResolvedAlias>> {
    let live_loans = live_loans.collect::<Vec<_>>();
    if !live_loans
        .iter()
        .any(|loan| facts.borrow.loans.get(*loan).storage_owner == place.root)
    {
        return Some(vec![ResolvedAlias {
            place,
            lineage: Vec::new(),
        }]);
    }
    access_sources::resolve_captured_with_loans(
        program,
        facts,
        state,
        statement_index,
        live_loans.into_iter(),
        place,
    )
}

/// Backing authority and exact invocation ancestry remain separate from
/// storage coincidence. Only the completed result's own input occurrence can
/// continue a temporary invocation hold without creating a second loan.
#[derive(Default)]
pub(in crate::checks::borrows) struct ArgumentBackings {
    pub(in crate::checks::borrows) places: Vec<ResolvedAlias>,
    capture: Handle<checked_trees::FlowOperandFact>,
    forwarded_inputs: Vec<Handle<checked_trees::FlowOperandFact>>,
}

impl ArgumentBackings {
    /// The resolver's exact actual occurrence, independently of its source
    /// expression or the places its descriptor happens to denote.
    pub(in crate::checks::borrows) fn represents(
        &self,
        capture: Handle<checked_trees::FlowOperandFact>,
    ) -> bool {
        capture.is_valid() && self.capture == capture
    }

    pub(in crate::checks::borrows) fn forwards(&self, input: &Self) -> bool {
        input.capture.is_valid()
            && self.capture != input.capture
            && self.forwarded_inputs.contains(&input.capture)
    }
}

/// A forwarded descriptor lends the referent captured when that actual
/// completed. Its private carrier read has finished; later actuals may replace
/// that carrier without retargeting the earlier loan. Existing alias resolution
/// still supplies lineage, using that observation's own live-loan constraints.
pub(in crate::checks::borrows) fn argument_backings(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    call: &checked_trees::BorrowCallFact,
    entry_constraints: HandleSpan<FlowConstraintRef>,
    access: &checked_trees::BorrowArgumentAccessFact,
) -> Result<ArgumentBackings, &'static str> {
    let place = CapturedPlace {
        incoming_reference: Default::default(),
        root: access.root,
        segments: facts.borrow.access_segments(access).to_vec(),
    };

    let selected = argument_capture(program, facts, state, call, access);
    let forwarded = !matches!(
        program.expression_table.expression(access.expression),
        typed_trees::expression::ExpressionNode::Borrow(_)
    ) && (validation::expression_type_reference_in_state(
        program,
        state.state_symbol,
        call.statement_index,
        access.expression,
    )
    .is_some_and(|reference| validation::reference_shell(program, reference).is_some())
        || selected.is_some_and(|capture| {
            super::completed_backings::value_observation_descriptor_source(program, facts, capture)
                .is_ok()
        }));
    if !forwarded {
        return place_backings(
            program,
            facts,
            state,
            call.statement_index,
            facts.flow.borrow_loan_constraints(entry_constraints),
            place,
        )
        .map(|places| ArgumentBackings {
            places,
            capture: selected.unwrap_or_default(),
            forwarded_inputs: Vec::new(),
        })
        .ok_or("the source place has no retained authority");
    }
    let selected = selected.ok_or_else(|| {
        if std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_some() {
            trace_argument_capture_refusal(facts, state, call, access);
        }
        "the access has no unique completed operand"
    })?;
    let resolved = super::completed_backings::resolve(program, facts, state, selected)?;
    Ok(ArgumentBackings {
        places: resolved.places,
        capture: selected,
        forwarded_inputs: resolved.forwarded_inputs,
    })
}

fn argument_capture(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    call: &checked_trees::BorrowCallFact,
    access: &checked_trees::BorrowArgumentAccessFact,
) -> Option<Handle<checked_trees::FlowOperandFact>> {
    let mut own_calls = facts
        .flow
        .control
        .calls
        .span(state.calls)?
        .iter()
        .filter(|source| {
            source.statement_index == call.statement_index
                && source.call_ordinal == call.call_ordinal
        });
    let Some(source) = own_calls.next() else {
        let observation = storage_address_observation(program, facts, state, call)?;
        let receiver = observation.receiver;
        return (facts.flow.control.operands.get(receiver).expression == access.expression)
            .then_some(receiver);
    };
    if own_calls.next().is_some()
        || source.target_symbol != call.target_symbol
        || source.accesses != call.accesses
    {
        return None;
    }
    let mut pending = Vec::new();
    for source in facts
        .flow
        .control
        .calls
        .span(state.calls)?
        .iter()
        .filter(|source| source.statement_index == call.statement_index)
    {
        pending.extend(source.operands.handles());
    }
    // Builtin observations have no selected callable row. Their actual
    // evaluated receiver is retained by the statement observation owner;
    // Member/Indexed share this exact handle with the operand backing graph.
    // Equality of expressions or addresses never deduplicates occurrences.
    for statement in facts.flow.control.statements.span(state.statements)? {
        if statement.statement_index == call.statement_index {
            pending.extend(
                facts
                    .flow
                    .control
                    .value_observations
                    .span(statement.value_observations)?
                    .iter()
                    .map(|observation| observation.receiver),
            );
        }
    }
    let mut visited = Vec::new();
    let mut selected = Handle::invalid();
    while let Some(handle) = pending.pop() {
        if visited.contains(&handle) {
            continue;
        }
        visited.push(handle);
        if !facts.flow.control.operands.is_valid(handle) {
            return None;
        }
        let value = facts.flow.control.operands.get(handle);
        if value.state_symbol != state.state_symbol || value.statement_index != call.statement_index
        {
            continue;
        }
        if value.expression == access.expression {
            if selected.is_valid() {
                return None;
            }
            selected = handle;
            continue;
        }
        if value.backing.is_valid() {
            pending.push(value.backing);
        }
        pending.extend(value.evaluated_inputs.handles());
        match value.completion {
            checked_trees::FlowOperandCompletion::Constructed(children) => {
                pending.extend(
                    facts
                        .flow
                        .control
                        .operand_children
                        .span(children)?
                        .iter()
                        .map(|child| child.value),
                );
            }
            checked_trees::FlowOperandCompletion::Selected(alternatives) => {
                pending.extend(
                    facts
                        .flow
                        .control
                        .operand_alternatives
                        .span(alternatives)?
                        .iter()
                        .map(|alternative| alternative.value),
                );
            }
            checked_trees::FlowOperandCompletion::Observation
            | checked_trees::FlowOperandCompletion::Copied { .. } => {}
        }
    }
    selected.is_valid().then_some(selected)
}

/// Address exposure is a compiler operation, not a machine invocation. Its
/// shared observation owner retains both the exact input and the post-operand
/// cut. Capture selection and conflict checking consume this same occurrence;
/// a missing ordinary call never authorizes an arena/expression fallback.
pub(super) fn storage_address_observation<'facts>(
    program: &dyn validation::CheckingInput,
    facts: &'facts CheckFacts,
    state: &FlowStateFact,
    call: &checked_trees::BorrowCallFact,
) -> Option<&'facts checked_trees::FlowValueObservationFact> {
    let control = &facts.flow.control;
    if !call.authored_expression.is_valid()
        || control.calls.span(state.calls)?.iter().any(|source| {
            source.statement_index == call.statement_index
                && source.call_ordinal == call.call_ordinal
        })
    {
        return None;
    }
    let typed_trees::expression::ExpressionNode::Call(operation) = program
        .expression_table
        .expression(call.authored_expression)
    else {
        return None;
    };
    let input = validation::compiler_operations::storage_address_operand(program, operation)?;
    let mut selected = None;
    for statement in control.statements.span(state.statements)? {
        if statement.statement_index != call.statement_index {
            continue;
        }
        for observation in control
            .value_observations
            .span(statement.value_observations)?
        {
            if observation.expression != call.authored_expression {
                continue;
            }
            if selected.is_some() || !control.operands.is_valid(observation.receiver) {
                return None;
            }
            let value = control.operands.get(observation.receiver);
            if value.expression != input
                || value.state_symbol != state.state_symbol
                || value.statement_index != call.statement_index
                || value.constraints != observation.constraints
            {
                return None;
            }
            selected = Some(observation);
        }
    }
    selected
}

/// Opt-in refusal context reads immutable occurrence inventories only.
/// Arena-wide matches are reported for diagnosis, never chosen as a fallback.
fn trace_argument_capture_refusal(
    facts: &CheckFacts,
    state: &FlowStateFact,
    call: &checked_trees::BorrowCallFact,
    access: &checked_trees::BorrowArgumentAccessFact,
) {
    let control = &facts.flow.control;
    eprintln!(
        "CHECKSITE argument-capture state={:?} statement={} ordinal={} access={:?}",
        state.state_symbol, call.statement_index, call.call_ordinal, access
    );
    for invocation in control.calls.span_or_empty(state.calls) {
        if invocation.statement_index == call.statement_index {
            eprintln!("CHECKSITE argument-capture-invocation {:?}", invocation);
        }
    }
    for (handle, value) in control.operands.iter() {
        if value.state_symbol == state.state_symbol
            && value.statement_index == call.statement_index
            && value.expression == access.expression
        {
            eprintln!(
                "CHECKSITE argument-capture-candidate {:?} {:?}",
                handle, value
            );
        }
    }
}

/// Whether an access of `kind` through `backings` interferes with the live
/// loan `loan_handle`.
pub(in crate::checks::borrows) fn access_interferes<'p>(
    program: &'p dyn ::validation::CheckingInput,
    borrow: &BorrowFacts,
    backings: &[ResolvedAlias],
    kind: &BorrowAccessKind,
    loan_handle: Handle<BorrowLoanFact>,
    premises: &[StatedOrderingPremise],
    bound_lookup: &mut Option<validation::ImmutableBoundLookup<'p>>,
) -> bool {
    let loan = borrow.loans.get(loan_handle);
    !backings.iter().all(|backing| {
        if backing.retains_loan(borrow, loan_handle) {
            return true;
        }
        // Distinct leaves returned together share an input lifetime, not
        // storage authority. Through another leaf of that same call compare
        // the actual referents; overlapping exclusive leaves still conflict.
        // External accesses and ancestor aliases keep the full input held.
        let shares_call = loan.retained_operand.is_some_and(|retained| {
            retained.formation_expression.is_valid()
                && backing.lineage.iter().any(|ancestor| {
                    let ancestor = borrow.loans.get(*ancestor);
                    ancestor.root == loan.root
                        && ancestor.incoming_reference == loan.incoming_reference
                        && ancestor.retained_operand == Some(retained)
                        // A parent and child inherit the same occurrence too.
                        // Only disjoint descriptor referents are peers; a
                        // projected access through the parent cannot fake it.
                        && captured_place_compatibility(
                            program,
                            &CapturedPlace {
                                incoming_reference: ancestor.incoming_reference,
                                root: ancestor.root,
                                segments: borrow.loan_segments(ancestor).to_vec(),
                            },
                            &ancestor.kind,
                            &CapturedPlace {
                                incoming_reference: loan.incoming_reference,
                                root: loan.root,
                                segments: borrow.loan_segments(loan).to_vec(),
                            },
                            &loan.kind,
                            premises,
                            bound_lookup,
                        ).disjoint
                })
        });
        let segments = borrow.loan_segments(loan);
        let depth = loan
            .retained_operand
            .filter(|_| !shares_call)
            .map_or(segments.len(), |retained| retained.depth);
        let loan_place = CapturedPlace {
            incoming_reference: loan.incoming_reference,
            root: loan.root,
            segments: segments[..depth].to_vec(),
        };
        captured_place_compatibility(
            program,
            &backing.place,
            kind,
            &loan_place,
            &loan.kind,
            premises,
            bound_lookup,
        )
        .non_interfering
    })
}

//! Join a completed operand to its existing call access or stored loan.
//!
//! Call entry is too late to recover an earlier operand's descriptor contents.
//! Resolve checked accesses in its completion constraints, preserving the original
//! access occurrence and every retained ancestor. Constructed children use
//! their own evaluations and resolved destination paths. Missing provenance
//! produces no resource: carry must still establish complete leaf coverage.
//! Lifetime retention keeps the complete enclosing owner, not a second copy
//! of referent geometry; the evaluated actual already owns its captured address.
//! A terminal region retains that exact footprint. It need not have one scalar
//! type, but its collection element must have no unresolved permission carriers.
//! These rows retain demands; they authorize no access, restoration or parking.

use super::permission_sources;
use crate::checks::borrows::loan_occurrences::LoanOccurrences;

use crate::published::places::place_at;
use arena::{Arena, Handle, HandleSpan};
use checked_trees::{
    BorrowLoanFact, BorrowLoanOwnerSegment, CapturedPlace, CheckFacts,
    CheckedCallBorrowLoanResource, CheckedCallBorrowLoanSource, CheckedDirectBorrowParentLifetime,
    FlowCallFact, FlowOperandFact, FlowStateFact,
};

pub(super) struct ResourceDraft {
    pub(super) resource: CheckedCallBorrowLoanResource,
    pub(super) path: Vec<BorrowLoanOwnerSegment>,
    pub(super) backing: CapturedPlace,
}

pub(super) fn install(program: &dyn validation::CheckingInput, facts: &mut CheckFacts) {
    let prepared_loans = LoanOccurrences::new(program, facts);
    let loan_occurrences = &prepared_loans;
    let mut resources = Vec::new();
    let mut parents = Arena::new();
    for (_, state) in facts.flow.control.states.iter() {
        for call in state.calls.handles() {
            let row = facts.flow.control.calls.get(call);
            for operand in row.argument_operands().handles() {
                collect(
                    program,
                    facts,
                    loan_occurrences,
                    state,
                    call,
                    operand,
                    operand,
                    &[],
                    &mut resources,
                    &mut parents,
                );
            }
        }
    }
    drop(prepared_loans);
    facts.borrow.call_loan_resources.reset_retain_capacity();
    facts.borrow.call_loan_paths.reset_retain_capacity();
    facts.borrow.call_loan_backing_paths.reset_retain_capacity();
    facts.borrow.call_loan_parents = parents;
    for draft in resources {
        let _ = install_resource(program, facts, draft);
    }
}

/// Publish one captured permission without changing any already published
/// handle. Completed output debts use the same original-source coordinates
/// as ordinary call inputs, even when the actual's slot has since changed.
pub(super) fn install_resource(
    program: &dyn validation::CheckingInput,
    facts: &mut CheckFacts,
    mut draft: ResourceDraft,
) -> Option<Handle<CheckedCallBorrowLoanResource>> {
    if !facts
        .flow
        .control
        .operands
        .is_valid(draft.resource.owner_operand)
    {
        return None;
    }
    let owner = facts
        .flow
        .control
        .operands
        .get(draft.resource.owner_operand);
    let path = crate::borrow::carrier_path(program, owner.type_reference, &draft.path)?;
    if draft.backing.root != draft.resource.parent_lifetime.root {
        return None;
    }
    draft.resource.backing_incoming_reference = draft.backing.incoming_reference;
    draft.resource.backing_incoming_owner_path = facts
        .borrow
        .call_loan_backing_paths
        .insert_many(draft.backing.incoming_owner_path);
    draft.resource.backing_path = facts
        .borrow
        .call_loan_backing_paths
        .insert_many(draft.backing.segments);
    draft.resource.capture_path = facts.borrow.call_loan_paths.insert_many(path);
    draft.resource.owner_path = facts.borrow.owner_segments.insert_many(draft.path);
    Some(facts.borrow.call_loan_resources.insert(draft.resource))
}

fn collect(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    loan_occurrences: &LoanOccurrences<'_>,
    state: &FlowStateFact,
    call: Handle<FlowCallFact>,
    owner_operand: Handle<FlowOperandFact>,
    operand: Handle<FlowOperandFact>,
    path: &[BorrowLoanOwnerSegment],
    resources: &mut Vec<ResourceDraft>,
    parents: &mut Arena<Handle<BorrowLoanFact>>,
) {
    if !facts.flow.control.operands.is_valid(operand) {
        return;
    }
    let capture = facts.flow.control.operands.get(operand);
    let Some(paths) =
        crate::borrow::view_link::borrow_carrying_owner_paths(program, capture.type_reference)
    else {
        return;
    };
    if paths.is_empty() {
        return;
    }
    if let checked_trees::FlowOperandCompletion::Selected(alternatives) = capture.completion {
        let Some(alternatives) = facts.flow.control.operand_alternatives.span(alternatives) else {
            return;
        };
        if alternatives.is_empty() {
            return;
        }
        let mut complete = Vec::new();
        for alternative in alternatives {
            if !alternative.source_arm.is_valid() {
                return;
            }
            let mut branch = Vec::new();
            collect(
                program,
                facts,
                loan_occurrences,
                state,
                call,
                owner_operand,
                alternative.value,
                path,
                &mut branch,
                parents,
            );
            if !paths.iter().all(|leaf| {
                branch
                    .iter()
                    .any(|draft| draft.path.strip_prefix(path) == Some(leaf.as_slice()))
            }) {
                return;
            }
            complete.extend(branch);
        }
        resources.extend(complete);
        return;
    }
    if !capture.constructed_children().is_empty() {
        for child in facts
            .flow
            .control
            .operand_children
            .span_or_empty(capture.constructed_children())
        {
            let mut child_path = path.to_vec();
            child_path.extend(crate::borrow::owner_path_from_place_segments(
                program,
                facts
                    .flow
                    .control
                    .operand_child_paths
                    .span_or_empty(child.path),
            ));
            collect(
                program,
                facts,
                loan_occurrences,
                state,
                call,
                owner_operand,
                child.value,
                &child_path,
                resources,
                parents,
            );
        }
        return;
    }
    if let Some(reference) = validation::reference_shell(program, capture.type_reference) {
        let access = checked_trees::BorrowAccessKind::from(reference.access);
        collect_suppliers(
            program,
            facts,
            loan_occurrences,
            state,
            call,
            owner_operand,
            operand,
            operand,
            &[],
            path,
            &access,
            false,
            &mut Vec::new(),
            resources,
            parents,
        );
        return;
    }
    let reference_paths =
        validation::borrow_contracts::carried_reference_paths(program, capture.type_reference);
    for leaf in paths {
        let leaf_path = crate::borrow::owner_place_segments(&leaf);
        if let Some(access) = reference_paths.as_ref().and_then(|paths| {
            paths
                .iter()
                .find_map(|(path, access)| (path == &leaf).then_some(access))
        }) && let Some(selected) = validation::completed_storage::reference_captures_at_path(
            program,
            &facts.flow.control,
            operand,
            &leaf_path,
            capture.storage_snapshot,
        ) {
            // A reached descriptor version supersedes the same slot's old
            // statement loan, even when its supplier cannot be recovered.
            // Missing coverage is conservative; stale provenance is unsound.
            if let Some(selected) = selected.captures() {
                let mut owner_path = path.to_vec();
                owner_path.extend(leaf.iter().copied());
                let mut complete = Vec::new();
                let mut resolved = !selected.is_empty();
                for selected in selected {
                    if !collect_suppliers(
                        program,
                        facts,
                        loan_occurrences,
                        state,
                        call,
                        owner_operand,
                        operand,
                        selected.capture,
                        &selected.projection,
                        &owner_path,
                        access,
                        false,
                        &mut Vec::new(),
                        &mut complete,
                        parents,
                    ) {
                        resolved = false;
                        break;
                    }
                }
                if resolved {
                    resources.extend(complete);
                }
            }
            continue;
        }
        let Some(source) = place_at(
            program,
            state.state_symbol,
            capture.statement_index,
            capture.expression,
        ) else {
            continue;
        };
        let mut source_path =
            crate::borrow::owner_path_from_place_segments(program, &source.segments);
        source_path.extend(leaf.iter().copied());
        for loan in facts
            .flow
            .borrow_loan_constraints(capture.constraints)
            .filter(|loan| {
                let loan = facts.borrow.loans.get(*loan);
                loan.storage_owner == source.root
                    && facts.borrow.loan_owner_path(loan) == source_path
            })
        {
            let row = facts.borrow.loans.get(loan);
            let mut owner_path = path.to_vec();
            owner_path.extend(leaf.iter().copied());
            resources.push(ResourceDraft {
                resource: CheckedCallBorrowLoanResource {
                    machine_symbol: state.machine_symbol,
                    state_symbol: state.state_symbol,
                    call,
                    owner_operand,
                    operand,
                    owner_path: HandleSpan::empty(),
                    capture_path: HandleSpan::empty(),
                    source: CheckedCallBorrowLoanSource::StoredLoan { loan },
                    access: row.kind.clone(),
                    parent_lifetime: CheckedDirectBorrowParentLifetime {
                        machine_symbol: state.machine_symbol,
                        state_symbol: state.state_symbol,
                        root: row.root,
                    },
                    backing_incoming_reference: Handle::invalid(),
                    backing_incoming_owner_path: HandleSpan::empty(),
                    backing_path: HandleSpan::empty(),
                },
                path: owner_path,
                backing: CapturedPlace {
                    incoming_reference: row.incoming_reference,
                    incoming_owner_path: facts
                        .borrow
                        .access_segments
                        .span_or_empty(row.incoming_owner_path)
                        .to_vec(),
                    root: row.root,
                    segments: facts.borrow.loan_segments(row).to_vec(),
                },
            });
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn collect_suppliers(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    loan_occurrences: &LoanOccurrences<'_>,
    state: &FlowStateFact,
    call: Handle<FlowCallFact>,
    owner_operand: Handle<FlowOperandFact>,
    operand: Handle<FlowOperandFact>,
    source: Handle<FlowOperandFact>,
    projection: &[facts::PlaceSegment],
    path: &[BorrowLoanOwnerSegment],
    output_access: &checked_trees::BorrowAccessKind,
    retained_by_store: bool,
    active: &mut Vec<(Handle<FlowOperandFact>, Vec<facts::PlaceSegment>)>,
    output: &mut Vec<ResourceDraft>,
    parents: &mut Arena<Handle<BorrowLoanFact>>,
) -> bool {
    let mut suppliers = Vec::new();
    if !permission_sources::collect(
        program,
        facts,
        loan_occurrences,
        state,
        call,
        source,
        projection,
        output_access,
        retained_by_store,
        active,
        &mut suppliers,
        parents,
        &mut permission_sources::PermissionFailures::default(),
    ) {
        return false;
    }
    output.extend(suppliers.into_iter().map(|supplier| ResourceDraft {
        resource: CheckedCallBorrowLoanResource {
            machine_symbol: state.machine_symbol,
            state_symbol: state.state_symbol,
            call,
            owner_operand,
            operand,
            source: supplier.source,
            access: output_access.clone(),
            parent_lifetime: CheckedDirectBorrowParentLifetime {
                machine_symbol: state.machine_symbol,
                state_symbol: state.state_symbol,
                root: supplier.backing.root,
            },
            ..Default::default()
        },
        path: path.to_vec(),
        backing: supplier.backing,
    }));
    true
}

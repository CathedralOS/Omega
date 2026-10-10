//! Join a completed operand to its existing call access or stored loan.
//!
//! Call entry is too late to recover an earlier operand's descriptor contents.
//! Resolve checked accesses in its completion constraints, preserving the original
//! access occurrence and every retained ancestor. Constructed children use
//! their own evaluations and resolved destination paths. Missing provenance
//! produces no resource: carry must still establish complete leaf coverage.
//! Lifetime retention keeps the complete enclosing owner, not a second copy
//! of referent geometry; the evaluated actual already owns its captured address.
//! These rows retain demands; they authorize no access, restoration or parking.

use arena::{Arena, Handle, HandleSpan};
use checked_trees::{
    BorrowLoanFact, BorrowLoanOwnerSegment, CapturedPlace, CheckFacts,
    CheckedCallBorrowLoanResource, CheckedCallBorrowLoanSource, CheckedDirectBorrowParentLifetime,
    FlowCallFact, FlowOperandFact, FlowStateFact,
};

struct ResourceDraft {
    resource: CheckedCallBorrowLoanResource,
    path: Vec<BorrowLoanOwnerSegment>,
}

pub(super) fn install(program: &dyn validation::CheckingInput, facts: &mut CheckFacts) {
    let mut resources = Vec::new();
    let mut parents = Arena::new();
    for (_, state) in facts.flow.control.states.iter() {
        for call in state.calls.handles() {
            let row = facts.flow.control.calls.get(call);
            for operand in row.argument_operands().handles() {
                collect(
                    program,
                    facts,
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
    facts.borrow.call_loan_resources.reset_retain_capacity();
    facts.borrow.call_loan_paths.reset_retain_capacity();
    facts.borrow.call_loan_parents = parents;
    for mut draft in resources {
        let owner = facts
            .flow
            .control
            .operands
            .get(draft.resource.owner_operand);
        let Some(path) = crate::borrow::carrier_path(program, owner.type_reference, &draft.path)
        else {
            continue;
        };
        draft.resource.capture_path = facts.borrow.call_loan_paths.insert_many(path);
        draft.resource.owner_path = facts.borrow.owner_segments.insert_many(draft.path);
        facts.borrow.call_loan_resources.insert(draft.resource);
    }
}

fn collect(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
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
    if !capture.children.is_empty() {
        for child in facts
            .flow
            .control
            .operand_children
            .span_or_empty(capture.children)
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
    let invocation = facts.flow.control.calls.get(call);
    let accesses = invocation.accesses.handles().filter(|access| {
        let access = facts.borrow.argument_accesses.get(*access);
        access.expression == capture.expression && access.end_before_access.is_none()
    });
    if validation::reference_shell(program, capture.type_reference).is_some() {
        for access in accesses {
            let access_row = facts.borrow.argument_accesses.get(access);
            let source = CapturedPlace {
                incoming_reference: Handle::invalid(),
                root_symbol: access_row.root_symbol,
                segments: facts.borrow.access_segments(access_row).to_vec(),
            };
            let Some(backings) = crate::checks::borrows::access_sources::resolve_captured_all(
                program,
                facts,
                state,
                capture.statement_index,
                capture.constraints,
                source,
            ) else {
                continue;
            };
            for backing in backings {
                resources.push(ResourceDraft {
                    resource: CheckedCallBorrowLoanResource {
                        machine_symbol: state.machine_symbol,
                        state_symbol: state.state_symbol,
                        call,
                        owner_operand,
                        operand,
                        owner_path: HandleSpan::empty(),
                        capture_path: HandleSpan::empty(),
                        source: CheckedCallBorrowLoanSource::CallAccess {
                            access,
                            parents: parents.insert_many(backing.lineage.iter().copied()),
                        },
                        access: access_row.kind.clone(),
                        parent_lifetime: CheckedDirectBorrowParentLifetime {
                            machine_symbol: state.machine_symbol,
                            state_symbol: state.state_symbol,
                            root_symbol: backing.place.root_symbol,
                        },
                    },
                    path: path.to_vec(),
                });
            }
        }
        // A reference actual must have a retained invocation access. A copied
        // observation or a nested call's expired access cannot stand in for it.
        return;
    }
    let Some(source) = validation::canonical_place_from_expression_in_state(
        program,
        state.state_symbol,
        capture.statement_index,
        capture.expression,
    ) else {
        return;
    };
    let Some(root) = source.root.as_symbol() else {
        return;
    };
    let source_prefix = crate::borrow::owner_path_from_place_segments(program, &source.segments);
    for leaf in paths {
        let mut source_path = source_prefix.clone();
        source_path.extend(leaf.iter().copied());
        for loan in facts
            .flow
            .borrow_loan_constraints(capture.constraints)
            .filter(|loan| {
                let loan = facts.borrow.loans.get(*loan);
                loan.owner_symbol == root && facts.borrow.loan_owner_path(loan) == source_path
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
                        root_symbol: row.root_symbol,
                    },
                },
                path: owner_path,
            });
        }
    }
}

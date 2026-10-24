//! Resolve a reached descriptor's permission supplier without rereading its
//! former slot. Call stores retain exact source captures and projections; a
//! temporary call access can end while its installed descriptor continues to
//! carry that supplier. Neither equal geometry nor lifetime roots create this
//! association. Every alternative must resolve, and cycles withhold coverage.

use super::{
    Arena, BorrowLoanFact, BorrowLoanOwnerSegment, CheckFacts, FlowCallFact, FlowOperandFact,
    FlowStateFact, Handle, ResourceDraft,
};
use checked_trees::{
    CheckedCallBorrowLoanResource, CheckedCallBorrowLoanSource, CheckedDirectBorrowParentLifetime,
};

pub(super) fn collect(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
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
    let Some((_, borrow_state)) =
        crate::flow::borrow_state_fact(&facts.borrow, state.machine_symbol, state.state_symbol)
    else {
        return false;
    };
    let query = (source, projection.to_vec());
    if active.contains(&query) {
        return false;
    }
    active.push(query);
    let before = output.len();
    if !facts.flow.control.operands.is_valid(source) {
        active.pop();
        return false;
    }
    let capture = facts.flow.control.operands.get(source);
    let retained_suppliers = validation::completed_storage::descriptor_suppliers(
        &facts.flow.control,
        capture.retention,
        &mut Vec::new(),
    );
    let frozen = super::frozen_reference_leaf(program, &facts.flow.control, source, projection);
    let resolved = if matches!(&frozen, super::FrozenReferenceSelection::Unavailable) {
        false
    } else if let checked_trees::FlowOperandCompletion::Selected(alternatives) = capture.completion
    {
        facts
            .flow
            .control
            .operand_alternatives
            .span(alternatives)
            .is_some_and(|rows| {
                !rows.is_empty()
                    && rows.iter().all(|alternative| {
                        alternative.source_arm.is_valid()
                            && collect(
                                program,
                                facts,
                                state,
                                call,
                                owner_operand,
                                operand,
                                alternative.value,
                                projection,
                                path,
                                output_access,
                                retained_by_store,
                                active,
                                output,
                                parents,
                            )
                    })
            })
    } else if let super::FrozenReferenceSelection::Endpoint {
        owner,
        leaf,
        path: leaf_path,
    } = frozen
    {
        if let Some(suppliers) = validation::completed_storage::descriptor_suppliers(
            &facts.flow.control,
            leaf.retention,
            &mut Vec::new(),
        ) {
            !suppliers.is_empty()
                && suppliers.iter().all(|supplier| {
                    facts
                        .flow
                        .control
                        .operand_retained_paths
                        .span(supplier.projection)
                        .is_some_and(|projection| {
                            collect(
                                program,
                                facts,
                                state,
                                call,
                                owner_operand,
                                operand,
                                supplier.operand,
                                projection,
                                path,
                                output_access,
                                true,
                                active,
                                output,
                                parents,
                            )
                        })
                })
        } else {
            let coverage = if projection.is_empty()
                && validation::reference_shell(program, capture.type_reference).is_some()
            {
                collect_invocation_accesses(
                    program,
                    facts,
                    state,
                    call,
                    owner_operand,
                    operand,
                    capture,
                    path,
                    output_access,
                    retained_by_store,
                    output,
                    parents,
                )
            } else {
                InvocationAccessCoverage::Absent
            };
            match coverage {
                InvocationAccessCoverage::Complete => true,
                InvocationAccessCoverage::Unavailable => false,
                InvocationAccessCoverage::Absent => {
                    if facts
                        .flow
                        .control
                        .operand_referents
                        .span(leaf.referents)
                        .is_some_and(|referents| {
                            !referents.is_empty()
                                && referents.iter().all(|referent| {
                                    facts
                                        .flow
                                        .control
                                        .referent_formations
                                        .is_valid(referent.formation)
                                        && facts
                                            .flow
                                            .control
                                            .referent_formations
                                            .get(referent.formation)
                                            .owner
                                            .is_valid()
                                })
                        })
                        && let Some(source_place) =
                            validation::canonical_place_from_expression_in_state(
                                program,
                                owner.state_symbol,
                                owner.statement_index,
                                owner.expression,
                            )
                    {
                        // This is the private frozen inventory's exact original loan cut,
                        // not a recovery from the consumer's current source slot. Geometry
                        // and formation handles do not select or create a loan.
                        let mut source_path = crate::borrow::owner_path_from_place_segments(
                            program,
                            &source_place.segments,
                        );
                        source_path.extend(crate::borrow::owner_path_from_place_segments(
                            program, &leaf_path,
                        ));
                        for loan in facts.flow.borrow_loan_constraints(leaf.constraints) {
                            let row = facts.borrow.loans.get(loan);
                            if !facts.borrow.state_owns_loan(borrow_state, loan)
                                || row.storage_owner != source_place.root
                                || facts.borrow.loan_owner_path(row) != source_path
                            {
                                continue;
                            }
                            output.push(ResourceDraft {
                                resource: CheckedCallBorrowLoanResource {
                                    machine_symbol: state.machine_symbol,
                                    state_symbol: state.state_symbol,
                                    call,
                                    owner_operand,
                                    operand,
                                    source: CheckedCallBorrowLoanSource::StoredLoan { loan },
                                    access: output_access.clone(),
                                    parent_lifetime: CheckedDirectBorrowParentLifetime {
                                        machine_symbol: state.machine_symbol,
                                        state_symbol: state.state_symbol,
                                        root: row.root,
                                    },
                                    ..Default::default()
                                },
                                path: path.to_vec(),
                                backing: checked_trees::CapturedPlace {
                                    incoming_reference: row.incoming_reference,
                                    root: row.root,
                                    segments: facts.borrow.loan_segments(row).to_vec(),
                                },
                            });
                        }
                        output.len() > before
                    } else {
                        false
                    }
                }
            }
        }
    } else if !projection.is_empty() {
        let selected = validation::completed_storage::reference_captures_at_path(
            program,
            &facts.flow.control,
            source,
            projection,
            capture.storage_snapshot,
        )
        .and_then(|selected| selected.captures());
        selected.is_some_and(|selected| {
            !selected.is_empty()
                && selected.into_iter().all(|selected| {
                    collect(
                        program,
                        facts,
                        state,
                        call,
                        owner_operand,
                        operand,
                        selected.capture,
                        &selected.projection,
                        path,
                        output_access,
                        retained_by_store,
                        active,
                        output,
                        parents,
                    )
                })
        })
    } else if let Some(suppliers) = retained_suppliers {
        !suppliers.is_empty()
            && suppliers.iter().all(|supplier| {
                collect(
                    program,
                    facts,
                    state,
                    call,
                    owner_operand,
                    operand,
                    supplier.operand,
                    match facts
                        .flow
                        .control
                        .operand_retained_paths
                        .span(supplier.projection)
                    {
                        Some(projection) => projection,
                        None => return false,
                    },
                    path,
                    output_access,
                    true,
                    active,
                    output,
                    parents,
                )
            })
    } else {
        match collect_invocation_accesses(
            program,
            facts,
            state,
            call,
            owner_operand,
            operand,
            capture,
            path,
            output_access,
            retained_by_store,
            output,
            parents,
        ) {
            InvocationAccessCoverage::Complete => true,
            InvocationAccessCoverage::Unavailable => false,
            InvocationAccessCoverage::Absent => {
                // A prior stored formation has no current invocation access. Its
                // exact occurrence, rather than the current slot path or backing
                // geometry, identifies the permission carried by this capture.
                for (loan, row) in facts.borrow.loans.iter().filter(|(handle, row)| {
                    facts.borrow.state_owns_loan(borrow_state, *handle)
                        && capture.expression.is_valid()
                        && row.formation_expression == capture.expression
                        && row.statement_index == capture.statement_index
                }) {
                    output.push(ResourceDraft {
                        resource: CheckedCallBorrowLoanResource {
                            machine_symbol: state.machine_symbol,
                            state_symbol: state.state_symbol,
                            call,
                            owner_operand,
                            operand,
                            source: CheckedCallBorrowLoanSource::StoredLoan { loan },
                            access: output_access.clone(),
                            parent_lifetime: CheckedDirectBorrowParentLifetime {
                                machine_symbol: state.machine_symbol,
                                state_symbol: state.state_symbol,
                                root: row.root,
                            },
                            ..Default::default()
                        },
                        path: path.to_vec(),
                        backing: checked_trees::CapturedPlace {
                            incoming_reference: row.incoming_reference,
                            root: row.root,
                            segments: facts.borrow.loan_segments(row).to_vec(),
                        },
                    });
                }
                output.len() > before
            }
        }
    };
    active.pop();
    if !resolved {
        output.truncate(before);
    }
    resolved
}

/// Exact checked access occurrences are permission evidence for a whole
/// evaluated descriptor, including one projected from an earlier owned copy.
/// A failed present roster cannot recover an older source-slot formation.
enum InvocationAccessCoverage {
    Absent,
    Complete,
    Unavailable,
}

fn collect_invocation_accesses(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    call: Handle<FlowCallFact>,
    owner_operand: Handle<FlowOperandFact>,
    operand: Handle<FlowOperandFact>,
    capture: &FlowOperandFact,
    path: &[BorrowLoanOwnerSegment],
    output_access: &checked_trees::BorrowAccessKind,
    retained_by_store: bool,
    output: &mut Vec<ResourceDraft>,
    parents: &mut Arena<Handle<BorrowLoanFact>>,
) -> InvocationAccessCoverage {
    let invocation = facts.flow.control.calls.get(call);
    let accesses = invocation
        .accesses
        .handles()
        .filter(|access| {
            let access = facts.borrow.argument_accesses.get(*access);
            access.expression == capture.expression
                && (retained_by_store || access.end_before_access.is_none())
        })
        .collect::<Vec<_>>();
    if accesses.is_empty() {
        return InvocationAccessCoverage::Absent;
    }
    let before = output.len();
    for access in accesses {
        let row = facts.borrow.argument_accesses.get(access);
        let Some(backings) = super::retained_places(
            program,
            facts,
            state,
            capture.statement_index,
            capture.constraints,
            checked_trees::CapturedPlace {
                incoming_reference: Handle::invalid(),
                root: row.root,
                segments: facts.borrow.access_segments(row).to_vec(),
            },
            &mut Vec::new(),
        ) else {
            output.truncate(before);
            return InvocationAccessCoverage::Unavailable;
        };
        if backings.is_empty() {
            output.truncate(before);
            return InvocationAccessCoverage::Unavailable;
        }
        for backing in backings {
            output.push(ResourceDraft {
                resource: CheckedCallBorrowLoanResource {
                    machine_symbol: state.machine_symbol,
                    state_symbol: state.state_symbol,
                    call,
                    owner_operand,
                    operand,
                    source: CheckedCallBorrowLoanSource::CallAccess {
                        access,
                        parents: parents.insert_many(backing.lineage),
                    },
                    access: output_access.clone(),
                    parent_lifetime: CheckedDirectBorrowParentLifetime {
                        machine_symbol: state.machine_symbol,
                        state_symbol: state.state_symbol,
                        root: backing.place.root,
                    },
                    ..Default::default()
                },
                path: path.to_vec(),
                backing: backing.place,
            });
        }
    }
    InvocationAccessCoverage::Complete
}

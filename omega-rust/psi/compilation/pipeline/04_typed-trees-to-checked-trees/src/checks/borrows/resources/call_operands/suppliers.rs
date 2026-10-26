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
    let Some((_, borrow_state)) = facts
        .borrow
        .first_state_borrow(state.machine_symbol, state.state_symbol)
    else {
        return false;
    };
    let query = (source, projection.to_vec());
    if active.contains(&query) {
        return false;
    }
    active.push(query);
    let before = output.len();
    let capture = facts.flow.control.operands.get(source);
    let retained_suppliers = validation::completed_storage::descriptor_suppliers(
        &facts.flow.control,
        capture.retention,
        &mut Vec::new(),
    );
    let resolved = if !capture.alternatives.is_empty() {
        capture.alternatives.handles().all(|alternative| {
            collect(
                program,
                facts,
                state,
                call,
                owner_operand,
                operand,
                alternative,
                projection,
                path,
                output_access,
                retained_by_store,
                active,
                output,
                parents,
            )
        })
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
                    // An opaque inline leaf can preserve its completed value
                    // occurrence without disclosing an exact descriptor supplier.
                    // Rejoin that occurrence's already-checked returned loan;
                    // recursively selecting the same subtree would only cycle.
                    if selected.capture == source && selected.projection == projection {
                        return collect_formed_projection(
                            program,
                            facts,
                            state,
                            call,
                            owner_operand,
                            operand,
                            capture,
                            projection,
                            path,
                            output_access,
                            output,
                        );
                    }
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
                    facts
                        .flow
                        .control
                        .operand_retained_paths
                        .span_or_empty(supplier.projection),
                    path,
                    output_access,
                    true,
                    active,
                    output,
                    parents,
                )
            })
    } else {
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
        if !accesses.is_empty() {
            accesses.into_iter().all(|access| {
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
                    return false;
                };
                if backings.is_empty() {
                    return false;
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
                    });
                }
                true
            })
        } else {
            // A prior stored formation has no current invocation access. Its
            // exact occurrence, rather than the current slot path or backing
            // geometry, identifies the permission carried by this capture.
            for (loan, row) in facts.borrow.state_loans(borrow_state).filter(|(_, row)| {
                capture.expression.is_valid()
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
                });
            }
            output.len() > before
        }
    };
    active.pop();
    if !resolved {
        output.truncate(before);
    }
    resolved
}

/// Retain the existing loan of one completed result leaf. Public backing
/// regions are deliberately absent from this lookup: they cannot identify a
/// supplier or create permission. Destination storage prefixes do not identify
/// the formation leaf either, so the checker records that relative path once.
#[allow(clippy::too_many_arguments)]
fn collect_formed_projection(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    call: Handle<FlowCallFact>,
    owner_operand: Handle<FlowOperandFact>,
    operand: Handle<FlowOperandFact>,
    capture: &FlowOperandFact,
    projection: &[facts::PlaceSegment],
    path: &[BorrowLoanOwnerSegment],
    output_access: &checked_trees::BorrowAccessKind,
    output: &mut Vec<ResourceDraft>,
) -> bool {
    if capture.state_symbol != state.state_symbol
        || !capture.expression.is_valid()
        || projection.iter().any(|segment| {
            !matches!(
                segment,
                facts::PlaceSegment::Field { .. }
                    | facts::PlaceSegment::Case { .. }
                    | facts::PlaceSegment::FixedIndex { .. }
            )
        })
    {
        return false;
    }
    let Some(declared) = validation::expression_type_reference_in_state(
        program,
        capture.state_symbol,
        capture.statement_index,
        capture.expression,
    ) else {
        return false;
    };
    if !validation::storage_origin_types_match(program, declared, capture.type_reference) {
        return false;
    }
    let Some((_, borrow_state)) = facts
        .borrow
        .first_state_borrow(state.machine_symbol, state.state_symbol)
    else {
        return false;
    };
    let before = output.len();
    for (loan, row) in facts.borrow.state_loans(borrow_state) {
        let Some(formed) = row.formation_owner_path else {
            continue;
        };
        if row.formation_expression != capture.expression
            || row.statement_index != capture.statement_index
            || row.kind.direct_reborrow_effect(output_access).is_none()
            || crate::borrow::owner_place_segments(
                facts.borrow.owner_segments.span_or_empty(formed),
            ) != projection
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
        });
    }
    output.len() > before
}

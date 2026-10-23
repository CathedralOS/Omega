//! Rebuilding the direct-borrow and reborrow resource arenas from the loan
//! and flow-lifetime ledgers, and resolving each reborrow's parent graph.
//!
//! One traversal over the flow ledger's state-owned loans projects both
//! rosters: a loan's recorded lineage decides whether it closes as a
//! direct-root resource or a reborrow draft, so neither roster can silently
//! omit loans the other sees. Every reconstruction joins the loan's
//! activation and weakening rows through the lifecycle reader.

use crate::checks::borrows::resources::invalid_reborrow_attenuation_diagnostic;
use crate::checks::borrows::resources::lifecycle::{
    exact_loan_lifecycle_rows, span_handle, weakening_boundary_key,
};
use crate::checks::borrows::resources::reborrow_drafts::{
    CheckedReborrowDispositionEventDraft, CheckedReborrowLoanResourceDraft, ParentResourceIndex,
    ResourceHandles,
};
use crate::checks::borrows::resources::reborrow_resource_drift;
use checked_trees::{
    BorrowFacts, BorrowLoanLineage, CheckedDirectBorrowLoanResource,
    CheckedDirectBorrowParentLifetime, CheckedDirectBorrowRestorationObligation,
    CheckedParentBorrowResource, CheckedReborrowLoanResource, FlowFacts, FlowInvalidationSource,
    ParentLexicalStatusAtChildEnd,
};
use diagnostics::Diagnostic;

/// Resolve the entire parent graph before either retained arena is reset.
/// Installation is therefore a purely indexed, infallible rewrite.
pub(crate) fn plan_resource_installation(
    direct: &[CheckedDirectBorrowLoanResource],
    reborrows: &[CheckedReborrowLoanResourceDraft],
) -> Result<Vec<ParentResourceIndex>, Vec<Diagnostic>> {
    let mut plan = Vec::with_capacity(reborrows.len());
    for (child_index, child) in reborrows.iter().enumerate() {
        let direct_matches = direct
            .iter()
            .enumerate()
            .filter(|(_, resource)| resource.loan == child.parent_loan)
            .map(|(index, _)| ParentResourceIndex::Direct(index));
        let reborrow_matches = reborrows[..child_index]
            .iter()
            .enumerate()
            .filter(|(_, resource)| resource.loan == child.parent_loan)
            .map(|(index, _)| ParentResourceIndex::Reborrow(index));
        let mut matches = direct_matches.chain(reborrow_matches);
        let Some(parent) = matches.next() else {
            return Err(reborrow_resource_drift());
        };
        if matches.next().is_some() {
            return Err(reborrow_resource_drift());
        }
        plan.push(parent);
    }
    Ok(plan)
}

pub(crate) fn install_borrow_resources(
    borrow: &mut BorrowFacts,
    direct: Vec<CheckedDirectBorrowLoanResource>,
    reborrows: &[CheckedReborrowLoanResourceDraft],
    installation: &[ParentResourceIndex],
    dispositions: &[CheckedReborrowDispositionEventDraft],
) {
    borrow.direct_loan_resources.reset_retain_capacity();
    borrow.reborrow_loan_resources.reset_retain_capacity();
    borrow.reborrow_disposition_events.reset_retain_capacity();

    let mut direct_handles = Vec::with_capacity(direct.len());
    for resource in direct {
        let handle = borrow.direct_loan_resources.insert(resource);
        direct_handles.push(handle);
    }

    let mut reborrow_handles: Vec<arena::Handle<CheckedReborrowLoanResource>> =
        Vec::with_capacity(reborrows.len());
    for (draft, parent) in reborrows.iter().zip(installation) {
        let parent_resource = match *parent {
            ParentResourceIndex::Direct(index) => CheckedParentBorrowResource::DirectRoot {
                resource: direct_handles[index],
            },
            ParentResourceIndex::Reborrow(index) => CheckedParentBorrowResource::Reborrow {
                resource: reborrow_handles[index],
            },
        };
        let handle = borrow
            .reborrow_loan_resources
            .insert(draft.close(parent_resource));
        reborrow_handles.push(handle);
    }

    let handles = ResourceHandles {
        direct: direct_handles,
        reborrows: reborrow_handles,
    };
    for disposition in dispositions {
        borrow
            .reborrow_disposition_events
            .insert(disposition.close(borrow, &handles));
    }
}

/// Rebuild both checked resource rosters in one traversal of the state-owned
/// loans: direct-root loans close as direct resources in their own arena
/// (every derived occurrence stays outside it), reborrows close as drafts for
/// the parent graph and disposition events. A loan's lineage selects its
/// roster; there is no second walk that could disagree about coverage.
pub(crate) fn reconstruct_borrow_resource_rosters(
    program: &dyn ::validation::CheckingInput,
    borrow: &BorrowFacts,
    flow: &FlowFacts,
) -> Result<
    (
        Vec<CheckedDirectBorrowLoanResource>,
        Vec<CheckedReborrowLoanResourceDraft>,
    ),
    Vec<Diagnostic>,
> {
    let mut direct = Vec::new();
    let mut reborrows = Vec::new();
    let mut diagnostics = Vec::new();

    for (_, state) in borrow.states.iter() {
        let Some(flow_state) = flow.control.state(state.machine_symbol, state.state_symbol) else {
            diagnostics.push(Diagnostic::error(
                "checked borrow resource has no exact flow-state owner",
            ));
            continue;
        };

        // Owner correspondence is required even for an empty roster, but no
        // loan in the program can belong to this state's empty span.
        if state.loans.is_empty() {
            continue;
        }

        for (loan_handle, loan) in borrow
            .loans
            .iter()
            .filter(|(handle, _)| borrow.state_owns_loan(state, *handle))
        {
            match &loan.lineage {
                BorrowLoanLineage::DirectRoot => {
                    // A direct-root loan formed on a reference-typed binding is
                    // still a referent reborrow: the binding carries no parent
                    // loan to rebase through (parameters and loan-less locals
                    // are the common cases), so its declared access is the
                    // parent authority and the access-pair rule decides the
                    // formation — `&write`/`&mut` on a `&u8` binding can never
                    // derive write authority from a shared referent. This is
                    // the same edge `Reborrow` loans face above the resource
                    // arenas, applied at the declared-access boundary.
                    if let Some(parent_access) =
                        crate::checks::borrows::details::binding_reference_access(
                            program,
                            state.machine_symbol,
                            state.state_symbol,
                            loan.statement_index,
                            loan.root,
                        )
                        && parent_access.direct_reborrow_effect(&loan.kind).is_none()
                    {
                        diagnostics.push(invalid_reborrow_attenuation_diagnostic(
                            &parent_access,
                            &loan.kind,
                        ));
                        continue;
                    }

                    let (activation_handle, weakening_handle) = match exact_loan_lifecycle_rows(
                        flow,
                        flow_state,
                        loan_handle,
                        loan.statement_index,
                    ) {
                        Ok(rows) => rows,
                        Err(drift) => {
                            diagnostics.push(drift);
                            continue;
                        }
                    };
                    let activation = flow.borrow_lifetimes.activations.get(activation_handle);
                    let weakening = flow.borrow_lifetimes.weakenings.get(weakening_handle);

                    let parent_lifetime = CheckedDirectBorrowParentLifetime {
                        machine_symbol: state.machine_symbol,
                        state_symbol: state.state_symbol,
                        root: loan.root,
                    };
                    let restoration = CheckedDirectBorrowRestorationObligation {
                        parent: parent_lifetime.clone(),
                        weakening_source: weakening.source,
                        weakening_reason: weakening.reason,
                    };
                    direct.push(CheckedDirectBorrowLoanResource {
                        loan: loan_handle,
                        machine_symbol: state.machine_symbol,
                        state_symbol: state.state_symbol,
                        owner_symbol: loan.owner_symbol,
                        owner_path: borrow.loan_owner_path(loan).to_vec(),
                        captured_place: checked_trees::CapturedPlace {
                            incoming_reference: loan.incoming_reference,
                            root: loan.root,
                            segments: borrow.loan_segments(loan).to_vec(),
                        },
                        access: loan.kind.clone(),
                        activation_source: activation.source,
                        weakening_source: weakening.source,
                        weakening_reason: weakening.reason,
                        parent_lifetime,
                        restoration,
                    });
                }

                BorrowLoanLineage::Reborrow { parent_loan } => {
                    let parent = borrow.loans.get(*parent_loan);
                    let Some(access_effect) = parent.kind.direct_reborrow_effect(&loan.kind) else {
                        diagnostics.push(invalid_reborrow_attenuation_diagnostic(
                            &parent.kind,
                            &loan.kind,
                        ));
                        continue;
                    };
                    let (child_activation, child_weakening) = match exact_loan_lifecycle_rows(
                        flow,
                        flow_state,
                        loan_handle,
                        loan.statement_index,
                    ) {
                        Ok(rows) => rows,
                        Err(drift) => {
                            diagnostics.push(drift);
                            continue;
                        }
                    };
                    let activation = flow.borrow_lifetimes.activations.get(child_activation);
                    let weakening = flow.borrow_lifetimes.weakenings.get(child_weakening);
                    let parent_weakenings = flow
                        .borrow_lifetimes
                        .weakenings
                        .span_or_empty(flow_state.borrow_weakenings)
                        .iter()
                        .enumerate()
                        .filter(|(_, weakening)| weakening.loan == *parent_loan)
                        .filter_map(|(offset, weakening)| {
                            span_handle(flow_state.borrow_weakenings, offset)
                                .map(|handle| (handle, weakening))
                        })
                        .collect::<Vec<_>>();
                    let [(parent_weakening, parent_weakening_fact)] = parent_weakenings.as_slice()
                    else {
                        diagnostics.push(Diagnostic::error(
                            "checked direct-reborrow parent status requires exactly one parent weakening",
                        ));
                        continue;
                    };
                    let Some(parent_lexical_status) = parent_lexical_status_at_child_end(
                        parent_weakening_fact.source,
                        parent_weakening_fact.reason,
                        weakening.source,
                        weakening.reason,
                    ) else {
                        diagnostics.push(Diagnostic::error(
                            "checked direct-reborrow parent status has an unsupported weakening boundary",
                        ));
                        continue;
                    };

                    let Some(statement) = flow
                        .control
                        .statements
                        .span_or_empty(flow_state.statements)
                        .iter()
                        .find(|statement| statement.statement_index == loan.statement_index)
                    else {
                        diagnostics.push(Diagnostic::error(
                            "checked direct-reborrow suspension has no exact formation statement",
                        ));
                        continue;
                    };
                    let parent_constraints = flow
                        .contexts
                        .constraint_refs
                        .span_or_empty(statement.entry_constraints)
                        .iter()
                        .enumerate()
                        .filter(|(_, constraint)| {
                            constraint.kind
                                == checked_trees::FlowConstraintKind::BorrowLoan {
                                    loan: *parent_loan,
                                }
                        })
                        .filter_map(|(offset, _)| span_handle(statement.entry_constraints, offset))
                        .collect::<Vec<_>>();
                    let [parent_entry_constraint] = parent_constraints.as_slice() else {
                        diagnostics.push(Diagnostic::error(
                            "checked direct-reborrow suspension requires exactly one parent entry constraint",
                        ));
                        continue;
                    };

                    reborrows.push(CheckedReborrowLoanResourceDraft {
                        loan: loan_handle,
                        machine_symbol: state.machine_symbol,
                        state_symbol: state.state_symbol,
                        owner_symbol: loan.owner_symbol,
                        owner_path: borrow.loan_owner_path(loan).to_vec(),
                        captured_place: checked_trees::CapturedPlace {
                            incoming_reference: loan.incoming_reference,
                            root: loan.root,
                            segments: borrow.loan_segments(loan).to_vec(),
                        },
                        access: loan.kind.clone(),
                        parent_access: parent.kind.clone(),
                        access_effect,
                        activation_source: activation.source,
                        weakening_source: weakening.source,
                        weakening_reason: weakening.reason,
                        parent_loan: *parent_loan,
                        child_activation,
                        parent_entry_constraint: *parent_entry_constraint,
                        child_weakening,
                        parent_weakening: *parent_weakening,
                        parent_lexical_status,
                    });
                }

                // Remaining lineages stay outside both rosters.
                BorrowLoanLineage::LifetimeDependency | BorrowLoanLineage::UnretainedDerived => {}
            }
        }
    }

    if diagnostics.is_empty() {
        Ok((direct, reborrows))
    } else {
        Err(diagnostics)
    }
}

fn parent_lexical_status_at_child_end(
    parent_source: FlowInvalidationSource,
    parent_reason: checked_trees::FlowBorrowWeakeningReason,
    child_source: FlowInvalidationSource,
    child_reason: checked_trees::FlowBorrowWeakeningReason,
) -> Option<ParentLexicalStatusAtChildEnd> {
    let parent = weakening_boundary_key(parent_source, parent_reason)?;
    let child = weakening_boundary_key(child_source, child_reason)?;
    Some(match parent.cmp(&child) {
        std::cmp::Ordering::Less => ParentLexicalStatusAtChildEnd::RetiredBeforeChild,
        std::cmp::Ordering::Equal => ParentLexicalStatusAtChildEnd::RetiredWithChild,
        std::cmp::Ordering::Greater => ParentLexicalStatusAtChildEnd::LivePastChild,
    })
}

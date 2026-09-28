//! Rebuilding the direct-borrow and reborrow resource arenas from the loan
//! and flow-lifetime ledgers.

use crate::checks::borrows::resources::lifecycle::weakening_boundary_key;
use crate::checks::borrows::resources::reborrow_drafts::{
    CheckedReborrowContainmentCertificateDraft, CheckedReborrowDispositionEventDraft,
    CheckedReborrowLoanResourceDraft, CheckedReborrowRestoredCallUseCertificateDraft,
    ParentResourceIndex, ResourceHandles,
};
use crate::checks::borrows::resources::retained_validation::invalid_reborrow_attenuation_diagnostic;
use checked_trees::{
    BorrowFacts, BorrowLoanLineage, CheckedDirectBorrowLoanResource,
    CheckedDirectBorrowParentLifetime, CheckedDirectBorrowRestorationObligation,
    CheckedParentBorrowResource, CheckedReborrowLoanResource, FlowFacts, FlowInvalidationSource,
    ParentLexicalStatusAtChildEnd,
};
use diagnostics::Diagnostic;

pub(crate) fn install_borrow_resources(
    borrow: &mut BorrowFacts,
    direct: Vec<CheckedDirectBorrowLoanResource>,
    reborrows: &[CheckedReborrowLoanResourceDraft],
    installation: &[ParentResourceIndex],
    dispositions: &[CheckedReborrowDispositionEventDraft],
    containments: &[CheckedReborrowContainmentCertificateDraft],
    restored_uses: &[CheckedReborrowRestoredCallUseCertificateDraft],
) {
    borrow.direct_loan_resources.reset_retain_capacity();
    borrow.reborrow_loan_resources.reset_retain_capacity();
    borrow.reborrow_disposition_events.reset_retain_capacity();
    borrow
        .reborrow_containment_certificates
        .reset_retain_capacity();
    borrow
        .reborrow_restored_call_use_certificates
        .reset_retain_capacity();

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
    let containment_handles = containments
        .iter()
        .map(|containment| {
            borrow
                .reborrow_containment_certificates
                .insert(containment.close(&handles))
        })
        .collect::<Vec<_>>();
    let disposition_handles = dispositions
        .iter()
        .map(|disposition| {
            let disposition = disposition.close(borrow, &handles);
            borrow.reborrow_disposition_events.insert(disposition)
        })
        .collect::<Vec<_>>();
    for restored_use in restored_uses {
        borrow
            .reborrow_restored_call_use_certificates
            .insert(restored_use.close(&handles, &disposition_handles, &containment_handles));
    }
}

pub(crate) fn reconstruct_direct_borrow_resources(
    program: &typed_trees::TypedTrees,
    borrow: &BorrowFacts,
    flow: &FlowFacts,
) -> Result<Vec<CheckedDirectBorrowLoanResource>, Vec<Diagnostic>> {
    let mut resources = Vec::new();
    let mut diagnostics = Vec::new();

    for (_, state) in borrow.states.iter() {
        let Some(flow_state) = flow.control.states.iter().find_map(|(_, candidate)| {
            (candidate.machine_symbol == state.machine_symbol
                && candidate.state_symbol == state.state_symbol)
                .then_some(candidate)
        }) else {
            diagnostics.push(Diagnostic::error(
                "checked direct-root borrow resource has no exact flow-state owner",
            ));
            continue;
        };

        for (loan_handle, loan) in borrow
            .loans
            .iter()
            .filter(|(handle, _)| borrow.state_owns_loan(state, *handle))
        {
            // Direct reborrows close in their own typed parent-resource arena;
            // every derived occurrence remains outside this root-only arena.
            if loan.lineage != BorrowLoanLineage::DirectRoot {
                continue;
            }

            // A direct-root loan formed on a reference-typed binding is still
            // a referent reborrow: the binding carries no parent loan to
            // rebase through (parameters and loan-less locals are the common
            // cases), so its declared access is the parent authority and the
            // access-pair rule decides the formation — `&write`/`&mut` on a
            // `&u8` binding can never derive write authority from a shared
            // referent. This is the same edge `Reborrow` loans face above the
            // resource arenas, applied at the declared-access boundary.
            if let Some(parent_access) = crate::checks::borrows::details::binding_reference_access(
                program,
                state.machine_symbol,
                state.state_symbol,
                loan.statement_index,
                loan.root_symbol,
            ) && parent_access.direct_reborrow_effect(&loan.kind).is_none()
            {
                diagnostics.push(invalid_reborrow_attenuation_diagnostic(
                    &parent_access,
                    &loan.kind,
                ));
                continue;
            }

            let activations = flow
                .borrow_lifetimes
                .activations
                .span_or_empty(flow_state.borrow_activations)
                .iter()
                .filter(|activation| activation.loan == loan_handle)
                .collect::<Vec<_>>();
            let weakenings = flow
                .borrow_lifetimes
                .weakenings
                .span_or_empty(flow_state.borrow_weakenings)
                .iter()
                .filter(|weakening| weakening.loan == loan_handle)
                .collect::<Vec<_>>();
            let ([activation], [weakening]) = (activations.as_slice(), weakenings.as_slice())
            else {
                diagnostics.push(Diagnostic::error(
                    "checked direct-root borrow resource requires exactly one activation and one weakening",
                ));
                continue;
            };
            if activation.source
                != (FlowInvalidationSource::Statement {
                    statement_index: loan.statement_index,
                })
            {
                diagnostics.push(Diagnostic::error(
                    "checked direct-root borrow activation drifted from loan formation",
                ));
                continue;
            }

            let parent_lifetime = CheckedDirectBorrowParentLifetime {
                machine_symbol: state.machine_symbol,
                state_symbol: state.state_symbol,
                root_symbol: loan.root_symbol,
            };
            let restoration = CheckedDirectBorrowRestorationObligation {
                parent: parent_lifetime.clone(),
                weakening_source: weakening.source,
                weakening_reason: weakening.reason,
            };
            resources.push(CheckedDirectBorrowLoanResource {
                loan: loan_handle,
                machine_symbol: state.machine_symbol,
                state_symbol: state.state_symbol,
                owner_symbol: loan.owner_symbol,
                owner_path: borrow.loan_owner_path(loan).to_vec(),
                captured_place: checked_trees::CapturedPlace {
                    root_symbol: loan.root_symbol,
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
    }

    if diagnostics.is_empty() {
        Ok(resources)
    } else {
        Err(diagnostics)
    }
}

pub(crate) fn reconstruct_reborrow_resource_drafts(
    borrow: &BorrowFacts,
    flow: &FlowFacts,
) -> Result<Vec<CheckedReborrowLoanResourceDraft>, Vec<Diagnostic>> {
    let mut resources = Vec::new();
    let mut diagnostics = Vec::new();

    for (_, state) in borrow.states.iter() {
        let Some(flow_state) = flow.control.states.iter().find_map(|(_, candidate)| {
            (candidate.machine_symbol == state.machine_symbol
                && candidate.state_symbol == state.state_symbol)
                .then_some(candidate)
        }) else {
            diagnostics.push(Diagnostic::error(
                "checked direct-reborrow resource has no exact flow-state owner",
            ));
            continue;
        };

        for (loan_handle, loan) in borrow
            .loans
            .iter()
            .filter(|(handle, _)| borrow.state_owns_loan(state, *handle))
        {
            let BorrowLoanLineage::Reborrow { parent_loan } = &loan.lineage else {
                continue;
            };
            let parent = borrow.loans.get(*parent_loan);
            let Some(access_effect) = parent.kind.direct_reborrow_effect(&loan.kind) else {
                diagnostics.push(invalid_reborrow_attenuation_diagnostic(
                    &parent.kind,
                    &loan.kind,
                ));
                continue;
            };
            let activations = flow
                .borrow_lifetimes
                .activations
                .span_or_empty(flow_state.borrow_activations)
                .iter()
                .enumerate()
                .filter(|(_, activation)| activation.loan == loan_handle)
                .filter_map(|(offset, activation)| {
                    span_handle(flow_state.borrow_activations, offset)
                        .map(|handle| (handle, activation))
                })
                .collect::<Vec<_>>();
            let weakenings = flow
                .borrow_lifetimes
                .weakenings
                .span_or_empty(flow_state.borrow_weakenings)
                .iter()
                .enumerate()
                .filter(|(_, weakening)| weakening.loan == loan_handle)
                .filter_map(|(offset, weakening)| {
                    span_handle(flow_state.borrow_weakenings, offset)
                        .map(|handle| (handle, weakening))
                })
                .collect::<Vec<_>>();
            if activations.len() != 1 || weakenings.len() != 1 {
                diagnostics.push(Diagnostic::error(
                    "checked direct-reborrow resource requires exactly one activation and one weakening",
                ));
                continue;
            }
            let (child_activation, activation) = activations[0];
            let (child_weakening, weakening) = weakenings[0];
            if activation.source
                != (FlowInvalidationSource::Statement {
                    statement_index: loan.statement_index,
                })
            {
                diagnostics.push(Diagnostic::error(
                    "checked direct-reborrow activation drifted from loan formation",
                ));
                continue;
            }
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
            let [(parent_weakening, parent_weakening_fact)] = parent_weakenings.as_slice() else {
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
                        == checked_trees::FlowConstraintKind::BorrowLoan { loan: *parent_loan }
                })
                .filter_map(|(offset, _)| span_handle(statement.entry_constraints, offset))
                .collect::<Vec<_>>();
            let [parent_entry_constraint] = parent_constraints.as_slice() else {
                diagnostics.push(Diagnostic::error(
                    "checked direct-reborrow suspension requires exactly one parent entry constraint",
                ));
                continue;
            };

            resources.push(CheckedReborrowLoanResourceDraft {
                loan: loan_handle,
                machine_symbol: state.machine_symbol,
                state_symbol: state.state_symbol,
                owner_symbol: loan.owner_symbol,
                owner_path: borrow.loan_owner_path(loan).to_vec(),
                captured_place: checked_trees::CapturedPlace {
                    root_symbol: loan.root_symbol,
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
    }

    if diagnostics.is_empty() {
        Ok(resources)
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

pub(crate) fn span_handle<T>(
    span: arena::HandleSpan<T>,
    offset: usize,
) -> Option<arena::Handle<T>> {
    let offset = u32::try_from(offset).ok()?;
    let arena_index = span.start().arena_index().checked_add(offset)?;
    Some(arena::Handle::from_parts(
        arena_index,
        span.start().generation(),
    ))
}

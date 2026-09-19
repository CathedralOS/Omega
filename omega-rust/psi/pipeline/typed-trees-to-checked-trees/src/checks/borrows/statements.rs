use checked_trees::{
    BorrowAccessKind, BorrowCompatibilityConclusion, BorrowCompatibilityDerivation,
    BorrowCompatibilityFormation, CheckFacts, CheckedBorrowCompatibilityCertificate,
    CheckedBorrowMutationCertificate, FlowStateFact,
};
use diagnostics::Diagnostic;

use crate::flow::{StateMutationSummaryCache, call_write_accesses, statement_mutated_place};
use crate::labels::symbol_name;
use crate::semantic_calls::find_state_in_machine;

use super::details::{active_loan_detail, canonical_place_label};
use super::overlap::{
    CompatibilityReplayDrift, StatedOrderingPremise,
    borrow_loan_compatibility_from_selector_snapshot,
    borrow_loan_compatibility_with_selector_snapshot, canonical_place_loan_compatibility,
    canonical_place_loan_compatibility_with_selector_snapshot,
    captured_place_loan_compatibility_from_selector_snapshot,
};

/// The replay evidence a retained certificate no longer reproduces.
fn compatibility_replay_diagnostic(drift: CompatibilityReplayDrift) -> Diagnostic {
    match drift {
        CompatibilityReplayDrift::SelectorSnapshot => Diagnostic::error(
            "checked borrow compatibility certificate selector snapshot drifted from its captured-place shape",
        ),
        CompatibilityReplayDrift::Premise => Diagnostic::error(
            "checked borrow compatibility certificate premise tokens drifted from their established scope evidence",
        ),
    }
}

/// The replay evidence a retained mutation certificate no longer reproduces.
fn mutation_replay_diagnostic(drift: CompatibilityReplayDrift) -> Diagnostic {
    match drift {
        CompatibilityReplayDrift::SelectorSnapshot => Diagnostic::error(
            "checked borrow mutation certificate selector snapshot drifted from its captured-place shape",
        ),
        CompatibilityReplayDrift::Premise => Diagnostic::error(
            "checked borrow mutation certificate premise tokens drifted from their established scope evidence",
        ),
    }
}

pub(super) fn check_statement_borrows(
    program: &typed_trees::TypedTrees,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    stated_premises: &[StatedOrderingPremise],
    diagnostics: &mut Vec<Diagnostic>,
    compatibility_certificates: &mut Vec<CheckedBorrowCompatibilityCertificate>,
    retained_compatibility_certificates: &[CheckedBorrowCompatibilityCertificate],
    retained_compatibility_certificates_consumed: &mut [bool],
    mutation_certificates: &mut Vec<CheckedBorrowMutationCertificate>,
    retained_mutation_certificates: &[CheckedBorrowMutationCertificate],
    retained_mutation_certificates_consumed: &mut [bool],
    state_mutation_summaries: &StateMutationSummaryCache,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) {
    let Some(state) =
        find_state_in_machine(program, state_flow.machine_symbol, state_flow.state_symbol)
    else {
        return;
    };
    if crate::lookup::machine_by_symbol(program, state_flow.machine_symbol).is_none() {
        return;
    }
    let Some(borrow_state) = facts.borrow.states.iter().find_map(|(_, state)| {
        (state.machine_symbol == state_flow.machine_symbol
            && state.state_symbol == state_flow.state_symbol)
            .then_some(state)
    }) else {
        return;
    };
    for statement in facts
        .flow
        .control
        .statements
        .span_or_empty(state_flow.statements)
    {
        let mut available_premises = stated_premises.to_vec();
        super::overlap::append_call_premises(
            program,
            facts,
            state_flow,
            statement.statement_index,
            call_frames,
            &mut available_premises,
        );
        let stated_premises = available_premises.as_slice();
        let Some(statement_node) = program
            .statement_table
            .statements(state.statement_nodes)
            .get(statement.statement_index)
        else {
            continue;
        };

        if let typed_trees::statement::StatementNode::RootBinding(binding) = statement_node {
            super::calls::check_exclusive_place_use(
                program,
                facts,
                state_flow,
                statement,
                binding.receiver,
                stated_premises,
                diagnostics,
            );
        }

        for (forming_loan_handle, loan) in facts.borrow.loans.iter().filter(|(handle, loan)| {
            facts.borrow.state_owns_loan(borrow_state, *handle)
                && loan.statement_index == statement.statement_index
        }) {
            for active_loan_handle in facts
                .flow
                .borrow_loan_constraints(statement.entry_constraints)
            {
                let active_loan = facts.borrow.loans.get(active_loan_handle);
                let retained =
                    retained_compatibility_certificates
                        .iter()
                        .enumerate()
                        .find(|certificate| {
                            let certificate = certificate.1;
                            certificate.formation.machine_symbol == state_flow.machine_symbol
                                && certificate.formation.state_symbol == state_flow.state_symbol
                                && certificate.formation.statement_index
                                    == statement.statement_index
                                && certificate.forming_loan == forming_loan_handle
                                && certificate.active_loan == active_loan_handle
                                && facts
                                    .borrow
                                    .compatibility_certificate_matches_resources(certificate)
                        });
                let (compatibility, selector_snapshot, premises, active_access) =
                    if let Some((retained_index, retained)) = retained {
                        let Some((forming_access, active_access)) = facts
                            .borrow
                            .compatibility_certificate_resource_accesses(retained)
                        else {
                            continue;
                        };
                        let compatibility = match borrow_loan_compatibility_from_selector_snapshot(
                            program,
                            facts,
                            loan,
                            forming_access,
                            active_loan,
                            active_access,
                            &retained.selector_snapshot,
                            stated_premises,
                            &retained.premises,
                        ) {
                            Ok(compatibility) => compatibility,
                            Err(drift) => {
                                diagnostics.push(compatibility_replay_diagnostic(drift));
                                continue;
                            }
                        };
                        retained_compatibility_certificates_consumed[retained_index] = true;
                        (
                            compatibility,
                            retained.selector_snapshot.clone(),
                            retained.premises.clone(),
                            active_access,
                        )
                    } else {
                        let evidence = borrow_loan_compatibility_with_selector_snapshot(
                            program,
                            facts,
                            loan,
                            active_loan,
                            stated_premises,
                        );
                        (
                            evidence.compatibility,
                            evidence.selector_snapshot,
                            evidence.premises,
                            &active_loan.kind,
                        )
                    };
                if compatibility.non_interfering
                    || carried_authority(
                        loan,
                        active_loan_handle,
                        active_loan,
                        active_access,
                        compatibility.containment,
                    )
                {
                    let certificate = CheckedBorrowCompatibilityCertificate {
                        formation: BorrowCompatibilityFormation {
                            machine_symbol: state_flow.machine_symbol,
                            state_symbol: state_flow.state_symbol,
                            statement_index: statement.statement_index,
                        },
                        forming_loan: forming_loan_handle,
                        active_loan: active_loan_handle,
                        forming_place: compatibility.left.clone(),
                        active_place: compatibility.right.clone(),
                        selector_snapshot,
                        derivation: if premises.is_empty() {
                            BorrowCompatibilityDerivation::Structural
                        } else {
                            BorrowCompatibilityDerivation::Premised
                        },
                        premises,
                        conclusion: BorrowCompatibilityConclusion {
                            disjoint: compatibility.disjoint,
                            containment: compatibility.containment,
                            non_interfering: compatibility.non_interfering,
                        },
                    };
                    debug_assert!(
                        facts
                            .borrow
                            .compatibility_certificate_matches_resources(&certificate),
                        "automatic borrow compatibility must retain exact state-owned resources"
                    );
                    if facts
                        .borrow
                        .compatibility_certificate_matches_resources(&certificate)
                    {
                        compatibility_certificates.push(certificate);
                    }
                    continue;
                }

                diagnostics.push(Diagnostic::error(format!(
                    "statement {} creates local borrow `{}` while local borrow `{}` is still active ({})",
                    statement.statement_index,
                    symbol_name(program, loan.owner_symbol),
                    symbol_name(program, active_loan.owner_symbol),
                    active_loan_detail(
                        state_flow,
                        facts,
                        active_loan_handle,
                        statement.statement_index,
                    )
                    .unwrap_or_else(|| format!("borrowed at statement {}", active_loan.statement_index)),
                )));
            }
        }

        let Some(mutated_place) = statement_mutated_place(
            program,
            state_flow.machine_symbol,
            state_flow.state_symbol,
            statement.statement_index,
            statement_node,
        ) else {
            continue;
        };

        for loan_handle in facts
            .flow
            .borrow_loan_constraints(statement.entry_constraints)
        {
            let loan = facts.borrow.loans.get(loan_handle);
            let retained =
                retained_mutation_certificates
                    .iter()
                    .enumerate()
                    .find(|(_, certificate)| {
                        certificate.formation.machine_symbol == state_flow.machine_symbol
                            && certificate.formation.state_symbol == state_flow.state_symbol
                            && certificate.formation.statement_index == statement.statement_index
                            && certificate.active_loan == loan_handle
                            && facts
                                .borrow
                                .mutation_certificate_matches_resources(certificate)
                    });
            let (compatibility, selector_snapshot, premises) =
                if let Some((retained_index, retained)) = retained {
                    let Some(active_access) =
                        facts.borrow.mutation_certificate_resource_access(retained)
                    else {
                        continue;
                    };
                    let compatibility =
                        match captured_place_loan_compatibility_from_selector_snapshot(
                            program,
                            &retained.mutated_place,
                            &BorrowAccessKind::Mutable,
                            loan,
                            active_access,
                            &facts.borrow,
                            &retained.selector_snapshot,
                            stated_premises,
                            &retained.premises,
                        ) {
                            Ok(compatibility) => compatibility,
                            Err(drift) => {
                                diagnostics.push(mutation_replay_diagnostic(drift));
                                continue;
                            }
                        };
                    retained_mutation_certificates_consumed[retained_index] = true;
                    (
                        compatibility,
                        retained.selector_snapshot.clone(),
                        retained.premises.clone(),
                    )
                } else {
                    let evidence = canonical_place_loan_compatibility_with_selector_snapshot(
                        program,
                        &mutated_place,
                        loan,
                        &facts.borrow,
                        stated_premises,
                    );
                    (
                        evidence.compatibility,
                        evidence.selector_snapshot,
                        evidence.premises,
                    )
                };
            if compatibility.non_interfering {
                // A mutation carries no provenance edge: the pair is admitted
                // only by the replayed non-interference verdict, and the
                // retained certificate may record nothing more.
                let certificate = CheckedBorrowMutationCertificate {
                    formation: BorrowCompatibilityFormation {
                        machine_symbol: state_flow.machine_symbol,
                        state_symbol: state_flow.state_symbol,
                        statement_index: statement.statement_index,
                    },
                    mutated_place: compatibility.left.clone(),
                    active_loan: loan_handle,
                    active_place: compatibility.right.clone(),
                    selector_snapshot,
                    derivation: if premises.is_empty() {
                        BorrowCompatibilityDerivation::Structural
                    } else {
                        BorrowCompatibilityDerivation::Premised
                    },
                    premises,
                    conclusion: BorrowCompatibilityConclusion {
                        disjoint: compatibility.disjoint,
                        containment: compatibility.containment,
                        non_interfering: compatibility.non_interfering,
                    },
                };
                debug_assert!(
                    facts
                        .borrow
                        .mutation_certificate_matches_resources(&certificate),
                    "automatic borrow mutation compatibility must retain the exact state-owned loan"
                );
                if facts
                    .borrow
                    .mutation_certificate_matches_resources(&certificate)
                {
                    mutation_certificates.push(certificate);
                }
                continue;
            }
            diagnostics.push(Diagnostic::error(format!(
                "statement {} mutates `{}` while local borrow `{}` is still active ({})",
                statement.statement_index,
                canonical_place_label(program, &mutated_place),
                symbol_name(program, loan.owner_symbol),
                active_loan_detail(state_flow, facts, loan_handle, statement.statement_index)
                    .unwrap_or_else(|| format!("borrowed at statement {}", loan.statement_index)),
            )));
        }
    }

    check_call_mutation_borrows(
        program,
        facts,
        state_flow,
        borrow_state,
        stated_premises,
        diagnostics,
        state_mutation_summaries,
        call_frames,
    );
}

/// Vec-views borrow rule (and, generally, owner-mutation-through-a-call vs a
/// live borrowed view).
///
/// A mutating call through an owner -- e.g. `Vec::push`/`Vec::index_mut` or any
/// `&mut self` boundary/state call that reallocates or writes the owner -- may
/// invalidate a borrowed slice/string view (`&[T]`/`&mut [T]`/`&string`) taken
/// from that owner. The owner-write rule already rejects this for *assignment*
/// statements; this extends the same conflict to *call* statements, whose
/// write accesses are computed by [`crate::flow::call_write_accesses`] (the receiver place
/// plus any mutable-argument places). A mutated place overlapping a loan that is
/// still live at the call point is rejected.
///
/// This reuses the existing loan-overlap engine, so it inherits the disjoint
/// subslice/element precision and stays conservative when a window is unknown.
/// A named same-machine jump is different from a returning call: its successor
/// writes happen after source-state scope exit. Replayed local-loan closure can
/// discharge that conflict when no argument carries a loan into the successor.
/// Guard/argument calls, carried references and reborrow lineages keep their
/// existing checks; a syntactic jump alone is not release evidence.
fn check_call_mutation_borrows(
    program: &typed_trees::TypedTrees,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    borrow_state: &checked_trees::StateBorrowFact,
    stated_premises: &[StatedOrderingPremise],
    diagnostics: &mut Vec<Diagnostic>,
    summary_cache: &StateMutationSummaryCache,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) {
    for borrow_call in facts.borrow.calls.span_or_empty(borrow_state.calls) {
        let mutated_places = call_write_accesses(
            program,
            state_flow.machine_symbol,
            state_flow.state_symbol,
            &facts.borrow,
            borrow_call,
            summary_cache,
        );
        if mutated_places.is_empty() {
            continue;
        }
        let mut available_premises = stated_premises.to_vec();
        super::overlap::append_call_premises(
            program,
            facts,
            state_flow,
            borrow_call.statement_index,
            call_frames,
            &mut available_premises,
        );
        let stated_premises = available_premises.as_slice();

        let exiting_source =
            source_exiting_without_carried_borrows(program, state_flow, borrow_call);

        // The loans live *at* the call are the entry constraints of the flow
        // statement the call belongs to.
        let Some(statement) = facts
            .flow
            .control
            .statements
            .span_or_empty(state_flow.statements)
            .iter()
            .find(|statement| statement.statement_index == borrow_call.statement_index)
        else {
            continue;
        };

        for loan_handle in facts
            .flow
            .borrow_loan_constraints(statement.entry_constraints)
        {
            let loan = facts.borrow.loans.get(loan_handle);
            if exiting_source.is_some_and(|state| {
                local_loan_ends_before_successor(program, facts, state_flow, state, loan_handle)
            }) {
                continue;
            }
            for mutated_place in &mutated_places {
                if canonical_place_loan_compatibility(
                    program,
                    mutated_place,
                    loan,
                    &facts.borrow,
                    stated_premises,
                )
                .non_interfering
                {
                    continue;
                }
                diagnostics.push(Diagnostic::error(format!(
                    "statement {} mutates `{}` while local borrow `{}` is still active ({})",
                    borrow_call.statement_index,
                    canonical_place_label(program, mutated_place),
                    symbol_name(program, loan.owner_symbol),
                    active_loan_detail(
                        state_flow,
                        facts,
                        loan_handle,
                        borrow_call.statement_index,
                    )
                    .unwrap_or_else(|| format!("borrowed at statement {}", loan.statement_index)),
                )));
                // One diagnostic per (call, loan) is enough.
                break;
            }
        }
    }
}

/// A named jump enters another state only after the caller's local scope ends.
/// Calls in its guard or arguments are ordinary pre-exit calls. Keep carried
/// loans out of this judgment: their successor use needs its own correspondence.
fn source_exiting_without_carried_borrows<'program>(
    program: &'program typed_trees::TypedTrees,
    flow: &FlowStateFact,
    call: &checked_trees::BorrowCallFact,
) -> Option<&'program typed_trees::state::State> {
    let source = find_state_in_machine(program, flow.machine_symbol, flow.state_symbol)?;
    let target = find_state_in_machine(program, flow.machine_symbol, call.target_symbol)?;
    let site = crate::semantic_calls::find_call_site(
        program,
        flow.machine_symbol,
        flow.state_symbol,
        call.statement_index,
        call.call_ordinal,
    )?;
    let crate::semantic_calls::CallSite::TransitionNamed { path, .. } = &site else {
        return None;
    };
    // A local state name has no explicit receiver; its head is the target
    // symbol, not a borrowed receiver alias.
    if path.members.count() != 1 || path.head_symbol != target.symbol {
        return None;
    }
    let machine = crate::lookup::machine_by_symbol(program, flow.machine_symbol)?;
    let arguments = crate::semantic_calls::call_site_argument_expressions(program, &site);
    let parameters = program.state_parameters(target);
    let mut positional = parameters.iter().filter(|parameter| !parameter.is_self);
    for argument in arguments {
        let parameter = positional.next()?;
        let actual =
            validation::expression_result_type_reference(program, machine, source, *argument)?;
        for reference in [parameter.type_reference, actual] {
            // Use the closed lifetime frontier, not the discovery-only owner
            // paths query: an unknown type is not evidence of absent loans.
            if !crate::borrow::view_link::substituted_result_is_view_free(program, reference, &[]) {
                return None;
            }
        }
    }
    positional.next().is_none().then_some(source)
}

/// Whether `forming_loan` is recorded as deriving its authority through this
/// exact active loan occurrence.
///
/// A retained reborrow names its unique parent loan handle; the edge is
/// replayed independently from the typed statement by lineage replay. An
/// unretained transfer (aggregate-carried or helper-derived) keeps only its
/// rebasing source owner local, so it may rejoin an active loan of that owner
/// only when the replayed containment verdict proves the forming place still
/// sits inside the active loan's captured place. Either way the active loan
/// must be exclusive: sharing a read parent does not suspend anything.
///
/// This judgment only reads already-recorded loan rows and the independently
/// replayed containment verdict. It creates no authority: the forming loan
/// was itself produced by rebasing through the named source, and the pair's
/// compatibility certificate still retains the honest spatial conclusion.
pub(super) fn carried_authority(
    forming_loan: &checked_trees::BorrowLoanFact,
    active_loan_handle: arena::Handle<checked_trees::BorrowLoanFact>,
    active_loan: &checked_trees::BorrowLoanFact,
    active_access: &checked_trees::BorrowAccessKind,
    containment: checked_trees::CapturedPlaceContainment,
) -> bool {
    if !active_access.is_exclusive()
        || !matches!(
            containment,
            checked_trees::CapturedPlaceContainment::Same
                | checked_trees::CapturedPlaceContainment::RightContainsLeft
        )
    {
        return false;
    }
    match forming_loan.lineage {
        checked_trees::BorrowLoanLineage::Reborrow { parent_loan } => {
            parent_loan == active_loan_handle
        }
        checked_trees::BorrowLoanLineage::UnretainedDerived => {
            forming_loan.source_owner_symbol == active_loan.owner_symbol
        }
        checked_trees::BorrowLoanLineage::DirectRoot => false,
    }
}

fn local_loan_ends_before_successor(
    program: &typed_trees::TypedTrees,
    facts: &CheckFacts,
    flow: &FlowStateFact,
    source: &typed_trees::state::State,
    loan_handle: arena::Handle<checked_trees::BorrowLoanFact>,
) -> bool {
    let loan = facts.borrow.loans.get(loan_handle);
    if loan.lineage != checked_trees::BorrowLoanLineage::DirectRoot
        || !program
            .statement_table
            .statements(source.statement_nodes)
            .iter()
            .any(|statement| {
                matches!(statement, typed_trees::statement::StatementNode::LocalData(local)
                if local.symbol == loan.owner_symbol && matches!(
                    program.type_reference_table.type_reference(local.type_reference),
                    typed_trees::types::TypeReferenceNode::Reference { .. }
                ))
            })
    {
        return false;
    }
    // Resource replay precedes this check. Match its exact state-exit closure,
    // not merely a syntactic reference local or a guessed last-use index.
    facts
        .borrow
        .direct_loan_resources
        .iter()
        .any(|(_, resource)| {
            resource.loan == loan_handle
                && resource.machine_symbol == flow.machine_symbol
                && resource.state_symbol == flow.state_symbol
                && resource.parent_lifetime.root_symbol == loan.root_symbol
                && resource.weakening_reason == checked_trees::FlowBorrowWeakeningReason::StateExit
                && resource.weakening_source
                    == checked_trees::FlowInvalidationSource::Statement {
                        statement_index: source.statement_nodes.count() as usize,
                    }
        })
}

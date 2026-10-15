//! Selected outcome-specific evidence installed on the caller's call sites.
//!
//! A caller arm that names one case's guarantee (`ensures Case -> { ... }`)
//! produces checked arm rows; this pass joins each selected row to the
//! emitted `CallStructural` operation through the call's recorded source
//! occurrence and the callee's already-canonical guarded rows, so guard,
//! position and obligation can never drift between the two sides. Rows a
//! bounded producer cannot yet name exactly are refused rather than guessed.

use crate::proofs::evidence_lowering::evidence_terms::{
    lower_evidence_contract_lanes, lower_evidence_interface,
};
use crate::proofs::evidence_lowering::outcome_specific_ensures::outcome_specific_ensure_rows;
use crate::proofs::evidence_lowering::proof_output_calls::{
    terminal_evidence_term_id, terminal_proposition_application_id,
};
use crate::proofs::{
    CheckedTrees, EvidenceTermDeclaration, EvidenceTermId, LoweredPsi, LoweringError,
    OperationResult, OutcomeSpecificCallEvidence, OutcomeSpecificCallEvidenceValidity, Proposition,
    PropositionApplicationIdentity, PropositionDeclaration,
};

/// The caller places one validity roster of checked occurrences names,
/// sorted and deduplicated as the codec's canonical order requires.
fn dependency_places(
    occurrences: &[checked_trees::OutcomeReferencedOccurrence],
    result_place: semantic_vocabulary::PlaceId,
    unsupported_root: &'static str,
) -> Result<Vec<semantic_vocabulary::PlaceId>, LoweringError> {
    let mut places = Vec::with_capacity(occurrences.len());
    for occurrence in occurrences {
        match occurrence.root {
            checked_trees::OutcomeOccurrenceRoot::CalleeResult => places.push(result_place),
            checked_trees::OutcomeOccurrenceRoot::Other => {
                return Err(LoweringError::Unsupported(unsupported_root));
            }
        }
    }
    places.sort();
    places.dedup();
    Ok(places)
}

pub(crate) fn lower_outcome_specific_call_evidence(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    lowered: &mut LoweredPsi,
    term_ids: &[Option<EvidenceTermId>],
    declarations: &[PropositionDeclaration],
    applications: &[PropositionApplicationIdentity],
    evidence_terms: &[EvidenceTermDeclaration],
) -> Result<(), LoweringError> {
    for (_, arm) in checked
        .facts
        .proof
        .outcome_specific_arms
        .iter()
        .filter(|(_, arm)| {
            arm.caller_machine_symbol == machine
                && arm.rows.iter().any(|row| row.selected_term.is_some())
        })
    {
        let Some(first_row) = arm.rows.iter().find(|row| row.selected_term.is_some()) else {
            continue;
        };
        let callee_symbol = checked
            .facts
            .proof
            .outcome_specific_guarantees
            .get(first_row.guarantee)
            .machine_symbol;
        let occurrence = lowered
            .source_call_occurrences
            .iter()
            .find(|occurrence| {
                occurrence.source_state == arm.caller_state_symbol
                    && occurrence.statement_index == arm.result_call_statement_index
            })
            .ok_or(LoweringError::Unsupported(
                "outcome-specific arm has no lowered call occurrence",
            ))?;
        let operation_id =
            occurrence
                .terminal_site
                .operation()
                .ok_or(LoweringError::Unsupported(
                    "outcome-specific arm names a completion edge, not a call operation",
                ))?;
        let (result_place, callee_id) = {
            let operation = lowered
                .semantic_module
                .machines
                .iter()
                .flat_map(|machine| machine.blocks.iter())
                .flat_map(|block| block.operations.iter())
                .find(|operation| operation.id == operation_id)
                .ok_or(LoweringError::Unsupported(
                    "outcome-specific arm operation is absent",
                ))?;
            let terminal_psi::call_structural_pair_pattern!(callee, ..) = &operation.kind else {
                return Err(LoweringError::Unsupported(
                    "outcome-specific arm operation is not a structural call",
                ));
            };
            let OperationResult::Structural(result) = &operation.result else {
                return Err(LoweringError::Unsupported(
                    "outcome-specific arm call has no structural result",
                ));
            };
            (result.place, *callee)
        };
        // A selection rides only a call whose callee is an exact payloadless
        // producer: scalar inputs decide the case but carry no custody, and
        // there are no structural inputs, entry claims, other contract rows
        // or crash routes. The Terminal contract replays the same callee
        // surface. A proposition that names an input needs the substitution
        // refused below; a closed proposition does not.
        let callee =
            lowered
                .semantic_module
                .machine(callee_id)
                .ok_or(LoweringError::Unsupported(
                    "outcome-specific arm callee is absent",
                ))?;
        if !callee.structural_parameters.is_empty()
            || !callee.entry_claims.is_empty()
            || !callee.content_entry_claims.is_empty()
            || !callee.contract.requires.is_empty()
            || !callee.contract.ensures.is_empty()
            || !callee.contract.crash_routes.is_empty()
            || !lower_evidence_contract_lanes(checked, callee_symbol, callee_id, term_ids)?
                .is_empty()
        {
            return Err(LoweringError::Unsupported(
                "outcome-specific selection from a callee with structural inputs or other contracts is unimplemented",
            ));
        }
        let callee_result_place = callee
            .result
            .structural()
            .ok_or(LoweringError::Unsupported(
                "outcome-specific arm callee has no structural result",
            ))?
            .place;
        let ensure_rows = outcome_specific_ensure_rows(
            identities,
            checked,
            callee_symbol,
            callee_id,
            &lowered.semantic_module,
            term_ids,
            evidence_terms,
        )?;

        let mut evidence_rows = Vec::new();
        for row in arm.rows.iter().filter(|row| row.selected_term.is_some()) {
            let guarantee = checked
                .facts
                .proof
                .outcome_specific_guarantees
                .get(row.guarantee);
            let output_field = guarantee
                .public_selector
                .clone()
                .ok_or(LoweringError::Unsupported("selected guarantee is unnamed"))?;
            let callee_term = terminal_evidence_term_id(
                term_ids,
                guarantee.evidence_term.ok_or(LoweringError::Unsupported(
                    "selected guarantee carries no evidence term",
                ))?,
                "selected guarantee term has no terminal identity",
            )?;
            let ensure = ensure_rows
                .iter()
                .find(|ensure| {
                    ensure.evidence.as_ref().is_some_and(|evidence| {
                        evidence.term == callee_term && evidence.output_field == output_field
                    })
                })
                .ok_or(LoweringError::Unsupported(
                    "selected guarantee is absent from the callee's guarded rows",
                ))?;
            let Proposition::Atom(callee_proposition) = ensure.proposition else {
                return Err(LoweringError::Unsupported(
                    "selected guarantee is not a nominal proposition",
                ));
            };
            let checked_term = checked.facts.proof.evidence_terms.get(
                guarantee
                    .evidence_term
                    .expect("selected evidence term checked"),
            );
            let instantiated =
                row.instantiated_proposition
                    .as_ref()
                    .ok_or(LoweringError::Unsupported(
                        "selected arm row carries no instantiated proposition",
                    ))?;
            let instantiated_proposition = terminal_proposition_application_id(
                identities,
                checked,
                term_ids,
                declarations,
                applications,
                instantiated,
            )?;
            // The caller's instantiated application may differ from the
            // callee's declared one in exactly one argument: the callee names
            // its reserved `result`, the caller names the call's own result.
            // That pair is the row's result substitution, between the
            // callee's result place and the call's result place; any other
            // difference (an input named by the proposition, another binder
            // or interface) has no Terminal lane yet.
            let result_substitution = if *instantiated == checked_term.proposition {
                None
            } else {
                let declared = &checked_term.proposition;
                let differing = (declared.arguments.len() == instantiated.arguments.len()
                    && declared.declaration == instantiated.declaration
                    && declared.binder_arguments == instantiated.binder_arguments
                    && declared.evidence_interface == instantiated.evidence_interface)
                    .then(|| {
                        declared
                            .arguments
                            .iter()
                            .zip(&instantiated.arguments)
                            .enumerate()
                            .filter(|(_, (declared, instantiated))| declared != instantiated)
                            .map(|(position, _)| position)
                            .collect::<Vec<_>>()
                    });
                match differing.as_deref() {
                    Some([position]) if declared.arguments[*position] == "result" => {
                        Some(terminal_psi::OutcomeSpecificCallResultSubstitution {
                            argument_position: u32::try_from(*position).map_err(|_| {
                                LoweringError::Unsupported(
                                    "outcome-specific result position exceeds u32",
                                )
                            })?,
                            callee_result: callee_result_place,
                            caller_result: result_place,
                        })
                    }
                    _ => {
                        return Err(LoweringError::Unsupported(
                            "outcome-specific result substitution is unimplemented",
                        ));
                    }
                }
            };
            // Validity roots are places of the caller: an occurrence of the
            // callee's `result` re-anchors to the call's result place. No
            // other occurrence has a caller place yet.
            let proposition_dependencies = dependency_places(
                &row.validity.referenced_occurrences,
                result_place,
                "outcome-specific proposition dependencies beyond the call result are unimplemented",
            )?;
            let interface_dependencies = row
                .validity
                .evidence_interface_scope
                .as_ref()
                .map(|scope| {
                    dependency_places(
                        &scope.retained_occurrences,
                        result_place,
                        "outcome-specific interface dependencies beyond the call result are unimplemented",
                    )
                })
                .transpose()?
                .unwrap_or_default();
            let evidence_interface = checked_term
                .evidence_interface
                .as_ref()
                .or(row
                    .validity
                    .evidence_interface_scope
                    .as_ref()
                    .map(|scope| &scope.interface))
                .ok_or(LoweringError::Unsupported(
                    "outcome-specific witness has no interface identity",
                ))
                .and_then(|interface| lower_evidence_interface(identities, checked, interface))?;
            evidence_rows.push(OutcomeSpecificCallEvidence {
                guard: ensure.guard,
                position: ensure.position,
                callee_obligation: ensure.obligation,
                callee_term,
                output_field,
                callee_proposition,
                instantiated_proposition,
                output: terminal_evidence_term_id(
                    term_ids,
                    row.selected_term.expect("selected row carries a term"),
                    "selected arm term has no terminal identity",
                )?,
                result_substitution,
                validity: OutcomeSpecificCallEvidenceValidity {
                    result: result_place,
                    proposition_dependencies,
                    evidence_interface,
                    interface_dependencies,
                },
                expected_use_count: 0,
                uses: Vec::new(),
            });
        }
        let operation = lowered
            .semantic_module
            .machines
            .iter_mut()
            .flat_map(|machine| machine.blocks.iter_mut())
            .flat_map(|block| block.operations.iter_mut())
            .find(|operation| operation.id == operation_id)
            .ok_or(LoweringError::Unsupported(
                "outcome-specific arm operation is absent",
            ))?;
        let terminal_psi::call_structural_pair_pattern!(selected_evidence, ..) =
            &mut operation.kind
        else {
            return Err(LoweringError::Unsupported(
                "outcome-specific arm operation is not a structural call",
            ));
        };
        *selected_evidence = evidence_rows;
    }
    Ok(())
}

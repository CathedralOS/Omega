//! Selected outcome-specific evidence installed on the caller's call sites.
//!
//! A caller arm that names one case's guarantee (`ensures Case -> { ... }`)
//! produces checked arm rows; this pass joins each selected row to the
//! emitted `CallStructural` operation through the call's recorded source
//! occurrence and the callee's already-canonical guarded rows, so guard,
//! position and obligation can never drift between the two sides. Rows a
//! bounded producer cannot yet name exactly are refused rather than guessed.
//!
//! A structural formal is not a scalar argument coordinate. Its checked field
//! chain resolves through the exact formal type, and the call operand retains
//! the evaluated actual root plus its own prefix. Owned unrestricted actuals
//! are value snapshots, so their propositions do not borrow the original home.
//! Borrowed inputs require a distinct lifetime intersection and remain refused.
//! This implements contracts.md (blob 0551b01c): matching result case and exact
//! argument/result substitution, preserving witness identity through erasure.

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
        match &occurrence.root {
            checked_trees::OutcomeOccurrenceRoot::CalleeResult => places.push(result_place),
            // An owned argument is an evaluated value snapshot, not a live source home.
            checked_trees::OutcomeOccurrenceRoot::CalleeParameter { .. }
            | checked_trees::OutcomeOccurrenceRoot::CalleeParameterProjection { .. }
            | checked_trees::OutcomeOccurrenceRoot::ClosedValue => {}
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
        // A statement can evaluate several calls, including a later operand
        // that mutates an earlier captured actual. The checked arm retains the
        // exact producing expression; statement order cannot select its call.
        let mut occurrences = lowered.source_call_occurrences.iter().filter(|occurrence| {
            occurrence.source_state == arm.caller_state_symbol
                && occurrence.statement_index == arm.result_call_statement_index
                && occurrence.source_site
                    == Some(checked_trees::NominalMachineUseSite::Expression(
                        arm.result_call_expression,
                    ))
        });
        let occurrence = occurrences.next().ok_or(LoweringError::Unsupported(
            "outcome-specific arm has no exact lowered call occurrence",
        ))?;
        if occurrences.next().is_some() {
            return Err(LoweringError::Unsupported(
                "outcome-specific arm call occurrence is not unique",
            ));
        }
        if occurrence.source_target != arm.result_call_target
            || checked
                .typed
                .machine_state(callee_symbol, arm.result_call_target)
                .is_none()
        {
            return Err(LoweringError::Unsupported(
                "outcome-specific arm call disagrees with its checked target owner",
            ));
        }
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
        // Selection annotates the ordinary call; it does not replace its
        // actual/formal custody. Unrestricted owned inputs carry snapshots.
        // Claims, borrowed inputs and additional contract/effect lanes retain
        // their explicit frontier until their exact evidence scopes are modeled.
        let callee =
            lowered
                .semantic_module
                .machine(callee_id)
                .ok_or(LoweringError::Unsupported(
                    "outcome-specific arm callee is absent",
                ))?;
        if callee.structural_parameters.iter().any(|parameter| {
            parameter.access != terminal_psi::StructuralAccess::Owned
                || parameter.multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted
                || !parameter.qualifications.is_empty()
                || !parameter.projected_qualifications.is_empty()
                || terminal_psi::structural_type_has_borrowed_storage_by(
                    |id| lowered.semantic_module.structural_type(id),
                    parameter.structural_type,
                )
        }) || !callee.entry_claims.is_empty()
            || !callee.content_entry_claims.is_empty()
            || !callee.contract.requires.is_empty()
            || !callee.contract.ensures.is_empty()
            || !callee.contract.crash_routes.is_empty()
            || !lower_evidence_contract_lanes(
                identities,
                checked,
                &lowered.semantic_module,
                callee_symbol,
                callee_id,
                term_ids,
            )?
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
            // Only retained checked roots authorize substitution. Structural
            // projections keep their resolved field chain; display labels and
            // equal endpoint values cannot select another actual or namespace.
            let declared = &checked_term.proposition;
            if declared.arguments.len() != instantiated.arguments.len()
                || declared.declaration != instantiated.declaration
                || declared.binder_arguments != instantiated.binder_arguments
                || declared.evidence_interface != instantiated.evidence_interface
            {
                return Err(LoweringError::Unsupported(
                    "outcome-specific substitution outside the proposition's arguments is unimplemented",
                ));
            }
            let argument_substitutions = ensure
                .evidence
                .as_ref()
                .expect("selected guarantee has evidence")
                .argument_subjects
                .clone();
            let mut result_substitution = None;
            for (position, (declared, instantiated)) in declared
                .arguments
                .iter()
                .zip(&instantiated.arguments)
                .enumerate()
            {
                match row.argument_roots.get(position) {
                    Some(checked_trees::OutcomeOccurrenceRoot::CalleeResult)
                        if result_substitution.is_none() =>
                    {
                        result_substitution =
                            Some(terminal_psi::OutcomeSpecificCallResultSubstitution {
                                argument_position: u32::try_from(position).map_err(|_| {
                                    LoweringError::Unsupported(
                                        "outcome-specific argument position exceeds u32",
                                    )
                                })?,
                                callee_result: callee_result_place,
                                caller_result: result_place,
                            });
                    }
                    Some(
                        checked_trees::OutcomeOccurrenceRoot::CalleeParameter { .. }
                        | checked_trees::OutcomeOccurrenceRoot::CalleeParameterProjection { .. },
                    ) => {}
                    Some(checked_trees::OutcomeOccurrenceRoot::ClosedValue)
                        if declared == instantiated => {}
                    _ => {
                        return Err(LoweringError::Unsupported(
                            "outcome-specific substitution of a computed proposition argument is unimplemented",
                        ));
                    }
                }
            }
            if row
                .validity
                .evidence_interface_scope
                .as_ref()
                .is_some_and(|scope| !scope.reference_regions.is_empty())
            {
                return Err(LoweringError::Unsupported(
                    "outcome-specific evidence interface reference scopes are unimplemented",
                ));
            }
            // The result remains an occurrence dependency; owned actuals are
            // snapshots. No borrowed structural actual passes the fence above.
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
                argument_substitutions,
                result_substitution,
                validity: OutcomeSpecificCallEvidenceValidity {
                    result: result_place,
                    proposition_dependencies,
                    evidence_interface,
                    interface_dependencies,
                },
                expected_use_count: u32::try_from(
                    checked
                        .facts
                        .proof
                        .proof_output_calls
                        .iter()
                        .filter(|(_, invocation)| {
                            invocation.caller_machine_symbol == arm.caller_machine_symbol
                                && invocation.runtime_call.is_some()
                        })
                        .map(|(_, invocation)| {
                            invocation
                                .evidence_arguments
                                .iter()
                                .filter(|argument| Some(argument.source) == row.selected_term)
                                .count()
                        })
                        .sum::<usize>(),
                )
                .map_err(|_| {
                    LoweringError::Unsupported("selected evidence use cardinality exceeds u32")
                })?,
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

/// One declaration-owned typed subject roster. Names are diagnostic payload;
/// checked formal positions and resolved field declarations supply identity.
/// Calls later join this exact roster with their ordinary evaluated operands.
pub(super) fn lower_argument_subjects(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    callee: &terminal_psi::TerminalMachine,
    module: &terminal_psi::TerminalModule,
    roots: &[checked_trees::OutcomeOccurrenceRoot],
) -> Result<Vec<terminal_psi::OutcomeSpecificCallArgumentSubstitution>, LoweringError> {
    let mut subjects = Vec::new();
    for (position, root) in roots.iter().enumerate() {
        match lower_argument_subject(identities, checked, machine, callee, module, position, root)?
        {
            terminal_psi::EvidenceArgumentSubject::Parameter(subject) => subjects.push(subject),
            terminal_psi::EvidenceArgumentSubject::Closed { .. }
            | terminal_psi::EvidenceArgumentSubject::Result { .. } => {}
            terminal_psi::EvidenceArgumentSubject::Unknown { .. } => {
                return Err(LoweringError::Unsupported(
                    "outcome-specific substitution of a computed proposition argument is unimplemented",
                ));
            }
        }
    }
    Ok(subjects)
}

pub(super) fn lower_argument_subject(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    callee: &terminal_psi::TerminalMachine,
    module: &terminal_psi::TerminalModule,
    position: usize,
    root: &checked_trees::OutcomeOccurrenceRoot,
) -> Result<terminal_psi::EvidenceArgumentSubject, LoweringError> {
    use terminal_psi::EvidenceArgumentSubject as Subject;
    let argument_position = u32::try_from(position)
        .map_err(|_| LoweringError::Unsupported("evidence argument position exceeds u32"))?;
    match root {
        checked_trees::OutcomeOccurrenceRoot::CalleeResult => {
            return Ok(Subject::Result { argument_position });
        }
        checked_trees::OutcomeOccurrenceRoot::ClosedValue => {
            return Ok(Subject::Closed { argument_position });
        }
        checked_trees::OutcomeOccurrenceRoot::Other => {
            return Ok(Subject::Unknown { argument_position });
        }
        _ => {}
    }
    let Some(entry) = checked
        .facts
        .flow
        .terminal_unit_effects
        .callable_entry_for_machine(machine)
    else {
        return Err(LoweringError::Unsupported(
            "outcome-specific subject has no published callable entry",
        ));
    };
    let (parameter_position, structural_path) =
        match root {
            checked_trees::OutcomeOccurrenceRoot::CalleeParameter { source_position } => {
                let Some(position) = entry
                    .scalar_parameters
                    .iter()
                    .position(|parameter| parameter.source_position == *source_position)
                else {
                    return Ok(Subject::Unknown { argument_position });
                };
                (position, None)
            }
            checked_trees::OutcomeOccurrenceRoot::CalleeParameterProjection {
                source_position,
                fields,
            } => {
                let position = entry
                    .structural_parameters
                    .iter()
                    .position(|parameter| parameter.position == *source_position)
                    .ok_or(LoweringError::Unsupported(
                        "outcome-specific projection names no structural parameter of its callee",
                    ))?;
                let parameter = callee.structural_parameters.get(position).ok_or(
                    LoweringError::Unsupported(
                        "outcome-specific structural parameter position is absent",
                    ),
                )?;
                let path = fields
                    .iter()
                    .map(|field| {
                        crate::scalar_lowering::computations::fields::field_path_identity(
                            checked, *field,
                        )
                        .map(|identity| {
                            terminal_psi::StructuralPathSegment::Field(
                                identities.intern(&identity.spelling()),
                            )
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                if terminal_semantics::primitive_read_projection_type(
                    module.structural_types.iter(),
                    parameter.structural_type,
                    &path,
                )
                .is_none()
                {
                    return Ok(Subject::Unknown { argument_position });
                }
                (position, Some(path))
            }
            checked_trees::OutcomeOccurrenceRoot::CalleeResult
            | checked_trees::OutcomeOccurrenceRoot::ClosedValue
            | checked_trees::OutcomeOccurrenceRoot::Other => {
                unreachable!("non-parameter subjects returned above")
            }
        };
    Ok(Subject::Parameter(
        terminal_psi::OutcomeSpecificCallArgumentSubstitution {
            argument_position,
            parameter_position: u32::try_from(parameter_position).map_err(|_| {
                LoweringError::Unsupported("outcome-specific parameter position exceeds u32")
            })?,
            structural_path,
        },
    ))
}

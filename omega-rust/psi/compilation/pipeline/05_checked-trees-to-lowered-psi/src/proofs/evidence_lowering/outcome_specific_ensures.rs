//! Outcome-specific ensures of the selected entry machine.

use crate::proofs::evidence_lowering::proof_output_calls::terminal_evidence_term_id;
use crate::proofs::{
    BTreeMap, CheckedPropositionBinderArgumentKind, CheckedTrees, EvidenceTermDeclaration,
    EvidenceTermId, LoweringError, MachineId, OutcomeSpecificEnsure, OutcomeSpecificEvidence,
    OutcomeSpecificGuard, Proposition, StructuralTypeShape, TerminalModule, dense_identity,
    obligation_id, unsupported,
};

pub(crate) fn outcome_specific_ensure_rows(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    selected_machine: symbols::SymbolHandle,
    terminal_machine_id: MachineId,
    module: &TerminalModule,
    term_ids: &[Option<EvidenceTermId>],
    evidence_terms: &[EvidenceTermDeclaration],
) -> Result<Vec<OutcomeSpecificEnsure>, LoweringError> {
    let guarantees = checked
        .facts
        .proof
        .outcome_specific_guarantees
        .iter()
        .filter_map(|(_, guarantee)| {
            (guarantee.machine_symbol == selected_machine).then_some(guarantee)
        })
        .collect::<Vec<_>>();
    if guarantees.is_empty() {
        return Ok(Vec::new());
    }
    // The guarded rows name cases of the result the machine's entry state
    // declares. Which case the lowered body actually returns is not decided
    // here: the verifier replays each row against the machine's exact
    // payloadless case exits.
    let state = checked
        .typed
        .machine(selected_machine)
        .and_then(|machine| checked.typed.machine_states(machine).first())
        .ok_or(LoweringError::Unsupported(
            "guarded payloadless producer state is absent",
        ))?;
    let checked_trees::types::TypeReferenceNode::Named {
        symbol: result_data,
        ..
    } = checked
        .typed
        .type_reference_table
        .type_reference(state.return_type)
    else {
        return unsupported("guarded payloadless producer result is not nominal");
    };
    let data = checked
        .typed
        .data_definition_by_symbol(*result_data)
        .ok_or(LoweringError::Unsupported(
            "guarded payloadless producer result data is absent",
        ))?;
    let terminal_machine =
        module
            .machine(terminal_machine_id)
            .ok_or(LoweringError::Unsupported(
                "guarded payloadless terminal machine is absent",
            ))?;
    let terminal_result =
        terminal_machine
            .result
            .structural()
            .ok_or(LoweringError::Unsupported(
                "guarded payloadless terminal result is not structural",
            ))?;
    let declaration = module
        .structural_type(terminal_result.structural_type)
        .ok_or(LoweringError::Unsupported(
            "guarded payloadless terminal result type is absent",
        ))?;
    let StructuralTypeShape::Sum { cases } = &declaration.shape else {
        return unsupported("guarded payloadless terminal result is not a sum");
    };

    let mut next_positions = BTreeMap::<OutcomeSpecificGuard, u32>::new();
    let mut rows = Vec::with_capacity(guarantees.len());
    for guarantee in guarantees {
        if guarantee.result_data != *result_data {
            return unsupported("guarded guarantee references a foreign result sum");
        }
        let case_identity = checked
            .typed
            .data_members(data)
            .iter()
            .find_map(|member| {
                let checked_trees::data::DataMember::Variant(variant) = member else {
                    return None;
                };
                (variant.symbol == guarantee.result_case).then(|| variant.member_identity())
            })
            .ok_or(LoweringError::Unsupported(
                "guarded guarantee references an unknown result case",
            ))?;
        let result_case = cases
            .iter()
            .find_map(|case| {
                (case.identity == identities.intern(&case_identity.spelling())).then_some(case.id)
            })
            .ok_or(LoweringError::Unsupported(
                "guarded guarantee case is absent from the terminal result sum",
            ))?;
        let guard = OutcomeSpecificGuard {
            result_type: terminal_result.structural_type,
            result_case,
        };
        let position = *next_positions.entry(guard).or_default();
        *next_positions
            .get_mut(&guard)
            .expect("guarded position was inserted") = position.checked_add(1).ok_or(
            LoweringError::Unsupported("guarded guarantee position exceeds u32"),
        )?;
        let (proposition, evidence) =
            match (guarantee.public_selector.as_ref(), guarantee.evidence_term) {
                (Some(selector), Some(term_handle)) => {
                    let checked_term = checked.facts.proof.evidence_terms.get(term_handle);
                    let checked_trees::domain::ProofFact::Proposition(application) =
                        checked.typed.proof_facts.get(guarantee.fact)
                    else {
                        return unsupported("named guarded guarantee is not nominal");
                    };
                    let normalized = checked
                        .typed
                        .normalize_nominal_proposition_application(application, None)
                        .ok_or(LoweringError::Unsupported(
                            "named guarded guarantee has no normalized proposition endpoint",
                        ))?;
                    if normalized.declaration != checked_term.proposition.declaration
                        || normalized.arguments != checked_term.proposition.arguments
                        || normalized.binder_arguments.len()
                            != checked_term.proposition.binder_arguments.len()
                        || normalized
                            .binder_arguments
                            .iter()
                            .zip(&checked_term.proposition.binder_arguments)
                            .any(|(left, right)| {
                                let kind = match left.kind {
                            checked_trees::proposition::PropositionBinderArgumentKind::Type => {
                                CheckedPropositionBinderArgumentKind::Type
                            }
                            checked_trees::proposition::PropositionBinderArgumentKind::Const => {
                                CheckedPropositionBinderArgumentKind::Const
                            }
                            checked_trees::proposition::PropositionBinderArgumentKind::Machine => {
                                CheckedPropositionBinderArgumentKind::Machine
                            }
                        };
                                kind != right.kind
                                    || left.identity != right.diagnostic_label
                                    || right.evidence_projection.is_some()
                            })
                    {
                        return unsupported(
                            "named guarded guarantee disagrees with its evidence term",
                        );
                    }
                    let term = terminal_evidence_term_id(
                        term_ids,
                        term_handle,
                        "guarded guarantee term has no terminal identity",
                    )?;
                    let declaration = evidence_terms
                        .iter()
                        .find(|declaration| declaration.id == term)
                        .ok_or(LoweringError::Unsupported(
                            "guarded guarantee term declaration is absent",
                        ))?;
                    (
                        Proposition::Atom(declaration.proposition),
                        Some(OutcomeSpecificEvidence {
                            term,
                            output_field: selector.clone(),
                            argument_subjects:
                                super::outcome_specific_calls::lower_argument_subjects(
                                    identities,
                                    checked,
                                    selected_machine,
                                    terminal_machine,
                                    module,
                                    &guarantee.argument_roots,
                                )?,
                        }),
                    )
                }
                (None, None) => {
                    if !guarantee.fact_is_truth {
                        return unsupported(
                            "unnamed guarded guarantee is outside the bounded truth proposition",
                        );
                    }
                    (Proposition::Truth, None)
                }
                _ => return unsupported("guarded guarantee has an incomplete evidence endpoint"),
            };
        rows.push(OutcomeSpecificEnsure {
            guard,
            position,
            obligation: obligation_id(1),
            proposition,
            evidence,
        });
    }
    rows.sort_by_key(|row| (row.guard, row.position));
    for (index, row) in rows.iter_mut().enumerate() {
        row.obligation = obligation_id(dense_identity(index)?);
    }
    Ok(rows)
}

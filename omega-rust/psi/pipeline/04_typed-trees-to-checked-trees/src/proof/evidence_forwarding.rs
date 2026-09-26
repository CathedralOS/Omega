//! Forwarded and projected evidence facts with definite-assignment
//! validation.

use crate::proof::outcome_arms::{
    exact_result_case, outcome_specific_assignment_matches_result, outcome_specific_fact_is_proved,
};
use crate::proof::proof_output_calls::{
    intake_call_ensures_propositions, intake_checked_proof_output_propositions,
};
use crate::proof::proposition_vocabulary::{
    contract_proposition_labels, lower_checked_proposition_application,
};
use checked_trees::{
    CheckedEvidenceTerm, ContractProofFactKind, ContractProofFactOwner, ProofFacts,
};
use symbols::SymbolHandle;

pub(crate) fn bind_evidence_forwarding_facts(
    program: &typed_trees::TypedTrees,
    proof: &mut ProofFacts,
) -> Result<(), Vec<diagnostics::Diagnostic>> {
    let mut diagnostics = Vec::new();
    let mut forwardings = arena::Arena::default();

    for forwarding in &program.evidence_forwardings {
        let output = evidence_term_named(
            &proof.evidence_terms,
            forwarding.machine_symbol,
            forwarding.target.as_str(),
            ContractProofFactKind::Ensures,
        );
        let source = evidence_term_named(
            &proof.evidence_terms,
            forwarding.machine_symbol,
            forwarding.source.as_str(),
            ContractProofFactKind::Requires,
        )
        .or_else(|| {
            proof
                .proof_output_calls
                .iter()
                .flat_map(|(_, invocation)| {
                    (invocation.caller_machine_symbol == forwarding.machine_symbol
                        && invocation.caller_state_symbol == forwarding.state_symbol
                        && invocation.source_statement_index < forwarding.source_statement_index)
                        .then_some(invocation.outputs.iter().filter_map(|output| output.output))
                        .into_iter()
                        .flatten()
                })
                .find(|term| proof.evidence_terms.get(*term).name == forwarding.source.as_str())
        });
        let Some(output) = output else {
            diagnostics.push(diagnostics::Diagnostic::error(format!(
                "evidence forwarding target `{}` is not a named ensures binding of this machine",
                forwarding.target
            )));
            continue;
        };
        let output_term = proof.evidence_terms.get(output);
        let guarded_output = proof
            .outcome_specific_guarantees
            .iter()
            .any(|(_, row)| row.evidence_term == Some(output));
        let source = if let Some(source) = source {
            let source_term = proof.evidence_terms.get(source);
            if output_term.evidence_interface != source_term.evidence_interface
                || (!guarded_output && output_term.proposition != source_term.proposition)
            {
                diagnostics.push(diagnostics::Diagnostic::error(format!(
                    "cannot forward evidence term `{}` into `{}` because their proposition identities differ",
                    forwarding.source, forwarding.target
                )));
                continue;
            }
            checked_trees::EvidenceAssignmentSource::Forwarded { term: source }
        } else if let Some(conformance_symbol) = forwarding.source_conformance {
            let Some(source) = checked_evidence_producer(
                program,
                output_term,
                conformance_symbol,
                forwarding.source.as_str(),
            ) else {
                diagnostics.push(diagnostics::Diagnostic::error(format!(
                    "subjectless conformance `{}` does not provide the exact `{}` evidence interface required by `{}`",
                    forwarding.source, output_term.evidence_type, forwarding.target
                )));
                continue;
            };
            source
        } else {
            diagnostics.push(diagnostics::Diagnostic::error(format!(
                "evidence forwarding source `{}` is not a named requires binding of this machine, an explicit subjectless conformance, or an available proof-output term",
                forwarding.source
            )));
            continue;
        };
        forwardings.append(checked_trees::EvidenceForwardingFact {
            machine_symbol: forwarding.machine_symbol,
            state_symbol: forwarding.state_symbol,
            statement_index: forwarding.statement_index,
            output,
            source,
        });
    }

    if diagnostics.is_empty() {
        proof.evidence_forwardings = forwardings;
        validate_evidence_forwarding_definite_assignment(program, proof)
    } else {
        Err(diagnostics)
    }
}

pub(crate) fn bind_evidence_projection_facts(
    program: &typed_trees::TypedTrees,
    proof: &mut ProofFacts,
) -> Result<(), Vec<diagnostics::Diagnostic>> {
    use checked_trees::CheckedEvidenceProjection;
    use typed_trees::domain::ProofFact;

    let mut diagnostics = Vec::new();
    let mut applications = Vec::new();
    for (fact_handle, fact) in program.proof_facts.iter() {
        let ProofFact::Proposition(application) = fact else {
            continue;
        };
        let Some(normalized) = program.normalize_nominal_proposition_application(application, None)
        else {
            continue;
        };
        let projections = application
            .binder_arguments
            .iter()
            .filter(|argument| argument.evidence_projection.is_some())
            .count();
        if projections == 0 {
            applications.push(lower_checked_proposition_application(normalized));
            continue;
        }
        let owners = proof
            .contract_facts
            .iter()
            .filter_map(|(_, contract)| (contract.fact == fact_handle).then_some(contract.owner))
            .collect::<Vec<_>>();
        let [owner] = owners.as_slice() else {
            diagnostics.push(diagnostics::Diagnostic::error(format!(
                "carrierless evidence projection in proposition `{}` has no unique named-contract scope",
                application.name
            )));
            continue;
        };
        let mut checked = lower_checked_proposition_application(normalized);
        let mut bound_projections = Vec::new();
        for typed_argument in application.binder_arguments.iter() {
            let Some(projection) = &typed_argument.evidence_projection else {
                continue;
            };
            let Some(term) =
                evidence_term_in_scope(&proof.evidence_terms, *owner, projection.term.as_str())
            else {
                diagnostics.push(diagnostics::Diagnostic::error(format!(
                    "carrierless evidence projection `{}.{}` names no retained evidence term in this contract scope",
                    projection.term, projection.member
                )));
                continue;
            };
            let term_definition = proof.evidence_terms.get(term);
            let Some(interface) = &term_definition.evidence_interface else {
                diagnostics.push(diagnostics::Diagnostic::error(format!(
                    "carrierless evidence projection `{}.{}` uses an unresolved evidence interface",
                    projection.term, projection.member
                )));
                continue;
            };
            let matching_rows = interface
                .requirements
                .iter()
                .filter(|row| program.symbols.name(row.requirement) == projection.member.as_str())
                .collect::<Vec<_>>();
            let [row] = matching_rows.as_slice() else {
                let reason = if matching_rows.is_empty() {
                    "does not contain"
                } else {
                    "contains more than one requirement named"
                };
                diagnostics.push(diagnostics::Diagnostic::error(format!(
                    "carrierless evidence interface `{}` {reason} `{}` for projection `{}.{}`",
                    term_definition.evidence_type,
                    projection.member,
                    projection.term,
                    projection.member
                )));
                continue;
            };
            bound_projections.push((
                format!("{}.{}", projection.term, projection.member),
                CheckedEvidenceProjection {
                    term,
                    declaring_trait: row.declaring_trait,
                    declaring_trait_arguments: row.declaring_trait_arguments.clone(),
                    requirement: row.requirement,
                },
            ));
        }
        for (label, projection) in bound_projections {
            let mut matched = false;
            for checked_argument in &mut checked.binder_arguments {
                if checked_argument.identity == label {
                    checked_argument.identity.clear();
                    checked_argument.evidence_projection = Some(projection.clone());
                    matched = true;
                }
            }
            if !matched {
                diagnostics.push(diagnostics::Diagnostic::error(format!(
                    "carrierless evidence projection `{label}` is not retained by the normalized proposition endpoint"
                )));
            }
        }
        applications.push(checked);
    }
    if diagnostics.is_empty() {
        proof.proposition_vocabulary.applications = applications;
        Ok(())
    } else {
        Err(diagnostics)
    }
}

fn evidence_term_in_scope(
    terms: &arena::Arena<CheckedEvidenceTerm>,
    owner: ContractProofFactOwner,
    name: &str,
) -> Option<arena::Handle<CheckedEvidenceTerm>> {
    let local = terms.iter().find_map(|(handle, term)| {
        (term.owner == owner && term.kind == ContractProofFactKind::Requires && term.name == name)
            .then_some(handle)
    });
    if local.is_some() {
        return local;
    }
    let ContractProofFactOwner::MachineState { machine_symbol, .. } = owner else {
        return None;
    };
    terms.iter().find_map(|(handle, term)| {
        (term.owner == ContractProofFactOwner::Machine { machine_symbol }
            && term.kind == ContractProofFactKind::Requires
            && term.name == name)
            .then_some(handle)
    })
}

fn checked_evidence_producer(
    program: &typed_trees::TypedTrees,
    output: &CheckedEvidenceTerm,
    conformance_symbol: SymbolHandle,
    source_name: &str,
) -> Option<checked_trees::EvidenceAssignmentSource> {
    use typed_trees::trait_definition::{
        ConformanceImplementation, ConformanceRowSource, ConformanceSubject,
    };

    let expected_interface = output.evidence_interface.as_ref()?;
    let conformance = program
        .conformances()
        .iter()
        .find(|candidate| candidate.symbol == conformance_symbol)?;
    if !matches!(conformance.subject, ConformanceSubject::Subjectless)
        || conformance.alias.as_ref()?.as_str() != source_name
    {
        return None;
    }
    let evidence_trait = program
        .traits()
        .iter()
        .find(|candidate| candidate.name == conformance.trait_name)?;
    let arguments = program
        .type_reference_table
        .type_reference_handles(conformance.arguments)
        .iter()
        .map(|argument| program.normalized_type_identity(*argument).into_string())
        .collect::<Vec<_>>();
    if evidence_trait.symbol != expected_interface.trait_symbol
        || arguments != expected_interface.arguments
    {
        return None;
    }
    let ConformanceImplementation::Closed { rows } = &conformance.implementation else {
        unreachable!("selected evidence producers are closed")
    };
    let rows = rows
        .iter()
        .map(|row| {
            let (requirement_identity, realization_identity) =
                crate::facts::normalized_dynamic_row_identities(program, row).ok()?;
            Some(checked_trees::DynamicConformanceRowFact {
                declaring_trait: row.declaring_trait,
                requirement: row.requirement,
                requirement_identity,
                realization_machine: row.realization_machine,
                realization_state: row.realization_state,
                realization_identity,
                source: match row.source {
                    ConformanceRowSource::Inline => {
                        checked_trees::DynamicConformanceRowSource::Inline
                    }
                    ConformanceRowSource::Reference => {
                        checked_trees::DynamicConformanceRowSource::Reference
                    }
                    ConformanceRowSource::TraitDefault => {
                        checked_trees::DynamicConformanceRowSource::TraitDefault
                    }
                },
            })
        })
        .collect::<Option<Vec<_>>>()?;

    Some(
        checked_trees::EvidenceAssignmentSource::ProducerConformance {
            conformance: conformance.symbol,
            evidence_trait: evidence_trait.symbol,
            rows,
        },
    )
}

fn validate_evidence_forwarding_definite_assignment(
    program: &typed_trees::TypedTrees,
    proof: &ProofFacts,
) -> Result<(), Vec<diagnostics::Diagnostic>> {
    use std::collections::{BTreeMap, BTreeSet, VecDeque};
    use typed_trees::statement::{StatementNode, TransitionExit, TransitionTargetNode};

    let mut diagnostic_messages = BTreeSet::new();

    for machine in program.machines() {
        let guarded_output_handles = proof
            .outcome_specific_guarantees
            .iter()
            .filter_map(|(_, row)| {
                (row.machine_symbol == machine.symbol)
                    .then_some(row.evidence_term)
                    .flatten()
            })
            .map(|handle| handle.arena_index())
            .collect::<BTreeSet<_>>();
        let unconditional_outputs = proof
            .evidence_terms
            .iter()
            .filter_map(|(handle, term)| {
                (term.owner
                    == (ContractProofFactOwner::Machine {
                        machine_symbol: machine.symbol,
                    })
                    && term.kind == ContractProofFactKind::Ensures)
                    .then_some(handle)
                    .filter(|handle| !guarded_output_handles.contains(&handle.arena_index()))
            })
            .collect::<Vec<_>>();
        let has_guarded_rows = proof
            .outcome_specific_guarantees
            .iter()
            .any(|(_, row)| row.machine_symbol == machine.symbol);
        if unconditional_outputs.is_empty() && !has_guarded_rows {
            continue;
        }
        let states = program.machine_states(machine);
        let Some(entry) = states.first() else {
            continue;
        };
        let initial_known = contract_proposition_labels(
            program,
            program.machine_contracts(machine),
            typed_trees::signature::SignatureContractKind::Requires,
            &[],
        );
        let mut work = VecDeque::from([(
            entry.symbol,
            BTreeMap::<u32, Option<arena::Handle<CheckedEvidenceTerm>>>::new(),
            initial_known,
        )]);
        let mut seen = BTreeSet::new();

        while let Some((state_symbol, mut assigned, mut known)) = work.pop_front() {
            let key = (
                state_symbol.arena_index(),
                assigned
                    .iter()
                    .map(|(output, source)| (*output, source.map(|source| source.arena_index())))
                    .collect::<Vec<_>>(),
                known.iter().cloned().collect::<Vec<_>>(),
            );
            if !seen.insert(key) {
                continue;
            }
            let Some(state) = states.iter().find(|state| state.symbol == state_symbol) else {
                continue;
            };
            known.extend(contract_proposition_labels(
                program,
                program.state_contracts(state),
                typed_trees::signature::SignatureContractKind::Requires,
                &[],
            ));

            let assignments = proof
                .evidence_forwardings
                .iter()
                .filter_map(|(_, forwarding)| {
                    (forwarding.machine_symbol == machine.symbol
                        && forwarding.state_symbol == state.symbol)
                        .then_some(forwarding)
                })
                .collect::<Vec<_>>();
            let statements = program.statement_table.statements(state.statement_nodes);
            if assignments
                .iter()
                .any(|forwarding| forwarding.statement_index > statements.len())
            {
                diagnostic_messages.insert(format!(
                    "named ensures evidence assignment coordinate is outside {}::{}",
                    machine.name, state.name
                ));
                continue;
            }

            let mut stops_fallthrough = false;
            for statement_index in 0..=statements.len() {
                intake_checked_proof_output_propositions(
                    proof,
                    machine.symbol,
                    state.symbol,
                    statement_index,
                    &mut known,
                );
                for forwarding in assignments
                    .iter()
                    .filter(|forwarding| forwarding.statement_index == statement_index)
                {
                    let source = match &forwarding.source {
                        checked_trees::EvidenceAssignmentSource::Forwarded { term } => Some(*term),
                        checked_trees::EvidenceAssignmentSource::ProducerConformance { .. } => None,
                    };
                    if assigned
                        .insert(forwarding.output.arena_index(), source)
                        .is_some()
                    {
                        let term = proof.evidence_terms.get(forwarding.output);
                        diagnostic_messages.insert(format!(
                            "named ensures evidence `{}` is assigned more than once on a reachable path through {}::{}",
                            term.name, machine.name, state.name
                        ));
                    }
                }

                let Some(statement) = statements.get(statement_index) else {
                    continue;
                };
                if let StatementNode::Call(call) = statement {
                    intake_call_ensures_propositions(program, call, &mut known);
                }
                let StatementNode::Transition(transition) = statement else {
                    continue;
                };
                if transition.exit == TransitionExit::Ordinary {
                    for target_handle in [transition.target, transition.continuation] {
                        if !target_handle.is_valid() {
                            continue;
                        }
                        match program.statement_table.transition_target(target_handle) {
                            TransitionTargetNode::Named { path, .. } => {
                                let target = if path.symbol == machine.symbol {
                                    entry.symbol
                                } else {
                                    path.symbol
                                };
                                if states.iter().any(|state| state.symbol == target) {
                                    work.push_back((target, assigned.clone(), known.clone()));
                                } else {
                                    append_missing_evidence_diagnostics(
                                        program,
                                        proof,
                                        machine,
                                        state,
                                        &unconditional_outputs,
                                        &assigned,
                                        None,
                                        &known,
                                        &mut diagnostic_messages,
                                    );
                                }
                            }
                            TransitionTargetNode::SelfTarget => {
                                work.push_back((entry.symbol, assigned.clone(), known.clone()));
                            }
                            TransitionTargetNode::Value(_) | TransitionTargetNode::Terminal => {
                                append_missing_evidence_diagnostics(
                                    program,
                                    proof,
                                    machine,
                                    state,
                                    &unconditional_outputs,
                                    &assigned,
                                    match program.statement_table.transition_target(target_handle) {
                                        TransitionTargetNode::Value(value) => Some(*value),
                                        TransitionTargetNode::Terminal => None,
                                        _ => unreachable!(),
                                    },
                                    &known,
                                    &mut diagnostic_messages,
                                );
                            }
                        }
                    }
                }
                // Resolved-to-typed lowering has already rejected every
                // non-exhaustive maximal transition run. A miss proceeds to
                // the next consecutive arm, but the run as a whole cannot
                // fall through. Evidence forwarding is erased from the
                // runtime statement table, so its recorded coordinate must
                // also split runs: an assignment between two authored
                // dispatches cannot be backdated into the first one's exits.
                let next_coordinate_has_assignment = assignments
                    .iter()
                    .any(|forwarding| forwarding.statement_index == statement_index + 1);
                let run_ends = transition.continuation.is_valid()
                    || next_coordinate_has_assignment
                    || !matches!(
                        statements.get(statement_index + 1),
                        Some(StatementNode::Transition(_))
                    );
                if run_ends {
                    stops_fallthrough = true;
                    break;
                }
            }

            if !stops_fallthrough {
                append_missing_evidence_diagnostics(
                    program,
                    proof,
                    machine,
                    state,
                    &unconditional_outputs,
                    &assigned,
                    statements.last().and_then(|statement| match statement {
                        StatementNode::Expression(expression) => Some(*expression),
                        _ => None,
                    }),
                    &known,
                    &mut diagnostic_messages,
                );
            }
        }
    }

    if diagnostic_messages.is_empty() {
        Ok(())
    } else {
        Err(diagnostic_messages
            .into_iter()
            .map(diagnostics::Diagnostic::error)
            .collect())
    }
}

fn append_missing_evidence_diagnostics(
    program: &typed_trees::TypedTrees,
    proof: &ProofFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    outputs: &[arena::Handle<CheckedEvidenceTerm>],
    assigned: &std::collections::BTreeMap<u32, Option<arena::Handle<CheckedEvidenceTerm>>>,
    result: Option<typed_trees::expression::ExpressionHandle>,
    known: &std::collections::BTreeSet<String>,
    messages: &mut std::collections::BTreeSet<String>,
) {
    for output in outputs {
        if !assigned.contains_key(&output.arena_index()) {
            messages.insert(format!(
                "named ensures evidence `{}` is not definitely assigned on the ordinary exit through {}::{}",
                proof.evidence_terms.get(*output).name,
                machine.name,
                state.name
            ));
        }
    }

    let guarded_rows = proof
        .outcome_specific_guarantees
        .iter()
        .filter_map(|(_, row)| (row.machine_symbol == machine.symbol).then_some(row))
        .collect::<Vec<_>>();
    if guarded_rows.is_empty() {
        return;
    }
    let Some((result_data, result_case)) =
        result.and_then(|result| exact_result_case(program, result))
    else {
        messages.insert(format!(
            "cannot classify the ordinary result case on exit through {}::{}; outcome-specific guarantees require an exact nominal result constructor",
            machine.name, state.name
        ));
        return;
    };
    for row in guarded_rows {
        if row.result_data != result_data {
            messages.insert(format!(
                "ordinary exit through {}::{} produces a result outside the declared outcome-specific result sum",
                machine.name, state.name
            ));
            continue;
        }
        let Some(output) = row.evidence_term else {
            if row.result_case == result_case
                && !outcome_specific_fact_is_proved(program, row.fact, result, known)
            {
                messages.insert(format!(
                    "cannot prove outcome-specific guarantee on the matching ordinary exit through {}::{} after substituting its concrete result",
                    machine.name, state.name
                ));
            }
            continue;
        };
        let term = proof.evidence_terms.get(output);
        let assigned_source = assigned.get(&output.arena_index());
        let is_assigned = assigned_source.is_some();
        if row.result_case == result_case && !is_assigned {
            messages.insert(format!(
                "outcome-specific evidence `{}` is not definitely assigned on the matching ordinary exit through {}::{}",
                term.name, machine.name, state.name
            ));
        } else if row.result_case == result_case {
            if let Some(Some(source)) = assigned_source
                && !outcome_specific_assignment_matches_result(
                    program, proof, row.fact, result, *source,
                )
            {
                messages.insert(format!(
                    "outcome-specific evidence `{}` does not inhabit its guarantee after substituting the concrete result on exit through {}::{}",
                    term.name, machine.name, state.name
                ));
            }
        } else if is_assigned {
            messages.insert(format!(
                "outcome-specific evidence `{}` is assigned on a nonmatching ordinary exit through {}::{}",
                term.name, machine.name, state.name
            ));
        }
    }
}

fn evidence_term_named(
    terms: &arena::Arena<CheckedEvidenceTerm>,
    machine_symbol: SymbolHandle,
    name: &str,
    kind: ContractProofFactKind,
) -> Option<arena::Handle<CheckedEvidenceTerm>> {
    terms.iter().find_map(|(handle, term)| {
        (term.owner == ContractProofFactOwner::Machine { machine_symbol }
            && term.kind == kind
            && term.name == name)
            .then_some(handle)
    })
}

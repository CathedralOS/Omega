mod payload;
mod places;

use super::*;
pub(crate) use places::contract_fact_place;

use payload::semantic_contract_payload;
use points::{contract_fact_origin, contract_fact_point};

pub(super) fn append_contract_semantic_facts(
    program: &psi_typed_trees::TypedTrees,
    proof: &ProofFacts,
    facts: &mut FactPlan,
) {
    let mut semantic_handles = Vec::with_capacity(proof.contract_facts.len());

    for (contract_handle, contract) in proof.contract_facts.iter() {
        let point = contract_fact_point(contract);
        let place = contract_fact_place(program, facts, contract);
        let payload = semantic_contract_payload(program, contract);
        let declaration_fact = Fact {
            place,
            point,
            origin: contract_fact_origin(contract),
            evidence: QualificationEvidence::default(),
            payload,
        };
        let fact = match contract.kind {
            ContractProofFactKind::Requires => {
                let dependency_places =
                    places::contract_fact_dependency_places(program, facts, contract);
                if dependency_places.is_empty() {
                    facts.append_fact_context(declaration_fact)
                } else {
                    let mut refs = HandleSpan::empty();
                    let mut first = None;
                    for place in dependency_places {
                        let fact = facts.append_fact(Fact {
                            place: FactPlace::Place(place),
                            ..declaration_fact
                        });
                        first.get_or_insert(fact);
                        facts.append_ref(&mut refs, fact);
                    }
                    facts.append_context(point, refs);
                    first.expect("a non-empty dependency set must append a fact")
                }
            }
            ContractProofFactKind::Ensures => facts.append_fact(declaration_fact),
        };
        let contract_index = usize::try_from(contract_handle.arena_index())
            .expect("contract fact handle index overflow");
        while semantic_handles.len() <= contract_index {
            semantic_handles.push(None);
        }
        semantic_handles[contract_index] = Some(fact);
    }

    for (_, call) in proof.contract_calls.iter() {
        let mut combined_ref_values = Vec::new();
        let mut requires = HandleSpan::empty();
        append_call_semantic_contract_refs(
            program,
            proof,
            facts,
            call,
            call.requires,
            FactOrigin::CallRequires,
            ProgramPoint::CallRequires {
                machine_symbol: call.caller_machine_symbol,
                state_symbol: call.caller_state_symbol,
                statement_index: call.statement_index,
                call_ordinal: call.call_ordinal,
            },
            &mut requires,
        );
        combined_ref_values.extend(facts.refs.span_or_empty(requires).iter().copied());
        if !requires.is_empty() {
            facts.append_context(
                ProgramPoint::CallRequires {
                    machine_symbol: call.caller_machine_symbol,
                    state_symbol: call.caller_state_symbol,
                    statement_index: call.statement_index,
                    call_ordinal: call.call_ordinal,
                },
                requires,
            );
        }

        let mut ensures = HandleSpan::empty();
        append_call_semantic_contract_refs(
            program,
            proof,
            facts,
            call,
            call.ensures,
            FactOrigin::CallEnsures,
            ProgramPoint::CallEnsures {
                machine_symbol: call.caller_machine_symbol,
                state_symbol: call.caller_state_symbol,
                statement_index: call.statement_index,
                call_ordinal: call.call_ordinal,
            },
            &mut ensures,
        );
        combined_ref_values.extend(facts.refs.span_or_empty(ensures).iter().copied());
        if !ensures.is_empty() {
            facts.append_context(
                ProgramPoint::CallEnsures {
                    machine_symbol: call.caller_machine_symbol,
                    state_symbol: call.caller_state_symbol,
                    statement_index: call.statement_index,
                    call_ordinal: call.call_ordinal,
                },
                ensures,
            );
        }
        let mut refs = HandleSpan::empty();
        for fact_ref in combined_ref_values {
            facts.refs.append_to_span(&mut refs, fact_ref);
        }
        facts.append_symbol_set(call.target_machine_symbol, refs);
    }

    // A proof-only output binding has no runtime statement and therefore no
    // ordinary `ContractCallFact`, but the call still establishes every
    // unconditional named guarantee. Materialize one zero-runtime ensures
    // context at the binding coordinate so omitted selectors contribute facts
    // without minting caller-local terms. The invocation retains the exact
    // ordinary argument substitution independently of whether an output term
    // was selected.
    for (_, invocation) in proof.proof_output_calls.iter() {
        if invocation.runtime_call.is_some() {
            continue;
        }
        let point = ProgramPoint::CallEnsures {
            machine_symbol: invocation.caller_machine_symbol,
            state_symbol: invocation.caller_state_symbol,
            statement_index: invocation.statement_index,
            call_ordinal: 0,
        };
        let mut refs = HandleSpan::empty();
        for output in &invocation.outputs {
            let Some((contract_handle, contract)) = proof
                .contract_facts
                .iter()
                .find(|(_, contract)| contract.evidence_term == Some(output.callee_output))
            else {
                continue;
            };
            let contract_index = usize::try_from(contract_handle.arena_index())
                .expect("contract fact handle index overflow");
            let Some(Some(source_fact)) = semantic_handles.get(contract_index) else {
                continue;
            };
            let mut source = *facts.facts.get(*source_fact);
            if let FactPayload::ContractPropositionApplication {
                ref mut instantiated,
                ..
            } = source.payload
            {
                *instantiated =
                    facts.append_instantiated_expression(output.instantiated_identity.clone());
            }
            let Some(evidence) = crate::qualification_evidence::call_contract_evidence(
                program,
                invocation.target_machine_symbol,
                invocation.target_state_symbol,
                contract,
                source.payload,
                true,
            ) else {
                continue;
            };
            let fact = facts.append_fact(Fact {
                point,
                origin: FactOrigin::CallEnsures,
                evidence,
                ..source
            });
            facts.append_ref(&mut refs, fact);
        }
        if !refs.is_empty() {
            facts.append_context(point, refs);
        }
    }

    // Guarded guarantees are published only at their exact caller arm. They
    // are deliberately absent from the producer call's unconditional ensures
    // point, so the transition flow's existing fallthrough restoration keeps
    // them out of sibling arms.
    for (_, arm) in proof.outcome_specific_arms.iter() {
        let point = ProgramPoint::Statement {
            machine_symbol: arm.caller_machine_symbol,
            state_symbol: arm.caller_state_symbol,
            statement_index: arm.statement_index,
        };
        let mut refs = HandleSpan::empty();
        for row in &arm.rows {
            let guarantee = proof.outcome_specific_guarantees.get(row.guarantee);
            let contract = ContractProofFact {
                kind: ContractProofFactKind::Ensures,
                owner: ContractProofFactOwner::MachineState {
                    machine_symbol: arm.caller_machine_symbol,
                    state_symbol: arm.caller_state_symbol,
                },
                fact: guarantee.fact,
                evidence_term: row.selected_term,
                qualification_authorization: None,
            };
            let mut payload = semantic_contract_payload(program, &contract);
            if let Some(identity) = &row.instantiated_identity {
                let instantiated = facts.append_instantiated_expression(identity.clone());
                match &mut payload {
                    FactPayload::ContractBooleanExpression {
                        instantiated: slot, ..
                    }
                    | FactPayload::ContractPropositionApplication {
                        instantiated: slot, ..
                    } => *slot = instantiated,
                    _ => {}
                }
            }
            let fact = facts.append_fact(Fact {
                place: FactPlace::Unknown,
                point,
                origin: FactOrigin::CallEnsures,
                evidence: QualificationEvidence::default(),
                payload,
            });
            facts.append_ref(&mut refs, fact);
        }
        if !refs.is_empty() {
            facts.append_context(point, refs);
        }
    }

    for (_, exit) in proof.contract_exits.iter() {
        let mut refs = HandleSpan::empty();
        append_semantic_contract_refs(proof, facts, &semantic_handles, exit.ensures, &mut refs);
        facts.append_context(
            ProgramPoint::Exit {
                machine_symbol: exit.machine_symbol,
                state_symbol: exit.state_symbol,
                statement_index: exit.statement_index,
            },
            refs,
        );
        facts.append_symbol_set(exit.machine_symbol, refs);
    }
}

fn instantiate_call_proposition_payload(
    program: &psi_typed_trees::TypedTrees,
    facts: &mut FactPlan,
    call: &ContractCallFact,
    contract: &ContractProofFact,
    payload: &mut FactPayload,
) {
    let FactPayload::ContractPropositionApplication { instantiated, .. } = payload else {
        return;
    };
    let psi_typed_trees::domain::ProofFact::Proposition(application) =
        program.proof_facts.get(contract.fact)
    else {
        return;
    };
    let Some(call_site) = crate::find_call_site(
        program,
        call.caller_machine_symbol,
        call.caller_state_symbol,
        call.statement_index,
        call.call_ordinal,
    ) else {
        return;
    };
    let Some(target_parameters) = crate::call_target_parameters(program, call.target_state_symbol)
    else {
        return;
    };
    let binder_labels = application
        .binder_arguments
        .iter()
        .map(|argument| argument.display_name())
        .collect::<Vec<_>>();
    let argument_labels = program
        .expression_table
        .expression_handles(application.arguments)
        .iter()
        .map(|argument| {
            crate::checks::contracts::labels::instantiate_call_contract_expression_label(
                program,
                call.caller_state_symbol,
                call.statement_index,
                &call_site,
                target_parameters,
                *argument,
            )
        })
        .collect::<Vec<_>>();
    if let Some(formula) = program.normalize_proposition_application_with_labels(
        application,
        &binder_labels,
        &argument_labels,
    ) {
        *instantiated = facts.append_instantiated_expression(formula.identity_label());
    }
}

fn append_call_semantic_contract_refs(
    program: &psi_typed_trees::TypedTrees,
    proof: &ProofFacts,
    facts: &mut FactPlan,
    call: &ContractCallFact,
    source_refs: HandleSpan<ContractProofFactRef>,
    origin: FactOrigin,
    point: ProgramPoint,
    refs: &mut HandleSpan<FactRef>,
) {
    for source_ref in proof.contract_fact_refs.span_or_empty(source_refs) {
        let contract = proof.contract_facts.get(source_ref.fact);
        let place = instantiate_call_contract_place(program, facts, call, contract);
        let mut payload = semantic_contract_payload(program, contract);
        instantiate_call_proposition_payload(program, facts, call, contract, &mut payload);
        let evidence = crate::qualification_evidence::call_contract_evidence(
            program,
            call.target_machine_symbol,
            call.target_state_symbol,
            contract,
            payload,
            matches!(origin, FactOrigin::CallEnsures),
        );
        let Some(evidence) = evidence else {
            continue;
        };
        let carry_origin = admitted_resource_carry_origin(program, payload, evidence);
        let fact = facts.append_fact(Fact {
            place,
            point,
            origin,
            evidence,
            payload,
        });
        facts.append_ref(refs, fact);
        if let Some(value) = carry_origin {
            let fact = facts.append_fact(Fact {
                place,
                point,
                origin,
                evidence,
                payload: FactPayload::CarryOrigin { value },
            });
            facts.append_ref(refs, fact);
        }
    }
}

fn admitted_resource_carry_origin(
    program: &psi_typed_trees::TypedTrees,
    payload: FactPayload,
    evidence: QualificationEvidence,
) -> Option<psi_typed_trees::expression::ExpressionHandle> {
    if evidence.origin != psi_language_semantics::QualificationEvidenceOrigin::AdmittedReceipt {
        return None;
    }
    let FactPayload::ContractDomainMembership {
        value,
        domain_symbol,
        ..
    } = payload
    else {
        return None;
    };
    let domain = program
        .domain_definitions()
        .iter()
        .find(|domain| domain.symbol == domain_symbol)?;
    (domain.predicate_body == psi_language_semantics::DomainPredicateBody::Bodyless
        && crate::checks::type_multiplicity(program, domain.target_type)
            == psi_language_semantics::Multiplicity::Linear)
        .then_some(value)
}

fn append_semantic_contract_refs(
    proof: &ProofFacts,
    facts: &mut FactPlan,
    semantic_handles: &[Option<psi_facts::FactHandle>],
    source_refs: HandleSpan<ContractProofFactRef>,
    refs: &mut HandleSpan<FactRef>,
) {
    for source_ref in proof.contract_fact_refs.span_or_empty(source_refs) {
        let source_index = usize::try_from(source_ref.fact.arena_index())
            .expect("contract fact ref handle index overflow");
        let Some(Some(fact)) = semantic_handles.get(source_index) else {
            continue;
        };
        facts.append_ref(refs, *fact);
    }
}

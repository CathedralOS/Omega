use super::points;
use crate::semantic_places::instantiate_call_contract_place;
use arena::HandleSpan;
use checked_trees::{
    ContractCallFact, ContractProofFact, ContractProofFactKind, ContractProofFactOwner,
    ContractProofFactRef, ProofFacts,
};
use facts::{
    Fact, FactOrigin, FactPayload, FactPlace, FactPlan, FactRef, ProgramPoint,
    QualificationEvidence,
};
use typed_trees::proposition::PropositionLabels;
mod payload;
mod places;
#[cfg(test)]
mod tests;
pub(crate) use places::contract_fact_place;

use payload::semantic_contract_payload;
use points::{contract_fact_origin, contract_fact_point};

pub(super) fn append_contract_semantic_facts(
    program: &typed_trees::TypedTrees,
    proof: &ProofFacts,
    facts: &mut FactPlan,
) {
    let mut semantic_handles = Vec::with_capacity(proof.contract_facts.len());

    for (contract_handle, contract) in proof.contract_facts.iter() {
        let point = contract_fact_point(program, contract);
        let place = contract_fact_place(program, facts, contract);
        let mut payload = semantic_contract_payload(program, contract);
        instantiate_inherited_contract_payload(program, proof, facts, contract, &mut payload);
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
            ContractProofFactKind::Ensures => {
                let fact = facts.append_fact(declaration_fact);
                // Keep declaration-owned dependency coordinates for review,
                // just as requires retain their read subjects. These remain
                // obligations: do not append an assumption context or import
                // the extra rows into an executable exit's premises.
                for place in places::contract_fact_dependency_places(program, facts, contract) {
                    if !facts.fact_place_equals(declaration_fact.place, place) {
                        facts.append_fact(Fact {
                            place: FactPlace::Place(place),
                            ..declaration_fact
                        });
                    }
                }
                fact
            }
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
            proof.contract_fact_refs.span_or_empty(call.requires),
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

        let point = ProgramPoint::CallEnsures {
            machine_symbol: call.caller_machine_symbol,
            state_symbol: call.caller_state_symbol,
            statement_index: call.statement_index,
            call_ordinal: call.call_ordinal,
        };
        // Each promise has its own validity dependencies. Keep its evidence
        // and dependency rows atomic, but do not let an unresolved or changed
        // operand of a sibling promise retire an independent scalar guarantee.
        // Flow already imports every context at this exact invocation point.
        for source_ref in proof.contract_fact_refs.span_or_empty(call.ensures) {
            let mut ensures = HandleSpan::empty();
            append_call_semantic_contract_refs(
                program,
                proof,
                facts,
                call,
                std::slice::from_ref(source_ref),
                FactOrigin::CallEnsures,
                point,
                &mut ensures,
            );
            combined_ref_values.extend(facts.refs.span_or_empty(ensures).iter().copied());
            if !ensures.is_empty() {
                facts.append_context(point, ensures);
            }
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
            let Some(evidence) = crate::facts::qualification_evidence::call_contract_evidence(
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
        for row in &arm.rows {
            // Each guarantee has its own validity intersection. Copies rooted
            // at that row's result/reference/interface dependencies belong to
            // one context so a write to any dependency invalidates the row,
            // without coupling otherwise independent guarded rows.
            let mut refs = HandleSpan::empty();
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
                inherited_scope: None,
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
            let dependency_places =
                places::outcome_specific_fact_dependency_places(program, facts, proof, arm, row);
            if dependency_places.is_empty() {
                let fact = facts.append_fact(Fact {
                    place: FactPlace::Unknown,
                    point,
                    origin: FactOrigin::CallEnsures,
                    evidence: QualificationEvidence::default(),
                    payload,
                });
                facts.append_ref(&mut refs, fact);
            } else {
                for place in dependency_places {
                    let fact = facts.append_fact(Fact {
                        place: FactPlace::Place(place),
                        point,
                        origin: FactOrigin::CallEnsures,
                        evidence: QualificationEvidence::default(),
                        payload,
                    });
                    facts.append_ref(&mut refs, fact);
                }
            }
            if !refs.is_empty() {
                facts.append_context(point, refs);
            }
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
                transition_target: exit.transition_target,
            },
            refs,
        );
        facts.append_symbol_set(exit.machine_symbol, refs);
    }
}

/// Resolve one inherited contract fact's migrated proposition endpoint: the
/// declaring trait, its requirement signature, and the satisfying state the
/// fact is owned by. `None` fails closed -- the fact keeps its authored schema
/// identity rather than publishing a partial substitution.
fn inherited_contract_scope_parts<'program>(
    program: &'program typed_trees::TypedTrees,
    proof: &'program ProofFacts,
    contract: &ContractProofFact,
) -> Option<(
    &'program typed_trees::trait_definition::TraitDefinition,
    &'program typed_trees::signature::StateSignature,
    &'program typed_trees::state::State,
    &'program checked_trees::InheritedContractScope,
)> {
    let scope = proof
        .inherited_contract_scopes
        .get(contract.inherited_scope?);
    let trait_definition = program
        .traits()
        .iter()
        .find(|definition| definition.symbol == scope.declaring_trait)?;
    let requirement = program
        .trait_machine_signatures(trait_definition)
        .iter()
        .find(|signature| signature.symbol == scope.requirement)?;
    let ContractProofFactOwner::MachineState { state_symbol, .. } = contract.owner else {
        return None;
    };
    let satisfier_state = program.machines().iter().find_map(|machine| {
        program
            .machine_states(machine)
            .iter()
            .find(|state| state.symbol == state_symbol)
    })?;
    Some((trait_definition, requirement, satisfier_state, scope))
}

/// Instantiate one requirement-authored proposition fact through the exact
/// conformance edge recorded on it. The migrated obligation keeps its
/// obligation kind; only the endpoint family and the argument labels move from
/// the requirement's schema onto the satisfying state's own terms.
fn instantiate_inherited_contract_payload(
    program: &typed_trees::TypedTrees,
    proof: &ProofFacts,
    facts: &mut FactPlan,
    contract: &ContractProofFact,
    payload: &mut FactPayload,
) {
    let FactPayload::ContractPropositionApplication { instantiated, .. } = payload else {
        return;
    };
    let Some((trait_definition, requirement, satisfier_state, scope)) =
        inherited_contract_scope_parts(program, proof, contract)
    else {
        return;
    };
    let typed_trees::domain::ProofFact::Proposition(application) =
        program.proof_facts.get(contract.fact)
    else {
        return;
    };
    let Some(label) = validation::inherited_requirement_proposition_label(
        program,
        trait_definition,
        requirement,
        satisfier_state,
        &scope.trait_arguments,
        application,
    ) else {
        return;
    };
    *instantiated = facts.append_instantiated_expression(label);
}

fn instantiate_call_contract_payload(
    program: &typed_trees::TypedTrees,
    proof: &ProofFacts,
    facts: &mut FactPlan,
    call: &ContractCallFact,
    contract: &ContractProofFact,
    payload: &mut FactPayload,
) {
    if !matches!(
        payload,
        FactPayload::ContractPropositionApplication { .. }
            | FactPayload::ContractBooleanExpression { .. }
    ) {
        return;
    }
    let Some(call_site) = crate::semantic_calls::find_call_site(
        program,
        call.caller_machine_symbol,
        call.caller_state_symbol,
        call.statement_index,
        call.call_ordinal,
    ) else {
        return;
    };
    let target_parameters = if let Some(dispatch) = call_site.static_requirement_dispatch() {
        let Some(requirement) = program
            .traits()
            .iter()
            .find(|definition| definition.symbol == dispatch.declaring_trait)
            .and_then(|definition| {
                program
                    .trait_machine_signatures(definition)
                    .iter()
                    .find(|requirement| requirement.symbol == dispatch.requirement)
            })
        else {
            return;
        };
        program.state_signature_parameters(requirement)
    } else {
        let Some(parameters) =
            crate::semantic_calls::call_target_parameters(program, call.target_state_symbol)
        else {
            return;
        };
        parameters
    };
    // A migrated fact's expressions name the requirement's parameters, not the
    // satisfying state's. Alias the requirement's exact parameter identities
    // onto the target parameter row so call-argument resolution stays
    // positional; requirement arity is already validated against the target.
    let mut alias_parameters = Vec::new();
    let mut inherited_endpoint = None;
    if let Some((trait_definition, requirement, satisfier_state, scope)) =
        inherited_contract_scope_parts(program, proof, contract)
    {
        let required_parameters = program.state_signature_parameters(requirement);
        // Boundary satisfiers may carry one extra leading trait-typed adapter
        // parameter; align through the validation helper so the requirement
        // telescope binds positionally to the satisfier's own parameter row.
        // Arity drift fails closed, keeping the authored schema identity.
        let Some(satisfier_parameters) = validation::inherited_satisfier_parameters(
            program,
            trait_definition,
            requirement,
            satisfier_state,
        ) else {
            return;
        };
        alias_parameters = required_parameters
            .iter()
            .zip(satisfier_parameters.iter())
            .map(
                |(required, actual)| typed_trees::signature::StateParameter {
                    symbol: required.symbol,
                    name: required.name.clone(),
                    type_reference: actual.type_reference,
                    is_const: actual.is_const,
                    is_mutable: actual.is_mutable,
                    is_self: actual.is_self,
                    relevance: actual.relevance,
                },
            )
            .collect();
        if matches!(payload, FactPayload::ContractPropositionApplication { .. }) {
            let typed_trees::domain::ProofFact::Proposition(application) =
                program.proof_facts.get(contract.fact)
            else {
                return;
            };
            let Some(endpoint) = validation::inherited_requirement_proposition_application(
                program,
                trait_definition,
                requirement,
                satisfier_state,
                &scope.trait_arguments,
                application,
            ) else {
                return;
            };
            inherited_endpoint = Some(endpoint);
        }
    }
    let label_parameters: &[typed_trees::signature::StateParameter] =
        if inherited_endpoint.is_some() || !alias_parameters.is_empty() {
            &alias_parameters
        } else {
            target_parameters
        };
    if let FactPayload::ContractBooleanExpression {
        expression,
        instantiated,
        ..
    } = payload
    {
        // A call promise belongs to its captured actuals and result occurrence.
        // Publishing the callee's uninstantiated spelling could identify its
        // formal `value` or reserved `result` with unrelated caller binders.
        let label = crate::checks::contracts::labels::instantiate_call_contract_expression_label(
            program,
            call.caller_state_symbol,
            call.statement_index,
            &call_site,
            label_parameters,
            *expression,
        );
        *instantiated = facts.append_instantiated_expression(label);
        return;
    }
    let FactPayload::ContractPropositionApplication { instantiated, .. } = payload else {
        return;
    };
    let (application, binder_labels) = match inherited_endpoint {
        Some(endpoint) => (endpoint.application, endpoint.binder_labels),
        None => {
            let typed_trees::domain::ProofFact::Proposition(application) =
                program.proof_facts.get(contract.fact)
            else {
                return;
            };
            let binder_labels = application
                .binder_arguments
                .iter()
                .map(|argument| argument.display_name())
                .collect::<Vec<_>>();
            (application.clone(), binder_labels)
        }
    };
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
                label_parameters,
                *argument,
            )
        })
        .collect::<Vec<_>>();
    if let Some(formula) = program.normalize_proposition_application(
        &application,
        Some(PropositionLabels {
            binder_labels: &binder_labels,
            argument_labels: &argument_labels,
        }),
    ) {
        *instantiated = facts.append_instantiated_expression(formula.identity_label());
    }
}

fn append_call_semantic_contract_refs(
    program: &typed_trees::TypedTrees,
    proof: &ProofFacts,
    facts: &mut FactPlan,
    call: &ContractCallFact,
    source_refs: &[ContractProofFactRef],
    origin: FactOrigin,
    point: ProgramPoint,
    refs: &mut HandleSpan<FactRef>,
) {
    for source_ref in source_refs {
        let contract = proof.contract_facts.get(source_ref.fact);
        let place = instantiate_call_contract_place(program, facts, call, contract);
        let mut payload = semantic_contract_payload(program, contract);
        instantiate_call_contract_payload(program, proof, facts, call, contract, &mut payload);
        if matches!(payload, FactPayload::ContractBooleanExpression { instantiated, .. } if !instantiated.is_valid())
        {
            // Missing call/parameter custody cannot publish a callee-local
            // Boolean expression into the caller's semantic context.
            continue;
        }
        let evidence = crate::facts::qualification_evidence::call_contract_evidence(
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
        // A Boolean postcondition's outer expression is not its storage
        // dependency. Retain each exact instantiated operand in the same
        // context so the shared mutation filter retires the whole promise
        // when any referenced value changes.
        if origin == FactOrigin::CallEnsures
            && matches!(payload, FactPayload::ContractBooleanExpression { .. })
        {
            for occurrence in crate::facts::contract_occurrences::fact_referenced_occurrences(
                program,
                contract.fact,
            ) {
                let dependency =
                    crate::semantic_places::instantiate_call_contract_expression_place(
                        program, facts, call, occurrence,
                    )
                    .map(FactPlace::Place)
                    .unwrap_or(FactPlace::Unknown);
                if dependency == place {
                    continue;
                }
                let dependency_fact = facts.append_fact(Fact {
                    place: dependency,
                    point,
                    origin,
                    evidence,
                    payload: FactPayload::StorageDependency { dependent: fact },
                });
                facts.append_ref(refs, dependency_fact);
            }
        }
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
    program: &typed_trees::TypedTrees,
    payload: FactPayload,
    evidence: QualificationEvidence,
) -> Option<typed_trees::expression::ExpressionHandle> {
    if evidence.origin != language_semantics::QualificationEvidenceOrigin::AdmittedReceipt {
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
    (domain.predicate_body == language_semantics::DomainPredicateBody::Bodyless
        && crate::checks::type_multiplicity(program, domain.target_type)
            == language_semantics::Multiplicity::Linear)
        .then_some(value)
}

fn append_semantic_contract_refs(
    proof: &ProofFacts,
    facts: &mut FactPlan,
    semantic_handles: &[Option<facts::FactHandle>],
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

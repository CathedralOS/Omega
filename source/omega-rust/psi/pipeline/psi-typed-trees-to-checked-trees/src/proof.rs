use crate::context::*;
use psi_checked_trees::CheckedEvidenceTerm;
mod contracts;
mod float_meaning;
mod obligations;

pub(crate) use contracts::machine_parameter_evidence_signatures;
use contracts::{
    append_inherited_trait_contract_facts, append_machine_contract_facts,
    append_operator_declaration_contract_facts, append_state_contract_facts,
    append_state_signature_contract_facts, build_contract_call_facts, build_contract_exit_facts,
    build_contract_operator_use_facts, estimated_contract_fact_capacity,
};
use obligations::lower_proof_obligation;

#[cfg(test)]
pub(crate) fn build_proof_facts(
    program: &psi_typed_trees::TypedTrees,
    proof_plan: &psi_proof::obligations::ProofPlan,
    borrow: &BorrowFacts,
) -> ProofFacts {
    build_proof_facts_with_operators(
        program,
        proof_plan,
        borrow,
        &CheckedOperatorFacts::default(),
    )
}

pub(crate) fn build_proof_facts_with_operators(
    program: &psi_typed_trees::TypedTrees,
    proof_plan: &psi_proof::obligations::ProofPlan,
    borrow: &BorrowFacts,
    operators: &CheckedOperatorFacts,
) -> ProofFacts {
    let mut obligations = psi_arena::Arena::with_capacity(proof_plan.obligations.len());
    let mut contract_facts =
        psi_arena::Arena::with_capacity(estimated_contract_fact_capacity(program));
    let mut outcome_specific_guarantees = psi_arena::Arena::default();
    let mut evidence_terms = psi_arena::Arena::default();

    for (_, obligation) in proof_plan.obligations.iter() {
        obligations.append(lower_proof_obligation(obligation));
    }

    for machine in program.machines() {
        append_machine_contract_facts(program, machine, &mut contract_facts, &mut evidence_terms);
        let mut guarded_lane_position = 0usize;
        for contract in program.machine_contracts(machine) {
            let psi_typed_trees::signature::SignatureContractKind::EnsuresForResultCase {
                result_data,
                result_case,
            } = &contract.kind
            else {
                continue;
            };
            for fact in fact_handles(contract.facts) {
                let evidence_term = contract.binding.as_ref().map(|binding| {
                    let psi_typed_trees::domain::ProofFact::Proposition(application) =
                        program.proof_facts.get(fact)
                    else {
                        unreachable!("validated named guarded guarantee must bind a proposition")
                    };
                    let normalized = program
                        .normalize_nominal_proposition_application(application)
                        .expect("validated named guarded guarantee must have a nominal endpoint");
                    let (evidence_type, evidence_interface) = match &normalized.classification {
                        psi_typed_trees::proposition::PropositionEvidenceClassification::Witness {
                            evidence,
                            interface,
                        } => (
                            evidence.clone(),
                            interface.as_ref().map(lower_checked_evidence_interface),
                        ),
                        psi_typed_trees::proposition::PropositionEvidenceClassification::FactOnly => {
                            unreachable!("validated named guarded guarantee must bind witness evidence")
                        }
                    };
                    let term = evidence_terms.append(CheckedEvidenceTerm {
                        name: binding.as_str().to_owned(),
                        owner: ContractProofFactOwner::Machine {
                            machine_symbol: machine.symbol,
                        },
                        kind: ContractProofFactKind::Ensures,
                        lane_position: guarded_lane_position,
                        proposition: lower_checked_proposition_application(normalized),
                        evidence_type,
                        evidence_interface,
                    });
                    guarded_lane_position += 1;
                    term
                });
                outcome_specific_guarantees.append(
                    psi_checked_trees::OutcomeSpecificGuaranteeFact {
                        machine_symbol: machine.symbol,
                        result_data: *result_data,
                        result_case: *result_case,
                        public_selector: contract
                            .binding
                            .as_ref()
                            .map(|binding| binding.as_str().to_owned()),
                        fact,
                        evidence_term,
                    },
                );
            }
        }
        for state in program.machine_states(machine) {
            append_state_contract_facts(
                program,
                machine,
                state,
                &mut contract_facts,
                &mut evidence_terms,
            );
        }
        append_inherited_trait_contract_facts(
            program,
            machine,
            &mut contract_facts,
            &evidence_terms,
        );
        for (owner_symbol, _, contract) in
            machine_parameter_evidence_signatures(program, program.machine_type_parameters(machine))
        {
            append_state_signature_contract_facts(
                program,
                owner_symbol,
                std::slice::from_ref(contract),
                &mut contract_facts,
                &mut evidence_terms,
            );
        }
    }
    for definition in program.data_definitions() {
        for (owner_symbol, _, contract) in
            machine_parameter_evidence_signatures(program, program.data_type_parameters(definition))
        {
            append_state_signature_contract_facts(
                program,
                owner_symbol,
                std::slice::from_ref(contract),
                &mut contract_facts,
                &mut evidence_terms,
            );
        }
    }
    for definition in program.domain_definitions() {
        for (owner_symbol, _, contract) in machine_parameter_evidence_signatures(
            program,
            program.domain_type_parameters(definition),
        ) {
            append_state_signature_contract_facts(
                program,
                owner_symbol,
                std::slice::from_ref(contract),
                &mut contract_facts,
                &mut evidence_terms,
            );
        }
    }
    for trait_definition in program.traits() {
        for (owner_symbol, _, contract) in machine_parameter_evidence_signatures(
            program,
            program.trait_type_parameters(trait_definition),
        ) {
            append_state_signature_contract_facts(
                program,
                owner_symbol,
                std::slice::from_ref(contract),
                &mut contract_facts,
                &mut evidence_terms,
            );
        }
        for requirement in program.trait_machine_signatures(trait_definition) {
            for (owner_symbol, _, contract) in machine_parameter_evidence_signatures(
                program,
                program.state_signature_type_parameters(requirement),
            ) {
                append_state_signature_contract_facts(
                    program,
                    owner_symbol,
                    std::slice::from_ref(contract),
                    &mut contract_facts,
                    &mut evidence_terms,
                );
            }
        }
        append_state_signature_contract_facts(
            program,
            trait_definition.symbol,
            program.trait_machine_signatures(trait_definition),
            &mut contract_facts,
            &mut evidence_terms,
        );
    }
    for operator in program.operators().iter().chain(
        program
            .domain_definitions()
            .iter()
            .flat_map(|domain| program.domain_operators(domain)),
    ) {
        append_operator_declaration_contract_facts(program, operator, &mut contract_facts);
    }
    let (mut contract_fact_refs, contract_calls) =
        build_contract_call_facts(program, borrow, &contract_facts);
    let contract_operator_uses = build_contract_operator_use_facts(
        program,
        operators,
        &mut contract_facts,
        &mut contract_fact_refs,
    );
    let contract_exits =
        build_contract_exit_facts(program, &contract_facts, &mut contract_fact_refs);
    let proposition_vocabulary = build_checked_proposition_vocabulary(program);

    ProofFacts::with_roots(
        obligations,
        contract_facts,
        outcome_specific_guarantees,
        psi_arena::Arena::default(),
        evidence_terms,
        psi_arena::Arena::default(),
        psi_arena::Arena::default(),
        psi_arena::Arena::default(),
        contract_fact_refs,
        contract_calls,
        contract_exits,
        contract_operator_uses,
        Vec::new(),
        Vec::new(),
        proposition_vocabulary,
    )
}

pub(crate) use float_meaning::bind_float_meaning_projection_facts;

pub(crate) fn bind_evidence_forwarding_facts(
    program: &psi_typed_trees::TypedTrees,
    proof: &mut ProofFacts,
) -> Result<(), Vec<psi_diagnostics::Diagnostic>> {
    let mut diagnostics = Vec::new();
    let mut forwardings = psi_arena::Arena::default();

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
            diagnostics.push(psi_diagnostics::Diagnostic::error(format!(
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
                diagnostics.push(psi_diagnostics::Diagnostic::error(format!(
                    "cannot forward evidence term `{}` into `{}` because their proposition identities differ",
                    forwarding.source, forwarding.target
                )));
                continue;
            }
            psi_checked_trees::EvidenceAssignmentSource::Forwarded { term: source }
        } else if let Some(conformance_symbol) = forwarding.source_conformance {
            let Some(source) = checked_evidence_producer(
                program,
                output_term,
                conformance_symbol,
                forwarding.source.as_str(),
            ) else {
                diagnostics.push(psi_diagnostics::Diagnostic::error(format!(
                    "subjectless conformance `{}` does not provide the exact `{}` evidence interface required by `{}`",
                    forwarding.source, output_term.evidence_type, forwarding.target
                )));
                continue;
            };
            source
        } else {
            diagnostics.push(psi_diagnostics::Diagnostic::error(format!(
                "evidence forwarding source `{}` is not a named requires binding of this machine, an explicit subjectless conformance, or an available proof-output term",
                forwarding.source
            )));
            continue;
        };
        forwardings.append(psi_checked_trees::EvidenceForwardingFact {
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

pub(crate) fn bind_proof_output_call_facts(
    program: &psi_typed_trees::TypedTrees,
    proof: &mut ProofFacts,
) -> Result<(), Vec<psi_diagnostics::Diagnostic>> {
    use psi_typed_trees::expression::ExpressionNode;

    let mut diagnostics = Vec::new();
    let mut invocations = psi_arena::Arena::default();

    for package in &program.proof_output_calls {
        let ExpressionNode::Call(call) = program.expression_table.expression(package.call) else {
            diagnostics.push(psi_diagnostics::Diagnostic::error(
                "proof-output binding requires a direct call",
            ));
            continue;
        };
        let Some((target_machine, target_state)) = program.machines().iter().find_map(|machine| {
            program
                .machine_states(machine)
                .iter()
                .find(|state| state.symbol == call.target_symbol)
                .map(|state| (machine, state))
        }) else {
            diagnostics.push(psi_diagnostics::Diagnostic::error(format!(
                "proof-output call `{}` must target a concrete machine state",
                call.target
            )));
            continue;
        };
        let static_requirement = match checked_static_requirement_dispatch(
            program,
            package.machine_symbol,
            call,
            target_machine,
            target_state,
        ) {
            Ok(dispatch) => dispatch,
            Err(diagnostic) => {
                diagnostics.push(diagnostic);
                continue;
            }
        };

        let concrete = target_machine.lifetime_parameters.is_empty()
            && target_machine.type_parameters.is_empty()
            && target_machine.conformance_bounds.is_empty()
            && call.machine_arguments.is_empty();
        let immediate = program.machine_states(target_machine).len() == 1
            && target_machine.supply_mode == psi_language_semantics::MachineSupplyMode::CheckedBody;
        let runtime_value_type = target_state
            .return_type
            .is_valid()
            .then(|| program.primitive_type_reference(target_state.return_type))
            .flatten();
        let proof_only = !target_state.return_type.is_valid();
        if !concrete || !immediate || (!proof_only && runtime_value_type.is_none()) {
            diagnostics.push(psi_diagnostics::Diagnostic::error(format!(
                "proof-output call `{}` is currently limited to a concrete one-state Unit- or scalar-result machine",
                call.target
            )));
            continue;
        }

        let public_owner = static_requirement.as_ref().map(|(_, requirement)| {
            ContractProofFactOwner::StateSignature {
                owner_symbol: call
                    .static_requirement_dispatch
                    .as_ref()
                    .expect("checked static requirement dispatch")
                    .declaring_trait,
                state_symbol: requirement.symbol,
            }
        });
        let mut callee_inputs = proof
            .evidence_terms
            .iter()
            .filter_map(|(handle, term)| {
                let owner_matches = public_owner.map_or_else(
                    || {
                        term.owner
                            == ContractProofFactOwner::Machine {
                                machine_symbol: target_machine.symbol,
                            }
                            || term.owner
                                == (ContractProofFactOwner::MachineState {
                                    machine_symbol: target_machine.symbol,
                                    state_symbol: target_state.symbol,
                                })
                    },
                    |owner| term.owner == owner,
                );
                (term.kind == ContractProofFactKind::Requires && owner_matches).then_some(handle)
            })
            .collect::<Vec<_>>();
        callee_inputs.sort_by_key(|handle| proof.evidence_terms.get(*handle).lane_position);
        let mut callee_outputs = proof
            .evidence_terms
            .iter()
            .filter_map(|(handle, term)| {
                let owner_matches = public_owner.map_or(
                    term.owner
                        == (ContractProofFactOwner::Machine {
                            machine_symbol: target_machine.symbol,
                        }),
                    |owner| term.owner == owner,
                );
                (term.kind == ContractProofFactKind::Ensures
                    && owner_matches
                    && !proof
                        .outcome_specific_guarantees
                        .iter()
                        .any(|(_, row)| row.evidence_term == Some(handle)))
                .then_some(handle)
            })
            .collect::<Vec<_>>();
        callee_outputs.sort_by_key(|handle| proof.evidence_terms.get(*handle).lane_position);
        if callee_outputs.is_empty() {
            diagnostics.push(psi_diagnostics::Diagnostic::error(format!(
                "proof-output call `{}` requires at least one unconditional named ensures output",
                call.target
            )));
            continue;
        }
        if call.evidence_arguments.len() != callee_inputs.len() {
            diagnostics.push(psi_diagnostics::Diagnostic::error(format!(
                "proof-output call `{}` supplies {} erased evidence argument{} but its named requires lane has {}",
                call.target,
                call.evidence_arguments.len(),
                if call.evidence_arguments.len() == 1 { "" } else { "s" },
                callee_inputs.len(),
            )));
            continue;
        }
        let mut fields = std::collections::BTreeMap::new();
        let mut local_names = std::collections::BTreeSet::new();
        let mut invalid = false;
        for binding in &package.bindings {
            if binding.output_field.as_str() == "value" && binding.binding.as_str() == "_" {
                diagnostics.push(psi_diagnostics::Diagnostic::error(
                    "proof-output binding cannot discard its runtime Type result",
                ));
                invalid = true;
                continue;
            }
            if fields
                .insert(binding.output_field.as_str(), binding)
                .is_some()
            {
                diagnostics.push(psi_diagnostics::Diagnostic::error(format!(
                    "proof-output selector `{}` is bound more than once",
                    binding.output_field
                )));
                invalid = true;
            }
            if binding.binding.as_str() != "_" && !local_names.insert(binding.binding.as_str()) {
                diagnostics.push(psi_diagnostics::Diagnostic::error(format!(
                    "evidence term `{}` is bound more than once in this package pattern",
                    binding.binding
                )));
                invalid = true;
            }
        }
        let value_binding = fields.get("value").copied();
        match (runtime_value_type, value_binding) {
            (Some(_), None) => {
                diagnostics.push(psi_diagnostics::Diagnostic::error(format!(
                    "proof-output binding from `{}` is missing its runtime Type result",
                    call.target
                )));
                invalid = true;
            }
            (None, Some(_)) => {
                diagnostics.push(psi_diagnostics::Diagnostic::error(format!(
                    "proof-only call `{}` has no runtime Type result to bind",
                    call.target
                )));
                invalid = true;
            }
            _ => {}
        }
        for field in fields.keys() {
            if !(*field == "value" && runtime_value_type.is_some())
                && !callee_outputs
                    .iter()
                    .any(|output| proof.evidence_terms.get(*output).name == *field)
            {
                diagnostics.push(psi_diagnostics::Diagnostic::error(format!(
                    "call `{}` publishes no proof-output selector `{field}`",
                    call.target
                )));
                invalid = true;
            }
        }

        let runtime_call = if let Some(statement_index) = package.runtime_call_statement_index {
            let Some(caller_state) = program.machines().iter().find_map(|machine| {
                program
                    .machine_states(machine)
                    .iter()
                    .find(|state| state.symbol == package.state_symbol)
            }) else {
                diagnostics.push(psi_diagnostics::Diagnostic::error(
                    "proof-output binding has no caller state",
                ));
                continue;
            };
            let runtime_statement = program
                .statement_table
                .statements(caller_state.statement_nodes)
                .get(statement_index);
            let exact_runtime_statement =
                match (runtime_value_type, value_binding, runtime_statement) {
                    (
                        Some(runtime_value_type),
                        Some(value_binding),
                        Some(psi_typed_trees::statement::StatementNode::LocalData(local)),
                    ) => {
                        let local_call = local
                            .initial_value
                            .is_valid()
                            .then(|| program.expression_table.expression(local.initial_value));
                        local.name == value_binding.binding
                            && program.primitive_type_reference(local.type_reference)
                                == Some(runtime_value_type)
                            && matches!(local_call, Some(ExpressionNode::Call(local_call))
                                if local_call.target_symbol == call.target_symbol)
                    }
                    (
                        None,
                        None,
                        Some(psi_typed_trees::statement::StatementNode::Call(unit_call)),
                    ) => unit_call.target_symbol == call.target_symbol,
                    _ => false,
                };
            if !exact_runtime_statement
                || package.statement_index != statement_index.saturating_add(1)
            {
                diagnostics.push(psi_diagnostics::Diagnostic::error(
                    "proof-output binding does not match its exact ordinary call",
                ));
                continue;
            }
            let mut matching_calls = proof.contract_calls.iter().filter_map(|(_, fact)| {
                (fact.caller_machine_symbol == package.machine_symbol
                    && fact.caller_state_symbol == package.state_symbol
                    && fact.statement_index == statement_index
                    && fact.call_ordinal == 0)
                    .then_some(fact)
            });
            let Some(contract_call) = matching_calls.next() else {
                diagnostics.push(psi_diagnostics::Diagnostic::error(
                    "proof-output call has no exact checked contract-call row",
                ));
                continue;
            };
            if matching_calls.next().is_some()
                || contract_call.target_machine_symbol != target_machine.symbol
                || contract_call.target_state_symbol != target_state.symbol
            {
                diagnostics.push(psi_diagnostics::Diagnostic::error(
                    "proof-output call disagrees with its checked contract-call row",
                ));
                continue;
            }
            Some(psi_checked_trees::ProofOutputRuntimeCallFact {
                statement_index,
                call_ordinal: 0,
            })
        } else {
            if runtime_value_type.is_some() {
                diagnostics.push(psi_diagnostics::Diagnostic::error(format!(
                    "proof-output call `{}` is missing its runtime Type result",
                    call.target
                )));
                continue;
            }
            None
        };
        for binding in &package.bindings {
            if binding.binding.as_str() == "_" {
                continue;
            }
            let duplicate = proof.evidence_terms.iter().any(|(_, term)| {
                term.name == binding.binding.as_str()
                    && (term.owner
                        == (ContractProofFactOwner::Machine {
                            machine_symbol: package.machine_symbol,
                        })
                        || term.owner
                            == (ContractProofFactOwner::MachineState {
                                machine_symbol: package.machine_symbol,
                                state_symbol: package.state_symbol,
                            }))
            }) || invocations.iter().any(
                |(_, invocation): (_, &psi_checked_trees::ProofOutputCallFact)| {
                    invocation.caller_machine_symbol == package.machine_symbol
                        && invocation.caller_state_symbol == package.state_symbol
                        && invocation.outputs.iter().any(|output| {
                            output.output.is_some_and(|output| {
                                proof.evidence_terms.get(output).name == binding.binding.as_str()
                            })
                        })
                },
            );
            if duplicate {
                diagnostics.push(psi_diagnostics::Diagnostic::error(format!(
                    "evidence term `{}` is already bound in this machine state",
                    binding.binding
                )));
                invalid = true;
            }
        }
        if invalid {
            continue;
        }

        let mut evidence_arguments = Vec::with_capacity(callee_inputs.len());
        for (input_position, (authored, callee_input)) in call
            .evidence_arguments
            .iter()
            .zip(callee_inputs)
            .enumerate()
        {
            let Some(source) =
                proof_output_source_term_by_name(proof, &invocations, package, authored.as_str())
            else {
                diagnostics.push(psi_diagnostics::Diagnostic::error(format!(
                    "unknown incoming evidence term `{}` in proof-output call `{}`",
                    authored, call.target,
                )));
                invalid = true;
                continue;
            };
            let Some((instantiated_proposition, instantiated_identity)) =
                instantiate_proof_output_proposition(
                    program,
                    proof,
                    package,
                    call,
                    static_requirement.as_ref().map_or_else(
                        || program.state_parameters(target_state),
                        |(_, requirement)| program.state_signature_parameters(requirement),
                    ),
                    callee_input,
                )
            else {
                diagnostics.push(psi_diagnostics::Diagnostic::error(format!(
                    "proof-output call `{}` cannot instantiate erased requires position {}",
                    call.target, input_position,
                )));
                invalid = true;
                continue;
            };
            if proof.evidence_terms.get(source).proposition != instantiated_proposition {
                diagnostics.push(psi_diagnostics::Diagnostic::error(format!(
                    "evidence term `{}` does not inhabit erased requires position {} of proof-output call `{}`",
                    authored, input_position, call.target,
                )));
                invalid = true;
                continue;
            }
            append_proposition_application_if_missing(proof, &instantiated_proposition);
            evidence_arguments.push(psi_checked_trees::ProofOutputEvidenceArgumentFact {
                input_position,
                callee_input,
                source,
                instantiated_proposition,
                instantiated_identity,
            });
        }
        if invalid {
            continue;
        }

        let mut outputs = Vec::with_capacity(callee_outputs.len());
        for callee_output in callee_outputs {
            let declaration = proof.evidence_terms.get(callee_output).clone();
            let Some((instantiated_proposition, instantiated_identity)) =
                instantiate_proof_output_proposition(
                    program,
                    proof,
                    package,
                    call,
                    static_requirement.as_ref().map_or_else(
                        || program.state_parameters(target_state),
                        |(_, requirement)| program.state_signature_parameters(requirement),
                    ),
                    callee_output,
                )
            else {
                diagnostics.push(psi_diagnostics::Diagnostic::error(format!(
                    "proof-output call `{}` cannot instantiate output `{}`",
                    call.target, declaration.name,
                )));
                invalid = true;
                continue;
            };
            append_proposition_application_if_missing(proof, &instantiated_proposition);
            let binding = fields.get(declaration.name.as_str());
            let output = binding
                .filter(|binding| binding.binding.as_str() != "_")
                .map(|binding| {
                    proof.evidence_terms.append(CheckedEvidenceTerm {
                        name: binding.binding.as_str().to_owned(),
                        owner: ContractProofFactOwner::MachineState {
                            machine_symbol: package.machine_symbol,
                            state_symbol: package.state_symbol,
                        },
                        kind: ContractProofFactKind::Ensures,
                        lane_position: declaration.lane_position,
                        proposition: instantiated_proposition.clone(),
                        evidence_type: declaration.evidence_type,
                        evidence_interface: declaration.evidence_interface,
                    })
                });
            outputs.push(psi_checked_trees::ProofOutputFact {
                output_position: declaration.lane_position,
                callee_output,
                instantiated_proposition,
                instantiated_identity,
                output,
            });
        }
        if invalid {
            continue;
        }
        invocations.append(psi_checked_trees::ProofOutputCallFact {
            caller_machine_symbol: package.machine_symbol,
            caller_state_symbol: package.state_symbol,
            statement_index: package.statement_index,
            source_statement_index: package.source_statement_index,
            runtime_call,
            target_machine_symbol: target_machine.symbol,
            target_state_symbol: target_state.symbol,
            static_requirement_dispatch: static_requirement.map(|(fact, _)| fact),
            evidence_arguments,
            outputs,
        });
    }

    if diagnostics.is_empty() {
        proof.proof_output_calls = invocations;
        Ok(())
    } else {
        Err(diagnostics)
    }
}

fn checked_static_requirement_dispatch<'program>(
    program: &'program psi_typed_trees::TypedTrees,
    caller_machine: psi_symbols::SymbolHandle,
    call: &psi_typed_trees::expression::TableCallExpression,
    realization_machine: &'program psi_typed_trees::machine::Machine,
    realization_state: &'program psi_typed_trees::state::State,
) -> Result<
    Option<(
        psi_checked_trees::StaticRequirementDispatchFact,
        &'program psi_typed_trees::signature::StateSignature,
    )>,
    psi_diagnostics::Diagnostic,
> {
    use psi_typed_trees::domain::ProofFact;
    use psi_typed_trees::signature::SignatureContractKind;
    use psi_typed_trees::trait_definition::ConformanceRowSource;

    let Some(dispatch) = call.static_requirement_dispatch.as_ref() else {
        return Ok(None);
    };
    let rejected = |reason: &str| {
        psi_diagnostics::Diagnostic::error(format!(
            "static named-witness requirement call `{}` is outside the first closed dispatch rung: {reason}",
            call.target,
        ))
    };

    if dispatch.application_fingerprint == 0
        || dispatch.application_commitment.is_zero()
        || dispatch.realization_machine != realization_machine.symbol
        || dispatch.realization_state != realization_state.symbol
        || call.target_symbol != realization_state.symbol
    {
        return Err(rejected(
            "its retained public/private dispatch identities do not match the executable target",
        ));
    }

    let applications = program
        .machine_specializations
        .iter()
        .filter(|specialization| specialization.instance == caller_machine)
        .flat_map(|specialization| &specialization.conformance_applications)
        .filter(|application| {
            application.fingerprint == dispatch.application_fingerprint
                && application.commitment == dispatch.application_commitment
        })
        .collect::<Vec<_>>();
    let [application] = applications.as_slice() else {
        return Err(rejected(
            "its exact owner-scoped closed conformance application is absent or ambiguous",
        ));
    };
    if application.trait_definition != dispatch.declaring_trait {
        return Err(rejected(
            "inherited parent-trait requirement rows remain unsupported",
        ));
    }
    let rows = application
        .rows
        .iter()
        .filter(|row| {
            row.declaring_trait == dispatch.declaring_trait
                && row.requirement == dispatch.requirement
                && row.realization_machine == dispatch.realization_machine
                && row.realization_state == dispatch.realization_state
        })
        .collect::<Vec<_>>();
    if rows.len() != 1 {
        return Err(rejected(
            "its closed conformance application does not contain one exact requirement-to-realization row",
        ));
    }

    let Some(trait_definition) = program
        .traits()
        .iter()
        .find(|definition| definition.symbol == dispatch.declaring_trait)
    else {
        return Err(rejected("its declaring trait is absent"));
    };
    let requirements = program
        .trait_machine_signatures(trait_definition)
        .iter()
        .filter(|requirement| requirement.symbol == dispatch.requirement)
        .collect::<Vec<_>>();
    let [requirement] = requirements.as_slice() else {
        return Err(rejected("its public requirement is absent or ambiguous"));
    };
    let Some(selected) = program
        .conformances()
        .iter()
        .find(|conformance| conformance.symbol == application.declaration)
    else {
        return Err(rejected("its selected conformance declaration is absent"));
    };
    let selected_rows = program
        .closed_conformance_rows(selected)
        .ok_or_else(|| rejected("its selected conformance is not one complete closed map"))?;
    let exact_rows = selected_rows
        .iter()
        .filter(|row| {
            row.declaring_trait == dispatch.declaring_trait
                && row.requirement == dispatch.requirement
                && row.realization_machine == dispatch.realization_machine
                && row.realization_state == dispatch.realization_state
        })
        .collect::<Vec<_>>();
    let [selected_row] = exact_rows.as_slice() else {
        return Err(rejected(
            "its selected conformance no longer owns one exact realization row",
        ));
    };
    if selected_row.source == ConformanceRowSource::TraitDefault {
        return Err(rejected("trait-default realizations remain unsupported"));
    }

    let concrete = application.lifetime_arguments.is_empty()
        && application.type_arguments.is_empty()
        && application.const_arguments.is_empty()
        && application.machine_arguments.is_empty()
        && application.trait_arguments.is_empty()
        && trait_definition.lifetime_parameters.is_empty()
        && program.trait_type_parameters(trait_definition).is_empty()
        && requirement.lifetime_parameters.is_empty()
        && program
            .state_signature_type_parameters(requirement)
            .is_empty()
        && selected.lifetime_parameters.is_empty()
        && program.conformance_type_parameters(selected).is_empty()
        && realization_machine.lifetime_parameters.is_empty()
        && program
            .machine_type_parameters(realization_machine)
            .is_empty();
    if !concrete {
        return Err(rejected(
            "the trait, requirement, selected conformance, and realization must be concrete and non-generic",
        ));
    }
    if program.machine_states(realization_machine).len() != 1
        || realization_machine.supply_mode != psi_language_semantics::MachineSupplyMode::CheckedBody
        || requirement.return_type.is_valid()
        || realization_state.return_type.is_valid()
    {
        return Err(rejected(
            "the requirement and its checked realization must be one-state Unit callables",
        ));
    }

    let contracts = program.state_signature_contracts(requirement);
    if contracts.iter().any(|contract| {
        !matches!(
            contract.kind,
            SignatureContractKind::Requires | SignatureContractKind::Ensures
        )
    }) {
        return Err(rejected(
            "outcome-guarded and crash contract rows remain unsupported",
        ));
    }
    let named_requires = contracts
        .iter()
        .filter(|contract| {
            contract.binding.is_some() && contract.kind == SignatureContractKind::Requires
        })
        .collect::<Vec<_>>();
    let named_ensures = contracts
        .iter()
        .filter(|contract| {
            contract.binding.is_some() && contract.kind == SignatureContractKind::Ensures
        })
        .collect::<Vec<_>>();
    let ([required_input], [required_output]) =
        (named_requires.as_slice(), named_ensures.as_slice())
    else {
        return Err(rejected(
            "the public requirement must own exactly one named requires input and one unconditional named ensures output",
        ));
    };
    if contracts.len() != 2 {
        return Err(rejected(
            "additional unnamed public requires or ensures rows remain unsupported",
        ));
    }
    for contract in [*required_input, *required_output] {
        let [ProofFact::Proposition(proposition)] =
            program.proof_facts.span_or_empty(contract.facts)
        else {
            return Err(rejected(
                "each public named lane must contain one witness-bearing proposition",
            ));
        };
        if !proposition.binder_arguments.is_empty()
            || !program
                .expression_table
                .expression_handles(proposition.arguments)
                .is_empty()
        {
            return Err(rejected(
                "the public witness proposition must be subjectless and non-generic",
            ));
        }
    }

    Ok(Some((
        psi_checked_trees::StaticRequirementDispatchFact {
            application_fingerprint: dispatch.application_fingerprint,
            application_commitment: dispatch.application_commitment,
            declaring_trait: dispatch.declaring_trait,
            requirement: dispatch.requirement,
            realization_machine: dispatch.realization_machine,
            realization_state: dispatch.realization_state,
        },
        requirement,
    )))
}

/// Bind outcome-specific producer guarantees to the one transition arm that
/// tests the saved result of a direct immutable call. Broader value-origin
/// tracing intentionally remains fail-closed for this stage.
pub(crate) fn bind_outcome_specific_arm_facts(
    program: &psi_typed_trees::TypedTrees,
    proof: &mut ProofFacts,
) -> Result<(), Vec<psi_diagnostics::Diagnostic>> {
    use psi_typed_trees::expression::ExpressionNode;
    use psi_typed_trees::statement::{StatementNode, TransitionGuardNode};

    let mut diagnostics = Vec::new();
    let mut arms = psi_arena::Arena::default();

    for caller_machine in program.machines() {
        for caller_state in program.machine_states(caller_machine) {
            let statements = program
                .statement_table
                .statements(caller_state.statement_nodes);
            for (statement_index, statement) in statements.iter().enumerate() {
                let StatementNode::Transition(transition) = statement else {
                    continue;
                };
                let selectors = program
                    .statement_table
                    .outcome_proof_selectors(transition.proof_selectors);
                let TransitionGuardNode::When(guard) = transition.guard else {
                    if !selectors.is_empty() {
                        diagnostics.push(psi_diagnostics::Diagnostic::error(
                            "outcome proof selectors require an exact guarded result-case arm",
                        ));
                    }
                    continue;
                };
                let Some((result_expression, result_case)) =
                    exact_outcome_case_test(program, guard)
                else {
                    if !selectors.is_empty() {
                        diagnostics.push(psi_diagnostics::Diagnostic::error(
                            "outcome proof selectors require one exact nominal result-case test",
                        ));
                    }
                    continue;
                };
                let ExpressionNode::Name(result_path) =
                    program.expression_table.expression(result_expression)
                else {
                    if !selectors.is_empty() {
                        diagnostics.push(psi_diagnostics::Diagnostic::error(
                            "outcome proof selectors require a saved immutable direct-call result",
                        ));
                    }
                    continue;
                };
                let result_symbol = std::iter::once(result_path.head_symbol)
                    .chain(
                        program
                            .expression_table
                            .name_path_member_symbols(result_path.member_symbols)
                            .iter()
                            .copied(),
                    )
                    .chain(std::iter::once(result_path.symbol))
                    .find(|symbol| symbol.is_valid())
                    .unwrap_or_else(SymbolHandle::invalid);
                let result_name = match program
                    .expression_table
                    .name_path_members(result_path.members)
                {
                    [name] => Some(name.as_str()),
                    _ => None,
                };
                let result_calls = statements
                    .iter()
                    .take(statement_index)
                    .enumerate()
                    .filter_map(|(index, statement)| {
                        let StatementNode::LocalData(local) = statement else {
                            return None;
                        };
                        let identity_matches = if result_symbol.is_valid() {
                            local.symbol == result_symbol
                        } else {
                            result_name.is_some_and(|name| local.name.as_str() == name)
                        };
                        if !identity_matches || local.is_mutable {
                            return None;
                        }
                        let ExpressionNode::Call(call) =
                            program.expression_table.expression(local.initial_value)
                        else {
                            return None;
                        };
                        Some((index, call))
                    })
                    .collect::<Vec<_>>();
                let [(result_call_statement_index, result_call)] = result_calls.as_slice() else {
                    if !selectors.is_empty() {
                        diagnostics.push(psi_diagnostics::Diagnostic::error(
                            "outcome proof selectors are currently limited to one unambiguous direct call captured in an immutable local",
                        ));
                    }
                    continue;
                };
                let Some((target_machine, target_state)) =
                    program.machines().iter().find_map(|machine| {
                        program
                            .machine_states(machine)
                            .iter()
                            .find(|state| state.symbol == result_call.target_symbol)
                            .map(|state| (machine, state))
                    })
                else {
                    if !selectors.is_empty() {
                        diagnostics.push(psi_diagnostics::Diagnostic::error(format!(
                            "outcome proof selector source call `{}` must target a concrete machine state",
                            result_call.target
                        )));
                    }
                    continue;
                };
                let Some(result_data) = program.data_definitions().iter().find_map(|definition| {
                    program.data_members(definition).iter().any(|member| {
                        matches!(member, psi_typed_trees::data::DataMember::Variant(variant) if variant.symbol == result_case)
                    }).then_some(definition.symbol)
                }) else {
                    if !selectors.is_empty() {
                        diagnostics.push(psi_diagnostics::Diagnostic::error(
                            "outcome proof selector case does not resolve to a declared nominal sum case",
                        ));
                    }
                    continue;
                };

                let matching_rows = proof
                    .outcome_specific_guarantees
                    .iter()
                    .filter_map(|(handle, row)| {
                        (row.machine_symbol == target_machine.symbol
                            && row.result_data == result_data
                            && row.result_case == result_case)
                            .then_some((handle, row.clone()))
                    })
                    .collect::<Vec<_>>();
                if matching_rows.is_empty() && selectors.is_empty() {
                    continue;
                }
                let mut selected_names = std::collections::BTreeSet::new();
                let mut invalid = false;
                for selector in selectors {
                    if !selected_names.insert(selector.binding.as_str()) {
                        diagnostics.push(psi_diagnostics::Diagnostic::error(format!(
                            "outcome evidence term `{}` is bound more than once in this arm",
                            selector.binding
                        )));
                        invalid = true;
                    }
                    if !matching_rows.iter().any(|(_, row)| {
                        row.public_selector.as_deref() == Some(selector.output_field.as_str())
                    }) {
                        diagnostics.push(psi_diagnostics::Diagnostic::error(format!(
                            "outcome proof selector `{}` is not a named guarantee of the matching result case",
                            selector.output_field
                        )));
                        invalid = true;
                    }
                }
                if invalid {
                    continue;
                }

                let substitutions = outcome_call_substitutions(
                    program,
                    caller_state.symbol,
                    result_expression,
                    result_call,
                    target_state,
                );
                let mut arm_rows = Vec::with_capacity(matching_rows.len());
                for (guarantee, row) in matching_rows {
                    let (instantiated_proposition, instantiated_identity) =
                        instantiate_outcome_arm_fact(program, row.fact, &substitutions);
                    let referenced_occurrences =
                        crate::contract_occurrences::fact_referenced_occurrences(program, row.fact);
                    let validity = psi_checked_trees::OutcomeSpecificValidityFact {
                        result_occurrence: result_expression,
                        evidence_interface_scope: instantiated_proposition.as_ref().and_then(
                            |proposition| {
                                outcome_evidence_interface_scope(
                                    program,
                                    proposition,
                                    &referenced_occurrences,
                                )
                            },
                        ),
                        referenced_occurrences,
                    };
                    let selected = row.public_selector.as_deref().and_then(|public| {
                        selectors
                            .iter()
                            .find(|selector| selector.output_field.as_str() == public)
                    });
                    let selected_term = selected.and_then(|selector| {
                        let producer = row.evidence_term?;
                        let mut term = proof.evidence_terms.get(producer).clone();
                        let proposition = instantiated_proposition.clone()?;
                        term.name = selector.binding.as_str().to_owned();
                        term.owner = ContractProofFactOwner::MachineState {
                            machine_symbol: caller_machine.symbol,
                            state_symbol: caller_state.symbol,
                        };
                        term.lane_position = arm_rows.len();
                        term.proposition = proposition;
                        Some(proof.evidence_terms.append(term))
                    });
                    arm_rows.push(psi_checked_trees::OutcomeSpecificArmRowFact {
                        guarantee,
                        instantiated_proposition,
                        instantiated_identity,
                        validity,
                        selected_term,
                    });
                }
                arms.append(psi_checked_trees::OutcomeSpecificArmFact {
                    caller_machine_symbol: caller_machine.symbol,
                    caller_state_symbol: caller_state.symbol,
                    statement_index,
                    result_call_statement_index: *result_call_statement_index,
                    result_data,
                    result_case,
                    result_expression,
                    rows: arm_rows,
                });
            }
        }
    }

    if diagnostics.is_empty() {
        proof.outcome_specific_arms = arms;
        Ok(())
    } else {
        Err(diagnostics)
    }
}

fn outcome_evidence_interface_scope(
    program: &psi_typed_trees::TypedTrees,
    proposition: &psi_checked_trees::CheckedPropositionApplication,
    retained_occurrences: &[psi_typed_trees::expression::ExpressionHandle],
) -> Option<psi_checked_trees::OutcomeSpecificEvidenceInterfaceScopeFact> {
    let interface = proposition.evidence_interface.clone()?;
    let definition = program
        .propositions()
        .iter()
        .find(|definition| definition.symbol == proposition.declaration)?;
    let psi_typed_trees::proposition::PropositionBody::Witness { evidence } = definition.body
    else {
        return None;
    };
    let mut reference_regions = Vec::new();
    append_evidence_interface_reference_regions(program, evidence, &mut reference_regions);
    reference_regions.sort_by_key(|reference| reference.arena_index());
    reference_regions.dedup();
    Some(
        psi_checked_trees::OutcomeSpecificEvidenceInterfaceScopeFact {
            interface,
            evidence_type: evidence,
            reference_regions,
            retained_occurrences: retained_occurrences.to_vec(),
        },
    )
}

fn append_evidence_interface_reference_regions(
    program: &psi_typed_trees::TypedTrees,
    type_reference: psi_typed_trees::types::TypeReferenceHandle,
    regions: &mut Vec<psi_typed_trees::types::TypeReferenceHandle>,
) {
    if !type_reference.is_valid() {
        return;
    }
    use psi_typed_trees::types::TypeReferenceNode;
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => {
            regions.push(type_reference);
            append_evidence_interface_reference_regions(program, *referee, regions);
        }
        TypeReferenceNode::Constrained { base_type, .. } => {
            append_evidence_interface_reference_regions(program, *base_type, regions);
        }
        TypeReferenceNode::FixedArray { element_type, .. }
        | TypeReferenceNode::Slice { element_type } => {
            append_evidence_interface_reference_regions(program, *element_type, regions);
        }
        TypeReferenceNode::Generic { arguments, .. } => {
            for argument in program
                .type_reference_table
                .type_reference_handles(*arguments)
            {
                append_evidence_interface_reference_regions(program, *argument, regions);
            }
        }
        TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::Named { .. }
        | TypeReferenceNode::Unit => {}
    }
}

pub(crate) fn exact_outcome_case_test(
    program: &psi_typed_trees::TypedTrees,
    expression: psi_typed_trees::expression::ExpressionHandle,
) -> Option<(psi_typed_trees::expression::ExpressionHandle, SymbolHandle)> {
    use psi_typed_trees::expression::{BinaryOperator, ExpressionNode};
    let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) else {
        return None;
    };
    if binary.operator == BinaryOperator::Equal {
        let is_true = |candidate| {
            matches!(
                program.expression_table.expression(candidate),
                ExpressionNode::Boolean(true)
            )
        };
        if is_true(binary.left) {
            return exact_outcome_case_test(program, binary.right);
        }
        if is_true(binary.right) {
            return exact_outcome_case_test(program, binary.left);
        }
        let case = |candidate| {
            match program.expression_table.expression(candidate) {
            ExpressionNode::Name(path) if program.data_definitions().iter().any(|definition| {
                program.data_members(definition).iter().any(|member| {
                    matches!(member, psi_typed_trees::data::DataMember::Variant(variant) if variant.symbol == path.symbol)
                })
            }) => Some(path.symbol),
            _ => None,
        }
        };
        if let Some(case) = case(binary.right) {
            return Some((binary.left, case));
        }
        if let Some(case) = case(binary.left) {
            return Some((binary.right, case));
        }
    }
    None
}

fn outcome_call_substitutions(
    program: &psi_typed_trees::TypedTrees,
    _caller_state_symbol: SymbolHandle,
    result_expression: psi_typed_trees::expression::ExpressionHandle,
    call: &psi_typed_trees::expression::TableCallExpression,
    target_state: &psi_typed_trees::state::State,
) -> Vec<(SymbolHandle, String, String)> {
    let mut substitutions = Vec::new();
    let arguments = program.expression_table.expression_handles(call.arguments);
    let mut argument_index = 0usize;
    for parameter in program.state_parameters(target_state) {
        let value = if parameter.is_self {
            call.receiver.is_valid().then_some(call.receiver)
        } else {
            let value = arguments.get(argument_index).copied();
            argument_index += 1;
            value
        };
        if let Some(value) = value {
            substitutions.push((
                parameter.symbol,
                parameter.name.as_str().to_owned(),
                program.render_proof_expression_with_parameters(value, &[]),
            ));
        }
    }
    substitutions.push((
        SymbolHandle::invalid(),
        "result".to_owned(),
        program.render_proof_expression_with_parameters(result_expression, &[]),
    ));
    substitutions
}

fn instantiate_outcome_arm_fact(
    program: &psi_typed_trees::TypedTrees,
    fact: psi_arena::Handle<psi_typed_trees::domain::ProofFact>,
    substitutions: &[(SymbolHandle, String, String)],
) -> (
    Option<psi_checked_trees::CheckedPropositionApplication>,
    Option<String>,
) {
    let psi_typed_trees::domain::ProofFact::Proposition(application) =
        program.proof_facts.get(fact)
    else {
        let identity = match program.proof_facts.get(fact) {
            psi_typed_trees::domain::ProofFact::Expression(expression) => {
                Some(program.render_proof_expression_with_parameters(*expression, substitutions))
            }
            psi_typed_trees::domain::ProofFact::Membership(_) => None,
            psi_typed_trees::domain::ProofFact::Proposition(_) => unreachable!(),
        };
        return (None, identity);
    };
    let binder_labels = application
        .binder_arguments
        .iter()
        .map(|argument| {
            substitutions
                .iter()
                .find(|(symbol, _, _)| *symbol == argument.symbol)
                .map(|(_, _, replacement)| replacement.clone())
                .unwrap_or_else(|| argument.display_name())
        })
        .collect::<Vec<_>>();
    let argument_labels = program
        .expression_table
        .expression_handles(application.arguments)
        .iter()
        .map(|argument| program.render_proof_expression_with_parameters(*argument, substitutions))
        .collect::<Vec<_>>();
    let proposition = program
        .normalize_nominal_proposition_application_with_labels(
            application,
            &binder_labels,
            &argument_labels,
        )
        .map(lower_checked_proposition_application);
    let identity = program
        .normalize_proposition_application_with_labels(
            application,
            &binder_labels,
            &argument_labels,
        )
        .map(|formula| formula.identity_label());
    (proposition, identity)
}

fn proof_output_source_term_by_name(
    proof: &ProofFacts,
    invocations: &psi_arena::Arena<psi_checked_trees::ProofOutputCallFact>,
    package: &psi_typed_trees::typed_trees::ProofOutputCall,
    name: &str,
) -> Option<psi_arena::Handle<CheckedEvidenceTerm>> {
    proof
        .evidence_terms
        .iter()
        .find_map(|(handle, term)| {
            let owner_matches = term.owner
                == (ContractProofFactOwner::Machine {
                    machine_symbol: package.machine_symbol,
                })
                || term.owner
                    == (ContractProofFactOwner::MachineState {
                        machine_symbol: package.machine_symbol,
                        state_symbol: package.state_symbol,
                    });
            (owner_matches && term.kind == ContractProofFactKind::Requires && term.name == name)
                .then_some(handle)
        })
        .or_else(|| {
            invocations
                .iter()
                .filter_map(|(_, invocation)| {
                    (invocation.caller_machine_symbol == package.machine_symbol
                        && invocation.caller_state_symbol == package.state_symbol
                        && invocation.source_statement_index < package.source_statement_index)
                        .then_some(invocation.outputs.iter().filter_map(|output| output.output))
                })
                .flatten()
                .find(|term| proof.evidence_terms.get(*term).name == name)
        })
}

fn instantiate_proof_output_proposition(
    program: &psi_typed_trees::TypedTrees,
    proof: &ProofFacts,
    package: &psi_typed_trees::typed_trees::ProofOutputCall,
    call: &psi_typed_trees::expression::TableCallExpression,
    target_parameters: &[psi_typed_trees::signature::StateParameter],
    term: psi_arena::Handle<CheckedEvidenceTerm>,
) -> Option<(psi_checked_trees::CheckedPropositionApplication, String)> {
    let contract = proof
        .contract_facts
        .iter()
        .map(|(_, contract)| contract)
        .find(|contract| contract.evidence_term == Some(term))?;
    let psi_typed_trees::domain::ProofFact::Proposition(application) =
        program.proof_facts.get(contract.fact)
    else {
        return None;
    };
    let call_site = crate::CallSite::Expression {
        expression: package.call,
        call,
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
                package.state_symbol,
                package.statement_index,
                &call_site,
                target_parameters,
                *argument,
            )
        })
        .collect::<Vec<_>>();
    let normalized = program.normalize_nominal_proposition_application_with_labels(
        application,
        &binder_labels,
        &argument_labels,
    )?;
    let identity = program
        .normalize_proposition_application_with_labels(
            application,
            &binder_labels,
            &argument_labels,
        )?
        .identity_label();
    Some((lower_checked_proposition_application(normalized), identity))
}

fn append_proposition_application_if_missing(
    proof: &mut ProofFacts,
    application: &psi_checked_trees::CheckedPropositionApplication,
) {
    if !proof
        .proposition_vocabulary
        .applications
        .contains(application)
    {
        proof
            .proposition_vocabulary
            .applications
            .push(application.clone());
    }
}

pub(crate) fn bind_evidence_projection_facts(
    program: &psi_typed_trees::TypedTrees,
    proof: &mut ProofFacts,
) -> Result<(), Vec<psi_diagnostics::Diagnostic>> {
    use psi_checked_trees::CheckedEvidenceProjection;
    use psi_typed_trees::domain::ProofFact;

    let mut diagnostics = Vec::new();
    let mut applications = Vec::new();
    for (fact_handle, fact) in program.proof_facts.iter() {
        let ProofFact::Proposition(application) = fact else {
            continue;
        };
        let Some(normalized) = program.normalize_nominal_proposition_application(application)
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
            diagnostics.push(psi_diagnostics::Diagnostic::error(format!(
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
                diagnostics.push(psi_diagnostics::Diagnostic::error(format!(
                    "carrierless evidence projection `{}.{}` names no retained evidence term in this contract scope",
                    projection.term, projection.member
                )));
                continue;
            };
            let term_definition = proof.evidence_terms.get(term);
            let Some(interface) = &term_definition.evidence_interface else {
                diagnostics.push(psi_diagnostics::Diagnostic::error(format!(
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
                diagnostics.push(psi_diagnostics::Diagnostic::error(format!(
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
                diagnostics.push(psi_diagnostics::Diagnostic::error(format!(
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
    terms: &psi_arena::Arena<CheckedEvidenceTerm>,
    owner: ContractProofFactOwner,
    name: &str,
) -> Option<psi_arena::Handle<CheckedEvidenceTerm>> {
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
    program: &psi_typed_trees::TypedTrees,
    output: &CheckedEvidenceTerm,
    conformance_symbol: SymbolHandle,
    source_name: &str,
) -> Option<psi_checked_trees::EvidenceAssignmentSource> {
    use psi_typed_trees::trait_definition::{
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
            Some(psi_checked_trees::DynamicConformanceRowFact {
                declaring_trait: row.declaring_trait,
                requirement: row.requirement,
                requirement_identity,
                realization_machine: row.realization_machine,
                realization_state: row.realization_state,
                realization_identity,
                source: match row.source {
                    ConformanceRowSource::Inline => {
                        psi_checked_trees::DynamicConformanceRowSource::Inline
                    }
                    ConformanceRowSource::Reference => {
                        psi_checked_trees::DynamicConformanceRowSource::Reference
                    }
                    ConformanceRowSource::TraitDefault => {
                        psi_checked_trees::DynamicConformanceRowSource::TraitDefault
                    }
                },
            })
        })
        .collect::<Option<Vec<_>>>()?;

    Some(
        psi_checked_trees::EvidenceAssignmentSource::ProducerConformance {
            conformance: conformance.symbol,
            evidence_trait: evidence_trait.symbol,
            rows,
        },
    )
}

fn validate_evidence_forwarding_definite_assignment(
    program: &psi_typed_trees::TypedTrees,
    proof: &ProofFacts,
) -> Result<(), Vec<psi_diagnostics::Diagnostic>> {
    use psi_typed_trees::statement::{StatementNode, TransitionExit, TransitionTargetNode};
    use std::collections::{BTreeMap, BTreeSet, VecDeque};

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
            psi_typed_trees::signature::SignatureContractKind::Requires,
            &[],
        );
        let mut work = VecDeque::from([(
            entry.symbol,
            BTreeMap::<u32, Option<psi_arena::Handle<CheckedEvidenceTerm>>>::new(),
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
                psi_typed_trees::signature::SignatureContractKind::Requires,
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
                        psi_checked_trees::EvidenceAssignmentSource::Forwarded { term } => {
                            Some(*term)
                        }
                        psi_checked_trees::EvidenceAssignmentSource::ProducerConformance {
                            ..
                        } => None,
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
            .map(psi_diagnostics::Diagnostic::error)
            .collect())
    }
}

fn append_missing_evidence_diagnostics(
    program: &psi_typed_trees::TypedTrees,
    proof: &ProofFacts,
    machine: &psi_typed_trees::machine::Machine,
    state: &psi_typed_trees::state::State,
    outputs: &[psi_arena::Handle<CheckedEvidenceTerm>],
    assigned: &std::collections::BTreeMap<u32, Option<psi_arena::Handle<CheckedEvidenceTerm>>>,
    result: Option<psi_typed_trees::expression::ExpressionHandle>,
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

fn exact_result_case(
    program: &psi_typed_trees::TypedTrees,
    result: psi_typed_trees::expression::ExpressionHandle,
) -> Option<(SymbolHandle, SymbolHandle)> {
    use psi_typed_trees::expression::ExpressionNode;
    let (data_symbol, case_symbol) = match program.expression_table.expression(result) {
        ExpressionNode::Name(path) if path.head_symbol.is_valid() && path.symbol.is_valid() => {
            (path.head_symbol, path.symbol)
        }
        ExpressionNode::StructLiteral(literal) => (literal.type_symbol, literal.case_symbol?),
        _ => return None,
    };
    let data = program
        .data_definitions()
        .iter()
        .find(|data| data.symbol == data_symbol)?;
    program
        .data_members(data)
        .iter()
        .any(|member| {
            matches!(
                member,
                psi_typed_trees::data::DataMember::Variant(variant)
                    if variant.symbol == case_symbol
            )
        })
        .then_some((data_symbol, case_symbol))
}

fn outcome_specific_fact_is_proved(
    program: &psi_typed_trees::TypedTrees,
    fact: psi_arena::Handle<psi_typed_trees::domain::ProofFact>,
    result: Option<psi_typed_trees::expression::ExpressionHandle>,
    known: &std::collections::BTreeSet<String>,
) -> bool {
    use psi_typed_trees::domain::ProofFact;
    use psi_typed_trees::expression::ExpressionNode;

    let Some(result) = result else {
        return false;
    };
    let result_label = program.render_proof_expression_with_parameters(result, &[]);
    let substitutions = [(SymbolHandle::invalid(), "result".to_owned(), result_label)];
    match program.proof_facts.get(fact) {
        ProofFact::Expression(expression) => {
            matches!(
                program.expression_table.expression(*expression),
                ExpressionNode::Boolean(true)
            ) || known.contains(&format!(
                "boolean:{}",
                program.render_proof_expression_with_parameters(*expression, &substitutions)
            ))
        }
        ProofFact::Proposition(application) => {
            proposition_application_label(program, application, &substitutions)
                .is_some_and(|goal| known.contains(&goal))
        }
        ProofFact::Membership(_) => false,
    }
}

fn outcome_specific_assignment_matches_result(
    program: &psi_typed_trees::TypedTrees,
    proof: &ProofFacts,
    fact: psi_arena::Handle<psi_typed_trees::domain::ProofFact>,
    result: Option<psi_typed_trees::expression::ExpressionHandle>,
    source: psi_arena::Handle<CheckedEvidenceTerm>,
) -> bool {
    let Some(result) = result else {
        return false;
    };
    let psi_typed_trees::domain::ProofFact::Proposition(application) =
        program.proof_facts.get(fact)
    else {
        return false;
    };
    let result_label = program.render_proof_expression_with_parameters(result, &[]);
    let substitutions = [(SymbolHandle::invalid(), "result".to_owned(), result_label)];
    let binder_labels = application
        .binder_arguments
        .iter()
        .map(|argument| {
            substitutions
                .iter()
                .find(|(symbol, _, _)| *symbol == argument.symbol)
                .map(|(_, _, replacement)| replacement.clone())
                .unwrap_or_else(|| argument.display_name())
        })
        .collect::<Vec<_>>();
    let argument_labels = program
        .expression_table
        .expression_handles(application.arguments)
        .iter()
        .map(|argument| program.render_proof_expression_with_parameters(*argument, &substitutions))
        .collect::<Vec<_>>();
    program
        .normalize_nominal_proposition_application_with_labels(
            application,
            &binder_labels,
            &argument_labels,
        )
        .map(lower_checked_proposition_application)
        .is_some_and(|expected| proof.evidence_terms.get(source).proposition == expected)
}

fn contract_proposition_labels(
    program: &psi_typed_trees::TypedTrees,
    contracts: &[psi_typed_trees::signature::SignatureContract],
    kind: psi_typed_trees::signature::SignatureContractKind,
    substitutions: &[(SymbolHandle, String, String)],
) -> std::collections::BTreeSet<String> {
    use psi_typed_trees::domain::ProofFact;

    contracts
        .iter()
        .filter(|contract| contract.kind == kind)
        .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts))
        .filter_map(|fact| match fact {
            ProofFact::Expression(expression) => Some(format!(
                "boolean:{}",
                program.render_proof_expression_with_parameters(*expression, substitutions)
            )),
            ProofFact::Proposition(application) => {
                proposition_application_label(program, application, substitutions)
            }
            ProofFact::Membership(_) => None,
        })
        .collect()
}

fn proposition_application_label(
    program: &psi_typed_trees::TypedTrees,
    application: &psi_typed_trees::proposition::PropositionApplication,
    substitutions: &[(SymbolHandle, String, String)],
) -> Option<String> {
    let binder_labels = application
        .binder_arguments
        .iter()
        .map(|argument| {
            substitutions
                .iter()
                .find(|(symbol, _, _)| *symbol == argument.symbol)
                .map(|(_, _, replacement)| replacement.clone())
                .unwrap_or_else(|| argument.display_name())
        })
        .collect::<Vec<_>>();
    let argument_labels = program
        .expression_table
        .expression_handles(application.arguments)
        .iter()
        .map(|argument| program.render_proof_expression_with_parameters(*argument, substitutions))
        .collect::<Vec<_>>();
    program
        .normalize_proposition_application_with_labels(
            application,
            &binder_labels,
            &argument_labels,
        )
        .map(|formula| formula.identity_label())
}

fn intake_checked_proof_output_propositions(
    proof: &ProofFacts,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    known: &mut std::collections::BTreeSet<String>,
) {
    for (_, invocation) in proof.proof_output_calls.iter() {
        if invocation.caller_machine_symbol == machine_symbol
            && invocation.caller_state_symbol == state_symbol
            && invocation.statement_index == statement_index
        {
            known.extend(
                invocation
                    .outputs
                    .iter()
                    .map(|output| output.instantiated_identity.clone()),
            );
        }
    }
}

fn intake_call_ensures_propositions(
    program: &psi_typed_trees::TypedTrees,
    call: &psi_typed_trees::statement::TableCall,
    known: &mut std::collections::BTreeSet<String>,
) {
    let Some((callee, state)) = program.machines().iter().find_map(|machine| {
        if machine.symbol == call.target_symbol {
            return program
                .machine_states(machine)
                .first()
                .map(|state| (machine, state));
        }
        program
            .machine_states(machine)
            .iter()
            .find(|state| state.symbol == call.target_symbol)
            .map(|state| (machine, state))
    }) else {
        return;
    };
    let parameters = program.state_parameters(state);
    let arguments = program.statement_table.expression_handles(call.arguments);
    let receiver = psi_typed_trees::expression::display_name_path(
        program.statement_table.name_path_members(call.receiver),
        "::",
    );
    let mut argument_index = 0usize;
    let substitutions = parameters
        .iter()
        .map(|parameter| {
            let replacement = if parameter.is_self {
                receiver.clone()
            } else {
                let label = arguments
                    .get(argument_index)
                    .map(|argument| program.render_proof_expression_with_parameters(*argument, &[]))
                    .unwrap_or_else(|| parameter.name.as_str().to_owned());
                argument_index = argument_index.saturating_add(1);
                label
            };
            (
                parameter.symbol,
                parameter.name.as_str().to_owned(),
                replacement,
            )
        })
        .collect::<Vec<_>>();
    known.extend(contract_proposition_labels(
        program,
        program.machine_contracts(callee),
        psi_typed_trees::signature::SignatureContractKind::Ensures,
        &substitutions,
    ));
    known.extend(contract_proposition_labels(
        program,
        program.state_contracts(state),
        psi_typed_trees::signature::SignatureContractKind::Ensures,
        &substitutions,
    ));
}

fn evidence_term_named(
    terms: &psi_arena::Arena<CheckedEvidenceTerm>,
    machine_symbol: SymbolHandle,
    name: &str,
    kind: ContractProofFactKind,
) -> Option<psi_arena::Handle<CheckedEvidenceTerm>> {
    terms.iter().find_map(|(handle, term)| {
        (term.owner == ContractProofFactOwner::Machine { machine_symbol }
            && term.kind == kind
            && term.name == name)
            .then_some(handle)
    })
}

fn build_checked_proposition_vocabulary(
    program: &psi_typed_trees::TypedTrees,
) -> psi_checked_trees::CheckedPropositionVocabulary {
    let declarations = program
        .propositions()
        .iter()
        .filter_map(|declaration| {
            let evidence = match declaration.body {
                psi_typed_trees::proposition::PropositionBody::Primitive => {
                    psi_checked_trees::CheckedPropositionEvidence::FactOnly
                }
                psi_typed_trees::proposition::PropositionBody::Witness { evidence } => {
                    psi_checked_trees::CheckedPropositionEvidence::Witness {
                        evidence_type: program.display_type_reference(evidence),
                    }
                }
                psi_typed_trees::proposition::PropositionBody::Transparent { .. } => return None,
            };
            let binders = program
                .proposition_binders(declaration)
                .iter()
                .map(|binder| psi_checked_trees::CheckedPropositionBinder {
                    name: binder.name.as_str().to_owned(),
                    kind: match binder.kind {
                        psi_typed_trees::proposition::PropositionBinderKind::Type => {
                            psi_checked_trees::CheckedPropositionBinderKind::Type
                        }
                        psi_typed_trees::proposition::PropositionBinderKind::Const {
                            type_reference,
                        } => psi_checked_trees::CheckedPropositionBinderKind::Const {
                            type_identity: program.display_type_reference(type_reference),
                        },
                        psi_typed_trees::proposition::PropositionBinderKind::Machine => {
                            psi_checked_trees::CheckedPropositionBinderKind::Machine
                        }
                    },
                })
                .collect();
            let parameter_types = program
                .proposition_parameters(declaration)
                .iter()
                .map(|parameter| program.display_type_reference(parameter.type_reference))
                .collect();
            Some(psi_checked_trees::CheckedPropositionDeclaration {
                symbol: declaration.symbol,
                name: declaration.name.as_str().to_owned(),
                is_public: declaration.is_public,
                binders,
                parameter_types,
                evidence,
            })
        })
        .collect();
    let applications = program
        .proof_facts
        .iter()
        .filter_map(|(_, fact)| {
            let psi_typed_trees::domain::ProofFact::Proposition(application) = fact else {
                return None;
            };
            let normalized = program.normalize_nominal_proposition_application(application)?;
            Some(lower_checked_proposition_application(normalized))
        })
        .collect();
    psi_checked_trees::CheckedPropositionVocabulary {
        declarations,
        applications,
    }
}

fn lower_checked_proposition_application(
    normalized: psi_typed_trees::proposition::NormalizedPropositionApplicationIdentity,
) -> psi_checked_trees::CheckedPropositionApplication {
    let evidence_interface = match &normalized.classification {
        psi_typed_trees::proposition::PropositionEvidenceClassification::FactOnly => None,
        psi_typed_trees::proposition::PropositionEvidenceClassification::Witness {
            interface,
            ..
        } => interface.as_ref().map(lower_checked_evidence_interface),
    };
    psi_checked_trees::CheckedPropositionApplication {
        declaration: normalized.declaration,
        binder_arguments: normalized
            .binder_arguments
            .into_iter()
            .map(
                |argument| psi_checked_trees::CheckedPropositionBinderArgument {
                    kind: match argument.kind {
                        psi_typed_trees::proposition::PropositionBinderArgumentKind::Type => {
                            psi_checked_trees::CheckedPropositionBinderArgumentKind::Type
                        }
                        psi_typed_trees::proposition::PropositionBinderArgumentKind::Const => {
                            psi_checked_trees::CheckedPropositionBinderArgumentKind::Const
                        }
                        psi_typed_trees::proposition::PropositionBinderArgumentKind::Machine => {
                            psi_checked_trees::CheckedPropositionBinderArgumentKind::Machine
                        }
                    },
                    identity: argument.identity,
                    evidence_projection: None,
                },
            )
            .collect(),
        arguments: normalized.arguments,
        evidence_interface,
    }
}

fn lower_checked_evidence_interface(
    interface: &psi_typed_trees::proposition::NormalizedEvidenceInterfaceIdentity,
) -> psi_checked_trees::CheckedEvidenceInterfaceIdentity {
    psi_checked_trees::CheckedEvidenceInterfaceIdentity {
        trait_symbol: interface.trait_symbol,
        arguments: interface
            .arguments
            .iter()
            .map(|argument| argument.as_str().to_owned())
            .collect(),
        requirements: interface
            .requirements
            .iter()
            .map(
                |requirement| psi_checked_trees::CheckedEvidenceRequirementIdentity {
                    declaring_trait: requirement.declaring_trait,
                    declaring_trait_arguments: requirement.declaring_trait_arguments.clone(),
                    requirement: requirement.requirement,
                },
            )
            .collect(),
    }
}

fn fact_handles(
    facts: HandleSpan<psi_typed_trees::domain::ProofFact>,
) -> impl Iterator<Item = Handle<psi_typed_trees::domain::ProofFact>> {
    (0..facts.count()).map(move |offset| {
        Handle::from_parts(
            facts
                .start()
                .arena_index()
                .checked_add(offset)
                .expect("proof fact handle index overflow"),
            facts.start().generation(),
        )
    })
}

fn contract_fact_kind(
    kind: &psi_typed_trees::signature::SignatureContractKind,
) -> Option<ContractProofFactKind> {
    match kind {
        psi_typed_trees::signature::SignatureContractKind::Requires => {
            Some(ContractProofFactKind::Requires)
        }
        psi_typed_trees::signature::SignatureContractKind::Ensures => {
            Some(ContractProofFactKind::Ensures)
        }
        psi_typed_trees::signature::SignatureContractKind::EnsuresForResultCase { .. } => None,
        psi_typed_trees::signature::SignatureContractKind::Crashes { .. } => None,
    }
}

pub(crate) use contracts::contract_target_from_state_symbol;

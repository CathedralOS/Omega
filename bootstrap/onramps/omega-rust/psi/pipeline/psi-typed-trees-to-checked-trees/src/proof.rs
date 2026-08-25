use crate::context::*;
use psi_checked_trees::CheckedEvidenceTerm;
mod contracts;
mod float_meaning;
mod obligations;

pub(crate) use contracts::machine_parameter_evidence_signatures;
use contracts::{
    append_inherited_trait_contract_facts, append_machine_contract_facts,
    append_state_contract_facts, append_state_signature_contract_facts, build_contract_call_facts,
    build_contract_exit_facts, build_contract_operator_use_facts, estimated_contract_fact_capacity,
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
    let mut evidence_terms = psi_arena::Arena::default();

    for (_, obligation) in proof_plan.obligations.iter() {
        obligations.append(lower_proof_obligation(obligation));
    }

    for machine in program.machines() {
        append_machine_contract_facts(program, machine, &mut contract_facts, &mut evidence_terms);
        for state in program.machine_states(machine) {
            append_state_contract_facts(
                program,
                machine,
                state,
                &mut contract_facts,
                &mut evidence_terms,
            );
        }
        append_inherited_trait_contract_facts(program, machine, &mut contract_facts);
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
        let source = if let Some(source) = source {
            let source_term = proof.evidence_terms.get(source);
            if output_term.proposition != source_term.proposition
                || output_term.evidence_interface != source_term.evidence_interface
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

        let mut callee_inputs = proof
            .evidence_terms
            .iter()
            .filter_map(|(handle, term)| {
                (term.kind == ContractProofFactKind::Requires
                    && (term.owner
                        == ContractProofFactOwner::Machine {
                            machine_symbol: target_machine.symbol,
                        }
                        || term.owner
                            == (ContractProofFactOwner::MachineState {
                                machine_symbol: target_machine.symbol,
                                state_symbol: target_state.symbol,
                            })))
                .then_some(handle)
            })
            .collect::<Vec<_>>();
        callee_inputs.sort_by_key(|handle| proof.evidence_terms.get(*handle).lane_position);
        let mut callee_outputs = proof
            .evidence_terms
            .iter()
            .filter_map(|(handle, term)| {
                (term.kind == ContractProofFactKind::Ensures
                    && term.owner
                        == (ContractProofFactOwner::Machine {
                            machine_symbol: target_machine.symbol,
                        }))
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
                    target_state,
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
                    target_state,
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
    target_state: &psi_typed_trees::state::State,
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
                target_state,
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

    Some(
        psi_checked_trees::EvidenceAssignmentSource::ProducerConformance {
            conformance: conformance.symbol,
            evidence_trait: evidence_trait.symbol,
            rows: rows
                .iter()
                .map(|row| psi_checked_trees::DynamicConformanceRowFact {
                    declaring_trait: row.declaring_trait,
                    requirement: row.requirement,
                    realization_machine: row.realization_machine,
                    realization_state: row.realization_state,
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
                .collect(),
        },
    )
}

fn validate_evidence_forwarding_definite_assignment(
    program: &psi_typed_trees::TypedTrees,
    proof: &ProofFacts,
) -> Result<(), Vec<psi_diagnostics::Diagnostic>> {
    use psi_typed_trees::statement::{StatementNode, TransitionExit, TransitionTargetNode};
    use std::collections::{BTreeSet, VecDeque};

    let mut diagnostic_messages = BTreeSet::new();

    for machine in program.machines() {
        let outputs = proof
            .evidence_terms
            .iter()
            .filter_map(|(handle, term)| {
                (term.owner
                    == (ContractProofFactOwner::Machine {
                        machine_symbol: machine.symbol,
                    })
                    && term.kind == ContractProofFactKind::Ensures)
                    .then_some(handle)
            })
            .collect::<Vec<_>>();
        if outputs.is_empty() {
            continue;
        }
        let states = program.machine_states(machine);
        let Some(entry) = states.first() else {
            continue;
        };
        let mut work = VecDeque::from([(entry.symbol, BTreeSet::<u32>::new())]);
        let mut seen = BTreeSet::new();

        while let Some((state_symbol, mut assigned)) = work.pop_front() {
            let key = (
                state_symbol.arena_index(),
                assigned.iter().copied().collect::<Vec<_>>(),
            );
            if !seen.insert(key) {
                continue;
            }
            let Some(state) = states.iter().find(|state| state.symbol == state_symbol) else {
                continue;
            };

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
                for forwarding in assignments
                    .iter()
                    .filter(|forwarding| forwarding.statement_index == statement_index)
                {
                    if !assigned.insert(forwarding.output.arena_index()) {
                        let term = proof.evidence_terms.get(forwarding.output);
                        diagnostic_messages.insert(format!(
                            "named ensures evidence `{}` is assigned more than once on a reachable path through {}::{}",
                            term.name, machine.name, state.name
                        ));
                    }
                }

                let Some(StatementNode::Transition(transition)) = statements.get(statement_index)
                else {
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
                                    work.push_back((target, assigned.clone()));
                                } else {
                                    append_missing_evidence_diagnostics(
                                        proof,
                                        machine,
                                        state,
                                        &outputs,
                                        &assigned,
                                        &mut diagnostic_messages,
                                    );
                                }
                            }
                            TransitionTargetNode::SelfTarget => {
                                work.push_back((entry.symbol, assigned.clone()));
                            }
                            TransitionTargetNode::Value(_) | TransitionTargetNode::Terminal => {
                                append_missing_evidence_diagnostics(
                                    proof,
                                    machine,
                                    state,
                                    &outputs,
                                    &assigned,
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
                    proof,
                    machine,
                    state,
                    &outputs,
                    &assigned,
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
    proof: &ProofFacts,
    machine: &psi_typed_trees::machine::Machine,
    state: &psi_typed_trees::state::State,
    outputs: &[psi_arena::Handle<CheckedEvidenceTerm>],
    assigned: &std::collections::BTreeSet<u32>,
    messages: &mut std::collections::BTreeSet<String>,
) {
    for output in outputs {
        if !assigned.contains(&output.arena_index()) {
            messages.insert(format!(
                "named ensures evidence `{}` is not definitely assigned on the ordinary exit through {}::{}",
                proof.evidence_terms.get(*output).name,
                machine.name,
                state.name
            ));
        }
    }
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
        psi_typed_trees::signature::SignatureContractKind::Boundary => {
            Some(ContractProofFactKind::Boundary)
        }
        psi_typed_trees::signature::SignatureContractKind::Crashes { .. } => None,
    }
}

pub(crate) use contracts::contract_target_from_state_symbol;

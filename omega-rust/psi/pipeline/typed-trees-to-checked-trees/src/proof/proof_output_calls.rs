//! Proof-output call facts, open requirement checks and named witness lanes.

use crate::proof::proposition_vocabulary::{
    contract_proposition_labels, lower_checked_proposition_application,
};
use checked_trees::{
    CheckedEvidenceTerm, ContractProofFactKind, ContractProofFactOwner, ProofFacts,
};
use symbols::SymbolHandle;
use typed_trees::proposition::{ProofSubstitutions, PropositionLabels};

pub(crate) fn bind_proof_output_call_facts(
    program: &typed_trees::TypedTrees,
    proof: &mut ProofFacts,
) -> Result<(), Vec<diagnostics::Diagnostic>> {
    use typed_trees::expression::ExpressionNode;

    let mut diagnostics = Vec::new();
    let mut invocations = arena::Arena::default();

    for package in &program.proof_output_calls {
        let ExpressionNode::Call(call) = program.expression_table.expression(package.call) else {
            diagnostics.push(diagnostics::Diagnostic::error(
                "proof-output binding requires a direct call",
            ));
            continue;
        };
        let machine_target =
            crate::semantic_calls::find_state_with_machine(program, call.target_symbol);
        let open_requirement = crate::lookup::machine_by_symbol(program, package.machine_symbol)
            .map(|machine| {
                validation::named_conformance_target_requirement(
                    program,
                    machine,
                    call.target_symbol,
                )
            })
            .transpose();
        let open_requirement = match open_requirement {
            Ok(requirement) => requirement.flatten(),
            Err(error) => {
                diagnostics.push(diagnostics::Diagnostic::error(error));
                continue;
            }
        };
        let static_requirement = if let Some((machine, state)) = machine_target {
            match checked_static_requirement_dispatch(
                program,
                package.machine_symbol,
                call,
                machine,
                state,
            ) {
                Ok(dispatch) => dispatch,
                Err(diagnostic) => {
                    diagnostics.push(diagnostic);
                    continue;
                }
            }
        } else if open_requirement.is_some() && call.static_requirement_dispatch.is_none() {
            None
        } else {
            diagnostics.push(diagnostics::Diagnostic::error(format!("proof-output call `{}` must target a checked machine state or exact generic requirement", call.target)));
            continue;
        };
        if let Some((owner, requirement)) = open_requirement
            && let Err(diagnostic) = check_open_proof_output_requirement(
                program,
                package.machine_symbol,
                call,
                owner,
                requirement,
            )
        {
            diagnostics.push(diagnostic);
            continue;
        }
        // Generic templates use the same checked contract lanes as their
        // closed instances. An evidence call names its public signature;
        // only a closed call carries a concrete realization dispatch row.
        let caller_is_generic = crate::lookup::machine_by_symbol(program, package.machine_symbol)
            .is_some_and(|machine| {
                !machine.lifetime_parameters.is_empty()
                    || !machine.type_parameters.is_empty()
                    || !machine.conformance_bounds.is_empty()
            });
        if !caller_is_generic
            && machine_target.is_some_and(|(machine, _)| {
                !machine.lifetime_parameters.is_empty()
                    || !machine.type_parameters.is_empty()
                    || !machine.conformance_bounds.is_empty()
                    || !call.machine_arguments.is_empty()
            })
        {
            diagnostics.push(diagnostics::Diagnostic::error(format!(
                "proof-output call `{}` from a concrete caller requires one complete concrete application", call.target,
            )));
            continue;
        }
        let (target_machine_symbol, target_state_symbol, return_type, immediate) =
            if let Some((machine, state)) = machine_target {
                (
                    machine.symbol,
                    state.symbol,
                    state.return_type,
                    program.machine_states(machine).len() == 1
                        && machine.supply_mode
                            == language_semantics::MachineSupplyMode::CheckedBody,
                )
            } else if let Some((owner, requirement)) = open_requirement {
                (owner, requirement.symbol, requirement.return_type, true)
            } else {
                continue;
            };
        let runtime_value_type = return_type
            .is_valid()
            .then(|| program.primitive_type_reference(return_type))
            .flatten();
        let proof_only = !return_type.is_valid();
        if !immediate || (!proof_only && runtime_value_type.is_none()) {
            diagnostics.push(diagnostics::Diagnostic::error(format!(
                "proof-output call `{}` is currently limited to a one-state Unit- or scalar-result checked machine or exact generic requirement",
                call.target
            )));
            continue;
        }

        let public_owner = static_requirement
            .as_ref()
            .map(|(_, requirement)| ContractProofFactOwner::StateSignature {
                owner_symbol: call
                    .static_requirement_dispatch
                    .as_ref()
                    .expect("checked static requirement dispatch")
                    .declaring_trait,
                state_symbol: requirement.symbol,
            })
            .or_else(|| {
                open_requirement.map(|(owner_symbol, requirement)| {
                    ContractProofFactOwner::StateSignature {
                        owner_symbol,
                        state_symbol: requirement.symbol,
                    }
                })
            });
        let target_parameters = static_requirement
            .as_ref()
            .map(|(_, requirement)| program.state_signature_parameters(requirement))
            .or_else(|| {
                open_requirement
                    .map(|(_, requirement)| program.state_signature_parameters(requirement))
            })
            .or_else(|| machine_target.map(|(_, state)| program.state_parameters(state)))
            .unwrap_or_default();
        let mut callee_inputs = proof
            .evidence_terms
            .iter()
            .filter_map(|(handle, term)| {
                let owner_matches = public_owner.map_or_else(
                    || {
                        term.owner
                            == ContractProofFactOwner::Machine {
                                machine_symbol: target_machine_symbol,
                            }
                            || term.owner
                                == (ContractProofFactOwner::MachineState {
                                    machine_symbol: target_machine_symbol,
                                    state_symbol: target_state_symbol,
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
                            machine_symbol: target_machine_symbol,
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
            diagnostics.push(diagnostics::Diagnostic::error(format!(
                "proof-output call `{}` requires at least one unconditional named ensures output",
                call.target
            )));
            continue;
        }
        if call.evidence_arguments.len() != callee_inputs.len() {
            diagnostics.push(diagnostics::Diagnostic::error(format!(
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
                diagnostics.push(diagnostics::Diagnostic::error(
                    "proof-output binding cannot discard its runtime Type result",
                ));
                invalid = true;
                continue;
            }
            if fields
                .insert(binding.output_field.as_str(), binding)
                .is_some()
            {
                diagnostics.push(diagnostics::Diagnostic::error(format!(
                    "proof-output selector `{}` is bound more than once",
                    binding.output_field
                )));
                invalid = true;
            }
            if binding.binding.as_str() != "_" && !local_names.insert(binding.binding.as_str()) {
                diagnostics.push(diagnostics::Diagnostic::error(format!(
                    "evidence term `{}` is bound more than once in this package pattern",
                    binding.binding
                )));
                invalid = true;
            }
        }
        let value_binding = fields.get("value").copied();
        match (runtime_value_type, value_binding) {
            (Some(_), None) => {
                diagnostics.push(diagnostics::Diagnostic::error(format!(
                    "proof-output binding from `{}` is missing its runtime Type result",
                    call.target
                )));
                invalid = true;
            }
            (None, Some(_)) => {
                diagnostics.push(diagnostics::Diagnostic::error(format!(
                    "proof-only call `{}` has no runtime Type result to bind",
                    call.target
                )));
                invalid = true;
            }
            _ => {}
        }
        for field in fields.keys() {
            if (*field != "value" || runtime_value_type.is_none())
                && !callee_outputs
                    .iter()
                    .any(|output| proof.evidence_terms.get(*output).name == *field)
            {
                diagnostics.push(diagnostics::Diagnostic::error(format!(
                    "call `{}` publishes no proof-output selector `{field}`",
                    call.target
                )));
                invalid = true;
            }
        }

        let runtime_call = if let Some(statement_index) = package.runtime_call_statement_index {
            let Some(caller_state) =
                crate::semantic_calls::find_state(program, package.state_symbol)
            else {
                diagnostics.push(diagnostics::Diagnostic::error(
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
                        Some(typed_trees::statement::StatementNode::LocalData(local)),
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
                    (None, None, Some(typed_trees::statement::StatementNode::Call(unit_call))) => {
                        unit_call.target_symbol == call.target_symbol
                    }
                    _ => false,
                };
            if !exact_runtime_statement
                || package.statement_index != statement_index.saturating_add(1)
            {
                diagnostics.push(diagnostics::Diagnostic::error(
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
                diagnostics.push(diagnostics::Diagnostic::error(
                    "proof-output call has no exact checked contract-call row",
                ));
                continue;
            };
            if matching_calls.next().is_some()
                || contract_call.target_machine_symbol != target_machine_symbol
                || contract_call.target_state_symbol != target_state_symbol
            {
                diagnostics.push(diagnostics::Diagnostic::error(
                    "proof-output call disagrees with its checked contract-call row",
                ));
                continue;
            }
            Some(checked_trees::ProofOutputRuntimeCallFact {
                statement_index,
                call_ordinal: 0,
            })
        } else {
            if runtime_value_type.is_some() {
                diagnostics.push(diagnostics::Diagnostic::error(format!(
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
                |(_, invocation): (_, &checked_trees::ProofOutputCallFact)| {
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
                diagnostics.push(diagnostics::Diagnostic::error(format!(
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
                diagnostics.push(diagnostics::Diagnostic::error(format!(
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
                    target_parameters,
                    callee_input,
                )
            else {
                diagnostics.push(diagnostics::Diagnostic::error(format!(
                    "proof-output call `{}` cannot instantiate erased requires position {}",
                    call.target, input_position,
                )));
                invalid = true;
                continue;
            };
            if proof.evidence_terms.get(source).proposition != instantiated_proposition {
                diagnostics.push(diagnostics::Diagnostic::error(format!(
                    "evidence term `{}` does not inhabit erased requires position {} of proof-output call `{}`",
                    authored, input_position, call.target,
                )));
                invalid = true;
                continue;
            }
            append_proposition_application_if_missing(proof, &instantiated_proposition);
            evidence_arguments.push(checked_trees::ProofOutputEvidenceArgumentFact {
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
                    target_parameters,
                    callee_output,
                )
            else {
                diagnostics.push(diagnostics::Diagnostic::error(format!(
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
            outputs.push(checked_trees::ProofOutputFact {
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
        invocations.append(checked_trees::ProofOutputCallFact {
            caller_machine_symbol: package.machine_symbol,
            caller_state_symbol: package.state_symbol,
            statement_index: package.statement_index,
            source_statement_index: package.source_statement_index,
            runtime_call,
            target_machine_symbol,
            target_state_symbol,
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

fn check_open_proof_output_requirement(
    program: &typed_trees::TypedTrees,
    caller: SymbolHandle,
    call: &typed_trees::expression::TableCallExpression,
    owner: SymbolHandle,
    requirement: &typed_trees::signature::StateSignature,
) -> Result<(), diagnostics::Diagnostic> {
    let rejected = |reason: &str| {
        diagnostics::Diagnostic::error(format!(
            "static named-witness requirement call `{}` cannot use its generic public contract: {reason}",
            call.target,
        ))
    };
    let Some(definition) = program
        .traits()
        .iter()
        .find(|definition| definition.symbol == owner)
    else {
        return Err(rejected("its declaring trait is absent"));
    };
    if !definition.lifetime_parameters.is_empty()
        || !program.trait_type_parameters(definition).is_empty()
        || !requirement.lifetime_parameters.is_empty()
        || !program
            .state_signature_type_parameters(requirement)
            .is_empty()
    {
        return Err(rejected(
            "the public trait and requirement must be concrete and non-generic",
        ));
    }
    let unit = !requirement.return_type.is_valid();
    let scalar = matches!(
        program.primitive_type_reference(requirement.return_type),
        Some(typed_trees::types::PrimitiveType::I32 | typed_trees::types::PrimitiveType::Bool)
    ) && program.state_signature_parameters(requirement).is_empty()
        && program
            .expression_table
            .expression_handles(call.arguments)
            .is_empty()
        && crate::lookup::machine_by_symbol(program, caller)
            .is_some_and(|machine| machine.attached_data.is_none());
    if !unit && !scalar {
        return Err(rejected(
            "the public requirement must be Unit, or exact i32 or bool with a free caller and zero ordinary arguments",
        ));
    }
    check_public_named_witness_lanes(program, requirement).map_err(rejected)
}

fn checked_static_requirement_dispatch<'program>(
    program: &'program typed_trees::TypedTrees,
    caller_machine: symbols::SymbolHandle,
    call: &typed_trees::expression::TableCallExpression,
    realization_machine: &'program typed_trees::machine::Machine,
    realization_state: &'program typed_trees::state::State,
) -> Result<
    Option<(
        checked_trees::StaticRequirementDispatchFact,
        &'program typed_trees::signature::StateSignature,
    )>,
    diagnostics::Diagnostic,
> {
    let Some(dispatch) = call.static_requirement_dispatch.as_ref() else {
        return Ok(None);
    };
    let rejected = |reason: &str| {
        diagnostics::Diagnostic::error(format!(
            "static named-witness requirement call `{}` is outside the first closed dispatch rung: {reason}",
            call.target,
        ))
    };

    if dispatch.application_report_fingerprint == 0
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
            application.report_fingerprint == dispatch.application_report_fingerprint
                && application.commitment == dispatch.application_commitment
        })
        .collect::<Vec<_>>();
    let [application] = applications.as_slice() else {
        return Err(rejected(
            "its exact owner-scoped closed conformance application is absent or ambiguous",
        ));
    };
    if application.trait_definition != dispatch.declaring_trait {
        let Some(application_trait) = program
            .traits()
            .iter()
            .find(|definition| definition.symbol == application.trait_definition)
        else {
            return Err(rejected("its application trait is absent"));
        };
        if !trait_requires_transitively(program, application_trait, dispatch.declaring_trait) {
            return Err(rejected(
                "its declaring trait is absent from the application trait's requirement closure",
            ));
        }
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
    let [_selected_row] = exact_rows.as_slice() else {
        return Err(rejected(
            "its selected conformance no longer owns one exact realization row",
        ));
    };
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
    let unit_result =
        !requirement.return_type.is_valid() && !realization_state.return_type.is_valid();
    let requirement_result = program.primitive_type_reference(requirement.return_type);
    let realization_result = program.primitive_type_reference(realization_state.return_type);
    let bounded_scalar_result = requirement_result == realization_result
        && matches!(
            requirement_result,
            Some(typed_trees::types::PrimitiveType::I32 | typed_trees::types::PrimitiveType::Bool)
        )
        && program.state_signature_parameters(requirement).is_empty()
        && program.state_parameters(realization_state).is_empty()
        && program
            .expression_table
            .expression_handles(call.arguments)
            .is_empty()
        && !call.receiver.is_valid()
        && crate::lookup::machine_by_symbol(program, caller_machine)
            .is_some_and(|machine| machine.attached_data.is_none());
    if program.machine_states(realization_machine).len() != 1
        || realization_machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
        || !(unit_result || bounded_scalar_result)
    {
        return Err(rejected(
            "the requirement and realization must be one-state Unit callables, or the scalar extension must be exact i32 or bool with a free caller, receiverless requirement and realization, and zero ordinary arguments",
        ));
    }

    check_public_named_witness_lanes(program, requirement).map_err(rejected)?;

    Ok(Some((
        checked_trees::StaticRequirementDispatchFact {
            application_report_fingerprint: dispatch.application_report_fingerprint,
            application_commitment: dispatch.application_commitment,
            declaring_trait: dispatch.declaring_trait,
            requirement: dispatch.requirement,
            realization_machine: dispatch.realization_machine,
            realization_state: dispatch.realization_state,
        },
        requirement,
    )))
}

/// Whether `required_trait` appears in the transitive `requires` closure of
/// `trait_definition`. Inherited rows keep the parent's declaring-trait
/// identity, so a dispatch declared on a parent is admitted through a
/// conformance to its descendant.
fn trait_requires_transitively(
    program: &typed_trees::TypedTrees,
    trait_definition: &typed_trees::trait_definition::TraitDefinition,
    required_trait: SymbolHandle,
) -> bool {
    let mut pending = vec![trait_definition];
    let mut visited = Vec::new();
    while let Some(definition) = pending.pop() {
        if visited.contains(&definition.symbol) {
            continue;
        }
        visited.push(definition.symbol);
        for requirement in program.trait_requirements(definition) {
            if requirement.symbol == required_trait {
                return true;
            }
            if let Some(parent) = program
                .traits()
                .iter()
                .find(|candidate| candidate.symbol == requirement.symbol)
            {
                pending.push(parent);
            }
        }
    }
    false
}

fn check_public_named_witness_lanes(
    program: &typed_trees::TypedTrees,
    requirement: &typed_trees::signature::StateSignature,
) -> Result<(), &'static str> {
    use typed_trees::domain::ProofFact;
    use typed_trees::signature::SignatureContractKind;
    let contracts = program.state_signature_contracts(requirement);
    if contracts.iter().any(|contract| {
        !matches!(
            contract.kind,
            SignatureContractKind::Requires | SignatureContractKind::Ensures
        )
    }) {
        return Err("outcome-guarded and crash contract rows remain unsupported");
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
    if named_ensures.is_empty() {
        return Err(
            "the public requirement must own at least one unconditional named ensures output",
        );
    }
    if contracts.len() != named_requires.len() + named_ensures.len() {
        return Err("every public requires and ensures row must be named");
    }
    for contract in named_requires.into_iter().chain(named_ensures) {
        let [ProofFact::Proposition(proposition)] =
            program.proof_facts.span_or_empty(contract.facts)
        else {
            return Err("each public named lane must contain one witness-bearing proposition");
        };
        if !proposition.binder_arguments.is_empty()
            || !program
                .expression_table
                .expression_handles(proposition.arguments)
                .is_empty()
        {
            return Err("the public witness proposition must be subjectless and non-generic");
        }
    }

    Ok(())
}

fn proof_output_source_term_by_name(
    proof: &ProofFacts,
    invocations: &arena::Arena<checked_trees::ProofOutputCallFact>,
    package: &typed_trees::typed_trees::ProofOutputCall,
    name: &str,
) -> Option<arena::Handle<CheckedEvidenceTerm>> {
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
    program: &typed_trees::TypedTrees,
    proof: &ProofFacts,
    package: &typed_trees::typed_trees::ProofOutputCall,
    call: &typed_trees::expression::TableCallExpression,
    target_parameters: &[typed_trees::signature::StateParameter],
    term: arena::Handle<CheckedEvidenceTerm>,
) -> Option<(checked_trees::CheckedPropositionApplication, String)> {
    let contract = proof
        .contract_facts
        .iter()
        .map(|(_, contract)| contract)
        .find(|contract| contract.evidence_term == Some(term))?;
    let typed_trees::domain::ProofFact::Proposition(application) =
        program.proof_facts.get(contract.fact)
    else {
        return None;
    };
    let call_site = crate::semantic_calls::CallSite::Expression {
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
    let normalized = program.normalize_nominal_proposition_application(
        application,
        Some(PropositionLabels {
            binder_labels: &binder_labels,
            argument_labels: &argument_labels,
        }),
    )?;
    let identity = program
        .normalize_proposition_application(
            application,
            Some(PropositionLabels {
                binder_labels: &binder_labels,
                argument_labels: &argument_labels,
            }),
        )?
        .identity_label();
    Some((lower_checked_proposition_application(normalized), identity))
}

fn append_proposition_application_if_missing(
    proof: &mut ProofFacts,
    application: &checked_trees::CheckedPropositionApplication,
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

pub(crate) fn intake_checked_proof_output_propositions(
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

pub(crate) fn intake_call_ensures_propositions(
    program: &typed_trees::TypedTrees,
    call: &typed_trees::statement::TableCall,
    known: &mut std::collections::BTreeSet<String>,
) {
    let Some((callee, state)) = crate::lookup::machine_by_symbol(program, call.target_symbol)
        .and_then(|machine| {
            program
                .machine_states(machine)
                .first()
                .map(|state| (machine, state))
        })
        .or_else(|| crate::semantic_calls::find_state_with_machine(program, call.target_symbol))
    else {
        return;
    };
    let parameters = program.state_parameters(state);
    let arguments = program.statement_table.expression_handles(call.arguments);
    let receiver = typed_trees::expression::display_name_path(
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
                    .map(|argument| {
                        program.render_proof_expression(*argument, ProofSubstitutions::None)
                    })
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
        typed_trees::signature::SignatureContractKind::Ensures,
        &substitutions,
    ));
    known.extend(contract_proposition_labels(
        program,
        program.state_contracts(state),
        typed_trees::signature::SignatureContractKind::Ensures,
        &substitutions,
    ));
}

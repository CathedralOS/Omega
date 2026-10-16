use std::collections::BTreeSet;
use typed_trees::proposition::ProofSubstitutions;

use diagnostics::Diagnostic;
use typed_trees::domain::ProofFact;
use typed_trees::machine::Machine;
use typed_trees::signature::{SignatureContract, SignatureContractKind, StateParameter};
use typed_trees::statement::TableCall;

pub(crate) fn validate_proposition_entailment(
    program: &dyn crate::CheckingInput,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for machine in program.machines() {
        if machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody {
            continue;
        }
        let machine_requires = proposition_labels(
            program,
            program.machine_contracts(machine),
            SignatureContractKind::Requires,
            &[],
            true,
        );
        let machine_ensures = proposition_labels(
            program,
            program.machine_contracts(machine),
            SignatureContractKind::Ensures,
            &[],
            false,
        );
        // An explicit producer assignment discharges the proposition itself;
        // the checked path-sensitive assignment analysis separately proves
        // that every ordinary outcome constructs each named output exactly
        // once. Keep those two judgments independent here.
        let produced_evidence = produced_evidence_labels(program, machine, diagnostics);
        for state in program.machine_states(machine) {
            let mut known = machine_requires.iter().cloned().collect::<BTreeSet<_>>();
            known.extend(proposition_labels(
                program,
                program.state_contracts(state),
                SignatureContractKind::Requires,
                &[],
                true,
            ));
            let statements = program.statement_table.statements(state.statement_nodes);
            for (statement_index, statement) in statements.iter().enumerate() {
                intake_proof_output_propositions(
                    program,
                    machine,
                    state.symbol,
                    statement_index,
                    &mut known,
                );
                if let Some(call) = statement.as_call() {
                    intake_call_propositions(
                        program,
                        machine,
                        state,
                        call,
                        &mut known,
                        diagnostics,
                    );
                }
            }
            intake_proof_output_propositions(
                program,
                machine,
                state.symbol,
                statements.len(),
                &mut known,
            );

            let mut required = machine_ensures.clone();
            required.extend(proposition_labels(
                program,
                program.state_contracts(state),
                SignatureContractKind::Ensures,
                &[],
                false,
            ));
            let structural_proposition_goals = program
                .machine_contracts(machine)
                .iter()
                .chain(program.state_contracts(state))
                .filter(|contract| contract.kind == SignatureContractKind::Ensures)
                .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts))
                .filter_map(|fact| match fact {
                    ProofFact::Proposition(application) => program
                        .normalize_proposition_application(application, None)
                        .map(|formula| (formula.identity_label(), application)),
                    _ => None,
                })
                .collect::<Vec<_>>();
            for goal in required {
                let structurally_entailed = structural_proposition_goals.iter().any(
                    |(label, application)| {
                        label == &goal
                            && crate::proof_contracts::contract_entailment::transparent_proposition_application_entailed(
                                program,
                                machine,
                                state,
                                application,
                                None,
                            )
                    },
                );
                // Instruction observations require the captured CFG event and
                // operand frontiers. The ordinary checked-flow exit obligation
                // remains mandatory; this logical-source pass cannot discharge
                // or reject that distinct evidence class before flow exists.
                let instruction_flow_obligation =
                    structural_proposition_goals
                        .iter()
                        .any(|(label, application)| {
                            label == &goal
                                && super::instruction_effects::instruction_effect_contract(
                                    program,
                                    application,
                                )
                                .is_some()
                        });
                if !known.contains(&goal)
                    && !produced_evidence.contains(&goal)
                    && !structurally_entailed
                    && !instruction_flow_obligation
                {
                    diagnostics.push(Diagnostic::error(format!(
                        "checked machine `{}` cannot establish proposition ensure `{goal}` in state `{}`; require it, cite a checked/accepted proof that ensures it, or supply its declared evidence",
                        machine.name.as_str(),
                        state.name.as_str(),
                    )));
                }
            }
        }
    }
}

fn intake_proof_output_propositions(
    program: &dyn crate::CheckingInput,
    caller: &Machine,
    state_symbol: symbols::SymbolHandle,
    statement_index: usize,
    known: &mut BTreeSet<String>,
) {
    for package in &program.proof_output_calls {
        if package.machine_symbol != caller.symbol
            || package.state_symbol != state_symbol
            || package.statement_index != statement_index
        {
            continue;
        }
        let typed_trees::expression::ExpressionNode::Call(call) =
            program.expression_table.expression(package.call)
        else {
            continue;
        };
        let Some(callee) = program.machines().iter().find(|candidate| {
            program
                .machine_states(candidate)
                .iter()
                .any(|candidate_state| candidate_state.symbol == call.target_symbol)
        }) else {
            continue;
        };
        known.extend(proposition_labels(
            program,
            program.machine_contracts(callee),
            SignatureContractKind::Ensures,
            &[],
            true,
        ));
    }
}

fn produced_evidence_labels(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<String> {
    let mut labels = Vec::new();
    for assignment in &program.evidence_forwardings {
        if assignment.machine_symbol != machine.symbol {
            continue;
        }
        let Some(conformance) = assignment.source_conformance else {
            continue;
        };
        let Some(contract) = program.machine_contracts(machine).iter().find(|contract| {
            contract.kind == SignatureContractKind::Ensures
                && contract
                    .binding
                    .as_ref()
                    .is_some_and(|binding| binding == &assignment.target)
        }) else {
            continue;
        };
        let Some((expected_interface_label, expected_interface)) = program
            .proof_facts
            .span_or_empty(contract.facts)
            .iter()
            .find_map(|fact| {
                let application = fact.as_proposition()?;
                let normalized =
                    program.normalize_nominal_proposition_application(application, None)?;
                match normalized.classification {
                    typed_trees::proposition::PropositionEvidenceClassification::Witness {
                        evidence,
                        interface,
                    } => Some((evidence, interface)),
                    typed_trees::proposition::PropositionEvidenceClassification::FactOnly => None,
                }
            })
        else {
            continue;
        };
        let Some(expected_interface) = expected_interface else {
            diagnostics.push(Diagnostic::error(format!(
                "named implementation `{}` cannot provide unresolved generic evidence interface `{expected_interface_label}` required by `{}`",
                assignment.source, assignment.target
            )));
            continue;
        };
        if select_named_evidence_implementation(
            program,
            conformance,
            assignment.source.as_str(),
            &expected_interface,
        )
        .is_none()
        {
            diagnostics.push(Diagnostic::error(format!(
                "named implementation `{}` does not provide the exact `{expected_interface_label}` evidence interface required by `{}`",
                assignment.source, assignment.target
            )));
            continue;
        }
        labels.extend(proposition_labels(
            program,
            std::slice::from_ref(contract),
            SignatureContractKind::Ensures,
            &[],
            false,
        ));
    }
    labels
}

pub fn select_named_evidence_implementation<'program>(
    program: &'program dyn crate::CheckingInput,
    conformance_symbol: symbols::SymbolHandle,
    source_name: &str,
    expected_interface: &typed_trees::proposition::NormalizedEvidenceInterfaceIdentity,
) -> Option<(
    &'program typed_trees::trait_definition::Conformance,
    symbols::SymbolHandle,
)> {
    let conformance = program.conformance_by_symbol(conformance_symbol)?;
    if conformance.alias.as_ref()?.as_str() != source_name {
        return None;
    }
    let evidence_trait = program
        .traits()
        .iter()
        .find(|candidate| candidate.name == conformance.trait_name)?;
    let selected_interface = typed_trees::proposition::NormalizedEvidenceInterfaceIdentity {
        trait_symbol: evidence_trait.symbol,
        arguments: program
            .type_reference_table
            .type_reference_handles(conformance.arguments)
            .iter()
            .map(|argument| program.normalized_type_identity(*argument))
            .collect(),
        requirements: expected_interface.requirements.clone(),
    };
    (&selected_interface == expected_interface).then_some((conformance, evidence_trait.symbol))
}

fn intake_call_propositions(
    program: &dyn crate::CheckingInput,
    caller: &Machine,
    caller_state: &typed_trees::state::State,
    call: &TableCall,
    known: &mut BTreeSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some((callee, callee_state)) = call_target(program, call) else {
        return;
    };
    let substitutions =
        call_parameter_substitutions(program, call, program.state_parameters(callee_state));
    let arguments = program.statement_table.expression_handles(call.arguments);
    // A requirement holds when the caller knows its exact label or when the
    // caller's premises prove its unfolded definition under the call's
    // argument substitution.
    let missing = program
        .machine_contracts(callee)
        .iter()
        .filter(|contract| contract.kind == SignatureContractKind::Requires)
        .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts))
        .filter_map(ProofFact::as_proposition)
        .filter_map(|application| {
            Some((
                application,
                application_label(program, application, &substitutions)?,
            ))
        })
        .find(|(application, label)| {
            !known.contains(label)
                && !crate::proof_contracts::contract_entailment::transparent_call_requirement_entailed(
                    program,
                    caller,
                    caller_state,
                    callee,
                    callee_state,
                    arguments,
                    application,
                )
        });
    if let Some((_, missing)) = missing {
        diagnostics.push(Diagnostic::error(format!(
            "checked machine `{}` cannot cite `{}`: proposition requirement `{missing}` is not established at the call",
            caller.name.as_str(),
            callee.name.as_str(),
        )));
        return;
    }
    known.extend(proposition_labels(
        program,
        program.machine_contracts(callee),
        SignatureContractKind::Ensures,
        &substitutions,
        true,
    ));
}

fn call_target<'a>(
    program: &'a dyn crate::CheckingInput,
    call: &TableCall,
) -> Option<(&'a Machine, &'a typed_trees::state::State)> {
    for machine in program.machines() {
        if machine.symbol == call.target_symbol {
            return Some((machine, program.machine_states(machine).first()?));
        }
        if let Some(state) = program.state_in_machine(machine, call.target_symbol) {
            return Some((machine, state));
        }
    }
    None
}

fn call_parameter_substitutions(
    program: &dyn crate::CheckingInput,
    call: &TableCall,
    parameters: &[StateParameter],
) -> Vec<(symbols::SymbolHandle, String, String)> {
    let arguments = program.statement_table.expression_handles(call.arguments);
    let mut argument_index = 0usize;
    parameters
        .iter()
        .map(|parameter| {
            let label = {
                let label = arguments
                    .get(argument_index)
                    .map(|argument| program.expression_table.display_name(*argument))
                    .unwrap_or_else(|| parameter.name.as_str().to_owned());
                argument_index = argument_index.saturating_add(1);
                label
            };
            (parameter.symbol, parameter.name.as_str().to_owned(), label)
        })
        .collect()
}

fn proposition_labels(
    program: &dyn crate::CheckingInput,
    contracts: &[SignatureContract],
    kind: SignatureContractKind,
    substitutions: &[(symbols::SymbolHandle, String, String)],
    include_boolean_expressions: bool,
) -> Vec<String> {
    contracts
        .iter()
        .filter(|contract| contract.kind == kind)
        .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts))
        .filter_map(|fact| {
            let Some(application) = fact.as_proposition() else {
                return match fact {
                    ProofFact::Expression(expression) if include_boolean_expressions => {
                        Some(format!(
                            "boolean:{}",
                            program.render_proof_expression(
                                *expression,
                                ProofSubstitutions::ByParameter(substitutions)
                            )
                        ))
                    }
                    ProofFact::Expression(_)
                    | ProofFact::Membership(_)
                    | ProofFact::Proposition(_) => None,
                };
            };
            application_label(program, application, substitutions)
        })
        .collect()
}

/// The normalized identity label of one proposition application with the
/// given parameter substitution applied to its binders and arguments.
fn application_label(
    program: &dyn crate::CheckingInput,
    application: &typed_trees::proposition::PropositionApplication,
    substitutions: &[(symbols::SymbolHandle, String, String)],
) -> Option<String> {
    let binder_labels = application
        .binder_arguments
        .iter()
        .map(|argument| {
            substitutions
                .iter()
                .find(|(symbol, _, _)| *symbol == argument.symbol)
                .map(|(_, _, label)| label.clone())
                .unwrap_or_else(|| argument.display_name())
        })
        .collect::<Vec<_>>();
    let argument_labels = program
        .expression_table
        .expression_handles(application.arguments)
        .iter()
        .map(|argument| {
            program
                .render_proof_expression(*argument, ProofSubstitutions::ByParameter(substitutions))
        })
        .collect::<Vec<_>>();
    program.normalized_identity_label(application, &binder_labels, &argument_labels)
}

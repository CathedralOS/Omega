use std::collections::BTreeSet;

use psi_diagnostics::Diagnostic;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::domain::ProofFact;
use psi_typed_trees::machine::Machine;
use psi_typed_trees::signature::{SignatureContract, SignatureContractKind, StateParameter};
use psi_typed_trees::statement::{StatementNode, TableCall};

pub(crate) fn validate_proposition_entailment(
    program: &TypedTrees,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for machine in program.machines() {
        if machine.supply_mode != psi_language_semantics::MachineSupplyMode::CheckedBody {
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
                if let StatementNode::Call(call) = statement {
                    intake_call_propositions(program, machine, call, &mut known, diagnostics);
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
            for goal in required {
                if !known.contains(&goal) && !produced_evidence.contains(&goal) {
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
    program: &TypedTrees,
    caller: &Machine,
    state_symbol: psi_symbols::SymbolHandle,
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
        let psi_typed_trees::expression::ExpressionNode::Call(call) =
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
    program: &TypedTrees,
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
                let ProofFact::Proposition(application) = fact else {
                    return None;
                };
                let normalized = program.normalize_nominal_proposition_application(application)?;
                match normalized.classification {
                    psi_typed_trees::proposition::PropositionEvidenceClassification::Witness {
                        evidence,
                        interface,
                    } => Some((evidence, interface)),
                    psi_typed_trees::proposition::PropositionEvidenceClassification::FactOnly => {
                        None
                    }
                }
            })
        else {
            continue;
        };
        let Some(expected_interface) = expected_interface else {
            diagnostics.push(Diagnostic::error(format!(
                "subjectless conformance `{}` cannot provide unresolved generic evidence interface `{expected_interface_label}` required by `{}`",
                assignment.source, assignment.target
            )));
            continue;
        };
        if select_subjectless_evidence_conformance(
            program,
            conformance,
            assignment.source.as_str(),
            &expected_interface,
        )
        .is_none()
        {
            diagnostics.push(Diagnostic::error(format!(
                "subjectless conformance `{}` does not provide the exact `{expected_interface_label}` evidence interface required by `{}`",
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

pub fn select_subjectless_evidence_conformance<'program>(
    program: &'program TypedTrees,
    conformance_symbol: psi_symbols::SymbolHandle,
    source_name: &str,
    expected_interface: &psi_typed_trees::proposition::NormalizedEvidenceInterfaceIdentity,
) -> Option<(
    &'program psi_typed_trees::trait_definition::Conformance,
    psi_symbols::SymbolHandle,
)> {
    use psi_typed_trees::trait_definition::{ConformanceImplementation, ConformanceSubject};

    let conformance = program
        .conformances()
        .iter()
        .find(|candidate| candidate.symbol == conformance_symbol)?;
    if !matches!(conformance.subject, ConformanceSubject::Subjectless)
        || conformance.alias.as_ref()?.as_str() != source_name
        || !matches!(
            conformance.implementation,
            ConformanceImplementation::Closed { .. }
        )
    {
        return None;
    }
    let evidence_trait = program
        .traits()
        .iter()
        .find(|candidate| candidate.name == conformance.trait_name)?;
    let selected_interface = psi_typed_trees::proposition::NormalizedEvidenceInterfaceIdentity {
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
    program: &TypedTrees,
    caller: &Machine,
    call: &TableCall,
    known: &mut BTreeSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some((callee, parameters)) = call_target(program, call) else {
        return;
    };
    let substitutions = call_parameter_substitutions(program, call, parameters);
    let requires = proposition_labels(
        program,
        program.machine_contracts(callee),
        SignatureContractKind::Requires,
        &substitutions,
        false,
    );
    if let Some(missing) = requires.iter().find(|required| !known.contains(*required)) {
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
    program: &'a TypedTrees,
    call: &TableCall,
) -> Option<(&'a Machine, &'a [StateParameter])> {
    for machine in program.machines() {
        if machine.symbol == call.target_symbol {
            let state = program.machine_states(machine).first()?;
            return Some((machine, program.state_parameters(state)));
        }
        if let Some(state) = program
            .machine_states(machine)
            .iter()
            .find(|state| state.symbol == call.target_symbol)
        {
            return Some((machine, program.state_parameters(state)));
        }
    }
    None
}

fn call_parameter_substitutions(
    program: &TypedTrees,
    call: &TableCall,
    parameters: &[StateParameter],
) -> Vec<(psi_symbols::SymbolHandle, String, String)> {
    let arguments = program.statement_table.expression_handles(call.arguments);
    let receiver = psi_typed_trees::expression::display_name_path(
        program.statement_table.name_path_members(call.receiver),
        "::",
    );
    let mut argument_index = 0usize;
    parameters
        .iter()
        .map(|parameter| {
            let label = if parameter.is_self {
                receiver.clone()
            } else {
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
    program: &TypedTrees,
    contracts: &[SignatureContract],
    kind: SignatureContractKind,
    substitutions: &[(psi_symbols::SymbolHandle, String, String)],
    include_boolean_expressions: bool,
) -> Vec<String> {
    contracts
        .iter()
        .filter(|contract| contract.kind == kind)
        .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts))
        .filter_map(|fact| {
            let ProofFact::Proposition(application) = fact else {
                return match fact {
                    ProofFact::Expression(expression) if include_boolean_expressions => {
                        Some(format!(
                            "boolean:{}",
                            program.render_proof_expression_with_parameters(
                                *expression,
                                substitutions,
                            )
                        ))
                    }
                    ProofFact::Expression(_)
                    | ProofFact::Membership(_)
                    | ProofFact::Proposition(_) => None,
                };
            };
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
                    program.render_proof_expression_with_parameters(*argument, substitutions)
                })
                .collect::<Vec<_>>();
            program
                .normalize_proposition_application_with_labels(
                    application,
                    &binder_labels,
                    &argument_labels,
                )
                .map(|formula| formula.identity_label())
        })
        .collect()
}

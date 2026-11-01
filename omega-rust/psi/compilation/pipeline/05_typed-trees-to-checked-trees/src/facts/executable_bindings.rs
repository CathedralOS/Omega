//! Freeze the source-to-execution bindings after all typed arenas stop changing.
use checked_trees::{
    CheckedCaptureArgument, CheckedCaptureArgumentValue, CheckedCaptureInputs,
    CheckedExecutableBindings, CheckedExpressionSource, CheckedRequirementMachines,
    CheckedSelectedMachine, CheckedSourceCallTarget, CheckedSourceTransition,
};
use symbols::JoinedPath;
use typed_trees::expression::ExpressionNode;
use typed_trees::static_applications::StaticMachineBindingValue;

pub(crate) fn record(program: &dyn validation::CheckingInput) -> CheckedExecutableBindings {
    let mut facts = CheckedExecutableBindings::default();
    for (expression, node) in program.expression_table.iter_expressions() {
        let source = match node {
            ExpressionNode::SavedSubject(saved) => CheckedExpressionSource::Saved {
                capture: saved.capture,
                source: program
                    .expression_table
                    .saved_subject_source(saved)
                    .unwrap_or_else(typed_trees::expression::ExpressionHandle::invalid),
            },
            ExpressionNode::Call(call) => {
                facts.calls.push(CheckedSourceCallTarget {
                    target: call.target_symbol,
                    diagnostic_name: call.target.as_str().to_owned(),
                });
                match program
                    .expression_table
                    .build_selection_request(call.build_selection)
                {
                    Some(typed_trees::expression::BuildSelectionRequest::ProviderReadiness(
                        request,
                    )) => CheckedExpressionSource::ProviderReadiness {
                        selection: call.build_selection,
                        requirement: request.requirement,
                    },
                    _ => CheckedExpressionSource::Value,
                }
            }
            _ => CheckedExpressionSource::Value,
        };
        facts.insert_expression(expression, source);
    }
    for (_, capture) in program.captured_machine_applications.iter() {
        let arguments = program
            .expression_table
            .static_machine_application_bindings(capture.binding_application)
            .iter()
            .map(|binding| CheckedCaptureArgument {
                parameter: binding.parameter,
                value: match &binding.value {
                    StaticMachineBindingValue::Type(reference) => {
                        CheckedCaptureArgumentValue::Type(
                            program.normalized_type_identity(*reference),
                        )
                    }
                    StaticMachineBindingValue::Const(reference) => {
                        CheckedCaptureArgumentValue::Const(
                            program
                                .static_const_type_reference_identity(*reference)
                                .map(|identity| {
                                    program
                                        .type_identities
                                        .with_mut(|table| table.intern(&identity))
                                })
                                .unwrap_or_default(),
                        )
                    }
                    StaticMachineBindingValue::Machine(_) => CheckedCaptureArgumentValue::Machine,
                    StaticMachineBindingValue::RuntimeValue { .. } => {
                        CheckedCaptureArgumentValue::RuntimeValue
                    }
                    StaticMachineBindingValue::Conformance(_) => {
                        CheckedCaptureArgumentValue::Conformance
                    }
                },
            })
            .collect();
        facts.captures.push(CheckedCaptureInputs {
            instance: capture.instance,
            arguments,
            machines: program
                .expression_table
                .static_machine_application_selected_machines(capture.binding_application)
                .iter()
                .map(|argument| argument.symbol)
                .collect(),
        });
    }
    for application in program.closed_requirement_applications() {
        facts.requirements.push(CheckedRequirementMachines {
            requirement: application.signature.symbol,
            machines: program
                .expression_table
                .static_machine_application_bindings(application.application)
                .iter()
                .filter_map(|binding| match &binding.value {
                    StaticMachineBindingValue::Machine(target) => Some(CheckedSelectedMachine {
                        symbol: target.symbol,
                        diagnostic_path: target.path.joined_path(),
                    }),
                    _ => None,
                })
                .collect(),
        });
    }
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for (statement_index, statement) in
                crate::flow::indexed_statements(program, state.statement_nodes)
            {
                if let typed_trees::statement::StatementNode::Call(call) = statement {
                    facts.calls.push(CheckedSourceCallTarget {
                        target: call.target_symbol,
                        diagnostic_name: call.target.as_str().to_owned(),
                    });
                }
                let Some(transition) = statement.as_transition() else {
                    continue;
                };
                let named_targets = [transition.target, transition.continuation]
                    .into_iter()
                    .filter(|target| target.is_valid())
                    .filter_map(
                        |target| match program.statement_table.transition_target(target) {
                            typed_trees::statement::TransitionTargetNode::Named {
                                path, ..
                            } => Some(path.symbol),
                            _ => None,
                        },
                    )
                    .filter(|symbol| symbol.is_valid())
                    .collect();
                facts.transitions.push(CheckedSourceTransition {
                    state: state.symbol,
                    statement_index,
                    primary: transition.target,
                    named_targets,
                });
            }
        }
    }
    facts
}

pub(crate) fn freeze_contract_subjects(
    program: &dyn validation::CheckingInput,
    proof: &mut checked_trees::ProofFacts,
) {
    let handles = proof
        .contract_facts
        .iter()
        .map(|(handle, _)| handle)
        .collect::<Vec<_>>();
    for handle in handles {
        let row = proof.contract_facts.get(handle);
        if row.evidence_term.is_none() {
            continue;
        }
        let checked_trees::ContractProofFactOwner::Machine { machine_symbol } = row.owner else {
            continue;
        };
        let Some(machine) = program
            .machines()
            .iter()
            .find(|machine| machine.symbol == machine_symbol)
        else {
            continue;
        };
        let Some(entry) = program.machine_states(machine).first() else {
            continue;
        };
        let Some(application) = program.proof_facts.get(row.fact).as_proposition() else {
            continue;
        };
        let roots = proof.nominal_argument_roots.insert_many(
            program
                .expression_table
                .expression_handles(application.arguments)
                .iter()
                .map(|expression| validation::contract_argument_root(program, entry, *expression)),
        );
        proof.contract_facts.get_mut(handle).nominal_argument_roots = Some(roots);
    }
}

pub(crate) fn freeze_declaration_arguments(
    program: &dyn validation::CheckingInput,
    plan: &mut checked_trees::CheckedRankArrivalPlan,
) {
    let handles = plan
        .application_calls
        .iter()
        .map(|(handle, _)| handle)
        .collect::<Vec<_>>();
    for handle in handles {
        let source = plan.application_calls.get(handle).arguments;
        let arguments = plan.application_call_arguments.insert_many(
            program
                .expression_table
                .expression_handles(source)
                .iter()
                .copied(),
        );
        plan.application_calls.get_mut(handle).checked_arguments = arguments;
    }
}

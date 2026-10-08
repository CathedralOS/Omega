//! Preserve exact substitutions before result elaboration or executable
//! retargeting consumes their proposals. Calls own their lexical application;
//! instances own a closed tuple shared by their callers. Neither row grants
//! permission to satisfy a bound. Immutable checking owes that judgment.

use super::{
    CallSelection, CallSite, Candidate, ExpressionNode, StatementNode, SymbolHandle, TypedTrees,
};
use typed_trees::static_applications::{
    StaticMachineApplicationHandle, StaticMachineBinding, StaticMachineBindingValue,
};

pub(super) fn candidate_bindings(candidate: &Candidate) -> Vec<StaticMachineBinding> {
    let mut bindings = Vec::new();
    for ((parameter, _), actual) in candidate
        .template
        .type_parameters
        .iter()
        .zip(&candidate.type_bindings)
    {
        if let Some(actual) = actual {
            bindings.push(StaticMachineBinding {
                parameter: *parameter,
                value: StaticMachineBindingValue::Type(*actual),
            });
        }
    }
    for (ordinal, ((parameter, _, _), actual)) in candidate
        .template
        .const_parameters
        .iter()
        .zip(&candidate.const_bindings)
        .enumerate()
    {
        if let Some(actual) = actual {
            let value = if candidate.runtime_value_bindings[ordinal].is_some() {
                StaticMachineBindingValue::RuntimeValue(*actual)
            } else {
                StaticMachineBindingValue::Const(*actual)
            };
            bindings.push(StaticMachineBinding {
                parameter: *parameter,
                value,
            });
        }
    }
    for ((parameter, _, _), actual) in candidate
        .template
        .machine_parameters
        .iter()
        .zip(&candidate.machine_bindings)
    {
        if let Some(actual) = actual {
            bindings.push(StaticMachineBinding {
                parameter: *parameter,
                value: StaticMachineBindingValue::Machine(actual.clone()),
            });
        }
    }
    for (parameter, actual) in candidate
        .template
        .evidence_parameters
        .iter()
        .zip(&candidate.evidence_bindings)
    {
        if let Some(actual) = actual {
            bindings.push(StaticMachineBinding {
                parameter: parameter.binder,
                value: StaticMachineBindingValue::Conformance(actual.clone()),
            });
        }
    }
    bindings
}

pub(super) fn retain_call(
    program: &mut TypedTrees,
    candidate: &Candidate,
    selection: &CallSelection,
) {
    let (application, target_symbol, target, arguments) = match selection.site {
        CallSite::Expression(handle) => {
            let Some(call) = program.expression_table.call_node(handle) else {
                return;
            };
            (
                call.authored_machine_application,
                call.target_symbol,
                call.target.clone(),
                call.machine_arguments.to_vec(),
            )
        }
        CallSite::Statement(handle) => {
            let Some(call) = program.statement_table.statement(handle).as_call() else {
                return;
            };
            (
                call.authored_machine_application,
                call.target_symbol,
                call.target.clone(),
                call.machine_arguments.to_vec(),
            )
        }
    };
    let application = if application.is_valid() {
        application
    } else {
        let application = program.expression_table.retain_static_machine_application(
            target_symbol,
            target,
            &arguments,
        );
        program
            .expression_table
            .static_machine_application_mut(application)
            .lexical_owner = selection.lexical_owner;
        match selection.site {
            CallSite::Expression(handle) => {
                let ExpressionNode::Call(call) = program.expression_table.expression_mut(handle)
                else {
                    unreachable!()
                };
                call.authored_machine_application = application;
            }
            CallSite::Statement(handle) => {
                let StatementNode::Call(call) = program.statement_table.statement_mut(handle)
                else {
                    unreachable!()
                };
                call.authored_machine_application = application;
            }
        }
        application
    };
    program
        .expression_table
        .static_machine_application_mut(application)
        .binding_declaration = candidate.template.template_symbol;
    program
        .expression_table
        .set_static_machine_application_bindings(application, &candidate_bindings(candidate));
}

pub(super) fn retain_instance(
    source: Option<&dyn validation::CheckingInput>,
    program: &mut TypedTrees,
    candidate: &Candidate,
    instance: SymbolHandle,
) -> StaticMachineApplicationHandle {
    let mut bindings = candidate_bindings(candidate);
    if source.is_some() {
        super::body_cloning::copy_binding_type_payloads(source, program, &mut bindings, &[]);
    }
    let application = program.expression_table.retain_static_machine_application(
        candidate.template.template_symbol,
        typed_trees::name::Identifier::generated(&candidate.template.template_name),
        &[],
    );
    program
        .expression_table
        .static_machine_application_mut(application)
        .lexical_owner = instance;
    program
        .expression_table
        .static_machine_application_mut(application)
        .binding_declaration = candidate.template.template_symbol;
    program
        .expression_table
        .set_static_machine_application_bindings(application, &bindings);
    application
}

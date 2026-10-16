//! Preserve exact substitutions before result elaboration or executable
//! retargeting consumes their proposals. Calls own their lexical application;
//! instances own a closed tuple shared by their callers. Neither row grants
//! permission to satisfy a bound. Immutable checking owes that judgment.

use super::{
    CallSelection, CallSite, Candidate, ExpressionHandle, ExpressionNode, StatementNode,
    SymbolHandle, TypedTrees,
};
use crate::monomorphization::const_arguments;
use typed_trees::static_applications::{
    StaticMachineApplicationHandle, StaticMachineBinding, StaticMachineBindingValue,
};

pub(super) fn candidate_bindings(
    program: &dyn ::validation::CheckingInput,
    candidate: &Candidate,
) -> Vec<StaticMachineBinding> {
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
            let value = if let Some(argument) = &candidate.runtime_value_bindings[ordinal] {
                StaticMachineBindingValue::RuntimeValue {
                    type_reference: *actual,
                    subject: argument.symbol,
                    expression: const_arguments::runtime_subject_expression(program, argument)
                        .unwrap_or_else(ExpressionHandle::invalid),
                }
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

pub(super) fn retain_authored_call(
    program: &mut TypedTrees,
    site: CallSite,
    lexical_owner: SymbolHandle,
) -> StaticMachineApplicationHandle {
    let (application, target_symbol, target, arguments) = match site {
        CallSite::Expression(handle) => {
            let Some(call) = program.expression_table.call_node(handle) else {
                return StaticMachineApplicationHandle::invalid();
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
                return StaticMachineApplicationHandle::invalid();
            };
            (
                call.authored_machine_application,
                call.target_symbol,
                call.target.clone(),
                call.machine_arguments.to_vec(),
            )
        }
    };
    if application.is_valid() {
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
            .lexical_owner = lexical_owner;
        match site {
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
    }
}

pub(super) fn retain_call(
    program: &mut TypedTrees,
    candidate: &Candidate,
    selection: &CallSelection,
) {
    let application = retain_authored_call(program, selection.site, selection.lexical_owner);
    if !application.is_valid() {
        return;
    }
    let authored = program
        .expression_table
        .static_machine_application(application)
        .registration_operation;
    if authored.is_valid()
        && program.symbols.get(authored).kind == symbols::SymbolKind::MachineParameter
    {
        // The callable parameter owns the authored invocation telescope. Its
        // selected provider has an independent instance tuple; rewriting the
        // call cannot replace the requirement's binders with that provider's.
        program
            .expression_table
            .static_machine_application_mut(application)
            .binding_declaration = authored;
        return;
    }
    program
        .expression_table
        .static_machine_application_mut(application)
        .binding_declaration = candidate.template.template_symbol;
    let bindings = candidate_bindings(program, candidate);
    program
        .expression_table
        .set_static_machine_application_bindings(application, &bindings);
}

pub(super) fn retain_instance(
    source: Option<&dyn validation::CheckingInput>,
    program: &mut TypedTrees,
    candidate: &Candidate,
    instance: SymbolHandle,
) -> StaticMachineApplicationHandle {
    let mut bindings = candidate_bindings(program, candidate);
    // Carrier sharing does not make an instance the author of its operands.
    // Preserve the representative source application's lexical region scope;
    // every other source edge still owns its separate admission. Roster-only
    // instances have no source call and retain their declaration context.
    let (lexical_owner, lifetimes) = if candidate.binding_application.is_valid() {
        let origin = source.unwrap_or(program);
        (
            origin
                .expression_table
                .static_machine_application(candidate.binding_application)
                .lexical_owner,
            origin
                .expression_table
                .static_machine_application_lifetimes(candidate.binding_application)
                .to_vec(),
        )
    } else {
        (instance, Vec::new())
    };
    // Runtime callers share a carrier-specialized instance. No one caller's
    // subject belongs to that instance's static tuple.
    for binding in &mut bindings {
        if let StaticMachineBindingValue::RuntimeValue {
            subject,
            expression,
            ..
        } = &mut binding.value
        {
            *subject = SymbolHandle::invalid();
            *expression = ExpressionHandle::invalid();
        }
    }
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
        .lexical_owner = lexical_owner;
    program
        .expression_table
        .static_machine_application_mut(application)
        .binding_declaration = candidate.template.template_symbol;
    program
        .expression_table
        .set_static_machine_application_bindings(application, &bindings);
    program
        .expression_table
        .set_static_machine_application_lifetimes(application, &lifetimes);
    application
}

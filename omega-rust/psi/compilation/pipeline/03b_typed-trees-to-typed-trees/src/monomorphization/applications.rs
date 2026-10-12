//! Preserve exact substitutions before result elaboration or executable
//! retargeting consumes their proposals. Calls own their lexical application;
//! instances own a closed tuple shared by their callers. Neither row grants
//! permission to satisfy a bound. Immutable checking owes that judgment.

use super::{CallSelection, CallSite, Candidate, StatementNode, SymbolHandle, TypedTrees};
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
            let value = if let Some(argument) = &candidate.runtime_value_bindings[ordinal] {
                StaticMachineBindingValue::RuntimeValue {
                    type_reference: *actual,
                    subject: argument.symbol,
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

pub(crate) fn retain_authored_call(
    program: &mut TypedTrees,
    site: CallSite,
    lexical_owner: SymbolHandle,
) -> StaticMachineApplicationHandle {
    let (application, target_symbol, target, arguments) = match site {
        CallSite::Expression(handle) => {
            let Some(call) = program.expression_table.source_call_node(handle) else {
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
            .lexical_owner = lexical_owner;
        match site {
            CallSite::Expression(handle) => {
                let call = program
                    .expression_table
                    .source_call_node_mut(handle)
                    .expect("retained original invocation remained occupied");
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
    // Region-only calls have no specialization candidate to publish their
    // receiving declaration later. The retained original registration owns
    // this coordinate independently of any executable instance or tuple.
    let retained = program
        .expression_table
        .static_machine_application(application);
    if !retained.binding_declaration.is_valid() {
        let declaration =
            typed_trees::static_applications::StaticApplicationDeclaration::for_selected_interface(
                program,
                retained.registration_operation,
                retained.selected_interface,
            )
            .map(|declaration| declaration.symbol())
            .unwrap_or_default();
        if declaration.is_valid() {
            program
                .expression_table
                .static_machine_application_mut(application)
                .binding_declaration = declaration;
        }
    }
    application
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
    program
        .expression_table
        .static_machine_application_mut(application)
        .binding_declaration = candidate.template.template_symbol;
    program
        .expression_table
        .set_static_machine_application_receiving_data_parameter(
            application,
            selection.receiving_data_parameter,
        );
    let previous = program
        .expression_table
        .static_machine_application(application)
        .enclosing_data_applications;
    if program
        .type_reference_table
        .type_reference_handles(previous)
        != selection.enclosing_data_applications
    {
        let applications = program
            .type_reference_table
            .insert_type_reference_handles(selection.enclosing_data_applications.iter().copied());
        program
            .expression_table
            .set_static_machine_application_enclosing_data_applications(application, applications);
    }
    program
        .expression_table
        .set_static_machine_application_bindings(application, &candidate_bindings(candidate));
}

pub(super) fn retain_instance(
    source: Option<&dyn validation::CheckingInput>,
    program: &mut TypedTrees,
    candidate: &Candidate,
    instance: SymbolHandle,
) -> Result<StaticMachineApplicationHandle, diagnostics::Diagnostic> {
    let mut bindings = candidate_bindings(candidate);
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
    let captured_lexical_scope = if candidate.binding_application.is_valid() {
        let origin = source.unwrap_or(program);
        let captured = origin
            .expression_table
            .static_machine_application(candidate.binding_application)
            .captured_lexical_scope;
        if captured.is_valid() {
            Some(
                origin
                    .captured_static_lexical_scope(captured)
                    .ok_or_else(|| {
                        diagnostics::Diagnostic::error(
                            "source instance lost its original lexical capture",
                        )
                    })?
                    .clone(),
            )
        } else {
            None
        }
    } else {
        None
    };
    let captured_lexical_scope = captured_lexical_scope
        .as_ref()
        .map(|scope| super::captured_lexical_scopes::retain(program, scope))
        .transpose()?
        .unwrap_or_default();
    // Runtime callers share a carrier-specialized instance. No one caller's
    // subject belongs to that instance's static tuple.
    for binding in &mut bindings {
        if let StaticMachineBindingValue::RuntimeValue { subject, .. } = &mut binding.value {
            *subject = SymbolHandle::invalid();
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
        .captured_lexical_scope = captured_lexical_scope;
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
    Ok(application)
}

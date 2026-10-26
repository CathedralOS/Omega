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
                    subject: match argument {
                        super::RuntimeValueBinding::Authored(argument) => argument.symbol,
                        _ => SymbolHandle::invalid(),
                    },
                    expression: match argument {
                        super::RuntimeValueBinding::Authored(argument) => {
                            const_arguments::runtime_subject_expression(program, argument)
                                .unwrap_or_else(ExpressionHandle::invalid)
                        }
                        _ => ExpressionHandle::invalid(),
                    },
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
            if let Some(query) = &call.native_code_layout {
                (
                    call.authored_machine_application,
                    query.operand.symbol,
                    query
                        .operand
                        .path
                        .last()
                        .cloned()
                        .unwrap_or_else(|| call.target.clone()),
                    query
                        .operand
                        .application
                        .as_ref()
                        .map(|application| application.arguments.to_vec())
                        .unwrap_or_default(),
                )
            } else {
                (
                    call.authored_machine_application,
                    call.target_symbol,
                    call.target.clone(),
                    call.machine_arguments.to_vec(),
                )
            }
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
    // The registration retains the authored callable binder. Once selection
    // chooses a concrete template, its retained tuple belongs to that exact
    // template; common argument correspondence transports it to the contract.
    program
        .expression_table
        .static_machine_application_mut(application)
        .binding_declaration = candidate.template.template_symbol;
    let mut bindings = candidate_bindings(program, candidate);
    // Executable candidates retain only closed proposals. The source call also
    // owns each explicitly supplied symbolic const, independently of whether
    // a concrete instance exists yet. Missing/inferred operands stay missing;
    // immutable checking still admits carrier, formation and every bound.
    let parameters = program
        .machine_by_symbol(candidate.template.template_symbol)
        .map(|declaration| program.machine_type_parameters(declaration).to_vec())
        .unwrap_or_default();
    let arguments = program
        .expression_table
        .static_machine_application_arguments(application)
        .to_vec();
    for (parameter, argument) in parameters.iter().zip(&arguments) {
        if !matches!(
            parameter.kind,
            typed_trees::data::TypeParameterKind::Const { .. }
        ) || bindings
            .iter()
            .any(|binding| binding.parameter == parameter.symbol)
        {
            continue;
        }
        if let Ok(actual) =
            program.static_operand_type_reference_in_scope(argument, selection.lexical_owner)
        {
            bindings.push(StaticMachineBinding {
                parameter: parameter.symbol,
                value: StaticMachineBindingValue::Const(actual),
            });
        }
    }
    // Source checking needs inferred values before executable retargeting,
    // including inside an open generic caller. Reuse these exact expressions
    // when realization later appends the ordinary hidden operands.
    let runtime = candidate
        .runtime_value_bindings
        .iter()
        .flatten()
        .cloned()
        .collect::<Vec<_>>();
    let operands =
        super::body_rewriting::runtime_value_operands(program, selection.lexical_state, &runtime);
    let mut operands = operands.into_iter();
    for binding in &mut bindings {
        if let StaticMachineBindingValue::RuntimeValue {
            subject,
            expression,
            ..
        } = &mut binding.value
        {
            let Some(operand) = operands.next() else {
                break;
            };
            if !subject.is_valid() {
                *expression = operand;
            }
        }
    }
    program
        .expression_table
        .set_static_machine_application_bindings(application, &bindings);
}

pub(super) fn retain_instance(
    source: Option<&dyn validation::CheckingInput>,
    program: &mut TypedTrees,
    candidate: &Candidate,
    instance: SymbolHandle,
) -> Result<StaticMachineApplicationHandle, Vec<super::Diagnostic>> {
    let mut bindings = candidate_bindings(program, candidate);
    // Carrier sharing does not make an instance the author of its operands.
    // Preserve the representative source application's lexical region scope;
    // every other source edge still owns its separate admission. Roster-only
    // instances have no source call and retain their declaration context.
    let (lexical_owner, lifetimes) = if candidate.binding_application.is_valid() {
        let origin = source.unwrap_or(program);
        let source_application = origin
            .expression_table
            .static_machine_application(candidate.binding_application);
        let mut lifetimes = origin
            .expression_table
            .static_machine_application_lifetimes(candidate.binding_application)
            .to_vec();
        let source_declaration = source_application.binding_declaration;
        let target_declaration = candidate.template.template_symbol;
        if source_declaration != target_declaration
            && origin.target_machine_families.iter().any(|family| {
                let contains = |symbol| {
                    family.canonical == symbol
                        || family.members.iter().any(|member| member.machine == symbol)
                };
                contains(source_declaration) && contains(target_declaration)
            })
        {
            let source_machine = origin
                .machine_by_symbol(source_declaration)
                .ok_or_else(|| {
                    vec![super::Diagnostic::error(
                        "family lifetime tuple lost its source declaration",
                    )]
                })?;
            let target_machine = origin
                .machine_by_symbol(target_declaration)
                .ok_or_else(|| {
                    vec![super::Diagnostic::error(
                        "family lifetime tuple lost its target declaration",
                    )]
                })?;
            if source_machine.lifetime_parameters.len() != target_machine.lifetime_parameters.len()
            {
                return Err(vec![super::Diagnostic::error(
                    "family lifetime telescopes have incompatible positions",
                )]);
            }
            for binding in &mut lifetimes {
                let ordinal = source_machine
                    .lifetime_parameters
                    .iter()
                    .position(|parameter| parameter == &binding.parameter)
                    .ok_or_else(|| {
                        vec![super::Diagnostic::error(
                            "family lifetime tuple names no source binder",
                        )]
                    })?;
                binding.parameter = target_machine.lifetime_parameters[ordinal].clone();
            }
        }
        (source_application.lexical_owner, lifetimes)
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
    Ok(application)
}

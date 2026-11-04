//! A copied call selects code for its own caller's arguments.
//!
//! Retained source bindings survive executable specialization. Restore those
//! bindings before ordinary selection runs again; a previous caller's capture
//! or invocation instance cannot supply the copied call's environment.

use crate::monomorphization::{CallSite, Candidate, ExpressionHandle, ExpressionNode, HandleSpan};
use crate::tables::{expressions_of, expressions_of_mut};
use symbols::{SymbolHandle, SymbolKind};
use typed_trees::TypedTrees;
use typed_trees::expression::StaticMachineArgument;
use typed_trees::references::StaticArgumentCall;
use typed_trees::statement::StatementNode;
use typed_trees::static_applications::StaticMachineBindingValue;
use typed_trees::typed_trees::StaticRequirementDispatch;
use typed_trees::types::TypeReferenceNode;

struct CopiedCall {
    target: SymbolHandle,
    machine_parameter: SymbolHandle,
    application: typed_trees::static_applications::StaticMachineApplicationHandle,
    dispatch: Option<StaticRequirementDispatch>,
    arguments: Vec<ExpressionHandle>,
    static_arguments: Vec<StaticMachineArgument>,
}

pub(super) fn restore(
    program: &mut TypedTrees,
    candidate: &Candidate<'_>,
    states: &[(SymbolHandle, SymbolHandle)],
    expression_start: usize,
    copied_states: HandleSpan<typed_trees::state::State>,
) -> Result<(), &'static str> {
    let statements = program
        .machine_states
        .span_or_empty(copied_states)
        .iter()
        .flat_map(|state| state.statement_nodes.handles())
        .collect::<Vec<_>>();
    let expressions = expressions_of(program)
        .iter_expressions_from(expression_start)
        .map(|(handle, _)| handle)
        .collect::<Vec<_>>();
    for site in statements
        .into_iter()
        .map(CallSite::Statement)
        .chain(expressions.into_iter().map(CallSite::Expression))
    {
        let Some(call) = snapshot(program, site) else {
            continue;
        };
        if !call.application.is_valid() {
            continue;
        }
        let source = expressions_of(program)
            .static_machine_application(call.application)
            .clone();
        let operation = source.registration_operation;
        let recursive_entry = states.first().is_some_and(|(entry, _)| {
            operation == *entry || operation == candidate.template.template_symbol
        });
        // A subordinate state is still in the copied activation. An entry
        // invocation instead owns another complete static application.
        if !recursive_entry && states.iter().any(|(_, copied)| *copied == call.target) {
            continue;
        }
        let old_target = states
            .iter()
            .find(|(_, copied)| *copied == call.target)
            .map_or(call.target, |(original, _)| *original);
        let old_target = if recursive_entry
            && states
                .first()
                .is_some_and(|(_, entry)| program.symbols.get(*entry).parent == call.target)
        {
            candidate.template.template_symbol
        } else {
            old_target
        };
        restore_call(program, site, call, source, old_target)?;
    }
    Ok(())
}

/// Restore copied expressions before ordinary preparation selects their code.
/// The copy owner supplies exact handles, including applications copied with
/// each operand; this operation never searches by source text or spelling.
pub fn restore_expressions(
    program: &mut TypedTrees,
    expressions: &[ExpressionHandle],
) -> Result<(), &'static str> {
    for &expression in expressions {
        let site = CallSite::Expression(expression);
        let Some(call) = snapshot(program, site) else {
            continue;
        };
        if !call.application.is_valid() {
            continue;
        }
        let source = expressions_of(program)
            .static_machine_application(call.application)
            .clone();
        if !source.registration_operation.is_valid() {
            return Err("copied call has no exact authored operation");
        }
        // A qualifier has its own telescope, independent of the method's
        // arguments. Recover its complete operand before discarding dispatch.
        let implementation = if let Some(dispatch) = &call.dispatch {
            if !dispatch.selection_application.is_valid() {
                return Err("copied selected call has no source implementation application");
            }
            let selected =
                expressions_of(program).static_machine_application(dispatch.selection_application);
            if selected.registration_operation != dispatch.requirement {
                return Err("copied implementation selection belongs to another requirement");
            }
            let selection = selected
                .conformance_selection
                .as_ref()
                .ok_or("copied selected call has no source implementation operand")?;
            let operand = expressions_of(program)
                .static_machine_application_arguments(dispatch.selection_application)
                .get(selection.operand_ordinal as usize)
                .filter(|operand| operand.names_implementation(&program.symbols))
                .cloned()
                .ok_or("copied selected call has no complete implementation operand")?;
            Some(operand)
        } else {
            None
        };
        let old_target = call.target;
        let restored_roster = restore_call(program, site, call, source.clone(), old_target)?;
        let Some(restored) = snapshot(program, site) else {
            return Err("copied source call disappeared during restoration");
        };
        // The old Value suffix was checked against its retained bindings above.
        // A fresh source application will evaluate those copied operands through
        // its telescope and append its own suffix during ordinary preparation.
        let arguments = restored_roster.map_or(restored.arguments.clone(), |(count, _)| {
            restored.arguments[..count].to_vec()
        });
        let method_arguments = if implementation.is_some()
            && restored_roster.is_some_and(|(_, has_parameters)| !has_parameters)
        {
            Vec::new()
        } else {
            restored.static_arguments
        };
        let application = expressions_of_mut(program).retain_static_machine_application(
            source.registration_operation,
            source.target,
            &method_arguments,
        );
        let fresh = expressions_of_mut(program).static_machine_application_mut(application);
        fresh.input_selections = source.input_selections;
        // A selected map is restored below as a separate applied qualifier.
        // Its selection ordinal must not be reinterpreted as a method binder.
        fresh.conformance_selection = None;
        let lifetimes = expressions_of(program)
            .static_machine_application_lifetimes(restored.application)
            .to_vec();
        expressions_of_mut(program).set_static_machine_application_lifetimes(
            application,
            source.lifetime_binding_declaration,
            &lifetimes,
        );
        let qualifier = implementation
            .as_ref()
            .map(|operand| program.static_operand_type_reference(operand))
            .transpose()?;
        let receiver = implementation.map(|operand| {
            let name =
                typed_trees::name::Identifier::generated(program.symbols.name(operand.symbol));
            crate::monomorphization::insert_subject_name(
                expressions_of_mut(program),
                name,
                operand.symbol,
            )
        });
        // Only the source occurrence crosses to the new caller. Its selected
        // body, inferred bindings and closed conformance must be derived again.
        let arguments = expressions_of_mut(program).insert_expression_handles(arguments);
        let ExpressionNode::Call(call) = expressions_of_mut(program).expression_mut(expression)
        else {
            return Err("copied source call lost its expression");
        };
        call.authored_machine_application = application;
        call.target_symbol = source.registration_operation;
        call.static_requirement_dispatch = None;
        call.machine_arguments = method_arguments.into();
        if let Some(qualifier) = qualifier {
            call.requirement_application = qualifier;
            call.receiver = receiver.expect("a copied qualifier retains its selected operand");
        }
        call.arguments = arguments;
    }
    Ok(())
}

fn restore_call(
    program: &mut TypedTrees,
    site: CallSite,
    mut call: CopiedCall,
    source: typed_trees::static_applications::StaticMachineApplication,
    old_target: SymbolHandle,
) -> Result<Option<(usize, bool)>, &'static str> {
    let operation = source.registration_operation;
    let original = declaration_entry(program, authored_target(program, old_target)?);
    let placeholder = call.machine_parameter.is_valid()
        || (operation.is_valid()
            && matches!(
                program.symbols.get(operation).kind,
                SymbolKind::MachineParameter | SymbolKind::ConformanceParameter
            ))
        || (operation.is_valid()
            && program.symbols.get(operation).parent.is_valid()
            && program
                .symbols
                .get(program.symbols.get(operation).parent)
                .kind
                == SymbolKind::ConformanceParameter);
    let declaration_target = if let Some(dispatch) = &call.dispatch {
        if original != dispatch.realization_state {
            return Err("copied selected call lost its authored realization entry");
        }
        dispatch.realization_state
    } else if placeholder {
        original
    } else {
        let declared = declaration_entry(program, operation);
        if original != declared {
            // Unresolved ordinary calls still belong to source admission.
            // A closed target cannot hide a different declaration here.
            if old_target == call.target && original == call.target {
                return Ok(None);
            }
            return Err("copied call's executable belongs to another source declaration");
        }
        declared
    };
    if call.dispatch.is_none() && !placeholder && call.target == declaration_target {
        return Ok(None);
    }
    let signature = call.dispatch.as_ref().and_then(|dispatch| {
        program
            .trait_machine_signature(dispatch.requirement)
            .map(|(_, signature)| signature)
    });
    let parameters = if let Some(machine) = program.machine_by_symbol(source.binding_declaration) {
        let parameters = program.machine_callable_parameters(machine);
        signature.map_or(parameters.clone(), |signature| {
            program.requirement_callable_parameters(signature, &parameters)
        })
    } else if let Some((_, signature)) = program.trait_machine_signature(source.binding_declaration)
    {
        program
            .requirement_invocation_contract_parameters(signature)
            .to_vec()
    } else {
        Vec::new()
    };
    let bindings = expressions_of(program)
        .static_machine_application_bindings(call.application)
        .to_vec();
    let ordinary = if let Some(signature) = signature {
        program.state_signature_parameters(signature).len()
    } else if let Some((_, signature)) = program.trait_machine_signature(declaration_target) {
        program.state_signature_parameters(signature).len()
    } else if let Some((_, signature)) = program.machine_parameter_signature(operation) {
        program.state_signature_parameters(signature).len()
    } else if let Some(state) = program.state_by_symbol(declaration_target) {
        program.state_parameters(state).len()
    } else {
        // Bare callable placeholders have not selected an executable yet.
        return Ok(None);
    };
    let runtime = parameters
        .iter()
        .filter_map(|parameter| {
            bindings
                .iter()
                .find(|binding| binding.parameter == parameter.symbol)
                .filter(|binding| {
                    matches!(
                        binding.value,
                        StaticMachineBindingValue::RuntimeValue { .. }
                    )
                })
        })
        .collect::<Vec<_>>();
    if call.arguments.len() != ordinary {
        if call.arguments.len() != ordinary + runtime.len() {
            return Err("copied call lost its ordinary and realized Value operand roster");
        }
        for (binding, actual) in runtime.iter().zip(&call.arguments[ordinary..]) {
            let StaticMachineBindingValue::RuntimeValue {
                subject,
                expression,
                ..
            } = binding.value
            else {
                return Err("copied Value tail has another binding category");
            };
            if expression.is_valid() {
                if expression != *actual {
                    return Err("copied Value operand differs from its retained evaluation");
                }
            } else if !subject.is_valid()
                || !validation::canonical_place_from_expression(program, *actual).is_some_and(
                    |place| place.root.as_symbol() == Some(subject) && place.segments.is_empty(),
                )
            {
                return Err("copied Value operand belongs to another runtime subject");
            }
        }
    }
    if (!placeholder || call.dispatch.is_some()) && !parameters.is_empty() && !bindings.is_empty() {
        let mut arguments = Vec::with_capacity(parameters.len());
        for parameter in &parameters {
            let mut matches = bindings
                .iter()
                .filter(|binding| binding.parameter == parameter.symbol);
            let binding = matches
                .next()
                .ok_or("copied call lost an inferred source operand")?;
            if matches.next().is_some() {
                return Err("copied call repeats an inferred source operand");
            }
            arguments.push(match &binding.value {
                StaticMachineBindingValue::Type(reference)
                | StaticMachineBindingValue::Const(reference) => {
                    program.static_operand_from_type_reference(*reference)?
                }
                StaticMachineBindingValue::Machine(argument)
                | StaticMachineBindingValue::Conformance(argument) => argument.clone(),
                StaticMachineBindingValue::RuntimeValue { expression, .. }
                    if expression.is_valid() =>
                {
                    let reference = program
                        .type_reference_table
                        .insert(TypeReferenceNode::ConstExpression(*expression));
                    StaticMachineArgument {
                        type_reference: reference,
                        ..Default::default()
                    }
                }
                StaticMachineBindingValue::RuntimeValue { subject, .. } if subject.is_valid() => {
                    StaticMachineArgument {
                        symbol: *subject,
                        path: Box::from([program.symbols.name(*subject).into()]),
                        ..Default::default()
                    }
                }
                StaticMachineBindingValue::RuntimeValue { .. } => {
                    return Err("copied runtime Value has no retained subject");
                }
            });
        }
        call.static_arguments = arguments;
    } else {
        call.static_arguments = expressions_of(program)
            .static_machine_application_arguments(call.application)
            .to_vec();
    }
    if let Some(dispatch) = &mut call.dispatch {
        dispatch.contract_signature = SymbolHandle::invalid();
        if dispatch.evidence_parameter.is_valid() {
            call.target = operation;
            call.dispatch = None;
        } else {
            call.target = dispatch.realization_state;
        }
    } else {
        call.target = if placeholder {
            operation
        } else {
            declaration_target
        };
    }
    publish(program, site, call);
    Ok(Some((ordinary, !parameters.is_empty())))
}

fn snapshot(program: &TypedTrees, site: CallSite) -> Option<CopiedCall> {
    let (call, arguments, target, machine_parameter, dispatch, static_arguments) = match site {
        CallSite::Expression(expression) => {
            let call = expressions_of(program).call_node(expression)?;
            (
                StaticArgumentCall::Expression(expression, call),
                expressions_of(program).expression_handles(call.arguments),
                call.target_symbol,
                call.static_machine_parameter,
                call.static_requirement_dispatch.clone(),
                call.machine_arguments.to_vec(),
            )
        }
        CallSite::Statement(statement) => {
            let call = program.statement_table.statement(statement).as_call()?;
            (
                StaticArgumentCall::Statement(call),
                program.statement_table.expression_handles(call.arguments),
                call.target_symbol,
                call.static_machine_parameter,
                call.static_requirement_dispatch.clone(),
                call.machine_arguments.to_vec(),
            )
        }
    };
    Some(CopiedCall {
        target,
        machine_parameter,
        application: call.application(),
        dispatch,
        arguments: arguments.to_vec(),
        static_arguments,
    })
}

fn publish(program: &mut TypedTrees, site: CallSite, restored: CopiedCall) {
    match site {
        CallSite::Expression(expression) => {
            let arguments =
                expressions_of_mut(program).insert_expression_handles(restored.arguments);
            if let ExpressionNode::Call(call) =
                expressions_of_mut(program).expression_mut(expression)
            {
                call.target_symbol = restored.target;
                call.static_requirement_dispatch = restored.dispatch;
                call.machine_arguments = restored.static_arguments.into();
                call.arguments = arguments;
            }
        }
        CallSite::Statement(statement) => {
            let arguments = program
                .statement_table
                .insert_expression_handles(restored.arguments);
            if let StatementNode::Call(call) = program.statement_table.statement_mut(statement) {
                call.target_symbol = restored.target;
                call.static_requirement_dispatch = restored.dispatch;
                call.machine_arguments = restored.static_arguments.into();
                call.arguments = arguments;
            }
        }
    }
}

fn declaration_entry(program: &TypedTrees, declaration: SymbolHandle) -> SymbolHandle {
    program
        .machine_by_symbol(declaration)
        .and_then(|machine| program.machine_states(machine).first())
        .map_or(declaration, |entry| entry.symbol)
}

/// Follow only the instance's recorded template and state correspondence.
/// Capture arguments belong to their old caller and are not selection input.
fn authored_target(
    program: &TypedTrees,
    mut target: SymbolHandle,
) -> Result<SymbolHandle, &'static str> {
    let mut visited = Vec::new();
    loop {
        if visited.contains(&target) {
            return Err("copied call has cyclic executable provenance");
        }
        visited.push(target);
        if let Some(closed) = program.closed_requirement_application(target) {
            target = closed.template;
            continue;
        }
        let declaration = program.machine_by_symbol(target);
        let Some(machine) = declaration.or_else(|| program.machine_holding_state(target)) else {
            return Ok(target);
        };
        let Some(template) = program.executable_application_template(machine.symbol)? else {
            return Ok(target);
        };
        if declaration.is_some() {
            target = template;
        } else if let Some((_, capture)) = program
            .captured_machine_applications
            .iter()
            .find(|(_, capture)| capture.instance == machine.symbol)
        {
            let mut states = program
                .captured_machine_states
                .span_or_empty(capture.states)
                .iter()
                .filter(|state| state.instance == target);
            let template_state = states
                .next()
                .ok_or("copied call lost its captured state correspondence")?
                .template;
            if states.next().is_some() {
                return Err("copied call has ambiguous captured state correspondence");
            }
            target = template_state;
        } else {
            let ordinal =
                typed_trees::state::state_position(program.machine_states(machine), target)
                    .ok_or("copied call lost its specialized state position")?;
            target = program
                .machine_by_symbol(template)
                .and_then(|template| program.machine_states(template).get(ordinal))
                .ok_or("copied call lost its template state")?
                .symbol;
        }
    }
}

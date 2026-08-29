//! Contextual lifetime resolution for generic conformance applications.
//!
//! This module consumes ordinary parameter-position borrow constraints. It is
//! deliberately separate from application closure: absent, ambiguous, and
//! conflicting regions remain errors rather than declaration-based guesses.

use crate::conformance_applications::{
    static_argument_identity, substituted_type_identity_with_lifetimes,
};
use psi_diagnostics::Diagnostic;
use psi_symbols::{SymbolHandle, SymbolKind};
use psi_typed_trees::TypedTrees;
use psi_typed_trees::data::TypeParameterKind;
use psi_typed_trees::expression::{ExpressionHandle, ExpressionNode, StaticMachineArgument};
use psi_typed_trees::name::Identifier;
use psi_typed_trees::statement::{StatementHandle, StatementNode};
use psi_typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

#[derive(Clone, Copy)]
enum ElisionCallSite {
    Expression(ExpressionHandle),
    Statement(StatementHandle),
}

struct ElisionCall {
    site: ElisionCallSite,
    caller_machine: SymbolHandle,
    caller_state: SymbolHandle,
    target: SymbolHandle,
    arguments: Vec<ExpressionHandle>,
    machine_arguments: Box<[StaticMachineArgument]>,
}

/// Resolve erased conformance regions from the same parameter-position borrow
/// facts used by ordinary calls. This runs before strict application closure:
/// it only fills a complete, uniquely constrained lifetime lane and otherwise
/// leaves the authored application untouched for the normal diagnostic.
pub(crate) fn resolve_elided_conformance_lifetimes(
    program: &mut TypedTrees,
) -> Result<(), Vec<Diagnostic>> {
    let calls = collect_elision_calls(program);
    let mut diagnostics = Vec::new();
    for mut call in calls {
        let Some((callee_machine, callee_state)) = program.machines().iter().find_map(|machine| {
            program
                .machine_states(machine)
                .iter()
                .find(|state| state.symbol == call.target)
                .map(|state| (machine, state))
        }) else {
            continue;
        };
        let Some(caller_machine) = program
            .machines()
            .iter()
            .find(|machine| machine.symbol == call.caller_machine)
        else {
            continue;
        };
        let caller_state = program
            .machine_states(caller_machine)
            .iter()
            .find(|state| state.symbol == call.caller_state);

        let mut call_lifetimes = Vec::<(String, String)>::new();
        let mut conflicting_call_lifetime = false;
        let parameters = program.state_parameters(callee_state);
        let skip = parameters.len().saturating_sub(call.arguments.len());
        for (argument, parameter) in call.arguments.iter().zip(parameters.iter().skip(skip)) {
            let Some(actual) = psi_validation::declared_place_type_raw(
                program,
                caller_machine,
                caller_state,
                *argument,
            ) else {
                continue;
            };
            collect_lifetime_bindings(
                program,
                parameter.type_reference,
                actual,
                &callee_machine.lifetime_parameters,
                &mut call_lifetimes,
                &mut conflicting_call_lifetime,
            );
        }
        if conflicting_call_lifetime {
            diagnostics.push(Diagnostic::error(format!(
                "call to `{}` has conflicting ordinary borrow constraints for an erased lifetime",
                callee_state.name
            )));
            continue;
        }

        let machine_substitutions =
            machine_static_substitutions(program, callee_machine, &call.machine_arguments);
        let evidence_bounds = callee_machine
            .conformance_bounds
            .iter()
            .filter(|bound| bound.binder.is_some())
            .collect::<Vec<_>>();
        let mut evidence_index = 0usize;
        for selected in &mut call.machine_arguments {
            if !selected.symbol.is_valid()
                || !matches!(
                    program.symbols.get(selected.symbol).kind,
                    SymbolKind::Conformance | SymbolKind::ConformanceParameter
                )
            {
                continue;
            }
            let Some(bound) = evidence_bounds.get(evidence_index) else {
                break;
            };
            evidence_index += 1;
            if !matches!(
                program.symbols.get(selected.symbol).kind,
                SymbolKind::Conformance
            ) {
                continue;
            }
            let Some(conformance) = program
                .conformances()
                .iter()
                .find(|conformance| conformance.symbol == selected.symbol)
            else {
                continue;
            };
            if conformance.lifetime_parameters.is_empty() {
                continue;
            }
            let mut inferred = Vec::<(String, String)>::new();
            let mut conflict = false;
            for (declared, expected) in program
                .type_reference_table
                .type_reference_handles(conformance.arguments)
                .iter()
                .zip(&bound.arguments)
            {
                collect_conformance_lifetime_bindings(
                    program,
                    *declared,
                    *expected,
                    &conformance.lifetime_parameters,
                    &call_lifetimes,
                    &mut inferred,
                    &mut conflict,
                );
            }
            let explicit = selected.application.as_ref().and_then(|application| {
                (application.lifetime_arguments.len() == conformance.lifetime_parameters.len())
                    .then(|| {
                        application
                            .lifetime_arguments
                            .iter()
                            .map(|argument| argument.as_str().to_owned())
                            .collect::<Vec<_>>()
                    })
            });
            let resolved = explicit.or_else(|| {
                conformance
                    .lifetime_parameters
                    .iter()
                    .map(|parameter| {
                        inferred.iter().find_map(|(name, value)| {
                            (name == parameter.as_str()).then(|| value.clone())
                        })
                    })
                    .collect::<Option<Vec<_>>>()
                    .filter(|_| !conflict)
            });
            let Some(resolved) = resolved else {
                continue;
            };

            let conformance_substitutions = conformance_static_substitutions(
                program,
                conformance,
                selected
                    .application
                    .as_deref()
                    .map_or(&[], |app| &app.arguments),
            );
            let inferred_substitutions = conformance
                .lifetime_parameters
                .iter()
                .zip(&resolved)
                .map(|(parameter, argument)| (parameter.as_str().to_owned(), argument.clone()))
                .collect::<Vec<_>>();
            let shapes_match = program
                .type_reference_table
                .type_reference_handles(conformance.arguments)
                .iter()
                .zip(&bound.arguments)
                .all(|(declared, expected)| {
                    substituted_type_identity_with_lifetimes(
                        program,
                        *declared,
                        &conformance_substitutions,
                        &inferred_substitutions,
                    ) == substituted_type_identity_with_lifetimes(
                        program,
                        *expected,
                        &machine_substitutions,
                        &call_lifetimes,
                    )
                });
            if !shapes_match {
                if selected
                    .application
                    .as_ref()
                    .is_some_and(|application| !application.lifetime_arguments.is_empty())
                {
                    diagnostics.push(Diagnostic::error(format!(
                        "conformance `{}` supplies lifetime arguments that disagree with the call's ordinary borrow constraints",
                        conformance
                            .alias
                            .as_ref()
                            .map_or("<unnamed>", |name| name.as_str())
                    )));
                }
                continue;
            }
            if let Some(application) = &mut selected.application
                && application.lifetime_arguments.is_empty()
            {
                application.lifetime_arguments = resolved
                    .into_iter()
                    .map(Identifier::generated)
                    .collect::<Vec<_>>()
                    .into_boxed_slice();
            }
        }

        match call.site {
            ElisionCallSite::Expression(handle) => {
                let ExpressionNode::Call(current) = program.expression_table.expression_mut(handle)
                else {
                    continue;
                };
                current.machine_arguments = call.machine_arguments;
            }
            ElisionCallSite::Statement(handle) => {
                let StatementNode::Call(current) = program.statement_table.statement_mut(handle)
                else {
                    continue;
                };
                current.machine_arguments = call.machine_arguments;
            }
        }
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

fn collect_elision_calls(program: &TypedTrees) -> Vec<ElisionCall> {
    let mut calls = Vec::new();
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for offset in 0..state.statement_nodes.count() {
                let handle = psi_arena::Handle::from_parts(
                    state.statement_nodes.start().arena_index() + offset,
                    state.statement_nodes.start().generation(),
                );
                match program.statement_table.statement(handle) {
                    StatementNode::Call(call) => {
                        calls.push(ElisionCall {
                            site: ElisionCallSite::Statement(handle),
                            caller_machine: machine.symbol,
                            caller_state: state.symbol,
                            target: call.target_symbol,
                            arguments: program
                                .statement_table
                                .expression_handles(call.arguments)
                                .to_vec(),
                            machine_arguments: call.machine_arguments.clone(),
                        });
                        for argument in program.statement_table.expression_handles(call.arguments) {
                            collect_expression_elision_calls(
                                program,
                                machine.symbol,
                                state.symbol,
                                *argument,
                                &mut calls,
                            );
                        }
                    }
                    StatementNode::Expression(expression) => collect_expression_elision_calls(
                        program,
                        machine.symbol,
                        state.symbol,
                        *expression,
                        &mut calls,
                    ),
                    StatementNode::LocalData(local) => collect_expression_elision_calls(
                        program,
                        machine.symbol,
                        state.symbol,
                        local.initial_value,
                        &mut calls,
                    ),
                    StatementNode::Assignment(assignment) => {
                        collect_expression_elision_calls(
                            program,
                            machine.symbol,
                            state.symbol,
                            assignment.target,
                            &mut calls,
                        );
                        collect_expression_elision_calls(
                            program,
                            machine.symbol,
                            state.symbol,
                            assignment.value,
                            &mut calls,
                        );
                    }
                    StatementNode::AssemblyFact(fact) => collect_expression_elision_calls(
                        program,
                        machine.symbol,
                        state.symbol,
                        fact.expression,
                        &mut calls,
                    ),
                    _ => {}
                }
            }
        }
    }
    calls
}

fn collect_expression_elision_calls(
    program: &TypedTrees,
    caller_machine: SymbolHandle,
    caller_state: SymbolHandle,
    expression: ExpressionHandle,
    calls: &mut Vec<ElisionCall>,
) {
    if !expression.is_valid()
        || calls.iter().any(
            |call| matches!(call.site, ElisionCallSite::Expression(handle) if handle == expression),
        )
    {
        return;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Call(call) => {
            calls.push(ElisionCall {
                site: ElisionCallSite::Expression(expression),
                caller_machine,
                caller_state,
                target: call.target_symbol,
                arguments: program
                    .expression_table
                    .expression_handles(call.arguments)
                    .to_vec(),
                machine_arguments: call.machine_arguments.clone(),
            });
            collect_expression_elision_calls(
                program,
                caller_machine,
                caller_state,
                call.receiver,
                calls,
            );
            for argument in program.expression_table.expression_handles(call.arguments) {
                collect_expression_elision_calls(
                    program,
                    caller_machine,
                    caller_state,
                    *argument,
                    calls,
                );
            }
        }
        ExpressionNode::ArrayLiteral(values) => {
            for value in program.expression_table.expression_handles(*values) {
                collect_expression_elision_calls(
                    program,
                    caller_machine,
                    caller_state,
                    *value,
                    calls,
                );
            }
        }
        ExpressionNode::Atomic(atomic) => {
            collect_expression_elision_calls(
                program,
                caller_machine,
                caller_state,
                atomic.value,
                calls,
            );
            collect_expression_elision_calls(
                program,
                caller_machine,
                caller_state,
                atomic.result,
                calls,
            );
        }
        ExpressionNode::Binary(binary) => {
            collect_expression_elision_calls(
                program,
                caller_machine,
                caller_state,
                binary.left,
                calls,
            );
            collect_expression_elision_calls(
                program,
                caller_machine,
                caller_state,
                binary.right,
                calls,
            );
        }
        ExpressionNode::Cast(cast) => collect_expression_elision_calls(
            program,
            caller_machine,
            caller_state,
            cast.value,
            calls,
        ),
        ExpressionNode::Indexed(indexed) => {
            collect_expression_elision_calls(
                program,
                caller_machine,
                caller_state,
                indexed.collection,
                calls,
            );
            collect_expression_elision_calls(
                program,
                caller_machine,
                caller_state,
                indexed.index,
                calls,
            );
        }
        ExpressionNode::Member(member) => collect_expression_elision_calls(
            program,
            caller_machine,
            caller_state,
            member.receiver,
            calls,
        ),
        ExpressionNode::Borrow(inner) => collect_expression_elision_calls(
            program,
            caller_machine,
            caller_state,
            inner.target,
            calls,
        ),
        ExpressionNode::Range(range) => {
            collect_expression_elision_calls(
                program,
                caller_machine,
                caller_state,
                range.start,
                calls,
            );
            collect_expression_elision_calls(
                program,
                caller_machine,
                caller_state,
                range.end,
                calls,
            );
        }
        ExpressionNode::StructLiteral(literal) => {
            for field in program.expression_table.struct_fields(literal.fields) {
                collect_expression_elision_calls(
                    program,
                    caller_machine,
                    caller_state,
                    field.value,
                    calls,
                );
            }
        }
        ExpressionNode::Unary(unary) => collect_expression_elision_calls(
            program,
            caller_machine,
            caller_state,
            unary.operand,
            calls,
        ),
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => {}
    }
}

fn bind_lifetime(
    bindings: &mut Vec<(String, String)>,
    parameter: &str,
    argument: &str,
    conflict: &mut bool,
) {
    if let Some((_, existing)) = bindings.iter().find(|(name, _)| name == parameter) {
        if existing != argument {
            *conflict = true;
        }
    } else {
        bindings.push((parameter.to_owned(), argument.to_owned()));
    }
}

fn resolved_lifetime<'a>(bindings: &'a [(String, String)], name: &str) -> Option<&'a str> {
    bindings
        .iter()
        .rev()
        .find_map(|(parameter, argument)| (parameter == name).then_some(argument.as_str()))
}

fn collect_lifetime_bindings(
    program: &TypedTrees,
    required: TypeReferenceHandle,
    actual: TypeReferenceHandle,
    parameters: &[Identifier],
    bindings: &mut Vec<(String, String)>,
    conflict: &mut bool,
) {
    match (
        program.type_reference_table.type_reference(required),
        program.type_reference_table.type_reference(actual),
    ) {
        (
            TypeReferenceNode::Reference {
                referee: required_referee,
                lifetime: required_lifetime,
                ..
            },
            TypeReferenceNode::Reference {
                referee: actual_referee,
                lifetime: actual_lifetime,
                ..
            },
        ) => {
            if let (Some(required), Some(actual)) = (required_lifetime, actual_lifetime)
                && parameters
                    .iter()
                    .any(|parameter| parameter.as_str() == required.as_str())
            {
                bind_lifetime(bindings, required.as_str(), actual.as_str(), conflict);
            }
            collect_lifetime_bindings(
                program,
                *required_referee,
                *actual_referee,
                parameters,
                bindings,
                conflict,
            );
        }
        (
            TypeReferenceNode::Constrained {
                base_type: required,
                ..
            },
            TypeReferenceNode::Constrained {
                base_type: actual, ..
            },
        ) => collect_lifetime_bindings(program, *required, *actual, parameters, bindings, conflict),
        (TypeReferenceNode::Constrained { base_type, .. }, _) => {
            collect_lifetime_bindings(program, *base_type, actual, parameters, bindings, conflict)
        }
        (
            TypeReferenceNode::Generic {
                base_name: required_base,
                lifetime_arguments: required_lifetimes,
                arguments: required_arguments,
                ..
            },
            TypeReferenceNode::Generic {
                base_name: actual_base,
                lifetime_arguments: actual_lifetimes,
                arguments: actual_arguments,
                ..
            },
        ) if required_base == actual_base => {
            for (required, actual) in required_lifetimes.iter().zip(actual_lifetimes) {
                if parameters
                    .iter()
                    .any(|parameter| parameter.as_str() == required.as_str())
                {
                    bind_lifetime(bindings, required.as_str(), actual.as_str(), conflict);
                }
            }
            for (required, actual) in program
                .type_reference_table
                .type_reference_handles(*required_arguments)
                .iter()
                .zip(
                    program
                        .type_reference_table
                        .type_reference_handles(*actual_arguments),
                )
            {
                collect_lifetime_bindings(
                    program, *required, *actual, parameters, bindings, conflict,
                );
            }
        }
        (
            TypeReferenceNode::Slice {
                element_type: required,
            },
            TypeReferenceNode::Slice {
                element_type: actual,
            },
        )
        | (
            TypeReferenceNode::FixedArray {
                element_type: required,
                ..
            },
            TypeReferenceNode::FixedArray {
                element_type: actual,
                ..
            },
        ) => collect_lifetime_bindings(program, *required, *actual, parameters, bindings, conflict),
        _ => {}
    }
}

fn collect_conformance_lifetime_bindings(
    program: &TypedTrees,
    declared: TypeReferenceHandle,
    expected: TypeReferenceHandle,
    parameters: &[Identifier],
    expected_lifetimes: &[(String, String)],
    bindings: &mut Vec<(String, String)>,
    conflict: &mut bool,
) {
    match (
        program.type_reference_table.type_reference(declared),
        program.type_reference_table.type_reference(expected),
    ) {
        (
            TypeReferenceNode::Reference {
                referee: declared_referee,
                lifetime: declared_lifetime,
                ..
            },
            TypeReferenceNode::Reference {
                referee: expected_referee,
                lifetime: expected_lifetime,
                ..
            },
        ) => {
            if let (Some(declared), Some(expected)) = (declared_lifetime, expected_lifetime)
                && parameters
                    .iter()
                    .any(|parameter| parameter.as_str() == declared.as_str())
                && let Some(expected) = resolved_lifetime(expected_lifetimes, expected.as_str())
            {
                bind_lifetime(bindings, declared.as_str(), expected, conflict);
            }
            collect_conformance_lifetime_bindings(
                program,
                *declared_referee,
                *expected_referee,
                parameters,
                expected_lifetimes,
                bindings,
                conflict,
            );
        }
        (
            TypeReferenceNode::Constrained {
                base_type: declared,
                ..
            },
            TypeReferenceNode::Constrained {
                base_type: expected,
                ..
            },
        ) => collect_conformance_lifetime_bindings(
            program,
            *declared,
            *expected,
            parameters,
            expected_lifetimes,
            bindings,
            conflict,
        ),
        (
            TypeReferenceNode::Generic {
                base_name: declared_base,
                lifetime_arguments: declared_lifetimes,
                arguments: declared_arguments,
                ..
            },
            TypeReferenceNode::Generic {
                base_name: expected_base,
                lifetime_arguments: expected_arguments,
                arguments,
                ..
            },
        ) if declared_base == expected_base => {
            for (declared, expected) in declared_lifetimes.iter().zip(expected_arguments) {
                if parameters
                    .iter()
                    .any(|parameter| parameter.as_str() == declared.as_str())
                    && let Some(expected) = resolved_lifetime(expected_lifetimes, expected.as_str())
                {
                    bind_lifetime(bindings, declared.as_str(), expected, conflict);
                }
            }
            for (declared, expected) in program
                .type_reference_table
                .type_reference_handles(*declared_arguments)
                .iter()
                .zip(
                    program
                        .type_reference_table
                        .type_reference_handles(*arguments),
                )
            {
                collect_conformance_lifetime_bindings(
                    program,
                    *declared,
                    *expected,
                    parameters,
                    expected_lifetimes,
                    bindings,
                    conflict,
                );
            }
        }
        (
            TypeReferenceNode::Slice {
                element_type: declared,
            },
            TypeReferenceNode::Slice {
                element_type: expected,
            },
        )
        | (
            TypeReferenceNode::FixedArray {
                element_type: declared,
                ..
            },
            TypeReferenceNode::FixedArray {
                element_type: expected,
                ..
            },
        ) => collect_conformance_lifetime_bindings(
            program,
            *declared,
            *expected,
            parameters,
            expected_lifetimes,
            bindings,
            conflict,
        ),
        _ => {}
    }
}

fn machine_static_substitutions(
    program: &TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
    arguments: &[StaticMachineArgument],
) -> Vec<(SymbolHandle, String)> {
    let parameters = program.machine_type_parameters(machine);
    static_substitutions(program, parameters, arguments)
}

fn conformance_static_substitutions(
    program: &TypedTrees,
    conformance: &psi_typed_trees::trait_definition::Conformance,
    arguments: &[StaticMachineArgument],
) -> Vec<(SymbolHandle, String)> {
    static_substitutions(
        program,
        program.conformance_type_parameters(conformance),
        arguments,
    )
}

fn static_substitutions(
    program: &TypedTrees,
    parameters: &[psi_typed_trees::data::TypeParameter],
    arguments: &[StaticMachineArgument],
) -> Vec<(SymbolHandle, String)> {
    let mut substitutions = Vec::new();
    let mut type_index = 0usize;
    let mut const_index = 0usize;
    let type_parameters = parameters
        .iter()
        .filter(|parameter| matches!(parameter.kind, TypeParameterKind::Type))
        .collect::<Vec<_>>();
    let const_parameters = parameters
        .iter()
        .filter(|parameter| matches!(parameter.kind, TypeParameterKind::Const { .. }))
        .collect::<Vec<_>>();
    for argument in arguments {
        if argument.const_literal.is_some() {
            if let Some(parameter) = const_parameters.get(const_index) {
                substitutions.push((
                    parameter.symbol,
                    static_argument_identity(program, argument),
                ));
                const_index += 1;
            }
            continue;
        }
        if !argument.symbol.is_valid() {
            continue;
        }
        if matches!(
            program.symbols.get(argument.symbol).kind,
            SymbolKind::BuiltinType | SymbolKind::Data | SymbolKind::TypeParameter
        ) && let Some(parameter) = type_parameters.get(type_index)
        {
            substitutions.push((
                parameter.symbol,
                static_argument_identity(program, argument),
            ));
            type_index += 1;
        }
    }
    substitutions
}

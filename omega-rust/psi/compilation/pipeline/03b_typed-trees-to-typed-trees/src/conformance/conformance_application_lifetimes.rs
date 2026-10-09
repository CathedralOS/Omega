//! Contextual lifetime resolution for generic conformance applications.
//!
//! This module consumes ordinary parameter-position borrow constraints. It is
//! deliberately separate from application closure: absent, ambiguous, and
//! conflicting regions remain errors rather than declaration-based guesses.

use diagnostics::Diagnostic;
use symbols::{SymbolHandle, SymbolKind};
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode, StaticMachineArgument};
use typed_trees::name::Identifier;
use typed_trees::statement::{StatementHandle, StatementNode};
use typed_trees::type_identity::{
    AppliedTypeIdentityScope, IndexIdentityArgument, applied_type_identity,
};
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

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
        let Some((callee_machine, callee_state)) =
            validation::find_state_with_machine(program, call.target)
        else {
            continue;
        };
        let Some(caller_machine) = program.machine_by_symbol(call.caller_machine) else {
            continue;
        };
        // The caller state must resolve inside the caller machine itself; a
        // whole-program find could admit a same-named state stored elsewhere.
        let caller_state =
            validation::find_state_in_machine(program, call.caller_machine, call.caller_state);

        let mut call_lifetimes = Vec::<(String, String)>::new();
        let mut conflicting_call_lifetime = false;
        let parameters = program.state_parameters(callee_state);
        let skip = parameters.len().saturating_sub(call.arguments.len());
        for (argument, parameter) in call.arguments.iter().zip(parameters.iter().skip(skip)) {
            let Some(actual) = validation::declared_place_type_raw(
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

        let application = match call.site {
            ElisionCallSite::Expression(handle) => program
                .expression_table
                .call_node(handle)
                .map(|call| call.authored_machine_application),
            ElisionCallSite::Statement(handle) => program
                .statement_table
                .statement(handle)
                .as_call()
                .map(|call| call.authored_machine_application),
        };

        let evidence_bounds = callee_machine.conformance_bounds.iter().collect::<Vec<_>>();
        let mut evidence_index = 0usize;
        for argument_ordinal in 0..call.machine_arguments.len() {
            let mut selected = call.machine_arguments[argument_ordinal].clone();
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
            let Some(conformance) = program.conformance_by_symbol(selected.symbol) else {
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

            let machine_substitutions = static_substitutions(
                program.machine_type_parameters(callee_machine),
                &call.machine_arguments,
            );
            let conformance_substitutions = static_substitutions(
                program.conformance_type_parameters(conformance),
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
            let declared_arguments = program
                .type_reference_table
                .type_reference_handles(conformance.arguments);
            let shapes_match = declared_arguments.len() == bound.arguments.len()
                && declared_arguments
                    .iter()
                    .zip(&bound.arguments)
                    .all(|(declared, expected)| {
                        applied_type_identity(
                            program,
                            *declared,
                            call.caller_machine,
                            &[AppliedTypeIdentityScope {
                                declaration: conformance.symbol,
                                arguments: &conformance_substitutions,
                                lifetimes: &inferred_substitutions,
                            }],
                        )
                        .zip(applied_type_identity(
                            program,
                            *expected,
                            call.caller_machine,
                            &[AppliedTypeIdentityScope {
                                declaration: callee_machine.symbol,
                                arguments: &machine_substitutions,
                                lifetimes: &call_lifetimes,
                            }],
                        ))
                        .is_some_and(|(declared, expected)| declared == expected)
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
            call.machine_arguments[argument_ordinal] = selected;
        }

        if let Some(application) = application.filter(|application| application.is_valid()) {
            let lifetimes = call_lifetimes
                .iter()
                .map(|(parameter, argument)| {
                    typed_trees::static_applications::StaticMachineLifetimeBinding {
                        parameter: Identifier::generated(parameter),
                        argument: Identifier::generated(argument),
                    }
                })
                .collect::<Vec<_>>();
            program
                .expression_table
                .set_static_machine_application_lifetimes(application, &lifetimes);
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

fn collect_elision_calls(program: &dyn ::validation::CheckingInput) -> Vec<ElisionCall> {
    let mut calls = Vec::new();
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for handle in state.statement_nodes.handles() {
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
    program: &dyn ::validation::CheckingInput,
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
        ExpressionNode::Match(dispatch) => {
            program.expression_table.match_deep_all(dispatch, |handle| {
                collect_expression_elision_calls(
                    program,
                    caller_machine,
                    caller_state,
                    handle,
                    calls,
                );
                true
            });
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
        typed_trees::literal_expression_pattern!() => {}
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
    program: &dyn ::validation::CheckingInput,
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
        typed_trees::constrained_type_reference_pair!(required, actual) => {
            collect_lifetime_bindings(program, *required, *actual, parameters, bindings, conflict)
        }
        (TypeReferenceNode::Constrained { base_type, .. }, _) => {
            collect_lifetime_bindings(program, *base_type, actual, parameters, bindings, conflict)
        }
        (
            TypeReferenceNode::Generic {
                base_symbol: required_base,
                lifetime_arguments: required_lifetimes,
                arguments: required_arguments,
                ..
            },
            TypeReferenceNode::Generic {
                base_symbol: actual_base,
                lifetime_arguments: actual_lifetimes,
                arguments: actual_arguments,
                ..
            },
        ) if required_base.is_valid() && required_base == actual_base => {
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
    program: &dyn ::validation::CheckingInput,
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
                base_symbol: declared_base,
                lifetime_arguments: declared_lifetimes,
                arguments: declared_arguments,
                ..
            },
            TypeReferenceNode::Generic {
                base_symbol: expected_base,
                lifetime_arguments: expected_arguments,
                arguments,
                ..
            },
        ) if declared_base.is_valid() && declared_base == expected_base => {
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

fn static_substitutions<'argument>(
    parameters: &[typed_trees::data::TypeParameter],
    arguments: &'argument [StaticMachineArgument],
) -> Vec<(SymbolHandle, IndexIdentityArgument<'argument>)> {
    parameters
        .iter()
        .zip(arguments)
        .map(|(parameter, argument)| {
            (
                parameter.symbol,
                IndexIdentityArgument::StaticArgument(argument),
            )
        })
        .collect()
}

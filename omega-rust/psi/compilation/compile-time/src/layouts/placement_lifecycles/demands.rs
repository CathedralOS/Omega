//! Source-owned policy demand across ordinary specialization. An interned
//! callable tuple describes a signature, not an invocation or its requester.

use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::static_applications::{StaticMachineApplicationHandle, StaticMachineBindingValue};
use typed_trees::trait_definition::{AppliedTraitScope, TraitApplicationEnvironment};
use typed_trees::typed_trees::PlacementLifecycleOperation;
use typed_trees::types::{TypeOperandOrigin, TypeReferenceHandle, TypeReferenceNode};

pub(super) struct Demand {
    pub callable: SymbolHandle,
    pub operation: PlacementLifecycleOperation,
    pub policy: SymbolHandle,
    pub schema: SymbolHandle,
    pub sources: Vec<source::SourceSpan>,
}

struct OperationArguments {
    callable: SymbolHandle,
    operation: PlacementLifecycleOperation,
    policy: TypeReferenceHandle,
    schema: TypeReferenceHandle,
}

struct Invocation {
    owner: SymbolHandle,
    target: SymbolHandle,
    executable: SymbolHandle,
    application: StaticMachineApplicationHandle,
    source: source::SourceSpan,
    scope: Option<TraitApplicationEnvironment>,
}

pub(super) fn collect(program: &mut TypedTrees) -> Result<Vec<Demand>, Vec<Diagnostic>> {
    let calls = invocations(program);
    let mut demands: Vec<Demand> = Vec::new();
    for call in &calls {
        let Some(arguments) = operation_arguments(program, call)
            .map_err(|error| vec![error.with_source_span(call.source)])?
        else {
            continue;
        };
        let nominal = |reference| match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Named { symbol, .. } => Some(*symbol),
            _ => None,
        };
        let (Some(policy), Some(schema)) = (nominal(arguments.policy), nominal(arguments.schema))
        else {
            return Err(vec![
                Diagnostic::error(
                    "resident lifecycle policy and schema must be closed nominal declarations",
                )
                .with_source_span(call.source),
            ]);
        };
        let result = (|| {
            let mut sources = Vec::new();
            policy_sources(
                program,
                &calls,
                arguments.policy,
                call.source,
                &mut Vec::new(),
                &mut sources,
            )?;
            if sources.is_empty() {
                return Err("resident lifecycle policy has no supplying occurrence");
            }
            Ok(sources)
        })()
        .map_err(|message: &str| vec![Diagnostic::error(message).with_source_span(call.source)])?;
        if let Some(demand) = demands
            .iter_mut()
            .find(|demand| demand.callable == arguments.callable)
        {
            if demand.operation != arguments.operation
                || demand.policy != policy
                || demand.schema != schema
            {
                return Err(vec![
                    Diagnostic::error("resident lifecycle callable has competing closed operands")
                        .with_source_span(call.source),
                ]);
            }
            for source in result {
                if !demand.sources.contains(&source) {
                    demand.sources.push(source);
                }
            }
        } else {
            demands.push(Demand {
                callable: arguments.callable,
                operation: arguments.operation,
                policy,
                schema,
                sources: result,
            });
        }
    }
    Ok(demands)
}

/// Public requirements carry an applied trait scope; private execution leaves
/// carry an ordinary machine telescope. Each uses the actual invocation's
/// operands, not the first caller cached in its shared specialization.
fn operation_arguments(
    program: &mut TypedTrees,
    call: &Invocation,
) -> Result<Option<OperationArguments>, Diagnostic> {
    if let Some(operation) = super::core_lifecycle_operation(program, call.target) {
        let scope = call.scope.as_ref().ok_or_else(|| {
            Diagnostic::error("resident lifecycle call lost its occurrence-owned trait scope")
        })?;
        let arguments = program
            .materialize_trait_application_arguments(scope)
            .map_err(Diagnostic::error)?;
        let [policy, schema] = arguments.as_slice() else {
            return Err(Diagnostic::error(
                "resident lifecycle requires exact policy/schema operands",
            ));
        };
        return Ok(Some(OperationArguments {
            callable: call.target,
            operation,
            policy: *policy,
            schema: *schema,
        }));
    }
    let Some(contract) = typed_trees::typed_trees::PlacementExecutionDeclaration::authenticate(
        program,
        call.target,
    )?
    else {
        return Ok(None);
    };
    // A retained generic body is not an executable placement demand. Its
    // closed invocation supplies P/T and selects the policy when specialized.
    if program
        .machine(contract.machine())
        .is_some_and(|machine| program.callable_body_has_open_parameters(machine))
    {
        return Ok(None);
    }
    let (policy, schema) = contract.invocation_operands(program, call.owner, call.application)?;
    Ok(Some(OperationArguments {
        callable: contract.machine(),
        operation: contract.operation(),
        policy,
        schema,
    }))
}

fn declaring_environment(
    program: &TypedTrees,
    target: SymbolHandle,
    scope: Option<AppliedTraitScope<'_>>,
) -> Option<TraitApplicationEnvironment> {
    let closed = program.closed_requirement_application(target)?;
    scope?
        .declaring_scope(program, closed.declaring_trait)
        .map(|scope| scope.environment())
}

fn selected_scope<'program>(
    program: &'program TypedTrees,
    owner: SymbolHandle,
    dispatch: &typed_trees::typed_trees::StaticRequirementDispatch,
) -> Option<AppliedTraitScope<'program>> {
    let applications = program.selected_conformance_applications(
        owner,
        dispatch.selection_application,
        dispatch.requirement,
        dispatch.application_report_fingerprint,
        dispatch.application_commitment,
    );
    let [application] = applications.as_slice() else {
        return None;
    };
    application.trait_scope.scope(program)
}

fn invocations(program: &TypedTrees) -> Vec<Invocation> {
    let mut calls = Vec::new();
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for (position, _) in program
                .statement_table
                .indexed_statements(state.statement_nodes)
            {
                for site in
                    validation::collect_call_sites_in_statement(program, machine, state, position)
                {
                    match site {
                        validation::CallSite::Statement(call) => {
                            let target = call
                                .static_requirement_dispatch
                                .as_ref()
                                .map_or(call.target_symbol, |dispatch| dispatch.contract_symbol());
                            let scope = if let Some(dispatch) = &call.static_requirement_dispatch {
                                selected_scope(program, machine.symbol, dispatch)
                            } else {
                                validation::qualified_trait_scope(
                                    program,
                                    machine.symbol,
                                    call.receiver_symbol,
                                    validation::projected_statement_receiver_type_reference(
                                        program, state, position, call,
                                    ),
                                )
                            };
                            calls.push(Invocation {
                                owner: machine.symbol,
                                target,
                                executable: call.target_symbol,
                                application: call.authored_machine_application,
                                source: call.source_span,
                                scope: declaring_environment(program, target, scope),
                            });
                        }
                        validation::CallSite::Expression { expression, call } => {
                            let target = call
                                .static_requirement_dispatch
                                .as_ref()
                                .map_or(call.target_symbol, |dispatch| dispatch.contract_symbol());
                            let scope = if let Some(dispatch) = &call.static_requirement_dispatch {
                                selected_scope(program, machine.symbol, dispatch)
                            } else if call.requirement_application.is_valid() {
                                program.applied_requirement_scope(
                                    machine.symbol,
                                    call.requirement_application,
                                )
                            } else {
                                validation::qualified_trait_scope(
                                    program,
                                    machine.symbol,
                                    program
                                        .expression_table
                                        .name_path_node(call.receiver)
                                        .map_or_else(SymbolHandle::invalid, |path| path.symbol),
                                    validation::expression_type_reference_in_state(
                                        program,
                                        state.symbol,
                                        position,
                                        call.receiver,
                                    ),
                                )
                            };
                            calls.push(Invocation {
                                owner: machine.symbol,
                                target,
                                executable: call.target_symbol,
                                application: call.authored_machine_application,
                                source: program.expression_table.source_span(expression),
                                scope: declaring_environment(program, target, scope),
                            });
                        }
                        validation::CallSite::TransitionNamed {
                            path, source_span, ..
                        } => {
                            if !site.is_internal_state_transfer(program, machine.symbol) {
                                calls.push(Invocation {
                                    owner: machine.symbol,
                                    target: path.symbol,
                                    executable: path.symbol,
                                    application: StaticMachineApplicationHandle::invalid(),
                                    source: source_span,
                                    scope: None,
                                });
                            }
                        }
                    }
                }
            }
        }
    }
    calls
}

fn policy_sources(
    program: &TypedTrees,
    calls: &[Invocation],
    reference: TypeReferenceHandle,
    source: source::SourceSpan,
    visiting: &mut Vec<TypeOperandOrigin>,
    sources: &mut Vec<source::SourceSpan>,
) -> Result<(), &'static str> {
    // A substituted root owns its complete operand. Do not attribute its
    // children to the instance's first representative caller.
    let origin = program.type_reference_table.operand_origin(reference);
    if origin == TypeOperandOrigin::Lexical {
        if source.span.start >= source.span.end {
            return Err("policy operand has no authored invocation source");
        }
        if !sources.contains(&source) {
            sources.push(source);
        }
        return Ok(());
    }
    if visiting.contains(&origin) {
        return Ok(());
    }
    visiting.push(origin);
    let mut supplied = false;
    for call in calls {
        let parameter = match origin {
            TypeOperandOrigin::MachineParameter {
                instance,
                parameter,
            } => {
                let owner = validation::find_state(program, call.executable)
                    .map_or(call.executable, |(machine, _)| machine.symbol);
                if owner != instance {
                    continue;
                }
                parameter
            }
            TypeOperandOrigin::Lexical => unreachable!(),
        };
        if !call.application.is_valid() {
            return Err("supplying call lost its retained static application");
        }
        if program
            .expression_table
            .static_machine_application(call.application)
            .lexical_owner
            != call.owner
        {
            return Err("supplying static application belongs to a different caller");
        }
        let mut bindings = program
            .expression_table
            .static_machine_application_bindings(call.application)
            .iter()
            .filter(|binding| binding.parameter == parameter);
        let (Some(binding), None) = (bindings.next(), bindings.next()) else {
            return Err("supplying call lost its unique original type-parameter binding");
        };
        let StaticMachineBindingValue::Type(actual) = binding.value else {
            return Err("policy type parameter is supplied by a non-type operand");
        };
        supplied = true;
        policy_sources(program, calls, actual, call.source, visiting, sources)?;
    }
    visiting.pop();
    if !supplied {
        return Err("policy substitution has no retained supplying call");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use typed_trees::static_applications::StaticMachineBinding;
    use typed_trees::types::TypeReferenceNode;

    fn location(ordinal: usize) -> source::SourceSpan {
        source::SourceSpan {
            source_id: source::SourceId(ordinal),
            span: source::Span::new(1, 2),
        }
    }

    fn operand(program: &mut TypedTrees, origin: TypeOperandOrigin) -> TypeReferenceHandle {
        let reference = program.type_reference_table.insert(TypeReferenceNode::Unit);
        program
            .type_reference_table
            .set_operand_origin(reference, origin);
        reference
    }

    fn invocation(
        program: &mut TypedTrees,
        executable: SymbolHandle,
        parameter: SymbolHandle,
        actual: TypeReferenceHandle,
        source: source::SourceSpan,
    ) -> Invocation {
        let application = program.expression_table.retain_static_machine_application(
            executable,
            Default::default(),
            &[],
        );
        program
            .expression_table
            .set_static_machine_application_bindings(
                application,
                &[StaticMachineBinding {
                    parameter,
                    value: StaticMachineBindingValue::Type(actual),
                }],
            );
        Invocation {
            owner: SymbolHandle::invalid(),
            target: executable,
            executable,
            application,
            source,
            scope: None,
        }
    }

    #[test]
    fn shared_body_retains_every_supplying_requester() {
        let mut program = TypedTrees::default();
        let instance = SymbolHandle::from_parts(1, 1);
        let parameter = SymbolHandle::from_parts(2, 1);
        let closed = operand(&mut program, TypeOperandOrigin::Lexical);
        let policy = operand(
            &mut program,
            TypeOperandOrigin::MachineParameter {
                instance,
                parameter,
            },
        );
        let calls = [
            invocation(&mut program, instance, parameter, closed, location(1)),
            invocation(&mut program, instance, parameter, closed, location(2)),
        ];
        let mut sources = Vec::new();
        policy_sources(
            &program,
            &calls,
            policy,
            location(0),
            &mut Vec::new(),
            &mut sources,
        )
        .unwrap();
        assert_eq!(sources, vec![location(1), location(2)]);
    }

    #[test]
    fn hardcoded_policy_keeps_helper_custody() {
        let mut program = TypedTrees::default();
        let instance = SymbolHandle::from_parts(1, 1);
        let parameter = SymbolHandle::from_parts(2, 1);
        let policy = operand(&mut program, TypeOperandOrigin::Lexical);
        let calls = [
            invocation(&mut program, instance, parameter, policy, location(1)),
            invocation(&mut program, instance, parameter, policy, location(2)),
        ];
        let mut sources = Vec::new();
        policy_sources(
            &program,
            &calls,
            policy,
            location(0),
            &mut Vec::new(),
            &mut sources,
        )
        .unwrap();
        assert_eq!(sources, vec![location(0)]);
    }

    #[test]
    fn nested_wrapper_follows_original_binder_not_equal_closed_type() {
        let mut program = TypedTrees::default();
        let outer = SymbolHandle::from_parts(1, 1);
        let outer_parameter = SymbolHandle::from_parts(2, 1);
        let inner = SymbolHandle::from_parts(3, 1);
        let inner_parameter = SymbolHandle::from_parts(4, 1);
        let closed = operand(&mut program, TypeOperandOrigin::Lexical);
        let forwarded = operand(
            &mut program,
            TypeOperandOrigin::MachineParameter {
                instance: outer,
                parameter: outer_parameter,
            },
        );
        let policy = operand(
            &mut program,
            TypeOperandOrigin::MachineParameter {
                instance: inner,
                parameter: inner_parameter,
            },
        );
        let calls = [
            invocation(&mut program, outer, outer_parameter, closed, location(1)),
            invocation(&mut program, inner, inner_parameter, forwarded, location(2)),
            invocation(&mut program, inner, inner_parameter, closed, location(3)),
        ];
        let mut sources = Vec::new();
        policy_sources(
            &program,
            &calls,
            policy,
            location(0),
            &mut Vec::new(),
            &mut sources,
        )
        .unwrap();
        assert_eq!(sources, vec![location(1), location(3)]);
    }

    #[test]
    fn missing_supplier_cannot_borrow_the_instance_declaration() {
        let mut program = TypedTrees::default();
        let policy = operand(
            &mut program,
            TypeOperandOrigin::MachineParameter {
                instance: SymbolHandle::from_parts(1, 1),
                parameter: SymbolHandle::from_parts(2, 1),
            },
        );
        let result = policy_sources(
            &program,
            &[],
            policy,
            location(0),
            &mut Vec::new(),
            &mut Vec::new(),
        );
        assert_eq!(
            result,
            Err("policy substitution has no retained supplying call")
        );
    }

    #[test]
    fn generated_call_cannot_borrow_the_first_source_package() {
        let mut program = TypedTrees::default();
        let policy = operand(&mut program, TypeOperandOrigin::Lexical);
        let result = policy_sources(
            &program,
            &[],
            policy,
            Default::default(),
            &mut Vec::new(),
            &mut Vec::new(),
        );
        assert_eq!(
            result,
            Err("policy operand has no authored invocation source")
        );
    }
}

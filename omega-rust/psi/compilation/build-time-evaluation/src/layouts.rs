/*
    Compile-time evaluation and installation of `FixedLayout` plans,
    `LayoutStorage` applications, access views, access plans, access lifecycles,
    and reflected type schemas.

    We reflect checked type schemas into canonical `Schema` values, evaluate and
    validate authored layout and access policies across target scopes, and
    install validated geometry and access views into the typed program.
*/

pub(crate) mod access_lifecycles {
    use diagnostics::Diagnostic;
    use symbols::SymbolHandle;
    use typed_trees::typed_trees::{AccessLifecycleOperation, AccessLifecyclePlan};
    use typed_trees::{FrozenTypedTrees, TypedTrees};

    use crate::machine_execution::admission::{BuildTimeAdmissionPlan, BuildTimeInvocationCustody};
    use language_semantics::Multiplicity;

    mod demands {
        use crate::build_time_evaluation::expression_table;
        use crate::unique::{extend_unique, push_unique};
        use diagnostics::Diagnostic;
        use symbols::SymbolHandle;
        use typed_trees::TypedTrees;
        use typed_trees::static_applications::{
            StaticMachineApplicationHandle, StaticMachineBindingValue,
        };
        use typed_trees::trait_definition::{AppliedTraitScope, TraitApplicationEnvironment};
        use typed_trees::typed_trees::{AccessExecutionDeclaration, AccessLifecycleOperation};
        use typed_trees::types::{TypeOperandOrigin, TypeReferenceHandle, TypeReferenceNode};
        use validation::collect_call_sites_in_statement;

        pub(super) struct Demand {
            pub callable: SymbolHandle,
            pub operation: AccessLifecycleOperation,
            pub policy: SymbolHandle,
            pub schema: SymbolHandle,
            pub sources: Vec<source::SourceSpan>,
            pub owners: Vec<SymbolHandle>,
        }

        struct OperationArguments {
            callable: SymbolHandle,
            operation: AccessLifecycleOperation,
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
                if arguments.operation == AccessLifecycleOperation::Relocate
                    && validation::effective_type_carry_policy(program, &[], arguments.schema)
                        .address
                        != language_semantics::CarryAddress::Movable
                {
                    return Err(vec![
                Diagnostic::error(
                    "run relocation requires address: movable for the retained element type",
                )
                .with_source_span(call.source),
            ]);
                }
                let nominal =
                    |reference| match program.type_reference_table.type_reference(reference) {
                        TypeReferenceNode::Named { symbol, .. } => Some(*symbol),
                        _ => None,
                    };
                let schema_carrier = program
                    .type_reference_table
                    .unconstrained_type_reference(arguments.schema);
                if arguments.operation == AccessLifecycleOperation::View
                    && schema_carrier != arguments.schema
                {
                    return Err(vec![
                Diagnostic::error(
                    "nonresident access View cannot establish schema qualifications; initialization or existing resident custody is required",
                )
                .with_source_span(call.source),
            ]);
                }
                let (Some(policy), Some(schema)) =
                    (nominal(arguments.policy), nominal(schema_carrier))
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
                .map_err(|message: &str| {
                    vec![Diagnostic::error(message).with_source_span(call.source)]
                })?;
                if let Some(demand) = demands
                    .iter_mut()
                    .find(|demand| demand.callable == arguments.callable)
                {
                    if demand.operation != arguments.operation
                        || demand.policy != policy
                        || demand.schema != schema
                    {
                        return Err(vec![
                            Diagnostic::error(
                                "resident lifecycle callable has competing closed operands",
                            )
                            .with_source_span(call.source),
                        ]);
                    }
                    extend_unique(&mut demand.sources, result);
                    push_unique(&mut demand.owners, call.owner);
                } else {
                    demands.push(Demand {
                        callable: arguments.callable,
                        operation: arguments.operation,
                        policy,
                        schema,
                        sources: result,
                        owners: vec![call.owner],
                    });
                }
            }
            Ok(demands)
        }

        fn operation_arguments(
            program: &mut TypedTrees,
            call: &Invocation,
        ) -> Result<Option<OperationArguments>, Diagnostic> {
            if let Some(operation) = super::core_lifecycle_operation(program, call.target) {
                let scope = call.scope.as_ref().ok_or_else(|| {
                    Diagnostic::error(
                        "resident lifecycle call lost its occurrence-owned trait scope",
                    )
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
            let Some(contract) = AccessExecutionDeclaration::authenticate(program, call.target)?
            else {
                return Ok(None);
            };
            if program
                .machine(contract.machine())
                .is_some_and(|machine| program.callable_body_has_open_parameters(machine))
            {
                return Ok(None);
            }
            let (policy, schema) =
                contract.invocation_operands(program, call.owner, call.application)?;
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
                dispatch.evidence_parameter,
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
                            collect_call_sites_in_statement(program, machine, state, position)
                        {
                            match site {
                                validation::CallSite::Statement(call) => {
                                    let target = call
                                        .static_requirement_dispatch
                                        .as_ref()
                                        .map_or(call.target_symbol, |dispatch| {
                                            dispatch.contract_symbol()
                                        });
                                    let scope =
                                        if let Some(dispatch) = &call.static_requirement_dispatch {
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
                                        .map_or(call.target_symbol, |dispatch| {
                                            dispatch.contract_symbol()
                                        });
                                    let scope =
                                        if let Some(dispatch) = &call.static_requirement_dispatch {
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
                                                expression_table(program)
                                                    .name_path_node(call.receiver)
                                                    .map_or_else(SymbolHandle::invalid, |path| {
                                                        path.symbol
                                                    }),
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
            let origin = program.type_reference_table.operand_origin(reference);
            if origin == TypeOperandOrigin::Lexical {
                if source.span.start >= source.span.end {
                    return Err("policy operand has no authored invocation source");
                }
                push_unique(sources, source);
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
                if expression_table(program)
                    .static_machine_application(call.application)
                    .lexical_owner
                    != call.owner
                {
                    return Err("supplying static application belongs to a different caller");
                }
                let mut bindings = expression_table(program)
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

            fn sources_for(
                program: &TypedTrees,
                calls: &[Invocation],
                policy: TypeReferenceHandle,
            ) -> Vec<source::SourceSpan> {
                let mut sources = Vec::new();
                policy_sources(
                    program,
                    calls,
                    policy,
                    location(0),
                    &mut Vec::new(),
                    &mut sources,
                )
                .unwrap();
                sources
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
                assert_eq!(
                    sources_for(&program, &calls, policy),
                    vec![location(1), location(2)]
                );
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
                assert_eq!(sources_for(&program, &calls, policy), vec![location(0)]);
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
                assert_eq!(
                    sources_for(&program, &calls, policy),
                    vec![location(1), location(3)]
                );
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
    }
    mod storage_contract {
        use diagnostics::Diagnostic;
        use symbols::SymbolHandle;
        use typed_trees::TypedTrees;
        use typed_trees::data::{DataDefinition, DataField, DataMember};
        use typed_trees::typed_trees::{
            AccessContentShape, AccessExecutionDeclaration, AccessLifecycleOperation,
            AccessLifecycleOwner, AccessLifecycleStorage,
        };
        use typed_trees::types::TypeReferenceHandle;

        pub(super) fn profile_carrier(
            program: &TypedTrees,
            callable: SymbolHandle,
        ) -> Result<Option<TypeReferenceHandle>, Diagnostic> {
            let Some(declaration) = AccessExecutionDeclaration::authenticate(program, callable)?
            else {
                return Ok(None);
            };
            let state = program
                .state_by_symbol(declaration.state())
                .ok_or_else(|| Diagnostic::error("placement primitive entry is absent"))?;
            let parameters = program.state_parameters(state);
            let positions: &[usize] = match declaration.operation() {
                AccessLifecycleOperation::Initialize => &[2],
                AccessLifecycleOperation::MoveOut | AccessLifecycleOperation::View => &[1],
                AccessLifecycleOperation::Relocate => &[2, 3],
            };
            let mut carrier = None;
            for position in positions {
                let parameter = parameters.get(*position).ok_or_else(|| {
                    Diagnostic::error("placement primitive profile input is absent")
                })?;
                let typed_trees::types::TypeReferenceNode::Reference { referee, .. } = program
                    .type_reference_table
                    .type_reference(parameter.type_reference)
                else {
                    return Err(Diagnostic::error(
                        "placement primitive must borrow its original profile result",
                    ));
                };
                if program.closed_data_definition(*referee).is_none()
                    || carrier.is_some_and(|previous| {
                        program.normalized_type_identity(previous)
                            != program.normalized_type_identity(*referee)
                    })
                {
                    return Err(Diagnostic::error(
                        "placement primitive profiles must have one closed core carrier",
                    ));
                }
                carrier = Some(*referee);
            }
            Ok(carrier)
        }

        pub(super) fn check(
            program: &TypedTrees,
            callable: SymbolHandle,
            operation: AccessLifecycleOperation,
        ) -> Result<AccessLifecycleStorage, Diagnostic> {
            let execution = AccessExecutionDeclaration::authenticate(program, callable)?;
            let (parameters, result) = if let Some(application) =
                program.closed_requirement_application(callable)
            {
                (
                    program.state_signature_parameters(application.signature),
                    application.signature.return_type,
                )
            } else {
                let execution = execution.ok_or_else(|| {
                    Diagnostic::error("placement storage has no authenticated execution contract")
                })?;
                let state = program
                    .state_by_symbol(execution.state())
                    .ok_or_else(|| Diagnostic::error("placement execution lost its entry"))?;
                (program.state_parameters(state), state.return_type)
            };
            let parameter = parameters
                .first()
                .ok_or_else(|| Diagnostic::error("placement storage lost its source parameter"))?;
            let run = execution
                .is_some_and(|declaration| declaration.content() == AccessContentShape::Run);
            let (owner, occupied, extent_reference) = if run {
                let reference = match operation {
                    AccessLifecycleOperation::Initialize => {
                        let outcome = definition(program, result)?;
                        let ready = program
                            .data_members(outcome)
                            .iter()
                            .find_map(|member| match member {
                                DataMember::Variant(variant) => Some(variant),
                                _ => None,
                            })
                            .ok_or_else(|| {
                                Diagnostic::error("typed run initialization lost its Ready case")
                            })?;
                        program
                            .data_payload_fields(ready)
                            .first()
                            .ok_or_else(|| {
                                Diagnostic::error("typed run initialization lost its owner payload")
                            })?
                            .type_reference
                    }
                    AccessLifecycleOperation::MoveOut | AccessLifecycleOperation::Relocate => {
                        parameter.type_reference
                    }
                    AccessLifecycleOperation::View => {
                        return Err(Diagnostic::error(
                            "typed run is not a borrowed placement operation",
                        ));
                    }
                };
                let data = definition(program, reference)?;
                let storage = field(program, data, "storage")?;
                let count = field(program, data, "count")?;
                let domains = validation::field_domains::declared_domain_constraint_records(
                    program, reference,
                );
                let domain = |name: &str| {
                    let matches = domains
                        .iter()
                        .filter(|row| {
                            program.domain_definition(row.symbol).is_some_and(|domain| {
                                domain.name.as_str().rsplit("::").next() == Some(name)
                            })
                        })
                        .map(|row| row.semantic_id)
                        .collect::<Vec<_>>();
                    match matches.as_slice() {
                        [domain] => Ok(*domain),
                        _ => Err(Diagnostic::error(
                            "typed run lost its exact content qualification",
                        )),
                    }
                };
                let extent_reference = if matches!(
                    operation,
                    AccessLifecycleOperation::MoveOut | AccessLifecycleOperation::Relocate
                ) {
                    storage.type_reference
                } else {
                    parameter.type_reference
                };
                (
                    AccessLifecycleOwner::Run {
                        carrier: program
                            .type_reference_table
                            .peeled_type_reference(reference),
                        storage,
                        count,
                        singleton: if operation == AccessLifecycleOperation::Initialize {
                            Some(domain("Singleton")?)
                        } else {
                            None
                        },
                    },
                    Some(domain("Live")?),
                    extent_reference,
                )
            } else {
                (AccessLifecycleOwner::Extent, None, parameter.type_reference)
            };
            let extent = definition(program, extent_reference)?;
            if !validation::is_core_extent(program, extent.symbol) {
                return Err(Diagnostic::error(
                    "placement storage is not the core extent",
                ));
            }
            let domains = validation::field_domains::declared_type_domain_rows(
                program,
                parameter.type_reference,
            )
            .into_iter()
            .chain(validation::field_domains::declared_type_domain_rows(
                program, result,
            ))
            .collect::<Vec<_>>();
            let domain = |name: &str| {
                let mut matches = domains
                    .iter()
                    .filter(|row| {
                        validation::is_core_extent_domain(program, row.domain_symbol, name)
                    })
                    .map(|row| row.semantic_domain)
                    .collect::<Vec<_>>();
                matches.sort_unstable_by_key(|domain| domain.0);
                matches.dedup();
                match matches.as_slice() {
                    [domain] => Ok(*domain),
                    _ => Err(Diagnostic::error(
                        "placement extent lost an exact source qualification",
                    )),
                }
            };
            let owns_content = operation != AccessLifecycleOperation::View;
            Ok(AccessLifecycleStorage {
                carrier: program
                    .type_reference_table
                    .peeled_type_reference(extent_reference),
                base: field(program, extent, "base")?,
                length: field(program, extent, "length")?,
                granted: domain("Granted")?,
                vacant: if owns_content {
                    Some(domain("Vacant")?)
                } else {
                    None
                },
                occupied: if owns_content {
                    Some(match occupied {
                        Some(domain) => domain,
                        None => domain("Resident")?,
                    })
                } else {
                    None
                },
                owner,
            })
        }

        fn definition(
            program: &TypedTrees,
            reference: TypeReferenceHandle,
        ) -> Result<&DataDefinition, Diagnostic> {
            let reference = program
                .type_reference_table
                .peeled_type_reference(reference);
            program
                .data_application_definition(reference)
                .or_else(|| program.closed_data_definition(reference))
                .ok_or_else(|| {
                    Diagnostic::error("placement storage lost its exact applied declaration")
                })
        }

        fn field(
            program: &TypedTrees,
            data: &DataDefinition,
            name: &str,
        ) -> Result<DataField, Diagnostic> {
            program
                .data_members(data)
                .iter()
                .filter_map(DataMember::as_field)
                .find(|field| field.name.as_str() == name)
                .cloned()
                .ok_or_else(|| {
                    Diagnostic::error(format!(
                        "placement storage lost its authenticated {name} field"
                    ))
                })
        }
    }

    pub fn retain_access_lifecycle_plans(
        program: FrozenTypedTrees,
        authority: Option<&dyn crate::BuildTimeSelectionAuthority>,
        scopes: &[crate::FixedLayoutTargetScope],
    ) -> Result<FrozenTypedTrees, Vec<Diagnostic>> {
        let mut program = program.into_inner();
        let demands = demands::collect(&mut program)?;
        if demands.is_empty() {
            return Ok(program.into());
        }
        let admission = BuildTimeAdmissionPlan::infer(&program, None)?;
        let checking = super::plan_laid::checking_target_applications(&program, scopes)?;
        for demand in demands {
            let source = demand.sources[0];
            let diagnostic = |reason| vec![Diagnostic::error(reason).with_source_span(source)];
            let policy = demand.policy;
            let schema = demand.schema;
            let mut policies = program.machines().iter().filter(|machine| {
                machine.attached_data_symbol == policy
                    && machine.name.as_str().rsplit("::").next() == Some("plan")
            });
            let (Some(policy_machine), None) = (policies.next(), policies.next()) else {
                return Err(diagnostic(
                    "resident lifecycle policy must select one attached plan machine".into(),
                ));
            };
            for source in &demand.sources {
                admit(
                    &program,
                    &admission,
                    policy_machine.symbol,
                    *source,
                    authority,
                )?;
            }
            let policy_machine_symbol = policy_machine.symbol;
            let schema_definition = program.data_definition_by_symbol(schema).ok_or_else(|| {
                diagnostic("resident lifecycle lost its schema declaration".into())
            })?;
            if demand.operation == AccessLifecycleOperation::View {
                validate_nonresident_schema(&program, schema).map_err(diagnostic)?;
            }
            let mut targets = Vec::new();
            if scopes.is_empty() {
                targets.push(None);
            } else {
                for (scope, selected) in scopes.iter().zip(&checking) {
                    if !demand.owners.iter().any(|owner| selected.contains(*owner)) {
                        continue;
                    }
                    if let Some(facts) = &scope.facts {
                        facts
                            .validate()
                            .map_err(|reason| diagnostic(reason.into()))?;
                        if facts.profile != scope.target {
                            return Err(diagnostic(
                                "lifecycle scope differs from its target geometry".into(),
                            ));
                        }
                    }
                    if !targets.contains(&scope.facts) {
                        targets.push(scope.facts.clone());
                    }
                }
            }
            if targets.is_empty() {
                return Err(diagnostic(
                    "lifecycle invocation has no source checking context".into(),
                ));
            }
            let storage = storage_contract::check(&program, demand.callable, demand.operation)
                .map_err(|error| vec![error.with_source_span(source)])?;
            let profile_carrier = storage_contract::profile_carrier(&program, demand.callable)
                .map_err(|error| vec![error.with_source_span(source)])?;
            let mut placements = Vec::new();
            for target in targets {
                let retained_placement = program
                    .access_view_plans
                    .iter()
                    .find(|view| {
                        view.policy_symbol == policy
                            && view.schema_symbol == schema
                            && view.policy_plan_machine_symbol == policy_machine_symbol
                    })
                    .map(|view| &view.placement)
                    .or_else(|| {
                        program
                            .access_lifecycle_plans
                            .iter()
                            .find_map(|(_, row)| match row {
                                AccessLifecyclePlan::Evaluated {
                                    policy: retained_policy,
                                    schema: retained_schema,
                                    policy_machine: retained_machine,
                                    placement,
                                    target: retained_target,
                                    ..
                                } if *retained_policy == policy
                                    && *retained_schema == schema
                                    && *retained_machine == policy_machine_symbol =>
                                {
                                    (*retained_target == target).then_some(placement)
                                }
                                _ => None,
                            })
                    });
                let placement = match retained_placement {
                    Some(placement) => placement.clone(),
                    None => super::access_plans::compute_selected_access_plan(
                        &program,
                        policy_machine,
                        schema_definition,
                        target.as_ref(),
                        &Default::default(),
                    )
                    .map_err(diagnostic)?,
                };
                placements.push((target, placement));
            }
            let invocations = program
                .access_policy_invocations
                .insert_many(demand.sources);
            for (target, placement) in placements {
                program
                    .access_lifecycle_plans
                    .append(AccessLifecyclePlan::Evaluated {
                        callable: demand.callable,
                        operation: demand.operation,
                        policy,
                        schema,
                        policy_machine: policy_machine_symbol,
                        invocations,
                        target,
                        placement,
                        storage: storage.clone(),
                        profile_carrier,
                    });
            }
        }
        Ok(program.into())
    }

    fn validate_nonresident_schema(
        program: &TypedTrees,
        schema: SymbolHandle,
    ) -> Result<(), String> {
        let mut records = vec![schema];
        let mut visited = Vec::new();
        while let Some(symbol) = crate::worklist::unvisited(&mut records, &mut visited) {
            let Some(data) = program.data_definition_by_symbol(symbol) else {
                continue;
            };
            if !data.requires_facts.is_empty() || data.zero_gated {
                return Err(format!(
                    "nonresident access View cannot establish record predicates for `{}`; independent content-validity evidence is required",
                    program.symbols.name(symbol),
                ));
            }
            let mut represented_types = Vec::new();
            for field in program.live_data_fields(program.data_members(data)) {
                if program.type_multiplicity(field.type_reference) != Multiplicity::Unrestricted {
                    return Err(format!(
                        "nonresident access View cannot acquire non-copy field `{}`; initialization or existing resident custody is required",
                        field.name,
                    ));
                }
                represented_types.push(field.type_reference);
            }
            while let Some(type_reference) = represented_types.pop() {
                use typed_trees::types::TypeReferenceNode;
                match program.type_reference_table.type_reference(type_reference) {
                    TypeReferenceNode::Named { symbol, .. } => records.push(*symbol),
                    TypeReferenceNode::FixedArray { element_type, .. } => {
                        represented_types.push(*element_type);
                    }
                    TypeReferenceNode::Constrained { .. } => {
                        return Err("nonresident access View cannot establish field qualifications; independent content-validity evidence is required".into());
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }

    pub fn admit_access_lifecycle_plans(
        program: &TypedTrees,
        authority: Option<&dyn crate::BuildTimeSelectionAuthority>,
    ) -> Result<(), Vec<Diagnostic>> {
        if program
            .access_lifecycle_plans
            .iter()
            .all(|(_, row)| matches!(row, AccessLifecyclePlan::Absent))
        {
            return Ok(());
        }
        let admission = BuildTimeAdmissionPlan::infer(program, None)?;
        for (_, row) in program.access_lifecycle_plans.iter() {
            let AccessLifecyclePlan::Evaluated {
                policy_machine,
                invocations,
                ..
            } = row
            else {
                continue;
            };
            let sources = program
                .access_policy_invocations
                .span_or_empty(*invocations);
            if sources.is_empty() || sources.len() != invocations.len() {
                return Err(vec![Diagnostic::error(
                    "resident lifecycle plan lost its supplying invocations",
                )]);
            }
            for source in sources {
                admit(program, &admission, *policy_machine, *source, authority)?;
            }
        }
        Ok(())
    }

    fn admit(
        program: &TypedTrees,
        admission: &BuildTimeAdmissionPlan,
        policy: SymbolHandle,
        source: source::SourceSpan,
        authority: Option<&dyn crate::BuildTimeSelectionAuthority>,
    ) -> Result<(), Vec<Diagnostic>> {
        let machine = program.machine(policy).ok_or_else(|| {
            vec![
                Diagnostic::error("resident lifecycle lost its policy machine")
                    .with_source_span(source),
            ]
        })?;
        admission
            .require_invocation_with_authority(
                program,
                machine,
                BuildTimeInvocationCustody::Source(source),
                authority,
            )
            .map_err(|rejection| {
                vec![
                    Diagnostic::error(rejection.reason)
                        .with_source_span(rejection.source_span.unwrap_or(source)),
                ]
            })
    }

    fn core_lifecycle_operation(
        program: &TypedTrees,
        target: SymbolHandle,
    ) -> Option<AccessLifecycleOperation> {
        let application = program.closed_requirement_application(target)?;
        if !is_core_layout_declaration(program, application.declaring_trait)
            || program.symbols.name(application.declaring_trait) != "AccessLifecycle"
            || !is_core_layout_declaration(program, application.template)
        {
            return None;
        }
        match program
            .symbols
            .name(application.template)
            .rsplit("::")
            .next()?
        {
            "initialize" => Some(AccessLifecycleOperation::Initialize),
            "move_out" => Some(AccessLifecycleOperation::MoveOut),
            _ => None,
        }
    }

    fn is_core_layout_declaration(program: &TypedTrees, symbol: SymbolHandle) -> bool {
        program
            .symbols
            .symbol_source_span(symbol)
            .and_then(|span| program.symbols.source_file(span))
            .is_some_and(|source| {
                symbols::core_source_at(source)
                    == Some(language_core::core_vocabulary::CoreSource::Layout)
            })
    }
}
pub(crate) mod access_plans {
    use crate::evaluation_requests::BuildTimeOperationEvaluation;
    use build_time_values::BuildTimeValue;
    use typed_trees::TypedTrees;

    use crate::BuildTimeAdmissionPlan;

    mod access_value {
        use std::collections::BTreeSet;

        use access_plans::{
            AccessExposure, AccessFields, AccessPermissions, AccessPlan, AtomicPermissions,
            BoundaryReach, BoundaryServiceReachId, ExternalRead, FieldAccess, ObservationModel,
            ValidatedAccessFields, ValidatedAccessPlan, validate_access_fields,
            validate_access_plan,
        };
        use build_time_values::BuildTimeValue;
        use layout_plans::{LayoutPlacementReport, LayoutPlanReport};
        use typed_trees::TypedTrees;

        use crate::layouts::layout_plans::validate_plan;
        use crate::layouts::schema_reflection::{
            SchemaFieldInfo, build_selected_schema_value, schema_fields_for_target,
        };

        use super::evaluate_selected_policy;

        const ACCESS_FIELD_CAPACITY: usize = 32;
        const BOUNDARY_REACH_CAPACITY: usize = 32;

        pub(crate) fn compute_selected_access_plan(
            typed: &TypedTrees,
            machine: &typed_trees::machine::Machine,
            schema: &typed_trees::data::DataDefinition,
            target: Option<&layout_plans::LayoutTargetFacts>,
            accounting: &crate::evaluation_accounting::EvaluationAccounting,
        ) -> Result<ValidatedAccessPlan, String> {
            let policy_machine = machine.name.as_str();
            let (schema_fields, schema_report_fingerprint) =
                schema_fields_for_target(typed, schema, target)?;
            let schema_value = build_selected_schema_value(typed, schema, &schema_fields)?;
            let plan =
                evaluate_selected_policy(typed, machine, vec![schema_value], "access", accounting)?;
            let fields = struct_fields(&plan).map_err(|reason| invalid(policy_machine, reason))?;
            let layout_value =
                named_field(fields, "layout").map_err(|reason| invalid(policy_machine, reason))?;
            let layout = validate_plan(
                layout_value,
                &schema_fields,
                schema_report_fingerprint,
                policy_machine,
            )?;
            let access = validate_access_value(fields, &schema_fields, &layout, policy_machine)?;
            let reach_value =
                named_field(fields, "reach").map_err(|reason| invalid(policy_machine, reason))?;
            let reach = parse_boundary_reach(reach_value)
                .map_err(|reason| invalid(policy_machine, reason))?;

            validate_access_plan(AccessPlan {
                layout,
                access: access.plan().clone(),
                reach,
            })
            .map_err(|diagnostic| invalid(policy_machine, diagnostic.to_string()))
        }

        pub(super) fn validate_access_value(
            fields: &[(String, BuildTimeValue)],
            schema_fields: &[SchemaFieldInfo],
            layout: &LayoutPlanReport,
            policy_machine: &str,
        ) -> Result<ValidatedAccessFields, String> {
            let field_count = uint_value(
                named_field(fields, "field_count")
                    .map_err(|reason| invalid(policy_machine, reason))?,
                "field_count",
            )
            .map_err(|reason| invalid(policy_machine, reason))?;
            if field_count != schema_fields.len() as u64 {
                return Err(invalid(
                    policy_machine,
                    format!(
                        "access field_count is {field_count}, but reflected schema requires exactly {} decisions",
                        schema_fields.len()
                    ),
                ));
            }
            if field_count > ACCESS_FIELD_CAPACITY as u64 {
                return Err(invalid(
                    policy_machine,
                    format!(
                        "access field_count {field_count} exceeds bootstrap capacity {ACCESS_FIELD_CAPACITY}"
                    ),
                ));
            }
            let entries = array_value(
                named_field(fields, "fields").map_err(|reason| invalid(policy_machine, reason))?,
                "fields",
            )
            .map_err(|reason| invalid(policy_machine, reason))?;
            if entries.len() < field_count as usize {
                return Err(invalid(
                    policy_machine,
                    format!(
                        "access field_count is {field_count}, but the plan carries only {} field decisions",
                        entries.len()
                    ),
                ));
            }

            let mut normalized =
                AccessFields::inaccessible(layout).map_err(|diagnostic| diagnostic.to_string())?;
            let mut seen = BTreeSet::new();
            for (entry_index, value) in entries.iter().take(field_count as usize).enumerate() {
                let entry_fields = struct_fields(value).map_err(|reason| {
                    invalid(
                        policy_machine,
                        format!("access entry {entry_index} {reason}"),
                    )
                })?;
                let key = uint_value(
                    named_field(entry_fields, "key")
                        .map_err(|reason| invalid(policy_machine, reason))?,
                    "key",
                )
                .map_err(|reason| invalid(policy_machine, reason))?;
                let Some(schema_field) = schema_fields.iter().find(|field| field.key == key) else {
                    return Err(invalid(
                        policy_machine,
                        format!(
                            "access entry {entry_index} refers to unknown schema field key {key}"
                        ),
                    ));
                };
                if !seen.insert(key) {
                    return Err(invalid(
                        policy_machine,
                        format!(
                            "access plan contains more than one decision for field `{}`",
                            schema_field.name
                        ),
                    ));
                }
                let source_access = named_field(entry_fields, "access")
                    .map_err(|reason| invalid(policy_machine, reason))?;
                let access = parse_field_access(source_access, schema_field, layout)
                    .map_err(|reason| invalid(policy_machine, reason))?;
                let normalized_key = normalized
                    .entries()
                    .iter()
                    .find(|entry| entry.field() == schema_field.name)
                    .map(|entry| entry.key())
                    .ok_or_else(|| {
                        invalid(
                            policy_machine,
                            format!(
                                "validated layout has no canonical access slot for field `{}`",
                                schema_field.name
                            ),
                        )
                    })?;
                normalized
                    .set(normalized_key, access)
                    .map_err(|diagnostic| invalid(policy_machine, diagnostic.to_string()))?;
            }
            if seen.len() != schema_fields.len() {
                let missing = schema_fields
                    .iter()
                    .filter(|field| !seen.contains(&field.key))
                    .map(|field| format!("`{}`", field.name))
                    .collect::<Vec<_>>()
                    .join(", ");
                return Err(invalid(
                    policy_machine,
                    format!("access plan omits schema field decisions for {missing}"),
                ));
            }

            validate_access_fields(normalized, layout)
                .map_err(|diagnostic| invalid(policy_machine, diagnostic.to_string()))
        }

        fn parse_field_access(
            value: &BuildTimeValue,
            schema_field: &SchemaFieldInfo,
            layout: &LayoutPlanReport,
        ) -> Result<FieldAccess, String> {
            let BuildTimeValue::Case { variant, payload } = value else {
                return Err(format!(
                    "access for field `{}` is not a FieldAccess case",
                    schema_field.name
                ));
            };
            if schema_field.primitive.is_none()
                && matches!(short_name(variant), "Stable" | "External")
            {
                let transfer_width_bits = schema_field.scalar_leaf_width_bits.ok_or_else(|| {
            format!(
                "field `{}` requires a supported scalar-leaf array projection for aggregate access",
                schema_field.name
            )
        })?;
                let (observation, read, take) = match short_name(variant) {
                    "Stable" => (
                        ObservationModel::Stable,
                        bool_payload(payload, "read")?,
                        false,
                    ),
                    _ => {
                        let read = external_read_payload(payload)?;
                        (
                            ObservationModel::External,
                            read == ExternalRead::Read,
                            read == ExternalRead::Take,
                        )
                    }
                };
                return Ok(FieldAccess::Region {
                    byte_length: schema_field.size,
                    byte_alignment: schema_field.align,
                    transfer_width_bits,
                    observation,
                    permissions: AccessPermissions {
                        read,
                        take,
                        write: bool_payload(payload, "write")?,
                        ..Default::default()
                    },
                    exposure: exposure_payload(payload)?,
                });
            }
            match short_name(variant) {
                "Inaccessible" => {
                    require_empty_payload(payload, "Inaccessible")?;
                    Ok(FieldAccess::Inaccessible)
                }
                "Stable" => {
                    let width = transfer_width_bits(schema_field, layout)?;
                    Ok(FieldAccess::Stable {
                        transfer_width_bits: width,
                        read: bool_payload(payload, "read")?,
                        write: bool_payload(payload, "write")?,
                        exposure: exposure_payload(payload)?,
                    })
                }
                "External" => {
                    require_scalar_access_field(schema_field)?;
                    let width = transfer_width_bits(schema_field, layout)?;
                    Ok(FieldAccess::External {
                        transfer_width_bits: width,
                        read: external_read_payload(payload)?,
                        write: bool_payload(payload, "write")?,
                        exposure: exposure_payload(payload)?,
                    })
                }
                "Atomic" => {
                    require_scalar_access_field(schema_field)?;
                    let width = transfer_width_bits(schema_field, layout)?;
                    let operations = payload_field(payload, "operations")?;
                    Ok(FieldAccess::Atomic {
                        transfer_width_bits: width,
                        operations: parse_atomic_permissions(operations)?,
                        exposure: exposure_payload(payload)?,
                    })
                }
                other => Err(format!(
                    "access for field `{}` uses unknown FieldAccess case `{other}`",
                    schema_field.name
                )),
            }
        }

        fn require_scalar_access_field(schema_field: &SchemaFieldInfo) -> Result<(), String> {
            schema_field.primitive.map(|_| ()).ok_or_else(|| {
        format!(
            "field `{}` is aggregate; the current access vocabulary admits only Inaccessible for aggregate fields",
            schema_field.name
        )
    })
        }

        fn transfer_width_bits(
            schema_field: &SchemaFieldInfo,
            layout: &LayoutPlanReport,
        ) -> Result<u16, String> {
            let placements = layout
                .entries
                .iter()
                .filter(|entry| entry.field == schema_field.name)
                .map(|entry| entry.placement)
                .collect::<Vec<_>>();
            let width = match placements.as_slice() {
                [LayoutPlacementReport::At { .. }] => {
                    schema_field.size.checked_mul(8).ok_or_else(|| {
                        format!("field `{}` transfer width overflows", schema_field.name)
                    })?
                }
                [LayoutPlacementReport::IntegerAt { stored_width, .. }] => *stored_width,
                [
                    LayoutPlacementReport::Bits {
                        container_width, ..
                    },
                    ..,
                ] => *container_width,
                [] => {
                    return Err(format!(
                        "field `{}` has no validated layout placement",
                        schema_field.name
                    ));
                }
                _ => schema_field.size.checked_mul(8).ok_or_else(|| {
                    format!("field `{}` transfer width overflows", schema_field.name)
                })?,
            };
            u16::try_from(width).map_err(|_| {
        format!(
            "field `{}` transfer width {width} does not fit the normalized access vocabulary",
            schema_field.name
        )
    })
        }

        fn parse_atomic_permissions(value: &BuildTimeValue) -> Result<AtomicPermissions, String> {
            let fields = struct_fields(value)?;
            Ok(AtomicPermissions {
                load: bool_named_field(fields, "load")?,
                store: bool_named_field(fields, "store")?,
                fetch_add: bool_named_field(fields, "fetch_add")?,
                fetch_sub: bool_named_field(fields, "fetch_sub")?,
                fetch_xor: bool_named_field(fields, "fetch_xor")?,
                fetch_or: bool_named_field(fields, "fetch_or")?,
                fetch_and: bool_named_field(fields, "fetch_and")?,
                swap: bool_named_field(fields, "swap")?,
                compare_exchange: bool_named_field(fields, "compare_exchange")?,
                compare_exchange_once: bool_named_field(fields, "compare_exchange_once")?,
                try_exchange: bool_named_field(fields, "try_exchange")?,
                try_exchange_once: bool_named_field(fields, "try_exchange_once")?,
            })
        }

        fn exposure_payload(
            payload: &[(String, BuildTimeValue)],
        ) -> Result<AccessExposure, String> {
            let value = payload_field(payload, "exposure")?;
            let BuildTimeValue::Case { variant, payload } = value else {
                return Err("FieldAccess exposure is not an Exposure case".into());
            };
            require_empty_payload(payload, short_name(variant))?;
            match short_name(variant) {
                "Exported" => Ok(AccessExposure::Exported),
                "BindingPrivate" => Ok(AccessExposure::BindingPrivate),
                other => Err(format!("unknown Exposure case `{other}")),
            }
        }

        fn external_read_payload(
            payload: &[(String, BuildTimeValue)],
        ) -> Result<ExternalRead, String> {
            let value = payload_field(payload, "read")?;
            let BuildTimeValue::Case { variant, payload } = value else {
                return Err("external read policy is not an ExternalRead case".into());
            };
            require_empty_payload(payload, short_name(variant))?;
            match short_name(variant) {
                "None" => Ok(ExternalRead::None),
                "Read" => Ok(ExternalRead::Read),
                "Take" => Ok(ExternalRead::Take),
                other => Err(format!("unknown ExternalRead case `{other}")),
            }
        }

        fn parse_boundary_reach(value: &BuildTimeValue) -> Result<BoundaryReach, String> {
            let fields = struct_fields(value)?;
            let service_count = uint_value(named_field(fields, "service_count")?, "service_count")?;
            if service_count > BOUNDARY_REACH_CAPACITY as u64 {
                return Err(format!(
                    "boundary service_count {service_count} exceeds bootstrap capacity {BOUNDARY_REACH_CAPACITY}"
                ));
            }
            let services = array_value(named_field(fields, "services")?, "services")?;
            if services.len() < service_count as usize {
                return Err(format!(
                    "boundary service_count is {service_count}, but the plan carries only {} service entries",
                    services.len()
                ));
            }
            let mut seen = BTreeSet::new();
            let mut normalized = Vec::with_capacity(service_count as usize);
            for (index, value) in services.iter().take(service_count as usize).enumerate() {
                let identity = uint_value(value, format!("services[{index}]").as_str())?;
                if !seen.insert(identity) {
                    return Err(format!(
                        "boundary reach repeats service identity {identity}"
                    ));
                }
                normalized.push(
                    BoundaryServiceReachId::from_normalized_identity(identity)
                        .map_err(|diagnostic| diagnostic.to_string())?,
                );
            }
            Ok(BoundaryReach::from_services(normalized))
        }

        fn struct_fields(value: &BuildTimeValue) -> Result<&[(String, BuildTimeValue)], String> {
            match value {
                BuildTimeValue::Struct { fields, .. } => Ok(fields),
                other => Err(format!("is not a record value: {other:?}")),
            }
        }

        fn named_field<'a>(
            fields: &'a [(String, BuildTimeValue)],
            name: &str,
        ) -> Result<&'a BuildTimeValue, String> {
            fields
                .iter()
                .find(|(field, _)| field == name)
                .map(|(_, value)| value)
                .ok_or_else(|| format!("carries no `{name}` field"))
        }

        fn payload_field<'a>(
            fields: &'a [(String, BuildTimeValue)],
            name: &str,
        ) -> Result<&'a BuildTimeValue, String> {
            fields
                .iter()
                .find(|(field, _)| field == name)
                .map(|(_, value)| value)
                .ok_or_else(|| format!("case payload carries no `{name}` field"))
        }

        fn uint_value(value: &BuildTimeValue, name: &str) -> Result<u64, String> {
            match value {
                BuildTimeValue::Int(value) => Ok(*value as u64),
                other => Err(format!("`{name}` is not an integer: {other:?}")),
            }
        }

        fn bool_named_field(
            fields: &[(String, BuildTimeValue)],
            name: &str,
        ) -> Result<bool, String> {
            match named_field(fields, name)? {
                BuildTimeValue::Bool(value) => Ok(*value),
                other => Err(format!("`{name}` is not a bool: {other:?}")),
            }
        }

        fn bool_payload(payload: &[(String, BuildTimeValue)], name: &str) -> Result<bool, String> {
            match payload_field(payload, name)? {
                BuildTimeValue::Bool(value) => Ok(*value),
                other => Err(format!("case payload `{name}` is not a bool: {other:?}")),
            }
        }

        fn array_value<'a>(
            value: &'a BuildTimeValue,
            name: &str,
        ) -> Result<&'a [BuildTimeValue], String> {
            match value {
                BuildTimeValue::Array(values) => Ok(values),
                other => Err(format!("`{name}` is not an array: {other:?}")),
            }
        }

        fn require_empty_payload(
            payload: &[(String, BuildTimeValue)],
            case: &str,
        ) -> Result<(), String> {
            if payload.is_empty() {
                Ok(())
            } else {
                Err(format!(
                    "nullary case `{case}` unexpectedly carries a payload"
                ))
            }
        }

        fn short_name(name: &str) -> &str {
            name.rsplit("::").next().unwrap_or(name)
        }

        fn invalid(policy_machine: &str, reason: impl AsRef<str>) -> String {
            format!(
                "policy `{policy_machine}` produced an invalid placed-access plan: {}",
                reason.as_ref()
            )
        }
    }

    pub(crate) use access_value::compute_selected_access_plan;

    fn evaluate_selected_policy(
        typed: &TypedTrees,
        machine: &typed_trees::machine::Machine,
        arguments: Vec<BuildTimeValue>,
        policy_kind: &str,
        accounting: &crate::evaluation_accounting::EvaluationAccounting,
    ) -> Result<BuildTimeValue, String> {
        let policy_machine = machine.name.as_str();
        let admission = BuildTimeAdmissionPlan::infer(typed, None)
            .map_err(|diagnostics| diagnostics::join_messages(&diagnostics))?;
        admission.require_common_floor(typed, machine)?;
        let mut request = super::policy_arguments::policy_request(typed, machine, arguments)?;
        admission.require_invocation_failures(
            crate::BuildTimeProgram::Typed(typed),
            machine,
            &mut request.arguments,
            None,
        )?;
        accounting
            .evaluate(
                crate::evaluation_requests::BuildMachineProgram::Typed(typed),
                request,
                &format!("{policy_kind} policy `{policy_machine}`"),
            )
            .map(BuildTimeOperationEvaluation::into_value)
            .map_err(String::from)
    }
}
pub(crate) mod access_views {
    use crate::build_time_evaluation::type_references;
    use crate::unique::push_unique;
    use std::collections::BTreeMap;
    use std::sync::Arc;

    use crate::build_time_evaluation::EvaluationDomain;
    use access_plans::AccessPlanId;
    use diagnostics::Diagnostic;
    use language_semantics::DataSupplyMode;
    use syntax_trees::SyntaxTrees;
    use syntax_trees::item::{DataField, DataMember, Item};
    use syntax_trees::types::{TypeReferenceHandle, TypeReferenceNode};
    use typed_trees::TypedTrees;

    mod applied_results {
        use diagnostics::Diagnostic;
        use symbol_resolved_trees::data::TypeParameterKind;
        use symbol_resolved_trees::{SymbolResolvedTrees, types::TypeReference};
        use symbol_resolved_trees_to_typed_trees::access_borrow_factory_of;
        use symbols::{SymbolHandle, SymbolKind};
        use syntax_trees::SyntaxTrees;
        use syntax_trees::identifier::Identifier;
        use syntax_trees::types::{TypeReferenceHandle, TypeReferenceNode};
        use syntax_trees_to_symbol_resolved_trees::pre_resolution::resolve_call_result_selection;

        pub(super) struct SelectedArgument {
            pub(super) reference: TypeReferenceHandle,
            pub(super) declaration: Result<source::SourceSpan, String>,
        }

        pub(super) fn retain_applied_results(
            syntax: &mut SyntaxTrees,
            context: &crate::BuildTimeSources<'_>,
        ) -> Result<Vec<SelectedArgument>, Vec<Diagnostic>> {
            if !syntax.expressions.iter_expressions().any(|(_, node)| {
                matches!(node,
        syntax_trees::expression::ExpressionNode::Call(call) if !call.machine_arguments.is_empty())
            }) {
                return Ok(Vec::new());
            }
            let resolved =
                resolve_call_result_selection(context.resolution_request(syntax.clone()))?;
            let mut selections = Vec::new();
            let call_targets = CallTargetMachines::of(&resolved);
            for (_, node) in resolved.tables.bodies.expressions.iter_expressions() {
                let symbol_resolved_trees::expression::ExpressionNode::Call(call) = node else {
                    continue;
                };
                let Some(machine) = call_targets.machine(&resolved, call.target_symbol) else {
                    continue;
                };
                let factory =
                    access_borrow_factory_of(&resolved, machine).map_err(|diagnostic| {
                        vec![diagnostic.with_source_span(call.target.source_span())]
                    })?;
                let parameters = resolved.machine_type_parameters(machine);
                if parameters.len() != call.machine_arguments.len() {
                    if factory.is_some() {
                        return Err(vec![
                    Diagnostic::error(
                        "borrowed View requires its exact applied policy and schema type arguments",
                    )
                    .with_source_span(call.target.source_span()),
                ]);
                    }
                    continue;
                }
                let mut substitutions = Vec::new();
                for (parameter, argument) in parameters.iter().zip(&call.machine_arguments) {
                    if !matches!(parameter.kind, TypeParameterKind::Type) {
                        continue;
                    }
                    let symbol = if argument.type_reference.is_valid() {
                        let mut reference = resolved.child_type_reference(argument.type_reference);
                        while let TypeReference::AliasUse { expansion, .. } = reference {
                            reference = expansion;
                        }
                        match reference {
                            TypeReference::Named { symbol, .. } => *symbol,
                            _ => continue,
                        }
                    } else {
                        argument.symbol
                    };
                    let reference = argument
                        .path
                        .first()
                        .map(|name| name.source_span())
                        .unwrap_or(call.target.source_span());
                    substitutions.push((parameter.symbol, symbol, reference));
                }
                let state = resolved
                    .machine_state_handles(machine.states)
                    .iter()
                    .find_map(|state| {
                        let state = resolved.machine_state(*state);
                        (machine.symbol == call.target_symbol || state.symbol == call.target_symbol)
                            .then_some(state)
                    });
                if let Some(state) = state
                    && let Some(result) = &state.return_type
                {
                    retain_result(
                        syntax,
                        &resolved,
                        result,
                        &substitutions,
                        call.target.source_span(),
                        &mut selections,
                        &mut Vec::new(),
                    )?;
                }
            }
            Ok(selections)
        }

        fn selected_nominal(
            resolved: &SymbolResolvedTrees,
            reference: &TypeReference,
            substitutions: &[(SymbolHandle, SymbolHandle, source::SourceSpan)],
            invocation: source::SourceSpan,
        ) -> Option<(SymbolHandle, source::SourceSpan)> {
            let mut reference = reference;
            while let TypeReference::AliasUse { expansion, .. } = reference {
                reference = expansion;
            }
            let TypeReference::Named { symbol, .. } = reference else {
                return None;
            };
            let (symbol, reference) = substitutions
                .iter()
                .find(|(formal, _, _)| formal == symbol)
                .map(|(_, actual, reference)| (*actual, *reference))
                .unwrap_or((*symbol, invocation));
            (symbol.is_valid() && resolved.symbols.get(symbol).kind == SymbolKind::Data)
                .then_some((symbol, reference))
        }

        fn retain_result(
            syntax: &mut SyntaxTrees,
            resolved: &SymbolResolvedTrees,
            result: &TypeReference,
            substitutions: &[(SymbolHandle, SymbolHandle, source::SourceSpan)],
            invocation: source::SourceSpan,
            selections: &mut Vec<SelectedArgument>,
            visited_callables: &mut Vec<
                symbol_resolved_trees::callable_values::RuntimeCallableContractHandle,
            >,
        ) -> Result<(), Vec<Diagnostic>> {
            macro_rules! retain_child {
                ($result:expr $(,)?) => {
                    retain_result(
                        syntax,
                        resolved,
                        $result,
                        substitutions,
                        invocation,
                        selections,
                        visited_callables,
                    )
                };
            }
            match result {
                TypeReference::AliasUse { expansion, .. } => retain_child!(expansion)?,
                TypeReference::Reference(reference) => {
                    retain_child!(resolved.child_type_reference(reference.referee))?
                }
                TypeReference::Constrained(reference) => {
                    retain_child!(resolved.child_type_reference(reference.base_type))?
                }
                TypeReference::FixedArray(reference) => {
                    retain_child!(resolved.child_type_reference(reference.element_type))?
                }
                TypeReference::Slice(reference) => {
                    retain_child!(resolved.child_type_reference(reference.element_type))?
                }
                TypeReference::Generic(application) => {
                    let arguments = resolved.child_type_references(application.arguments);
                    let sealed = resolved
                        .symbols
                        .symbol_source_span(application.base_symbol)
                        .and_then(|span| resolved.symbols.source_file(span))
                        .is_some_and(|source| {
                            symbols::core_source_at(source)
                                == Some(language_core::core_vocabulary::CoreSource::Layout)
                        });
                    if sealed && resolved.symbols.name(application.base_symbol) == "Access" {
                        let [policy, schema] = arguments else {
                            return Err(vec![
                        Diagnostic::error(
                            "applied Access result must retain exactly policy and schema arguments",
                        )
                        .with_source_span(invocation),
                    ]);
                        };
                        if let (Some(policy), Some(schema)) = (
                            selected_nominal(resolved, policy, substitutions, invocation),
                            selected_nominal(resolved, schema, substitutions, invocation),
                        ) {
                            let mut actuals = Vec::new();
                            for (symbol, reference) in [policy, schema] {
                                let declaration = resolved.symbols.symbol_source_span(symbol).ok_or_else(|| vec![Diagnostic::error("placed result nominal argument lost its selected declaration identity").with_source_span(reference)])?;
                                let actual = syntax.type_references.insert_named(Identifier::new(
                                    resolved.symbols.name(symbol),
                                    reference,
                                ));
                                selections.push(SelectedArgument {
                                    reference: actual,
                                    declaration: Ok(declaration),
                                });
                                actuals.push(actual);
                            }
                            let arguments = syntax
                                .type_references
                                .insert_type_reference_handles(actuals);
                            let reference =
                                syntax.type_references.insert(TypeReferenceNode::Generic {
                                    base_name: Identifier::new("Access", invocation),
                                    lifetime_arguments: Vec::new(),
                                    arguments,
                                });
                            let declaration = resolved
                                .symbols
                                .symbol_source_span(application.base_symbol)
                                .ok_or_else(|| {
                                    vec![
                                        Diagnostic::error(
                                            "placed result lost its exact template declaration",
                                        )
                                        .with_source_span(invocation),
                                    ]
                                })?;
                            selections.push(SelectedArgument {
                                reference,
                                declaration: Ok(declaration),
                            });
                        }
                    }
                    for argument in arguments {
                        retain_child!(argument)?;
                    }
                }
                TypeReference::Callable { contract, .. } => {
                    if !resolved
                        .tables
                        .declarations
                        .runtime_callable_contracts
                        .is_valid(*contract)
                    {
                        return Err(vec![
                            Diagnostic::error("placed result contains a stale callable contract")
                                .with_source_span(invocation),
                        ]);
                    }
                    if !visited_callables.contains(contract) {
                        visited_callables.push(*contract);
                        let signature = &resolved
                            .tables
                            .declarations
                            .runtime_callable_contracts
                            .get(*contract)
                            .signature;
                        let inputs =
                            symbol_resolved_trees::callable_values::ResolvedSignatureInputs::of(
                                resolved, signature,
                            );
                        for (_, reference) in inputs.types {
                            retain_child!(reference)?;
                        }
                    }
                }
                TypeReference::DynamicTrait { arguments, .. } => {
                    for argument in resolved.child_type_references(*arguments) {
                        retain_child!(argument)?;
                    }
                }
                TypeReference::Named { .. }
                | TypeReference::ConstExpression(_)
                | TypeReference::Unit => {}
            }
            Ok(())
        }

        struct CallTargetMachines {
            first: Vec<Option<(SymbolHandle, usize)>>,
        }

        impl CallTargetMachines {
            fn of(program: &SymbolResolvedTrees) -> Self {
                let mut first = Vec::new();
                for (position, machine) in program.machines.iter().enumerate() {
                    let states = program
                        .machine_state_handles(machine.states)
                        .iter()
                        .map(|state| program.machine_state(*state).symbol);
                    for symbol in std::iter::once(machine.symbol).chain(states) {
                        let slot = symbol.arena_index() as usize;
                        if first.len() <= slot {
                            first.resize(slot + 1, None);
                        }
                        first[slot].get_or_insert((symbol, position));
                    }
                }
                Self { first }
            }

            fn machine<'program>(
                &self,
                program: &'program SymbolResolvedTrees,
                target: SymbolHandle,
            ) -> Option<&'program symbol_resolved_trees::machine::Machine> {
                match self
                    .first
                    .get(target.arena_index() as usize)
                    .copied()
                    .flatten()
                {
                    None => None,
                    Some((symbol, position)) if symbol == target => {
                        Some(&program.machines[position])
                    }
                    Some(_) => program.machines.iter().find(|machine| {
                        machine.symbol == target
                            || program
                                .machine_state_handles(machine.states)
                                .iter()
                                .any(|state| program.machine_state(*state).symbol == target)
                    }),
                }
            }
        }
    }
    mod plan_installation {
        use crate::build_time_evaluation::type_reference_table;
        use std::collections::BTreeMap;

        use access_plans::{AtomicPermissions, FieldAccess, ValidatedAccessPlan};
        use diagnostics::Diagnostic;
        use language_semantics::Multiplicity;
        use typed_trees::TypedTrees;

        use super::{AccessViewRecord, accessor_operations};
        use symbols::SymbolHandle;
        use typed_trees::data::DataMember;
        use typed_trees::typed_trees::{AccessAtomicResidentContract, AccessViewPlan};
        use validation::access_atomic_observing_results;

        fn observing_result_contracts(
            operations: AtomicPermissions,
            multiplicity: Multiplicity,
        ) -> Result<
            Option<Vec<typed_trees::typed_trees::AccessAtomicObservingResultContract>>,
            Multiplicity,
        > {
            if !operations.compare_exchange && !operations.compare_exchange_once {
                return Ok(None);
            }
            if multiplicity != Multiplicity::Unrestricted {
                return Err(multiplicity);
            }
            Ok(Some(access_atomic_observing_results(operations)))
        }

        pub(super) fn selected_access_view<'a>(
            typed: &'a TypedTrees,
            record: &AccessViewRecord,
            policy_symbol: SymbolHandle,
            schema_symbol: SymbolHandle,
        ) -> Result<(&'a typed_trees::data::DataDefinition, SymbolHandle), Vec<Diagnostic>>
        {
            let mut views = typed
                .data_definitions()
                .iter()
                .filter(|definition| definition.name.as_str() == record.synthetic_name);
            let (Some(view), None) = (views.next(), views.next()) else {
                return Err(vec![Diagnostic::error(format!(
                    "access view `{}` lost its unique derived data definition after typing",
                    record.synthetic_name
                ))]);
            };
            let templates = typed
                .data_definitions()
                .iter()
                .filter(|definition| {
                    definition.name.as_str().rsplit("::").next() == Some("Access")
                        && typed.symbols.symbol_source_span(definition.symbol)
                            == Some(record.template_source)
                })
                .collect::<Vec<_>>();
            let template_symbol = match templates.as_slice() {
                [template] => template.symbol,
                _ if typed.symbols.source_file(record.template_source).is_none() => {
                    SymbolHandle::default()
                }
                _ => {
                    return Err(vec![
                        Diagnostic::error(
                            "access view lost its unique canonical template declaration",
                        )
                        .with_source_span(record.template_source),
                    ]);
                }
            };
            if template_symbol.is_valid() {
                use typed_trees::types::TypeReferenceNode;
                let origin = view.generic_instance.ok_or_else(|| {
                    vec![Diagnostic::error(
                        "access view lost its retained applied origin",
                    )]
                })?;
                let TypeReferenceNode::Generic {
                    base_symbol,
                    arguments,
                    lifetime_arguments,
                    ..
                } = typed.type_reference_table.type_reference(origin)
                else {
                    return Err(vec![Diagnostic::error(
                        "access view retained origin is not an exact application",
                    )]);
                };
                let operands = type_reference_table(typed).type_reference_handles(*arguments);
                if *base_symbol != template_symbol
                    || !lifetime_arguments.is_empty()
                    || !matches!(operands, [policy_operand, schema_operand]
                if matches!(typed.type_reference_table.type_reference(*policy_operand), TypeReferenceNode::Named { symbol, .. } if *symbol == policy_symbol)
                && matches!(typed.type_reference_table.type_reference(*schema_operand), TypeReferenceNode::Named { symbol, .. } if *symbol == schema_symbol))
                {
                    return Err(vec![Diagnostic::error(
                        "access view origin changed its exact template/policy/schema producers",
                    )]);
                }
            }
            Ok((view, template_symbol))
        }

        pub(super) fn install_access_view_plan(
            typed: &mut TypedTrees,
            record: &AccessViewRecord,
            placement: &ValidatedAccessPlan,
        ) -> Result<(), Vec<Diagnostic>> {
            let policy_name = record
                .policy_machine
                .strip_suffix("::plan")
                .unwrap_or(&record.policy_machine);
            let policy = typed
                .data_definitions()
                .iter()
                .find(|definition| {
                    definition.name.as_str() == policy_name
                        && typed.symbols.symbol_source_span(definition.symbol)
                            == Some(record.policy_source)
                })
                .ok_or_else(|| {
                    vec![Diagnostic::error(format!(
                        "access view `{}` lost nominal policy `{}` after typing",
                        record.synthetic_name, record.policy_machine
                    ))]
                })?;
            let policy_plan_machine = typed
                .machines()
                .iter()
                .find(|machine| {
                    machine.name.as_str() == record.policy_machine
                        && typed.symbols.symbol_source_span(machine.symbol)
                            == Some(record.policy_machine_source)
                })
                .ok_or_else(|| {
                    vec![Diagnostic::error(format!(
                        "access view `{}` lost exact policy machine `{}` after typing",
                        record.synthetic_name, record.policy_machine
                    ))]
                })?;
            let schema = typed
                .data_definitions()
                .iter()
                .find(|definition| {
                    definition.name.as_str() == record.schema_data
                        && typed.symbols.symbol_source_span(definition.symbol)
                            == Some(record.schema_source)
                })
                .ok_or_else(|| {
                    vec![Diagnostic::error(format!(
                        "access view `{}` lost schema `{}` after typing",
                        record.synthetic_name, record.schema_data
                    ))]
                })?;
            if policy_plan_machine.attached_data_symbol != policy.symbol {
                return Err(vec![
                    Diagnostic::error(
                        "placed policy machine changed its exact attached owner after typing",
                    )
                    .with_source_span(record.policy_machine_source),
                ]);
            }
            let schema_fields = typed
                .data_members(schema)
                .iter()
                .filter_map(|member| match member {
                    DataMember::Field(field) => Some(field),
                    DataMember::Variant(_) => None,
                })
                .map(|field| (field.name.as_str().to_owned(), field))
                .collect::<BTreeMap<_, _>>();
            let (view, template_symbol) =
                selected_access_view(typed, record, policy.symbol, schema.symbol)?;
            let view_fields = typed
                .data_members(view)
                .iter()
                .filter_map(|member| match member {
                    DataMember::Field(field) => Some((field.name.as_str().to_owned(), field)),
                    DataMember::Variant(_) => None,
                })
                .collect::<BTreeMap<_, _>>();
            let schema_symbol = schema.symbol;
            let data_symbol = view.symbol;
            let policy_symbol = policy.symbol;
            let policy_plan_machine_symbol = policy_plan_machine.symbol;

            let mut fields = Vec::new();
            for entry in placement.access().plan().entries() {
                if matches!(entry.access(), FieldAccess::Inaccessible) {
                    continue;
                }
                let schema_field = schema_fields.get(entry.field()).copied().ok_or_else(|| {
                    vec![Diagnostic::error(format!(
                        "access view `{}` lost schema field `{}` after typing",
                        record.synthetic_name,
                        entry.field()
                    ))]
                })?;
                let view_field = view_fields.get(entry.field()).copied().ok_or_else(|| {
                    vec![Diagnostic::error(format!(
                        "access view `{}` lost admitted field `{}` after typing",
                        record.synthetic_name,
                        entry.field()
                    ))]
                })?;
                if schema_field.identity != view_field.identity {
                    return Err(vec![Diagnostic::error(format!(
                        "access view `{}` field `{}` changed stable member identity during accessor derivation",
                        record.synthetic_name,
                        entry.field()
                    ))]);
                }
                let mut accessor_leaf = view_field.type_reference;
                while let typed_trees::types::TypeReferenceNode::FixedArray {
                    element_type, ..
                } = typed.type_reference_table.type_reference(accessor_leaf)
                {
                    accessor_leaf = *element_type;
                }
                let accessor_name = typed
            .named_type_reference(accessor_leaf)
            .map(|name| name.as_str().to_owned())
            .ok_or_else(|| {
                vec![Diagnostic::error(format!(
                    "access view `{}` field `{}` lost its nominal accessor type after typing",
                    record.synthetic_name,
                    entry.field()
                ))]
            })?;
                let accessor_type_symbol =
                    type_reference_table(typed).type_symbol(view_field.type_reference);
                let accessor_data = typed
                    .data_definitions()
                    .iter()
                    .filter(|definition| {
                        if accessor_type_symbol.is_valid() {
                            definition.symbol == accessor_type_symbol
                        } else {
                            matches!(entry.access(), FieldAccess::Atomic { .. })
                                && definition.name.as_str() == accessor_name
                        }
                    })
                    .collect::<Vec<_>>();
                let [accessor_data] = accessor_data.as_slice() else {
                    return Err(vec![Diagnostic::error(format!(
                        "access view `{}` field `{}` must retain one exact generated accessor data definition",
                        record.synthetic_name,
                        entry.field()
                    ))]);
                };
                let mut accessor_targets = Vec::new();
                for operation in accessor_operations(entry.access()) {
                    let machines = typed
                        .machines()
                        .iter()
                        .filter(|machine| {
                            machine.attached_data.as_ref().is_some_and(|attached| {
                                attached.as_str() == accessor_name
                                    && machine
                                        .name
                                        .as_str()
                                        .rsplit("::")
                                        .next()
                                        .is_some_and(|name| name == operation)
                            })
                        })
                        .collect::<Vec<_>>();
                    let [machine] = machines.as_slice() else {
                        return Err(vec![Diagnostic::error(format!(
                            "access view `{}` field `{}` must retain one exact generated `{operation}` accessor machine",
                            record.synthetic_name,
                            entry.field()
                        ))]);
                    };
                    let [state] = typed.machine_states(machine) else {
                        return Err(vec![Diagnostic::error(format!(
                            "access view `{}` field `{}` generated `{operation}` accessor must have one exact callable state",
                            record.synthetic_name,
                            entry.field()
                        ))]);
                    };
                    accessor_targets.push(typed_trees::typed_trees::AccessorTarget {
                        operation: operation.to_owned(),
                        machine_symbol: machine.symbol,
                        state_symbol: state.symbol,
                    });
                }
                let atomic_resident = match entry.access() {
                    FieldAccess::Atomic {
                        transfer_width_bits,
                        operations,
                        ..
                    } => {
                        let multiplicity = typed.type_multiplicity(schema_field.type_reference);
                        match observing_result_contracts(*operations, multiplicity) {
                            Ok(None) => None,
                            Ok(Some(observing_results)) => Some(AccessAtomicResidentContract {
                                field_symbol: schema_field.symbol,
                                resident_type: schema_field.type_reference,
                                multiplicity,
                                transfer_width_bits: *transfer_width_bits,
                                compare_exchange: operations.compare_exchange,
                                compare_exchange_once: operations.compare_exchange_once,
                                observing_results,
                            }),
                            Err(multiplicity) => {
                                return Err(vec![Diagnostic::error(format!(
                                    "access view `{}` field `{}` cannot admit observing compare-exchange for a {multiplicity:?} resident; failure exposes the resident and requires unrestricted copyability",
                                    record.synthetic_name,
                                    entry.field()
                                ))]);
                            }
                        }
                    }
                    _ => None,
                };
                fields.push(typed_trees::typed_trees::AccessFieldPlan {
                    access_key: entry.key(),
                    view_field_symbol: view_field.symbol,
                    field_name: entry.field().to_owned(),
                    member_identity: schema_field.identity,
                    field_symbol: schema_field.symbol,
                    accessor_name,
                    accessor_type: view_field.type_reference,
                    accessor_data_symbol: accessor_data.symbol,
                    accessor_targets,
                    value_type: schema_field.type_reference,
                    access: entry.access().clone(),
                    atomic_resident,
                });
            }
            typed.access_view_plans.push(AccessViewPlan {
                template_symbol,
                data_name: record.synthetic_name.clone(),
                data_symbol,
                policy_name: policy_name.to_owned(),
                policy_symbol,
                policy_plan_machine_symbol,
                schema_name: record.schema_data.clone(),
                schema_symbol,
                placement: placement.clone(),
                fields,
            });
            Ok(())
        }
    }

    pub(super) fn toolchain_layout_declaration(
        sources: Option<&source::SourceMap>,
        span: source::SourceSpan,
    ) -> bool {
        sources.is_none_or(|sources| {
            sources.file_at(span).is_some_and(|source| {
                symbols::core_source_at(source)
                    == Some(language_core::core_vocabulary::CoreSource::Layout)
            })
        })
    }

    fn has_accessor_template(forest: &SyntaxTrees, sources: Option<&source::SourceMap>) -> bool {
        forest.root_items().any(|item| {
            matches!(item, Item::Data(data)
            if data.name.as_str(forest).rsplit("::").next() == Some("Accessor")
                && toolchain_layout_declaration(sources, data.name.source_span()))
        })
    }
    mod record_synthesis {
        use crate::build_time_evaluation::type_references;
        use std::collections::BTreeMap;

        use access_plans::{AccessExposure, FieldAccess, ValidatedAccessPlan};
        use arena::{Handle, HandleSpan};
        use diagnostics::Diagnostic;
        use language_semantics::{DataSupplyMode, Multiplicity};
        use syntax_trees::SyntaxTrees;
        use syntax_trees::identifier::Identifier;
        use syntax_trees::item::{
            DataDefinition, DataField, DataMember, DataProperties, Item, Machine,
        };
        use syntax_trees::types::{TypeReferenceHandle, TypeReferenceNode};

        use super::{Application, ApplicationKey, DeclarationKey, PendingRewrite, SchemaRecord};
        use source::SourceMap;

        fn generated_identifier(
            name: impl Into<String>,
            provenance: source::SourceSpan,
        ) -> Identifier {
            Identifier::new(name.into(), provenance)
        }

        fn application_provenance(application: &Application) -> source::SourceSpan {
            application
                .invocation_sources
                .first()
                .copied()
                .unwrap_or_default()
        }

        pub(super) fn synthesize_probe_records(
            syntax: &mut SyntaxTrees,
            retained_templates: Option<&SyntaxTrees>,
            applications: &[Application],
            rewrites: &[PendingRewrite],
            schemas: &BTreeMap<DeclarationKey, SchemaRecord>,
            sources: Option<&SourceMap>,
        ) -> Result<(), Vec<Diagnostic>> {
            let template_spans = |forest: &SyntaxTrees| {
                forest
                    .root_items()
                    .filter_map(|item| item.as_data())
                    .filter(|data| {
                        data.name.as_str(forest).rsplit("::").next() == Some("Accessor")
                            && super::toolchain_layout_declaration(sources, data.name.source_span())
                    })
                    .map(|data| data.name.source_span())
                    .collect::<Vec<_>>()
            };
            let templates = match retained_templates {
                Some(forest) => template_spans(forest),
                None => template_spans(syntax),
            };
            let [template_span] = templates.as_slice() else {
                return Err(vec![Diagnostic::error(
                    "access probe requires one exact sealed accessor template",
                )]);
            };
            let template = generated_identifier("Accessor", *template_span);
            for application in applications {
                let members = schemas[&application.key.schema]
                    .fields
                    .iter()
                    .map(|field| {
                        let value_leaf = projection_leaf(syntax, field.type_reference);
                        let arguments = syntax
                            .tables
                            .type_references
                            .insert_type_reference_handles([value_leaf]);
                        let accessor = syntax
                            .tables
                            .type_references
                            .insert_generic(template.clone(), arguments);
                        let accessor = projection_shape(syntax, field.type_reference, accessor);
                        DataMember::Field(DataField {
                            identity: field.identity,
                            name: field.name.clone(),
                            relevance: field.relevance,
                            type_reference: accessor,
                        })
                    })
                    .collect::<Vec<_>>();
                let origin_node = type_references(syntax)
                    .type_reference(application.origin)
                    .clone();
                let origin = syntax.type_references.insert(origin_node);
                push_record(
                    syntax,
                    origin,
                    &application.synthetic_name,
                    members,
                    application.generated_is_public,
                    application_provenance(application),
                );
            }
            rewrite_applications(syntax, rewrites);
            Ok(())
        }

        pub(super) fn synthesize_exact_records(
            syntax: &mut SyntaxTrees,
            retained_templates: Option<&SyntaxTrees>,
            applications: &[Application],
            rewrites: &[PendingRewrite],
            schemas: &BTreeMap<DeclarationKey, SchemaRecord>,
            plans: &BTreeMap<ApplicationKey, ValidatedAccessPlan>,
            sources: Option<&SourceMap>,
        ) -> Result<(), Vec<Diagnostic>> {
            let cohort_template;
            let template: &SyntaxTrees = match retained_templates {
                Some(forest) => forest,
                None => {
                    cohort_template = syntax.clone();
                    &cohort_template
                }
            };
            let template_machines = template
                .root_items()
                .filter_map(|item| {
                    let machine = item.as_machine()?;
                    sealed_accessor_template(template, machine, sources).then_some(machine.clone())
                })
                .collect::<Vec<_>>();

            for application in applications {
                let plan = &plans[&application.key];
                let schema = &schemas[&application.key.schema];
                let mut members = Vec::new();
                for field in &schema.fields {
                    let entry = plan
                        .access()
                        .plan()
                        .entries()
                        .iter()
                        .find(|entry| entry.field() == field.name.as_str(syntax));
                    if field.relevance.is_erased() && entry.is_none() {
                        continue;
                    }
                    let Some(entry) = entry else {
                        return Err(vec![Diagnostic::error(format!(
                            "access view `{}` lost canonical schema field `{}`",
                            application.synthetic_name,
                            field.name.as_str(syntax)
                        ))]);
                    };
                    if matches!(entry.access(), FieldAccess::Inaccessible) {
                        continue;
                    }

                    let provenance = application_provenance(application);
                    let value_leaf = projection_leaf(syntax, field.type_reference);
                    let accessor_name = accessor_name(syntax, application, field, entry.access())?;
                    let is_exported = access_is_exported(entry.access());
                    syntax.push_root_item(Item::Data(DataDefinition {
                        cleanup_selection: None,
                        name: generated_identifier(accessor_name.clone(), provenance),
                        is_public: application.generated_is_public,
                        supply_mode: DataSupplyMode::BoundaryOpaque,
                        lifetime_parameters: Vec::new(),
                        type_parameters: HandleSpan::empty(),
                        conformance_bounds: Vec::new(),
                        generic_instance: None,
                        properties: DataProperties::default(),
                        requires_facts: HandleSpan::empty(),
                        members: HandleSpan::empty(),
                        quotient: None,
                    }));
                    for operation in accessor_operations(entry.access()) {
                        let Some(machine) = template_machines.iter().find(|machine| {
                            machine
                                .name
                                .as_str(template)
                                .rsplit("::")
                                .next()
                                .is_some_and(|name| name == operation)
                        }) else {
                            return Err(vec![Diagnostic::error(format!(
                                "compiler accessor template `AccessField::{operation}` is unavailable"
                            ))]);
                        };
                        clone_accessor_machine(
                            syntax,
                            template,
                            machine,
                            &accessor_name,
                            value_leaf,
                            is_exported && application.generated_is_public,
                            provenance,
                        );
                    }
                    let field_type = syntax
                        .tables
                        .type_references
                        .insert_named(generated_identifier(accessor_name, provenance));
                    let field_type = projection_shape(syntax, field.type_reference, field_type);
                    members.push(DataMember::Field(DataField {
                        identity: field.identity,
                        name: field.name.clone(),
                        relevance: field.relevance,
                        type_reference: field_type,
                    }));
                }
                let origin_node = type_references(syntax)
                    .type_reference(application.origin)
                    .clone();
                let origin = syntax.type_references.insert(origin_node);
                push_record(
                    syntax,
                    origin,
                    &application.synthetic_name,
                    members,
                    application.generated_is_public,
                    application_provenance(application),
                );
            }
            rewrite_applications(syntax, rewrites);
            retire_accessor_templates(syntax, sources);
            Ok(())
        }

        fn projection_leaf(
            syntax: &SyntaxTrees,
            mut reference: TypeReferenceHandle,
        ) -> TypeReferenceHandle {
            while let TypeReferenceNode::FixedArray { element_type, .. } =
                syntax.type_references.type_reference(reference)
            {
                reference = *element_type;
            }
            reference
        }

        fn projection_shape(
            syntax: &mut SyntaxTrees,
            source: TypeReferenceHandle,
            accessor: TypeReferenceHandle,
        ) -> TypeReferenceHandle {
            match syntax.type_references.type_reference(source).clone() {
                TypeReferenceNode::FixedArray {
                    element_type,
                    length,
                } => {
                    let element_type = projection_shape(syntax, element_type, accessor);
                    syntax
                        .type_references
                        .insert(TypeReferenceNode::FixedArray {
                            element_type,
                            length,
                        })
                }
                _ => accessor,
            }
        }

        fn accessor_name(
            syntax: &SyntaxTrees,
            application: &Application,
            field: &DataField,
            access: &FieldAccess,
        ) -> Result<String, Vec<Diagnostic>> {
            let base = if matches!(access, FieldAccess::Atomic { .. }) {
                match type_references(&syntax.tables).type_reference(field.type_reference) {
                    TypeReferenceNode::Named(name) => match name.as_str(syntax) {
                        "bool" => "AtomicBool#AccessField",
                        "u32" => "AtomicU32#AccessField",
                        "u64" => "AtomicU64#AccessField",
                        _ => {
                            return Err(vec![Diagnostic::error(format!(
                                "placed atomic field `{}` in `{}` requires schema type `bool`, `u32`, or `u64`",
                                field.name.as_str(syntax),
                                application.synthetic_name
                            ))]);
                        }
                    },
                    _ => {
                        return Err(vec![Diagnostic::error(format!(
                            "placed atomic field `{}` in `{}` requires a plain schema type `bool`, `u32`, or `u64`",
                            field.name.as_str(syntax),
                            application.synthetic_name
                        ))]);
                    }
                }
            } else {
                "Accessor"
            };
            Ok(format!(
                "{base}<{},{}>",
                application.synthetic_name,
                field.name.as_str(syntax)
            ))
        }

        fn sealed_accessor_template(
            syntax: &SyntaxTrees,
            machine: &Machine,
            sources: Option<&SourceMap>,
        ) -> bool {
            machine
                .attached_data
                .as_ref()
                .is_some_and(|attached| attached.as_str(syntax) == "Accessor")
                && sources.is_none_or(|sources| {
                    sources
                        .file_at(machine.name.source_span())
                        .is_some_and(|source| {
                            symbols::core_source_at(source)
                                == Some(language_core::core_vocabulary::CoreSource::Layout)
                        })
                })
        }

        pub(super) fn retire_accessor_templates(
            syntax: &mut SyntaxTrees,
            sources: Option<&SourceMap>,
        ) {
            let mut retained_roots = Vec::with_capacity(syntax.root_item_count());
            for handle in syntax.root_item_handles() {
                let is_template = matches!(syntax.root_item(*handle), Item::Machine(machine)
            if sealed_accessor_template(syntax, machine, sources));
                if !is_template {
                    retained_roots.push(*handle);
                }
            }
            syntax.roots.items = retained_roots;
        }

        pub(super) fn accessor_operations(access: &FieldAccess) -> Vec<&'static str> {
            access.operation_names()
        }

        fn clone_accessor_machine(
            syntax: &mut SyntaxTrees,
            template: &SyntaxTrees,
            machine: &Machine,
            accessor_name: &str,
            field_type: TypeReferenceHandle,
            is_public: bool,
            provenance: source::SourceSpan,
        ) {
            let watermark = syntax.tables.type_references.node_count();
            let Item::Machine(mut clone) =
                syntax.copy_item_from(template, &Item::Machine(machine.clone()))
            else {
                unreachable!("copied accessor template remains a machine");
            };
            let operation = machine
                .name
                .as_str(template)
                .rsplit("::")
                .next()
                .expect("accessor template operation");
            clone.name = generated_identifier(format!("{accessor_name}::{operation}"), provenance);
            clone.attached_data = Some(generated_identifier(accessor_name, provenance));
            clone.is_public = is_public;
            clone.type_parameters = HandleSpan::empty();
            let subject = TypeReferenceNode::Named(generated_identifier(accessor_name, provenance));
            for (handle, base_name) in
                type_references(&syntax.tables).generic_nodes_from(watermark, &syntax.text)
            {
                if base_name == "Accessor" {
                    syntax
                        .tables
                        .type_references
                        .replace_type_reference(handle, subject.clone());
                }
            }
            let replacement = type_references(&syntax.tables)
                .type_reference(field_type)
                .clone();
            for (handle, name) in
                type_references(&syntax.tables).named_nodes_from(watermark, &syntax.text)
            {
                if name == "T" {
                    syntax
                        .tables
                        .type_references
                        .replace_type_reference(handle, replacement.clone());
                }
            }
            syntax.push_root_item(Item::Machine(clone));
        }

        fn access_is_exported(access: &FieldAccess) -> bool {
            matches!(
                access,
                FieldAccess::Stable {
                    exposure: AccessExposure::Exported,
                    ..
                } | FieldAccess::Region {
                    exposure: AccessExposure::Exported,
                    ..
                } | FieldAccess::External {
                    exposure: AccessExposure::Exported,
                    ..
                } | FieldAccess::Atomic {
                    exposure: AccessExposure::Exported,
                    ..
                }
            )
        }

        fn push_record(
            syntax: &mut SyntaxTrees,
            origin: TypeReferenceHandle,
            name: &str,
            members: Vec<DataMember>,
            is_public: bool,
            provenance: source::SourceSpan,
        ) {
            let mut first = Handle::invalid();
            let mut count = 0u32;
            for member in members {
                let handle = syntax.tables.items.append_data_member(member);
                if count == 0 {
                    first = handle;
                }
                count += 1;
            }
            syntax.push_root_item(Item::Data(DataDefinition {
                cleanup_selection: None,
                name: generated_identifier(name, provenance),
                is_public,
                supply_mode: DataSupplyMode::CheckedShape,
                lifetime_parameters: Vec::new(),
                type_parameters: HandleSpan::empty(),
                conformance_bounds: Vec::new(),
                generic_instance: Some(origin),
                properties: DataProperties {
                    multiplicity: Multiplicity::Linear,
                    carry: None,
                },
                requires_facts: HandleSpan::empty(),
                members: HandleSpan::from_parts(first, count),
                quotient: None,
            }));
        }

        fn rewrite_applications(syntax: &mut SyntaxTrees, rewrites: &[PendingRewrite]) {
            for rewrite in rewrites {
                syntax.tables.type_references.replace_type_reference(
                    rewrite.type_reference,
                    TypeReferenceNode::Named(Identifier::generated(
                        rewrite.synthetic_name.as_str(),
                    )),
                );
            }
        }
    }
    mod template_applications {
        use crate::build_time_evaluation::type_references;
        use syntax_trees::SyntaxTrees;
        use syntax_trees::expression::{ExpressionHandle, ExpressionNode, StaticMachineArgument};
        use syntax_trees::item::{
            ConformanceMember, DataMember, Item, Machine, TypeParameter, TypeParameterKind,
        };
        use syntax_trees::statement::{
            StatementHandle, StatementNode, TransitionGuardNode, TransitionTargetNode,
        };
        use syntax_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};

        // Clauses can depend on template arguments even when their parameter
        // and result types are closed. Keep those observations in the same
        // walk, masking each callable's own value formals.
        #[derive(Default)]
        struct TemplateDependencies {
            applications: Vec<TypeReferenceHandle>,
            depends_on_template: bool,
            bound_values: Vec<syntax_trees::identifier::Identifier>,
        }

        impl TemplateDependencies {
            fn observe_value_name(
                &mut self,
                syntax: &SyntaxTrees,
                binders: &[&str],
                name: &syntax_trees::identifier::Identifier,
            ) {
                if binders.contains(&name.as_str(syntax))
                    && !self
                        .bound_values
                        .iter()
                        .any(|bound| bound.as_str(syntax) == name.as_str(syntax))
                {
                    self.depends_on_template = true;
                }
            }
        }

        macro_rules! binders {
            ($syntax:expr => $parameters:expr => $outer:expr) => {
                telescope($syntax, $syntax.items.type_parameters($parameters), $outer)
            };
        }

        pub(super) fn scoped_template_applications(
            syntax: &SyntaxTrees,
            selected_owners: &[(source::SourceSpan, Result<source::SourceSpan, String>)],
        ) -> Vec<TypeReferenceHandle> {
            let mut deferred = TemplateDependencies::default();
            for item in syntax.root_items() {
                match item {
                    Item::Machine(machine) => {
                        visit_machine(syntax, machine, &[], selected_owners, &mut deferred)
                    }
                    Item::Proposition(definition) => {
                        let binders = binders!(syntax => definition.type_parameters => &[]);
                        visit_parameters(
                            syntax,
                            syntax.items.type_parameters(definition.type_parameters),
                            &binders,
                            &mut deferred,
                        );
                        for parameter in syntax.items.state_parameters(definition.parameters) {
                            visit_type(
                                syntax,
                                syntax.items.state_parameter(*parameter).type_reference,
                                &binders,
                                &mut deferred,
                            );
                        }
                        match &definition.body {
                            syntax_trees::item::PropositionBody::Witness { evidence } => {
                                visit_type(syntax, *evidence, &binders, &mut deferred);
                            }
                            syntax_trees::item::PropositionBody::Transparent { proposition } => {
                                visit_expression(syntax, *proposition, &binders, &mut deferred)
                            }
                            syntax_trees::item::PropositionBody::Primitive => {}
                        }
                    }
                    Item::Data(data) => {
                        let binders = binders!(syntax => data.type_parameters => &[]);
                        visit_parameters(
                            syntax,
                            syntax.items.type_parameters(data.type_parameters),
                            &binders,
                            &mut deferred,
                        );
                        visit_facts(
                            syntax,
                            syntax.items.proof_facts(data.requires_facts),
                            &binders,
                            &mut deferred,
                        );
                        if let Some(quotient) = &data.quotient {
                            visit_type(syntax, quotient.carrier, &binders, &mut deferred);
                        }
                        for member in syntax.items.data_members(data.members) {
                            match member {
                                DataMember::Field(field) => {
                                    visit_type(
                                        syntax,
                                        field.type_reference,
                                        &binders,
                                        &mut deferred,
                                    );
                                }
                                DataMember::Variant(variant) => {
                                    for field in syntax.items.data_payload_fields(variant.payload) {
                                        visit_type(
                                            syntax,
                                            field.type_reference,
                                            &binders,
                                            &mut deferred,
                                        );
                                    }
                                }
                                DataMember::Retired(_) => {}
                            }
                        }
                    }
                    Item::Trait(definition) => {
                        let binders = binders!(syntax => definition.type_parameters => &[]);
                        visit_parameters(
                            syntax,
                            syntax.items.type_parameters(definition.type_parameters),
                            &binders,
                            &mut deferred,
                        );
                        for parent in
                            type_references(syntax).type_reference_handles(definition.parents)
                        {
                            visit_type(syntax, *parent, &binders, &mut deferred);
                        }
                        if let Some(reference) = definition.refines {
                            visit_type(syntax, reference, &binders, &mut deferred);
                        }
                        for bound in &definition.conformance_bounds {
                            for argument in
                                type_references(syntax).type_reference_handles(bound.arguments)
                            {
                                visit_type(syntax, *argument, &binders, &mut deferred);
                            }
                        }
                        for signature in syntax.items.state_signatures(definition.machines) {
                            let signature = syntax.items.state_signature(*signature);
                            let binders = binders!(syntax => signature.type_parameters => &binders);
                            visit_parameters(
                                syntax,
                                syntax.items.type_parameters(signature.type_parameters),
                                &binders,
                                &mut deferred,
                            );
                            visit_contracts(
                                syntax,
                                syntax.items.capability_contracts(signature.contracts),
                                &binders,
                                &mut deferred,
                            );
                            visit_type(syntax, signature.return_type, &binders, &mut deferred);
                            for parameter in syntax.items.state_parameters(signature.parameters) {
                                visit_type(
                                    syntax,
                                    syntax.items.state_parameter(*parameter).type_reference,
                                    &binders,
                                    &mut deferred,
                                );
                            }
                            for statement in syntax.items.statements(signature.default_body) {
                                visit_statement(syntax, *statement, &binders, &mut deferred);
                            }
                        }
                    }
                    Item::Conformance(definition) => {
                        let binders = binders!(syntax => definition.type_parameters => &[]);
                        visit_parameters(
                            syntax,
                            syntax.items.type_parameters(definition.type_parameters),
                            &binders,
                            &mut deferred,
                        );
                        for argument in type_references(syntax)
                            .type_reference_handles(definition.trait_arguments)
                        {
                            visit_type(syntax, *argument, &binders, &mut deferred);
                        }
                        for member in syntax.items.conformance_members(definition.members) {
                            match member {
                                ConformanceMember::Machine(machine)
                                | ConformanceMember::Synthesized(machine)
                                | ConformanceMember::TraitDefault { machine, .. } => visit_machine(
                                    syntax,
                                    machine,
                                    &binders,
                                    selected_owners,
                                    &mut deferred,
                                ),
                                // A selection row names an existing machine and has no body.
                                ConformanceMember::Selection { .. } => {}
                            }
                        }
                    }
                    _ => {}
                }
            }
            deferred.applications
        }

        fn telescope<'a>(
            syntax: &'a SyntaxTrees,
            parameters: &'a [TypeParameter],
            outer: &[&'a str],
        ) -> Vec<&'a str> {
            let mut binders = outer.to_vec();
            binders.extend(
                parameters
                    .iter()
                    .map(|parameter| parameter.name.as_str(syntax)),
            );
            binders
        }

        fn visit_parameters(
            syntax: &SyntaxTrees,
            parameters: &[TypeParameter],
            binders: &[&str],
            deferred: &mut TemplateDependencies,
        ) {
            for parameter in parameters {
                match &parameter.kind {
                    TypeParameterKind::Const { type_reference }
                    | TypeParameterKind::Value { type_reference } => {
                        visit_type(syntax, *type_reference, binders, deferred);
                    }
                    TypeParameterKind::Machine {
                        contract:
                            Some(syntax_trees::item::MachineParameterContract::Structural(signature)),
                    } => {
                        visit_signature(syntax, signature, binders, deferred);
                    }
                    TypeParameterKind::Machine {
                        contract:
                            Some(syntax_trees::item::MachineParameterContract::Nominal {
                                trait_arguments,
                                ..
                            }),
                    } => {
                        for argument in
                            type_references(syntax).type_reference_handles(*trait_arguments)
                        {
                            visit_type(syntax, *argument, binders, deferred);
                        }
                    }
                    TypeParameterKind::Proposition {
                        contract: Some(signature),
                    } => {
                        for parameter in syntax.items.state_parameters(signature.parameters) {
                            visit_type(
                                syntax,
                                syntax.items.state_parameter(*parameter).type_reference,
                                binders,
                                deferred,
                            );
                        }
                    }
                    _ => {}
                }
            }
        }

        fn visit_signature(
            syntax: &SyntaxTrees,
            signature: &syntax_trees::item::StateSignature,
            outer: &[&str],
            deferred: &mut TemplateDependencies,
        ) -> bool {
            let binders = binders!(syntax => signature.type_parameters => outer);
            let previously_dependent = deferred.depends_on_template;
            deferred.depends_on_template = false;
            let values_start = deferred.bound_values.len();
            deferred.bound_values.extend(
                syntax
                    .items
                    .state_parameters(signature.parameters)
                    .iter()
                    .map(|parameter| syntax.items.state_parameter(*parameter).name.clone()),
            );
            visit_parameters(
                syntax,
                syntax.items.type_parameters(signature.type_parameters),
                &binders,
                deferred,
            );
            visit_type(syntax, signature.return_type, &binders, deferred);
            for parameter in syntax.items.state_parameters(signature.parameters) {
                visit_type(
                    syntax,
                    syntax.items.state_parameter(*parameter).type_reference,
                    &binders,
                    deferred,
                );
            }
            for bound in &signature.conformance_bounds {
                for argument in type_references(syntax).type_reference_handles(bound.arguments) {
                    visit_type(syntax, *argument, &binders, deferred);
                }
            }
            visit_contracts(
                syntax,
                syntax.items.capability_contracts(signature.contracts),
                &binders,
                deferred,
            );
            for name in syntax
                .items
                .identifier_path_members(signature.service_reaches)
                .iter()
                .chain(syntax.items.identifier_path_members(signature.invokes))
            {
                deferred.observe_value_name(syntax, &binders, name);
            }
            let depends = deferred.depends_on_template;
            deferred.depends_on_template |= previously_dependent;
            deferred.bound_values.truncate(values_start);
            depends
        }

        fn visit_machine(
            syntax: &SyntaxTrees,
            machine: &Machine,
            outer: &[&str],
            selected_owners: &[(source::SourceSpan, Result<source::SourceSpan, String>)],
            deferred: &mut TemplateDependencies,
        ) {
            let mut ambient = outer.to_vec();
            if let Some(selected) = selected_owners
                .iter()
                .find(|(declaration, _)| *declaration == machine.name.source_span())
                .and_then(|(_, owner)| owner.as_ref().ok())
            {
                let mut owners = syntax.root_items().filter_map(|item| match item {
                    Item::Data(data) if data.name.source_span() == *selected => Some(data),
                    _ => None,
                });
                if let (Some(owner), None) = (owners.next(), owners.next()) {
                    ambient.extend(
                        syntax
                            .items
                            .type_parameters(owner.type_parameters)
                            .iter()
                            .map(|parameter| parameter.name.as_str(syntax)),
                    );
                }
            }
            let binders = binders!(syntax => machine.type_parameters => &ambient);
            if binders.is_empty() {
                return;
            }
            visit_contracts(
                syntax,
                syntax.items.capability_contracts(machine.contracts),
                &binders,
                deferred,
            );
            for clause in syntax.items.satisfies_clauses(machine.satisfies) {
                for argument in type_references(syntax).type_reference_handles(clause.arguments) {
                    visit_type(syntax, *argument, &binders, deferred);
                }
                visit_expression(syntax, clause.via_expression, &binders, deferred);
            }
            for expression in syntax
                .expressions
                .expression_handles(machine.ranking_subjects)
                .iter()
                .chain(
                    syntax
                        .expressions
                        .expression_handles(machine.ranking_view_arguments),
                )
            {
                visit_expression(syntax, *expression, &binders, deferred);
            }
            visit_expression(syntax, machine.ranking_range, &binders, deferred);
            visit_parameters(
                syntax,
                syntax.items.type_parameters(machine.type_parameters),
                &binders,
                deferred,
            );
            for bound in &machine.conformance_bounds {
                for argument in type_references(syntax).type_reference_handles(bound.arguments) {
                    visit_type(syntax, *argument, &binders, deferred);
                }
            }
            for state in syntax.items.state_handles(machine.states) {
                let state = syntax.items.state(*state);
                visit_contracts(
                    syntax,
                    syntax.items.capability_contracts(state.contracts),
                    &binders,
                    deferred,
                );
                visit_type(syntax, state.return_type, &binders, deferred);
                for parameter in syntax.items.state_parameters(state.parameters) {
                    visit_type(
                        syntax,
                        syntax.items.state_parameter(*parameter).type_reference,
                        &binders,
                        deferred,
                    );
                }
                for statement in syntax.items.statements(state.statements) {
                    visit_statement(syntax, *statement, &binders, deferred);
                }
            }
        }

        fn visit_type(
            syntax: &SyntaxTrees,
            reference: TypeReferenceHandle,
            binders: &[&str],
            deferred: &mut TemplateDependencies,
        ) -> bool {
            if !reference.is_valid() {
                return false;
            }
            let previously_dependent = deferred.depends_on_template;
            deferred.depends_on_template = false;
            let direct_dependency = match syntax.type_references.type_reference(reference) {
                TypeReferenceNode::Named(name) => {
                    let depends = binders.contains(&name.as_str(syntax));
                    deferred.depends_on_template |= depends;
                    depends
                }
                TypeReferenceNode::Callable { signature } => visit_signature(
                    syntax,
                    syntax.items.state_signature(*signature),
                    binders,
                    deferred,
                ),
                TypeReferenceNode::Reference { referee, .. } => {
                    visit_type(syntax, *referee, binders, deferred)
                }
                TypeReferenceNode::Constrained {
                    base_type,
                    constraints,
                } => {
                    let mut depends = visit_type(syntax, *base_type, binders, deferred);
                    for constraint in syntax.type_references.constraints(*constraints) {
                        if let TypeConstraintNode::Domain(domain) = constraint {
                            deferred.observe_value_name(syntax, binders, &domain.name);
                            for argument in
                                type_references(syntax).type_reference_handles(domain.arguments)
                            {
                                depends |= visit_type(syntax, *argument, binders, deferred);
                            }
                        }
                    }
                    depends
                }
                TypeReferenceNode::FixedArray {
                    element_type,
                    length,
                } => {
                    let depends = visit_type(syntax, *element_type, binders, deferred);
                    match length {
                        syntax_trees::types::FixedArrayLength::Literal(_) => {}
                        syntax_trees::types::FixedArrayLength::ConstParameter(name)
                        | syntax_trees::types::FixedArrayLength::ConstCall(name) => {
                            deferred.observe_value_name(syntax, binders, name);
                        }
                        syntax_trees::types::FixedArrayLength::Expression(reference) => {
                            visit_type(syntax, *reference, binders, deferred);
                        }
                    }
                    depends
                }
                TypeReferenceNode::Slice { element_type } => {
                    visit_type(syntax, *element_type, binders, deferred)
                }
                TypeReferenceNode::Generic { arguments, .. } => {
                    let mut depends = false;
                    for argument in syntax.type_references.type_reference_handles(*arguments) {
                        depends |= visit_type(syntax, *argument, binders, deferred);
                    }
                    if depends && !deferred.applications.contains(&reference) {
                        deferred.applications.push(reference);
                    }
                    depends
                }
                TypeReferenceNode::DynamicTrait {
                    trait_arguments, ..
                } => {
                    let mut depends = false;
                    for argument in type_references(syntax).type_reference_handles(*trait_arguments)
                    {
                        depends |= visit_type(syntax, *argument, binders, deferred);
                    }
                    depends
                }
                TypeReferenceNode::ConstExpression(expression) => {
                    visit_expression(syntax, *expression, binders, deferred);
                    false
                }
                TypeReferenceNode::StaticSelection(selection) => visit_static_arguments(
                    syntax,
                    std::slice::from_ref(selection),
                    binders,
                    deferred,
                ),
                TypeReferenceNode::Unit => false,
            };
            let depends = direct_dependency || deferred.depends_on_template;
            deferred.depends_on_template = previously_dependent || depends;
            depends
        }

        fn visit_static_arguments(
            syntax: &SyntaxTrees,
            arguments: &[StaticMachineArgument],
            binders: &[&str],
            deferred: &mut TemplateDependencies,
        ) -> bool {
            let mut depends = false;
            for argument in arguments {
                if let Some(name) = argument.path.first() {
                    deferred.observe_value_name(syntax, binders, name);
                }
                if let Some(projection) = &argument.evidence_projection {
                    deferred.observe_value_name(syntax, binders, &projection.term);
                }
                depends |= visit_type(syntax, argument.type_reference, binders, deferred);
                if let Some(application) = &argument.application {
                    depends |=
                        visit_static_arguments(syntax, &application.arguments, binders, deferred);
                    if let Some(member) = syntax.expressions.static_member(application.member) {
                        depends |= visit_static_arguments(
                            syntax,
                            std::slice::from_ref(member),
                            binders,
                            deferred,
                        );
                    }
                }
            }
            depends
        }

        fn visit_statement(
            syntax: &SyntaxTrees,
            statement: StatementHandle,
            binders: &[&str],
            deferred: &mut TemplateDependencies,
        ) {
            match syntax.statements.statement(statement) {
                StatementNode::Assembly(handle) => {
                    let graph = syntax.statements.assembly(*handle);
                    for block in syntax.statements.assembly_blocks(graph.blocks) {
                        for expression in syntax
                            .statements
                            .assembly_expressions(block.requires)
                            .iter()
                            .chain(syntax.statements.assembly_expressions(block.ensures))
                        {
                            visit_expression(syntax, *expression, binders, deferred);
                        }
                        for instruction in
                            syntax.statements.assembly_instructions(block.instructions)
                        {
                            visit_statement(syntax, *instruction, binders, deferred);
                        }
                    }
                }
                StatementNode::RootBinding(binding) => {
                    visit_expression(syntax, binding.receiver, binders, deferred);
                    visit_expression(syntax, binding.implementation_operand, binders, deferred);
                }
                StatementNode::AssemblyFact(fact) => {
                    visit_expression(syntax, fact.expression, binders, deferred)
                }
                StatementNode::Assignment(assignment) => {
                    visit_expression(syntax, assignment.target, binders, deferred);
                    visit_expression(syntax, assignment.value, binders, deferred);
                }
                StatementNode::Call(call) => {
                    deferred.observe_value_name(syntax, binders, &call.target);
                    visit_expression(syntax, call.callee, binders, deferred);
                    visit_static_arguments(syntax, &call.machine_arguments, binders, deferred);
                    for argument in syntax.statements.expression_handles(call.arguments) {
                        visit_expression(syntax, *argument, binders, deferred);
                    }
                }
                StatementNode::ProofOutputBindingStatement(binding) => {
                    visit_expression(syntax, binding.call, binders, deferred)
                }
                StatementNode::Expression(expression) => {
                    visit_expression(syntax, *expression, binders, deferred)
                }
                StatementNode::LocalData(local) => {
                    visit_type(syntax, local.type_reference, binders, deferred);
                    visit_expression(syntax, local.initial_value, binders, deferred);
                }
                StatementNode::Transition(transition) => {
                    if let TransitionGuardNode::When(guard) = transition.guard {
                        visit_expression(syntax, guard, binders, deferred);
                    }
                    for target in transition.targets() {
                        match syntax.statements.transition_target(target) {
                            TransitionTargetNode::Named { arguments, .. } => {
                                for argument in syntax.statements.expression_handles(*arguments) {
                                    visit_expression(syntax, *argument, binders, deferred);
                                }
                            }
                            TransitionTargetNode::Value(value) => {
                                visit_expression(syntax, *value, binders, deferred)
                            }
                            TransitionTargetNode::Terminal => {}
                        }
                    }
                }
            }
        }

        fn visit_expression(
            syntax: &SyntaxTrees,
            expression: ExpressionHandle,
            binders: &[&str],
            deferred: &mut TemplateDependencies,
        ) {
            if !expression.is_valid() {
                return;
            }
            match syntax.expressions.expression(expression) {
                ExpressionNode::SavedSubject(_) => {}
                ExpressionNode::Match(dispatch) => {
                    visit_expression(syntax, dispatch.subject, binders, deferred);
                    for arm in syntax.expressions.match_arms(dispatch.arms) {
                        for dependency in syntax.expressions.match_pattern_dependencies(arm.pattern)
                        {
                            visit_expression(syntax, dependency, binders, deferred);
                        }
                        visit_expression(syntax, arm.value, binders, deferred);
                    }
                }
                ExpressionNode::ArrayLiteral(expressions) => {
                    for expression in syntax.expressions.expression_handles(*expressions) {
                        visit_expression(syntax, *expression, binders, deferred);
                    }
                }
                ExpressionNode::Atomic(atomic) => {
                    visit_expression(syntax, atomic.value, binders, deferred);
                    visit_expression(syntax, atomic.result, binders, deferred);
                }
                ExpressionNode::Binary(binary) => {
                    visit_expression(syntax, binary.left, binders, deferred);
                    visit_expression(syntax, binary.right, binders, deferred);
                }
                ExpressionNode::Cast(cast) => {
                    visit_type(syntax, cast.target_type, binders, deferred);
                    for argument in type_references(syntax)
                        .type_reference_handles(cast.semantic_domain_arguments)
                    {
                        visit_type(syntax, *argument, binders, deferred);
                    }
                    visit_expression(syntax, cast.value, binders, deferred);
                }
                ExpressionNode::Call(call) => {
                    deferred.observe_value_name(syntax, binders, &call.target);
                    visit_expression(syntax, call.callee, binders, deferred);
                    visit_static_arguments(syntax, &call.machine_arguments, binders, deferred);
                    visit_expression(syntax, call.receiver, binders, deferred);
                    for argument in syntax.expressions.expression_handles(call.arguments) {
                        visit_expression(syntax, *argument, binders, deferred);
                    }
                }
                ExpressionNode::Indexed(indexed) => {
                    visit_expression(syntax, indexed.collection, binders, deferred);
                    visit_expression(syntax, indexed.index, binders, deferred);
                }
                ExpressionNode::Membership(membership) => {
                    if let Some(name) = syntax
                        .items
                        .identifier_path_members(membership.domain)
                        .first()
                    {
                        deferred.observe_value_name(syntax, binders, name);
                    }
                    visit_expression(syntax, membership.value, binders, deferred)
                }
                ExpressionNode::Member(member) => {
                    visit_expression(syntax, member.receiver, binders, deferred)
                }
                ExpressionNode::Borrow(borrow) => {
                    visit_expression(syntax, borrow.target, binders, deferred)
                }
                ExpressionNode::Range(range) => {
                    visit_expression(syntax, range.start, binders, deferred);
                    visit_expression(syntax, range.end, binders, deferred);
                }
                ExpressionNode::StructLiteral(literal) => {
                    visit_type(syntax, literal.application_type, binders, deferred);
                    deferred.observe_value_name(syntax, binders, &literal.constructor_name);
                    for field in syntax.expressions.struct_fields(literal.fields) {
                        visit_expression(syntax, field.value, binders, deferred);
                    }
                }
                ExpressionNode::Unary(unary) => {
                    visit_expression(syntax, unary.operand, binders, deferred)
                }
                ExpressionNode::TypeExpression(reference)
                | ExpressionNode::ZeroValue(reference) => {
                    visit_type(syntax, *reference, binders, deferred);
                }
                ExpressionNode::Name(path) => {
                    if let Some(name) = syntax.items.identifier_path_members(*path).first() {
                        deferred.observe_value_name(syntax, binders, name);
                    }
                }
                ExpressionNode::Boolean(_)
                | ExpressionNode::Float(_)
                | ExpressionNode::UndeterminedNaN(_)
                | ExpressionNode::Integer(_)
                | ExpressionNode::String(_) => {}
            }
        }

        fn visit_facts(
            syntax: &SyntaxTrees,
            facts: &[syntax_trees::item::ProofFact],
            binders: &[&str],
            deferred: &mut TemplateDependencies,
        ) {
            for fact in facts {
                match fact {
                    syntax_trees::item::ProofFact::Expression(expression) => {
                        visit_expression(syntax, *expression, binders, deferred)
                    }
                    syntax_trees::item::ProofFact::Membership(membership) => {
                        visit_expression(syntax, membership.value, binders, deferred);
                        for argument in type_references(syntax)
                            .type_reference_handles(membership.domain_arguments)
                        {
                            visit_type(syntax, *argument, binders, deferred);
                        }
                    }
                }
            }
        }

        fn visit_contracts(
            syntax: &SyntaxTrees,
            contracts: &[syntax_trees::item::CapabilityContract],
            binders: &[&str],
            deferred: &mut TemplateDependencies,
        ) {
            for contract in contracts {
                visit_facts(
                    syntax,
                    syntax.items.proof_facts(contract.facts),
                    binders,
                    deferred,
                );
            }
        }
    }

    use template_applications::scoped_template_applications;

    use plan_installation::{install_access_view_plan, selected_access_view};
    use record_synthesis::{
        accessor_operations, retire_accessor_templates, synthesize_exact_records,
        synthesize_probe_records,
    };

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct AccessViewRecord {
        pub synthetic_name: String,
        pub policy_machine: String,
        pub schema_data: String,
        pub normalized_access_plan: AccessPlanId,
        pub invocation_sources: Vec<source::SourceSpan>,
        template_source: source::SourceSpan,
        policy_source: source::SourceSpan,
        policy_machine_source: source::SourceSpan,
        schema_source: source::SourceSpan,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
    struct DeclarationKey(usize, usize, usize);

    impl From<source::SourceSpan> for DeclarationKey {
        fn from(source: source::SourceSpan) -> Self {
            Self(source.source_id.0, source.span.start, source.span.end)
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
    struct ApplicationKey {
        template: DeclarationKey,
        policy: DeclarationKey,
        schema: DeclarationKey,
    }

    #[derive(Clone)]
    struct SchemaRecord {
        fields: Vec<DataField>,
    }

    #[derive(Clone)]
    struct DataCandidate {
        name: String,
        source: source::SourceSpan,
        is_public: bool,
        plain: bool,
        supply_mode: DataSupplyMode,
        fields: Vec<DataField>,
        member_count: usize,
    }

    #[derive(Clone)]
    struct PolicyMachineCandidate {
        machine_source: source::SourceSpan,
    }

    #[derive(Debug, Clone)]
    struct Application {
        key: ApplicationKey,
        origin: TypeReferenceHandle,
        synthetic_name: String,
        policy: String,
        schema: String,
        template_source: source::SourceSpan,
        policy_source: source::SourceSpan,
        policy_machine_source: source::SourceSpan,
        schema_source: source::SourceSpan,
        generated_is_public: bool,
        invocation_sources: Vec<source::SourceSpan>,
    }

    #[derive(Debug, Clone)]
    pub(crate) struct PendingRewrite {
        pub(crate) type_reference: TypeReferenceHandle,
        pub(crate) synthetic_name: String,
    }

    type Discovery = (
        Vec<Application>,
        Vec<PendingRewrite>,
        BTreeMap<DeclarationKey, SchemaRecord>,
    );

    pub fn desugar_access_views(
        syntax: &mut SyntaxTrees,
        context: crate::BuildTimeSources<'_>,
    ) -> Result<Vec<AccessViewRecord>, Vec<Diagnostic>> {
        let sources = context.sources.clone();
        let source_scoped_top_level_bindings = context.source_scoped_top_level_bindings;
        let selection_authority = context.selection_authority.clone();
        let retained_templates: Option<&SyntaxTrees> =
            if has_accessor_template(syntax, sources.as_deref()) {
                None
            } else {
                [
                    context.declaration_syntax,
                    context.retained_declaration_syntax,
                ]
                .into_iter()
                .flatten()
                .find(|forest| has_accessor_template(forest, sources.as_deref()))
            };
        let owner_requests = syntax
            .root_items()
            .filter_map(|item| match item {
                Item::Machine(machine) => machine
                    .attached_data
                    .as_ref()
                    .map(|owner| (machine.name.source_span(), owner.clone())),
                _ => None,
            })
            .collect::<Vec<_>>();
        let owner_declarations =
        syntax_trees_to_symbol_resolved_trees::pre_resolution::select_data_declarations_with_retained(
            syntax,
            retained_templates,
            sources.clone(),
            source_scoped_top_level_bindings.to_vec(),
            &owner_requests
                .iter()
                .map(|(_, owner)| owner.clone())
                .collect::<Vec<_>>(),
            context.retained_base(),
        )?;
        let selected_owners = owner_requests
            .into_iter()
            .zip(owner_declarations)
            .map(|((machine, _), owner)| (machine, owner))
            .collect::<Vec<_>>();
        let mut selected_arguments = applied_results::retain_applied_results(syntax, &context)?;
        let requests = type_references(syntax)
            .generic_nodes()
            .into_iter()
            .filter_map(|reference| {
                let TypeReferenceNode::Generic {
                    base_name,
                    arguments,
                    ..
                } = syntax.type_references.type_reference(reference)
                else {
                    return None;
                };
                Some((
                    reference,
                    base_name.clone(),
                    type_references(syntax)
                        .type_reference_handles(*arguments)
                        .to_vec(),
                ))
            })
            .collect::<Vec<_>>();
        let names = requests
            .iter()
            .flat_map(|(reference, base, arguments)| {
                let mut names = vec![(*reference, base.clone())];
                for argument in arguments {
                    if let TypeReferenceNode::Named(name) =
                        syntax.type_references.type_reference(*argument)
                    {
                        names.push((*argument, name.clone()));
                    }
                }
                names
            })
            .filter(|(reference, _)| {
                !selected_arguments
                    .iter()
                    .any(|selected| selected.reference == *reference)
            })
            .collect::<Vec<_>>();
        let declarations =
        syntax_trees_to_symbol_resolved_trees::pre_resolution::select_data_declarations_with_retained(
            syntax,
            retained_templates,
            sources.clone(),
            source_scoped_top_level_bindings.to_vec(),
            &names
                .iter()
                .map(|(_, name)| name.clone())
                .collect::<Vec<_>>(),
            context.retained_base(),
        )?;
        for ((reference, _), declaration) in names.into_iter().zip(declarations) {
            selected_arguments.push(applied_results::SelectedArgument {
                reference,
                declaration,
            });
        }
        let (applications, rewrites, schemas) = discover_applications(
            syntax,
            retained_templates,
            sources.as_deref(),
            selection_authority.as_deref(),
            &selected_arguments,
            &selected_owners,
        )?;
        if applications.is_empty() {
            retire_accessor_templates(syntax, sources.as_deref());
            return Ok(Vec::new());
        }

        let mut probe = syntax.clone();
        synthesize_probe_records(
            &mut probe,
            retained_templates,
            &applications,
            &rewrites,
            &schemas,
            sources.as_deref(),
        )?;
        let probe = crate::syntax_probes::normalize_generic_data(
            probe,
            sources.clone(),
            source_scoped_top_level_bindings,
            context.retained_base(),
            [
                context.declaration_syntax,
                context.retained_declaration_syntax,
            ],
        )?;
        let frontier = syntax_trees_to_symbol_resolved_trees::resolve_deferred(
            context.resolution_request(probe),
        )?;
        let (mut typed, domain) =
            prepare_access_probe(frontier, &applications, context.predecessor.clone())?;
        crate::const_evaluation::const_lengths::evaluate_const_array_lengths_in(
            &mut typed,
            selection_authority.clone(),
            &domain,
        )?;
        crate::const_evaluation::const_domain_facts::evaluate_selected_domain_facts(
            &mut typed,
            selection_authority.clone(),
            crate::SelectedBuildTimeOperators::default(),
            &domain,
        )?;
        super::plan_laid::compute_plan_laid_layouts_in(
            &mut typed,
            selection_authority.clone(),
            &domain,
        )?;
        let execution = domain.execution_program(&typed);

        let mut plans = BTreeMap::new();
        for application in &applications {
            let (policy_machine, schema) = selected_access_declarations(
                &execution,
                application.policy_machine_source,
                application.schema_source,
            )
            .map_err(|reason| vec![Diagnostic::error(reason)])?;
            admit_policy_invocations(
                &execution,
                policy_machine,
                &application.invocation_sources,
                selection_authority.clone(),
            )
            .map_err(|reason| {
                vec![Diagnostic::error(format!(
                    "access view `{}`: {reason}",
                    application.synthetic_name
                ))]
            })?;
            let plan = super::access_plans::compute_selected_access_plan(
                &execution,
                policy_machine,
                schema,
                None,
                &domain.accounting,
            )
            .map_err(|reason| {
                vec![Diagnostic::error(format!(
                    "access view `{}`: {reason}",
                    application.synthetic_name
                ))]
            })?;
            plans.insert(application.key, plan);
        }

        synthesize_exact_records(
            syntax,
            retained_templates,
            &applications,
            &rewrites,
            &schemas,
            &plans,
            sources.as_deref(),
        )?;
        Ok(applications
            .into_iter()
            .map(|application| {
                let plan = &plans[&application.key];
                AccessViewRecord {
                    synthetic_name: application.synthetic_name,
                    policy_machine: format!("{}::plan", application.policy),
                    schema_data: application.schema,
                    normalized_access_plan: plan.identity(),
                    invocation_sources: application.invocation_sources,
                    template_source: application.template_source,
                    policy_source: application.policy_source,
                    policy_machine_source: application.policy_machine_source,
                    schema_source: application.schema_source,
                }
            })
            .collect())
    }

    fn prepare_access_probe(
        frontier: syntax_trees_to_symbol_resolved_trees::DeferredResolvedFrontier,
        applications: &[Application],
        predecessor: Option<symbol_resolved_trees_to_typed_trees::SeededTypingBase>,
    ) -> Result<(TypedTrees, EvaluationDomain), Vec<Diagnostic>> {
        let machines = applications
            .iter()
            .map(|application| application.policy_machine_source)
            .collect::<Vec<_>>();
        let mut declarations = Vec::new();
        for source in applications
            .iter()
            .flat_map(|application| [application.policy_source, application.schema_source])
        {
            let mut candidates = frontier
                .trees()
                .roots
                .data_definitions
                .iter()
                .filter(|data| {
                    frontier.trees().symbols.symbol_source_span(data.symbol) == Some(source)
                });
            let (Some(declaration), None) = (candidates.next(), candidates.next()) else {
                return Err(vec![
                    Diagnostic::error("access-view probe lost its unique reflected declaration")
                        .with_source_span(source),
                ]);
            };
            push_unique(&mut declarations, declaration.symbol);
        }
        crate::syntax_probes::prepare_evaluation(
            frontier,
            &machines,
            &[],
            &declarations,
            predecessor,
        )
    }

    pub(crate) fn validate_access_view_plans_in(
        typed: &mut TypedTrees,
        records: &[AccessViewRecord],
        selection_authority: Option<Arc<dyn crate::BuildTimeSelectionAuthority>>,
        domain: &crate::build_time_evaluation::EvaluationDomain,
    ) -> Result<(), Vec<Diagnostic>> {
        if records.is_empty() {
            return Ok(());
        }
        let mut roots = Vec::new();
        for record in records {
            let (policy, schema) = selected_access_declarations(
                typed,
                record.policy_machine_source,
                record.schema_source,
            )
            .map_err(|reason| vec![Diagnostic::error(reason)])?;
            let (view, _) =
                selected_access_view(typed, record, policy.attached_data_symbol, schema.symbol)?;
            push_unique(&mut roots, policy.symbol);
            push_unique(&mut roots, schema.symbol);
            push_unique(&mut roots, view.symbol);
        }
        let evaluation = domain.demanded_roots(typed, &roots)?;
        let mut execution = evaluation.execution_program(typed);
        crate::const_evaluation::const_lengths::evaluate_const_array_lengths_in(
            &mut execution,
            selection_authority.clone(),
            &evaluation,
        )?;
        crate::const_evaluation::const_domain_facts::evaluate_selected_domain_facts(
            &mut execution,
            selection_authority.clone(),
            crate::SelectedBuildTimeOperators::default(),
            &evaluation,
        )?;
        super::plan_laid::compute_plan_laid_layouts_in(
            &mut execution,
            selection_authority.clone(),
            &evaluation,
        )?;
        let mut plans = Vec::with_capacity(records.len());
        for record in records {
            let (policy_machine, schema) = selected_access_declarations(
                &execution,
                record.policy_machine_source,
                record.schema_source,
            )
            .map_err(|reason| vec![Diagnostic::error(reason)])?;
            admit_policy_invocations(
                &execution,
                policy_machine,
                &record.invocation_sources,
                selection_authority.clone(),
            )
            .map_err(|reason| {
                vec![Diagnostic::error(format!(
                    "access view `{}`: {reason}",
                    record.synthetic_name
                ))]
            })?;
            let plan = super::access_plans::compute_selected_access_plan(
                &execution,
                policy_machine,
                schema,
                None,
                &domain.accounting,
            )
            .map_err(|reason| {
                vec![Diagnostic::error(format!(
                    "access view `{}`: {reason}",
                    record.synthetic_name
                ))]
            })?;
            if plan.identity() != record.normalized_access_plan {
                return Err(vec![Diagnostic::error(format!(
                    "access view `{}` changed normalized placement identity between accessor derivation and the authoritative typed program",
                    record.synthetic_name
                ))]);
            }
            plans.push(plan);
        }
        crate::const_evaluation::const_lengths::publish_evaluated_array_lengths(
            typed,
            &execution,
            &evaluation,
        );
        for (record, plan) in records.iter().zip(&plans) {
            install_access_view_plan(typed, record, plan)?;
        }
        Ok(())
    }

    fn selected_access_declarations(
        typed: &TypedTrees,
        policy_source: source::SourceSpan,
        schema_source: source::SourceSpan,
    ) -> Result<
        (
            &typed_trees::machine::Machine,
            &typed_trees::data::DataDefinition,
        ),
        String,
    > {
        let mut machines = typed.machines().iter().filter(|machine| {
            typed.symbols.symbol_source_span(machine.symbol) == Some(policy_source)
        });
        let (Some(machine), None) = (machines.next(), machines.next()) else {
            return Err(
                "access-view evaluation lost its unique selected policy machine".to_owned(),
            );
        };
        let mut schemas = typed.data_definitions().iter().filter(|schema| {
            typed.symbols.symbol_source_span(schema.symbol) == Some(schema_source)
        });
        let (Some(schema), None) = (schemas.next(), schemas.next()) else {
            return Err(
                "access-view evaluation lost its unique selected schema declaration".to_owned(),
            );
        };
        Ok((machine, schema))
    }

    fn admit_policy_invocations(
        typed: &TypedTrees,
        machine: &typed_trees::machine::Machine,
        invocation_sources: &[source::SourceSpan],
        selection_authority: Option<Arc<dyn crate::BuildTimeSelectionAuthority>>,
    ) -> Result<(), String> {
        let Some(selection_authority) = selection_authority else {
            return Ok(());
        };
        let admission = crate::BuildTimeAdmissionPlan::infer(typed, Some(selection_authority))
            .map_err(|diagnostics| diagnostics::join_messages(&diagnostics))?;
        for source in invocation_sources {
            admission.require_common_floor_for_invocation(
                typed,
                machine,
                crate::BuildTimeInvocationCustody::Source(*source),
            )?;
        }
        Ok(())
    }

    fn discover_applications(
        syntax: &SyntaxTrees,
        retained_templates: Option<&SyntaxTrees>,
        sources: Option<&source::SourceMap>,
        selection_authority: Option<&dyn crate::BuildTimeSelectionAuthority>,
        selected_arguments: &[applied_results::SelectedArgument],
        selected_owners: &[(source::SourceSpan, Result<source::SourceSpan, String>)],
    ) -> Result<Discovery, Vec<Diagnostic>> {
        let mut data = Vec::new();
        let mut access_policies = Vec::new();
        for item in syntax.root_items() {
            match item {
                Item::Data(definition) => {
                    let fields = syntax
                        .tables
                        .items
                        .data_members(definition.members)
                        .iter()
                        .filter_map(|member| match member {
                            DataMember::Field(field) => Some(field.clone()),
                            DataMember::Variant(_) | DataMember::Retired(_) => None,
                        })
                        .collect::<Vec<_>>();
                    data.push(DataCandidate {
                        name: definition.name.as_str(syntax).to_owned(),
                        source: definition.name.source_span(),
                        is_public: definition.is_public,
                        plain: definition.type_parameters.is_empty(),
                        supply_mode: definition.supply_mode,
                        fields,
                        member_count: syntax.tables.items.data_members(definition.members).len(),
                    });
                }
                Item::Machine(machine)
                    if machine.attached_data.as_ref().is_some_and(|attached| {
                        machine.name.as_str(syntax) == format!("{}::plan", attached.as_str(syntax))
                    }) =>
                {
                    access_policies.push(PolicyMachineCandidate {
                        machine_source: machine.name.source_span(),
                    });
                }
                _ => {}
            }
        }
        if let Some(forest) = retained_templates {
            for item in forest.root_items() {
                let Item::Data(definition) = item else {
                    continue;
                };
                if !toolchain_layout_declaration(sources, definition.name.source_span()) {
                    continue;
                }
                data.push(DataCandidate {
                    name: definition.name.as_str(forest).to_owned(),
                    source: definition.name.source_span(),
                    is_public: definition.is_public,
                    plain: definition.type_parameters.is_empty(),
                    supply_mode: definition.supply_mode,
                    fields: Vec::new(),
                    member_count: forest.tables.items.data_members(definition.members).len(),
                });
            }
        }

        let mut diagnostics = Vec::new();
        let mut applications = Vec::new();
        let mut rewrites = Vec::new();
        let mut schemas = BTreeMap::new();
        let deferred = scoped_template_applications(syntax, selected_owners);
        for type_reference in syntax.tables.type_references.generic_nodes() {
            if deferred.contains(&type_reference) {
                continue;
            }
            let TypeReferenceNode::Generic {
                base_name,
                lifetime_arguments,
                arguments,
            } = syntax.tables.type_references.type_reference(type_reference)
            else {
                unreachable!("generic_nodes returned a non-generic node");
            };
            let template = selected_arguments
                .iter()
                .find(|selected| selected.reference == type_reference)
                .and_then(|selected| selected.declaration.as_ref().ok().copied());
            let canonical = template
                .and_then(|declaration| sources.and_then(|sources| sources.file_at(declaration)))
                .is_some_and(|source| {
                    symbols::core_source_at(source)
                        == Some(language_core::core_vocabulary::CoreSource::Layout)
                })
                && template.is_some_and(|declaration| {
                    data.iter().any(|candidate| {
                        candidate.source == declaration
                            && candidate.name.rsplit("::").next() == Some("Access")
                    })
                });
            if !canonical && (sources.is_some() || base_name.as_str(syntax) != "Access") {
                continue;
            }
            if !lifetime_arguments.is_empty() {
                diagnostics.push(Diagnostic::error(
                    "`Access<Policy, Schema>` takes no authored lifetime arguments",
                ));
                continue;
            }
            let argument_handles =
                type_references(&syntax.tables).type_reference_handles(*arguments);
            let [policy_handle, schema_handle] = argument_handles else {
                diagnostics.push(Diagnostic::error(
                    "`Access` takes exactly a nominal access policy and a plain schema",
                ));
                continue;
            };
            let Some((mut policy, policy_source)) = named_argument(syntax, *policy_handle) else {
                diagnostics.push(Diagnostic::error(
                    "the first `Access` argument must be a nominal access-policy data name",
                ));
                continue;
            };
            let Some((mut schema, schema_use_source)) = named_argument(syntax, *schema_handle)
            else {
                diagnostics.push(Diagnostic::error(
                    "the second `Access` argument must be a plain schema data name",
                ));
                continue;
            };
            let mut selection_failed = false;
            for (handle, reference) in [
                (*policy_handle, policy_source),
                (*schema_handle, schema_use_source),
            ] {
                match selected_arguments
                    .iter()
                    .find(|selected| selected.reference == handle)
                {
                    Some(selected) => {
                        if let Err(reason) = &selected.declaration {
                            diagnostics.push(
                                Diagnostic::error(reason.clone()).with_source_span(reference),
                            );
                            selection_failed = true;
                        }
                    }
                    None if sources.is_some() => {
                        diagnostics.push(
                            Diagnostic::error(
                                "closed placement operand has no retained nominal selection",
                            )
                            .with_source_span(reference),
                        );
                        selection_failed = true;
                    }
                    None => {}
                }
            }
            if selection_failed {
                continue;
            }
            let policy_candidates = data
                .iter()
                .filter(|candidate| {
                    selected_arguments
                        .iter()
                        .find(|selected| selected.reference == *policy_handle)
                        .map_or(sources.is_none() && candidate.name == policy, |selected| {
                            selected
                                .declaration
                                .as_ref()
                                .is_ok_and(|declaration| candidate.source == *declaration)
                        })
                })
                .collect::<Vec<_>>();
            let [policy_data] = policy_candidates.as_slice() else {
                diagnostics.push(
                Diagnostic::error(if policy_candidates.is_empty() {
                    format!("`Access<{policy}, {schema}>` names an unknown access policy `{policy}`")
                } else {
                    format!("`Access<{policy}, {schema}>` cannot select one exact access policy `{policy}`")
                })
                .with_source_span(policy_source),
            );
                continue;
            };
            policy = policy_data.name.clone();
            let policy_machines = access_policies
                .iter()
                .filter(|candidate| {
                    selected_owners.iter().any(|(machine, owner)| {
                        *machine == candidate.machine_source
                            && owner
                                .as_ref()
                                .is_ok_and(|owner| *owner == policy_data.source)
                    }) && sources.is_none_or(|sources| {
                        sources.same_checked_instance(policy_data.source, candidate.machine_source)
                    })
                })
                .collect::<Vec<_>>();
            let [policy_machine] = policy_machines.as_slice() else {
                diagnostics.push(
                Diagnostic::error(format!(
                    "`Access<{policy}, {schema}>` must select one exact `{policy}::plan` machine"
                ))
                .with_source_span(policy_source),
            );
                continue;
            };
            if !same_package(sources, policy_data.source, policy_machine.machine_source) {
                diagnostics.push(
                Diagnostic::error(format!(
                    "`Access<{policy}, {schema}>` cannot pair access policy `{policy}` with a `{policy}::plan` machine from another package"
                ))
                .with_source_span(policy_source),
            );
                continue;
            }
            let schema_candidates = data
                .iter()
                .filter(|candidate| {
                    selected_arguments
                        .iter()
                        .find(|selected| selected.reference == *schema_handle)
                        .map_or(sources.is_none() && candidate.name == schema, |selected| {
                            selected
                                .declaration
                                .as_ref()
                                .is_ok_and(|declaration| candidate.source == *declaration)
                        })
                })
                .collect::<Vec<_>>();
            let [schema_data] = schema_candidates.as_slice() else {
                diagnostics.push(
                    Diagnostic::error(if schema_candidates.is_empty() {
                        format!("`Access<{policy}, {schema}>` names an unknown schema `{schema}`")
                    } else {
                        format!(
                            "`Access<{policy}, {schema}>` cannot select one exact schema `{schema}`"
                        )
                    })
                    .with_source_span(schema_use_source),
                );
                continue;
            };
            schema = schema_data.name.clone();
            if !declaration_is_visible(
                sources,
                policy_source,
                policy_data.source,
                policy_data.is_public,
            ) {
                diagnostics.push(
                Diagnostic::error(format!(
                    "`Access<{policy}, {schema}>` cannot use private access policy `{policy}` from another package"
                ))
                .with_source_span(policy_source),
            );
                continue;
            }
            if !declaration_is_visible(
                sources,
                schema_use_source,
                schema_data.source,
                schema_data.is_public,
            ) {
                diagnostics.push(
                Diagnostic::error(format!(
                    "`Access<{policy}, {schema}>` cannot use private schema `{schema}` from another package"
                ))
                .with_source_span(schema_use_source),
            );
                continue;
            }
            if let Err(reason) = require_declaration_selection_authority(
                sources,
                selection_authority,
                policy_source,
                policy_data.source,
                "access policy",
            ) {
                diagnostics.push(Diagnostic::error(reason).with_source_span(policy_source));
                continue;
            }
            if let Err(reason) = require_declaration_selection_authority(
                sources,
                selection_authority,
                schema_use_source,
                schema_data.source,
                "placed schema",
            ) {
                diagnostics.push(Diagnostic::error(reason).with_source_span(schema_use_source));
                continue;
            }
            if !policy_data.is_public {
                diagnostics.push(
                Diagnostic::error(format!(
                    "access policy `{policy}` used by `Access<{policy}, {schema}>` must be public"
                ))
                .with_source_span(policy_source),
            );
                continue;
            }
            if !schema_data.is_public {
                diagnostics.push(
                    Diagnostic::error(format!(
                        "schema `{schema}` used by `Access<{policy}, {schema}>` must be public"
                    ))
                    .with_source_span(schema_use_source),
                );
                continue;
            }
            if !schema_data.plain
                || schema_data.supply_mode == DataSupplyMode::BoundaryOpaque
                || schema_data.fields.is_empty()
                || schema_data.fields.len() != schema_data.member_count
            {
                diagnostics.push(Diagnostic::error(format!(
                    "placed schema `{schema}` must be a nonempty, non-generic transparent record"
                )));
                continue;
            }

            let key = ApplicationKey {
                template: template.unwrap_or_default().into(),
                policy: policy_data.source.into(),
                schema: schema_data.source.into(),
            };
            let synthetic_name = if let Some(application) = applications
                .iter()
                .find(|application: &&Application| application.key == key)
            {
                application.synthetic_name.clone()
            } else {
                let base = format!("Access<{policy},{schema}>");
                if applications
                    .iter()
                    .any(|application| application.synthetic_name == base)
                {
                    format!("{base}#{}", applications.len())
                } else {
                    base
                }
            };
            rewrites.push(PendingRewrite {
                type_reference,
                synthetic_name: synthetic_name.clone(),
            });
            schemas.entry(key.schema).or_insert_with(|| SchemaRecord {
                fields: schema_data.fields.clone(),
            });
            if let Some(application) = applications
                .iter_mut()
                .find(|application: &&mut Application| application.key == key)
            {
                push_unique(&mut application.invocation_sources, policy_source);
            } else {
                applications.push(Application {
                    key,
                    origin: type_reference,
                    synthetic_name,
                    policy,
                    schema,
                    template_source: template.unwrap_or_default(),
                    policy_source: policy_data.source,
                    policy_machine_source: policy_machine.machine_source,
                    schema_source: schema_data.source,
                    generated_is_public: policy_data.is_public && schema_data.is_public,
                    invocation_sources: vec![policy_source],
                });
            }
        }
        if diagnostics.is_empty() {
            Ok((applications, rewrites, schemas))
        } else {
            Err(diagnostics)
        }
    }

    fn same_package(
        sources: Option<&source::SourceMap>,
        left: source::SourceSpan,
        right: source::SourceSpan,
    ) -> bool {
        sources.is_none_or(|sources| sources.same_package(left, right))
    }

    fn declaration_is_visible(
        sources: Option<&source::SourceMap>,
        requester: source::SourceSpan,
        declaration: source::SourceSpan,
        is_public: bool,
    ) -> bool {
        is_public || same_package(sources, requester, declaration)
    }

    fn require_declaration_selection_authority(
        sources: Option<&source::SourceMap>,
        authority: Option<&dyn crate::BuildTimeSelectionAuthority>,
        requester: source::SourceSpan,
        declaration: source::SourceSpan,
        context: &str,
    ) -> Result<(), String> {
        // A missing package-edge table does not merge separately issued contracts.
        if let Some(sources) = sources
            && let (Some(requester_file), Some(declaration_file)) =
                (sources.file_at(requester), sources.file_at(declaration))
            && (requester_file.origin == source::SourceOrigin::ToolchainContract
                || declaration_file.origin == source::SourceOrigin::ToolchainContract)
        {
            return if sources.same_checked_instance(requester, declaration)
                || requester_file.origin == source::SourceOrigin::Toolchain
                || declaration_file.origin == source::SourceOrigin::Toolchain
            {
                Ok(())
            } else {
                Err(format!("{context} crosses closed-contract owner custody"))
            };
        }
        let Some(authority) = authority else {
            return Ok(());
        };
        let Some(sources) = sources else {
            return Err(format!(
                "{context} selection lacks compiler-owned source/package provenance"
            ));
        };
        let Some(requester_file) = sources.file_at(requester) else {
            return Err(format!(
                "{context} selection lacks requesting source/package provenance"
            ));
        };
        let Some(declaration_file) = sources.file_at(declaration) else {
            return Err(format!(
                "{context} selection lacks declaration source/package provenance"
            ));
        };
        if requester_file.origin == source::SourceOrigin::Toolchain
            || declaration_file.origin == source::SourceOrigin::Toolchain
        {
            return Ok(());
        }
        let Some(requester_package) = requester_file.package_identity else {
            return Err(format!(
                "{context} selection has user source without reconciled requesting package custody"
            ));
        };
        let Some(declaration_package) = declaration_file.package_identity else {
            return Err(format!(
                "{context} selection has user declaration without reconciled package custody"
            ));
        };
        if authority.allows_declaration_selection(requester_package, declaration_package) {
            return Ok(());
        }
        Err(format!(
            "{context} selects package {} from package {} without direct dependency authority",
            authority.package_label(declaration_package),
            authority.package_label(requester_package),
        ))
    }

    fn named_argument(
        syntax: &SyntaxTrees,
        handle: TypeReferenceHandle,
    ) -> Option<(String, source::SourceSpan)> {
        match syntax.tables.type_references.type_reference(handle) {
            TypeReferenceNode::Named(name) => {
                Some((name.as_str(syntax).to_owned(), name.source_span()))
            }
            _ => None,
        }
    }
}
pub(crate) mod layout_plans {
    use typed_trees::TypedTrees;

    use crate::build_time_evaluation::type_reference_table;
    use language_semantics::callback_declarations::PrivateCallbackSlotSelection;
    use symbols::SymbolKind;
    use typed_trees::expression::StaticMachineArgument;

    mod plan_validation {
        use layout_plans::{LayoutFieldEntryReport, LayoutPlacementReport, LayoutPlanReport};
        use typed_trees::types::PrimitiveType;

        use crate::layouts::schema_reflection::SchemaFieldInfo;
        use build_time_values::BuildTimeValue;

        const PLAN_ENTRY_CAPACITY: usize = 64;

        pub(crate) fn validate_plan(
            plan: &BuildTimeValue,
            schema_fields: &[SchemaFieldInfo],
            schema_report_fingerprint: u64,
            policy_machine: &str,
        ) -> Result<LayoutPlanReport, String> {
            let fail = |reason: String| {
                format!("policy `{policy_machine}` produced an invalid plan: {reason}")
            };
            let BuildTimeValue::Struct { fields, .. } = plan else {
                return Err(fail(format!("expected a Plan struct, got {plan:?}")));
            };
            let field = |name: &str| -> Result<&BuildTimeValue, String> {
                fields
                    .iter()
                    .find(|(field, _)| field == name)
                    .map(|(_, value)| value)
                    .ok_or_else(|| fail(format!("the plan carries no `{name}` field")))
            };
            let uint_field = |name: &str| -> Result<u64, String> {
                match field(name)? {
                    BuildTimeValue::Int(value) => Ok(*value as u64),
                    other => Err(fail(format!(
                        "plan field `{name}` is not an integer: {other:?}"
                    ))),
                }
            };

            let entry_count = uint_field("entry_count")?;
            if entry_count > PLAN_ENTRY_CAPACITY as u64 {
                return Err(fail(format!(
                    "entry_count {entry_count} is outside 0..={PLAN_ENTRY_CAPACITY}"
                )));
            }
            let entry_count = entry_count as usize;
            let size_is_dynamic = match field("size_is_dynamic")? {
                BuildTimeValue::Bool(value) => *value,
                other => return Err(fail(format!("size_is_dynamic is not a bool: {other:?}"))),
            };
            let size_fixed = uint_field("size_fixed")?;
            let align = uint_field("align")?;
            if align == 0 || !align.is_power_of_two() {
                return Err(fail(format!(
                    "alignment {align} is not a positive power of two"
                )));
            }
            let BuildTimeValue::Array(entry_cells) = field("entries")? else {
                return Err(fail(
                    "plan `entries` is not an array of FieldEntry values".to_owned(),
                ));
            };
            if entry_count > entry_cells.len() {
                return Err(fail(format!(
                    "entry_count is {entry_count}, but the plan carries only {} entries",
                    entry_cells.len()
                )));
            }

            let case_name =
                |variant: &str| variant.rsplit("::").next().unwrap_or(variant).to_owned();
            let lookup = |key: u64| schema_fields.iter().position(|field| field.key == key);
            let field_entry = |entry_index: usize| -> Result<(usize, &BuildTimeValue), String> {
                let Some(BuildTimeValue::Struct { fields, .. }) = entry_cells.get(entry_index)
                else {
                    return Err(fail(format!(
                        "entry {entry_index} is missing or not a FieldEntry"
                    )));
                };
                let key = match fields.iter().find(|(name, _)| name == "key") {
                    Some((_, BuildTimeValue::Int(key))) => *key as u64,
                    other => {
                        return Err(fail(format!(
                            "entry {entry_index} has no integer key: {other:?}"
                        )));
                    }
                };
                let Some(field_index) = lookup(key) else {
                    return Err(fail(format!(
                        "entry {entry_index} refers to unknown field key {key}"
                    )));
                };
                let placement = fields
                    .iter()
                    .find(|(name, _)| name == "representation")
                    .map(|(_, value)| value)
                    .ok_or_else(|| {
                        fail(format!("entry {entry_index} carries no `representation`"))
                    })?;
                Ok((field_index, placement))
            };

            let mut at_counts = vec![0usize; schema_fields.len()];
            for entry_index in 0..entry_count {
                let (field_index, placement) = field_entry(entry_index)?;
                if let BuildTimeValue::Case { variant, .. } = placement
                    && case_name(variant) == "At"
                {
                    at_counts[field_index] += 1;
                }
            }

            let mut entries = Vec::with_capacity(entry_count);
            let mut source_spans: Vec<Vec<(u64, u64)>> = vec![Vec::new(); schema_fields.len()];
            let mut repeated_at_offsets: Vec<Vec<u64>> = vec![Vec::new(); schema_fields.len()];
            let mut destination_spans: Vec<(u64, u64, String)> = Vec::new();
            let mut offsets_by_field = vec![None; schema_fields.len()];
            let mut kinds_by_field: Vec<Option<&'static str>> = vec![None; schema_fields.len()];

            for entry_index in 0..entry_count {
                let (field_index, placement) = field_entry(entry_index)?;
                let schema_field = &schema_fields[field_index];
                match decode_placement(placement).map_err(&fail)? {
                    LayoutPlacementReport::At { offset } => {
                        let tiled = at_counts[field_index] > 1;
                        let placement_size = if tiled {
                            let Some(repeated) = schema_field.repeated else {
                                return Err(fail(format!(
                                    "field `{}` has more than one `At` placement but is not an outer fixed array",
                                    schema_field.name
                                )));
                            };
                            if at_counts[field_index] as u64 != repeated.element_count {
                                return Err(fail(format!(
                                    "repeated field `{}` has {} `At` placements, expected one for each of its {} elements",
                                    schema_field.name,
                                    at_counts[field_index],
                                    repeated.element_count
                                )));
                            }
                            if let Some(kind) = kinds_by_field[field_index]
                                && kind != "RepeatedAt"
                            {
                                return Err(fail(format!(
                                    "field `{}` mixes repeated `At` with another placement",
                                    schema_field.name
                                )));
                            }
                            kinds_by_field[field_index] = Some("RepeatedAt");
                            repeated.element_size
                        } else {
                            if kinds_by_field[field_index].replace("At").is_some() {
                                return Err(fail(format!(
                                    "field `{}` has more than one placement",
                                    schema_field.name
                                )));
                            }
                            schema_field.size
                        };
                        if offset % schema_field.align != 0 {
                            return Err(fail(format!(
                                "field `{}` at offset {offset} violates its alignment {}",
                                schema_field.name, schema_field.align
                            )));
                        }
                        let end = offset.checked_add(placement_size).ok_or_else(|| {
                            fail(format!("field `{}` placement overflows", schema_field.name))
                        })?;
                        let start_bit = offset.checked_mul(8).ok_or_else(|| {
                            fail(format!("field `{}` placement overflows", schema_field.name))
                        })?;
                        let end_bit = end.checked_mul(8).ok_or_else(|| {
                            fail(format!("field `{}` placement overflows", schema_field.name))
                        })?;
                        destination_spans.push((start_bit, end_bit, schema_field.name.clone()));
                        if tiled {
                            repeated_at_offsets[field_index].push(offset);
                        } else {
                            offsets_by_field[field_index] = Some(offset);
                        }
                        entries.push(LayoutFieldEntryReport {
                            field: schema_field.name.clone(),
                            member_identity: schema_field.identity,
                            placement: LayoutPlacementReport::At { offset },
                        });
                    }
                    LayoutPlacementReport::IntegerAt {
                        offset,
                        stored_width,
                        interpretation,
                    } => {
                        if kinds_by_field[field_index].replace("IntegerAt").is_some() {
                            return Err(fail(format!(
                                "field `{}` has more than one placement",
                                schema_field.name
                            )));
                        }
                        if stored_width == 0 || stored_width > 64 || !stored_width.is_multiple_of(8)
                        {
                            return Err(fail(format!(
                                "field `{}` stored integer width {stored_width} is not a supported whole-byte width in 8..=64",
                                schema_field.name
                            )));
                        }
                        let Some(primitive) = schema_field.primitive else {
                            return Err(fail(format!(
                                "field `{}` uses `IntegerAt`, but aggregate fields support only `At` placement",
                                schema_field.name
                            )));
                        };
                        if !primitive.accepts_integer_literal() || primitive == PrimitiveType::Addr
                        {
                            return Err(fail(format!(
                                "field `{}` uses `IntegerAt`, but its semantic type `{}` is not a fixed-width integer carrier",
                                schema_field.name,
                                primitive.name()
                            )));
                        }
                        validate_integer_decode_range(schema_field, stored_width, interpretation)?;
                        let end = offset.checked_add(stored_width / 8).ok_or_else(|| {
                            fail(format!("field `{}` placement overflows", schema_field.name))
                        })?;
                        let start_bit = offset.checked_mul(8).ok_or_else(|| {
                            fail(format!("field `{}` placement overflows", schema_field.name))
                        })?;
                        let end_bit = end.checked_mul(8).ok_or_else(|| {
                            fail(format!("field `{}` placement overflows", schema_field.name))
                        })?;
                        destination_spans.push((start_bit, end_bit, schema_field.name.clone()));
                        entries.push(LayoutFieldEntryReport {
                            field: schema_field.name.clone(),
                            member_identity: schema_field.identity,
                            placement: LayoutPlacementReport::IntegerAt {
                                offset,
                                stored_width,
                                interpretation,
                            },
                        });
                    }
                    LayoutPlacementReport::Bits {
                        container,
                        container_width,
                        destination_lsb,
                        source_lsb,
                        width,
                    } => {
                        if schema_field.primitive.is_none() {
                            return Err(fail(format!(
                                "field `{}` uses `Bits`, but aggregate fields support only `At` placement",
                                schema_field.name
                            )));
                        }
                        if matches!(
                            kinds_by_field[field_index],
                            Some("At" | "RepeatedAt" | "IntegerAt")
                        ) {
                            return Err(fail(format!(
                                "field `{}` mixes a whole-field and `Bits` placement",
                                schema_field.name
                            )));
                        }
                        kinds_by_field[field_index] = Some("Bits");
                        if container_width == 0 || width == 0 {
                            return Err(fail(format!(
                                "field `{}` has a non-positive or negative bit-fragment component",
                                schema_field.name
                            )));
                        }
                        let destination_end = destination_lsb
                            .checked_add(width)
                            .ok_or_else(|| fail("destination bit range overflows".to_owned()))?;
                        let source_end = source_lsb
                            .checked_add(width)
                            .ok_or_else(|| fail("source bit range overflows".to_owned()))?;
                        if destination_end > container_width {
                            return Err(fail(format!(
                                "field `{}` fragment destination {}..{} exceeds its {container_width}-bit container",
                                schema_field.name, destination_lsb, destination_end
                            )));
                        }
                        if source_end > schema_field.source_bits {
                            return Err(fail(format!(
                                "field `{}` fragment source {}..{} exceeds its {}-bit value",
                                schema_field.name, source_lsb, source_end, schema_field.source_bits
                            )));
                        }
                        let absolute_start = container
                            .checked_mul(8)
                            .and_then(|base| base.checked_add(destination_lsb))
                            .ok_or_else(|| fail("destination bit range overflows".to_owned()))?;
                        let absolute_end = absolute_start
                            .checked_add(width)
                            .ok_or_else(|| fail("destination bit range overflows".to_owned()))?;
                        destination_spans.push((
                            absolute_start,
                            absolute_end,
                            schema_field.name.clone(),
                        ));
                        source_spans[field_index].push((source_lsb, source_end));
                        entries.push(LayoutFieldEntryReport {
                            field: schema_field.name.clone(),
                            member_identity: schema_field.identity,
                            placement: LayoutPlacementReport::Bits {
                                container,
                                container_width,
                                destination_lsb,
                                source_lsb,
                                width,
                            },
                        });
                    }
                }
            }

            for (index, schema_field) in schema_fields.iter().enumerate() {
                match kinds_by_field[index] {
                    None => {
                        return Err(fail(format!(
                            "field `{}` has no placement",
                            schema_field.name
                        )));
                    }
                    Some("At") => {}
                    Some("RepeatedAt") => {
                        let repeated = schema_field
                            .repeated
                            .expect("only a reflected outer fixed array enters repeated At");
                        let offsets = &mut repeated_at_offsets[index];
                        offsets.sort_unstable();
                        let stride = offsets[1] - offsets[0];
                        if stride < repeated.element_size
                            || offsets.windows(2).any(|pair| pair[1] - pair[0] != stride)
                        {
                            return Err(fail(format!(
                                "repeated field `{}` element placements do not have one nonoverlapping constant stride",
                                schema_field.name
                            )));
                        }
                    }
                    Some("IntegerAt") => {}
                    Some("Bits") => {
                        let spans = &mut source_spans[index];
                        spans.sort_unstable();
                        let mut cursor = 0;
                        for &(start, end) in spans.iter() {
                            if start != cursor {
                                return Err(fail(format!(
                                    "field `{}` source fragments do not tile exactly: expected next bit {cursor}, found {start}",
                                    schema_field.name
                                )));
                            }
                            cursor = end;
                        }
                        if cursor != schema_field.source_bits {
                            return Err(fail(format!(
                                "field `{}` source fragments end at bit {cursor}, expected {}",
                                schema_field.name, schema_field.source_bits
                            )));
                        }
                    }
                    _ => unreachable!(),
                }
            }

            destination_spans.sort_by_key(|span| span.0);
            for pair in destination_spans.windows(2) {
                let (_, end_a, field_a) = &pair[0];
                let (start_b, _, field_b) = &pair[1];
                if start_b < end_a {
                    return Err(fail(format!(
                        "destination placements for fields `{field_a}` and `{field_b}` overlap"
                    )));
                }
            }
            if !size_is_dynamic {
                if let Some((_, end, field_name)) = destination_spans.last() {
                    let size_bits = size_fixed.checked_mul(8).ok_or_else(|| {
                        fail(format!("fixed size {size_fixed} overflows in bits"))
                    })?;
                    if *end > size_bits {
                        return Err(fail(format!(
                            "field `{field_name}` ends at bit {end}, past the fixed size {size_bits} bits",
                        )));
                    }
                }
                if size_fixed % align != 0 {
                    return Err(fail(format!(
                        "fixed size {size_fixed} is not a multiple of the alignment {align}"
                    )));
                }
            }

            entries.sort_by_key(|entry| {
                let field_index = schema_fields
                    .iter()
                    .position(|field| field.name == entry.field)
                    .unwrap_or(usize::MAX);
                let source_lsb = match &entry.placement {
                    LayoutPlacementReport::At { offset }
                        if schema_fields[field_index].repeated.is_some() =>
                    {
                        *offset
                    }
                    LayoutPlacementReport::At { .. } => 0,
                    LayoutPlacementReport::IntegerAt { .. } => 0,
                    LayoutPlacementReport::Bits { source_lsb, .. } => *source_lsb,
                };
                (
                    schema_fields[field_index]
                        .identity
                        .unwrap_or(field_index as u64),
                    source_lsb,
                )
            });

            let offsets = offsets_by_field
                .iter()
                .all(Option::is_some)
                .then(|| offsets_by_field.into_iter().map(Option::unwrap).collect());

            Ok(LayoutPlanReport {
                schema_report_fingerprint,
                entries,
                offsets,
                size: (!size_is_dynamic).then_some(size_fixed),
                align,
            })
        }

        fn validate_integer_decode_range(
            field: &SchemaFieldInfo,
            stored_width: u64,
            interpretation: layout_plans::IntegerInterpretation,
        ) -> Result<(), String> {
            let primitive = field
                .primitive
                .ok_or_else(|| format!("field `{}` is not a scalar integer", field.name))?;
            let semantic_width = field.size * 8;
            let carrier_fits = match interpretation {
                layout_plans::IntegerInterpretation::Signed => {
                    primitive.is_signed_integer() && stored_width <= semantic_width
                }
                layout_plans::IntegerInterpretation::Unsigned => {
                    if primitive.is_signed_integer() {
                        stored_width < semantic_width
                    } else {
                        stored_width <= semantic_width
                    }
                }
            };
            if !carrier_fits {
                return Err(format!(
                    "field `{}` cannot totally decode a {stored_width}-bit {} integer into `{}`",
                    field.name,
                    match interpretation {
                        layout_plans::IntegerInterpretation::Signed => "signed",
                        layout_plans::IntegerInterpretation::Unsigned => "unsigned",
                    },
                    primitive.name()
                ));
            }

            if let Some((minimum, maximum)) = field.declared_range {
                let (stored_minimum, stored_maximum) = interpretation.stored_bounds(stored_width);
                if stored_minimum < i128::from(minimum) || stored_maximum > i128::from(maximum) {
                    return Err(format!(
                        "field `{}` stored integer range {stored_minimum}..={stored_maximum} does not fit its declared semantic range {minimum}..={maximum}",
                        field.name
                    ));
                }
            }
            Ok(())
        }

        pub(super) fn decode_placement(
            value: &BuildTimeValue,
        ) -> Result<LayoutPlacementReport, String> {
            let BuildTimeValue::Case { variant, payload } = value else {
                return Err("placement is not a FieldPlan case".to_owned());
            };
            let case_name = |name: &str| name.rsplit("::").next().unwrap_or(name).to_owned();
            let uint = |name: &str| -> Result<u64, String> {
                match payload.iter().find(|(field, _)| field == name) {
                    Some((_, BuildTimeValue::Int(value))) => Ok(*value as u64),
                    other => Err(format!(
                        "representation carries no integer `{name}`: {other:?}"
                    )),
                }
            };
            match case_name(variant).as_str() {
                "At" => Ok(LayoutPlacementReport::At {
                    offset: uint("offset")?,
                }),
                "Bits" => Ok(LayoutPlacementReport::Bits {
                    container: uint("container")?,
                    container_width: uint("container_width")?,
                    destination_lsb: uint("destination_lsb")?,
                    source_lsb: uint("source_lsb")?,
                    width: uint("width")?,
                }),
                "IntegerAt" => {
                    let interpretation = match payload
                        .iter()
                        .find(|(field, _)| field == "interpretation")
                    {
                        Some((_, BuildTimeValue::Case { variant, payload }))
                            if payload.is_empty() =>
                        {
                            match case_name(variant).as_str() {
                                "Signed" => layout_plans::IntegerInterpretation::Signed,
                                "Unsigned" => layout_plans::IntegerInterpretation::Unsigned,
                                other => {
                                    return Err(format!(
                                        "unknown integer interpretation `{other}`"
                                    ));
                                }
                            }
                        }
                        other => {
                            return Err(format!(
                                "representation carries no signed/unsigned integer interpretation: {other:?}"
                            ));
                        }
                    };
                    Ok(LayoutPlacementReport::IntegerAt {
                        offset: uint("offset")?,
                        stored_width: uint("stored_width")?,
                        interpretation,
                    })
                }
                other => Err(format!("fixed-layout placement `{other}` is not supported")),
            }
        }
    }
    pub(crate) use plan_validation::validate_plan;

    pub type NativeLayoutPlanReport =
        layout_plans::NativeLayoutPlanReport<typed_trees::PrivateCallbackSlotApplication>;
    pub type PrivateCallbackLayoutDemandReport = layout_plans::PrivateCallbackLayoutDemandReport<
        typed_trees::PrivateCallbackSlotApplication,
    >;

    pub(crate) fn normalize_private_callback_demands(
        typed: &TypedTrees,
        policy_machine: &typed_trees::machine::Machine,
        receipts: &[crate::evaluation_requests::PrivateLayoutPlacementReceipt],
        policy_name: &str,
        selected_implementation: Option<(&StaticMachineArgument, symbols::SymbolHandle)>,
    ) -> Result<Vec<PrivateCallbackLayoutDemandReport>, String> {
        let mut normalized = Vec::with_capacity(receipts.len());
        for receipt in receipts {
            let selected = &receipt.selected_slot;
            if selected.const_literal.is_some()
                || selected.evidence_projection.is_some()
                || !selected.symbol.is_valid()
                || typed.symbols.get(selected.symbol).kind != SymbolKind::Conformance
            {
                return Err(format!(
                    "policy `{policy_name}` selected a private layout slot that is not one exact named conformance"
                ));
            }
            let conformance = typed
            .conformance_by_symbol(selected.symbol)
            .ok_or_else(|| {
                format!(
                    "policy `{policy_name}` selected private layout conformance `{}` whose declaration was not retained",
                    selected.display_name()
                )
            })?;
            let trait_definition = typed
                .trait_definition_by_symbol(conformance.trait_symbol)
                .ok_or_else(|| {
                    format!(
                        "private layout conformance `{}` lost its exact trait declaration",
                        selected.display_name()
                    )
                })?;
            if trait_definition.name.as_str() != "PrivateCallbackSlot"
                || typed.symbols.symbol_source_origin(trait_definition.symbol)
                    != Some(source::SourceOrigin::Toolchain)
            {
                return Err(format!(
                    "private layout conformance `{}` must implement core `PrivateCallbackSlot<Layout, Requirement>`",
                    selected.display_name()
                ));
            }
            let arguments =
                type_reference_table(typed).type_reference_handles(conformance.arguments);
            let [semantic_argument, layout_argument, requirement_argument] = arguments else {
                return Err(format!(
                    "private layout conformance `{}` must retain semantic type, fixed-layout implementation and callback requirement",
                    selected.display_name()
                ));
            };
            let closed_application =
                validation::conformance_applications::close_conformance_application(
                    typed, selected,
                )
                .map_err(|diagnostic| diagnostic.to_string())?;
            let (active, active_owner) = selected_implementation.ok_or_else(||
            "private callback placement requires the exact selected fixed-layout implementation application".to_owned())?;
            let expected = closed_trait_argument_identity(
                typed,
                &closed_application,
                *layout_argument,
                policy_machine.symbol,
            )?;
            let actual = typed_trees::type_identity::portable_applied_index_identity(
                typed,
                typed_trees::type_identity::IndexIdentityArgument::StaticArgument(active),
                active_owner,
                &[],
            )
            .ok_or("active layout policy has no complete portable implementation identity")?;
            if expected != actual {
                return Err(format!(
                    "policy `{policy_name}` selected private slot `{}` for a different exact fixed-layout implementation application",
                    selected.display_name()
                ));
            }
            let layout_subject_identity = typed
                .type_identity_text(actual)
                .ok_or("active layout application lost its portable identity")?;
            let typed_trees::types::TypeReferenceNode::Named {
                symbol: requirement,
                ..
            } = type_reference_table(typed).type_reference(*requirement_argument)
            else {
                return Err(format!(
                    "private layout conformance `{}` callback argument is not one exact requirement declaration",
                    selected.display_name()
                ));
            };
            if !requirement.is_valid() || typed.symbols.get(*requirement).kind != SymbolKind::State
            {
                return Err(format!(
                    "private layout conformance `{}` callback argument is not one exact trait requirement",
                    selected.display_name()
                ));
            }
            let requirement_parent = typed.symbols.get(*requirement).parent;
            if !requirement_parent.is_valid()
                || typed.symbols.get(requirement_parent).kind != SymbolKind::Trait
                || !typed
                    .traits()
                    .iter()
                    .any(|owner| owner.symbol == requirement_parent && owner.is_boundary)
            {
                return Err(format!(
                    "private layout conformance `{}` must name one exact boundary-trait callback requirement",
                    selected.display_name()
                ));
            }
            let requirement_trait = typed
                .trait_definition_by_symbol(requirement_parent)
                .expect("boundary callback owner was found above");
            let requirement_row = typed
            .trait_machine_signature_by_symbol(requirement_trait, *requirement)
            .ok_or_else(|| {
                format!(
                    "private layout conformance `{}` callback requirement was not retained in its declaring trait",
                    selected.display_name()
                )
            })?;

            let slot_identity = format!(
                "{}#{:016x}",
                typed.normalized_hermetic_symbol_identity(selected.symbol)?,
                closed_application.report_fingerprint,
            );
            let semantic_identity = closed_trait_argument_identity(
                typed,
                &closed_application,
                *semantic_argument,
                policy_machine.symbol,
            )?;
            let callback_requirement_identity =
                typed.trait_requirement_callable_identity_text(requirement_trait, requirement_row);
            let slot_application = PrivateCallbackSlotSelection {
                declaration_identity: typed.normalized_hermetic_symbol_identity(selected.symbol)?,
                trait_identity: typed
                    .normalized_hermetic_symbol_identity(trait_definition.symbol)?,
                semantic_identity: typed
                    .type_identity_text(semantic_identity)
                    .ok_or("private layout slot lost its exact substituted semantic type")?,
                layout_identity: layout_subject_identity.clone(),
                requirement_identity: callback_requirement_identity.clone(),
                application_report_fingerprint: closed_application.report_fingerprint,
                application_commitment: closed_application.commitment.as_bytes(),
                application_encoding: closed_application.identity_encoding.clone(),
            };
            let placement = match &receipt.placement {
                build_time_values::BuildTimeValue::Int(offset) => {
                    layout_plans::LayoutPlacementReport::At {
                        offset: u64::try_from(*offset)
                            .map_err(|_| "private callback offset must be a nonnegative u64")?,
                    }
                }
                value => plan_validation::decode_placement(value)?,
            };
            if let Some(existing) =
                normalized
                    .iter_mut()
                    .find(|existing: &&mut PrivateCallbackLayoutDemandReport| {
                        existing.slot_identity == slot_identity
                    })
            {
                if existing.slot_application.selection != slot_application {
                    return Err(format!(
                        "policy `{policy_name}` changes the exact application of a repeated private slot"
                    ));
                }
                existing.placements.push(placement);
                continue;
            }
            normalized.push(PrivateCallbackLayoutDemandReport {
                slot_application: typed_trees::PrivateCallbackSlotApplication {
                    application: closed_application,
                    selection: slot_application,
                },
                slot_identity,
                layout_subject_identity,
                callback_requirement_identity,
                placements: vec![placement],
            });
        }
        for demand in &mut normalized {
            demand
                .placements
                .sort_unstable_by_key(layout_plans::layout_placement_sort_key);
            if demand.placements.len() > 1
                && demand.placements.iter().any(|placement| {
                    !matches!(placement, layout_plans::LayoutPlacementReport::Bits { .. })
                })
            {
                return Err(format!(
                    "policy `{policy_name}` mixes or repeats whole-slot private callback placements"
                ));
            }
        }
        normalized.sort_unstable_by(|left, right| left.slot_identity.cmp(&right.slot_identity));
        Ok(normalized)
    }

    fn closed_trait_argument_identity(
        typed: &TypedTrees,
        application: &typed_trees::typed_trees::ClosedConformanceApplication,
        argument: typed_trees::types::TypeReferenceHandle,
        owner: symbols::SymbolHandle,
    ) -> Result<typed_trees::type_identity::TypeIdentityId, String> {
        use typed_trees::type_identity::{AppliedTypeIdentityScope, IndexIdentityArgument};
        let declaration = typed
            .conformance_by_symbol(application.declaration)
            .ok_or("private layout slot lost its exact map declaration")?;
        let substitutions = typed
            .conformance_type_parameters(declaration)
            .iter()
            .zip(&application.arguments)
            .map(|(parameter, argument)| {
                (
                    parameter.symbol,
                    IndexIdentityArgument::StaticArgument(argument),
                )
            })
            .collect::<Vec<_>>();
        let lifetimes = declaration
            .lifetime_parameters
            .iter()
            .zip(&application.lifetime_arguments)
            .map(|(parameter, actual)| (parameter.as_str().to_owned(), actual.clone()))
            .collect::<Vec<_>>();
        typed_trees::type_identity::portable_applied_index_identity(
            typed,
            IndexIdentityArgument::TypeArgument(argument),
            owner,
            &[AppliedTypeIdentityScope {
                declaration: declaration.symbol,
                arguments: &substitutions,
                lifetimes: &lifetimes,
            }],
        )
        .ok_or("private layout slot has no complete implementation argument identity".into())
    }
}
pub(crate) mod plan_laid {
    use crate::product::provider_declarations::{
        retain_provider_defaults, source_checking_applications,
    };
    use source::DependencyScope;
    use typed_trees::references::collect_selected_program_type_occurrences;

    mod layout_installation {
        use crate::build_time_evaluation::type_reference_table;
        use crate::evaluation_requests::BuildMachineEvaluationRequest;
        use diagnostics::Diagnostic;
        use layout_plans::LayoutPlacementReport;
        use typed_trees::references::collect_selected_program_type_occurrences;
        use typed_trees::{
            PlanLaidBitField, PlanLaidBitFragment, PlanLaidGeometry, PlanLaidIntegerField,
            PlanLaidLayout, PlanLaidRealizations, PlanLaidRepeatedField, PlanLaidSchemaArgument,
            PlanLaidSchemaField, PlanLaidSchemaFieldKind, PlanLaidTargetLayout, TypedTrees,
        };
        use validation::conformance_applications::validate_data_evidence_application;

        struct FixedRepresentationRequest {
            application_name: String,
            policy_machine: String,
        }

        struct ScopedRepresentationRequest {
            storage: typed_trees::layout_storage::LayoutStorageArguments,
            targets: Vec<layout_plans::LayoutTargetFacts>,
            dependencies: Vec<RepresentationDependency>,
            invocation_owners: Vec<symbols::SymbolHandle>,
        }

        #[derive(Clone, Copy)]
        struct RepresentationDependency {
            storage: typed_trees::layout_storage::LayoutStorageArguments,
            lexical_owner: symbols::SymbolHandle,
        }

        fn applicable_targets(
            typed: &TypedTrees,
            domain: &crate::build_time_evaluation::EvaluationDomain,
            storage: typed_trees::layout_storage::LayoutStorageArguments,
        ) -> Vec<layout_plans::LayoutTargetFacts> {
            let Some(inputs) = &domain.layout_applications else {
                return domain.layout_targets.clone();
            };
            let identity = typed.normalized_type_identity(storage.application);
            let mut targets = Vec::new();
            for input in inputs.iter().filter(|input| {
                typed.type_identity_by_text(&input.application_identity) == identity
            }) {
                if !targets.contains(&input.target) {
                    targets.push(input.target.clone());
                }
            }
            targets
        }

        fn invocation_owners(
            typed: &TypedTrees,
            domain: &crate::build_time_evaluation::EvaluationDomain,
            storage: typed_trees::layout_storage::LayoutStorageArguments,
            owner: symbols::SymbolHandle,
        ) -> Vec<symbols::SymbolHandle> {
            if !typed_trees::domain::index_argument_is_closed(typed, storage.policy) {
                return vec![owner];
            }
            let Some(inputs) = &domain.layout_applications else {
                return vec![owner];
            };
            let identity = typed.normalized_type_identity(storage.application);
            let mut owners = Vec::new();
            for input in inputs.iter().filter(|input| {
                typed.type_identity_by_text(&input.application_identity) == identity
            }) {
                if !owners.contains(&input.lexical_owner) {
                    owners.push(input.lexical_owner);
                }
            }
            owners
        }

        pub(crate) fn compute_plan_laid_layouts_in(
            typed: &mut TypedTrees,
            selection_authority: Option<std::sync::Arc<dyn crate::BuildTimeSelectionAuthority>>,
            domain: &crate::build_time_evaluation::EvaluationDomain,
        ) -> Result<(), Vec<Diagnostic>> {
            let requests = scoped_representation_requests(typed, domain)?;
            if requests.is_empty() {
                return Ok(());
            }
            typed_trees_to_typed_trees::prepare_execution_applications(typed)?;
            install_requested_layouts(typed, selection_authority, domain, requests)
        }

        pub(super) fn compute_prepared_plan_laid_layouts_in(
            typed: &mut TypedTrees,
            selection_authority: Option<std::sync::Arc<dyn crate::BuildTimeSelectionAuthority>>,
            domain: &crate::build_time_evaluation::EvaluationDomain,
        ) -> Result<(), Vec<Diagnostic>> {
            let requests = scoped_representation_requests(typed, domain)?;
            if requests.is_empty() {
                return Ok(());
            }
            install_requested_layouts(typed, selection_authority, domain, requests)
        }

        fn scoped_representation_requests(
            typed: &TypedTrees,
            domain: &crate::build_time_evaluation::EvaluationDomain,
        ) -> Result<Vec<ScopedRepresentationRequest>, Vec<Diagnostic>> {
            let fail = |reason: String| vec![Diagnostic::error(reason)];
            let mut requests = Vec::new();
            for occurrence in collect_selected_program_type_occurrences(typed, domain.work()) {
                let Some(storage) = typed.layout_storage_arguments(occurrence.reference) else {
                    continue;
                };
                validate_data_evidence_application(
                    typed,
                    storage.application,
                    occurrence.lexical_owner,
                )?;
                if let Some(known) =
                    requests
                        .iter_mut()
                        .find(|known: &&mut ScopedRepresentationRequest| {
                            typed.normalized_type_identity(known.storage.application)
                                == typed.normalized_type_identity(storage.application)
                        })
                {
                    for owner in invocation_owners(typed, domain, storage, occurrence.lexical_owner)
                    {
                        if !known.invocation_owners.contains(&owner) {
                            known.invocation_owners.push(owner);
                        }
                    }
                    continue;
                }
                if typed.data_field_definition(storage.semantic_type).is_some() {
                    let policy_is_closed =
                        typed_trees::domain::index_argument_is_closed(typed, storage.policy);
                    let policy_declaration = typed.type_reference_table.type_symbol(storage.policy);
                    if !policy_is_closed
                        && policy_declaration.is_valid()
                        && typed.symbols.get(policy_declaration).kind
                            == symbols::SymbolKind::ConformanceParameter
                    {
                        continue;
                    }
                    let targets = applicable_targets(typed, domain, storage);
                    if !policy_is_closed && targets.is_empty() {
                        continue;
                    }
                    requests.push(ScopedRepresentationRequest {
                        storage,
                        targets,
                        dependencies: representation_dependencies(typed, storage.semantic_type)?,
                        invocation_owners: invocation_owners(
                            typed,
                            domain,
                            storage,
                            occurrence.lexical_owner,
                        ),
                    });
                }
            }
            let mut pending = (0..requests.len()).collect::<Vec<_>>();
            while let Some(index) = pending.pop() {
                let inherited = requests[index].targets.clone();
                for dependency in requests[index].dependencies.clone() {
                    if let Some(position) = requests.iter().position(|known| {
                        typed.normalized_type_identity(known.storage.application)
                            == typed.normalized_type_identity(dependency.storage.application)
                    }) {
                        let known = &mut requests[position];
                        let before = known.targets.len();
                        if !inherited.is_empty()
                            && !known.invocation_owners.contains(&dependency.lexical_owner)
                        {
                            known.invocation_owners.push(dependency.lexical_owner);
                        }
                        for target in &inherited {
                            if !known.targets.contains(target) {
                                known.targets.push(target.clone());
                            }
                        }
                        if known.targets.len() != before && !pending.contains(&position) {
                            pending.push(position);
                        }
                        continue;
                    }
                    validate_data_evidence_application(
                        typed,
                        dependency.storage.application,
                        dependency.lexical_owner,
                    )?;
                    pending.push(requests.len());
                    requests.push(ScopedRepresentationRequest {
                        storage: dependency.storage,
                        targets: inherited.clone(),
                        dependencies: representation_dependencies(
                            typed,
                            dependency.storage.semantic_type,
                        )?,
                        invocation_owners: if inherited.is_empty() {
                            Vec::new()
                        } else {
                            vec![dependency.lexical_owner]
                        },
                    });
                }
            }
            let mut ordered = Vec::with_capacity(requests.len());
            while !requests.is_empty() {
                let mut ready = None;
                for (position, request) in requests.iter().enumerate() {
                    if request.dependencies.iter().all(|dependency| {
                        !requests.iter().any(|pending| {
                            typed.normalized_type_identity(pending.storage.application)
                                == typed.normalized_type_identity(dependency.storage.application)
                        })
                    }) {
                        ready = Some(position);
                        break;
                    }
                }
                let Some(position) = ready else {
                    return Err(fail(
                        "fixed representation applications have a cyclic storage dependency".into(),
                    ));
                };
                ordered.push(requests.remove(position));
            }
            let requests = ordered;
            if requests.is_empty() {
                return Ok(requests);
            }
            if domain.layout_targets.is_empty() && domain.layout_applications.is_none() {
                return Ok(Vec::new());
            }
            for (position, target) in domain.layout_targets.iter().enumerate() {
                target.validate().map_err(|reason| fail(reason.into()))?;
                if domain.layout_targets[..position]
                    .iter()
                    .any(|known| known.profile == target.profile)
                {
                    return Err(fail(
                        "fixed representation evaluation repeats a target input".into(),
                    ));
                }
            }
            Ok(requests)
        }

        fn install_requested_layouts(
            typed: &mut TypedTrees,
            selection_authority: Option<std::sync::Arc<dyn crate::BuildTimeSelectionAuthority>>,
            domain: &crate::build_time_evaluation::EvaluationDomain,
            requests: Vec<ScopedRepresentationRequest>,
        ) -> Result<(), Vec<Diagnostic>> {
            let fail = |reason: String| vec![Diagnostic::error(reason)];
            let mut replaced = requests
                .iter()
                .map(|request| typed.normalized_type_identity(request.storage.application))
                .collect::<Vec<_>>();
            replaced.sort_unstable();
            replaced.dedup();
            let existing = std::mem::take(&mut typed.plan_laid_layouts);
            typed.plan_laid_layouts = existing
                .into_iter()
                .filter(|plan| {
                    replaced
                        .binary_search(&typed.normalized_type_identity(plan.application))
                        .is_err()
                })
                .collect();
            let mut execution = domain.execution_program(typed);
            let admission =
                crate::BuildTimeAdmissionPlan::infer(&execution, selection_authority.clone())?;
            'application: for request in requests {
                let storage = request.storage;
                let policy_is_closed =
                    typed_trees::domain::index_argument_is_closed(typed, storage.policy);
                let mut targets = request.targets;
                let invocation_owners = request.invocation_owners;
                targets.sort_by(|left, right| left.profile.cmp(&right.profile));
                let semantic = typed
            .data_field_definition(storage.semantic_type)
            .ok_or_else(|| {
                fail(format!(
                    "fixed representation lost its exact declared semantic application `{}`",
                    typed.display_type_reference(storage.semantic_type)
                ))
            })?
            .clone();
                if semantic.supply_mode != language_semantics::DataSupplyMode::CheckedShape {
                    return Err(fail(
                        "fixed representation cannot reflect opaque semantic storage".into(),
                    ));
                }
                if typed
                    .data_members(&semantic)
                    .iter()
                    .any(|member| member.as_field().is_none())
                {
                    return Err(fail("fixed representation requires a record: programmable case/tag placement is not supported by this placement vocabulary".into()));
                }
                let selected = execution
                    .static_operand_from_type_reference(storage.policy)
                    .map_err(|reason| fail(reason.into()))?;
                let application =
                    validation::conformance_applications::close_conformance_application(
                        &execution, &selected,
                    )
                    .map_err(|error| vec![error])?;
                if application.trait_definition != storage.contract {
                    return Err(fail(
                "fixed representation policy does not satisfy the exact core FixedLayout contract"
                    .into(),
            ));
                }
                let [row] = application.rows.as_slice() else {
                    return Err(fail(
                        "fixed representation policy must retain its one exact plan row".into(),
                    ));
                };
                let realized = execution
                    .captured_machine_applications
                    .iter()
                    .find_map(|(_, captured)| {
                        (captured.template == row.realization_machine
                            && captured.conformance_application == application)
                            .then_some(captured.instance)
                    })
                    .unwrap_or(row.realization_machine);
                let machine = execution.machine_by_symbol(realized).ok_or_else(|| {
                    fail("fixed representation lost its checked policy body".into())
                })?;
                if !targets.is_empty() && invocation_owners.is_empty() {
                    return Err(fail(
                "fixed representation invocation lost its exact demanding occurrence custody"
                    .into(),
            ));
                }
                for owner in invocation_owners.iter().filter(|_| !targets.is_empty()) {
                    if !owner.is_valid()
                        || execution
                            .symbols
                            .symbol_provenance_source_span(*owner)
                            .is_none()
                    {
                        return Err(fail(
                            "fixed representation invocation has an invalid source owner".into(),
                        ));
                    }
                    admission
                        .require_common_floor_for_invocation(
                            &execution,
                            machine,
                            crate::BuildTimeInvocationCustody::Symbol(*owner),
                        )
                        .map_err(|reason| fail(reason.to_string()))?;
                }
                let schema_fields = typed
                    .live_data_fields(typed.data_members(&semantic))
                    .map(|field| (field.name.as_str().to_owned(), field.type_reference))
                    .collect::<Vec<_>>();
                let mut field_symbols = None;
                let record = FixedRepresentationRequest {
                    application_name: typed.display_type_reference(storage.application),
                    policy_machine: machine.name.as_str().to_owned(),
                };
                let mut alternatives = Vec::new();
                for target in &targets {
                    let (fields, fingerprint) =
                        super::super::schema_reflection::schema_fields_for_target(
                            typed,
                            &semantic,
                            Some(target),
                        )
                        .map_err(fail)?;
                    let symbols = fields
                        .iter()
                        .map(|field| field.schema_field)
                        .collect::<Vec<_>>();
                    if field_symbols
                        .as_ref()
                        .is_some_and(|known| *known != symbols)
                    {
                        return Err(fail(
                            "target schema changed its exact semantic field roster".into(),
                        ));
                    }
                    field_symbols = Some(symbols);
                    let schema_value =
                        super::super::schema_reflection::build_selected_schema_value(
                            typed, &semantic, &fields,
                        )
                        .map_err(fail)?;
                    let retained = super::super::schema_reflection::canonical_schema_fields(&fields).into_iter().map(|field| Ok(PlanLaidSchemaField {
            schema_field: field.schema_field, key: field.key, identity: field.identity,
            byte_size: field.size, byte_alignment: field.align,
            layout_source: field.layout_source,
            kind: match field.kind { "Scalar" => PlanLaidSchemaFieldKind::Scalar,
                "Nested" => PlanLaidSchemaFieldKind::Nested,
                "Repeated" => PlanLaidSchemaFieldKind::Repeated,
                _ => return Err(fail("fixed representation retained Schema ABI does not support this field kind".into())), },
        })).collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
                    let mut retired_identities = semantic.retired_identities.clone();
                    retired_identities.sort_unstable();
                    let schema_argument = Some(PlanLaidSchemaArgument {
                        abi_version: 4,
                        fields: typed.plan_laid_schema_fields.insert_many(retained),
                        retired_field_identities: typed
                            .plan_laid_retired_field_identities
                            .insert_many(retired_identities.iter().map(|identity| {
                                typed_trees::PlanLaidRetiredFieldIdentity {
                                    identity: *identity,
                                }
                            })),
                    });
                    let mut arguments = vec![
                        schema_value.clone(),
                        build_time_values::BuildTimeValue::LayoutTarget(target.clone()),
                    ];
                    admission
                        .require_invocation_failures(
                            crate::BuildTimeProgram::Typed(&execution),
                            machine,
                            &mut arguments,
                            None,
                        )
                        .map_err(fail)?;
                    let evaluation = match domain.accounting.evaluate(
                crate::evaluation_requests::BuildMachineProgram::Typed(&execution),
                BuildMachineEvaluationRequest::symbol(machine.symbol, arguments),
                &format!("layout policy `{}` under `{}`", machine.name, target.profile),
            ) {
                Ok(evaluation) => evaluation,
                Err(crate::evaluation_requests::BuildMachineEvaluationError::StaticInputUnavailable(
                    _,
                )) if !policy_is_closed => {
                    continue 'application;
                }
                Err(reason) => return Err(fail(reason.to_string())),
            };
                    let layout = super::super::layout_plans::validate_plan(
                        evaluation.value(),
                        &fields,
                        fingerprint,
                        machine.name.as_str(),
                    )
                    .map_err(fail)?;
                    let private_callback_demands =
                        super::super::layout_plans::normalize_private_callback_demands(
                            &execution,
                            machine,
                            evaluation.private_layout_placements(),
                            machine.name.as_str(),
                            Some((
                                &selected,
                                execution
                                    .data_implementation_applications
                                    .iter()
                                    .find_map(|(_, demand)| {
                                        (execution
                                            .normalized_type_identity(demand.source_application)
                                            == execution
                                                .normalized_type_identity(storage.application))
                                        .then_some(demand.lexical_owner)
                                    })
                                    .unwrap_or_else(|| {
                                        type_reference_table(&execution)
                                            .operand_lifetime_owner(storage.policy)
                                    }),
                            )),
                        )
                        .map_err(fail)?;
                    let geometry = install_geometry(
                        typed,
                        &record,
                        &schema_fields,
                        crate::layouts::layout_plans::NativeLayoutPlanReport {
                            layout,
                            private_callback_demands,
                        },
                    )?;
                    alternatives.push(PlanLaidTargetLayout {
                        schema_argument,
                        target: target.profile.as_str().into(),
                        machine: machine.symbol,
                        target_facts: target.clone(),
                        geometry,
                    });
                }
                let field_symbols = field_symbols.unwrap_or_else(|| {
                    typed
                        .live_data_fields(typed.data_members(&semantic))
                        .map(|field| field.symbol)
                        .collect()
                });
                let layout = PlanLaidLayout {
                    application: storage.application,
                    semantic_type: storage.semantic_type,
                    policy_application: application.clone(),
                    target_inputs: targets,
                    data_name: record.application_name,
                    data_symbol: semantic.symbol,
                    field_symbols: field_symbols.clone(),
                    schema_symbol: semantic.symbol,
                    schema_field_symbols: field_symbols,
                    policy_symbol: application.declaration,
                    policy_plan_machine_symbol: machine.symbol,
                    schema_argument: None,
                    realizations: PlanLaidRealizations::TargetInputs(
                        typed.plan_laid_target_layouts.insert_many(alternatives),
                    ),
                };
                typed.plan_laid_layouts.push(layout);
                execution.plan_laid_layouts = typed.plan_laid_layouts.clone();
                execution.plan_laid_target_layouts = typed.plan_laid_target_layouts.clone();
                execution.plan_laid_schema_fields = typed.plan_laid_schema_fields.clone();
                execution.plan_laid_retired_field_identities =
                    typed.plan_laid_retired_field_identities.clone();
            }
            Ok(())
        }

        pub(crate) fn validate_layout_application_coverage(
            typed: &TypedTrees,
            inputs: Option<&[crate::LayoutApplicationTargetInput]>,
        ) -> Result<(), Vec<Diagnostic>> {
            if let Some(inputs) = inputs {
                let open = typed_trees::references::collect_program_type_occurrences(typed)
                    .into_iter()
                    .filter_map(|occurrence| typed.layout_storage_arguments(occurrence.reference))
                    .filter(|storage| {
                        !typed_trees::domain::index_argument_is_closed(typed, storage.policy)
                    })
                    .map(|storage| typed.normalized_type_identity(storage.application))
                    .collect::<Vec<_>>();
                for input in inputs {
                    let identity = typed.type_identity_by_text(&input.application_identity);
                    if identity.is_valid() && open.contains(&identity) {
                        continue;
                    }
                    if !identity.is_valid()
                        || !typed.plan_laid_layouts.iter().any(|plan| {
                            typed.normalized_type_identity(plan.application) == identity
                                && plan.target_inputs.contains(&input.target)
                        })
                    {
                        return Err(vec![Diagnostic::error(format!(
                            "layout application `{}` target demand for `{}` did not rejoin its exact evaluated representation",
                            input.application_identity, input.target.profile
                        ))]);
                    }
                }
            }
            Ok(())
        }

        fn representation_dependencies(
            typed: &TypedTrees,
            semantic_type: typed_trees::types::TypeReferenceHandle,
        ) -> Result<Vec<RepresentationDependency>, Vec<Diagnostic>> {
            use typed_trees::types::TypeReferenceNode;
            let owner = typed
                .data_field_definition(semantic_type)
                .map(|data| data.symbol)
                .unwrap_or_default();
            let mut work = vec![(semantic_type, owner)];
            let mut seen = Vec::new();
            let mut dependencies = Vec::new();
            while let Some((reference, lexical_owner)) = work.pop() {
                let identity = typed.normalized_type_identity(reference);
                let Some(index) = identity.storage_index() else {
                    continue;
                };
                if seen.len() <= index {
                    seen.resize(index + 1, false);
                }
                if std::mem::replace(&mut seen[index], true) {
                    continue;
                }
                if let Some(storage) = typed.layout_storage_arguments(reference) {
                    dependencies.push(RepresentationDependency {
                        storage,
                        lexical_owner,
                    });
                    continue;
                }
                let reference = type_reference_table(typed).unconstrained_type_reference(reference);
                match typed.type_reference_table.type_reference(reference) {
                    TypeReferenceNode::FixedArray { element_type, .. } => {
                        work.push((*element_type, lexical_owner))
                    }
                    TypeReferenceNode::Named { .. } | TypeReferenceNode::Generic { .. } => {
                        if let Some(data) = typed.data_field_definition(reference) {
                            work.extend(
                                typed
                                    .live_data_fields(typed.data_members(data))
                                    .map(|field| (field.type_reference, data.symbol)),
                            );
                        }
                    }
                    _ => {}
                }
            }
            Ok(dependencies)
        }

        fn install_geometry(
            typed: &TypedTrees,
            record: &FixedRepresentationRequest,
            schema_fields: &[(String, typed_trees::types::TypeReferenceHandle)],
            native_report: crate::layouts::layout_plans::NativeLayoutPlanReport,
        ) -> Result<PlanLaidGeometry, Vec<Diagnostic>> {
            let report = &native_report.layout;
            let size = report.size.ok_or_else(|| vec![Diagnostic::error(format!(
        "policy `{}` produced a dynamic plan: a dynamic plan cannot be a value type -- values need offsets, bytes need mints",
        record.policy_machine
    ))])?;
            let field_count = schema_fields.len();

            let mut offsets = vec![None; field_count];
            let mut bit_fields = Vec::<PlanLaidBitField>::new();
            let mut integer_fields = Vec::<PlanLaidIntegerField>::new();
            let mut repeated_fields = Vec::<PlanLaidRepeatedField>::new();
            for (field_index, (field_name, field_type)) in schema_fields.iter().enumerate() {
                let field_entries = report
                    .entries
                    .iter()
                    .filter(|entry| entry.field == *field_name)
                    .collect::<Vec<_>>();
                match field_entries.as_slice() {
                    [entry] if matches!(entry.placement, LayoutPlacementReport::At { .. }) => {
                        let LayoutPlacementReport::At { offset } = entry.placement else {
                            unreachable!()
                        };
                        offsets[field_index] = Some(usize::try_from(offset).map_err(|_| {
                    vec![Diagnostic::error(format!(
                        "plan-laid value type `{}`: byte offset {offset} cannot be represented on this compiler host",
                        record.application_name
                    ))]
                })?);
                    }
                    [entry]
                        if matches!(entry.placement, LayoutPlacementReport::IntegerAt { .. }) =>
                    {
                        let LayoutPlacementReport::IntegerAt {
                            offset,
                            stored_width,
                            interpretation,
                        } = entry.placement
                        else {
                            unreachable!()
                        };
                        offsets[field_index] = Some(usize::try_from(offset).map_err(|_| {
                    vec![Diagnostic::error(format!(
                        "plan-laid value type `{}`: stored-integer byte offset {offset} cannot be represented on this compiler host",
                        record.application_name
                    ))]
                })?);
                        integer_fields.push(PlanLaidIntegerField {
                    field_index,
                    stored_width_bits: u16::try_from(stored_width).map_err(|_| {
                        vec![Diagnostic::error(format!(
                            "plan-laid value type `{}`: stored-integer width {stored_width} exceeds the backend width vocabulary",
                            record.application_name
                        ))]
                    })?,
                    interpretation,
                    write_is_total: stored_integer_write_is_total(
                        typed,
                        *field_type,
                        stored_width,
                        interpretation,
                    ),
                });
                    }
                    entries
                        if entries.len() > 1
                            && entries.iter().all(|entry| {
                                matches!(entry.placement, LayoutPlacementReport::At { .. })
                            }) =>
                    {
                        let typed_trees::types::TypeReferenceNode::FixedArray {
                            length: typed_trees::types::FixedArrayLength::Literal(element_count),
                            ..
                        } = typed.type_reference_table.type_reference(*field_type)
                        else {
                            return Err(vec![Diagnostic::error(format!(
                                "plan-laid value type `{}`: field `{field_name}` has repeated byte placements but is not a literal outer fixed array",
                                record.application_name
                            ))]);
                        };
                        if entries.len() != *element_count {
                            return Err(vec![Diagnostic::error(format!(
                                "plan-laid value type `{}`: field `{field_name}` has {} element placements but its outer fixed array has {element_count} elements",
                                record.application_name,
                                entries.len()
                            ))]);
                        }
                        let mut element_offsets = entries
                            .iter()
                            .map(|entry| match entry.placement {
                                LayoutPlacementReport::At { offset } => offset,
                                _ => unreachable!("repeated placements were filtered to At"),
                            })
                            .collect::<Vec<_>>();
                        element_offsets.sort_unstable();
                        let stride = element_offsets[1] - element_offsets[0];
                        if stride == 0
                            || element_offsets
                                .windows(2)
                                .any(|pair| pair[1] - pair[0] != stride)
                        {
                            return Err(vec![Diagnostic::error(format!(
                                "plan-laid value type `{}`: field `{field_name}` does not retain one positive constant destination stride",
                                record.application_name
                            ))]);
                        }
                        offsets[field_index] = Some(
                    usize::try_from(element_offsets[0]).map_err(|_| {
                        vec![Diagnostic::error(format!(
                            "plan-laid value type `{}`: repeated field offset {} cannot be represented on this compiler host",
                            record.application_name, element_offsets[0]
                        ))]
                    })?,
                );
                        repeated_fields.push(PlanLaidRepeatedField {
                    field_index,
                    element_stride: usize::try_from(stride).map_err(|_| {
                        vec![Diagnostic::error(format!(
                            "plan-laid value type `{}`: repeated field stride {stride} cannot be represented on this compiler host",
                            record.application_name
                        ))]
                    })?,
                });
                    }
                    entries
                        if entries.iter().all(|entry| {
                            matches!(entry.placement, LayoutPlacementReport::Bits { .. })
                        }) =>
                    {
                        let mut fragments = Vec::with_capacity(entries.len());
                        for entry in entries {
                            let LayoutPlacementReport::Bits {
                                container,
                                container_width,
                                destination_lsb,
                                source_lsb,
                                width,
                            } = entry.placement
                            else {
                                unreachable!()
                            };
                            let fragment = PlanLaidBitFragment {
                        container_byte_offset: usize::try_from(container).map_err(|_| {
                            vec![Diagnostic::error(format!(
                                "plan-laid value type `{}`: bit-container offset {container} cannot be represented on this compiler host",
                                record.application_name
                            ))]
                        })?,
                        container_width_bits: u16::try_from(container_width).map_err(|_| {
                            vec![Diagnostic::error(format!(
                                "plan-laid value type `{}`: bit-container width {container_width} exceeds the backend width vocabulary",
                                record.application_name
                            ))]
                        })?,
                        destination_lsb: u16::try_from(destination_lsb).map_err(|_| {
                            vec![Diagnostic::error(format!(
                                "plan-laid value type `{}`: destination bit {destination_lsb} exceeds the backend width vocabulary",
                                record.application_name
                            ))]
                        })?,
                        source_lsb: u16::try_from(source_lsb).map_err(|_| {
                            vec![Diagnostic::error(format!(
                                "plan-laid value type `{}`: source bit {source_lsb} exceeds the backend width vocabulary",
                                record.application_name
                            ))]
                        })?,
                        width: u16::try_from(width).map_err(|_| {
                            vec![Diagnostic::error(format!(
                                "plan-laid value type `{}`: fragment width {width} exceeds the backend width vocabulary",
                                record.application_name
                            ))]
                        })?,
                    };
                            offsets[field_index].get_or_insert(fragment.container_byte_offset);
                            fragments.push(fragment);
                        }
                        bit_fields.push(PlanLaidBitField {
                            field_index,
                            fragments,
                        });
                    }
                    _ => {
                        return Err(vec![Diagnostic::error(format!(
                            "plan-laid value type `{}`: field `{field_name}` does not have one \
                     byte placement or a completely tiled bit-fragment placement",
                            record.application_name
                        ))]);
                    }
                }
            }

            let offsets = offsets
                .iter()
                .copied()
                .collect::<Option<Vec<_>>>()
                .expect("validated plan supplies every field");
            let size = usize::try_from(size).map_err(|_| {
        vec![Diagnostic::error(format!(
            "plan-laid value type `{}`: fixed size {size} cannot be represented on this compiler host",
            record.application_name
        ))]
    })?;
            let align = usize::try_from(report.align).map_err(|_| {
        vec![Diagnostic::error(format!(
            "plan-laid value type `{}`: alignment {} cannot be represented on this compiler host",
            record.application_name, report.align
        ))]
    })?;
            Ok(PlanLaidGeometry {
                validated_layout: report.clone(),
                private_callback_demands: native_report.private_callback_demands,
                offsets,
                bit_fields,
                integer_fields,
                repeated_fields,
                size,
                align,
            })
        }

        fn stored_integer_write_is_total(
            typed: &TypedTrees,
            field_type: typed_trees::types::TypeReferenceHandle,
            stored_width: u64,
            interpretation: layout_plans::IntegerInterpretation,
        ) -> bool {
            let Some(primitive) = typed.primitive_type_reference(field_type) else {
                return false;
            };
            let (admitted_minimum, admitted_maximum) =
                if let Some(range) = validation::scalar_representation_range(typed, field_type) {
                    (i128::from(range.minimum), i128::from(range.maximum))
                } else {
                    let width = primitive.scalar_byte_size().unwrap_or(0) * 8;
                    if width == 0 {
                        return false;
                    }
                    if primitive.is_signed_integer() {
                        (-(1i128 << (width - 1)), (1i128 << (width - 1)) - 1)
                    } else {
                        (0, (1i128 << width) - 1)
                    }
                };
            let (stored_minimum, stored_maximum) = interpretation.stored_bounds(stored_width);
            admitted_minimum >= stored_minimum && admitted_maximum <= stored_maximum
        }
    }
    pub(crate) use layout_installation::compute_plan_laid_layouts_in;
    use layout_installation::compute_prepared_plan_laid_layouts_in;

    use diagnostics::Diagnostic;
    use typed_trees::TypedTrees;

    pub fn has_fixed_layout_applications(typed: &TypedTrees) -> bool {
        typed_trees::references::collect_program_type_occurrences(typed)
            .into_iter()
            .any(|occurrence| {
                typed
                    .layout_storage_arguments(occurrence.reference)
                    .is_some()
            })
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct FixedLayoutTargetScope {
        pub target: String,
        pub facts: Option<layout_plans::LayoutTargetFacts>,
        pub entry_declarations: Option<Vec<symbols::SymbolHandle>>,
        pub provider_declarations: Vec<crate::product::provider_declarations::ProviderSelection>,
    }

    pub fn evaluate_fixed_layout_applications(
        typed: &mut TypedTrees,
        scopes: &[FixedLayoutTargetScope],
        selection_authority: Option<std::sync::Arc<dyn crate::BuildTimeSelectionAuthority>>,
    ) -> Result<(), Vec<Diagnostic>> {
        if !has_fixed_layout_applications(typed) {
            return Ok(());
        }
        typed_trees_to_typed_trees::prepare_execution_applications(typed)?;
        let checking = checking_target_applications(typed, scopes)?;
        let mut targets = Vec::new();
        let mut inputs = Vec::new();
        let facts_supplied = scopes.iter().any(|scope| scope.facts.is_some());
        for (scope, work) in scopes.iter().zip(&checking) {
            if scope.facts.is_none() && facts_supplied {
                continue;
            }
            if let Some(facts) = &scope.facts {
                facts
                    .validate()
                    .map_err(|reason| vec![Diagnostic::error(reason)])?;
                if facts.profile != scope.target {
                    return Err(vec![Diagnostic::error(
                        "fixed representation scope differs from its explicit target facts",
                    )]);
                }
                if !targets.contains(facts) {
                    targets.push(facts.clone());
                }
            }
            for occurrence in collect_selected_program_type_occurrences(typed, work) {
                let Some(storage) = typed.layout_storage_arguments(occurrence.reference) else {
                    continue;
                };
                if typed.data_field_definition(storage.semantic_type).is_none() {
                    continue;
                }
                let facts = scope.facts.as_ref().ok_or_else(|| vec![Diagnostic::error(format!(
                "fixed representation application `{}` requires unavailable representation facts for `{}`",
                typed.display_type_reference(storage.application), scope.target,
            ))])?;
                let identity = typed.normalized_type_identity(storage.application);
                let application_identity = typed.type_identity_text(identity).ok_or_else(|| {
                    vec![Diagnostic::error(
                        "fixed representation lost its canonical application identity",
                    )]
                })?;
                let input = crate::LayoutApplicationTargetInput {
                    application_identity,
                    target: facts.clone(),
                    lexical_owner: occurrence.lexical_owner,
                };
                if !inputs.contains(&input) {
                    inputs.push(input);
                }
            }
        }
        let mut domain = crate::build_time_evaluation::EvaluationDomain::all();
        domain.layout_targets = targets;
        domain.layout_applications = Some(inputs);
        compute_prepared_plan_laid_layouts_in(typed, selection_authority, &domain)?;
        layout_installation::validate_layout_application_coverage(
            typed,
            domain.layout_applications.as_deref(),
        )?;
        for occurrence in typed_trees::references::collect_program_type_occurrences(typed) {
            if let Some(storage) = typed.layout_storage_arguments(occurrence.reference)
                && typed.data_field_definition(storage.semantic_type).is_some()
                && typed_trees::domain::index_argument_is_closed(typed, storage.policy)
                && typed
                    .plan_laid_layout_for_type(storage.application)
                    .is_none()
            {
                return Err(vec![Diagnostic::error(
                    "fixed representation application has no explicit target-input closure",
                )]);
            }
        }
        Ok(())
    }

    pub(super) fn checking_target_applications(
        typed: &TypedTrees,
        scopes: &[FixedLayoutTargetScope],
    ) -> Result<Vec<typed_trees::references::DeclarationSelection>, Vec<Diagnostic>> {
        let defaults = retain_provider_defaults(
            crate::product::provider_declarations::ProviderPlanningContext::new(
                typed,
                DependencyScope::Product,
            ),
        )?;
        let mut topology =
            typed_trees::typed_trees::calls::application_graph::source_application_plan(typed);
        let dynamic = validation::describe_dynamic_conformances(typed)?;
        let symbols = &mut topology.symbols;
        topology.machines.for_each_mut(|_, machine| {
            machine.dynamic_realizations = symbols.insert_many(
                dynamic
                    .selections
                    .iter()
                    .filter(|selection| selection.machine == machine.machine)
                    .flat_map(|selection| selection.rows.iter().map(|row| row.realization_machine)),
            );
        });
        crate::BuildTimeAdmissionPlan::validate_runtime_effects(typed)?;
        let operational = validation::infer_operational_may(typed);
        source_checking_applications(typed, scopes, &defaults, &topology, &operational)
            .map_err(|reason| vec![Diagnostic::error(reason)])
    }
}
mod policy_arguments {
    use crate::build_time_evaluation::type_reference_table;
    use crate::evaluation_requests::BuildMachineEvaluationRequest;
    use build_time_values::BuildTimeValue;
    use typed_trees::{TypedTrees, machine::Machine, types::TypeReferenceNode};

    pub(super) fn policy_request<'a>(
        typed: &TypedTrees,
        machine: &Machine,
        arguments: Vec<BuildTimeValue>,
    ) -> Result<BuildMachineEvaluationRequest<'a>, String> {
        let entry = typed
            .machine_states(machine)
            .first()
            .ok_or_else(|| format!("policy `{}` has no entry state", machine.name.as_str()))?;
        let parameters = typed.state_parameters(entry);
        let positional_count = parameters.len();
        if positional_count != arguments.len() {
            let missing_context = || {
                format!(
                    "policy `{}` needs one unique declared data context in addition to its {} supplied argument(s)",
                    machine.name.as_str(),
                    arguments.len()
                )
            };
            let position = typed
                .data_binding_position(machine.attached_data_symbol, parameters)
                .ok_or_else(missing_context)?;
            let parameter = &parameters[position];
            if positional_count != arguments.len() + 1 {
                return Err(missing_context());
            }
            let context_type =
                match type_reference_table(typed).type_reference(parameter.type_reference) {
                    TypeReferenceNode::Reference { referee, .. } => *referee,
                    _ => parameter.type_reference,
                };
            let TypeReferenceNode::Named { symbol, .. } =
                typed.type_reference_table.type_reference(context_type)
            else {
                return Err(format!(
                    "policy `{}` context must name its declared data type",
                    machine.name.as_str()
                ));
            };
            let data = typed
                .data_definition_by_symbol(*symbol)
                .ok_or_else(missing_context)?;
            if validation::data_requires_establishment(typed, data)
                || !validation::zeroed_structural_parts(typed, context_type)
            {
                return Err(format!(
                    "policy `{}` context `{}` is not an established zero-initialized value",
                    machine.name.as_str(),
                    parameter.name.as_str()
                ));
            }
            // The context is a zero-initialized value of the policy's data type.
            // Only the removed tree interpreter could build one, so a policy
            // that takes its data context is refused here.
            return Err(crate::evaluation_requests::no_typed_tree_engine(&format!(
                "policy `{}`, which takes its data context `{}`,",
                machine.name.as_str(),
                parameter.name.as_str()
            ))
            .to_string());
        }
        Ok(BuildMachineEvaluationRequest::symbol(
            machine.symbol,
            arguments,
        ))
    }
}
pub(crate) mod schema_reflection {
    use crate::build_time_evaluation::type_reference_table;
    use typed_trees::TypedTrees;
    use typed_trees::data::DataDefinition;
    use typed_trees::types::{PrimitiveType, TypeReferenceHandle};

    mod schema_value {
        use typed_trees::TypedTrees;
        use typed_trees::data::{DataMember, DataShapeKind};

        use super::{
            SCHEMA_FIELD_CAPACITY, SchemaFieldInfo, local_schema_field_discriminator,
            reflected_field_layout,
        };
        use build_time_values::BuildTimeValue;

        fn optional_identity(identity: Option<u64>) -> BuildTimeValue {
            match identity {
                Some(identity) => BuildTimeValue::Case {
                    variant: "Some".to_owned(),
                    payload: vec![("value".to_owned(), BuildTimeValue::Int(identity as i64))],
                },
                None => BuildTimeValue::Case {
                    variant: "None".to_owned(),
                    payload: Vec::new(),
                },
            }
        }

        fn padded_identities(identities: &[u64]) -> Vec<BuildTimeValue> {
            let mut identities = identities.to_vec();
            identities.sort_unstable();
            (0..SCHEMA_FIELD_CAPACITY)
                .map(|index| {
                    BuildTimeValue::Int(identities.get(index).copied().unwrap_or_default() as i64)
                })
                .collect()
        }

        fn build_schema_field_value(field: Option<&SchemaFieldInfo>) -> BuildTimeValue {
            let (key, size, align, identity, kind) = field
                .map(|field| {
                    (
                        field.key,
                        field.size,
                        field.align,
                        field.identity,
                        field.kind,
                    )
                })
                .unwrap_or((0, 0, 1, None, "Scalar"));
            BuildTimeValue::Struct {
                type_name: "SchemaField".to_owned(),
                fields: vec![
                    ("key".to_owned(), BuildTimeValue::Int(key as i64)),
                    ("size".to_owned(), BuildTimeValue::Int(size as i64)),
                    ("align".to_owned(), BuildTimeValue::Int(align as i64)),
                    ("identity".to_owned(), optional_identity(identity)),
                    (
                        "number".to_owned(),
                        BuildTimeValue::Int(identity.map(|identity| identity as i64).unwrap_or(-1)),
                    ),
                    (
                        "kind".to_owned(),
                        BuildTimeValue::Case {
                            variant: kind.to_owned(),
                            payload: Vec::new(),
                        },
                    ),
                    (
                        "layout_source".to_owned(),
                        BuildTimeValue::Case {
                            variant: match field
                                .map(|field| field.layout_source)
                                .unwrap_or_default()
                            {
                                typed_trees::PlanLaidFieldLayoutSource::Default => "Default",
                                typed_trees::PlanLaidFieldLayoutSource::Scalar => "Scalar",
                                typed_trees::PlanLaidFieldLayoutSource::Explicit => "Explicit",
                            }
                            .to_owned(),
                            payload: Vec::new(),
                        },
                    ),
                ],
            }
        }

        pub(crate) fn build_selected_schema_value(
            typed: &TypedTrees,
            data: &typed_trees::data::DataDefinition,
            schema_fields: &[SchemaFieldInfo],
        ) -> Result<BuildTimeValue, String> {
            let schema_data = data.name.as_str();
            let schema_fields = super::canonical_schema_fields(schema_fields);
            let mut fields = Vec::with_capacity(SCHEMA_FIELD_CAPACITY);
            for index in 0..SCHEMA_FIELD_CAPACITY {
                fields.push(build_schema_field_value(schema_fields.get(index).copied()));
            }
            let variants = typed
                .data_members(data)
                .iter()
                .filter_map(|member| match member {
                    DataMember::Variant(variant) => Some(variant),
                    DataMember::Field(_) => None,
                })
                .collect::<Vec<_>>();
            if variants.len() > SCHEMA_FIELD_CAPACITY {
                return Err(format!(
                    "schema data `{schema_data}` has {} cases; reflected Schema supports at most {}",
                    variants.len(),
                    SCHEMA_FIELD_CAPACITY
                ));
            }
            reject_local_discriminator_collisions(
                schema_data,
                "case",
                variants.iter().map(|variant| {
                    (
                        variant.name.to_string(),
                        local_schema_field_discriminator(schema_data, variant.name.as_str()),
                    )
                }),
            )?;
            let mut cases = Vec::with_capacity(SCHEMA_FIELD_CAPACITY);
            for index in 0..SCHEMA_FIELD_CAPACITY {
                let Some(variant) = variants.get(index).copied() else {
                    cases.push(BuildTimeValue::Struct {
                        type_name: "SchemaCase".to_owned(),
                        fields: vec![
                            ("key".to_owned(), BuildTimeValue::Int(0)),
                            ("identity".to_owned(), optional_identity(None)),
                            (
                                "payload_fields".to_owned(),
                                BuildTimeValue::Array(
                                    (0..SCHEMA_FIELD_CAPACITY)
                                        .map(|_| build_schema_field_value(None))
                                        .collect(),
                                ),
                            ),
                            ("payload_field_count".to_owned(), BuildTimeValue::Int(0)),
                            (
                                "retired_payload_identities".to_owned(),
                                BuildTimeValue::Array(padded_identities(&[])),
                            ),
                            (
                                "retired_payload_identity_count".to_owned(),
                                BuildTimeValue::Int(0),
                            ),
                        ],
                    });
                    continue;
                };
                let payload = typed.data_payload_fields(variant);
                if payload.len() > SCHEMA_FIELD_CAPACITY
                    || variant.retired_payload_identities.len() > SCHEMA_FIELD_CAPACITY
                {
                    return Err(format!(
                        "schema data `{schema_data}` case `{}` exceeds the reflected Schema capacity of {} payload fields or tombstones",
                        variant.name, SCHEMA_FIELD_CAPACITY
                    ));
                }
                reject_local_discriminator_collisions(
                    schema_data,
                    "payload field",
                    payload.iter().map(|field| {
                        let qualified = format!("{}::{}", variant.name, field.name);
                        let key = local_schema_field_discriminator(schema_data, qualified.as_str());
                        (field.name.to_string(), key)
                    }),
                )?;
                let payload_intervals = validation::case_requires_field_intervals(typed, variant);
                let payload_fields = (0..SCHEMA_FIELD_CAPACITY)
                    .map(|payload_index| {
                        let info = payload.get(payload_index).map(|field| {
                            let reflected = reflected_field_layout(
                                typed,
                                field.type_reference,
                                None,
                                payload_intervals.as_deref().and_then(|rows| {
                                    rows.iter()
                                        .find(|(symbol, _, _)| *symbol == field.symbol)
                                        .map(|(_, low, high)| (*low, *high))
                                }),
                            );
                            let (
                                size,
                                align,
                                source_bits,
                                primitive,
                                kind,
                                declared_range,
                                repeated,
                            ) = reflected.unwrap_or((0, 1, 0, None, "Nested", None, None));
                            SchemaFieldInfo {
                                schema_field: field.symbol,
                                name: field.name.to_string(),
                                identity: field.identity,
                                key: local_schema_field_discriminator(
                                    schema_data,
                                    format!("{}::{}", variant.name, field.name).as_str(),
                                ),
                                size,
                                align,
                                source_bits,
                                primitive,
                                kind,
                                layout_source: typed.field_layout_source(field.type_reference),
                                declared_range,
                                repeated,
                                scalar_leaf_width_bits: super::scalar_leaf_width_bits(
                                    typed,
                                    field.type_reference,
                                ),
                            }
                        });
                        build_schema_field_value(info.as_ref())
                    })
                    .collect();
                cases.push(BuildTimeValue::Struct {
                    type_name: "SchemaCase".to_owned(),
                    fields: vec![
                        (
                            "key".to_owned(),
                            BuildTimeValue::Int(local_schema_field_discriminator(
                                schema_data,
                                variant.name.as_str(),
                            ) as i64),
                        ),
                        ("identity".to_owned(), optional_identity(variant.identity)),
                        (
                            "payload_fields".to_owned(),
                            BuildTimeValue::Array(payload_fields),
                        ),
                        (
                            "payload_field_count".to_owned(),
                            BuildTimeValue::Int(payload.len() as i64),
                        ),
                        (
                            "retired_payload_identities".to_owned(),
                            BuildTimeValue::Array(padded_identities(
                                &variant.retired_payload_identities,
                            )),
                        ),
                        (
                            "retired_payload_identity_count".to_owned(),
                            BuildTimeValue::Int(variant.retired_payload_identities.len() as i64),
                        ),
                    ],
                });
            }
            let shape = typed_trees::data::DataDefinition::shape_kind_from_members(
                typed.data_members(data),
            );
            let (retired_fields, retired_cases) = match shape {
                DataShapeKind::Record => (data.retired_identities.as_slice(), &[][..]),
                DataShapeKind::Enum => (&[][..], data.retired_identities.as_slice()),
                DataShapeKind::Empty | DataShapeKind::Mixed => (&[][..], &[][..]),
            };
            Ok(BuildTimeValue::Struct {
                type_name: "Schema".to_owned(),
                fields: vec![
                    ("fields".to_owned(), BuildTimeValue::Array(fields)),
                    (
                        "field_count".to_owned(),
                        BuildTimeValue::Int(schema_fields.len() as i64),
                    ),
                    (
                        "retired_field_identities".to_owned(),
                        BuildTimeValue::Array(padded_identities(retired_fields)),
                    ),
                    (
                        "retired_field_identity_count".to_owned(),
                        BuildTimeValue::Int(retired_fields.len() as i64),
                    ),
                    ("cases".to_owned(), BuildTimeValue::Array(cases)),
                    (
                        "case_count".to_owned(),
                        BuildTimeValue::Int(variants.len() as i64),
                    ),
                    (
                        "retired_case_identities".to_owned(),
                        BuildTimeValue::Array(padded_identities(retired_cases)),
                    ),
                    (
                        "retired_case_identity_count".to_owned(),
                        BuildTimeValue::Int(retired_cases.len() as i64),
                    ),
                ],
            })
        }

        fn reject_local_discriminator_collisions(
            schema: &str,
            scope: &str,
            entries: impl IntoIterator<Item = (String, u64)>,
        ) -> Result<(), String> {
            let mut seen = std::collections::BTreeMap::new();
            for (name, discriminator) in entries {
                if let Some(prior) = seen.insert(discriminator, name.clone()) {
                    return Err(format!(
                        "schema data `{schema}` has a compiler-local {scope} discriminator collision between `{prior}` and `{name}`"
                    ));
                }
            }
            Ok(())
        }
    }
    pub(crate) use schema_value::build_selected_schema_value;

    const SCHEMA_FIELD_CAPACITY: usize = 32;

    pub(crate) fn local_schema_field_discriminator(schema: &str, field: &str) -> u64 {
        let mut hash = semantic_vocabulary::Fnv1a::new();
        for byte in schema.bytes().chain(*b"::").chain(field.bytes()) {
            hash.byte(byte);
        }
        hash.finish_or(1)
    }

    #[derive(Debug, Clone)]
    pub(crate) struct SchemaFieldInfo {
        pub(crate) schema_field: symbols::SymbolHandle,
        pub(crate) name: String,
        pub(crate) identity: Option<u64>,
        pub(crate) key: u64,
        pub(crate) size: u64,
        pub(crate) align: u64,
        pub(crate) source_bits: u64,
        pub(crate) primitive: Option<PrimitiveType>,
        pub(crate) kind: &'static str,
        pub(crate) layout_source: typed_trees::PlanLaidFieldLayoutSource,
        pub(crate) declared_range: Option<(i64, i64)>,
        pub(crate) repeated: Option<RepeatedFieldInfo>,
        pub(crate) scalar_leaf_width_bits: Option<u16>,
    }

    #[derive(Debug, Clone, Copy)]
    pub(crate) struct RepeatedFieldInfo {
        pub(crate) element_size: u64,
        pub(crate) element_count: u64,
    }

    pub(crate) fn canonical_schema_fields(fields: &[SchemaFieldInfo]) -> Vec<&SchemaFieldInfo> {
        let mut ordered = fields.iter().collect::<Vec<_>>();
        if ordered.iter().any(|field| field.identity.is_some()) {
            ordered.sort_unstable_by_key(|field| field.identity);
        }
        ordered
    }

    pub(crate) fn schema_fields_for_target(
        typed: &TypedTrees,
        data: &DataDefinition,
        target: Option<&layout_plans::LayoutTargetFacts>,
    ) -> Result<(Vec<SchemaFieldInfo>, u64), String> {
        let schema_data = data.name.as_str();
        if data.quotient.is_some() {
            return Err(format!(
                "schema reflection cannot observe quotient `{schema_data}`: retained representatives are opaque and require a named lifted operation"
            ));
        }

        let field_intervals = validation::data_requires_field_intervals(typed, data);
        let mut fields = Vec::new();
        for member in typed.data_members(data) {
            let Some(field) = member.as_field() else {
                continue;
            };
            if field.relevance.is_erased() {
                continue;
            }
            let (size, align, source_bits, primitive, kind, declared_range, repeated) =
            reflected_field_layout(
                typed,
                field.type_reference,
                target,
                field_intervals.as_deref().and_then(|rows| rows.iter()
                    .find(|(symbol, _, _)| *symbol == field.symbol)
                    .map(|(_, low, high)| (*low, *high))),
            ).map_err(|reason| {
                format!(
                    "schema data `{schema_data}` field `{}` has no reflected storage geometry: {reason}",
                    field.name,
                )
            })?;
            if size == 0 {
                continue;
            }
            let key = local_schema_field_discriminator(schema_data, field.name.as_str());
            if fields
                .iter()
                .any(|existing: &SchemaFieldInfo| existing.key == key)
            {
                return Err(format!(
                    "schema data `{schema_data}` has a compiler-local field discriminator collision involving `{}`",
                    field.name
                ));
            }
            fields.push(SchemaFieldInfo {
                schema_field: field.symbol,
                name: field.name.to_string(),
                identity: field.identity,
                key,
                size,
                align,
                source_bits,
                primitive,
                kind,
                layout_source: typed.field_layout_source(field.type_reference),
                declared_range,
                repeated,
                scalar_leaf_width_bits: scalar_leaf_width_bits(typed, field.type_reference),
            });
        }
        if fields.len() > SCHEMA_FIELD_CAPACITY {
            return Err(format!(
                "schema data `{schema_data}` has {} fields; the current layout slice supports at most {}",
                fields.len(),
                SCHEMA_FIELD_CAPACITY
            ));
        }
        if data.retired_identities.len() > SCHEMA_FIELD_CAPACITY {
            return Err(format!(
                "schema data `{schema_data}` has {} retired identities; reflected Schema supports at most {} per scope",
                data.retired_identities.len(),
                SCHEMA_FIELD_CAPACITY
            ));
        }
        Ok((fields, normalized_schema_report_fingerprint(typed, data)))
    }

    fn scalar_leaf_width_bits(
        typed: &TypedTrees,
        mut reference: TypeReferenceHandle,
    ) -> Option<u16> {
        loop {
            if let Some(primitive) = typed.primitive_type_reference(reference) {
                return u16::try_from(primitive_byte_size(primitive)?.checked_mul(8)?).ok();
            }
            reference = type_reference_table(typed).unconstrained_type_reference(reference);
            match typed.type_reference_table.type_reference(reference) {
                typed_trees::types::TypeReferenceNode::FixedArray { element_type, .. } => {
                    reference = *element_type
                }
                _ => return None,
            }
        }
    }

    pub fn normalized_schema_report_fingerprint(typed: &TypedTrees, data: &DataDefinition) -> u64 {
        typed_trees::identity::normalized_schema_report_fingerprint(typed, data)
    }

    fn declared_source_bits(
        typed: &TypedTrees,
        type_reference: TypeReferenceHandle,
        primitive: PrimitiveType,
        byte_size: u64,
        stated: Option<(Option<i64>, Option<i64>)>,
    ) -> u64 {
        if primitive == PrimitiveType::Bool {
            return 1;
        }
        let width = byte_size * 8;
        if !primitive.is_integer() {
            return width;
        }
        let (mut minimum, mut maximum) = if primitive.is_signed_integer() {
            (-(1i128 << (width - 1)), (1i128 << (width - 1)) - 1)
        } else {
            (0, (1i128 << width) - 1)
        };
        if let Some(range) = validation::scalar_representation_range(typed, type_reference) {
            minimum = minimum.max(i128::from(range.minimum));
            maximum = maximum.min(i128::from(range.maximum));
        }
        if let Some((low, high)) = stated {
            if let Some(low) = low {
                minimum = minimum.max(i128::from(low));
            }
            if let Some(high) = high {
                maximum = maximum.min(i128::from(high));
            }
        }
        if minimum < 0 {
            return width;
        }
        let Ok(maximum) = u64::try_from(maximum) else {
            return width;
        };
        u64::from((u64::BITS - maximum.leading_zeros()).max(1))
    }

    pub(super) fn primitive_byte_size(primitive: PrimitiveType) -> Option<u64> {
        Some(match primitive {
            PrimitiveType::I8 | PrimitiveType::U8 | PrimitiveType::Bool => 1,
            PrimitiveType::I16 | PrimitiveType::U16 => 2,
            PrimitiveType::I32 | PrimitiveType::U32 | PrimitiveType::F32 => 4,
            PrimitiveType::I64 | PrimitiveType::U64 | PrimitiveType::F64 => 8,
            _ => return None,
        })
    }

    type ReflectedFieldLayout = (
        u64,
        u64,
        u64,
        Option<PrimitiveType>,
        &'static str,
        Option<(i64, i64)>,
        Option<RepeatedFieldInfo>,
    );

    pub(super) fn reflected_field_layout(
        typed: &TypedTrees,
        reference: TypeReferenceHandle,
        target: Option<&layout_plans::LayoutTargetFacts>,
        stated: Option<(Option<i64>, Option<i64>)>,
    ) -> Result<ReflectedFieldLayout, String> {
        if let Some(primitive) = typed.primitive_type_reference(reference) {
            let size = primitive_byte_size(primitive)
                .ok_or("primitive has no fixed byte representation")?;
            return Ok((
                size,
                size,
                declared_source_bits(typed, reference, primitive, size, stated),
                Some(primitive),
                "Scalar",
                validation::scalar_representation_range(typed, reference)
                    .map(|range| (range.minimum, range.maximum)),
                None,
            ));
        }
        let carrier = type_reference_table(typed).unconstrained_type_reference(reference);
        let (size, align) = reflected_member_layout(typed, carrier, target, &mut Vec::new())?;
        let repeated = match typed.type_reference_table.type_reference(carrier) {
            typed_trees::types::TypeReferenceNode::FixedArray {
                element_type,
                length: typed_trees::types::FixedArrayLength::Literal(length),
            } => {
                let (element_size, _) =
                    reflected_member_layout(typed, *element_type, target, &mut Vec::new())?;
                Some(RepeatedFieldInfo {
                    element_size,
                    element_count: u64::try_from(*length)
                        .map_err(|_| "array count is outside the storage range")?,
                })
            }
            _ => None,
        };
        Ok((
            size,
            align,
            size.checked_mul(8).ok_or("field bit count overflows")?,
            None,
            if repeated.is_some() {
                "Repeated"
            } else {
                "Nested"
            },
            None,
            repeated,
        ))
    }

    fn reflected_member_layout(
        typed: &TypedTrees,
        reference: TypeReferenceHandle,
        target: Option<&layout_plans::LayoutTargetFacts>,
        visiting: &mut Vec<typed_trees::type_identity::TypeIdentityId>,
    ) -> Result<(u64, u64), String> {
        use typed_trees::types::{FixedArrayLength, TypeReferenceNode};
        if let Some(primitive) = typed.primitive_type_reference(reference) {
            let size = primitive_byte_size(primitive)
                .ok_or("primitive has no fixed byte representation")?;
            return Ok((size, size));
        }
        let carrier = type_reference_table(typed).unconstrained_type_reference(reference);
        match typed.type_reference_table.type_reference(carrier) {
            TypeReferenceNode::Reference { referee, .. } => {
                let target = target.ok_or("reference storage requires explicit target geometry")?;
                target.validate()?;
                let words = typed
                    .reference_descriptor_word_count(*referee)
                    .ok_or("reference has no closed descriptor shape")?;
                Ok((
                    target
                        .pointer_size
                        .checked_mul(words)
                        .ok_or("reference footprint overflows")?,
                    target.pointer_alignment,
                ))
            }
            TypeReferenceNode::FixedArray {
                element_type,
                length: FixedArrayLength::Literal(length),
            } => {
                let (size, align) =
                    reflected_member_layout(typed, *element_type, target, visiting)?;
                let count = u64::try_from(*length)
                    .map_err(|_| "array count is outside the storage range")?;
                Ok((
                    size.checked_mul(count).ok_or("array footprint overflows")?,
                    align,
                ))
            }
            TypeReferenceNode::Named { .. } | TypeReferenceNode::Generic { .. } => {
                if let Some(layout) = typed.plan_laid_layout_for_type(carrier) {
                    let geometry = layout
                        .geometry_for_target(typed, target.map(|target| target.profile.as_str()))
                        .ok_or("selected layout has no unique geometry for this target")?;
                    return Ok((
                        geometry
                            .validated_layout
                            .size
                            .ok_or("selected layout has no fixed footprint")?,
                        geometry.validated_layout.align,
                    ));
                }
                if typed.layout_storage_arguments(carrier).is_some() {
                    return Err("representation recipe has no admitted layout".into());
                }
                let data = typed
                    .data_field_definition(carrier)
                    .ok_or("type has no closed data declaration")?;
                let identity = typed.normalized_type_identity(carrier);
                if data.quotient.is_some() {
                    return Err("quotient representatives are opaque to schema reflection".into());
                }
                if data.supply_mode != language_semantics::DataSupplyMode::CheckedShape {
                    return Err("data has no checked storage shape".into());
                }
                if visiting.contains(&identity) {
                    return Err("by-value storage contains a recursive cycle".into());
                }
                visiting.push(identity);
                let result = (|| {
                    let members = typed.data_members(data);
                    if members.iter().any(|member| member.as_variant().is_some()) {
                        let target =
                            target.ok_or("sum storage requires explicit target tag geometry")?;
                        target.validate()?;
                        let field_geometry =
                            |field: &typed_trees::data::DataField, visiting: &mut Vec<_>| {
                                let (byte_size, byte_alignment) = reflected_member_layout(
                                    typed,
                                    field.type_reference,
                                    Some(target),
                                    visiting,
                                )?;
                                Ok::<_, String>(layout_plans::StorageGeometry {
                                    byte_size,
                                    byte_alignment,
                                })
                            };
                        let mut common = Vec::new();
                        let mut cases = Vec::new();
                        for member in members {
                            match member {
                                typed_trees::data::DataMember::Field(field) => {
                                    if !field.relevance.is_erased() {
                                        common.push(field_geometry(field, visiting)?);
                                    }
                                }
                                typed_trees::data::DataMember::Variant(case) => {
                                    let mut payload = Vec::new();
                                    for field in typed.data_payload_fields(case) {
                                        if !field.relevance.is_erased() {
                                            payload.push(field_geometry(field, visiting)?);
                                        }
                                    }
                                    cases.push(payload);
                                }
                            }
                        }
                        let geometry = layout_plans::conventional_sum_geometry(
                            layout_plans::StorageGeometry {
                                byte_size: target.case_tag_size,
                                byte_alignment: target.case_tag_alignment,
                            },
                            common,
                            cases,
                        )
                        .map_err(|reason| format!("sum storage geometry is invalid: {reason:?}"))?
                        .storage;
                        return Ok((geometry.byte_size, geometry.byte_alignment));
                    }
                    let mut offset = 0u64;
                    let mut aggregate_align = 1u64;
                    for member in typed.data_members(data) {
                        let field = member
                            .as_field()
                            .ok_or("record contains a non-field member")?;
                        if field.relevance.is_erased() {
                            continue;
                        }
                        let (size, align) =
                            reflected_member_layout(typed, field.type_reference, target, visiting)?;
                        offset = checked_align_up(offset, align)
                            .and_then(|offset| offset.checked_add(size))
                            .ok_or("record field offset overflows or has invalid alignment")?;
                        aggregate_align = aggregate_align.max(align);
                    }
                    Ok((
                        checked_align_up(offset, aggregate_align)
                            .ok_or("record footprint overflows or has invalid alignment")?,
                        aggregate_align,
                    ))
                })();
                visiting.pop();
                result
            }
            _ => Err("type has no fixed reflected storage shape".into()),
        }
    }

    pub(super) fn checked_align_up(value: u64, align: u64) -> Option<u64> {
        if align == 0 {
            return None;
        }
        value
            .checked_add(align - 1)
            .map(|value| value / align * align)
    }

    #[cfg(test)]
    mod tests {
        use super::{reflected_member_layout, schema_fields_for_target};
        use symbols::{SymbolKind, SymbolNameRef, SymbolTableBuilder};
        use typed_trees::TypedTrees;
        use typed_trees::data::{DataDefinition, DataField, DataMember, DataVariant};
        use typed_trees::name::Identifier;
        use typed_trees::types::{FixedArrayLength, TypeReferenceNode};

        #[test]
        fn empty_record_has_an_empty_physical_schema() {
            let typed = TypedTrees::default();
            let record = DataDefinition::default();
            let (fields, fingerprint) = schema_fields_for_target(&typed, &record, None)
                .expect("an empty record needs no physical field entries");
            assert!(fields.is_empty());
            assert_eq!(
                fingerprint,
                typed_trees::identity::normalized_schema_report_fingerprint(&typed, &record)
            );
        }

        #[test]
        fn nested_sum_geometry_uses_selected_tag_and_preserves_array_stride() {
            let mut symbols = SymbolTableBuilder::default();
            let root = symbols.insert_root(SymbolKind::Root, SymbolNameRef::Static("geometry"));
            let sum = symbols
                .insert_children(root, [(SymbolKind::Data, SymbolNameRef::Static("Choice"))])
                .start();
            let mut typed = TypedTrees {
                symbols: symbols.finish(),
                ..TypedTrees::default()
            };
            let byte = typed.type_reference_table.insert(TypeReferenceNode::Named {
                symbol: symbols::SymbolHandle::invalid(),
                name: Identifier::from("u8"),
            });
            let word = typed.type_reference_table.insert(TypeReferenceNode::Named {
                symbol: symbols::SymbolHandle::invalid(),
                name: Identifier::from("u64"),
            });
            let payload = typed.data_payload_fields.insert_many([DataField {
                name: Identifier::from("value"),
                type_reference: word,
                ..Default::default()
            }]);
            let members = typed.data_members.insert_many([
                DataMember::Field(DataField {
                    name: Identifier::from("common"),
                    type_reference: byte,
                    ..Default::default()
                }),
                DataMember::Variant(DataVariant {
                    name: Identifier::from("Empty"),
                    ..Default::default()
                }),
                DataMember::Variant(DataVariant {
                    name: Identifier::from("Held"),
                    payload,
                    ..Default::default()
                }),
            ]);
            typed.push_data_definition(DataDefinition {
                symbol: sum,
                name: Identifier::from("Choice"),
                members,
                ..Default::default()
            });
            let choice = typed.type_reference_table.insert(TypeReferenceNode::Named {
                symbol: sum,
                name: Identifier::from("Choice"),
            });
            let array = typed
                .type_reference_table
                .insert(TypeReferenceNode::FixedArray {
                    element_type: choice,
                    length: FixedArrayLength::Literal(2),
                });
            let mut target = layout_plans::LayoutTargetFacts {
                profile: "test".into(),
                pointer_size: 8,
                pointer_alignment: 8,
                case_tag_size: 4,
                case_tag_alignment: 4,
                little_endian: true,
            };
            assert_eq!(
                reflected_member_layout(&typed, choice, None, &mut Vec::new()),
                Err("sum storage requires explicit target tag geometry".into())
            );
            assert_eq!(
                reflected_member_layout(&typed, choice, Some(&target), &mut Vec::new()),
                Ok((16, 8))
            );
            assert_eq!(
                reflected_member_layout(&typed, array, Some(&target), &mut Vec::new()),
                Ok((32, 8))
            );
            target.case_tag_size = 16;
            target.case_tag_alignment = 16;
            assert_eq!(
                reflected_member_layout(&typed, choice, Some(&target), &mut Vec::new()),
                Ok((32, 16))
            );
            assert_eq!(
                reflected_member_layout(&typed, array, Some(&target), &mut Vec::new()),
                Ok((64, 16))
            );
            target.case_tag_alignment = 3;
            assert_eq!(
                reflected_member_layout(&typed, choice, Some(&target), &mut Vec::new()),
                Err("layout target facts have no complete immutable representation profile".into())
            );
        }
    }
}

/*
Contextual conformance, provider, and installed-interrupt call specialization
(`contextual_calls`) alongside trait requirement application inference and call
instantiation (`requirement_applications`).
*/

pub(crate) mod contextual_calls {
    pub(crate) mod applications {
        use crate::monomorphization::{
            Candidate, Diagnostic, SymbolHandle, TypeReferenceHandle, TypedTrees,
        };
        use crate::tables::{expressions_of, expressions_of_mut};
        use typed_trees::{
            signature::StateSignature, state::State,
            static_applications::StaticMachineApplicationHandle,
        };

        pub(crate) fn retain_view(
            program: &mut TypedTrees,
            provider: &Candidate<'_>,
            invocation: StaticMachineApplicationHandle,
            ordinal: usize,
            selected: SymbolHandle,
            demand: SymbolHandle,
            expected: &StateSignature,
            caller_types: &[(SymbolHandle, TypeReferenceHandle)],
        ) -> Result<(), Vec<Diagnostic>> {
            let caller_lifetimes = expressions_of(program)
                .static_machine_application_lifetimes(invocation)
                .to_vec();
            program
                .prepare_applied_machine_callable(
                    invocation,
                    ordinal,
                    provider.binding_application,
                    selected,
                    demand,
                    expected,
                    caller_types,
                    &caller_lifetimes,
                )
                .map_err(|message| vec![Diagnostic::error(message)])
        }

        pub(crate) fn retain_provider(
            program: &mut TypedTrees,
            provider: &Candidate<'_>,
            state: &State,
            invocation: StaticMachineApplicationHandle,
            lifetimes: &[typed_trees::static_applications::StaticMachineLifetimeBinding],
        ) -> StaticMachineApplicationHandle {
            let owner = expressions_of(program)
                .static_machine_application(invocation)
                .lexical_owner;
            let retained = expressions_of_mut(program).retain_static_machine_application(
                state.symbol,
                state.name.clone(),
                &[],
            );
            let record = expressions_of_mut(program).static_machine_application_mut(retained);
            record.lexical_owner = owner;
            record.binding_declaration = provider.template.template_symbol;
            let bindings =
                crate::monomorphization::applications::candidate_bindings(program, provider);
            expressions_of_mut(program)
                .set_static_machine_application_bindings(retained, &bindings);
            expressions_of_mut(program).set_static_machine_application_lifetimes(
                retained,
                provider.template.template_symbol,
                lifetimes,
            );
            retained
        }
    }
    pub(crate) mod closed_requirements {
        use super::{CallSite, Diagnostic, TypedTrees, invocations};
        use crate::monomorphization::failed;
        use crate::tables::expressions_of;
        use invocations::machine_demands;

        pub(crate) fn retain(program: &mut TypedTrees) -> Result<(), Vec<Diagnostic>> {
            for invocation in invocations::collect(program)? {
                let (target, application) = match invocation.site {
                    CallSite::Expression(handle) => {
                        let Some(call) = expressions_of(program).call_node(handle) else {
                            continue;
                        };
                        (call.target_symbol, call.authored_machine_application)
                    }
                    CallSite::Statement(handle) => {
                        let Some(call) = program.statement_table.statement(handle).as_call() else {
                            continue;
                        };
                        (call.target_symbol, call.authored_machine_application)
                    }
                };
                let Some(closed) = program.closed_requirement_application(target) else {
                    continue;
                };
                if closed.template != invocation.target || !application.is_valid() {
                    return Err(vec![Diagnostic::error(
                        "closed requirement demand lost its exact source invocation",
                    )]);
                }
                let (_, signature) = program
                    .trait_machine_signature(closed.template)
                    .ok_or_else(|| {
                        failed("closed requirement demand lost its source declaration")
                    })?;
                let source_demands =
                    machine_demands(program, program.state_signature_type_parameters(signature));
                if source_demands.len() != invocation.requirements.len() {
                    return Err(vec![Diagnostic::error(
                        "closed requirement demand changed its machine binder roster",
                    )]);
                }
                for (ordinal, (argument, demand)) in invocation
                    .machine_arguments
                    .iter()
                    .zip(&invocation.requirements)
                    .enumerate()
                {
                    let source = source_demands
                        .get(ordinal)
                        .filter(|source| source.parameter == demand.parameter)
                        .ok_or_else(|| {
                            failed("closed requirement demand lost its source machine binder")
                        })?;
                    let retained = program.applied_machine_provider(
                        application,
                        ordinal,
                        argument.symbol,
                        source.parameter,
                        &source.signature,
                    );
                    let Some(provider) = retained else {
                        if program.has_applied_machine_selection(
                            application,
                            ordinal,
                            argument.symbol,
                        ) {
                            return Err(vec![Diagnostic::error(
                                "closed requirement call lost its determined source machine demand",
                            )]);
                        }
                        continue;
                    };
                    let bindings =
                        expressions_of(program).static_machine_application_bindings(provider);
                    let lifetimes =
                        expressions_of(program).static_machine_application_lifetimes(provider);
                    let caller_operands = &invocation.types;
                    let caller_lifetimes = expressions_of(program)
                        .static_machine_application_lifetimes(application)
                        .to_vec();
                    if let Some(reason) = program.applied_machine_callable_mismatch(
                        application,
                        ordinal,
                        argument.symbol,
                        source.parameter,
                        bindings,
                        lifetimes,
                        &source.signature,
                        caller_operands,
                        &caller_lifetimes,
                    ) {
                        return Err(vec![Diagnostic::error(format!(
                            "call to {} cannot reuse {} for {}: {reason}",
                            program.symbols.name(invocation.target),
                            program.symbols.name(argument.symbol),
                            program.symbols.name(source.parameter),
                        ))]);
                    }
                    if program.applied_machine_callable_matches(
                        application,
                        ordinal,
                        argument.symbol,
                        demand.parameter,
                        bindings,
                        lifetimes,
                        &demand.signature,
                        caller_operands,
                        &caller_lifetimes,
                    ) {
                        continue;
                    }
                    program
                        .prepare_applied_machine_callable(
                            application,
                            ordinal,
                            provider,
                            argument.symbol,
                            demand.parameter,
                            &demand.signature,
                            caller_operands,
                            &caller_lifetimes,
                        )
                        .map_err(|message| vec![Diagnostic::error(message)])?;
                }
                program
                    .prepare_selected_machine_contracts(application)
                    .map_err(|message| vec![Diagnostic::error(message)])?;
            }
            Ok(())
        }
    }
    pub(crate) use closed_requirements::retain as retain_closed_requirement_demands;
    pub(crate) mod conformance_demands {
        use crate::monomorphization::{Diagnostic, TypedTrees};
        use crate::tables::{expressions_of, expressions_of_mut};
        use symbols::SymbolKind;
        use typed_trees::data::TypeParameterKind;
        use typed_trees::static_applications::{
            StaticMachineApplicationHandle, StaticMachineBinding, StaticMachineBindingValue,
            StaticMachineLifetimeBinding,
        };
        use typed_trees::typed_trees::{
            ClosedConformanceApplication, ClosedConformanceApplicationSite,
        };

        pub(crate) fn prepare(program: &mut TypedTrees) -> Result<(), Vec<Diagnostic>> {
            let pending = program
                .closed_conformance_application_sites()
                .into_iter()
                .filter(|(_, _, _, app)| !app.callable_application.is_valid())
                .map(|(site, _, supplying, app)| (site, supplying, app.clone()))
                .collect::<Vec<_>>();
            for (site, supplying, application) in pending {
                prepare_site(program, site, supplying, application)?;
            }
            Ok(())
        }

        pub(crate) fn prepare_occurrence(
            program: &mut TypedTrees,
            occurrence: StaticMachineApplicationHandle,
        ) -> Result<(), Vec<Diagnostic>> {
            program
                .prepare_closed_conformance_occurrence_scope(occurrence)
                .map_err(|reason| failure(&reason))?;
            let site = ClosedConformanceApplicationSite::Occurrence(occurrence);
            let application = program
                .closed_conformance_application_at_mut(site)
                .ok_or_else(|| failure("captured demand lost its source occurrence"))?
                .clone();
            if application.callable_application.is_valid() {
                return Ok(());
            }
            prepare_site(program, site, occurrence, application)
        }

        fn prepare_site(
            program: &mut TypedTrees,
            site: ClosedConformanceApplicationSite,
            supplying: StaticMachineApplicationHandle,
            application: ClosedConformanceApplication,
        ) -> Result<(), Vec<Diagnostic>> {
            let declaration = program
                .conformance_by_symbol(application.declaration)
                .cloned()
                .ok_or_else(|| failure("captured callable demand lost its map declaration"))?;
            let parameters = program.conformance_type_parameters(&declaration).to_vec();
            if parameters.len() != application.arguments.len()
                || declaration.lifetime_parameters.len() != application.lifetime_arguments.len()
            {
                return Err(failure(
                    "captured callable demand requires its complete map telescope",
                ));
            }

            let source = expressions_of(program)
                .static_machine_application(supplying)
                .clone();
            let invocation = expressions_of_mut(program).retain_static_machine_application(
                declaration.symbol,
                declaration.alias.clone().unwrap_or_default(),
                &application.arguments,
            );
            let record = expressions_of_mut(program).static_machine_application_mut(invocation);
            record.lexical_owner = source.lexical_owner;
            record.binding_declaration = declaration.symbol;
            let mut operands = Vec::new();
            let mut bindings = Vec::new();
            for (parameter, argument) in parameters.iter().zip(application.arguments.iter()) {
                let value = match &parameter.kind {
                    TypeParameterKind::Type
                    | TypeParameterKind::Const { .. }
                    | TypeParameterKind::Value { .. } => {
                        let actual = program
                            .static_operand_type_reference(argument)
                            .map_err(failure)?;
                        operands.push((parameter.symbol, actual));
                        if matches!(parameter.kind, TypeParameterKind::Type) {
                            StaticMachineBindingValue::Type(actual)
                        } else {
                            StaticMachineBindingValue::Const(actual)
                        }
                    }
                    TypeParameterKind::Machine { .. } => {
                        StaticMachineBindingValue::Machine(argument.clone())
                    }
                    _ => {
                        return Err(failure(
                            "captured callable demand has unsupported declaration operand",
                        ));
                    }
                };
                bindings.push(StaticMachineBinding {
                    parameter: parameter.symbol,
                    value,
                });
            }
            expressions_of_mut(program)
                .set_static_machine_application_bindings(invocation, &bindings);
            let lifetimes = declaration
                .lifetime_parameters
                .iter()
                .cloned()
                .zip(application.lifetime_arguments.iter())
                .map(|(parameter, argument)| StaticMachineLifetimeBinding {
                    parameter,
                    argument: typed_trees::name::Identifier::generated(argument),
                })
                .collect::<Vec<_>>();
            expressions_of_mut(program).set_static_machine_application_lifetimes(
                invocation,
                declaration.symbol,
                &lifetimes,
            );
            let mut selected = Vec::new();
            for (parameter, argument) in parameters.iter().zip(application.arguments.iter()) {
                let TypeParameterKind::Machine { contract } = &parameter.kind else {
                    continue;
                };
                let signature = program
                    .machine_parameter_contract_view(contract)
                    .map(|view| view.signature().clone())
                    .ok_or_else(|| {
                        failure("captured callable demand lost its required signature")
                    })?;
                let ordinal = selected.len();
                let closed = super::providers::close(
                    program,
                    argument,
                    parameter.symbol,
                    &signature,
                    &operands,
                    invocation,
                    ordinal,
                )?;
                let mut executable = argument.clone();
                if let Some(entry) = closed {
                    executable.symbol = entry;
                    executable.application = None;
                } else if program.symbols.get(argument.symbol).kind == SymbolKind::MachineParameter
                {
                    let provider = expressions_of_mut(program).retain_static_machine_application(
                        argument.symbol,
                        typed_trees::name::Identifier::generated(argument.display_name()),
                        &[],
                    );
                    let descriptor =
                        expressions_of_mut(program).static_machine_application_mut(provider);
                    descriptor.binding_declaration = argument.symbol;
                    descriptor.lexical_owner = source.lexical_owner;
                    program
                        .prepare_applied_machine_callable(
                            invocation,
                            ordinal,
                            provider,
                            argument.symbol,
                            parameter.symbol,
                            &signature,
                            &operands,
                            &lifetimes,
                        )
                        .map_err(failure)?;
                } else {
                    return Err(failure(
                        "captured Machine argument requires a complete selected provider tuple",
                    ));
                }
                selected.push(executable);
            }
            expressions_of_mut(program)
                .set_static_machine_application_selected_machines(invocation, &selected);
            program
                .prepare_selected_machine_contracts(invocation)
                .map_err(failure)?;
            program
                .closed_conformance_application_at_mut(site)
                .ok_or_else(|| failure("captured demand lost its source occurrence"))?
                .callable_application = invocation;
            Ok(())
        }
        fn failure(message: &str) -> Vec<Diagnostic> {
            vec![Diagnostic::error(message)]
        }
    }
    pub(crate) use conformance_demands::prepare as prepare_conformance_machine_demands;
    pub(crate) use conformance_demands::prepare_occurrence as prepare_conformance_occurrence;
    pub(crate) mod demands {
        use super::invocations::{MachineDemand, machine_demands};
        use crate::monomorphization::{Diagnostic, SymbolHandle, TypeReferenceHandle, TypedTrees};
        use crate::tables::expressions_of;
        use typed_trees::name::Identifier;
        use typed_trees::static_applications::{
            StaticMachineApplicationHandle, StaticMachineLifetimeBinding,
        };

        pub(crate) struct MemberDemands {
            requirements: Vec<MachineDemand>,
            operands: Vec<(SymbolHandle, TypeReferenceHandle)>,
            lifetimes: Vec<(Identifier, Identifier)>,
        }

        pub(crate) fn members(
            program: &TypedTrees,
            target: SymbolHandle,
            operands: &[(SymbolHandle, TypeReferenceHandle)],
        ) -> Result<Vec<MemberDemands>, Vec<Diagnostic>> {
            let Some(head) = program
                .machines()
                .iter()
                .find(|machine| program.machine_or_state_symbol(machine, target))
            else {
                return Ok(Vec::new());
            };
            let Some(family) = program
                .target_machine_families
                .iter()
                .find(|family| family.canonical == head.symbol)
            else {
                return Ok(Vec::new());
            };
            let head_parameters = program.machine_type_parameters(head);
            let mut demands = Vec::with_capacity(family.members.len());
            for member in &family.members {
                let machine = program
                    .machine_by_symbol(member.machine)
                    .ok_or_else(|| failure("target-family demand lost its retained declaration"))?;
                let parameters = program.machine_type_parameters(machine);
                if machine.lifetime_parameters.len() != head.lifetime_parameters.len() {
                    return Err(failure(
                        "target-family demand lost its lifetime binder correspondence",
                    ));
                }
                let mut member_operands = Vec::with_capacity(operands.len());
                for (parameter, actual) in operands {
                    let corresponding = head_parameters
                        .iter()
                        .position(|binding| binding.symbol == *parameter)
                        .and_then(|position| parameters.get(position))
                        .ok_or_else(|| {
                            failure("target-family demand lost its generic binder correspondence")
                        })?;
                    member_operands.push((corresponding.symbol, *actual));
                }
                demands.push(MemberDemands {
                    requirements: machine_demands(program, parameters),
                    operands: member_operands,
                    lifetimes: head
                        .lifetime_parameters
                        .iter()
                        .cloned()
                        .zip(machine.lifetime_parameters.iter().cloned())
                        .collect(),
                });
            }
            Ok(demands)
        }

        pub(crate) fn retain(
            program: &mut TypedTrees,
            invocation: StaticMachineApplicationHandle,
            ordinal: usize,
            selected: SymbolHandle,
            head: &MachineDemand,
            members: &[MemberDemands],
        ) -> Result<(), Vec<Diagnostic>> {
            if members.is_empty() {
                return Ok(());
            }
            let provider = program.applied_machine_provider(
                invocation,
                ordinal,
                selected,
                head.parameter,
                &head.signature,
            );
            let Some(provider) = provider else {
                if program.has_applied_machine_selection(invocation, ordinal, selected) {
                    return Err(failure(
                        "portable selection lost its determined provider tuple",
                    ));
                }
                return Ok(());
            };
            let caller_lifetimes = expressions_of(program)
                .static_machine_application_lifetimes(invocation)
                .to_vec();
            let provider_bindings = expressions_of(program)
                .static_machine_application_bindings(provider)
                .to_vec();
            let provider_lifetimes = expressions_of(program)
                .static_machine_application_lifetimes(provider)
                .to_vec();
            for member in members {
                let demand = member.requirements.get(ordinal).ok_or_else(|| {
                    failure("target-family demand lost its machine binder correspondence")
                })?;
                let mut lifetimes = Vec::with_capacity(caller_lifetimes.len());
                for binding in &caller_lifetimes {
                    let (_, parameter) = member
                        .lifetimes
                        .iter()
                        .find(|(head, _)| head == &binding.parameter)
                        .ok_or_else(|| {
                            failure("target-family demand lost its applied lifetime binder")
                        })?;
                    lifetimes.push(StaticMachineLifetimeBinding {
                        parameter: parameter.clone(),
                        argument: binding.argument.clone(),
                    });
                }
                if program.applied_machine_callable_matches(
                    invocation,
                    ordinal,
                    selected,
                    demand.parameter,
                    &provider_bindings,
                    &provider_lifetimes,
                    &demand.signature,
                    &member.operands,
                    &lifetimes,
                ) {
                    continue;
                }
                program
                    .prepare_applied_machine_callable(
                        invocation,
                        ordinal,
                        provider,
                        selected,
                        demand.parameter,
                        &demand.signature,
                        &member.operands,
                        &lifetimes,
                    )
                    .map_err(failure)?;
            }
            Ok(())
        }

        fn failure(message: &str) -> Vec<Diagnostic> {
            vec![Diagnostic::error(message)]
        }
    }
    pub(crate) mod installed_interrupts {
        use crate::monomorphization::{CallSite, Diagnostic, SymbolHandle, TypedTrees};
        use crate::tables::{expressions_of, expressions_of_mut};
        use typed_trees::expression::{InstalledInterruptOperationRequest, StaticMachineArgument};
        use typed_trees::static_applications::{
            StaticMachineApplicationHandle, StaticMachineBindingValue, StaticMachineLifetimeBinding,
        };
        use typed_trees::trait_definition::{AppliedTraitScope, RetainedTraitScope};

        pub(crate) fn prepare(program: &mut TypedTrees) -> Result<bool, Vec<Diagnostic>> {
            let before = (
                program.machines().len(),
                program.closed_requirement_applications().count(),
            );
            let mut sites = Vec::new();
            for machine in program.machines() {
                for state in program.machine_states(machine) {
                    for (handle, statement) in program
                        .statement_table
                        .iter_statements(state.statement_nodes)
                    {
                        if let Some(call) = statement.as_call()
                            && let Some(request) = &call.installed_interrupt_operation
                        {
                            sites.push((
                                CallSite::Statement(handle),
                                machine.symbol,
                                request.clone(),
                            ));
                        }
                        let mut expressions = Vec::new();
                        validation::collect_statement_expression_trees(
                            program,
                            statement,
                            &mut expressions,
                        );
                        for handle in expressions {
                            if let Some(call) = expressions_of(program).call_node(handle)
                                && let Some(request) = &call.installed_interrupt_operation
                                && !sites
                                    .iter()
                                    .any(|(site, _, _)| *site == CallSite::Expression(handle))
                            {
                                sites.push((
                                    CallSite::Expression(handle),
                                    machine.symbol,
                                    request.clone(),
                                ));
                            }
                        }
                    }
                }
            }
            for (site, owner, mut request) in sites {
                if !request.registrar_application.is_valid() {
                    let (application, target, receiving_scope) =
                        prepare_registrar(program, owner, &request)?;
                    request.registrar_application = application;
                    request.registrar_target = target;
                    request.registrar_receiving_scope = receiving_scope;
                }
                match site {
                    CallSite::Expression(handle) => {
                        let typed_trees::expression::ExpressionNode::Call(call) =
                            expressions_of_mut(program).expression_mut(handle)
                        else {
                            unreachable!("retained installation expression remains a call")
                        };
                        call.installed_interrupt_operation = Some(request);
                    }
                    CallSite::Statement(handle) => {
                        let typed_trees::statement::StatementNode::Call(call) =
                            program.statement_table.statement_mut(handle)
                        else {
                            unreachable!("retained installation statement remains a call")
                        };
                        call.installed_interrupt_operation = Some(request);
                    }
                }
            }
            Ok(before
                != (
                    program.machines().len(),
                    program.closed_requirement_applications().count(),
                ))
        }

        fn prepare_registrar(
            program: &mut TypedTrees,
            owner: SymbolHandle,
            request: &InstalledInterruptOperationRequest,
        ) -> Result<
            (
                StaticMachineApplicationHandle,
                SymbolHandle,
                RetainedTraitScope,
            ),
            Vec<Diagnostic>,
        > {
            let authored =
                expressions_of_mut(program).copy_static_argument_from_self(&request.registrar);
            let mut operand = expressions_of_mut(program).copy_static_argument_from_self(&authored);
            crate::monomorphization::normalize_const_arguments(
                program,
                owner,
                std::slice::from_mut(&mut operand),
            )
            .map_err(|reason| failure(&reason))?;
            let authored_member = leaf(program, &authored)?;
            let member = leaf(program, &operand)?;
            let requirement = member.symbol;
            let (declaring_trait, declared) = program
                .trait_machine_signature(requirement)
                .map(|(definition, signature)| (definition.symbol, signature.clone()))
                .ok_or_else(|| {
                    failure("installation registrar lost its exact requirement declaration")
                })?;
            let receiving_environment = if operand
                .application
                .as_ref()
                .is_some_and(|app| app.member.is_valid())
            {
                let mut qualifier = operand.clone();
                qualifier
                    .application
                    .as_mut()
                    .expect("qualified operand")
                    .member = Default::default();
                let reference = program
                    .static_operand_type_reference_in_scope(&qualifier, owner)
                    .map_err(failure)?;
                let scope = AppliedTraitScope::from_type_reference(program, reference, owner)
                    .filter(|scope| scope.declaration.symbol == request.registrar_trait)
                    .ok_or_else(|| {
                        failure("installation registrar has no complete selected owner application")
                    })?;
                scope.environment()
            } else {
                let selected = program
                    .trait_definition_by_symbol(request.registrar_trait)
                    .ok_or_else(|| failure("installation registrar lost its selected trait"))?;
                AppliedTraitScope::abstract_scope(selected).environment()
            };
            let environment = receiving_environment
                .scope(program)
                .ok_or_else(|| {
                    failure("installation registrar lost its selected application scope")
                })?
                .declaring_scope(program, declaring_trait)
                .ok_or_else(|| {
                    failure("installation registrar has no unique declaring application")
                })?
                .environment();
            let retained_receiving_scope =
                receiving_environment.retain(program).map_err(failure)?;
            let parameters = program.state_signature_type_parameters(&declared).to_vec();
            let source_arguments = authored_member
                .application
                .as_ref()
                .map_or(&[][..], |application| application.arguments.as_ref());
            let arguments = member
                .application
                .as_ref()
                .map_or(&[][..], |application| application.arguments.as_ref());
            let lifetimes = member.application.as_ref().map_or(&[][..], |application| {
                application.lifetime_arguments.as_ref()
            });
            let application = expressions_of_mut(program).retain_static_machine_application(
                requirement,
                declared.name.clone(),
                source_arguments,
            );
            let source = expressions_of_mut(program).static_machine_application_mut(application);
            source.lexical_owner = owner;
            source.binding_declaration = requirement;
            if parameters.len() != arguments.len()
                || declared.lifetime_parameters.len() != lifetimes.len()
            {
                return Ok((
                    application,
                    SymbolHandle::invalid(),
                    retained_receiving_scope,
                ));
            }
            let bindings = crate::monomorphization::requirement_applications::invocation_bindings(
                program,
                owner,
                &parameters,
                arguments,
            )?;
            expressions_of_mut(program)
                .set_static_machine_application_bindings(application, &bindings);
            let lifetime_bindings = declared
                .lifetime_parameters
                .iter()
                .cloned()
                .zip(lifetimes.iter().cloned())
                .map(|(parameter, argument)| StaticMachineLifetimeBinding {
                    parameter,
                    argument,
                })
                .collect::<Vec<_>>();
            expressions_of_mut(program).set_static_machine_application_lifetimes(
                application,
                requirement,
                &lifetime_bindings,
            );
            if super::static_operands::argument_is_open(program, &operand) {
                return Ok((
                    application,
                    SymbolHandle::invalid(),
                    retained_receiving_scope,
                ));
            }
            let receiving_scope = receiving_environment.scope(program).ok_or_else(|| {
                failure("installation registrar lost its selected application scope")
            })?;
            program
                .require_receiving_trait_operands(&receiving_scope)
                .map_err(failure)?;
            let scope = environment.scope(program).ok_or_else(|| {
                failure("installation registrar lost its owning application scope")
            })?;
            program
                .require_receiving_trait_operands(&scope)
                .map_err(failure)?;
            let trait_arguments = program
                .materialize_trait_application_arguments(&environment)
                .map_err(failure)?;
            let target = program
                .instantiate_requirement_application(
                    &environment,
                    requirement,
                    &trait_arguments,
                    application,
                )
                .map_err(failure)?;
            let signature = program
                .closed_requirement_application(target)
                .ok_or_else(|| failure("installation registrar lost its closed requirement"))?
                .signature
                .clone();
            let demands = super::invocations::machine_demands(
                program,
                program.requirement_invocation_contract_parameters(&signature),
            );
            let operands = bindings
                .iter()
                .filter_map(|binding| match binding.value {
                    StaticMachineBindingValue::Type(actual)
                    | StaticMachineBindingValue::Const(actual) => Some((binding.parameter, actual)),
                    _ => None,
                })
                .collect::<Vec<_>>();
            let mut selected = bindings
                .iter()
                .filter_map(|binding| match &binding.value {
                    StaticMachineBindingValue::Machine(argument) => Some(argument.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>();
            if selected.len() != demands.len() {
                return Err(failure(
                    "installation registrar changed its complete machine binder roster",
                ));
            }
            for (ordinal, (argument, demand)) in selected.iter_mut().zip(&demands).enumerate() {
                if let Some(entry) = super::providers::close(
                    program,
                    argument,
                    demand.parameter,
                    &demand.signature,
                    &operands,
                    application,
                    ordinal,
                )? {
                    argument.symbol = entry;
                    argument.application = None;
                }
            }
            expressions_of_mut(program)
                .set_static_machine_application_selected_machines(application, &selected);
            program
                .prepare_selected_machine_contracts(application)
                .map_err(failure)?;
            Ok((application, target, retained_receiving_scope))
        }

        fn leaf(
            program: &TypedTrees,
            root: &StaticMachineArgument,
        ) -> Result<StaticMachineArgument, Vec<Diagnostic>> {
            let mut operand = root;
            let mut selected_member = false;
            while let Some(application) = &operand.application
                && application.member.is_valid()
            {
                if selected_member {
                    return Err(failure(
                        "installation registrar has an additional applied owner; its complete qualified scope is not prepared",
                    ));
                }
                operand = expressions_of(program)
                    .static_member(application.member)
                    .ok_or_else(|| {
                        failure("installation registrar member lost its expression-table owner")
                    })?;
                selected_member = true;
            }
            Ok(operand.clone())
        }

        fn failure(reason: &str) -> Vec<Diagnostic> {
            vec![Diagnostic::error(reason)]
        }
    }
    pub(crate) mod invocations {
        use crate::monomorphization::{
            CallSite, ExpressionNode, StaticMachineArgument, SymbolHandle, TypeReferenceHandle,
            TypedTrees,
        };
        use crate::tables::expressions_of;
        use symbols::SymbolKind;
        use typed_trees::data::{TypeParameter, TypeParameterKind};
        use typed_trees::signature::StateSignature;
        use validation::collect_statement_expression_trees;
        use validation::parameter_expression_result_type_reference;
        pub(crate) mod operands {
            use super::{
                CallSite, StaticMachineArgument, SymbolHandle, TypeReferenceHandle, TypedTrees,
            };
            use crate::tables::expressions_of;
            use symbols::SymbolKind;
            use typed_trees::data::{TypeParameter, TypeParameterKind};

            pub(crate) fn derive(
                program: &mut TypedTrees,
                scope: &super::InvocationScope,
                site: CallSite,
                parameters: &[TypeParameter],
                selected: &[StaticMachineArgument],
                runtime: &[TypeReferenceHandle],
                arguments: &[typed_trees::expression::ExpressionHandle],
            ) -> Option<Vec<(SymbolHandle, TypeReferenceHandle)>> {
                let type_parameters = parameters
                    .iter()
                    .filter(|parameter| matches!(parameter.kind, TypeParameterKind::Type))
                    .map(|parameter| (parameter.symbol, parameter.name.as_str().to_owned()))
                    .collect::<Vec<_>>();
                let const_parameters = parameters
                    .iter()
                    .filter_map(|parameter| match parameter.kind {
                        TypeParameterKind::Const { type_reference } => Some((
                            parameter.symbol,
                            parameter.name.as_str().to_owned(),
                            type_reference,
                        )),
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                let selected_types = selected
                    .iter()
                    .filter_map(|argument| {
                        if argument.type_reference.is_valid() {
                            return Some(argument.type_reference);
                        }
                        if argument.symbol.is_valid()
                            && matches!(
                                program.symbols.get(argument.symbol).kind,
                                SymbolKind::BuiltinType
                                    | SymbolKind::Data
                                    | SymbolKind::TypeParameter
                            )
                            && !program
                                .data_type_parameter_by_symbol(argument.symbol)
                                .is_some_and(|parameter| {
                                    !matches!(parameter.kind, TypeParameterKind::Type)
                                })
                        {
                            return program
                                .type_reference_table
                                .find_named_type_reference(argument.symbol);
                        }
                        None
                    })
                    .collect::<Vec<_>>();
                if selected_types.len() > type_parameters.len() {
                    return None;
                }
                let mut types = type_parameters
                    .iter()
                    .zip(selected_types)
                    .map(|((symbol, _), actual)| (*symbol, actual))
                    .collect::<Vec<_>>();
                let selected_constants = selected
                    .iter()
                    .filter(|argument| {
                        argument.const_literal.is_some()
                            || program.static_const_argument_identity(argument).is_some()
                            || (argument.symbol.is_valid()
                                && program
                                    .data_type_parameter_by_symbol(argument.symbol)
                                    .is_some_and(|parameter| parameter.kind.is_const_or_value()))
                    })
                    .collect::<Vec<_>>();
                if selected_constants.len() > const_parameters.len() {
                    return None;
                }
                for ((symbol, _, _), argument) in const_parameters.iter().zip(selected_constants) {
                    let actual = program.static_operand_type_reference(argument).ok()?;
                    types.push((*symbol, actual));
                }
                let context = crate::monomorphization::BindingContext::new(
                    program,
                    &type_parameters,
                    &const_parameters,
                    0,
                );
                let mut inferred = crate::monomorphization::StaticProposals::default();
                for (required, argument) in runtime.iter().zip(arguments) {
                    if let Some(actual) = scope.result_type(program, *argument)
                        && (scope.has_open_parameters(program)
                            || !crate::monomorphization::selection::names_type_binder(
                                program, actual,
                            ))
                    {
                        crate::monomorphization::infer_static_bindings(
                            context,
                            *required,
                            actual,
                            &mut inferred,
                        );
                    }
                }
                for (_, ordinal, actual) in inferred.types {
                    let symbol = type_parameters[ordinal].0;
                    if !types.iter().any(|(parameter, _)| *parameter == symbol) {
                        types.push((symbol, actual));
                    }
                }
                for (_, ordinal, actual) in inferred.constants {
                    let symbol = const_parameters[ordinal].0;
                    if !types.iter().any(|(parameter, _)| *parameter == symbol) {
                        types.push((symbol, actual));
                    }
                }
                let application = site.authored_machine_application(program)?;
                if application.is_valid() {
                    for binding in
                        expressions_of(program).static_machine_application_bindings(application)
                    {
                        use typed_trees::static_applications::StaticMachineBindingValue;
                        let actual = match binding.value {
                            StaticMachineBindingValue::Type(actual)
                            | StaticMachineBindingValue::Const(actual) => actual,
                            _ => continue,
                        };
                        if let Some((_, previous)) = types
                            .iter_mut()
                            .find(|(parameter, _)| *parameter == binding.parameter)
                        {
                            *previous = actual;
                        } else if type_parameters
                            .iter()
                            .any(|(parameter, _)| *parameter == binding.parameter)
                            || const_parameters
                                .iter()
                                .any(|(parameter, _, _)| *parameter == binding.parameter)
                        {
                            types.push((binding.parameter, actual));
                        }
                    }
                }
                if type_parameters
                    .iter()
                    .any(|(parameter, _)| !types.iter().any(|(bound, _)| bound == parameter))
                    || const_parameters
                        .iter()
                        .any(|(parameter, _, _)| !types.iter().any(|(bound, _)| bound == parameter))
                {
                    return None;
                }
                Some(types)
            }
        }

        pub(crate) struct Invocation {
            pub site: CallSite,
            pub owner: SymbolHandle,
            pub target: SymbolHandle,
            pub machine_arguments: Vec<StaticMachineArgument>,
            pub requirements: Vec<MachineDemand>,
            pub types: Vec<(SymbolHandle, TypeReferenceHandle)>,
            pub use_site: InvocationUse,
        }

        #[derive(Clone, Copy, PartialEq, Eq)]
        pub(crate) enum InvocationUse {
            Executable,
            RetainedEvaluation,
        }

        pub(crate) enum InvocationScope {
            State {
                machine: SymbolHandle,
                state: SymbolHandle,
            },
            Declaration(SymbolHandle),
        }

        impl InvocationScope {
            fn owner(&self) -> SymbolHandle {
                match self {
                    Self::State { machine, .. } => *machine,
                    Self::Declaration(owner) => *owner,
                }
            }

            fn use_site(&self) -> InvocationUse {
                match self {
                    Self::State { .. } => InvocationUse::Executable,
                    Self::Declaration(_) => InvocationUse::RetainedEvaluation,
                }
            }

            pub(crate) fn result_type(
                &self,
                program: &dyn validation::CheckingInput,
                expression: typed_trees::expression::ExpressionHandle,
            ) -> Option<TypeReferenceHandle> {
                match self {
                    Self::State { machine, state } => {
                        let machine = program.machine_by_symbol(*machine)?;
                        let state = program
                            .machine_states(machine)
                            .iter()
                            .find(|candidate| candidate.symbol == *state)?;
                        validation::expression_result_type_reference(
                            program, machine, state, expression,
                        )
                    }
                    Self::Declaration(owner) => {
                        parameter_expression_result_type_reference(program, *owner, &[], expression)
                    }
                }
            }

            pub(crate) fn has_open_parameters(&self, program: &TypedTrees) -> bool {
                let mut owner = self.owner();
                while owner.is_valid() {
                    if let Some(machine) = program.machine_by_symbol(owner) {
                        return program.callable_body_has_open_parameters(machine);
                    }
                    owner = program.symbols.get(owner).parent;
                }
                false
            }
        }

        pub(crate) struct MachineDemand {
            pub parameter: SymbolHandle,
            pub signature: StateSignature,
        }

        pub(crate) fn machine_demands(
            program: &TypedTrees,
            parameters: &[TypeParameter],
        ) -> Vec<MachineDemand> {
            parameters
                .iter()
                .filter_map(|parameter| {
                    let TypeParameterKind::Machine { contract } = &parameter.kind else {
                        return None;
                    };
                    program
                        .machine_parameter_contract_view(contract)
                        .map(|view| MachineDemand {
                            parameter: parameter.symbol,
                            signature: view.signature().clone(),
                        })
                })
                .collect()
        }

        pub(crate) fn is_machine(program: &TypedTrees, symbol: SymbolHandle) -> bool {
            symbol.is_valid()
                && matches!(
                    program.symbols.get(symbol).kind,
                    SymbolKind::Machine | SymbolKind::State | SymbolKind::MachineParameter
                )
        }

        pub(crate) fn collect(
            program: &mut TypedTrees,
        ) -> Result<Vec<Invocation>, Vec<super::Diagnostic>> {
            let mut pending = Vec::new();
            let mut scopes = Vec::new();
            for machine in program.machines() {
                for state in program.machine_states(machine) {
                    let scope = scopes.len();
                    scopes.push(InvocationScope::State {
                        machine: machine.symbol,
                        state: state.symbol,
                    });
                    for (statement_handle, statement) in program
                        .statement_table
                        .iter_statements(state.statement_nodes)
                    {
                        if let Some(call) = statement.as_call() {
                            pending.push((
                                scope,
                                CallSite::Statement(statement_handle),
                                call.target_symbol,
                                call.machine_arguments.to_vec(),
                                program
                                    .statement_table
                                    .expression_handles(call.arguments)
                                    .to_vec(),
                            ));
                        }
                        let mut expressions = Vec::new();
                        collect_statement_expression_trees(program, statement, &mut expressions);
                        for expression in expressions {
                            let site = CallSite::Expression(expression);
                            let ExpressionNode::Call(call) =
                                expressions_of(program).expression(expression)
                            else {
                                continue;
                            };
                            pending.push((
                                scope,
                                site,
                                call.target_symbol,
                                call.machine_arguments.to_vec(),
                                expressions_of(program)
                                    .expression_handles(call.arguments)
                                    .to_vec(),
                            ));
                        }
                    }
                }
            }
            for declaration in program.const_declarations() {
                let scope = scopes.len();
                scopes.push(InvocationScope::Declaration(declaration.symbol));
                let mut expressions = Vec::new();
                for root in [
                    declaration.authored_initializer,
                    declaration.materialized_initializer,
                ] {
                    if expressions_of(program).expression_is_valid(root) {
                        validation::collect_expression_tree(program, root, &mut expressions);
                    }
                }
                for expression in expressions {
                    let Some(call) = expressions_of(program).call_node(expression) else {
                        continue;
                    };
                    pending.push((
                        scope,
                        CallSite::Expression(expression),
                        call.target_symbol,
                        call.machine_arguments.to_vec(),
                        expressions_of(program)
                            .expression_handles(call.arguments)
                            .to_vec(),
                    ));
                }
            }
            let mut invocations = Vec::new();
            let mut diagnostics = Vec::new();
            for (scope, site, target, selected, arguments) in pending {
                if invocations
                    .iter()
                    .any(|invocation: &Invocation| invocation.site == site)
                {
                    continue;
                }
                if let Some(invocation) = proposal(
                    program,
                    &scopes[scope],
                    site,
                    target,
                    &selected,
                    &arguments,
                    &mut diagnostics,
                ) {
                    invocations.push(invocation);
                }
            }
            if diagnostics.is_empty() {
                Ok(invocations)
            } else {
                Err(diagnostics)
            }
        }

        fn proposal(
            program: &mut TypedTrees,
            scope: &InvocationScope,
            site: CallSite,
            selected_target: SymbolHandle,
            selected: &[StaticMachineArgument],
            arguments: &[typed_trees::expression::ExpressionHandle],
            diagnostics: &mut Vec<super::Diagnostic>,
        ) -> Option<Invocation> {
            let application = site.authored_machine_application(program)?;
            let (target, mut selected, selected_machines) = if application.is_valid() {
                (
                    expressions_of(program)
                        .static_machine_application(application)
                        .registration_operation,
                    expressions_of(program)
                        .static_machine_application_arguments(application)
                        .to_vec(),
                    expressions_of(program)
                        .static_machine_application_selected_machines(application)
                        .to_vec(),
                )
            } else {
                (selected_target, selected.to_vec(), Vec::new())
            };
            let retained_owner = if application.is_valid() {
                expressions_of(program)
                    .static_machine_application(application)
                    .lexical_owner
            } else {
                SymbolHandle::invalid()
            };
            let owner = if retained_owner.is_valid() {
                retained_owner
            } else {
                scope.owner()
            };
            if let Err(message) = crate::monomorphization::body_rewriting::normalize_const_arguments(
                program,
                owner,
                &mut selected,
            ) {
                diagnostics.push(super::Diagnostic::error(message));
                return None;
            }
            if !selected
                .iter()
                .any(|argument| is_machine(program, argument.symbol))
            {
                return None;
            }
            let (parameters, runtime): (&[TypeParameter], Vec<TypeReferenceHandle>) =
                if let Some(machine) = program
                    .machines()
                    .iter()
                    .find(|machine| program.machine_or_state_symbol(machine, target))
                {
                    let entry = program
                        .machine_states(machine)
                        .iter()
                        .find(|state| state.symbol == target)
                        .or_else(|| {
                            (machine.symbol == target)
                                .then(|| program.machine_states(machine).first())
                                .flatten()
                        })?;
                    (
                        program.machine_type_parameters(machine),
                        program
                            .state_parameters(entry)
                            .iter()
                            .map(|parameter| parameter.type_reference)
                            .collect(),
                    )
                } else {
                    let signature = if let Some(application) =
                        program.closed_requirement_application(selected_target)
                    {
                        if application.template != target {
                            return None;
                        }
                        application.signature
                    } else {
                        program
                            .machine_parameter_signature(target)
                            .map(|(_, signature)| signature)
                            .or_else(|| {
                                program
                                    .traits()
                                    .iter()
                                    .flat_map(|definition| {
                                        program.trait_machine_signatures(definition)
                                    })
                                    .find(|signature| signature.symbol == target)
                            })?
                    };
                    (
                        program.requirement_invocation_contract_parameters(signature),
                        program
                            .state_signature_parameters(signature)
                            .iter()
                            .map(|parameter| parameter.type_reference)
                            .collect(),
                    )
                };
            let machine_arguments = if selected_machines.is_empty() {
                selected
                    .iter()
                    .filter(|argument| is_machine(program, argument.symbol))
                    .cloned()
                    .collect::<Vec<_>>()
            } else {
                selected_machines
            };
            let parameters = parameters.to_vec();
            let requirements = machine_demands(program, &parameters);
            if requirements.len() != machine_arguments.len() {
                return None;
            }
            let types = operands::derive(
                program,
                scope,
                site,
                &parameters,
                &selected,
                &runtime,
                arguments,
            )?;
            Some(Invocation {
                site,
                owner: scope.owner(),
                target,
                machine_arguments,
                requirements,
                types,
                use_site: scope.use_site(),
            })
        }
    }
    pub(crate) use installed_interrupts::prepare as prepare_installed_interrupts;
    pub(crate) mod providers {
        use crate::monomorphization::application_lifetimes as lifetime_arguments;
        use crate::tables::expressions_of;

        use crate::monomorphization::{
            Diagnostic, StaticMachineArgument, SymbolHandle, TypeReferenceHandle, TypedTrees,
            candidate, saved_calls,
        };
        use typed_trees::signature::StateSignature;
        use typed_trees::static_applications::StaticMachineApplicationHandle;
        use validation::contract_identities::{
            accepted_template_commitment, normalized_machine_identity,
        };

        fn authored_provider(
            program: &TypedTrees,
            machine_index: usize,
            selected: SymbolHandle,
        ) -> Option<(usize, SymbolHandle, StaticMachineApplicationHandle)> {
            let instance = &program.machines()[machine_index];
            let specialization = program
                .machine_specializations
                .iter()
                .find(|specialization| {
                    specialization.instance == instance.symbol
                        && specialization.instance != specialization.template
                })?;
            let offset = program
                .machine_states(instance)
                .iter()
                .position(|state| state.symbol == selected)
                .unwrap_or(0);
            let template_index = program.machine_position_by_symbol(specialization.template)?;
            let entry = program
                .machine_states(&program.machines()[template_index])
                .get(offset)?
                .symbol;
            Some((template_index, entry, specialization.binding_application))
        }

        pub(crate) fn close(
            program: &mut TypedTrees,
            selection: &StaticMachineArgument,
            demand: SymbolHandle,
            expected: &StateSignature,
            caller_types: &[(SymbolHandle, TypeReferenceHandle)],
            application: typed_trees::static_applications::StaticMachineApplicationHandle,
            ordinal: usize,
        ) -> Result<Option<SymbolHandle>, Vec<Diagnostic>> {
            let selected = selection.symbol;
            let mut source_selection = expressions_of(program)
                .static_machine_application_arguments(application)
                .iter()
                .filter(|argument| super::invocations::is_machine(program, argument.symbol))
                .nth(ordinal)
                .cloned()
                .unwrap_or_else(|| selection.clone());
            let owner = expressions_of(program)
                .static_machine_application(application)
                .lexical_owner;
            crate::monomorphization::body_rewriting::normalize_const_arguments(
                program,
                owner,
                std::slice::from_mut(&mut source_selection),
            )
            .map_err(|message| vec![Diagnostic::error(message)])?;
            let mut source_selected = source_selection.symbol;
            if !program.state_signature_type_parameters(expected).is_empty() {
                return Ok(None);
            }
            let Some(mut machine_index) = program
                .machines()
                .iter()
                .position(|machine| program.machine_or_state_symbol(machine, source_selected))
            else {
                return Ok(None);
            };
            let mut provider = candidate::from_machine(program, machine_index);
            if source_selection.application.is_none()
                && let Some((template_index, entry, tuple)) =
                    authored_provider(program, machine_index, source_selected)
            {
                let mut authored = candidate::from_machine(program, template_index);
                if crate::monomorphization::applications::bind_recorded_tuple(
                    program,
                    &mut authored,
                    tuple,
                ) {
                    machine_index = template_index;
                    source_selected = entry;
                    provider = authored;
                }
            }
            macro_rules! retain_selected {
                ($selected:expr $(,)?) => {
                    super::applications::retain_view(
                        program,
                        &provider,
                        application,
                        ordinal,
                        $selected,
                        demand,
                        expected,
                        caller_types,
                    )
                };
            }
            provider.binding_application = application;
            let state = program
                .machine_states(&program.machines()[machine_index])
                .iter()
                .find(|state| state.symbol == source_selected)
                .or_else(|| {
                    (program.machines()[machine_index].symbol == source_selected)
                        .then(|| {
                            program
                                .machine_states(&program.machines()[machine_index])
                                .first()
                        })
                        .flatten()
                })
                .cloned();
            let Some(state) = state else {
                return Ok(None);
            };
            let source_entry = state.symbol;
            let lifetimes =
                lifetime_arguments::bind(program, source_selected, &source_selection, application)?;
            if let Some(explicit) = &source_selection.application {
                crate::monomorphization::explicit_arguments::bind(
                    program,
                    &mut provider,
                    &state,
                    source_entry,
                    &explicit.arguments,
                )?;
            }
            let actual_types = program
                .state_parameters(&state)
                .iter()
                .map(|parameter| parameter.type_reference)
                .collect::<Vec<_>>();
            let expected_types = program
                .state_signature_parameters(expected)
                .iter()
                .map(|parameter| parameter.type_reference)
                .collect::<Vec<_>>();
            if actual_types.len() != expected_types.len() {
                return Ok(None);
            }
            let supplied_types = provider
                .type_bindings
                .iter()
                .map(Option::is_some)
                .collect::<Vec<_>>();
            let supplied_constants = provider
                .const_bindings
                .iter()
                .map(Option::is_some)
                .collect::<Vec<_>>();
            let mut proposals = crate::monomorphization::StaticProposals::default();
            for (actual, required) in actual_types
                .into_iter()
                .chain([state.return_type])
                .zip(expected_types.into_iter().chain([expected.return_type]))
            {
                let required = program
                    .apply_callable_type(required, caller_types, &[], expected.symbol)
                    .map_err(|message| vec![Diagnostic::error(message)])?;
                let context = crate::monomorphization::BindingContext::new(
                    program,
                    &provider.template.type_parameters,
                    &provider.template.const_parameters,
                    0,
                );
                crate::monomorphization::infer_static_bindings(
                    context,
                    actual,
                    required,
                    &mut proposals,
                );
            }
            for (_, ordinal, actual) in proposals.types {
                match provider.type_bindings[ordinal] {
                    None => provider.type_bindings[ordinal] = Some(actual),
                    Some(previous)
                        if !supplied_types[ordinal]
                            && program.normalized_type_identity_text(previous)
                                != program.normalized_type_identity_text(actual) =>
                    {
                        provider.conflicted = true
                    }
                    Some(_) => {}
                }
            }
            for (_, ordinal, actual) in proposals.constants {
                match provider.const_bindings[ordinal] {
                    None => provider.const_bindings[ordinal] = Some(actual),
                    Some(previous)
                        if !supplied_constants[ordinal]
                            && !crate::monomorphization::selection::same_const_identity(
                                program, previous, actual,
                            ) =>
                    {
                        provider.conflicted = true
                    }
                    Some(_) => {}
                }
            }
            if provider.conflicted
                || provider.type_bindings.iter().any(Option::is_none)
                || provider.const_bindings.iter().any(Option::is_none)
                || provider.machine_bindings.iter().any(Option::is_none)
                || provider.evidence_bindings.iter().any(Option::is_none)
            {
                return Ok(None);
            }
            let bindings =
                crate::monomorphization::applications::candidate_bindings(program, &provider);
            if program.applied_machine_callable_matches(
                application,
                ordinal,
                selected,
                demand,
                &bindings,
                &lifetimes,
                expected,
                caller_types,
                expressions_of(program).static_machine_application_lifetimes(application),
            ) {
                return Ok(None);
            }
            if !program.machines()[machine_index].has_specialization_parameters() {
                provider.binding_application = super::applications::retain_provider(
                    program,
                    &provider,
                    &state,
                    application,
                    &lifetimes,
                );
                retain_selected!(selected)?;
                return Ok(Some(source_entry));
            }
            if crate::monomorphization::provider_tuple_closure::operands_are_open(
                program, &provider,
            ) {
                provider.binding_application = super::applications::retain_provider(
                    program,
                    &provider,
                    &state,
                    application,
                    &lifetimes,
                );
                retain_selected!(selected)?;
                return Ok(Some(selected));
            }
            provider.binding_application = super::applications::retain_provider(
                program,
                &provider,
                &state,
                application,
                &lifetimes,
            );
            let existing = saved_calls::candidate_instance(program, program, &provider)
                .map(|instance| instance.instance);
            if let Some(existing) = existing {
                let state_ordinal = provider
                    .template
                    .state_symbols
                    .iter()
                    .position(|symbol| *symbol == source_entry);
                let entry = program
                    .machine_by_symbol(existing)
                    .and_then(|machine| {
                        state_ordinal
                            .and_then(|ordinal| program.machine_states(machine).get(ordinal))
                    })
                    .map(|state| state.symbol);
                if let Some(entry) = entry {
                    retain_selected!(entry)?;
                }
                return Ok(entry);
            }
            let Some(identity) =
                normalized_machine_identity(program, &program.machines()[machine_index])
            else {
                return Err(vec![Diagnostic::error(
                    "contextual machine application has no normalized declaration identity",
                )]);
            };
            let commitment = accepted_template_commitment(program, machine_index);
            let instance_ordinal = program
                .machine_specializations
                .iter()
                .filter(|instance| instance.template == provider.template.template_symbol)
                .count();
            let states = crate::monomorphization::body_cloning::clone_specialized_machine(
                None,
                program,
                &provider,
                instance_ordinal,
                identity,
                commitment,
            )?;
            let entry = states
                .into_iter()
                .find(|(original, _)| *original == source_entry)
                .map(|(_, concrete)| concrete);
            if let Some(entry) = entry {
                retain_selected!(entry)?;
            }
            Ok(entry)
        }
    }
    pub(crate) mod static_operands {
        use crate::monomorphization::{SymbolHandle, TypeReferenceHandle, TypedTrees};
        use symbols::SymbolKind;
        use typed_trees::expression::{ExpressionHandle, StaticMachineArgument};
        use typed_trees::references::{
            TypedReferences, visit_static_argument, visit_type_reference,
        };
        use typed_trees::types::{FixedArrayLength, TypeReferenceNode};

        #[derive(Default)]
        struct OperandReferences {
            symbols: Vec<SymbolHandle>,
            types: Vec<TypeReferenceHandle>,
            expressions: Vec<ExpressionHandle>,
        }

        impl Extend<SymbolHandle> for OperandReferences {
            fn extend<References: IntoIterator<Item = SymbolHandle>>(
                &mut self,
                symbols: References,
            ) {
                self.symbols.extend(symbols);
            }
        }

        impl TypedReferences for OperandReferences {
            fn type_reference(&mut self, reference: TypeReferenceHandle) -> bool {
                if self.types.contains(&reference) {
                    return false;
                }
                self.types.push(reference);
                true
            }

            fn expression(&mut self, expression: ExpressionHandle) -> bool {
                if self.expressions.contains(&expression) {
                    return false;
                }
                self.expressions.push(expression);
                true
            }
        }

        fn collect(program: &TypedTrees, reference: TypeReferenceHandle) -> OperandReferences {
            let mut references = OperandReferences::default();
            visit_type_reference(program, &mut references, reference);
            follow_data_applications(program, &mut references);
            references
        }

        fn follow_data_applications(program: &TypedTrees, references: &mut OperandReferences) {
            let mut symbol_ordinal = 0usize;
            while symbol_ordinal < references.symbols.len() {
                let symbol = references.symbols[symbol_ordinal];
                symbol_ordinal += 1;
                if let Some(application) = program
                    .data_definition_by_symbol(symbol)
                    .and_then(|definition| definition.generic_instance)
                {
                    visit_type_reference(program, references, application);
                }
            }
        }

        pub(crate) fn names_any(
            program: &TypedTrees,
            reference: TypeReferenceHandle,
            symbols: &[SymbolHandle],
        ) -> bool {
            collect(program, reference)
                .symbols
                .iter()
                .any(|symbol| symbols.contains(symbol))
        }

        pub(crate) fn is_open(program: &TypedTrees, reference: TypeReferenceHandle) -> bool {
            let references = collect(program, reference);
            references_are_open(program, &references)
        }

        pub(crate) fn argument_is_open(
            program: &TypedTrees,
            argument: &StaticMachineArgument,
        ) -> bool {
            let mut references = OperandReferences::default();
            visit_static_argument(program, &mut references, argument);
            follow_data_applications(program, &mut references);
            references_are_open(program, &references)
        }

        fn references_are_open(program: &TypedTrees, references: &OperandReferences) -> bool {
            references.symbols.iter().any(|symbol| {
                symbol.is_valid()
                    && matches!(
                        program.symbols.get(*symbol).kind,
                        SymbolKind::TypeParameter
                            | SymbolKind::MachineParameter
                            | SymbolKind::ConformanceParameter
                            | SymbolKind::Parameter
                            | SymbolKind::Local
                    )
            }) || references
                .expressions
                .iter()
                .any(|expression| program.pending_const_range_endpoints.contains(expression))
                || references.types.iter().any(|reference| {
                    matches!(
                        program.type_reference_table.type_reference(*reference),
                        TypeReferenceNode::FixedArray {
                            length: FixedArrayLength::ConstCall { .. },
                            ..
                        }
                    )
                })
        }
    }

    pub(crate) use static_operands::{argument_is_open, is_open as operand_is_open, names_any};

    use crate::monomorphization::{
        CallSite, Diagnostic, ExpressionNode, StatementNode, SymbolHandle, TypedTrees,
    };
    use crate::tables::{expressions_of, expressions_of_mut};

    pub(crate) fn close(program: &mut TypedTrees) -> Result<Vec<SymbolHandle>, Vec<Diagnostic>> {
        crate::monomorphization::requirement_applications::prepare_invocation_bindings(program)?;
        let invocations = invocations::collect(program)?;
        let mut changed = Vec::new();
        for invocation in invocations {
            let member_demands = demands::members(program, invocation.target, &invocation.types)?;
            let application = crate::monomorphization::applications::retain_authored_call(
                program,
                invocation.site,
                invocation.owner,
            );
            let declaration = program
                .machine_parameter_signature(invocation.target)
                .map(|_| invocation.target)
                .or_else(|| {
                    program
                        .trait_machine_signature(invocation.target)
                        .map(|_| invocation.target)
                })
                .unwrap_or_else(|| {
                    program
                        .symbols
                        .get(invocation.requirements[0].parameter)
                        .parent
                });
            let retained = expressions_of_mut(program).static_machine_application_mut(application);
            if !retained.binding_declaration.is_valid() {
                retained.binding_declaration = declaration;
            }
            if retained.binding_declaration == declaration {
                let mut bindings = expressions_of(program)
                    .static_machine_application_bindings(application)
                    .to_vec();
                for (parameter, actual) in &invocation.types {
                    let value = if program
                        .data_type_parameter_by_symbol(*parameter)
                        .is_some_and(|parameter| parameter.kind.is_const_or_value())
                    {
                        typed_trees::static_applications::StaticMachineBindingValue::Const(*actual)
                    } else {
                        typed_trees::static_applications::StaticMachineBindingValue::Type(*actual)
                    };
                    if let Some(binding) = bindings
                        .iter_mut()
                        .find(|binding| binding.parameter == *parameter)
                    {
                        binding.value = value;
                    } else {
                        bindings.push(typed_trees::static_applications::StaticMachineBinding {
                            parameter: *parameter,
                            value,
                        });
                    }
                }
                expressions_of_mut(program)
                    .set_static_machine_application_bindings(application, &bindings);
            }
            let mut selected = invocation.machine_arguments.clone();
            let mut selection_changed = false;
            for (ordinal, (argument, expected)) in selected
                .iter_mut()
                .zip(&invocation.requirements)
                .enumerate()
            {
                let closed_entry = providers::close(
                    program,
                    argument,
                    expected.parameter,
                    &expected.signature,
                    &invocation.types,
                    application,
                    ordinal,
                )?;
                demands::retain(
                    program,
                    application,
                    ordinal,
                    closed_entry.unwrap_or(argument.symbol),
                    expected,
                    &member_demands,
                )?;
                let Some(entry) = closed_entry else {
                    continue;
                };
                argument.symbol = entry;
                let region_only_selection = program.machines().iter().any(|machine| {
                    !machine.has_specialization_parameters()
                        && program
                            .machine_states(machine)
                            .iter()
                            .any(|state| state.symbol == entry)
                });
                if region_only_selection
                    || program
                        .machine_specializations
                        .iter()
                        .any(|specialization| {
                            program
                                .machine_by_symbol(specialization.instance)
                                .is_some_and(|machine| {
                                    program
                                        .machine_states(machine)
                                        .iter()
                                        .any(|state| state.symbol == entry)
                                })
                        })
                {
                    argument.application = None;
                }
                selection_changed = true;
            }
            if selection_changed {
                expressions_of_mut(program)
                    .set_static_machine_application_selected_machines(application, &selected);
            }
            program
                .prepare_selected_machine_contracts(application)
                .map_err(|message| vec![Diagnostic::error(message)])?;
            if invocation.use_site == invocations::InvocationUse::RetainedEvaluation {
                if selection_changed {
                    changed.push(invocation.owner);
                }
                continue;
            }
            let authored = match invocation.site {
                CallSite::Expression(handle) => {
                    let Some(call) = expressions_of(program).call_node(handle) else {
                        continue;
                    };
                    call.machine_arguments.clone()
                }
                CallSite::Statement(handle) => {
                    let Some(call) = program.statement_table.statement(handle).as_call() else {
                        continue;
                    };
                    call.machine_arguments.clone()
                }
            };
            let mut machine_ordinal = 0usize;
            let mut arguments = authored.clone();
            for argument in &mut arguments {
                if invocations::is_machine(program, argument.symbol) {
                    *argument = selected[machine_ordinal].clone();
                    machine_ordinal += 1;
                }
            }
            if !selection_changed && arguments == authored {
                continue;
            }
            match invocation.site {
                CallSite::Expression(handle) => {
                    if let ExpressionNode::Call(call) =
                        expressions_of_mut(program).expression_mut(handle)
                    {
                        call.authored_machine_application = application;
                        call.machine_arguments = arguments;
                    }
                }
                CallSite::Statement(handle) => {
                    if let StatementNode::Call(call) = program.statement_table.statement_mut(handle)
                    {
                        call.authored_machine_application = application;
                        call.machine_arguments = arguments;
                    }
                }
            }
            changed.push(invocation.owner);
        }
        Ok(changed)
    }
}

pub(crate) mod requirement_applications {
    use crate::monomorphization::failed;
    use crate::tables::expressions_of_mut;
    use diagnostics::Diagnostic;
    use symbols::SymbolHandle;
    use typed_trees::data::TypeParameterKind;
    pub(crate) mod calls {
        use crate::tables::{expressions_of, expressions_of_mut};
        use diagnostics::Diagnostic;
        use symbols::SymbolHandle;
        use typed_trees::data::TypeParameterKind;
        use typed_trees::expression::ExpressionNode;
        use typed_trees::statement::StatementNode;
        use typed_trees::{
            TypedTrees, expression::StaticMachineArgument, references::StaticArgumentCall,
            trait_definition::AppliedTraitScope, types::TypeReferenceHandle,
        };
        use validation::expression_traversal::collect_expression_tree;
        use validation::expression_type_reference_in_state;

        use crate::monomorphization::failed;

        pub(crate) mod invocation_binding_views {
            use crate::tables::expressions_of;
            use diagnostics::Diagnostic;
            use symbols::SymbolHandle;
            use typed_trees::{
                TypedTrees,
                expression::StaticMachineArgument,
                references::StaticArgumentCall,
                static_applications::{StaticMachineBinding, StaticMachineBindingValue},
            };

            use super::super::selected_applications;

            pub(crate) fn nominal_source<'program>(
                program: &'program TypedTrees,
                owner: SymbolHandle,
                binder: SymbolHandle,
                target: SymbolHandle,
            ) -> Result<
                Option<(
                    SymbolHandle,
                    typed_trees::trait_definition::AppliedTraitScope<'program>,
                )>,
                Vec<Diagnostic>,
            > {
                let Some(typed_trees::data::MachineParameterContractView::Nominal {
                    trait_definition,
                    requirement,
                }) = program.retained_static_machine_contract(binder)
                else {
                    return Ok(None);
                };
                let Some(provider) = program.machine_holding_state(target) else {
                    return Ok(None);
                };
                let authored = program.lifetime_declaration_owner(provider.symbol);
                let Some(entry) = program
                    .machine_by_symbol(authored)
                    .and_then(|machine| program.machine_states(machine).first())
                else {
                    return Ok(None);
                };
                if !program
                    .static_machine_binding_selects_target(owner, binder, entry.symbol)
                    .map_err(|reason| vec![Diagnostic::error(reason)])?
                    || !program.instantiated_machine_state_matches(
                        provider.symbol,
                        authored,
                        entry.symbol,
                        target,
                    )
                {
                    return Ok(None);
                }
                let template = program
                    .closed_requirement_application(requirement.symbol)
                    .map_or(requirement.symbol, |closed| closed.template);
                program
                    .machine_conformance_requirement_scope(
                        provider.symbol,
                        trait_definition.symbol,
                        template,
                    )
                    .map(|scope| scope.map(|scope| (template, scope)))
                    .map_err(|reason| vec![Diagnostic::error(reason)])
            }

            pub(crate) fn public_bindings(
                program: &mut TypedTrees,
                owner: SymbolHandle,
                call: StaticArgumentCall<'_>,
            ) -> Result<Option<Vec<StaticMachineBinding>>, Vec<Diagnostic>> {
                let invocation = call.application();
                let dispatch = match call {
                    StaticArgumentCall::Expression(_, call) => {
                        call.static_requirement_dispatch.as_ref()
                    }
                    StaticArgumentCall::Statement(call) => {
                        call.static_requirement_dispatch.as_ref()
                    }
                };
                let Some(dispatch) = dispatch else {
                    return Ok(Some(
                        expressions_of(program)
                            .static_machine_application_bindings(invocation)
                            .to_vec(),
                    ));
                };
                if program
                    .closed_requirement_application(dispatch.contract_symbol())
                    .is_some()
                {
                    return Ok(Some(Vec::new()));
                }
                let Some((_, signature)) = program.trait_machine_signature(dispatch.requirement)
                else {
                    return Ok(None);
                };
                let parameters = program.state_signature_type_parameters(signature).to_vec();
                let Some(mut arguments) = complete_arguments(program, owner, call) else {
                    return Ok(None);
                };
                crate::monomorphization::body_rewriting::normalize_const_arguments(
                    program,
                    owner,
                    &mut arguments,
                )
                .map_err(|message| vec![Diagnostic::error(message)])?;
                super::super::invocation_bindings(program, owner, &parameters, &arguments).map(Some)
            }

            pub(crate) fn complete_arguments(
                program: &TypedTrees,
                owner: SymbolHandle,
                call: StaticArgumentCall<'_>,
            ) -> Option<Vec<StaticMachineArgument>> {
                let (target, dispatch) = match call {
                    StaticArgumentCall::Expression(_, call) => (
                        call.target_symbol,
                        call.static_requirement_dispatch.as_ref(),
                    ),
                    StaticArgumentCall::Statement(call) => (
                        call.target_symbol,
                        call.static_requirement_dispatch.as_ref(),
                    ),
                };
                let requirement = dispatch.map_or_else(
                    || call.registration_operation(program),
                    |dispatch| dispatch.requirement,
                );
                let (_, signature) = program.trait_machine_signature(requirement)?;
                let parameters = program.state_signature_type_parameters(signature);
                let arguments = call.arguments(program);
                if arguments.len() == parameters.len() {
                    return Some(arguments.to_vec());
                }
                if arguments.len() > parameters.len() || !call.application().is_valid() {
                    return None;
                }
                let invocation = call.application();
                let source = expressions_of(program).static_machine_application(invocation);
                if source.lexical_owner != owner {
                    return None;
                }
                let bindings =
                    expressions_of(program).static_machine_application_bindings(invocation);
                if dispatch.is_none() {
                    if source.binding_declaration != requirement
                        || bindings.len() != parameters.len()
                    {
                        return None;
                    }
                    return parameters
                        .iter()
                        .enumerate()
                        .map(|(ordinal, parameter)| {
                            let mut selected = bindings
                                .iter()
                                .filter(|binding| binding.parameter == parameter.symbol);
                            let binding = selected.next()?;
                            if selected.next().is_some() {
                                return None;
                            }
                            match &binding.value {
                                StaticMachineBindingValue::Type(reference)
                                | StaticMachineBindingValue::Const(reference) => {
                                    program.static_operand_from_type_reference(*reference).ok()
                                }
                                StaticMachineBindingValue::Machine(argument)
                                | StaticMachineBindingValue::Conformance(argument) => {
                                    Some(argument.clone())
                                }
                                StaticMachineBindingValue::RuntimeValue {
                                    subject,
                                    expression,
                                    ..
                                } => {
                                    runtime_argument(program, call, ordinal, *subject, *expression)
                                }
                            }
                        })
                        .collect();
                }
                let dispatch = dispatch?;
                let applications = selected_applications(program, owner, dispatch);
                let [application] = applications.as_slice() else {
                    return None;
                };
                if program.realization_entry_template(target, application)?
                    != (dispatch.realization_machine, dispatch.realization_state)
                {
                    return None;
                }
                let declaration = source.binding_declaration;
                let machine = program.machine_by_symbol(declaration)?;
                let private_parameters = program.requirement_callable_parameters(
                    signature,
                    &program.machine_callable_parameters(machine),
                );
                if private_parameters.len() != parameters.len()
                    || bindings.len() != parameters.len()
                {
                    return None;
                }
                let mut original = declaration;
                let mut visited = Vec::new();
                while original != dispatch.realization_machine {
                    if visited.contains(&original) {
                        return None;
                    }
                    visited.push(original);
                    let template = program.executable_application_template(original).ok()??;
                    original = template;
                }
                let mut complete = Vec::with_capacity(parameters.len());
                for (ordinal, private) in private_parameters.into_iter().enumerate() {
                    let mut selected = bindings
                        .iter()
                        .filter(|binding| binding.parameter == private.symbol);
                    let binding = selected.next()?;
                    if selected.next().is_some() {
                        return None;
                    }
                    complete.push(match &binding.value {
                        StaticMachineBindingValue::Type(reference)
                        | StaticMachineBindingValue::Const(reference) => program
                            .static_operand_from_type_reference(*reference)
                            .ok()?,
                        StaticMachineBindingValue::Machine(argument)
                        | StaticMachineBindingValue::Conformance(argument) => argument.clone(),
                        StaticMachineBindingValue::RuntimeValue {
                            subject,
                            expression,
                            ..
                        } => runtime_argument(program, call, ordinal, *subject, *expression)?,
                    });
                }
                Some(complete)
            }

            fn runtime_argument(
                program: &TypedTrees,
                call: StaticArgumentCall<'_>,
                ordinal: usize,
                subject: SymbolHandle,
                expression: typed_trees::expression::ExpressionHandle,
            ) -> Option<StaticMachineArgument> {
                if subject.is_valid() && !expression.is_valid() {
                    return Some(StaticMachineArgument {
                        symbol: subject,
                        path: Box::from([program.symbols.name(subject).into()]),
                        ..Default::default()
                    });
                }
                if !expression.is_valid() {
                    return None;
                }
                let live = match call {
                    StaticArgumentCall::Expression(_, call) => call.machine_arguments.get(ordinal),
                    StaticArgumentCall::Statement(call) => call.machine_arguments.get(ordinal),
                }?;
                (crate::monomorphization::const_arguments::runtime_subject_expression(
                    program, live,
                ) == Some(expression))
                .then(|| live.clone())
            }
        }

        #[derive(Clone, PartialEq, Eq)]
        pub(crate) struct InvocationTuple {
            pub owner: SymbolHandle,
            pub lifetime_declaration: SymbolHandle,
            pub arguments: Vec<StaticMachineArgument>,
            pub lifetimes: Vec<typed_trees::static_applications::StaticMachineLifetimeBinding>,
        }

        pub(crate) struct PreparedInvocation {
            pub(crate) site: crate::monomorphization::CallSite,
            pub(crate) owner: SymbolHandle,
            pub(crate) state: SymbolHandle,
            pub(crate) target: SymbolHandle,
            pub(crate) tuple: InvocationTuple,
        }

        fn occurrence(
            program: &TypedTrees,
            machine: &typed_trees::machine::Machine,
            state: &typed_trees::state::State,
            position: usize,
            call: StaticArgumentCall<'_>,
        ) -> InvocationTuple {
            let owner = machine.symbol;
            let application = call.application();
            let arguments = invocation_binding_views::complete_arguments(program, owner, call)
                .or_else(|| inferred_method_arguments(program, machine, state, position, call))
                .unwrap_or_else(|| call.arguments(program).to_vec());
            if application.is_valid() {
                InvocationTuple {
                    owner: expressions_of(program)
                        .static_machine_application(application)
                        .lexical_owner,
                    lifetime_declaration: program
                        .expression_table
                        .static_machine_application(application)
                        .lifetime_binding_declaration,
                    arguments,
                    lifetimes: expressions_of(program)
                        .static_machine_application_lifetimes(application)
                        .to_vec(),
                }
            } else {
                InvocationTuple {
                    owner,
                    lifetime_declaration: SymbolHandle::invalid(),
                    arguments,
                    lifetimes: Vec::new(),
                }
            }
        }

        fn inferred_method_arguments(
            program: &TypedTrees,
            machine: &typed_trees::machine::Machine,
            state: &typed_trees::state::State,
            position: usize,
            call: StaticArgumentCall<'_>,
        ) -> Option<Vec<StaticMachineArgument>> {
            let (target, runtime_arguments, application) = match call {
                StaticArgumentCall::Expression(_, call) => (
                    call.target_symbol,
                    expressions_of(program).expression_handles(call.arguments),
                    call.requirement_application,
                ),
                StaticArgumentCall::Statement(call) => (
                    call.target_symbol,
                    program.statement_table.expression_handles(call.arguments),
                    TypeReferenceHandle::invalid(),
                ),
            };
            let receiver = || receiver_scope(program, machine, state, position, call);
            let (template, scope) =
                source_scope(program, machine.symbol, target, receiver, application).ok()??;
            let (_, declared_signature) = program.trait_machine_signature(template)?;
            let signature = program
                .applied_callable_signature(&scope, template)
                .or_else(|| {
                    (scope.substitutions.is_empty()
                        && scope.declaration.lifetime_parameters.is_empty())
                    .then_some(declared_signature)
                })?;
            let parameters = program.state_signature_type_parameters(signature);
            let explicit = call.arguments(program);
            if explicit.len() >= parameters.len() {
                return None;
            }
            let mut bindings = Vec::new();
            for (parameter, argument) in parameters.iter().zip(explicit) {
                if !matches!(
                    parameter.kind,
                    TypeParameterKind::Type | TypeParameterKind::Const { .. }
                ) {
                    continue;
                }
                let reference = if argument.type_reference.is_valid() {
                    Some(argument.type_reference)
                } else if argument.symbol.is_valid()
                    && argument.application.is_none()
                    && argument.const_literal.is_none()
                    && argument.evidence_projection.is_none()
                {
                    program
                        .type_reference_table
                        .find_named_type_reference(argument.symbol)
                } else {
                    None
                };
                if let Some(reference) = reference {
                    bindings.push((parameter.symbol, reference));
                }
            }
            for (formal, actual) in program
                .state_signature_parameters(signature)
                .iter()
                .zip(runtime_arguments)
            {
                let Some(actual) =
                    validation::expression_result_type_reference(program, machine, state, *actual)
                else {
                    continue;
                };
                validation::infer_call_type_arguments(
                    program,
                    parameters,
                    program.machine_type_parameters(machine),
                    formal.type_reference,
                    actual,
                    &mut bindings,
                );
            }
            let mut complete = explicit.to_vec();
            for parameter in &parameters[explicit.len()..] {
                if !matches!(
                    parameter.kind,
                    TypeParameterKind::Type | TypeParameterKind::Const { .. }
                ) {
                    return None;
                }
                let mut matches = bindings
                    .iter()
                    .filter(|(symbol, _)| *symbol == parameter.symbol);
                let (_, reference) = matches.next()?;
                if matches.next().is_some() {
                    return None;
                }
                complete.push(
                    program
                        .static_operand_from_type_reference(*reference)
                        .ok()?,
                );
            }
            Some(complete)
        }

        fn receiver_scope<'p>(
            program: &'p TypedTrees,
            machine: &typed_trees::machine::Machine,
            state: &typed_trees::state::State,
            position: usize,
            call: StaticArgumentCall<'_>,
        ) -> Option<AppliedTraitScope<'p>> {
            if matches!(call, StaticArgumentCall::Expression(_, call) if !call.receiver.is_valid())
            {
                return None;
            }
            let (symbol, reference) = match call {
                StaticArgumentCall::Statement(call) => (
                    call.receiver_symbol,
                    validation::projected_statement_receiver_type_reference(
                        program, state, position, call,
                    ),
                ),
                StaticArgumentCall::Expression(_, call) => (
                    expressions_of(program)
                        .name_path_node(call.receiver)
                        .map_or_else(SymbolHandle::invalid, |path| path.symbol),
                    expression_type_reference_in_state(
                        program,
                        state.symbol,
                        position,
                        call.receiver,
                    ),
                ),
            };
            validation::qualified_trait_scope(program, machine.symbol, symbol, reference)
        }

        fn instance<'p>(
            program: &'p TypedTrees,
            lexical_owner: SymbolHandle,
            target: SymbolHandle,
            receiver: impl FnOnce() -> Option<AppliedTraitScope<'p>>,
            application: TypeReferenceHandle,
            invocation: &InvocationTuple,
        ) -> Result<SymbolHandle, Vec<Diagnostic>> {
            let Some((template, scope)) =
                source_scope(program, lexical_owner, target, receiver, application)?
            else {
                return Ok(SymbolHandle::invalid());
            };
            Ok(instance_in_scope(program, template, &scope, invocation))
        }

        fn source_scope<'p>(
            program: &'p TypedTrees,
            lexical_owner: SymbolHandle,
            target: SymbolHandle,
            receiver: impl FnOnce() -> Option<AppliedTraitScope<'p>>,
            application: TypeReferenceHandle,
        ) -> Result<Option<(SymbolHandle, AppliedTraitScope<'p>)>, Vec<Diagnostic>> {
            if let Some(closed) = program.closed_requirement_application(target) {
                return closed
                    .trait_scope
                    .scope(program)
                    .map(|scope| Some((closed.template, scope)))
                    .ok_or_else(|| failed("applied public call lost its retained source scope"));
            }
            if application.is_valid() {
                let Some((owner, _)) = program.trait_machine_signature(target) else {
                    return Ok(None);
                };
                return Ok(program
                    .applied_requirement_scope(lexical_owner, application)
                    .and_then(|scope| scope.declaring_scope(program, owner.symbol))
                    .map(|scope| (target, scope)));
            }
            if program.trait_machine_signature(target).is_none()
                && let Some(machine) = program.machine_by_symbol(lexical_owner)
                && let Some(requirement) =
                    validation::named_conformance_target_requirement(program, machine, target)
                        .map_err(|message| vec![Diagnostic::error(message)])?
            {
                return Ok(Some((
                    requirement.signature.symbol,
                    requirement.application,
                )));
            }
            let template = program
                .closed_requirement_application(target)
                .map_or(target, |application| application.template);
            let Some((owner, _)) = program.trait_machine_signature(template) else {
                return Ok(None);
            };
            let qualifier_scope =
                receiver().and_then(|scope| scope.declaring_scope(program, owner.symbol));
            let scope = match qualifier_scope {
                Some(scope) => Some(scope),
                None if program.trait_type_parameters(owner).is_empty()
                    && owner.lifetime_parameters.is_empty() =>
                {
                    Some(AppliedTraitScope::abstract_scope(owner))
                }
                None => None,
            };
            Ok(scope.map(|scope| (template, scope)))
        }

        fn instance_in_scope(
            program: &TypedTrees,
            template: SymbolHandle,
            scope: &AppliedTraitScope<'_>,
            invocation: &InvocationTuple,
        ) -> SymbolHandle {
            let arguments = &invocation.arguments;
            let Some((_, signature)) = program.trait_machine_signature(template) else {
                return SymbolHandle::invalid();
            };
            let parameters = program.state_signature_type_parameters(signature);
            if parameters.len() != arguments.len() {
                return SymbolHandle::invalid();
            }
            let mut matches = program
        .closed_requirement_applications()
        .filter(|application| {
            application.template == template
                && program
                    .requirement_application_matches_contract_scope(application.signature.symbol, scope)
                && expressions_of(program)
                    .static_machine_application_lifetimes(application.application)
                    == invocation.lifetimes
                && (invocation.lifetimes.is_empty()
                    || expressions_of(program)
                        .static_machine_application(application.application)
                        .lexical_owner
                        == invocation.owner
                        && program
                            .expression_table
                            .static_machine_application(application.application)
                            .lifetime_binding_declaration
                            == invocation.lifetime_declaration)
                && parameters
                    .iter()
                    .zip(arguments)
                    .all(|(parameter, argument)| {
                        let bindings = expressions_of(program)
                            .static_machine_application_bindings(application.application);
                        let mut selected = bindings
                            .iter()
                            .filter(|binding| binding.parameter == parameter.symbol);
                        let Some(binding) = selected.next() else {
                            return false;
                        };
                        selected.next().is_none()
                            && (if matches!(binding.value, typed_trees::static_applications::StaticMachineBindingValue::RuntimeValue { .. }) {
                                matches!(parameter.kind, TypeParameterKind::Value { .. })
                                    && crate::monomorphization::const_arguments::is_runtime_value_argument(program, argument)
                            } else { program.static_binding_matches_argument(
                                &parameter.kind,
                                &binding.value,
                                argument,
                                invocation.owner,
                                expressions_of(program)
                                    .static_machine_application(application.application)
                                    .lexical_owner,
                            ) })
                    })
        });
            let Some(application) = matches.next() else {
                return SymbolHandle::invalid();
            };
            if matches.next().is_some() {
                return SymbolHandle::invalid();
            }
            application.signature.symbol
        }

        pub(crate) fn source_expression_roster(
            program: &TypedTrees,
            machine: SymbolHandle,
            state: SymbolHandle,
            position: usize,
            statement: &StatementNode,
        ) -> Vec<typed_trees::expression::ExpressionHandle> {
            let mut expressions = Vec::new();
            validation::collect_statement_expression_trees(program, statement, &mut expressions);
            for package in &program.proof_output_calls {
                if package.machine_symbol == machine
                    && package.state_symbol == state
                    && package
                        .runtime_call_statement_index
                        .unwrap_or(package.statement_index)
                        == position
                {
                    collect_expression_tree(program, package.call, &mut expressions);
                }
            }
            expressions
        }

        fn prepare_invocations(
            program: &mut TypedTrees,
        ) -> Result<Vec<PreparedInvocation>, Vec<Diagnostic>> {
            let mut invocations = Vec::new();
            for machine in program.machines() {
                for state in program.machine_states(machine) {
                    for (position, statement) in program
                        .statement_table
                        .indexed_statements(state.statement_nodes)
                    {
                        if let Some(call) = statement.as_call() {
                            invocations.push(PreparedInvocation {
                                site: crate::monomorphization::CallSite::Statement(
                                    state.statement_nodes.handle_at(position as u32),
                                ),
                                owner: machine.symbol,
                                state: state.symbol,
                                target: call.target_symbol,
                                tuple: occurrence(
                                    program,
                                    machine,
                                    state,
                                    position,
                                    StaticArgumentCall::Statement(call),
                                ),
                            });
                        }
                        for expression in source_expression_roster(
                            program,
                            machine.symbol,
                            state.symbol,
                            position,
                            statement,
                        ) {
                            if let Some(call) = expressions_of(program).call_node(expression) {
                                invocations.push(PreparedInvocation {
                                    site: crate::monomorphization::CallSite::Expression(expression),
                                    owner: machine.symbol,
                                    state: state.symbol,
                                    target: call.target_symbol,
                                    tuple: occurrence(
                                        program,
                                        machine,
                                        state,
                                        position,
                                        StaticArgumentCall::Expression(expression, call),
                                    ),
                                });
                            }
                        }
                    }
                }
            }
            for invocation in &mut invocations {
                crate::monomorphization::body_rewriting::normalize_const_arguments(
                    program,
                    invocation.tuple.owner,
                    &mut invocation.tuple.arguments,
                )
                .map_err(|message| vec![Diagnostic::error(message)])?;
            }
            Ok(invocations)
        }

        pub(crate) fn invocation_tuples<'p>(
            program: &'p TypedTrees,
            requirement: SymbolHandle,
            scope: &AppliedTraitScope<'_>,
        ) -> Result<Vec<InvocationTuple>, Vec<Diagnostic>> {
            let mut tuples = Vec::new();
            let mut add =
                |owner,
                 target,
                 nominal_binder,
                 receiver: &dyn Fn() -> Option<AppliedTraitScope<'p>>,
                 application: TypeReferenceHandle,
                 dispatch: Option<&typed_trees::typed_trees::StaticRequirementDispatch>,
                 invocation: InvocationTuple|
                 -> Result<(), Vec<Diagnostic>> {
                    let selected = if let Some(dispatch) = dispatch {
                        let scope = program
                            .selected_requirement_contract_scope(owner, dispatch)
                            .map_err(failed)?;
                        let template = program
                            .closed_requirement_application(dispatch.requirement)
                            .map_or(dispatch.requirement, |application| application.template);
                        Some((template, scope))
                    } else if let Some(source) = invocation_binding_views::nominal_source(
                        program,
                        owner,
                        nominal_binder,
                        target,
                    )? {
                        Some(source)
                    } else {
                        source_scope(program, owner, target, receiver, application)?
                    };
                    let Some((template, selected)) = selected else {
                        return Ok(());
                    };
                    if template == requirement
                        && program.requirement_contract_scopes_match(&selected, scope)
                        && !tuples.contains(&invocation)
                    {
                        tuples.push(invocation);
                    }
                    Ok(())
                };
            for machine in program.machines() {
                if program.callable_body_has_open_parameters(machine) {
                    continue;
                }
                for state in program.machine_states(machine) {
                    for (position, statement) in program
                        .statement_table
                        .indexed_statements(state.statement_nodes)
                    {
                        if let Some(call) = statement.as_call() {
                            add(
                                machine.symbol,
                                call.target_symbol,
                                call.static_machine_parameter,
                                &|| {
                                    validation::qualified_trait_scope(
                                        program,
                                        machine.symbol,
                                        call.receiver_symbol,
                                        validation::projected_statement_receiver_type_reference(
                                            program, state, position, call,
                                        ),
                                    )
                                },
                                TypeReferenceHandle::invalid(),
                                call.static_requirement_dispatch.as_ref(),
                                occurrence(
                                    program,
                                    machine,
                                    state,
                                    position,
                                    StaticArgumentCall::Statement(call),
                                ),
                            )?;
                        }
                        let expressions = source_expression_roster(
                            program,
                            machine.symbol,
                            state.symbol,
                            position,
                            statement,
                        );
                        for expression in expressions {
                            if let Some(call) = expressions_of(program).call_node(expression) {
                                add(
                                    machine.symbol,
                                    call.target_symbol,
                                    call.static_machine_parameter,
                                    &|| {
                                        if call.receiver.is_valid() {
                                            validation::qualified_trait_scope(
                                                program,
                                                machine.symbol,
                                                expressions_of(program)
                                                    .name_path_node(call.receiver)
                                                    .map_or_else(SymbolHandle::invalid, |path| {
                                                        path.symbol
                                                    }),
                                                expression_type_reference_in_state(
                                                    program,
                                                    state.symbol,
                                                    position,
                                                    call.receiver,
                                                ),
                                            )
                                        } else {
                                            None
                                        }
                                    },
                                    call.requirement_application,
                                    call.static_requirement_dispatch.as_ref(),
                                    occurrence(
                                        program,
                                        machine,
                                        state,
                                        position,
                                        StaticArgumentCall::Expression(expression, call),
                                    ),
                                )?;
                            }
                        }
                    }
                }
            }
            Ok(tuples)
        }

        fn dispatch_instance(
            program: &TypedTrees,
            owner: SymbolHandle,
            dispatch: &typed_trees::typed_trees::StaticRequirementDispatch,
            invocation: &InvocationTuple,
        ) -> Result<SymbolHandle, Vec<Diagnostic>> {
            let scope = program
                .selected_requirement_contract_scope(owner, dispatch)
                .map_err(failed)?;
            let template = program
                .closed_requirement_application(dispatch.requirement)
                .map_or(dispatch.requirement, |closed| closed.template);
            Ok(instance_in_scope(program, template, &scope, invocation))
        }

        pub(crate) fn prepare_invocation_bindings(
            program: &mut TypedTrees,
        ) -> Result<(), Vec<Diagnostic>> {
            let invocations = prepare_invocations(program)?;
            retain_invocation_bindings(program, &invocations)
        }

        fn retain_invocation_bindings(
            program: &mut TypedTrees,
            invocations: &[PreparedInvocation],
        ) -> Result<(), Vec<Diagnostic>> {
            for invocation in invocations {
                let (site, owner, target) = (invocation.site, invocation.owner, invocation.target);
                let mut template = program
                    .closed_requirement_application(target)
                    .map_or(target, |closed| closed.template);
                if program.trait_machine_signature(template).is_none()
                    && let Some((requirement, _)) = source_scope(
                        program,
                        owner,
                        target,
                        || None,
                        TypeReferenceHandle::invalid(),
                    )?
                {
                    template = requirement;
                }
                let lexical_signature = program
                    .machine_by_symbol(owner)
                    .and_then(|machine| program.machine_parameter_signature_in(machine, target));
                let signature = lexical_signature.or_else(|| {
                    program
                        .trait_machine_signature(template)
                        .map(|(_, signature)| signature)
                });
                let Some(signature) = signature else {
                    continue;
                };
                if lexical_signature.is_some() {
                    template = target;
                }
                let parameters = program.state_signature_type_parameters(signature).to_vec();
                if parameters.is_empty() {
                    continue;
                }
                let application = crate::monomorphization::applications::retain_authored_call(
                    program, site, owner,
                );
                let mut arguments = invocation.tuple.arguments.clone();
                if arguments.len() != parameters.len() {
                    continue;
                }
                let lexical_owner = expressions_of(program)
                    .static_machine_application(application)
                    .lexical_owner;
                crate::monomorphization::body_rewriting::normalize_const_arguments(
                    program,
                    lexical_owner,
                    &mut arguments,
                )
                .map_err(|message| vec![Diagnostic::error(message)])?;
                let bindings =
                    super::invocation_bindings(program, lexical_owner, &parameters, &arguments)?;
                expressions_of_mut(program)
                    .static_machine_application_mut(application)
                    .binding_declaration = template;
                expressions_of_mut(program)
                    .set_static_machine_application_bindings(application, &bindings);
            }
            Ok(())
        }

        pub(crate) fn retarget(program: &mut TypedTrees) -> Result<(), Vec<Diagnostic>> {
            let invocations = prepare_invocations(program)?;
            let invocation_map: std::collections::HashMap<
                (crate::monomorphization::CallSite, SymbolHandle),
                &InvocationTuple,
            > = invocations
                .iter()
                .map(|invocation| ((invocation.site, invocation.owner), &invocation.tuple))
                .collect();
            let mut expression_updates = Vec::new();
            let mut expression_dispatch_updates = Vec::new();
            let mut diagnostics = Vec::new();
            let mut statement_updates = Vec::new();
            let mut statement_dispatch_updates = Vec::new();
            for machine in program.machines() {
                if program.callable_body_has_open_parameters(machine) {
                    continue;
                }
                for state in program.machine_states(machine) {
                    for (position, statement) in program
                        .statement_table
                        .indexed_statements(state.statement_nodes)
                    {
                        if let Some(call) = statement.as_call() {
                            let site = crate::monomorphization::CallSite::Statement(
                                state.statement_nodes.handle_at(position as u32),
                            );
                            let invocation = invocation_map
                                .get(&(site, machine.symbol))
                                .copied()
                                .ok_or_else(|| {
                                failed("requirement call lost its prepared invocation")
                            })?;
                            if let Some(dispatch) = &call.static_requirement_dispatch {
                                let selected = dispatch_instance(
                                    program,
                                    machine.symbol,
                                    dispatch,
                                    invocation,
                                )?;
                                if selected.is_valid() {
                                    statement_dispatch_updates.push((
                                        state.statement_nodes.handle_at(position as u32),
                                        selected,
                                    ));
                                }
                            }
                            let selected = instance(
                                program,
                                machine.symbol,
                                call.target_symbol,
                                || {
                                    validation::qualified_trait_scope(
                                        program,
                                        machine.symbol,
                                        call.receiver_symbol,
                                        validation::projected_statement_receiver_type_reference(
                                            program, state, position, call,
                                        ),
                                    )
                                },
                                TypeReferenceHandle::invalid(),
                                invocation,
                            )?;
                            if selected.is_valid() {
                                statement_updates.push((
                                    state.statement_nodes.handle_at(position as u32),
                                    selected,
                                ));
                            }
                        }
                        let expressions = source_expression_roster(
                            program,
                            machine.symbol,
                            state.symbol,
                            position,
                            statement,
                        );
                        for expression in expressions {
                            let Some(call) = expressions_of(program).call_node(expression) else {
                                continue;
                            };
                            let site = crate::monomorphization::CallSite::Expression(expression);
                            let invocation = invocation_map
                                .get(&(site, machine.symbol))
                                .copied()
                                .ok_or_else(|| {
                                failed("requirement call lost its prepared invocation")
                            })?;
                            if let Some(dispatch) = &call.static_requirement_dispatch {
                                let selected = dispatch_instance(
                                    program,
                                    machine.symbol,
                                    dispatch,
                                    invocation,
                                )?;
                                if selected.is_valid() {
                                    if let Some((_, prior)) = expression_dispatch_updates
                                        .iter()
                                        .find(|(handle, _)| *handle == expression)
                                    {
                                        if *prior != selected {
                                            diagnostics.push(Diagnostic::error("one evidence expression occurs under disagreeing exact trait applications"));
                                        }
                                    } else {
                                        expression_dispatch_updates.push((expression, selected));
                                    }
                                }
                            }
                            let selected = instance(
                                program,
                                machine.symbol,
                                call.target_symbol,
                                || {
                                    if call.receiver.is_valid() {
                                        validation::qualified_trait_scope(
                                            program,
                                            machine.symbol,
                                            expressions_of(program)
                                                .name_path_node(call.receiver)
                                                .map_or_else(SymbolHandle::invalid, |path| {
                                                    path.symbol
                                                }),
                                            expression_type_reference_in_state(
                                                program,
                                                state.symbol,
                                                position,
                                                call.receiver,
                                            ),
                                        )
                                    } else {
                                        None
                                    }
                                },
                                call.requirement_application,
                                invocation,
                            )?;
                            if !selected.is_valid() {
                                continue;
                            }
                            if let Some((_, prior)) = expression_updates
                                .iter()
                                .find(|(handle, _)| *handle == expression)
                            {
                                if *prior != selected {
                                    diagnostics.push(Diagnostic::error("one requirement expression occurs under disagreeing exact trait applications"));
                                }
                            } else {
                                expression_updates.push((expression, selected));
                            }
                        }
                    }
                }
            }
            if !diagnostics.is_empty() {
                return Err(diagnostics);
            }
            retain_invocation_bindings(program, &invocations)?;
            for (handle, selected) in expression_updates {
                if let ExpressionNode::Call(call) =
                    expressions_of_mut(program).expression_mut(handle)
                {
                    call.target_symbol = selected;
                }
            }
            for (handle, selected) in statement_updates {
                if let StatementNode::Call(call) = program.statement_table.statement_mut(handle) {
                    call.target_symbol = selected;
                }
            }
            for (handle, selected) in expression_dispatch_updates {
                if let ExpressionNode::Call(call) =
                    expressions_of_mut(program).expression_mut(handle)
                    && let Some(dispatch) = &mut call.static_requirement_dispatch
                {
                    dispatch.contract_signature = selected;
                }
            }
            for (handle, selected) in statement_dispatch_updates {
                if let StatementNode::Call(call) = program.statement_table.statement_mut(handle)
                    && let Some(dispatch) = &mut call.static_requirement_dispatch
                {
                    dispatch.contract_signature = selected;
                }
            }
            for invocation in &invocations {
                super::runtime_operands::realize(program, invocation)?;
            }
            publish_expression_call_views(
                program,
                invocations.iter().filter_map(|invocation| {
                    let crate::monomorphization::CallSite::Expression(expression) = invocation.site
                    else {
                        return None;
                    };
                    Some((invocation.owner, expression))
                }),
            )?;
            publish_statement_call_views(
                program,
                invocations.into_iter().filter_map(|invocation| {
                    let crate::monomorphization::CallSite::Statement(statement) = invocation.site
                    else {
                        return None;
                    };
                    Some((invocation.owner, statement))
                }),
            )?;
            Ok(())
        }

        pub(crate) fn publish_expression_call_views(
            program: &mut TypedTrees,
            calls: impl IntoIterator<Item = (SymbolHandle, typed_trees::expression::ExpressionHandle)>,
        ) -> Result<(), Vec<Diagnostic>> {
            for (owner, expression) in calls {
                let Some(call) = expressions_of(program).call_node(expression).cloned() else {
                    continue;
                };
                let selected_scope = if let Some(dispatch) = &call.static_requirement_dispatch {
                    Some((
                        dispatch.requirement,
                        program
                            .selected_requirement_contract_scope(owner, dispatch)
                            .map_err(failed)?,
                    ))
                } else {
                    source_scope(
                        program,
                        owner,
                        call.target_symbol,
                        || None,
                        call.requirement_application,
                    )?
                };
                let Some((template, scope)) = selected_scope else {
                    continue;
                };
                let Some((declaration, declared)) = program.trait_machine_signature(template)
                else {
                    continue;
                };
                let Some(scope) = scope.declaring_scope(program, declaration.symbol) else {
                    continue;
                };
                let Some(selected) = program
                    .applied_callable_signature(&scope, template)
                    .or_else(|| scope.keeps_declared_signatures(program).then_some(declared))
                    .cloned()
                else {
                    continue;
                };
                let Some(bindings) = invocation_binding_views::public_bindings(
                    program,
                    owner,
                    StaticArgumentCall::Expression(expression, &call),
                )?
                else {
                    continue;
                };
                if let Some(signature) = program
                    .prepare_requirement_invocation(owner, &call, &bindings, &selected)
                    .map_err(|message| vec![Diagnostic::error(message)])?
                {
                    let references =
                        typed_trees::references::collect_signature_references(program, &signature);
                    crate::monomorphization::finalize_referenced_domain_instance_identities(
                        program, references,
                    )
                    .map_err(|diagnostic| vec![diagnostic])?;
                }
            }
            Ok(())
        }

        fn publish_statement_call_views(
            program: &mut TypedTrees,
            calls: impl IntoIterator<Item = (SymbolHandle, typed_trees::statement::StatementHandle)>,
        ) -> Result<(), Vec<Diagnostic>> {
            for (owner, statement) in calls {
                let Some(call) = program
                    .statement_table
                    .statement(statement)
                    .as_call()
                    .cloned()
                else {
                    continue;
                };
                let selected_scope = if let Some(dispatch) = &call.static_requirement_dispatch {
                    Some((
                        dispatch.requirement,
                        program
                            .selected_requirement_contract_scope(owner, dispatch)
                            .map_err(failed)?,
                    ))
                } else {
                    source_scope(
                        program,
                        owner,
                        call.target_symbol,
                        || None,
                        TypeReferenceHandle::invalid(),
                    )?
                };
                let Some((template, scope)) = selected_scope else {
                    continue;
                };
                let Some((declaration, declared)) = program.trait_machine_signature(template)
                else {
                    continue;
                };
                let Some(scope) = scope.declaring_scope(program, declaration.symbol) else {
                    continue;
                };
                let Some(selected) = program
                    .applied_callable_signature(&scope, template)
                    .or_else(|| scope.keeps_declared_signatures(program).then_some(declared))
                    .cloned()
                else {
                    continue;
                };
                let Some(bindings) = invocation_binding_views::public_bindings(
                    program,
                    owner,
                    StaticArgumentCall::Statement(&call),
                )?
                else {
                    continue;
                };
                if let Some(signature) = program
                    .prepare_requirement_statement_invocation(owner, &call, &bindings, &selected)
                    .map_err(|message| vec![Diagnostic::error(message)])?
                {
                    let references =
                        typed_trees::references::collect_signature_references(program, &signature);
                    crate::monomorphization::finalize_referenced_domain_instance_identities(
                        program, references,
                    )
                    .map_err(|diagnostic| vec![diagnostic])?;
                }
            }
            Ok(())
        }
    }
    pub(crate) use calls::prepare_invocation_bindings;
    pub(crate) mod demands {
        use typed_trees::{TypedTrees, trait_definition::AppliedTraitScope};

        pub(crate) struct Demand {
            pub requirement: symbols::SymbolHandle,
            pub argument_owner: symbols::SymbolHandle,
            pub environment: typed_trees::trait_definition::TraitApplicationEnvironment,
        }

        pub(crate) fn collect(program: &TypedTrees) -> Vec<Demand> {
            let mut scopes = Vec::<AppliedTraitScope<'_>>::new();
            let mut roots = program.applied_trait_roots();
            roots.extend(
                program
                    .prepared_callable_contracts()
                    .filter_map(|contract| {
                        contract.trait_scope.and_then(|scope| scope.scope(program))
                    }),
            );
            for root in roots {
                root.visit(program, &mut |scope| {
                    if !scopes
                        .iter()
                        .any(|existing| program.requirement_contract_scopes_match(existing, scope))
                    {
                        scopes.push(scope.clone());
                    }
                });
            }
            scopes
                .into_iter()
                .flat_map(|scope| {
                    program
                        .trait_machine_signatures(scope.declaration)
                        .iter()
                        .map(move |signature| Demand {
                            requirement: signature.symbol,
                            argument_owner: scope.argument_owner,
                            environment: scope.environment(),
                        })
                })
                .collect()
        }
    }
    pub(crate) mod inference {
        use crate::monomorphization::{CallSite, selection};
        use crate::tables::{expressions_of, expressions_of_mut};
        use diagnostics::Diagnostic;
        use symbols::SymbolHandle;
        use typed_trees::{
            TypedTrees,
            data::{TypeParameter, TypeParameterKind},
            expression::StaticMachineArgument,
            machine::Machine,
            references::StaticArgumentCall,
            signature::StateSignature,
            state::State,
            types::TypeReferenceHandle,
        };

        struct Invocation {
            site: CallSite,
            owner: SymbolHandle,
            target: SymbolHandle,
            parameters: Vec<TypeParameter>,
            declaration_parameters: Vec<TypeParameter>,
            signature: StateSignature,
            selected: Vec<StaticMachineArgument>,
            operands: Vec<(TypeReferenceHandle, TypeReferenceHandle)>,
        }

        pub(crate) fn prepare(program: &mut TypedTrees) -> Result<(), Vec<Diagnostic>> {
            loop {
                let mut pending = Vec::new();
                let mut seen = std::collections::HashSet::new();
                for machine in program.machines() {
                    for state in program.machine_states(machine) {
                        for (position, (statement_handle, statement)) in program
                            .statement_table
                            .iter_statements(state.statement_nodes)
                            .enumerate()
                        {
                            let expressions = super::calls::source_expression_roster(
                                program,
                                machine.symbol,
                                state.symbol,
                                position,
                                statement,
                            );
                            let sites = statement
                                .as_call()
                                .map(|_| CallSite::Statement(statement_handle))
                                .into_iter()
                                .chain(expressions.into_iter().map(CallSite::Expression));
                            for site in sites {
                                if seen.insert(site)
                                    && let Some(invocation) = observe(program, machine, state, site)
                                {
                                    pending.push(invocation);
                                }
                            }
                        }
                    }
                }
                let mut changed = false;
                for invocation in pending {
                    let Some(arguments) = infer(program, &invocation)? else {
                        continue;
                    };
                    let bindings = super::invocation_bindings(
                        program,
                        invocation.owner,
                        &invocation.declaration_parameters,
                        &arguments,
                    )?;
                    let application = crate::monomorphization::applications::retain_authored_call(
                        program,
                        invocation.site,
                        invocation.owner,
                    );
                    let source =
                        expressions_of_mut(program).static_machine_application_mut(application);
                    if source.lexical_owner != invocation.owner
                        || source.registration_operation != invocation.target
                        || (source.binding_declaration.is_valid()
                            && source.binding_declaration != invocation.target)
                    {
                        return Err(vec![Diagnostic::error(
                            "public method inference lost its exact source application",
                        )]);
                    }
                    source.binding_declaration = invocation.target;
                    expressions_of_mut(program)
                        .set_static_machine_application_bindings(application, &bindings);
                    let signature = match invocation.site {
                        CallSite::Expression(expression) => {
                            let call = expressions_of(program)
                                .call_node(expression)
                                .expect("observed public expression call")
                                .clone();
                            program.prepare_requirement_invocation(
                                invocation.owner,
                                &call,
                                &bindings,
                                &invocation.signature,
                            )
                        }
                        CallSite::Statement(statement) => {
                            let call = program
                                .statement_table
                                .statement(statement)
                                .as_call()
                                .expect("observed public statement call")
                                .clone();
                            program.prepare_requirement_statement_invocation(
                                invocation.owner,
                                &call,
                                &bindings,
                                &invocation.signature,
                            )
                        }
                    }
                    .map_err(|message| vec![Diagnostic::error(message)])?;
                    crate::monomorphization::finalize_callable_signature_identities(
                        program, signature,
                    )
                    .map_err(|diagnostic| vec![diagnostic])?;
                    changed = true;
                }
                if !changed {
                    return Ok(());
                }
                let signatures =
                    program
                        .prepare_applied_callable_signatures()
                        .map_err(|message| {
                            vec![Diagnostic::error(format!(
                                "inferred public method signature: {message}"
                            ))]
                        })?;
                crate::monomorphization::finalize_callable_signature_identities(
                    program, signatures,
                )
                .map_err(|diagnostic| vec![diagnostic])?;
            }
        }

        fn observe(
            program: &TypedTrees,
            machine: &Machine,
            state: &State,
            site: CallSite,
        ) -> Option<Invocation> {
            let (call, signature, arguments) = match site {
                CallSite::Expression(expression) => {
                    let call = expressions_of(program).call_node(expression)?;
                    if call.static_requirement_dispatch.is_some()
                        || program
                            .closed_requirement_application(call.target_symbol)
                            .is_some()
                    {
                        return None;
                    }
                    (
                        StaticArgumentCall::Expression(expression, call),
                        program.requirement_call_signature(machine.symbol, call)?,
                        expressions_of(program).expression_handles(call.arguments),
                    )
                }
                CallSite::Statement(statement) => {
                    let call = program.statement_table.statement(statement).as_call()?;
                    if call.static_requirement_dispatch.is_some()
                        || program
                            .closed_requirement_application(call.target_symbol)
                            .is_some()
                    {
                        return None;
                    }
                    (
                        StaticArgumentCall::Statement(call),
                        program.requirement_statement_call_signature(machine.symbol, call)?,
                        program.statement_table.expression_handles(call.arguments),
                    )
                }
            };
            let target = call.registration_operation(program);
            let (_, declaration) = program.trait_machine_signature(target)?;
            let parameters = program.state_signature_type_parameters(signature);
            let declaration_parameters = program.state_signature_type_parameters(declaration);
            if parameters.len() != declaration_parameters.len()
                || declaration_parameters
                    .iter()
                    .zip(parameters)
                    .any(|(declared, applied)| {
                        program.applied_signature_parameter(declaration, signature, declared.symbol)
                            != Some(applied.symbol)
                            || std::mem::discriminant(&declared.kind)
                                != std::mem::discriminant(&applied.kind)
                    })
            {
                return None;
            }
            let selected = call.arguments(program);
            if selected.len() >= parameters.len() {
                return None;
            }
            if call.application().is_valid() {
                let source = expressions_of(program).static_machine_application(call.application());
                let bindings =
                    expressions_of(program).static_machine_application_bindings(call.application());
                if source.lexical_owner == machine.symbol
                    && source.binding_declaration == target
                    && bindings.len() == declaration_parameters.len()
                    && declaration_parameters.iter().all(|parameter| {
                        bindings
                            .iter()
                            .filter(|binding| binding.parameter == parameter.symbol)
                            .count()
                            == 1
                    })
                {
                    return None;
                }
            }
            let runtime = program.state_signature_parameters(signature);
            if runtime.len() != arguments.len() {
                return None;
            }
            let operands = runtime
                .iter()
                .zip(arguments)
                .filter_map(|(required, argument)| {
                    let actual = validation::expression_result_type_reference(
                        program, machine, state, *argument,
                    )?;
                    if !program.callable_body_has_open_parameters(machine)
                        && selection::names_type_binder(program, actual)
                    {
                        return None;
                    }
                    Some((required.type_reference, actual))
                })
                .collect();
            Some(Invocation {
                site,
                owner: machine.symbol,
                target,
                parameters: parameters.to_vec(),
                declaration_parameters: declaration_parameters.to_vec(),
                signature: signature.clone(),
                selected: selected.to_vec(),
                operands,
            })
        }

        fn infer(
            program: &TypedTrees,
            invocation: &Invocation,
        ) -> Result<Option<Vec<StaticMachineArgument>>, Vec<Diagnostic>> {
            let types = invocation
                .parameters
                .iter()
                .filter(|&parameter| matches!(parameter.kind, TypeParameterKind::Type))
                .map(|parameter| (parameter.symbol, parameter.name.as_str().to_owned()))
                .collect::<Vec<_>>();
            let constants = invocation
                .parameters
                .iter()
                .filter_map(|parameter| {
                    let TypeParameterKind::Const { type_reference } = parameter.kind else {
                        return None;
                    };
                    Some((
                        parameter.symbol,
                        parameter.name.as_str().to_owned(),
                        type_reference,
                    ))
                })
                .collect::<Vec<_>>();
            let context = selection::BindingContext::new(program, &types, &constants, 0);
            let mut proposals = selection::StaticProposals::default();
            for (required, actual) in &invocation.operands {
                selection::infer_static_bindings(context, *required, *actual, &mut proposals);
            }
            let mut inferred = vec![None; invocation.parameters.len()];
            for (is_const, symbol, actual) in proposals
                .types
                .into_iter()
                .map(|(_, ordinal, actual)| (false, types[ordinal].0, actual))
                .chain(
                    proposals
                        .constants
                        .into_iter()
                        .map(|(_, ordinal, actual)| (true, constants[ordinal].0, actual)),
                )
            {
                let ordinal = invocation
                    .parameters
                    .iter()
                    .position(|parameter| parameter.symbol == symbol)
                    .expect("proposal belongs to this public telescope");
                if ordinal < invocation.selected.len() {
                    continue;
                }
                if let Some(previous) = inferred[ordinal] {
                    let agrees = if is_const {
                        selection::same_const_identity(program, previous, actual)
                    } else {
                        program.normalized_type_identity(previous)
                            == program.normalized_type_identity(actual)
                    };
                    if !agrees {
                        return Err(vec![Diagnostic::error(format!(
                            "public method inference has conflicting structural operands for `{}`",
                            invocation.parameters[ordinal].name,
                        ))]);
                    }
                } else {
                    inferred[ordinal] = Some(actual);
                }
            }
            let mut complete = invocation.selected.clone();
            for actual in inferred.into_iter().skip(complete.len()) {
                let Some(actual) = actual else {
                    return Ok(None);
                };
                complete.push(
                    program
                        .static_operand_from_type_reference(actual)
                        .map_err(|message| vec![Diagnostic::error(message)])?,
                );
            }
            Ok(Some(complete))
        }
    }
    pub(crate) mod runtime_operands {
        use crate::tables::{expressions_of, expressions_of_mut};
        use diagnostics::Diagnostic;
        use typed_trees::{TypedTrees, expression::ExpressionNode, statement::StatementNode};

        pub(crate) fn realize(
            program: &mut TypedTrees,
            invocation: &super::calls::PreparedInvocation,
        ) -> Result<(), Vec<Diagnostic>> {
            let (target, arguments) = match invocation.site {
                crate::monomorphization::CallSite::Expression(expression) => {
                    let Some(call) = expressions_of(program).call_node(expression) else {
                        return Ok(());
                    };
                    (call.target_symbol, call.arguments)
                }
                crate::monomorphization::CallSite::Statement(statement) => {
                    let Some(call) = program.statement_table.statement(statement).as_call() else {
                        return Ok(());
                    };
                    (call.target_symbol, call.arguments)
                }
            };
            let count = arguments.len();
            let Some(application) = program.closed_requirement_application(target) else {
                return Ok(());
            };
            let expected = program
                .state_signature_parameters(application.signature)
                .len();
            let runtime = application
                .local_arguments
                .iter()
                .zip(&invocation.tuple.arguments)
                .filter_map(|(coordinate, actual)| match coordinate {
                    typed_trees::typed_trees::RequirementLocalArgument::RuntimeValue { .. } => {
                        Some(crate::monomorphization::RuntimeValueBinding::Authored(
                            actual.clone(),
                        ))
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            if runtime.is_empty() {
                return Ok(());
            }
            let (_, declared) = program
                .trait_machine_signature(application.template)
                .ok_or_else(|| {
                    crate::monomorphization::failed(
                        "runtime requirement lost its authored signature",
                    )
                })?;
            let ordinary = program
                .applied_signature_authored_parameters(declared, application.signature)
                .ok_or_else(|| {
                    crate::monomorphization::failed(
                        "runtime requirement lost its Value formal correspondence",
                    )
                })?
                .len();
            if ordinary + runtime.len() != expected {
                return Err(crate::monomorphization::failed(
                    "runtime requirement projection lost its complete Value formal roster",
                ));
            }
            if count == expected {
                let operands = match invocation.site {
                    crate::monomorphization::CallSite::Expression(_) => {
                        expressions_of(program).expression_handles(arguments)[ordinary..].to_vec()
                    }
                    crate::monomorphization::CallSite::Statement(_) => {
                        program.statement_table.expression_handles(arguments)[ordinary..].to_vec()
                    }
                };
                if let Some(application) = invocation.site.authored_machine_application(program) {
                    crate::monomorphization::body_rewriting::record_runtime_call_operands(
                        program,
                        application,
                        &operands,
                    );
                }
                return Ok(());
            }
            if count + runtime.len() != expected {
                return Err(vec![Diagnostic::error(
                    "runtime requirement projection lost its ordinary operand roster",
                )]);
            }
            let operands = crate::monomorphization::body_rewriting::runtime_call_operands(
                program,
                invocation.site,
                invocation.state,
                &runtime,
            );
            if operands.len() != runtime.len() {
                return Err(vec![Diagnostic::error(
                    "runtime requirement projection lost an executable Value operand",
                )]);
            }
            if let Some(application) = invocation.site.authored_machine_application(program) {
                crate::monomorphization::body_rewriting::record_runtime_call_operands(
                    program,
                    application,
                    &operands,
                );
            }
            match invocation.site {
                crate::monomorphization::CallSite::Expression(expression) => {
                    let mut arguments = expressions_of(program)
                        .expression_handles(arguments)
                        .to_vec();
                    arguments.extend(operands);
                    let arguments =
                        expressions_of_mut(program).insert_expression_handles(arguments);
                    let ExpressionNode::Call(call) =
                        expressions_of_mut(program).expression_mut(expression)
                    else {
                        return Err(crate::monomorphization::failed(
                            "runtime requirement projection lost its expression call",
                        ));
                    };
                    call.arguments = arguments;
                }
                crate::monomorphization::CallSite::Statement(statement) => {
                    let arguments =
                        crate::monomorphization::body_rewriting::insert_arguments_with_subjects(
                            program, arguments, &operands,
                        );
                    let StatementNode::Call(call) =
                        program.statement_table.statement_mut(statement)
                    else {
                        return Err(crate::monomorphization::failed(
                            "runtime requirement projection lost its statement call",
                        ));
                    };
                    call.arguments = arguments;
                }
            }
            Ok(())
        }
    }
    use typed_trees::finite_family::FamilyProbe;
    use typed_trees::static_applications::{StaticMachineBinding, StaticMachineBindingValue};
    use typed_trees::typed_trees::{ClosedConformanceApplication, StaticRequirementDispatch};
    use typed_trees::{TypedTrees, expression::StaticMachineArgument};

    pub fn prepare_requirement_expression_views(
        program: &mut TypedTrees,
        roots: &[(SymbolHandle, typed_trees::expression::ExpressionHandle)],
    ) -> Result<(), Vec<Diagnostic>> {
        program
            .prepare_closed_conformance_scopes()
            .map_err(|message| vec![Diagnostic::error(message)])?;
        let signatures = program
            .prepare_applied_callable_signatures()
            .map_err(|message| vec![Diagnostic::error(message)])?;
        crate::monomorphization::finalize_callable_signature_identities(program, signatures)
            .map_err(|diagnostic| vec![diagnostic])?;

        #[derive(Default)]
        struct SourceExpressions {
            owner: SymbolHandle,
            seen: std::collections::HashSet<(
                SymbolHandle,
                typed_trees::expression::ExpressionHandle,
            )>,
            calls: Vec<(SymbolHandle, typed_trees::expression::ExpressionHandle)>,
        }
        impl Extend<SymbolHandle> for SourceExpressions {
            fn extend<T: IntoIterator<Item = SymbolHandle>>(&mut self, _: T) {}
        }
        impl typed_trees::references::TypedReferences for SourceExpressions {
            fn derived_declaration_references(&self) -> bool {
                false
            }
            fn expression(
                &mut self,
                expression: typed_trees::expression::ExpressionHandle,
            ) -> bool {
                if !self.seen.insert((self.owner, expression)) {
                    return false;
                }
                self.calls.push((self.owner, expression));
                true
            }
        }
        let mut source = SourceExpressions::default();
        for &(owner, root) in roots {
            source.owner = owner;
            typed_trees::references::visit_expression(program, &mut source, root);
        }
        let calls = source
            .calls
            .into_iter()
            .rev()
            .filter(|(owner, expression)| {
                crate::tables::expressions_of(program)
                    .call_node(*expression)
                    .is_some_and(|call| {
                        !call.authored_machine_application.is_valid()
                            || crate::tables::expressions_of(program)
                                .static_machine_application(call.authored_machine_application)
                                .lexical_owner
                                == *owner
                    })
            })
            .collect::<Vec<_>>();
        calls::publish_expression_call_views(program, calls)
    }

    pub(crate) fn instantiate_requirement_applications(
        program: &mut TypedTrees,
    ) -> Result<(), Vec<Diagnostic>> {
        program
            .prepare_closed_conformance_scopes()
            .map_err(|message| vec![Diagnostic::error(message)])?;
        crate::monomorphization::contextual_calls::prepare_conformance_machine_demands(program)?;
        let signatures = program
            .prepare_applied_callable_signatures()
            .map_err(|message| {
                vec![Diagnostic::error(format!(
                    "applied logical signatures: {message}"
                ))]
            })?;
        crate::monomorphization::finalize_callable_signature_identities(program, signatures)
            .map_err(|diagnostic| vec![diagnostic])?;
        inference::prepare(program)?;
        let demands = demands::collect(program);
        for demand in demands {
            let requirement = demand.requirement;
            let has_outer_operands = {
                let scope = demand.environment.scope(program).ok_or_else(|| {
                    vec![Diagnostic::error(
                        "boundary family lost its complete source application",
                    )]
                })?;
                if program.require_receiving_trait_operands(&scope).is_err()
                    || scope.portable_application_identity(program).is_none()
                {
                    continue;
                }
                !program.trait_type_parameters(scope.declaration).is_empty()
                    || !scope.declaration.lifetime_parameters.is_empty()
            };
            let trait_arguments = program
                .materialize_trait_application_arguments(&demand.environment)
                .map_err(|message| {
                    vec![Diagnostic::error(format!(
                        "public requirement operands: {message}"
                    ))]
                })?;
            let (_, signature) = program
                .trait_machine_signature(requirement)
                .ok_or_else(|| failed("boundary family lost its source requirement"))?;
            let name = signature.name.clone();
            let parameters = program.state_signature_type_parameters(signature).to_vec();
            let tuples = if parameters.is_empty() && signature.lifetime_parameters.is_empty() {
                if !has_outer_operands {
                    continue;
                }
                vec![calls::InvocationTuple {
                    owner: demand.argument_owner,
                    lifetime_declaration: requirement,
                    arguments: Vec::new(),
                    lifetimes: Vec::new(),
                }]
            } else {
                match program.finite_signature_family(signature) {
                    FamilyProbe::Finite { tuples, .. } => tuples
                        .into_iter()
                        .map(|tuple| {
                            tuple
                                .identities
                                .iter()
                                .map(|identity| tuple_argument(identity))
                                .collect::<Result<Vec<_>, _>>()
                        })
                        .collect::<Result<Vec<_>, _>>()?
                        .into_iter()
                        .map(|arguments| calls::InvocationTuple {
                            owner: demand.argument_owner,
                            lifetime_declaration: requirement,
                            arguments,
                            lifetimes: Vec::new(),
                        })
                        .collect(),
                    FamilyProbe::NotFinite(_) => {
                        let scope = demand.environment.scope(program).ok_or_else(|| {
                            vec![Diagnostic::error(
                                "static invocation lost its declaring scope",
                            )]
                        })?;
                        calls::invocation_tuples(program, requirement, &scope)?
                    }
                }
            };
            for invocation in tuples {
                let mut arguments = invocation.arguments;
                crate::monomorphization::body_rewriting::normalize_const_arguments(
                    program,
                    invocation.owner,
                    &mut arguments,
                )
                .map_err(|message| vec![Diagnostic::error(message)])?;
                if arguments.len() != parameters.len() {
                    let mut diagnostic = Diagnostic::error(format!(
                        "static method application requires its complete invocation telescope for `{}` (found {}, expected {})",
                        program.symbols.display_path(requirement, "::"),
                        arguments.len(),
                        parameters.len(),
                    ));
                    if let Some(span) = program.symbols.symbol_source_span(invocation.owner) {
                        diagnostic = diagnostic.with_source_span(span);
                    }
                    return Err(vec![diagnostic]);
                }
                let mut open = false;
                for (parameter, argument) in parameters.iter().zip(&arguments) {
                    match parameter.kind {
                        TypeParameterKind::Type | TypeParameterKind::Const { .. } => {
                            let actual = program
                                .static_operand_type_reference_in_scope(argument, invocation.owner)
                                .map_err(|message| vec![Diagnostic::error(message)])?;
                            open |= crate::monomorphization::contextual_calls::operand_is_open(
                                program, actual,
                            );
                        }
                        TypeParameterKind::Value { .. } => {
                            if !crate::monomorphization::const_arguments::is_runtime_value_argument(
                                program, argument,
                            ) {
                                let actual = program
                                    .static_operand_type_reference_in_scope(
                                        argument,
                                        invocation.owner,
                                    )
                                    .map_err(|message| vec![Diagnostic::error(message)])?;
                                open |= crate::monomorphization::contextual_calls::operand_is_open(
                                    program, actual,
                                );
                            }
                        }
                        TypeParameterKind::Machine { .. } => {
                            open |= program.symbols.get(argument.symbol).kind
                                == symbols::SymbolKind::MachineParameter;
                        }
                        TypeParameterKind::Implementation => {
                            open |= argument.symbol.is_valid()
                                && program.symbols.get(argument.symbol).kind
                                    == symbols::SymbolKind::ConformanceParameter;
                        }
                        _ => {}
                    }
                }
                if open {
                    continue;
                }
                let bindings =
                    invocation_bindings(program, invocation.owner, &parameters, &arguments)?;
                let application = expressions_of_mut(program).retain_static_machine_application(
                    requirement,
                    name.clone(),
                    &arguments,
                );
                let source =
                    expressions_of_mut(program).static_machine_application_mut(application);
                source.lexical_owner = invocation.owner;
                source.binding_declaration = requirement;
                expressions_of_mut(program).set_static_machine_application_lifetimes(
                    application,
                    requirement,
                    &invocation.lifetimes,
                );
                expressions_of_mut(program)
                    .set_static_machine_application_bindings(application, &bindings);
                let instance = program
                    .instantiate_requirement_application(
                        &demand.environment,
                        requirement,
                        &trait_arguments,
                        application,
                    )
                    .map_err(|message| vec![Diagnostic::error(message)])?;
                let signature = program
                    .closed_requirement_application(instance)
                    .ok_or_else(|| {
                        failed("closed requirement lost its produced callable signature")
                    })?
                    .signature;
                let references =
                    typed_trees::references::collect_signature_references(program, signature);
                crate::monomorphization::finalize_referenced_domain_instance_identities(
                    program, references,
                )
                .map_err(|diagnostic| vec![diagnostic])?;
            }
        }
        loop {
            let signature_count = program.prepared_callable_signatures().count();
            crate::conformance::explicit_requirement_calls::route_explicit_requirement_calls(
                program,
            )?;
            program
                .prepare_closed_conformance_scopes()
                .map_err(|message| vec![Diagnostic::error(message)])?;
            let (signatures, wrote) = program
                .prepare_applied_callable_signatures_reporting_change()
                .map_err(|message| {
                    vec![Diagnostic::error(format!(
                        "applied logical signatures: {message}"
                    ))]
                })?;
            if !wrote {
                debug_assert!(signatures.is_empty());
                break;
            }
            crate::monomorphization::finalize_callable_signature_identities(program, signatures)
                .map_err(|diagnostic| vec![diagnostic])?;
            if program.prepared_callable_signatures().count() == signature_count {
                let signatures = program
                    .prepare_applied_callable_signatures()
                    .map_err(|message| vec![Diagnostic::error(message)])?;
                crate::monomorphization::finalize_callable_signature_identities(
                    program, signatures,
                )
                .map_err(|diagnostic| vec![diagnostic])?;
                break;
            }
        }
        calls::retarget(program)?;
        crate::monomorphization::contextual_calls::retain_closed_requirement_demands(program)?;
        Ok(())
    }

    pub(crate) fn selected_applications<'a>(
        program: &'a TypedTrees,
        owner: SymbolHandle,
        dispatch: &StaticRequirementDispatch,
    ) -> Vec<&'a ClosedConformanceApplication> {
        program.selected_conformance_applications(
            owner,
            dispatch.selection_application,
            dispatch.evidence_parameter,
            dispatch.requirement,
            dispatch.application_report_fingerprint,
            dispatch.application_commitment,
        )
    }

    pub(crate) fn invocation_bindings(
        program: &mut TypedTrees,
        owner: SymbolHandle,
        parameters: &[typed_trees::data::TypeParameter],
        arguments: &[StaticMachineArgument],
    ) -> Result<Vec<StaticMachineBinding>, Vec<Diagnostic>> {
        parameters
        .iter()
        .zip(arguments)
        .map(|(parameter, argument)| {
            Ok(StaticMachineBinding {
                parameter: parameter.symbol,
                value: match &parameter.kind {
                    TypeParameterKind::Type => StaticMachineBindingValue::Type(
                        program
                            .static_operand_type_reference_in_scope(argument, owner)
                            .map_err(|message| vec![Diagnostic::error(message)])?,
                    ),
                    TypeParameterKind::Const { type_reference }
                    | TypeParameterKind::Value { type_reference } => {
                        if matches!(parameter.kind, TypeParameterKind::Value { .. })
                            && crate::monomorphization::const_arguments::is_runtime_value_argument(program, argument)
                        {
                            StaticMachineBindingValue::RuntimeValue {
                                type_reference: *type_reference,
                                subject: argument.symbol,
                                expression: crate::monomorphization::const_arguments::runtime_subject_expression(
                                    program, argument,
                                )
                                .unwrap_or_else(typed_trees::expression::ExpressionHandle::invalid),
                            }
                        } else {
                            let actual = if let Some(atom) =
                                crate::monomorphization::const_arguments::spelling(program, argument)
                            {
                                program.type_reference_table.insert(
                                    typed_trees::types::TypeReferenceNode::Named {
                                        symbol: symbols::SymbolHandle::invalid(),
                                        name: typed_trees::name::Identifier::generated(atom),
                                    },
                                )
                            } else {
                                program
                                    .static_operand_type_reference_in_scope(argument, owner)
                                    .map_err(|message| vec![Diagnostic::error(message)])?
                            };
                            StaticMachineBindingValue::Const(actual)
                        }
                    }
                    TypeParameterKind::Machine { .. } => {
                        StaticMachineBindingValue::Machine(argument.clone())
                    }
                    TypeParameterKind::Implementation => {
                        StaticMachineBindingValue::Conformance(argument.clone())
                    }
                    _ => {
                        return Err(vec![Diagnostic::error(
                            "static requirement invocation has an unsupported binder category",
                        )]);
                    }
                },
            })
        })
        .collect()
    }

    fn tuple_argument(identity: &str) -> Result<StaticMachineArgument, Vec<Diagnostic>> {
        let spelling = crate::monomorphization::dynamic_families::const_identity_spelling(identity)
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "boundary family has no represented canonical static operand",
                )]
            })?;
        if let Ok(integer) = spelling.parse::<i128>() {
            let text = integer.to_string();
            let literal = numerics::literals::IntegerLiteral::from_parts(
                integer < 0,
                numerics::literals::IntegerRadix::Decimal,
                text.strip_prefix('-').unwrap_or(&text),
            )
            .map_err(|_| {
                vec![Diagnostic::error(
                    "boundary family integer has no represented literal",
                )]
            })?;
            return Ok(StaticMachineArgument {
                const_literal: Some(literal),
                ..StaticMachineArgument::default()
            });
        }
        Ok(StaticMachineArgument {
            path: vec![typed_trees::name::Identifier::generated(spelling)].into_boxed_slice(),
            ..StaticMachineArgument::default()
        })
    }
}

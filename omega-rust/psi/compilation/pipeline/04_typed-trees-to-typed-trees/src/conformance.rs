/*
Name-owned generic conformance application lifetime resolution
(`conformance_application_lifetimes`) and explicit requirement call elaboration
(`explicit_requirement_calls`).
*/

pub(crate) mod conformance_application_lifetimes {
    use crate::tables::{expressions_of, expressions_of_mut};
    use diagnostics::Diagnostic;
    use symbols::{SymbolHandle, SymbolKind};
    use typed_trees::TypedTrees;
    use typed_trees::expression::{ExpressionHandle, ExpressionNode, StaticMachineArgument};
    use typed_trees::name::Identifier;
    use typed_trees::statement::{StatementHandle, StatementNode};
    use typed_trees::static_applications::StaticMachineLifetimeBinding;
    use typed_trees::type_identity::{AppliedTypeIdentityScope, IndexIdentityArgument};
    use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

    pub(crate) mod argument_scopes {
        use symbols::SymbolHandle;
        use typed_trees::type_identity::{AppliedTypeIdentityScope, IndexIdentityArgument};
        use typed_trees::type_identity::{lifetime_parameter_identity, lifetime_parameter_name};
        use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};
        use validation::canonical_place_from_expression_in_state;

        pub(crate) fn source_application(
            program: &dyn validation::CheckingInput,
            machine: &typed_trees::machine::Machine,
            state: &typed_trees::state::State,
            argument: typed_trees::expression::ExpressionHandle,
        ) -> Option<(TypeReferenceHandle, Vec<TypeReferenceHandle>)> {
            if let Some(place) = canonical_place_from_expression_in_state(
                program,
                state.symbol,
                usize::MAX,
                argument,
            ) {
                return validation::canonical_place_source_type_application(
                    program,
                    state.symbol,
                    usize::MAX,
                    &place,
                );
            }
            let source = program
                .expression_table
                .borrow_expression(argument)
                .map_or(argument, |borrow| borrow.target);
            crate::source_type_queries::expression_result_type_reference(
                program, machine, state, source,
            )
            .map(|reference| (reference, Vec::new()))
        }

        #[derive(Clone, Copy)]
        pub(crate) struct Scope<'scopes> {
            pub(crate) owner: SymbolHandle,
            pub(crate) frames: &'scopes [AppliedTypeIdentityScope<'scopes>],
        }

        impl<'scopes> Scope<'scopes> {
            pub(crate) fn resolve(
                mut self,
                program: &dyn validation::CheckingInput,
                mut reference: TypeReferenceHandle,
            ) -> Option<(TypeReferenceHandle, Self)> {
                let mut visited = Vec::new();
                loop {
                    let coordinate = (reference, self.owner, self.frames.len());
                    if !reference.is_valid() || visited.contains(&coordinate) {
                        return None;
                    }
                    visited.push(coordinate);
                    let owner = program
                        .type_reference_table
                        .operand_lifetime_owner(reference);
                    if owner.is_valid() {
                        let count = self
                            .frames
                            .iter()
                            .rposition(|frame| {
                                program.lifetime_declaration_owner(frame.declaration)
                                    == program.lifetime_declaration_owner(owner)
                            })
                            .map_or(0, |ordinal| ordinal + 1);
                        self.frames = &self.frames[..count];
                        self.owner = owner;
                    }
                    let TypeReferenceNode::Named { symbol, .. } =
                        program.type_reference_table.type_reference(reference)
                    else {
                        return Some((reference, self));
                    };
                    let supplied =
                        self.frames
                            .iter()
                            .enumerate()
                            .rev()
                            .find_map(|(ordinal, frame)| {
                                frame.arguments.iter().find_map(|(parameter, actual)| {
                                    if parameter != symbol {
                                        return None;
                                    }
                                    match actual {
                                        IndexIdentityArgument::TypeArgument(actual)
                                        | IndexIdentityArgument::TypeReference(actual) => {
                                            Some((*actual, ordinal))
                                        }
                                        _ => None,
                                    }
                                })
                            });
                    if let Some((actual, ordinal)) = supplied {
                        reference = actual;
                        self.frames = &self.frames[..ordinal];
                        continue;
                    }
                    if let Some(origin) = program
                        .data_definition_by_symbol(*symbol)
                        .and_then(|definition| definition.generic_instance)
                    {
                        reference = origin;
                        continue;
                    }
                    return Some((reference, self));
                }
            }

            pub(crate) fn argument(
                self,
                program: &dyn validation::CheckingInput,
                caller: SymbolHandle,
                name: &typed_trees::name::Identifier,
            ) -> Option<typed_trees::name::Identifier> {
                if !caller.is_valid() {
                    return Some(name.clone());
                }
                let mut owner = self.owner;
                let mut frames = self.frames;
                let mut operand = name.as_str();
                let region = loop {
                    let region = lifetime_parameter_identity(program, owner, operand)?;
                    let Some(index) = frames.iter().rposition(|frame| {
                        program.lifetime_declaration_owner(frame.declaration) == region.declaration
                    }) else {
                        break region;
                    };
                    let parameter = lifetime_parameter_name(program, region)?;
                    let (_, argument) = frames[index]
                        .lifetimes
                        .iter()
                        .find(|(declared, _)| declared == parameter.as_str())?;
                    owner = index
                        .checked_sub(1)
                        .map_or(caller, |outer| frames[outer].declaration);
                    operand = argument;
                    frames = &frames[..index];
                };
                let spelling = lifetime_parameter_name(program, region)?;
                (lifetime_parameter_identity(program, caller, spelling.as_str()) == Some(region))
                    .then(|| spelling.clone())
            }
        }
    }
    pub(crate) mod inherited_arguments {
        use symbols::SymbolHandle;
        use typed_trees::TypedTrees;
        use typed_trees::machine::GenericConformanceBound;
        use typed_trees::trait_definition::{
            AppliedTraitScope, Conformance, TraitApplicationArguments,
        };
        use typed_trees::types::TypeReferenceHandle;

        pub(crate) fn materialize(
            program: &mut TypedTrees,
            selected: &Conformance,
            bound: &GenericConformanceBound,
            receiving_owner: SymbolHandle,
        ) -> Result<(Vec<TypeReferenceHandle>, Vec<TypeReferenceHandle>), String> {
            let declaration = program
                .trait_definition_by_symbol(bound.trait_symbol)
                .ok_or("evidence bound lost its trait declaration")?;
            let mut required = AppliedTraitScope::new(
                program,
                declaration,
                TraitApplicationArguments {
                    lexical_owner: receiving_owner,
                    enclosing_data_applications: &[],
                    arguments: &bound.arguments,
                    lifetimes: &bound.lifetime_arguments,
                },
            )
            .ok_or("evidence bound lost its complete application")?;
            let mut active = Vec::new();
            while let Some(base) = &required.declaration.refines {
                if active.contains(&required.declaration.symbol) {
                    return Err("cyclic evidence refinement application".into());
                }
                active.push(required.declaration.symbol);
                required = required
                    .declaring_scope(program, base.symbol)
                    .ok_or("evidence refinement has no unique nominal application")?;
            }
            let supplied = AppliedTraitScope::from_conformance(program, selected)
                .and_then(|scope| scope.declaring_scope(program, required.declaration.symbol))
                .ok_or("selected map has no unique inherited application of the required trait")?;
            let supplied = supplied.environment();
            let required = required.environment();
            let supplied = program.materialize_trait_application_arguments(&supplied)?;
            let required = program.materialize_trait_application_arguments(&required)?;
            Ok((supplied, required))
        }
    }

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
        runtime: Option<typed_trees::callable_values::RuntimeCallBinding>,
        arguments: Vec<ExpressionHandle>,
        machine_arguments: Box<[StaticMachineArgument]>,
    }

    pub(crate) fn resolve_elided_conformance_lifetimes(
        program: &mut TypedTrees,
    ) -> Result<(), Vec<Diagnostic>> {
        let calls = collect_elision_calls(program);
        let mut diagnostics = Vec::new();
        for mut call in calls {
            let runtime_signature = call
                .runtime
                .map(|binding| binding.signature(program))
                .transpose()
                .map_err(|reason| vec![Diagnostic::error(reason)])?;
            let callee_machine = call
                .runtime
                .is_none()
                .then(|| validation::find_state(program, call.target))
                .flatten();
            let requirement = program
                .closed_requirement_application(call.target)
                .map_or(call.target, |application| application.template);
            let (callee_owner, callee_lifetimes, parameters, callee_name) =
                if let Some(signature) = runtime_signature {
                    (
                        signature.symbol,
                        &signature.lifetime_parameters[..],
                        program.state_signature_parameters(signature),
                        &signature.name,
                    )
                } else if let Some((machine, state)) = callee_machine {
                    (
                        machine.symbol,
                        &machine.lifetime_parameters[..],
                        program.state_parameters(state),
                        &state.name,
                    )
                } else if let Some((_, signature)) = program.trait_machine_signature(requirement) {
                    (
                        signature.symbol,
                        &signature.lifetime_parameters[..],
                        program.state_signature_parameters(signature),
                        &signature.name,
                    )
                } else {
                    continue;
                };
            let Some(caller_machine) = program.machine_by_symbol(call.caller_machine) else {
                continue;
            };
            let caller_machine = caller_machine.symbol;
            let caller_state =
                validation::find_state_in_machine(program, call.caller_machine, call.caller_state);

            let application = match call.site {
                ElisionCallSite::Expression(handle) => expressions_of(program)
                    .call_node(handle)
                    .map(|call| call.authored_machine_application),
                ElisionCallSite::Statement(handle) => program
                    .statement_table
                    .statement(handle)
                    .as_call()
                    .map(|call| call.authored_machine_application),
            };
            let application = application.filter(|_| call.runtime.is_none());
            let mut call_lifetimes = if let Some(application) =
                application.filter(|application| application.is_valid())
            {
                program
                    .static_application_lifetimes_for_declaration(application, callee_owner)
                    .map_err(|reason| vec![Diagnostic::error(reason)])?
                    .iter()
                    .map(|binding| {
                        (
                            binding.parameter.as_str().to_owned(),
                            binding.argument.as_str().to_owned(),
                        )
                    })
                    .collect::<Vec<_>>()
            } else {
                Vec::new()
            };
            let mut conflicting_call_lifetime = false;
            let skip = parameters.len().saturating_sub(call.arguments.len());
            for (argument, parameter) in call.arguments.iter().zip(parameters.iter().skip(skip)) {
                let Some((actual, applications)) = caller_state.and_then(|state| {
                    let machine = program.machine_by_symbol(caller_machine)?;
                    argument_scopes::source_application(program, machine, state, *argument)
                }) else {
                    continue;
                };
                let required = if expressions_of(program)
                    .borrow_expression(*argument)
                    .is_some()
                {
                    match program
                        .type_reference_table
                        .type_reference(parameter.type_reference)
                    {
                        TypeReferenceNode::Reference { referee, .. } => *referee,
                        _ => parameter.type_reference,
                    }
                } else {
                    parameter.type_reference
                };
                let inferred =
                    program.with_enclosing_data_identity_scopes(&applications, |frames| {
                        visit_lifetime_bindings_in_scope(
                            program,
                            required,
                            actual,
                            callee_lifetimes,
                            argument_scopes::Scope {
                                owner: caller_machine,
                                frames,
                            },
                            caller_machine,
                            RequiredLifetimeScope::Operand,
                            &mut |required, actual| {
                                if let Some(actual) = actual {
                                    bind_lifetime(
                                        &mut call_lifetimes,
                                        required.as_str(),
                                        actual.as_str(),
                                        &mut conflicting_call_lifetime,
                                    );
                                }
                            },
                        );
                    });
                if inferred.is_none() {
                    continue;
                }
            }
            if conflicting_call_lifetime {
                diagnostics.push(Diagnostic::error(format!(
                "call to `{}` has conflicting ordinary borrow constraints for an erased lifetime",
                callee_name
            )));
                continue;
            }

            let evidence_bounds = if let Some(signature) = runtime_signature {
                signature.conformance_bounds.clone()
            } else {
                callee_machine
                    .map_or_else(Vec::new, |(machine, _)| machine.conformance_bounds.clone())
            };
            let machine_parameters = if let Some(signature) = runtime_signature {
                let mut parameters = program.state_signature_type_parameters(signature).to_vec();
                for bound in &signature.conformance_bounds {
                    if !parameters
                        .iter()
                        .any(|parameter| parameter.symbol == bound.binder)
                    {
                        parameters.push(typed_trees::data::TypeParameter {
                            symbol: bound.binder,
                            name: bound.binder_name.clone(),
                            kind: typed_trees::data::TypeParameterKind::Implementation,
                            ..Default::default()
                        });
                    }
                }
                parameters
            } else {
                callee_machine.map_or_else(Vec::new, |(machine, _)| {
                    program.machine_callable_parameters(machine)
                })
            };
            let callee_machine = callee_machine
                .map(|(machine, _)| machine.symbol)
                .or_else(|| runtime_signature.map(|signature| signature.symbol));
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
                let Some(callee_machine) = callee_machine else {
                    break;
                };
                if !matches!(
                    program.symbols.get(selected.symbol).kind,
                    SymbolKind::Conformance
                ) {
                    continue;
                }
                let Some(conformance) = program.conformance_by_symbol(selected.symbol).cloned()
                else {
                    continue;
                };
                if conformance.lifetime_parameters.is_empty() {
                    continue;
                }
                // Include selected implementation binders in the same telescope as
                // ordinary type parameters, for both named and runtime signatures.
                let (declared_arguments, expected_arguments) =
                    match inherited_arguments::materialize(
                        program,
                        &conformance,
                        bound,
                        callee_machine,
                    ) {
                        Ok(arguments) => arguments,
                        Err(reason) => {
                            diagnostics.push(Diagnostic::error(format!(
                        "conformance `{}` cannot retain its inherited application for call lifetime constraints: {reason}",
                        program.symbols.name(conformance.symbol),
                    )));
                            continue;
                        }
                    };
                let mut inferred = Vec::<(String, String)>::new();
                let mut conflict = false;
                for (declared, expected) in declared_arguments.iter().zip(&expected_arguments) {
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

                let machine_substitutions =
                    static_substitutions(&machine_parameters, &call.machine_arguments);
                let conformance_substitutions = static_substitutions(
                    program.conformance_type_parameters(&conformance),
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
                let mut mismatched_argument = None;
                let shapes_match = declared_arguments.len() == expected_arguments.len()
                    && declared_arguments
                        .iter()
                        .zip(&expected_arguments)
                        .enumerate()
                        .all(|(ordinal, (declared, expected))| {
                            let declared_identity =
                                typed_trees::type_identity::applied_type_identity_in_scope(
                                    program,
                                    *declared,
                                    conformance.symbol,
                                    call.caller_machine,
                                    &[AppliedTypeIdentityScope {
                                        declaration: conformance.symbol,
                                        arguments: &conformance_substitutions,
                                        lifetimes: &inferred_substitutions,
                                    }],
                                );
                            let expected_identity =
                                typed_trees::type_identity::applied_type_identity_in_scope(
                                    program,
                                    *expected,
                                    callee_machine,
                                    call.caller_machine,
                                    &[AppliedTypeIdentityScope {
                                        declaration: callee_machine,
                                        arguments: &machine_substitutions,
                                        lifetimes: &call_lifetimes,
                                    }],
                                );
                            let matches = declared_identity
                                .zip(expected_identity)
                                .is_some_and(|(declared, expected)| declared == expected);
                            if !matches {
                                mismatched_argument =
                                    Some((ordinal, declared_identity, expected_identity));
                            }
                            matches
                        });
                if !shapes_match {
                    if selected
                        .application
                        .as_ref()
                        .is_some_and(|application| !application.lifetime_arguments.is_empty())
                    {
                        let detail = mismatched_argument.map_or_else(
                            || {
                                format!(
                                    "argument count {} instead of {}",
                                    declared_arguments.len(),
                                    expected_arguments.len()
                                )
                            },
                            |(ordinal, declared, expected)| {
                                format!(
                                    "argument {ordinal}: selected {:?}, required {:?}",
                                    declared
                                        .and_then(|identity| program.type_identity_text(identity)),
                                    expected
                                        .and_then(|identity| program.type_identity_text(identity)),
                                )
                            },
                        );
                        diagnostics.push(Diagnostic::error(format!(
                        "conformance `{}` supplies lifetime arguments that disagree with the call's ordinary borrow constraints at `{}` in `{}`: {detail}",
                        conformance.alias.as_ref().map_or("<unnamed>", |name| name.as_str()),
                        program.symbols.name(callee_machine),
                        program.symbols.display_path(call.caller_machine, "::"),
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
                let binding_declaration = program.lifetime_declaration_owner(callee_owner);
                let lifetimes = call_lifetimes
                    .iter()
                    .map(|(parameter, argument)| StaticMachineLifetimeBinding {
                        parameter: Identifier::generated(parameter),
                        argument: Identifier::generated(argument),
                    })
                    .collect::<Vec<_>>();
                let source =
                    expressions_of_mut(program).static_machine_application_mut(application);
                if !source.binding_declaration.is_valid() {
                    source.binding_declaration = binding_declaration;
                }
                expressions_of_mut(program).set_static_machine_application_lifetimes(
                    application,
                    binding_declaration,
                    &lifetimes,
                );
            }
            match call.site {
                ElisionCallSite::Expression(handle) => {
                    let ExpressionNode::Call(current) =
                        expressions_of_mut(program).expression_mut(handle)
                    else {
                        continue;
                    };
                    current.machine_arguments = call.machine_arguments;
                }
                ElisionCallSite::Statement(handle) => {
                    let StatementNode::Call(current) =
                        program.statement_table.statement_mut(handle)
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
                let walk = ElisionWalk {
                    program,
                    caller_machine: machine.symbol,
                    caller_state: state.symbol,
                };
                for handle in state.statement_nodes.handles() {
                    match program.statement_table.statement(handle) {
                        StatementNode::Call(call) => {
                            calls.push(ElisionCall {
                                site: ElisionCallSite::Statement(handle),
                                caller_machine: machine.symbol,
                                caller_state: state.symbol,
                                target: call.target_symbol,
                                runtime: call.runtime_call,
                                arguments: program
                                    .statement_table
                                    .expression_handles(call.arguments)
                                    .to_vec(),
                                machine_arguments: call.machine_arguments.clone(),
                            });
                            if let Some(runtime) = call.runtime_call {
                                walk.visit(runtime.callee, &mut calls);
                            }
                            for argument in
                                program.statement_table.expression_handles(call.arguments)
                            {
                                walk.visit(*argument, &mut calls);
                            }
                        }
                        StatementNode::Expression(expression) => {
                            walk.visit(*expression, &mut calls)
                        }
                        StatementNode::LocalData(local) => {
                            walk.visit(local.initial_value, &mut calls)
                        }
                        StatementNode::Assignment(assignment) => {
                            walk.visit(assignment.target, &mut calls);
                            walk.visit(assignment.value, &mut calls);
                        }
                        StatementNode::AssemblyFact(fact) => {
                            for expression in program.tables.proof_fact_expression_roots(fact.fact)
                            {
                                walk.visit(expression, &mut calls);
                            }
                        }
                        statement @ StatementNode::Transition(_) => {
                            for (_, expression) in program.tables.expression_roots(statement) {
                                walk.visit(expression, &mut calls);
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        calls
    }

    struct ElisionWalk<'a> {
        program: &'a dyn ::validation::CheckingInput,
        caller_machine: SymbolHandle,
        caller_state: SymbolHandle,
    }

    impl ElisionWalk<'_> {
        fn visit(&self, expression: ExpressionHandle, calls: &mut Vec<ElisionCall>) {
            if !expression.is_valid()
            || calls.iter().any(
                |call| matches!(call.site, ElisionCallSite::Expression(handle) if handle == expression),
            )
        {
            return;
        }
            match expressions_of(self.program).expression(expression) {
                // Selecting code has no evaluated receiver or invocation children.
                ExpressionNode::SavedSubject(_) | ExpressionNode::MachineValue(_) => {}
                ExpressionNode::Call(call) => {
                    calls.push(ElisionCall {
                        site: ElisionCallSite::Expression(expression),
                        caller_machine: self.caller_machine,
                        caller_state: self.caller_state,
                        target: call.target_symbol,
                        runtime: call.runtime_call,
                        arguments: expressions_of(self.program)
                            .expression_handles(call.arguments)
                            .to_vec(),
                        machine_arguments: call.machine_arguments.clone(),
                    });
                    self.visit(call.evaluated_callee(), calls);
                    for argument in expressions_of(self.program).expression_handles(call.arguments)
                    {
                        self.visit(*argument, calls);
                    }
                }
                ExpressionNode::ArrayLiteral(values) => {
                    for value in expressions_of(self.program).expression_handles(*values) {
                        self.visit(*value, calls);
                    }
                }
                ExpressionNode::Atomic(atomic) => {
                    self.visit(atomic.value, calls);
                    self.visit(atomic.result, calls);
                }
                ExpressionNode::Match(dispatch) => {
                    expressions_of(self.program).match_deep_all(dispatch, |handle| {
                        self.visit(handle, calls);
                        true
                    });
                }
                ExpressionNode::Binary(binary) => {
                    self.visit(binary.left, calls);
                    self.visit(binary.right, calls);
                }
                ExpressionNode::Cast(cast) => self.visit(cast.value, calls),
                ExpressionNode::Indexed(indexed) => {
                    self.visit(indexed.collection, calls);
                    self.visit(indexed.index, calls);
                }
                ExpressionNode::Member(member) => self.visit(member.receiver, calls),
                ExpressionNode::Borrow(inner) => self.visit(inner.target, calls),
                ExpressionNode::Range(range) => {
                    self.visit(range.start, calls);
                    self.visit(range.end, calls);
                }
                ExpressionNode::StructLiteral(literal) => {
                    for field in expressions_of(self.program).struct_fields(literal.fields) {
                        self.visit(field.value, calls);
                    }
                }
                ExpressionNode::Unary(unary) => self.visit(unary.operand, calls),
                ExpressionNode::TypeExpression(_) | typed_trees::literal_expression_pattern!() => {}
            }
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

    #[derive(Clone, Copy)]
    enum RequiredLifetimeScope {
        Operand,
        DeclaredField(SymbolHandle),
    }

    pub(crate) fn collect_declared_field_lifetime_bindings(
        program: &TypedTrees,
        declaration: &typed_trees::data::DataDefinition,
        field: &typed_trees::data::DataField,
        caller: SymbolHandle,
        applications: &[TypeReferenceHandle],
        actual: TypeReferenceHandle,
        bindings: &mut Vec<(String, String)>,
        conflict: &mut bool,
    ) -> Option<()> {
        if !caller.is_valid() {
            return None;
        }
        let (owner, selected) = program.any_field_owner(field.symbol)?;
        if owner.symbol != declaration.symbol || selected.type_reference != field.type_reference {
            return None;
        }
        if declaration.lifetime_parameters.is_empty() {
            return Some(());
        }
        let required_scope = if declaration.generic_instance.is_none() {
            RequiredLifetimeScope::DeclaredField(declaration.symbol)
        } else {
            RequiredLifetimeScope::Operand
        };
        program.with_enclosing_data_identity_scopes(applications, |frames| {
            visit_lifetime_bindings_in_scope(
                program,
                field.type_reference,
                actual,
                &declaration.lifetime_parameters,
                argument_scopes::Scope {
                    owner: caller,
                    frames,
                },
                caller,
                required_scope,
                &mut |required, actual| {
                    if let Some(actual) = actual {
                        bind_lifetime(bindings, required.as_str(), actual.as_str(), conflict);
                    }
                },
            );
        })
    }

    pub(crate) fn visit_lifetime_bindings(
        program: &dyn ::validation::CheckingInput,
        required: TypeReferenceHandle,
        actual: TypeReferenceHandle,
        parameters: &[Identifier],
        visit: &mut impl FnMut(&Identifier, Option<&Identifier>),
    ) {
        visit_lifetime_bindings_in_scope(
            program,
            required,
            actual,
            parameters,
            argument_scopes::Scope {
                owner: SymbolHandle::invalid(),
                frames: &[],
            },
            SymbolHandle::invalid(),
            RequiredLifetimeScope::Operand,
            visit,
        );
    }

    fn visit_lifetime_bindings_in_scope(
        program: &dyn validation::CheckingInput,
        required: TypeReferenceHandle,
        actual: TypeReferenceHandle,
        parameters: &[Identifier],
        actual_scope: argument_scopes::Scope<'_>,
        caller: SymbolHandle,
        required_scope: RequiredLifetimeScope,
        visit: &mut impl FnMut(&Identifier, Option<&Identifier>),
    ) {
        macro_rules! visit_binding {
            ($required:expr, $actual:expr $(,)?) => {
                visit_lifetime_bindings_in_scope(
                    program,
                    $required,
                    $actual,
                    parameters,
                    actual_scope,
                    caller,
                    required_scope,
                    visit,
                )
            };
        }
        let owner = program
            .type_reference_table
            .operand_lifetime_owner(required);
        if owner.is_valid()
            && !matches!(required_scope, RequiredLifetimeScope::DeclaredField(declaration) if owner == declaration)
        {
            return;
        }
        if matches!(required_scope, RequiredLifetimeScope::DeclaredField(_))
            && !matches!(
                program.type_reference_table.operand_origin(required),
                typed_trees::types::TypeOperandOrigin::Lexical
            )
        {
            return;
        }
        let Some((actual, actual_scope)) = actual_scope.resolve(program, actual) else {
            return;
        };
        let actual_nominal = match program.type_reference_table.type_reference(actual) {
            TypeReferenceNode::Generic {
                base_symbol,
                lifetime_arguments,
                arguments,
                ..
            } => Some((
                *base_symbol,
                lifetime_arguments.as_slice(),
                program
                    .type_reference_table
                    .type_reference_handles(*arguments),
            )),
            TypeReferenceNode::Named { symbol, .. } => Some((*symbol, &[][..], &[][..])),
            _ => None,
        };
        if let TypeReferenceNode::Generic {
            base_symbol: required_base,
            lifetime_arguments: required_lifetimes,
            arguments: required_arguments,
            ..
        } = program.type_reference_table.type_reference(required)
            && let Some((actual_base, actual_lifetimes, actual_arguments)) = actual_nominal
            && required_base.is_valid()
            && *required_base == actual_base
        {
            for (position, required) in required_lifetimes.iter().enumerate() {
                if parameters
                    .iter()
                    .any(|parameter| parameter.as_str() == required.as_str())
                {
                    let actual = actual_lifetimes
                        .get(position)
                        .and_then(|actual| actual_scope.argument(program, caller, actual));
                    visit(required, actual.as_ref());
                }
            }
            for (required, actual) in program
                .type_reference_table
                .type_reference_handles(*required_arguments)
                .iter()
                .zip(actual_arguments)
            {
                visit_binding!(*required, *actual);
            }
            return;
        }
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
                if let Some(required) = required_lifetime
                    && parameters
                        .iter()
                        .any(|parameter| parameter.as_str() == required.as_str())
                {
                    let actual = actual_lifetime
                        .as_ref()
                        .and_then(|actual| actual_scope.argument(program, caller, actual));
                    visit(required, actual.as_ref());
                }
                visit_binding!(*required_referee, *actual_referee);
            }
            typed_trees::constrained_type_reference_pair!(required, actual) => {
                visit_binding!(*required, *actual)
            }
            (TypeReferenceNode::Constrained { base_type, .. }, _) => {
                visit_binding!(*base_type, actual)
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
            ) => visit_binding!(*required, *actual),
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
                        && let Some(expected) =
                            resolved_lifetime(expected_lifetimes, expected.as_str())
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
}
pub(crate) mod explicit_requirement_calls {
    use crate::monomorphization::insert_subject_name;
    use crate::tables::{expressions_of, expressions_of_mut};
    use diagnostics::Diagnostic;
    use symbols::{SymbolHandle, SymbolKind};
    use typed_trees::TypedTrees;
    use typed_trees::expression::{ExpressionHandle, ExpressionNode, StaticMachineArgument};
    use typed_trees::name::Identifier;
    use typed_trees::statement::{StatementHandle, StatementNode};
    use typed_trees::static_applications::StaticConformanceSelection;
    use typed_trees::static_applications::StaticMachineApplicationHandle;
    use typed_trees::typed_trees::StaticRequirementDispatch;
    use typed_trees::typed_trees::{
        ClosedConformanceApplication, ClosedConformanceApplicationCommitment,
    };
    use typed_trees::types::TypeReferenceHandle;
    use validation::conformance_applications::close_conformance_application;

    pub(crate) mod application_arguments {
        use symbols::SymbolHandle;
        use typed_trees::TypedTrees;
        use typed_trees::data::TypeParameter;
        use typed_trees::type_identity::{
            AppliedTypeIdentityScope, IndexIdentityArgument, NormalizedTypeIdentity,
            applied_type_identity,
        };
        use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

        #[derive(Clone)]
        pub(crate) struct ApplicationScope {
            declaration: SymbolHandle,
            arguments: Vec<(SymbolHandle, TypeReferenceHandle, bool)>,
            lifetimes: Vec<(String, String)>,
        }

        impl ApplicationScope {
            pub(crate) fn new(
                declaration: SymbolHandle,
                parameters: &[TypeParameter],
                arguments: &[TypeReferenceHandle],
                lifetimes: Vec<(String, String)>,
            ) -> Option<Self> {
                if parameters.len() != arguments.len() {
                    return None;
                }
                Some(Self {
                    declaration,
                    arguments: parameters
                        .iter()
                        .zip(arguments)
                        .map(|(parameter, argument)| {
                            (
                                parameter.symbol,
                                *argument,
                                parameter.kind.is_const_or_value(),
                            )
                        })
                        .collect(),
                    lifetimes,
                })
            }
        }

        pub(crate) struct ApplicationArguments {
            pub(crate) declaration: SymbolHandle,
            pub(crate) arguments: Vec<TypeReferenceHandle>,
            pub(crate) lifetimes: Vec<String>,
            pub(crate) scopes: Vec<ApplicationScope>,
        }

        pub(crate) fn matches_declaring_application(
            program: &mut TypedTrees,
            lexical_owner: SymbolHandle,
            declaring_trait: SymbolHandle,
            supplied: &[TypeReferenceHandle],
            initial: ApplicationArguments,
        ) -> Option<Result<(), String>> {
            let supplied =
                argument_identities(program, lexical_owner, declaring_trait, supplied, &[])?;
            let mut pending = vec![(initial, Vec::new())];
            let mut matched = false;
            while let Some((application, mut ancestors)) = pending.pop() {
                if application.declaration == declaring_trait {
                    let expected = argument_identities(
                        program,
                        lexical_owner,
                        declaring_trait,
                        &application.arguments,
                        &application.scopes,
                    )?;
                    if expected != supplied {
                        let render = |identities: &[NormalizedTypeIdentity]| {
                            identities
                                .iter()
                                .map(|identity| {
                                    program.type_identity_text(*identity).unwrap_or_default()
                                })
                                .collect::<Vec<_>>()
                                .join(", ")
                        };
                        return Some(Err(format!(
                            "expected [{}], supplied [{}]",
                            render(&expected),
                            render(&supplied),
                        )));
                    }
                    matched = true;
                    continue;
                }
                if ancestors.contains(&application.declaration) {
                    continue;
                }
                ancestors.push(application.declaration);
                let definition = program.trait_definition_by_symbol(application.declaration)?;
                if definition.lifetime_parameters.len() != application.lifetimes.len() {
                    return None;
                }
                let scope = ApplicationScope::new(
                    definition.symbol,
                    program.trait_type_parameters(definition),
                    &application.arguments,
                    definition
                        .lifetime_parameters
                        .iter()
                        .map(|parameter| parameter.as_str().to_owned())
                        .zip(application.lifetimes)
                        .collect(),
                )?;
                let mut scopes = application.scopes;
                scopes.push(scope);
                for parent in program
                    .trait_requirements(definition)
                    .iter()
                    .chain(definition.refines.iter())
                {
                    pending.push((
                        ApplicationArguments {
                            declaration: parent.symbol,
                            arguments: program
                                .type_reference_table
                                .type_reference_handles(parent.arguments)
                                .to_vec(),
                            lifetimes: parent
                                .lifetime_arguments
                                .iter()
                                .map(|argument| argument.as_str().to_owned())
                                .collect(),
                            scopes: scopes.clone(),
                        },
                        ancestors.clone(),
                    ));
                }
            }
            matched.then_some(Ok(()))
        }

        fn argument_identities(
            program: &mut TypedTrees,
            lexical_owner: SymbolHandle,
            declaration: SymbolHandle,
            arguments: &[TypeReferenceHandle],
            scopes: &[ApplicationScope],
        ) -> Option<Vec<NormalizedTypeIdentity>> {
            let definition = program.trait_definition_by_symbol(declaration)?;
            let parameters = program.trait_type_parameters(definition).to_vec();
            let final_scope =
                ApplicationScope::new(declaration, &parameters, arguments, Vec::new())?;
            let owned_scopes = scopes
                .iter()
                .chain(std::iter::once(&final_scope))
                .collect::<Vec<_>>();
            let operands = owned_scopes
                .iter()
                .map(|scope| {
                    scope
                        .arguments
                        .iter()
                        .map(|(symbol, reference, value)| {
                            (
                                *symbol,
                                if *value {
                                    IndexIdentityArgument::TypeReference(*reference)
                                } else {
                                    IndexIdentityArgument::TypeArgument(*reference)
                                },
                            )
                        })
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();
            let scopes = owned_scopes
                .iter()
                .zip(&operands)
                .map(|(scope, arguments)| AppliedTypeIdentityScope {
                    declaration: scope.declaration,
                    arguments,
                    lifetimes: &scope.lifetimes,
                })
                .collect::<Vec<_>>();
            parameters
                .iter()
                .map(|parameter| {
                    let reference = program
                        .type_reference_table
                        .insert(TypeReferenceNode::Named {
                            symbol: parameter.symbol,
                            name: parameter.name.clone(),
                        });
                    applied_type_identity(program, reference, lexical_owner, &scopes)
                })
                .collect()
        }
    }
    use application_arguments::{ApplicationArguments, ApplicationScope};

    #[derive(Clone, Copy)]
    enum RequirementCallSite {
        Expression(ExpressionHandle),
        Statement(StatementHandle),
    }

    #[derive(Clone, Copy)]
    struct RequirementCallOccurrence {
        site: RequirementCallSite,
        application: StaticMachineApplicationHandle,
    }

    enum ImplementationSelection {
        StaticOperand,
        Qualifier(SymbolHandle),
    }

    struct ExplicitRequirementRewrite {
        call: RequirementCallOccurrence,
        target: SymbolHandle,
        name: Identifier,
        application_arguments: Box<[StaticMachineArgument]>,
        dispatch: Option<StaticRequirementDispatch>,
        implementation_receiver: Option<(Identifier, SymbolHandle)>,
    }

    pub(crate) fn route_explicit_requirement_calls(
        program: &mut TypedTrees,
    ) -> Result<(), Vec<Diagnostic>> {
        let mut requirements = Vec::new();
        for definition in program.traits() {
            for signature in program.trait_machine_signatures(definition) {
                requirements.push((
                    signature.symbol,
                    definition.symbol,
                    program.state_signature_type_parameters(signature),
                ));
            }
        }
        if requirements.is_empty() {
            return Ok(());
        }
        let mut rewrites = Vec::new();
        let mut diagnostics = Vec::new();
        let mut calls = Vec::new();
        let mut collect = |occurrence: RequirementCallOccurrence,
                           target,
                           qualifier,
                           arguments: &[StaticMachineArgument]| {
            let Some((requirement, declaring_trait, invocation_parameters)) = requirements
                .iter()
                .find(|(requirement, _, _)| *requirement == target)
            else {
                return;
            };
            let qualified = program.conformance_by_symbol(qualifier).is_some();
            let invocation_arity = invocation_parameters.len();
            let has_selection_operand = arguments.last().is_some_and(|argument| {
                argument.names_implementation(&program.symbols)
                    && invocation_parameters
                        .get(arguments.len() - 1)
                        .is_none_or(|parameter| {
                            parameter.kind != typed_trees::data::TypeParameterKind::Implementation
                        })
            });
            if (qualified || !has_selection_operand) && arguments.len() > invocation_arity {
                let source_span = match occurrence.site {
                    RequirementCallSite::Expression(handle) => {
                        expressions_of(program).source_span(handle)
                    }
                    RequirementCallSite::Statement(handle) => program
                        .statement_table
                        .statement(handle)
                        .as_call()
                        .map(|call| call.source_span)
                        .unwrap_or_default(),
                };
                let mut message = format!(
                    "`{}` expects {} static invocation argument(s), got {}",
                    program.symbols.display_path(*requirement, "::"),
                    invocation_arity,
                    arguments.len(),
                );
                if !qualified {
                    message.push_str("; explicit selection requires a named implementation as the final static argument");
                }
                diagnostics.push(Diagnostic::error(message).with_source_span(source_span));
                return;
            }
            let selection = if qualified {
                ImplementationSelection::Qualifier(qualifier)
            } else if has_selection_operand {
                ImplementationSelection::StaticOperand
            } else {
                return;
            };
            calls.push((
                occurrence,
                *requirement,
                *declaring_trait,
                arguments.to_vec(),
                selection,
            ));
        };
        for (handle, node) in expressions_of(program).iter_expressions() {
            if let ExpressionNode::Call(call) = node {
                collect(
                    RequirementCallOccurrence {
                        site: RequirementCallSite::Expression(handle),
                        application: call.authored_machine_application,
                    },
                    call.target_symbol,
                    expressions_of(program)
                        .name_path_node(call.receiver)
                        .map_or_else(SymbolHandle::invalid, |path| path.symbol),
                    &call.machine_arguments,
                );
            }
        }
        for machine in program.machines() {
            for state in program.machine_states(machine) {
                for (handle, statement) in program
                    .statement_table
                    .iter_statements(state.statement_nodes)
                {
                    if let Some(call) = statement.as_call() {
                        collect(
                            RequirementCallOccurrence {
                                site: RequirementCallSite::Statement(handle),
                                application: call.authored_machine_application,
                            },
                            call.target_symbol,
                            call.receiver_symbol,
                            &call.machine_arguments,
                        );
                    }
                }
            }
        }
        for (mut occurrence, requirement, declaring_trait, mut arguments, selection) in calls {
            let source_span = match occurrence.site {
                RequirementCallSite::Expression(handle) => {
                    expressions_of(program).source_span(handle)
                }
                RequirementCallSite::Statement(handle) => program
                    .statement_table
                    .statement(handle)
                    .as_call()
                    .map(|call| call.source_span)
                    .unwrap_or_default(),
            };
            let mut implementation_receiver = None;
            let execution_arguments = match selection {
                ImplementationSelection::StaticOperand => Box::default(),
                ImplementationSelection::Qualifier(implementation) => {
                    implementation_receiver = Some((
                        Identifier::generated(program.symbols.name(implementation)),
                        implementation,
                    ));
                    let invocation_arguments = arguments.into_boxed_slice();
                    let normalized = qualified_selection_arguments(
                        program,
                        occurrence,
                        requirement,
                        declaring_trait,
                        implementation,
                    );
                    match normalized {
                        Ok((application, selected)) => {
                            occurrence.application = application;
                            arguments = selected;
                        }
                        Err(message) => {
                            diagnostics
                                .push(Diagnostic::error(message).with_source_span(source_span));
                            continue;
                        }
                    }
                    invocation_arguments
                }
            };
            match explicit_requirement_rewrite(
                program,
                occurrence,
                requirement,
                declaring_trait,
                &arguments,
            ) {
                Ok(mut rewrite) => {
                    if let Some(dispatch) = &rewrite.dispatch
                        && dispatch.selection_application.is_valid()
                        && let Err(mut errors) =
                            crate::monomorphization::prepare_conformance_occurrence(
                                program,
                                dispatch.selection_application,
                            )
                    {
                        diagnostics.append(&mut errors);
                        continue;
                    }
                    rewrite.application_arguments = execution_arguments;
                    if implementation_receiver.is_some() {
                        rewrite.implementation_receiver = implementation_receiver;
                    }
                    rewrites.push(rewrite);
                }
                Err(message) => {
                    diagnostics.push(Diagnostic::error(message).with_source_span(source_span))
                }
            }
        }
        if !diagnostics.is_empty() {
            return Err(diagnostics);
        }
        for rewrite in rewrites {
            match rewrite.call.site {
                RequirementCallSite::Expression(handle) => {
                    let receiver = rewrite.implementation_receiver.map_or_else(
                        ExpressionHandle::invalid,
                        |(name, binder)| {
                            insert_subject_name(expressions_of_mut(program), name, binder)
                        },
                    );
                    let ExpressionNode::Call(call) =
                        expressions_of_mut(program).expression_mut(handle)
                    else {
                        continue;
                    };
                    call.target_symbol = rewrite.target;
                    call.target = rewrite.name;
                    call.receiver = receiver;
                    call.requirement_application = TypeReferenceHandle::invalid();
                    call.machine_arguments = rewrite.application_arguments;
                    call.static_requirement_dispatch = rewrite.dispatch;
                }
                RequirementCallSite::Statement(handle) => {
                    let mut receiver = arena::HandleSpan::default();
                    let binder = if let Some((name, binder)) = rewrite.implementation_receiver {
                        program
                            .statement_table
                            .push_name_path_member(&mut receiver, name);
                        binder
                    } else {
                        SymbolHandle::invalid()
                    };
                    let StatementNode::Call(call) = program.statement_table.statement_mut(handle)
                    else {
                        continue;
                    };
                    call.target_symbol = rewrite.target;
                    call.target = rewrite.name;
                    call.receiver = receiver;
                    call.receiver_root_symbol = binder;
                    call.receiver_symbol = binder;
                    call.machine_arguments = rewrite.application_arguments;
                    call.static_requirement_dispatch = rewrite.dispatch;
                }
            }
        }
        Ok(())
    }

    fn qualified_selection_arguments(
        program: &mut TypedTrees,
        occurrence: RequirementCallOccurrence,
        requirement: SymbolHandle,
        declaring_trait: SymbolHandle,
        implementation: SymbolHandle,
    ) -> Result<(StaticMachineApplicationHandle, Vec<StaticMachineArgument>), String> {
        if !occurrence.application.is_valid() {
            return Err("named implementation call lost its authored application".to_owned());
        }
        let owner = expressions_of(program)
            .static_machine_application(occurrence.application)
            .lexical_owner;
        let retained = retained_qualifier_selection(
            program,
            occurrence,
            owner,
            requirement,
            declaring_trait,
            implementation,
        );
        let qualifier = match occurrence.site {
            RequirementCallSite::Expression(handle) => expressions_of(program)
                .call_node(handle)
                .map(|call| call.requirement_application)
                .unwrap_or_default(),
            RequirementCallSite::Statement(_) => TypeReferenceHandle::invalid(),
        };
        if !qualifier.is_valid()
            && let Some(application) = retained
        {
            return Ok((
                application,
                expressions_of(program)
                    .static_machine_application_arguments(application)
                    .to_vec(),
            ));
        }
        let selected = if qualifier.is_valid() {
            let mut selected = program
                .static_operand_from_type_reference(qualifier)
                .map_err(str::to_owned)?;
            selected.type_reference = qualifier;
            selected
        } else {
            StaticMachineArgument {
                symbol: implementation,
                path: Box::from([Identifier::generated(program.symbols.name(implementation))]),
                ..StaticMachineArgument::default()
            }
        };
        if selected.symbol != implementation {
            return Err(format!(
                "named implementation qualifier `{}` names another map than receiver `{}`",
                program.symbols.display_path(selected.symbol, "::"),
                program.symbols.display_path(implementation, "::"),
            ));
        }
        close_conformance_application(program, &selected).map_err(|diagnostic| {
            format!(
                "{} (selected call `{}::{}` in `{}`)",
                diagnostic.message,
                program.symbols.display_path(implementation, "::"),
                program.symbols.name(requirement),
                program.symbols.display_path(owner, "::"),
            )
        })?;
        let supplier = program
            .static_operand_region_supplier(&selected, owner)
            .map_err(str::to_owned)?;
        let captures = selected
            .application
            .as_ref()
            .map_or(&[][..], |application| &application.arguments)
            .iter()
            .map(|argument| {
                program
                    .static_operand_type_reference(argument)
                    .map_err(str::to_owned)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let lifetimes = selected
            .application
            .as_ref()
            .map_or(&[][..], |application| &application.lifetime_arguments);
        let conformance = program
            .conformance_by_symbol(implementation)
            .ok_or("named implementation call lost its selected map")?;
        let environment =
            typed_trees::trait_definition::AppliedTraitScope::from_conformance_application(
                program,
                conformance,
                typed_trees::trait_definition::TraitApplicationArguments {
                    lexical_owner: supplier,
                    enclosing_data_applications: &[],
                    arguments: &captures,
                    lifetimes,
                },
            )
            .ok_or("named implementation qualifier does not supply its complete telescope")?
            .declaring_scope(program, declaring_trait)
            .map(|scope| scope.environment())
            .ok_or("named implementation call lost its exact declaring application")?;
        let references = program
            .materialize_trait_application_arguments(&environment)
            .map_err(str::to_owned)?;
        let mut arguments = references
            .into_iter()
            .map(|reference| {
                program
                    .static_operand_from_type_reference(reference)
                    .map_err(str::to_owned)
            })
            .collect::<Result<Vec<_>, _>>()?;
        arguments.push(selected);
        if let Some(application) = retained
            && expressions_of(program).static_machine_application_arguments(application)
                == arguments.as_slice()
        {
            return Ok((application, arguments));
        }
        let requirement_name = Identifier::generated(program.symbols.name(requirement));
        let application = expressions_of_mut(program).retain_static_machine_application(
            requirement,
            requirement_name,
            &arguments,
        );
        expressions_of_mut(program)
            .static_machine_application_mut(application)
            .lexical_owner = owner;
        Ok((application, arguments))
    }

    fn retained_qualifier_selection(
        program: &TypedTrees,
        occurrence: RequirementCallOccurrence,
        owner: SymbolHandle,
        requirement: SymbolHandle,
        declaring_trait: SymbolHandle,
        implementation: SymbolHandle,
    ) -> Option<StaticMachineApplicationHandle> {
        let dispatch = match occurrence.site {
            RequirementCallSite::Expression(handle) => expressions_of(program)
                .call_node(handle)?
                .static_requirement_dispatch
                .as_ref()?,
            RequirementCallSite::Statement(handle) => program
                .statement_table
                .statement(handle)
                .as_call()?
                .static_requirement_dispatch
                .as_ref()?,
        };
        if dispatch.requirement != requirement || dispatch.declaring_trait != declaring_trait {
            return None;
        }
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
        if application.declaration != implementation || !dispatch.selection_application.is_valid() {
            return None;
        }
        let source =
            expressions_of(program).static_machine_application(dispatch.selection_application);
        let selection = source.conformance_selection.as_ref()?;
        let arguments = expressions_of(program)
            .static_machine_application_arguments(dispatch.selection_application);
        let ordinal = selection.operand_ordinal as usize;
        if ordinal.checked_add(1) != Some(arguments.len()) {
            return None;
        }
        Some(dispatch.selection_application)
    }

    fn explicit_requirement_rewrite(
        program: &mut TypedTrees,
        call: RequirementCallOccurrence,
        requirement: SymbolHandle,
        declaring_trait: SymbolHandle,
        arguments: &[StaticMachineArgument],
    ) -> Result<ExplicitRequirementRewrite, String> {
        let requirement_name = program.symbols.display_path(requirement, "::");
        let Some((implementation, trait_arguments)) = arguments.split_last() else {
            return Err(format!("`{requirement_name}` names no implementation"));
        };
        let definition = program
            .trait_definition_by_symbol(declaring_trait)
            .ok_or_else(|| format!("`{requirement_name}` lost its declaring trait"))?;
        let parameters = program.trait_type_parameters(definition);
        if !implementation.names_implementation(&program.symbols)
            || trait_arguments.len() != parameters.len()
            || parameters
                .iter()
                .zip(trait_arguments)
                .any(|(parameter, argument)| {
                    (parameter.kind == typed_trees::data::TypeParameterKind::Implementation)
                        != argument.names_implementation(&program.symbols)
                })
        {
            return Err(format!(
                "`{requirement_name}` names its implementation as its one last static argument, after the trait's arguments"
            ));
        }
        let trait_references = trait_arguments
            .iter()
            .map(|argument| {
                program
                    .static_operand_type_reference(argument)
                    .map_err(str::to_owned)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let implementation_references = implementation
            .application
            .as_ref()
            .map_or(&[][..], |application| &application.arguments)
            .iter()
            .map(|argument| {
                program
                    .static_operand_type_reference(argument)
                    .map_err(str::to_owned)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let lexical_owner = if call.application.is_valid() {
            expressions_of(program)
                .static_machine_application(call.application)
                .lexical_owner
        } else {
            SymbolHandle::invalid()
        };
        let mut request = ExplicitApplication {
            program,
            call,
            requirement,
            requirement_name,
            declaring_trait,
            trait_arguments,
            implementation,
            trait_references,
            implementation_references,
            lexical_owner,
        };
        if request.program.symbols.get(implementation.symbol).kind
            == SymbolKind::ConformanceParameter
        {
            request.through_binder()
        } else {
            request.through_implementation()
        }
    }

    struct ExplicitApplication<'a> {
        program: &'a mut TypedTrees,
        call: RequirementCallOccurrence,
        requirement: SymbolHandle,
        requirement_name: String,
        declaring_trait: SymbolHandle,
        trait_arguments: &'a [StaticMachineArgument],
        implementation: &'a StaticMachineArgument,
        trait_references: Vec<TypeReferenceHandle>,
        implementation_references: Vec<TypeReferenceHandle>,
        lexical_owner: SymbolHandle,
    }

    impl ExplicitApplication<'_> {
        fn through_implementation(&mut self) -> Result<ExplicitRequirementRewrite, String> {
            let program = &*self.program;
            let mut application = close_conformance_application(program, self.implementation)
                .map_err(|diagnostic| diagnostic.message)?;
            let Some(conformance) = program
                .conformance_by_symbol(application.declaration)
                .cloned()
            else {
                return Err(format!(
                    "`{}` does not name an implementation",
                    self.implementation.display_name()
                ));
            };
            let implemented = self.application_display(
                conformance.trait_name.as_str(),
                program
                    .type_reference_table
                    .type_reference_handles(conformance.arguments),
            );
            let scope = ApplicationScope::new(
                conformance.symbol,
                program.conformance_type_parameters(&conformance),
                &self.implementation_references,
                conformance
                    .lifetime_parameters
                    .iter()
                    .map(|parameter| parameter.as_str().to_owned())
                    .zip(application.lifetime_arguments.iter().cloned())
                    .collect(),
            )
            .ok_or_else(|| self.wrong_trait(&implemented))?;
            let lifetimes = conformance
                .trait_lifetime_arguments
                .iter()
                .map(|ordinal| {
                    conformance
                        .lifetime_parameters
                        .get(*ordinal as usize)
                        .map(|name| name.as_str().to_owned())
                })
                .collect::<Option<Vec<_>>>()
                .ok_or_else(|| self.wrong_trait(&implemented))?;
            let expected = ApplicationArguments {
                declaration: application.trait_definition,
                arguments: program
                    .type_reference_table
                    .type_reference_handles(conformance.arguments)
                    .to_vec(),
                lifetimes,
                scopes: vec![scope],
            };
            self.check_trait_arguments(&implemented, expected)?;
            let program = &mut *self.program;
            let Some(row) = conformance
                .rows
                .iter()
                .find(|row| row.requirement == self.requirement)
            else {
                return Err(format!(
                    "implementation `{}` has no row for `{}`",
                    self.implementation.display_name(),
                    self.requirement_name
                ));
            };
            let source_application = Some(self.call.application)
                .filter(|handle| handle.is_valid())
                .ok_or_else(|| {
                    "direct conformance selection lost its authored application".to_owned()
                })?;
            let ordinal = expressions_of(program)
                .static_machine_application_arguments(source_application)
                .len()
                .checked_sub(1)
                .and_then(|value| u32::try_from(value).ok())
                .ok_or_else(|| {
                    "direct conformance selection lost its explicit operand".to_owned()
                })?;
            let target = row.realization_state;
            let row = row.clone();
            retain_prepared_conformance_views(
                program,
                source_application,
                ordinal,
                &mut application,
            )
            .map_err(str::to_owned)?;
            expressions_of_mut(program)
                .static_machine_application_mut(source_application)
                .conformance_selection = Some(Box::new(StaticConformanceSelection {
                operand_ordinal: ordinal,
                application: Some(Box::new(application.clone())),
            }));
            Ok(ExplicitRequirementRewrite {
                call: self.call,
                target,
                name: Identifier::generated(
                    row.realization_name
                        .as_str()
                        .rsplit("::")
                        .next()
                        .unwrap_or(row.realization_name.as_str()),
                ),
                application_arguments: Box::new([]),
                implementation_receiver: None,
                dispatch: Some(StaticRequirementDispatch {
                    selection_application: source_application,
                    evidence_parameter: SymbolHandle::invalid(),
                    application_report_fingerprint: application.report_fingerprint,
                    application_commitment: application.commitment,
                    declaring_trait: row.declaring_trait,
                    requirement: row.requirement,
                    contract_signature: SymbolHandle::invalid(),
                    realization_machine: row.realization_machine,
                    realization_state: row.realization_state,
                }),
            })
        }

        fn through_binder(&mut self) -> Result<ExplicitRequirementRewrite, String> {
            if self.implementation.application.is_some() {
                return Err("an implementation evidence binder is already selected and takes no application arguments".to_owned());
            }
            let program = &*self.program;
            let binder = self.implementation.symbol;
            let Some(bound) = program
                .callable_conformance_bound(self.lexical_owner, binder)
                .cloned()
            else {
                return Err(format!(
                    "evidence binder `{}` has no declared trait",
                    self.implementation.display_name()
                ));
            };
            let bounded = self.application_display(bound.trait_name.as_str(), &bound.arguments);
            let Some(trait_definition) = program.trait_definition_by_symbol(bound.trait_symbol)
            else {
                return Err(format!("evidence binder bound `{bounded}` names no trait"));
            };
            let mut closure = Vec::new();
            crate::monomorphization::collect_evidence_requirement_closure(
                program,
                trait_definition,
                &mut Vec::new(),
                &mut closure,
            );
            let Some(position) = closure
                .iter()
                .position(|signature| signature.symbol == self.requirement)
            else {
                return Err(self.wrong_trait(&bounded));
            };
            let Some(placeholder) = program
                .symbols
                .child_handles(binder)
                .into_iter()
                .flatten()
                .nth(position)
            else {
                return Err(format!(
                    "evidence binder `{}` has no requirement call for `{}`",
                    self.implementation.display_name(),
                    self.requirement_name
                ));
            };
            let target_name = Identifier::generated(closure[position].name.as_str());
            self.check_trait_arguments(
                &bounded,
                ApplicationArguments {
                    declaration: bound.trait_symbol,
                    arguments: bound.arguments.clone(),
                    lifetimes: Vec::new(),
                    scopes: Vec::new(),
                },
            )?;
            Ok(ExplicitRequirementRewrite {
                call: self.call,
                target: placeholder,
                name: target_name,
                application_arguments: Box::default(),
                dispatch: None,
                implementation_receiver: Some((bound.binder_name.clone(), binder)),
            })
        }

        fn application_display(
            &self,
            trait_name: &str,
            arguments: &[TypeReferenceHandle],
        ) -> String {
            format!(
                "{trait_name}<{}>",
                arguments
                    .iter()
                    .map(|argument| self.program.display_type_reference(*argument))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }

        fn relation(&self) -> &'static str {
            if self.program.symbols.get(self.implementation.symbol).kind
                == SymbolKind::ConformanceParameter
            {
                "is bound as"
            } else {
                "implements"
            }
        }

        fn wrong_trait(&self, implemented: &str) -> String {
            format!(
                "`{}` {} `{implemented}`, which neither is nor inherits `{}`, the trait that declares `{}`",
                self.implementation.display_name(),
                self.relation(),
                self.program
                    .symbols
                    .display_path(self.declaring_trait, "::"),
                self.requirement_name,
            )
        }

        fn check_trait_arguments(
            &mut self,
            implemented: &str,
            expected: ApplicationArguments,
        ) -> Result<(), String> {
            let supplier = self
                .program
                .static_operand_region_supplier(self.implementation, self.lexical_owner)
                .map_err(str::to_owned)?;
            let matched = application_arguments::matches_declaring_application(
                self.program,
                supplier,
                self.declaring_trait,
                &self.trait_references,
                expected,
            );
            let mismatch = match matched {
                Some(Ok(())) => return Ok(()),
                None => return Err(self.wrong_trait(implemented)),
                Some(Err(mismatch)) => mismatch,
            };
            Err(format!(
                "`{}` {} `{implemented}`, not the `{}<{}>` this call of `{}` names; {mismatch}",
                self.implementation.display_name(),
                self.relation(),
                self.program
                    .symbols
                    .display_path(self.declaring_trait, "::"),
                self.trait_arguments
                    .iter()
                    .map(StaticMachineArgument::display_name)
                    .collect::<Vec<_>>()
                    .join(", "),
                self.requirement_name,
            ))
        }
    }

    fn retain_prepared_conformance_views(
        program: &TypedTrees,
        handle: StaticMachineApplicationHandle,
        ordinal: u32,
        application: &mut ClosedConformanceApplication,
    ) -> Result<bool, &'static str> {
        let source = expressions_of(program).static_machine_application(handle);
        let Some(selection) = &source.conformance_selection else {
            return Ok(false);
        };
        let Some(previous) = selection.application.as_deref() else {
            return Ok(false);
        };
        let mut original = previous.clone();
        original.callable_application = Default::default();
        original.trait_scope = Default::default();
        if selection.operand_ordinal != ordinal || original != *application {
            return Ok(false);
        }
        let source_owner = source.lexical_owner;
        let region_supplier = program.conformance_selection_region_supplier(handle)?;
        if previous.trait_scope.scope(program).is_some()
            && previous.trait_scope.supplying_owner() != region_supplier
        {
            return Err("refreshed conformance scope has another supplying owner");
        }
        if previous.callable_application.is_valid()
            && expressions_of(program)
                .static_machine_application(previous.callable_application)
                .lexical_owner
                != source_owner
        {
            return Err("refreshed capture view has another supplying owner");
        }
        application.callable_application = previous.callable_application;
        application.trait_scope = previous.trait_scope;
        Ok(true)
    }

    pub(crate) fn refresh_occurrence_selections(
        program: &mut TypedTrees,
    ) -> Result<(), Vec<Diagnostic>> {
        let live = program.live_conformance_occurrence_applications();
        let pending = live
            .iter()
            .copied()
            .filter_map(|handle| {
                let source = expressions_of(program).static_machine_application(handle);
                source
                    .conformance_selection
                    .as_ref()
                    .map(|selection| (handle, selection.operand_ordinal))
            })
            .collect::<Vec<_>>();
        let replacements = {
            let checking = validation::CheckingContext::new(program);
            let mut replacements = Vec::with_capacity(pending.len());
            for (handle, ordinal) in pending {
                let operand = expressions_of(program)
                    .static_machine_application_arguments(handle)
                    .get(ordinal as usize)
                    .cloned()
                    .ok_or_else(|| {
                        vec![Diagnostic::error(
                            "direct conformance occurrence lost its selected operand",
                        )]
                    })?;
                let mut application =
                    close_conformance_application(&checking, &operand).map_err(|d| vec![d])?;
                let retained =
                    retain_prepared_conformance_views(program, handle, ordinal, &mut application)
                        .map_err(|reason| vec![Diagnostic::error(reason)])?;
                replacements.push((handle, application, retained));
            }
            replacements
        };
        let mut changed = Vec::new();
        for (handle, application, retained) in replacements {
            if !retained {
                changed.push(handle);
            }
            expressions_of_mut(program)
                .static_machine_application_mut(handle)
                .conformance_selection
                .as_mut()
                .unwrap()
                .application = Some(Box::new(application));
        }
        let mut expressions = Vec::new();
        let mut statements = Vec::new();
        for (handle, node) in expressions_of(program).iter_expressions() {
            if let ExpressionNode::Call(call) = node
                && let Some(dispatch) = call.static_requirement_dispatch.as_ref()
                && live.contains(&dispatch.selection_application)
            {
                expressions.push((handle, dispatch.selection_application));
            }
        }
        for machine in program.machines() {
            for state in program.machine_states(machine) {
                for handle in state.statement_nodes.handles() {
                    if let Some(call) = program.statement_table.statement(handle).as_call()
                        && let Some(dispatch) = call.static_requirement_dispatch.as_ref()
                        && live.contains(&dispatch.selection_application)
                    {
                        statements.push((handle, dispatch.selection_application));
                    }
                }
            }
        }
        let update =
            |program: &TypedTrees,
             handle|
             -> Result<(u64, ClosedConformanceApplicationCommitment), Vec<Diagnostic>> {
                let app = expressions_of(program)
                    .static_machine_application(handle)
                    .conformance_selection
                    .as_ref()
                    .and_then(|s| s.application.as_deref())
                    .ok_or_else(|| {
                        vec![Diagnostic::error(
                            "direct dispatch lost its occurrence-owned application",
                        )]
                    })?;
                Ok((app.report_fingerprint, app.commitment))
            };
        for (expression, handle) in expressions {
            let (fingerprint, commitment) = update(program, handle)?;
            let call = match expressions_of_mut(program).expression_mut(expression) {
                ExpressionNode::Call(call) => call,
                _ => unreachable!(),
            };
            let dispatch = call.static_requirement_dispatch.as_mut().unwrap();
            if changed.contains(&handle) || dispatch.selection_application != handle {
                dispatch.contract_signature = SymbolHandle::invalid();
            }
            dispatch.selection_application = handle;
            dispatch.application_report_fingerprint = fingerprint;
            dispatch.application_commitment = commitment;
        }
        for (statement, handle) in statements {
            let (fingerprint, commitment) = update(program, handle)?;
            let call = match program.statement_table.statement_mut(statement) {
                typed_trees::statement::StatementNode::Call(call) => call,
                _ => unreachable!(),
            };
            let dispatch = call.static_requirement_dispatch.as_mut().unwrap();
            if changed.contains(&handle) || dispatch.selection_application != handle {
                dispatch.contract_signature = SymbolHandle::invalid();
            }
            dispatch.selection_application = handle;
            dispatch.application_report_fingerprint = fingerprint;
            dispatch.application_commitment = commitment;
        }
        Ok(())
    }
}

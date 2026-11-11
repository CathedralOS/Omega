/*
Machine-call validation utilities (`machine_calls`): assembly services,
attached method bounds, boundary calls, compiler operations, contract
dependencies and identities, denotational calls, effects, fact citations,
invocations, result overloads, and terminal transitions.
*/
pub mod assembly_services {
    use language_semantics::ServiceReachId;
    use source::DependencyScope;
    use symbols::SymbolHandle;

    #[derive(Default)]
    pub(crate) struct AssemblyServices {
        build: ContextServices,
        product: ContextServices,
    }

    #[derive(Default)]
    pub(crate) struct ContextServices {
        machine_control: Selection,
        port_io: Selection,
    }

    #[derive(Default)]
    pub(crate) enum Selection {
        #[default]
        Missing,
        Unique(ServiceReachId),
        Ambiguous,
    }

    impl Selection {
        fn retain(&mut self, service: ServiceReachId) {
            *self = match self {
                Self::Missing => Self::Unique(service),
                Self::Unique(_) | Self::Ambiguous => Self::Ambiguous,
            };
        }

        fn require(&self) -> Result<ServiceReachId, &'static str> {
            match self {
                Self::Unique(service) => Ok(*service),
                Self::Missing => {
                    Err("the core service is unavailable in this body's dependency context")
                }
                Self::Ambiguous => {
                    Err("the core service is ambiguous in this body's dependency context")
                }
            }
        }
    }

    impl AssemblyServices {
        pub(crate) fn new(program: &dyn crate::CheckingInput) -> Self {
            let mut services = Self::default();
            for definition in program.service_reaches.definitions() {
                if program.symbol_toolchain_relative_path(definition.symbol)
                    != Some(std::path::Path::new("assembly.omg"))
                {
                    continue;
                }
                let Some(file) = program
                    .symbols
                    .symbol_source_span(definition.symbol)
                    .and_then(|span| program.symbols.source_file(span))
                else {
                    continue;
                };
                let Ok(identity) = program.normalized_hermetic_symbol_identity(definition.symbol)
                else {
                    continue;
                };
                let context = match file.dependency_scope {
                    DependencyScope::Build => &mut services.build,
                    DependencyScope::Product => &mut services.product,
                };
                let selection = match identity.as_str() {
                    "toolchain::MachineControl" => &mut context.machine_control,
                    "toolchain::PortIo" => &mut context.port_io,
                    _ => continue,
                };
                if let Some(service) = program.service_reaches.id_for_symbol(definition.symbol) {
                    selection.retain(service);
                }
            }
            services
        }

        pub(crate) fn resolve(
            &self,
            program: &dyn crate::CheckingInput,
            owner: SymbolHandle,
            service: &str,
        ) -> Result<ServiceReachId, &'static str> {
            let file = program
                .symbols
                .symbol_provenance_source_span(owner)
                .and_then(|span| program.symbols.source_file(span))
                .ok_or("the assembly body has no retained dependency context")?;
            let context = match file.dependency_scope {
                DependencyScope::Build => &self.build,
                DependencyScope::Product => &self.product,
            };
            match service {
                "MachineControl" => context.machine_control.require(),
                "PortIo" => context.port_io.require(),
                _ => Err("the assembly intrinsic names an unsupported core service"),
            }
        }
    }
}

pub mod attached_method_bounds {
    use std::collections::HashSet;

    use diagnostics::Diagnostic;
    use symbols::SymbolHandle;
    use typed_trees::types::TypeReferenceNode;

    pub fn validate_selected_attached_method_bounds(
        program: &dyn crate::CheckingInput,
    ) -> Result<(), Vec<Diagnostic>> {
        if !program
            .machines()
            .iter()
            .any(|machine| machine.generic_data_template.is_valid())
        {
            return Ok(());
        }
        let mut selected = HashSet::<SymbolHandle>::new();
        let mut expressions = Vec::new();
        for constant in program.const_declarations() {
            crate::queries::expression_traversal::collect_expression_tree(
                program,
                constant.authored_initializer,
                &mut expressions,
            );
            crate::queries::expression_traversal::collect_expression_tree(
                program,
                constant.materialized_initializer,
                &mut expressions,
            );
        }
        for call in program
            .proof_output_calls
            .iter()
            .filter(|call| !call.machine_symbol.is_valid())
        {
            crate::queries::expression_traversal::collect_expression_tree(
                program,
                call.call,
                &mut expressions,
            );
        }
        collect_expression_selections(program, &expressions, &mut selected);
        let mut visited = vec![false; program.machines().len()];
        loop {
            let mut advanced = false;
            for (machine, visited) in program.machines().iter().zip(&mut visited) {
                if *visited
                    || (machine.generic_data_template.is_valid()
                        && !method_is_selected(program, machine, &selected))
                {
                    continue;
                }
                *visited = true;
                advanced = true;
                expressions.clear();
                for call in program
                    .proof_output_calls
                    .iter()
                    .filter(|call| call.machine_symbol == machine.symbol)
                {
                    crate::queries::expression_traversal::collect_expression_tree(
                        program,
                        call.call,
                        &mut expressions,
                    );
                }
                for contract in program.machine_contracts(machine) {
                    crate::queries::expression_traversal::collect_contract_facts(
                        program,
                        contract.facts,
                        &mut expressions,
                    );
                }
                for owned in program.machine_owned_data(machine) {
                    crate::queries::expression_traversal::collect_expression_tree(
                        program,
                        owned.initial_value,
                        &mut expressions,
                    );
                }
                for state in program.machine_states(machine) {
                    for contract in program.state_contracts(state) {
                        crate::queries::expression_traversal::collect_contract_facts(
                            program,
                            contract.facts,
                            &mut expressions,
                        );
                    }
                    for statement in program.statement_table.statements(state.statement_nodes) {
                        crate::queries::expression_traversal::collect_statement_expression_trees(
                            program,
                            statement,
                            &mut expressions,
                        );
                        if let Some(call) = statement.as_call() {
                            selected.insert(call.target_symbol);
                            collect_static_selections(&call.machine_arguments, &mut selected);
                        }
                    }
                }
                collect_expression_selections(program, &expressions, &mut selected);
            }
            if !advanced {
                break;
            }
        }
        let mut diagnostics = Vec::new();
        for machine in program.machines().iter().filter(|machine| {
            machine.generic_data_template.is_valid()
                && method_is_selected(program, machine, &selected)
        }) {
            let Some(template) = program.machine_by_symbol(machine.generic_data_template) else {
                diagnostics.push(Diagnostic::error(
                    "generic-data method lost its exact template",
                ));
                continue;
            };
            let application = program
                .type_reference_table
                .type_reference(template.attached_data_application);
            let concrete = program
                .data_definition_by_symbol(machine.attached_data_symbol)
                .and_then(|owner| owner.generic_instance)
                .map(|application| program.type_reference_table.type_reference(application));
            let (
                TypeReferenceNode::Generic {
                    base_symbol,
                    arguments,
                    ..
                },
                Some(TypeReferenceNode::Generic {
                    base_symbol: concrete_base,
                    arguments: concrete_arguments,
                    ..
                }),
            ) = (application, concrete)
            else {
                diagnostics.push(Diagnostic::error(
                    "generic-data method lost its exact owner application",
                ));
                continue;
            };
            let arguments = program
                .type_reference_table
                .type_reference_handles(*arguments);
            let concrete_arguments = program
                .type_reference_table
                .type_reference_handles(*concrete_arguments);
            if *base_symbol != template.attached_data_symbol
                || base_symbol != concrete_base
                || arguments.len() != concrete_arguments.len()
            {
                diagnostics.push(Diagnostic::error(
                    "generic-data method owner application does not match its template",
                ));
                continue;
            }
            for parameter in program.machine_type_parameters(template) {
                let requirements = crate::declared_property_requirements(&parameter.bounds);
                if requirements.is_empty() {
                    continue;
                }
                let argument =
                    arguments
                        .iter()
                        .zip(concrete_arguments)
                        .find_map(|(formal, actual)| {
                            matches!(program.type_reference_table.type_reference(*formal),
                    TypeReferenceNode::Named { symbol, .. } if *symbol == parameter.symbol)
                            .then_some(*actual)
                        });
                if !argument.is_some_and(|argument| {
                    requirements.iter().all(|requirement| {
                        crate::type_satisfies_declared_property(
                            program,
                            &[],
                            argument,
                            *requirement,
                        )
                    })
                }) {
                    diagnostics.push(Diagnostic::error(format!(
                    "generic method `{}` has a selected owner argument that does not satisfy its authored type bounds for `{}`",
                    template.name, parameter.name,
                )));
                }
            }
        }
        crate::program_validation::finish_strict_diagnostics(diagnostics)
    }

    fn method_is_selected(
        program: &dyn crate::CheckingInput,
        machine: &typed_trees::machine::Machine,
        selected: &HashSet<SymbolHandle>,
    ) -> bool {
        selected.contains(&machine.symbol)
            || program
                .machine_states(machine)
                .iter()
                .any(|state| selected.contains(&state.symbol))
    }

    fn collect_expression_selections(
        program: &dyn crate::CheckingInput,
        expressions: &[typed_trees::expression::ExpressionHandle],
        selected: &mut HashSet<SymbolHandle>,
    ) {
        for expression in expressions {
            if let Some(call) = program.expression_table.call_node(*expression) {
                selected.insert(call.target_symbol);
                collect_static_selections(&call.machine_arguments, selected);
            }
        }
    }

    fn collect_static_selections(
        arguments: &[typed_trees::expression::StaticMachineArgument],
        selected: &mut HashSet<SymbolHandle>,
    ) {
        for argument in arguments {
            selected.insert(argument.symbol);
            if let Some(application) = &argument.application {
                collect_static_selections(&application.arguments, selected);
            }
        }
    }
}

pub mod boundary_calls {
    use checked_trees::CheckedTrees;
    use diagnostics::Diagnostic;
    use symbols::SymbolHandle;

    pub fn validate_boundary_calls(checked: &CheckedTrees) -> Result<(), Vec<Diagnostic>> {
        let program = &checked.typed;
        let calls = checked_boundary_call_coordinates(checked)?;
        let approved = approved_boundary_traits(program);
        let diagnostics: Vec<_> = calls
        .into_iter()
        .filter(|call| !approved.contains(&call.boundary_trait_symbol))
        .map(|call| {
            Diagnostic::error(format!(
                "unapproved boundary call: {} in {} exercises a boundary capability with no approved provider for that exact capability",
                symbol_name(program, call.boundary_trait_symbol),
                symbol_name(program, call.state_symbol),
            ))
        })
        .collect();
        crate::program_validation::finish_strict_diagnostics(diagnostics)
    }

    fn approved_boundary_traits(program: &dyn crate::CheckingInput) -> Vec<SymbolHandle> {
        let mut approved = Vec::new();
        for definition in program
            .traits()
            .iter()
            .filter(|definition| definition.is_boundary)
        {
            let exact_symbol_owners = program
                .traits()
                .iter()
                .filter(|candidate| candidate.symbol == definition.symbol)
                .count();
            let declaration_module = program.symbols.symbol_module(definition.symbol);
            let exact_name_owners = program
                .traits()
                .iter()
                .filter(|candidate| {
                    candidate.is_boundary
                        && candidate.name.as_str() == definition.name.as_str()
                        && program.symbols.symbol_module(candidate.symbol) == declaration_module
                        && program
                            .symbols
                            .same_symbol_source_package(candidate.symbol, definition.symbol)
                })
                .count();
            let has_exact_identity = definition.symbol.is_valid()
                && !definition.name.as_str().is_empty()
                && exact_symbol_owners == 1
                && exact_name_owners == 1;
            if has_exact_identity {
                approved.push(definition.symbol);
            }
        }
        approved
    }

    fn symbol_key(symbol: SymbolHandle) -> (u32, u32) {
        (symbol.arena_index(), symbol.generation())
    }

    fn equal_range<T>(
        sorted: &[T],
        symbol: SymbolHandle,
        symbol_of: impl Fn(&T) -> SymbolHandle,
    ) -> &[T] {
        let key = symbol_key(symbol);
        let start = sorted.partition_point(|row| symbol_key(symbol_of(row)) < key);
        let end = start + sorted[start..].partition_point(|row| symbol_key(symbol_of(row)) == key);
        &sorted[start..end]
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct BoundaryCallCoordinate {
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        target_state_symbol: SymbolHandle,
        boundary_trait_symbol: SymbolHandle,
        boundary_signature_symbol: SymbolHandle,
        statement_index: usize,
        call_ordinal: usize,
    }

    fn checked_boundary_call_coordinates(
        checked: &CheckedTrees,
    ) -> Result<Vec<BoundaryCallCoordinate>, Vec<Diagnostic>> {
        let program = &checked.typed;
        let flow = &checked.facts.flow.control;
        let mut diagnostics = Vec::new();
        let mut coordinates = Vec::new();

        let traits = program.traits();
        if traits.len() != program.roots.traits.count() as usize {
            diagnostics.push(Diagnostic::error(
                "provider approval checked flow has an invalid typed trait span",
            ));
        }
        for definition in traits {
            if program.trait_machine_signatures(definition).len()
                != definition.machines.count() as usize
            {
                diagnostics.push(Diagnostic::error(format!(
                "provider approval checked flow has an invalid typed signature span for trait {:?}",
                definition.symbol,
            )));
            }
        }
        let machines = program.machines();
        if machines.len() != program.roots.machines.count() as usize {
            diagnostics.push(Diagnostic::error(
                "provider approval checked flow has an invalid typed machine span",
            ));
        }
        for machine in machines {
            if program.machine_states(machine).len() != machine.states.count() as usize {
                diagnostics.push(Diagnostic::error(format!(
                "provider approval checked flow has an invalid typed state span for machine {:?}",
                machine.symbol,
            )));
            }
        }
        if !diagnostics.is_empty() {
            return Err(diagnostics);
        }

        let flow_coordinates = flow
            .states
            .iter()
            .map(|(_, flow_state)| (flow_state.machine_symbol, flow_state.state_symbol))
            .collect::<Vec<_>>();
        let mut coordinate_order = (0..flow_coordinates.len()).collect::<Vec<_>>();
        coordinate_order.sort_by_key(|&row| {
            let (machine, state) = flow_coordinates[row];
            (symbol_key(machine), symbol_key(state), row)
        });
        let mut repeated_coordinate = vec![false; flow_coordinates.len()];
        for pair in coordinate_order.windows(2) {
            if flow_coordinates[pair[0]] == flow_coordinates[pair[1]] {
                repeated_coordinate[pair[1]] = true;
            }
        }
        let mut machines_by_symbol = machines.iter().collect::<Vec<_>>();
        machines_by_symbol.sort_by_key(|machine| symbol_key(machine.symbol));
        let mut states_by_symbol = machines
            .iter()
            .flat_map(|machine| {
                program
                    .machine_states(machine)
                    .iter()
                    .map(move |state| (machine, state))
            })
            .collect::<Vec<_>>();
        states_by_symbol.sort_by_key(|(_, state)| symbol_key(state.symbol));

        for (row, (_, flow_state)) in flow.states.iter().enumerate() {
            if repeated_coordinate[row] {
                diagnostics.push(Diagnostic::error(format!(
                "provider approval checked flow contains duplicate state coordinate ({:?}, {:?})",
                flow_state.machine_symbol, flow_state.state_symbol,
            )));
                continue;
            }

            let machines = equal_range(&machines_by_symbol, flow_state.machine_symbol, |machine| {
                machine.symbol
            });
            let machine = match machines {
                [machine] => *machine,
                [] => {
                    diagnostics.push(Diagnostic::error(format!(
                    "provider approval checked-flow state {:?} has no exact typed machine owner {:?}",
                    flow_state.state_symbol, flow_state.machine_symbol,
                )));
                    continue;
                }
                _ => {
                    diagnostics.push(Diagnostic::error(format!(
                    "provider approval checked-flow state {:?} has duplicate exact typed machine owners {:?}",
                    flow_state.state_symbol, flow_state.machine_symbol,
                )));
                    continue;
                }
            };

            let state_owners =
                equal_range(&states_by_symbol, flow_state.state_symbol, |(_, state)| {
                    state.symbol
                });
            let typed_state = match state_owners {
                [(owner, state)] if owner.symbol == machine.symbol => *state,
                [(owner, _)] => {
                    diagnostics.push(Diagnostic::error(format!(
                    "provider approval checked-flow state {:?} belongs to typed machine {:?}, not {:?}",
                    flow_state.state_symbol, owner.symbol, flow_state.machine_symbol,
                )));
                    continue;
                }
                [] => {
                    diagnostics.push(Diagnostic::error(format!(
                    "provider approval checked-flow state {:?} is missing from its exact typed machine {:?}",
                    flow_state.state_symbol, flow_state.machine_symbol,
                )));
                    continue;
                }
                _ => {
                    diagnostics.push(Diagnostic::error(format!(
                    "provider approval checked-flow state {:?} has duplicate typed state owners",
                    flow_state.state_symbol,
                )));
                    continue;
                }
            };

            let Some(calls) = flow.calls.span(flow_state.calls) else {
                diagnostics.push(Diagnostic::error(format!(
                    "provider approval checked-flow state {:?} has an invalid call span",
                    flow_state.state_symbol,
                )));
                continue;
            };
            let statements = program
                .statement_table
                .statements(typed_state.statement_nodes);
            if statements.len() != typed_state.statement_nodes.count() as usize {
                diagnostics.push(Diagnostic::error(format!(
                    "provider approval checked-flow state {:?} has an invalid typed statement span",
                    flow_state.state_symbol,
                )));
                continue;
            }
            let statement_count = statements.len();
            let mut call_coordinates = Vec::new();
            for call in calls {
                let call_coordinate = (call.statement_index, call.call_ordinal);
                if call_coordinates.contains(&call_coordinate) {
                    diagnostics.push(Diagnostic::error(format!(
                    "provider approval checked-flow state {:?} contains duplicate call coordinate (statement {}, call {})",
                    flow_state.state_symbol, call.statement_index, call.call_ordinal,
                )));
                    continue;
                }
                call_coordinates.push(call_coordinate);
                if call.statement_index >= statement_count {
                    diagnostics.push(Diagnostic::error(format!(
                    "provider approval checked-flow call in state {:?} has out-of-range statement index {} for {} typed statements",
                    flow_state.state_symbol, call.statement_index, statement_count,
                )));
                    continue;
                }
                let Some(boundary_edges) = checked
                    .facts
                    .flow
                    .boundaries
                    .edges
                    .span(call.boundary_edges)
                else {
                    diagnostics.push(Diagnostic::error(format!(
                    "provider approval checked-flow call in state {:?} has an invalid boundary-edge span",
                    flow_state.state_symbol,
                )));
                    continue;
                };
                if !call.target_symbol.is_valid() {
                    if boundary_edges.is_empty() {
                        continue;
                    }
                    diagnostics.push(Diagnostic::error(format!(
                    "provider approval checked-flow call in state {:?} has no valid target symbol",
                    flow_state.state_symbol,
                )));
                    continue;
                }
                if boundary_edges.is_empty() {
                    match exact_direct_boundary_requirement(program, call.target_symbol) {
                        Ok(Some((boundary_trait_symbol, boundary_signature_symbol))) => {
                            coordinates.push(BoundaryCallCoordinate {
                                machine_symbol: flow_state.machine_symbol,
                                state_symbol: flow_state.state_symbol,
                                target_state_symbol: call.target_symbol,
                                boundary_trait_symbol,
                                boundary_signature_symbol,
                                statement_index: call.statement_index,
                                call_ordinal: call.call_ordinal,
                            });
                        }
                        Ok(None) => {}
                        Err(diagnostic) => diagnostics.push(diagnostic),
                    }
                    continue;
                }

                let mut exact_edges = Vec::new();
                for edge in boundary_edges {
                    if edge.statement_index != call.statement_index
                        || edge.call_ordinal != call.call_ordinal
                    {
                        diagnostics.push(Diagnostic::error(format!(
                        "provider approval checked boundary edge in state {:?} has coordinate (statement {}, call {}), not its owning call coordinate (statement {}, call {})",
                        flow_state.state_symbol,
                        edge.statement_index,
                        edge.call_ordinal,
                        call.statement_index,
                        call.call_ordinal,
                    )));
                        continue;
                    }
                    if edge.target_symbol != call.target_symbol {
                        diagnostics.push(Diagnostic::error(format!(
                        "provider approval checked boundary edge in state {:?} targets {:?}, not its owning call target {:?}",
                        flow_state.state_symbol, edge.target_symbol, call.target_symbol,
                    )));
                        continue;
                    }
                    let edge_identity =
                        (edge.boundary_trait_symbol, edge.boundary_signature_symbol);
                    if exact_edges.contains(&edge_identity) {
                        diagnostics.push(Diagnostic::error(format!(
                        "provider approval checked call in state {:?} contains duplicate exact boundary edge ({:?}, {:?})",
                        flow_state.state_symbol,
                        edge.boundary_trait_symbol,
                        edge.boundary_signature_symbol,
                    )));
                        continue;
                    }
                    exact_edges.push(edge_identity);
                    if let Err(diagnostic) = validate_exact_boundary_requirement(
                        program,
                        edge.boundary_trait_symbol,
                        edge.boundary_signature_symbol,
                        "provider approval checked boundary edge",
                    ) {
                        diagnostics.push(diagnostic);
                        continue;
                    }
                    coordinates.push(BoundaryCallCoordinate {
                        machine_symbol: flow_state.machine_symbol,
                        state_symbol: flow_state.state_symbol,
                        target_state_symbol: call.target_symbol,
                        boundary_trait_symbol: edge.boundary_trait_symbol,
                        boundary_signature_symbol: edge.boundary_signature_symbol,
                        statement_index: call.statement_index,
                        call_ordinal: call.call_ordinal,
                    });
                }
            }
        }

        if diagnostics.is_empty() {
            Ok(coordinates)
        } else {
            Err(diagnostics)
        }
    }

    fn exact_direct_boundary_requirement(
        program: &dyn crate::CheckingInput,
        target_symbol: SymbolHandle,
    ) -> Result<Option<(SymbolHandle, SymbolHandle)>, Diagnostic> {
        let matches = program
            .traits()
            .iter()
            .filter(|definition| definition.is_boundary)
            .flat_map(|definition| {
                boundary_signature_coordinates(program, definition)
                    .filter(move |signature| signature.symbol == target_symbol)
                    .map(move |signature| (definition.symbol, signature.symbol))
            })
            .collect::<Vec<_>>();
        match matches.as_slice() {
            [] => Ok(None),
            [(boundary_trait_symbol, boundary_signature_symbol)] => {
                validate_exact_boundary_requirement(
                    program,
                    *boundary_trait_symbol,
                    *boundary_signature_symbol,
                    "provider approval direct abstract boundary call",
                )?;
                Ok(Some((*boundary_trait_symbol, *boundary_signature_symbol)))
            }
            _ => Err(Diagnostic::error(format!(
                "provider approval call target {:?} resolves to {} exact direct boundary requirements",
                target_symbol,
                matches.len(),
            ))),
        }
    }

    fn boundary_signature_coordinates<'program>(
        program: &'program dyn crate::CheckingInput,
        owner: &'program typed_trees::trait_definition::TraitDefinition,
    ) -> impl Iterator<Item = &'program typed_trees::signature::StateSignature> {
        program.trait_machine_signatures(owner).iter().chain(
            program
                .closed_requirement_applications()
                .filter(move |application| application.declaring_trait == owner.symbol)
                .map(|application| application.signature),
        )
    }

    fn validate_exact_boundary_requirement(
        program: &dyn crate::CheckingInput,
        boundary_trait_symbol: SymbolHandle,
        boundary_signature_symbol: SymbolHandle,
        context: &str,
    ) -> Result<(), Diagnostic> {
        if !boundary_trait_symbol.is_valid() || !boundary_signature_symbol.is_valid() {
            return Err(Diagnostic::error(format!(
                "{context} has an invalid exact boundary trait or signature symbol",
            )));
        }
        let owners = program
            .traits()
            .iter()
            .filter(|definition| definition.symbol == boundary_trait_symbol)
            .collect::<Vec<_>>();
        let [owner] = owners.as_slice() else {
            return Err(Diagnostic::error(format!(
                "{context} trait {:?} resolves to {} exact typed owners",
                boundary_trait_symbol,
                owners.len(),
            )));
        };
        if !owner.is_boundary {
            return Err(Diagnostic::error(format!(
                "{context} trait {:?} is not a boundary trait",
                boundary_trait_symbol,
            )));
        }
        let owned_signatures = boundary_signature_coordinates(program, owner)
            .filter(|signature| signature.symbol == boundary_signature_symbol)
            .count();
        let global_signatures = program
            .traits()
            .iter()
            .flat_map(|definition| boundary_signature_coordinates(program, definition))
            .filter(|signature| signature.symbol == boundary_signature_symbol)
            .count();
        let state_matches = program
            .machines()
            .iter()
            .flat_map(|machine| program.machine_states(machine))
            .filter(|state| state.symbol == boundary_signature_symbol)
            .count();
        if owned_signatures != 1 || global_signatures != 1 || state_matches != 0 {
            return Err(Diagnostic::error(format!(
                "{context} signature {:?} is missing, duplicated, cross-owned, or collides with a typed state",
                boundary_signature_symbol,
            )));
        }
        Ok(())
    }

    fn symbol_name(program: &dyn crate::CheckingInput, symbol: SymbolHandle) -> String {
        if !symbol.is_valid() {
            return "unknown".to_owned();
        }

        if let Some(machine) = program.machine(symbol) {
            return machine.name.as_str().to_owned();
        }

        if let Some(state) = program.state_by_symbol(symbol) {
            return state.name.as_str().to_owned();
        }

        program.symbols.name(symbol).to_owned()
    }
}

pub mod callable_overloads {
    use crate::CheckingInput;
    use diagnostics::Diagnostic;
    use typed_trees::type_identity::CallableIdentityId;

    pub(crate) fn validate_named_callable_overload_declarations(
        program: &dyn CheckingInput,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        validate_machine_overloads(program, diagnostics);
        validate_trait_requirement_overloads(program, diagnostics);
    }

    fn validate_machine_overloads(program: &dyn CheckingInput, diagnostics: &mut Vec<Diagnostic>) {
        let mut seen: Vec<(CallableIdentityId, symbols::SymbolHandle)> = Vec::new();
        for machine in program
            .machines()
            .iter()
            .filter(|machine| machine.target.is_none())
        {
            let identity = program.machine_callable_identity(machine);
            if !identity.is_valid() {
                continue;
            }
            if seen.iter().any(|(previous_identity, previous_symbol)| {
                *previous_identity == identity
                    && !program
                        .symbols
                        .source_scopes_separate(*previous_symbol, machine.symbol)
            }) {
                diagnostics.push(duplicate_diagnostic(
                    program,
                    "machine",
                    &lexical_machine_name(program, machine),
                    identity,
                ));
            } else {
                seen.push((identity, machine.symbol));
            }
        }
    }

    fn validate_trait_requirement_overloads(
        program: &dyn CheckingInput,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        for trait_definition in program.traits() {
            let mut seen: Vec<(CallableIdentityId, symbols::SymbolHandle)> = Vec::new();
            for requirement in program.trait_machine_signatures(trait_definition) {
                let identity =
                    program.trait_requirement_callable_identity(trait_definition, requirement);
                if seen.iter().any(|(previous_identity, previous_symbol)| {
                    *previous_identity == identity
                        && !program
                            .symbols
                            .source_scopes_separate(*previous_symbol, requirement.symbol)
                }) {
                    diagnostics.push(duplicate_diagnostic(
                        program,
                        "requirement",
                        &format!("{}::{}", trait_definition.name, requirement.name),
                        identity,
                    ));
                } else {
                    seen.push((identity, requirement.symbol));
                }
            }
        }
    }

    fn lexical_machine_name(
        program: &dyn CheckingInput,
        machine: &typed_trees::machine::Machine,
    ) -> String {
        let mut names = vec![machine.name.as_str()];
        let mut parent = program.symbols.get(machine.symbol).parent;
        while parent.is_valid() && program.symbols.get(parent).kind == symbols::SymbolKind::Machine
        {
            names.push(program.symbols.name(parent));
            parent = program.symbols.get(parent).parent;
        }
        names.reverse();
        names.join("::")
    }

    fn duplicate_diagnostic(
        program: &dyn CheckingInput,
        kind: &str,
        name: &str,
        identity: CallableIdentityId,
    ) -> Diagnostic {
        let result_dispatch = program.callable_identity_result_dispatch(identity);
        let dispatch = if program.callable_result_dispatch_is_empty(result_dispatch) {
            "<empty>".to_owned()
        } else {
            program.callable_result_dispatch_text(result_dispatch)
        };
        Diagnostic::error(format!(
            "duplicate named {kind} overload `{name}` with parameter signature `{}` and result dispatch set `{dispatch}`; predicate-only result refinements do not distinguish overloads",
            program.callable_parameters_text(program.callable_identity_parameters(identity)),
        ))
    }
}

pub mod compiler_operations {
    pub(crate) mod build_declarations {
        use super::exact_build_prelude_data;
        use crate::frozen_program::expression_table;
        use crate::selection_resolution::AuthoredSelectionResolutionTarget;
        use language_semantics::declaration_selection::AuthoredDeclarationSelectionIntrinsic;
        use language_semantics::declaration_selection::BuildOperation;
        use symbols::JoinedPath;
        use typed_trees::expression::{ExpressionHandle, ExpressionNode};
        use typed_trees::product_operand::ProductOperandKind;

        pub fn is_build_declaration_selection(
            program: &dyn crate::CheckingInput,
            expression: ExpressionHandle,
        ) -> bool {
            let Some(call) = expression_table(program).call_node(expression) else {
                return false;
            };
            BuildOperation::from_call_target(call.target.as_str()).is_some_and(|operation| {
                operation.selects_product_declarations()
                    && super::classify_expression_call_intrinsic(program, call)
                        == Some(operation.intrinsic())
            })
        }

        pub fn resolve_build_static_operand(
            program: &dyn crate::CheckingInput,
            expression: ExpressionHandle,
            argument_index: usize,
            occurrence: source::SourceSpan,
        ) -> Result<AuthoredSelectionResolutionTarget, String> {
            if !is_build_declaration_selection(program, expression) {
                return Err(
                    "static operand does not belong to an authenticated Build operation".into(),
                );
            }
            let call = expression_table(program)
                .call_node(expression)
                .ok_or("Build static operand has no retained call")?;
            let operation = BuildOperation::from_call_target(call.target.as_str())
                .ok_or("Build static operand has no retained operation")?;
            let argument = call
                .machine_arguments
                .get(argument_index)
                .ok_or("Build static operand is absent")?;
            if operation == BuildOperation::WireCompatibilityRequest && argument_index >= 4 {
                let fact = argument
                    .path
                    .first()
                    .filter(|_| argument.path.len() == 1)
                    .filter(|_| {
                        typed_trees::product_operand::is_declaration_operand(
                            program.typed_program(),
                            argument,
                        )
                    })
                    .and_then(|name| {
                        language_semantics::wire::WireCompatibilityFact::from_name(name.as_str())
                    })
                    .ok_or_else(|| {
                        format!(
                            "unknown wire compatibility fact `{}`",
                            argument.path.joined_path()
                        )
                    })?;
                return Ok(AuthoredSelectionResolutionTarget::Intrinsic(
                    AuthoredDeclarationSelectionIntrinsic::WireCompatibilityFact(fact),
                ));
            }
            let kind = match (operation, argument_index) {
                (BuildOperation::ProviderSelection, 0)
                | (
                    BuildOperation::ServiceExclusion
                    | BuildOperation::ServiceAudit
                    | BuildOperation::BoundaryAcceptance,
                    0,
                ) => ProductOperandKind::BoundarySubject,
                (BuildOperation::ProviderSelection, 1) => ProductOperandKind::Provider,
                (BuildOperation::AssumedBindingContract, 0 | 1) => ProductOperandKind::Requirement,
                (BuildOperation::TestGroupRegistration, 0) => ProductOperandKind::TestRequirement,
                (BuildOperation::WireCompatibilityRequest, 0..=3)
                | (BuildOperation::RepresentationSelection, 0) => ProductOperandKind::Data,
                (BuildOperation::RepresentationSelection, 1) => ProductOperandKind::Representation,
                _ => {
                    return Err(
                        "Build static operand has no declaration role at this position".into(),
                    );
                }
            };
            let selected = typed_trees::product_operand::resolve_product_operand(
        program.typed_program(),
        argument,
        occurrence,
        kind,
            ).ok_or("Build declaration operand does not resolve to one visible product declaration of the required kind")?;
            if matches!(
                operation,
                BuildOperation::ServiceExclusion | BuildOperation::ServiceAudit
            ) && !program
                .product_declarations()
                .traits()
                .iter()
                .any(|definition| definition.symbol == selected && definition.is_boundary)
            {
                return Err(format!(
                    "Build service {} operand must name a boundary trait",
                    if operation == BuildOperation::ServiceAudit {
                        "audit"
                    } else {
                        "exclusion"
                    }
                ));
            }
            Ok(AuthoredSelectionResolutionTarget::Declaration(selected))
        }

        pub fn is_provider_selection_call(
            program: &dyn crate::CheckingInput,
            call: &typed_trees::expression::TableCallExpression,
        ) -> bool {
            BuildOperation::from_call_target(call.target.as_str())
                == Some(BuildOperation::ProviderSelection)
                && has_explicit_build_argument(program, call)
                && (2..=3).contains(
                    &expression_table(program)
                        .expression_handles(call.arguments)
                        .len(),
                )
                && call.machine_arguments.len() == 2
        }

        pub(crate) fn has_explicit_build_argument(
            program: &dyn crate::CheckingInput,
            call: &typed_trees::expression::TableCallExpression,
        ) -> bool {
            !call.target_symbol.is_valid()
                && call.receiver.is_valid()
                && matches!(expression_table(program).expression(call.receiver), ExpressionNode::Name(path)
            if exact_build_prelude_data(program, path.symbol, "Build"))
                && expression_table(program)
                    .expression_handles(call.arguments)
                    .first()
                    .is_some_and(|builder| exact_mutable_build_argument(program, *builder))
                && call.evidence_arguments.is_empty()
        }

        fn exact_mutable_build_argument(
            program: &dyn crate::CheckingInput,
            receiver: ExpressionHandle,
        ) -> bool {
            if !receiver.is_valid() {
                return false;
            }
            if let Some(borrow) = expression_table(program).borrow_expression(receiver) {
                return borrow.access == language_semantics::ReferenceAccess::Mutable
                    && crate::expression_type_symbol(program, borrow.target)
                        .is_some_and(|symbol| exact_build_prelude_data(program, symbol, "Build"));
            }
            crate::expression_place_type_reference(program, receiver, &[]).is_some_and(|reference| {
        matches!(program.type_reference_table.type_reference(reference),
            typed_trees::types::TypeReferenceNode::Reference { access, referee, .. }
                if *access == language_semantics::ReferenceAccess::Mutable
                    && exact_build_prelude_data(program, program.type_reference_table.type_symbol(*referee), "Build"))
            })
        }

        pub fn build_operation(
            program: &dyn crate::CheckingInput,
            expression: typed_trees::expression::ExpressionHandle,
        ) -> Option<language_semantics::declaration_selection::BuildOperation> {
            let typed_trees::expression::ExpressionNode::Call(call) =
                expression_table(program).expression(expression)
            else {
                return None;
            };
            let operation =
                language_semantics::declaration_selection::BuildOperation::from_call_target(
                    call.target.as_str(),
                )?;
            (super::classify_expression_call_intrinsic(program, call)
                == Some(operation.intrinsic()))
            .then_some(operation)
        }

        pub fn build_product_request_intrinsic(
            program: &dyn crate::CheckingInput,
            expression: ExpressionHandle,
            selection: &language_semantics::declaration_selection::AuthoredDeclarationSelection,
        ) -> Result<Option<AuthoredDeclarationSelectionIntrinsic>, String> {
            use language_semantics::declaration_selection::{
                AuthoredDeclarationSelectionKind as Kind, BuildSelectionRequest, ProductOperandRole,
            };
            let Some(call) = expression_table(program).call_node(expression) else {
                return Ok(None);
            };
            let Some(BuildSelectionRequest::Declaration(request)) =
                expression_table(program).build_selection_request(call.build_selection)
            else {
                return Ok(None);
            };
            request.validate_shape()?;
            let owner = program
                .traits()
                .iter()
                .find(|owner| {
                    owner.is_boundary
                        && owner.name.as_str() == "BuildBindingOperations"
                        && program.symbols.is_build_prelude_symbol(owner.symbol)
                        && program
                            .trait_machine_signatures(owner)
                            .iter()
                            .any(|signature| {
                                signature.symbol == call.target_symbol
                                    && matches!(
                                        signature.name.as_str(),
                                        "prepare" | "select" | "select_provider"
                                    )
                            })
                })
                .ok_or("Build product request has no exact compiler-owned recording operation")?;
            let _ = owner;
            if selection.kind() == Kind::Call && selection.source_span() == request.source_span {
                return Ok(Some(request.operation.intrinsic()));
            }
            if !matches!(selection.kind(), Kind::StaticArgument | Kind::Conformance) {
                return Ok(None);
            }
            let Some(position) = selection.top_level_static_argument_position() else {
                return Ok(None);
            };
            let operand = request
                .operands
                .get(position)
                .ok_or("Build product operand occurrence is outside its request")?;
            let role = request
                .operation
                .product_operand_role(position)
                .ok_or("Build product operand has no declared role")?;
            if operand.source_span != selection.source_span() {
                return Err("Build product operand lost its exact authored occurrence".into());
            }
            if role == ProductOperandRole::WireCompatibilityFact {
                let fact =
                    language_semantics::wire::WireCompatibilityFact::from_name(&operand.path)
                        .ok_or("Build wire demand names an unknown compatibility fact")?;
                Ok(Some(
                    AuthoredDeclarationSelectionIntrinsic::WireCompatibilityFact(fact),
                ))
            } else {
                Ok(Some(
                    AuthoredDeclarationSelectionIntrinsic::BuildProductOperand {
                        operation: request.operation,
                        position,
                        role,
                    },
                ))
            }
        }
    }
    pub(crate) mod descriptor_declarations {
        use crate::CheckingInput;
        use language_semantics::{MachineSupplyMode, ReferenceAccess, ServiceReachId};
        use symbols::SymbolHandle;
        use typed_trees::machine::Machine;
        use typed_trees::types::{
            FixedArrayLength, PrimitiveType, TypeReferenceHandle, TypeReferenceNode,
        };

        pub(crate) fn boundary_state<'a>(
            program: &'a dyn CheckingInput,
            machine: &'a Machine,
        ) -> Result<&'a typed_trees::state::State, &'static str> {
            if machine.body_is_present
                || machine.supply_mode != MachineSupplyMode::Boundary
                || machine.suspends
                || machine.blocks
                || !machine.lifetime_parameters.is_empty()
                || !machine.contracts.is_empty()
                || !machine.invokes.is_empty()
                || machine.spelling.is_some()
                || !program.machine_trait_conformances(machine).is_empty()
                || machine.target.is_some()
                || machine.authored_target.is_some()
                || machine.target_family_interface
                || !machine.conformance_bounds.is_empty()
                || machine.purpose != typed_trees::machine::MachinePurpose::Runtime
                || !machine.owned_data.is_empty()
                || machine.admitted_same_stack_bytes != 0
                || machine.service_reach_is_installation_bound
                || (program.machine_specialization(machine.symbol).is_none() && machine.is_public)
            {
                return Err(
                    "descriptor operation primitive requires its private bodyless core declaration",
                );
            }
            let [state] = program.machine_states(machine) else {
                return Err("descriptor operation declaration must have one entry signature");
            };
            if !state.contracts.is_empty() {
                return Err("descriptor operation primitive must not add entry contracts");
            }
            if program
                .state_parameters(state)
                .iter()
                .any(|parameter| parameter.is_const || parameter.relevance.is_erased())
            {
                return Err("descriptor operation primitive must retain every runtime operand");
            }
            Ok(state)
        }

        pub(crate) fn machine_control_service(
            program: &dyn CheckingInput,
            machine: &Machine,
        ) -> Result<ServiceReachId, &'static str> {
            let [service] = program
                .service_reach_rows
                .services(machine.service_reach_row)
            else {
                return Err("descriptor operation primitive must reach exactly MachineControl");
            };
            let definition = program
                .service_reaches
                .definition(*service)
                .ok_or("descriptor operation MachineControl service has no declaration")?;
            if program.symbol_toolchain_relative_path(definition.symbol)
                != Some(std::path::Path::new("assembly.omg"))
                || program
                    .normalized_hermetic_symbol_identity(definition.symbol)
                    .ok()
                    .as_deref()
                    != Some("toolchain::MachineControl")
            {
                return Err(
                    "descriptor operation primitive must reach the exact core MachineControl service",
                );
            }
            Ok(*service)
        }

        pub(crate) fn plain_primitive(
            program: &dyn CheckingInput,
            reference: TypeReferenceHandle,
            primitive: PrimitiveType,
        ) -> bool {
            let TypeReferenceNode::Named { symbol, .. } =
                program.type_reference_table.type_reference(reference)
            else {
                return false;
            };
            let expected = match primitive {
                PrimitiveType::U8 => symbols::BuiltinTypeAtom::U8,
                PrimitiveType::U16 => symbols::BuiltinTypeAtom::U16,
                PrimitiveType::U64 => symbols::BuiltinTypeAtom::U64,
                _ => return false,
            };
            program.symbols.builtin_type_atom(*symbol) == Some(expected)
        }

        pub(crate) fn array_loan(
            program: &dyn CheckingInput,
            reference: TypeReferenceHandle,
            access: ReferenceAccess,
            element: PrimitiveType,
        ) -> Result<(TypeReferenceHandle, &FixedArrayLength), &'static str> {
            let TypeReferenceNode::Reference {
                referee,
                access: actual,
                ..
            } = program.type_reference_table.type_reference(reference)
            else {
                return Err("descriptor operation storage must be an ordinary reference");
            };
            let TypeReferenceNode::FixedArray {
                element_type,
                length,
            } = program.type_reference_table.type_reference(*referee)
            else {
                return Err("descriptor operation storage must be an unqualified fixed array");
            };
            if *actual != access || !plain_primitive(program, *element_type, element) {
                return Err(
                    "descriptor operation storage has different element or access authority",
                );
            }
            Ok((*referee, length))
        }
        pub(crate) fn fixed_array_length(
            program: &dyn CheckingInput,
            reference: TypeReferenceHandle,
            access: ReferenceAccess,
            element: PrimitiveType,
        ) -> Result<usize, &'static str> {
            let (array, _) = array_loan(program, reference, access, element)?;
            program
                .closed_fixed_array_length(array)
                .ok_or("descriptor operation storage extent is not closed")
        }

        pub(crate) fn extent_matches_application(
            program: &dyn CheckingInput,
            machine: &Machine,
            extent: SymbolHandle,
            length: &FixedArrayLength,
        ) -> Result<bool, &'static str> {
            let substitution = if let Some(application) =
                program.machine_specialization(machine.symbol)
            {
                let Some(typed_trees::static_applications::StaticMachineBindingValue::Const(
                    actual,
                )) = program.specialization_binding(application, extent)
                else {
                    return Err("descriptor operation extent has no exact retained const argument");
                };
                Some((extent, *actual))
            } else {
                None
            };
            let expected = FixedArrayLength::ConstParameter {
                symbol: extent,
                name: Default::default(),
            };
            let expected = program.fixed_array_length_identity_with_simultaneous_substitutions(
                &expected,
                substitution.as_slice(),
            );
            let actual = program.fixed_array_length_identity(length, &[], &[]);
            Ok(expected.is_some() && expected == actual)
        }
    }
    pub(crate) mod segment_registers {
        use super::descriptor_declarations::{
            array_loan, boundary_state, extent_matches_application, fixed_array_length,
            machine_control_service, plain_primitive,
        };
        use crate::CheckingInput;
        use language_core::descriptor_tables::{
            DescriptorTableOperation, SegmentRegister, SegmentReload,
        };
        use language_semantics::{ReferenceAccess, ServiceReachId};
        use symbols::SymbolHandle;
        use typed_trees::data::TypeParameterKind;
        use typed_trees::machine::Machine;
        use typed_trees::types::PrimitiveType;

        #[derive(Clone, Copy)]
        pub(crate) enum SegmentDeclaration {
            Reload {
                extent: SymbolHandle,
                kind: SegmentReload,
                service: ServiceReachId,
            },
            Read {
                register: SegmentRegister,
                service: ServiceReachId,
            },
        }
        impl SegmentDeclaration {
            pub(crate) const fn service(self) -> ServiceReachId {
                match self {
                    Self::Reload { service, .. } | Self::Read { service, .. } => service,
                }
            }
        }

        #[derive(Clone, Copy)]
        pub(crate) enum SelectorOperation {
            Reload(SegmentReload),
            Read(SegmentRegister),
        }

        pub(crate) fn declaration(
            program: &dyn CheckingInput,
            machine: &Machine,
        ) -> Result<Option<SegmentDeclaration>, &'static str> {
            let original = program
                .machine_specialization(machine.symbol)
                .map_or(machine.symbol, |application| application.template);
            let path = program.symbol_toolchain_relative_path(original);
            let code = path == Some(std::path::Path::new("code_segment.omg"));
            let data = path == Some(std::path::Path::new("data_segment.omg"));
            if !code && !data {
                return Ok(None);
            }
            let identity = program
                .normalized_hermetic_symbol_identity(original)
                .map_err(|_| "selector operation has no exact toolchain identity")?;
            let operation = match (code, identity.as_str()) {
                (true, "toolchain::reload_code_segment") => {
                    SelectorOperation::Reload(SegmentReload::Code)
                }
                (true, "toolchain::read_code_selector") => {
                    SelectorOperation::Read(SegmentRegister::Code)
                }
                (false, "toolchain::reload_data_segments") => {
                    SelectorOperation::Reload(SegmentReload::Data)
                }
                (false, "toolchain::read_data_selector") => {
                    SelectorOperation::Read(SegmentRegister::Data)
                }
                (false, "toolchain::read_extra_selector") => {
                    SelectorOperation::Read(SegmentRegister::Extra)
                }
                (false, "toolchain::read_stack_selector") => {
                    SelectorOperation::Read(SegmentRegister::Stack)
                }
                (false, "toolchain::read_fs_selector") => {
                    SelectorOperation::Read(SegmentRegister::Fs)
                }
                (false, "toolchain::read_gs_selector") => {
                    SelectorOperation::Read(SegmentRegister::Gs)
                }
                _ => return Ok(None),
            };
            let original_machine = program
                .machines()
                .iter()
                .find(|candidate| candidate.symbol == original)
                .ok_or("selector operation lost its original declaration")?;
            if original_machine.is_public {
                return Err("selector primitive must be private");
            }
            let state = boundary_state(program, original_machine)?;
            let service = machine_control_service(program, original_machine)?;
            if machine_control_service(program, machine)? != service {
                return Err("selector application changed its exact MachineControl service");
            }
            let telescope = program
                .machine_specialization(original_machine.symbol)
                .map(|application| {
                    program
                        .data_type_parameters
                        .span_or_empty(application.template_parameters)
                })
                .unwrap_or_else(|| program.machine_type_parameters(original_machine));
            let kind = match operation {
                SelectorOperation::Read(register) => {
                    if !telescope.is_empty()
                        || !program.state_parameters(state).is_empty()
                        || !plain_primitive(program, state.return_type, PrimitiveType::U16)
                    {
                        return Err(
                            "selector read requires no arguments and an unqualified u16 result",
                        );
                    }
                    return Ok(Some(SegmentDeclaration::Read { register, service }));
                }
                SelectorOperation::Reload(kind) => kind,
            };
            let [extent] = telescope else {
                return Err("selector reload requires one const-u64 table extent");
            };
            let TypeParameterKind::Const { type_reference } = extent.kind else {
                return Err("selector table extent must be a const-u64 parameter");
            };
            if !plain_primitive(program, type_reference, PrimitiveType::U64) {
                return Err("selector table extent must be an unqualified u64");
            }
            reload_signature(program, original_machine, extent.symbol)?;
            Ok(Some(SegmentDeclaration::Reload {
                extent: extent.symbol,
                kind,
                service,
            }))
        }

        pub(crate) fn operation(
            program: &dyn CheckingInput,
            machine: &Machine,
        ) -> Result<Option<(DescriptorTableOperation, ServiceReachId)>, &'static str> {
            let Some(contract) = declaration(program, machine)? else {
                return Ok(None);
            };
            let operation = match contract {
                SegmentDeclaration::Read { register, .. } => {
                    let state = boundary_state(program, machine)?;
                    if !program.state_parameters(state).is_empty()
                        || !plain_primitive(program, state.return_type, PrimitiveType::U16)
                    {
                        return Err("selector read application changed its checked signature");
                    }
                    DescriptorTableOperation::ReadSegment { register }
                }
                SegmentDeclaration::Reload { extent, kind, .. } => {
                    let application = program
                        .machine_specialization(machine.symbol)
                        .ok_or("selector reload requires a closed table-extent application")?;
                    if !matches!(
                        program.specialization_binding(application, extent),
                        Some(typed_trees::static_applications::StaticMachineBindingValue::Const(_))
                    ) {
                        return Err("selector reload lost its exact const-extent substitution");
                    }
                    reload_signature(program, machine, extent)?;
                    let state = boundary_state(program, machine)?;
                    let gdt = &program.state_parameters(state)[0];
                    let entries = fixed_array_length(
                        program,
                        gdt.type_reference,
                        ReferenceAccess::Mutable,
                        PrimitiveType::U64,
                    )?;
                    if !(1..=8192).contains(&entries) {
                        return Err("selector reload requires 1..8192 GDT words");
                    }
                    DescriptorTableOperation::ReloadSegments {
                        entries: entries as u16,
                        kind,
                    }
                }
            };
            Ok(Some((operation, contract.service())))
        }

        fn reload_signature(
            program: &dyn CheckingInput,
            machine: &Machine,
            extent: SymbolHandle,
        ) -> Result<(), &'static str> {
            let state = boundary_state(program, machine)?;
            let [gdt, selector, scratch] = program.state_parameters(state) else {
                return Err("selector reload requires GDT, selector and scratch operands");
            };
            if !gdt.is_mutable || selector.is_mutable || !scratch.is_mutable {
                return Err(
                    "selector reload requires mutable table/scratch and a read-only selector",
                );
            }
            let (_, length) = array_loan(
                program,
                gdt.type_reference,
                ReferenceAccess::Mutable,
                PrimitiveType::U64,
            )?;
            if !extent_matches_application(program, machine, extent, length)?
                || !plain_primitive(program, selector.type_reference, PrimitiveType::U16)
                || !plain_primitive(program, state.return_type, PrimitiveType::U8)
                || fixed_array_length(
                    program,
                    scratch.type_reference,
                    ReferenceAccess::Mutable,
                    PrimitiveType::U8,
                )? != 10
            {
                return Err(
                    "selector reload changed its table extent, u16 selector, ten scratch bytes or byte result",
                );
            }
            Ok(())
        }
    }
    pub(crate) mod static_interrupts {
        use crate::CheckingInput;
        use language_core::static_interrupts::StaticInterruptOperation;
        use language_semantics::{MachineSupplyMode, ReferenceAccess};
        use symbols::SymbolHandle;
        use typed_trees::data::{MachineParameterContract, TypeParameter, TypeParameterKind};
        use typed_trees::machine::Machine;
        use typed_trees::signature::SignatureContractKind;
        use typed_trees::static_applications::StaticMachineBindingValue;
        use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

        const DECLARATIONS: &[u8] = language_core::static_interrupts::CORE_DECLARATIONS;
        const INTERRUPT_DECLARATIONS: &[u8] = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../../source/library/core/interrupt.omg"
        ));

        #[derive(Debug, Clone, Copy)]
        pub struct StaticInterruptDeclaration {
            pub operation: StaticInterruptOperation,
            pub declaration: SymbolHandle,
            pub entry: SymbolHandle,
        }

        impl StaticInterruptDeclaration {
            pub fn preserves_parameter_descriptors(self) -> bool {
                self.operation == StaticInterruptOperation::Install
            }
        }

        #[derive(Debug, Clone, Copy)]
        pub struct StaticInterruptContext {
            pub declaration: SymbolHandle,
            pub entry: SymbolHandle,
            pub referent: TypeReferenceHandle,
            pub acknowledgement: TypeReferenceHandle,
        }

        pub fn static_interrupt_context(
            program: &dyn CheckingInput,
            target: SymbolHandle,
        ) -> Result<Option<StaticInterruptContext>, &'static str> {
            let Some(machine) = program
                .machine_by_symbol(target)
                .or_else(|| program.machine_holding_state(target))
            else {
                return Ok(None);
            };
            let Some(declaration) = static_interrupt_declaration(program, machine)? else {
                return Ok(None);
            };
            if declaration.operation == StaticInterruptOperation::Context
                && crate::trace_flags::trace_installed_context()
            {
                eprintln!(
                    "INSTALLED_CONTEXT query target={:?} machine={:?} declaration={:?} specialization={:?} open_parameters={} return={:?}",
                    target,
                    machine.symbol,
                    declaration.declaration,
                    program
                        .machine_specialization(machine.symbol)
                        .map(|application| (
                            application.template,
                            application.instance,
                            application.binding_application
                        )),
                    machine.has_specialization_parameters(),
                    program.machine_states(machine).first().map(|state| program
                        .type_reference_table
                        .type_reference(state.return_type))
                );
            }
            if declaration.operation != StaticInterruptOperation::Context
                || program.machine_specialization(machine.symbol).is_none()
            {
                return Ok(None);
            }
            if machine.has_specialization_parameters() {
                return Err("installed context requires a complete closed type application");
            }
            let state = &program.machine_states(machine)[0];
            let (referent, _) = shared_reference(program, state.return_type)?;
            let closed = context_is_closed(program, referent);
            if crate::trace_flags::trace_installed_context() {
                eprintln!(
                    "INSTALLED_CONTEXT referent={:?} closed={closed}",
                    program.type_reference_table.type_reference(referent)
                );
            }
            if !closed {
                return Ok(None);
            }
            Ok(Some(StaticInterruptContext {
                declaration: declaration.declaration,
                entry: state.symbol,
                referent,
                acknowledgement: program.state_parameters(state)[0].type_reference,
            }))
        }

        pub fn static_interrupt_declaration(
            program: &dyn CheckingInput,
            machine: &Machine,
        ) -> Result<Option<StaticInterruptDeclaration>, &'static str> {
            let original = program
                .machine_specialization(machine.symbol)
                .map_or(machine.symbol, |application| application.template);
            let Some(language_core::sealed_intrinsics::SealedIntrinsic::StaticInterrupt(operation)) =
                program.symbols.sealed_intrinsic(original)
            else {
                return Ok(None);
            };
            let template = program
                .machine_by_symbol(original)
                .ok_or("static interrupt application lost its original declaration")?;
            authenticate_shape(program, template, operation)?;
            if template.symbol != machine.symbol {
                authenticate_shape(program, machine, operation)?;
            }
            Ok(Some(StaticInterruptDeclaration {
                operation,
                declaration: original,
                entry: program.machine_states(machine)[0].symbol,
            }))
        }

        fn authenticate_shape(
            program: &dyn CheckingInput,
            machine: &Machine,
            operation: StaticInterruptOperation,
        ) -> Result<(), &'static str> {
            for (invalid, reason) in [
                (
                    machine.body_is_present,
                    "static interrupt declaration unexpectedly has a body",
                ),
                (
                    machine.supply_mode != MachineSupplyMode::Boundary,
                    "static interrupt declaration is not a boundary operation",
                ),
                (
                    machine.suspends || machine.blocks,
                    "static interrupt declaration unexpectedly suspends or blocks",
                ),
                (
                    machine.spelling.is_some(),
                    "static interrupt declaration unexpectedly has a foreign spelling",
                ),
                (
                    machine.target.is_some()
                        || machine.authored_target.is_some()
                        || machine.target_family_interface,
                    "static interrupt declaration unexpectedly has a source target selection",
                ),
                (
                    machine.purpose != typed_trees::machine::MachinePurpose::Runtime,
                    "static interrupt declaration is not a runtime operation",
                ),
                (
                    !machine.owned_data.is_empty(),
                    "static interrupt declaration unexpectedly owns local data",
                ),
                (
                    !machine.conformance_bounds.is_empty(),
                    "static interrupt declaration unexpectedly has conformance bounds",
                ),
                (
                    !program.machine_trait_conformances(machine).is_empty(),
                    "static interrupt declaration unexpectedly implements a trait",
                ),
                (
                    machine.admitted_same_stack_bytes != 0,
                    "static interrupt declaration unexpectedly supplies a stack bound",
                ),
                (
                    machine.service_reach_is_installation_bound,
                    "static interrupt declaration unexpectedly has installation-bound service reach",
                ),
            ] {
                if invalid {
                    return Err(reason);
                }
            }
            let [state] = program.machine_states(machine) else {
                return Err("static interrupt operation must retain one entry signature");
            };
            let parameters = program.state_parameters(state);
            if parameters
                .iter()
                .any(|parameter| parameter.is_const || parameter.relevance.is_erased())
            {
                return Err("static interrupt operation must retain all runtime arguments");
            }
            if !state.statement_nodes.is_empty() {
                return Err("static interrupt boundary entry cannot contain executable statements");
            }
            let telescope = template_parameters(program, machine);
            if operation != StaticInterruptOperation::Install
                && (!machine.invokes.is_empty()
                    || !program
                        .service_reach_rows
                        .services(machine.service_reach_row)
                        .is_empty()
                    || machine.attached_data_symbol.is_valid())
            {
                return Err(
                    "installed context and completion helpers must retain their empty effects",
                );
            }
            match operation {
                StaticInterruptOperation::Install => {
                    let [table, gdt, task_state, stacks, arrivals, context] = parameters else {
                        return Err(
                            "interrupt installation requires table, GDT, task state, stacks, arrivals and context",
                        );
                    };
                    for parameter in [table, gdt, stacks] {
                        require_reference(
                            program,
                            parameter.type_reference,
                            ReferenceAccess::Mutable,
                        )?;
                    }
                    for parameter in [task_state, arrivals, context] {
                        require_reference(
                            program,
                            parameter.type_reference,
                            ReferenceAccess::Shared,
                        )?;
                    }
                }
                StaticInterruptOperation::Context | StaticInterruptOperation::BeginCompletion => {
                    let [acknowledgement] = parameters else {
                        return Err(
                            "installed interrupt context and completion operations require one acknowledgement borrow",
                        );
                    };
                    let (acknowledgement_type, input_lifetime) =
                        shared_reference(program, acknowledgement.type_reference)?;
                    require_core_data(
                        program,
                        acknowledgement_type,
                        "interrupt.omg",
                        "toolchain::InterruptAcknowledgement",
                        INTERRUPT_DECLARATIONS,
                    )?;
                    require_pending(program, machine, state, acknowledgement.symbol)?;
                    if operation == StaticInterruptOperation::Context {
                        let [context] = telescope else {
                            return Err("installed context requires exactly one type argument");
                        };
                        if !matches!(context.kind, TypeParameterKind::Type)
                            || machine.lifetime_parameters.len() != 1
                        {
                            return Err(
                                "installed context changed its type or arrival lifetime binder",
                            );
                        }
                        let (referent, output_lifetime) =
                            shared_reference(program, state.return_type)?;
                        let (Some(input), Some(output)) = (input_lifetime, output_lifetime) else {
                            return Err(
                                "installed context requires explicit input and result lifetimes",
                            );
                        };
                        let input = typed_trees::type_identity::lifetime_parameter_identity(
                            program.typed_program(),
                            machine.symbol,
                            input.as_str(),
                        );
                        let output = typed_trees::type_identity::lifetime_parameter_identity(
                            program.typed_program(),
                            machine.symbol,
                            output.as_str(),
                        );
                        if input.is_none() || input != output {
                            return Err(
                                "installed context must borrow for the acknowledgement's arrival lifetime",
                            );
                        }
                        require_context_type(program, machine, context, referent)?;
                    } else {
                        if !machine.lifetime_parameters.is_empty() || input_lifetime.is_some() {
                            return Err(
                                "completion begin changed its acknowledgement lifetime declaration",
                            );
                        }
                        require_completion_parameter(program, telescope)?;
                        require_core_data(
                            program,
                            state.return_type,
                            "static_interrupt.omg",
                            "toolchain::StaticInterruptCompletionPermit",
                            DECLARATIONS,
                        )?;
                    }
                }
                StaticInterruptOperation::FinishCompletion => {
                    let [permit] = parameters else {
                        return Err("installed interrupt completion requires one consumed permit");
                    };
                    if !telescope.is_empty()
                        || !machine.lifetime_parameters.is_empty()
                        || !machine.contracts.is_empty()
                        || !state.contracts.is_empty()
                        || !matches!(
                            program
                                .type_reference_table
                                .type_reference(state.return_type),
                            TypeReferenceNode::Unit
                        )
                    {
                        return Err(
                            "completion finish must consume its permit and return Unit without contracts",
                        );
                    }
                    require_core_data(
                        program,
                        permit.type_reference,
                        "static_interrupt.omg",
                        "toolchain::StaticInterruptCompletionPermit",
                        DECLARATIONS,
                    )?;
                }
            }
            Ok(())
        }

        fn template_parameters<'a>(
            program: &'a dyn CheckingInput,
            machine: &'a Machine,
        ) -> &'a [TypeParameter] {
            program
                .machine_specialization(machine.symbol)
                .map(|application| {
                    program
                        .data_type_parameters
                        .span_or_empty(application.template_parameters)
                })
                .unwrap_or_else(|| program.machine_type_parameters(machine))
        }

        fn context_is_closed(program: &dyn CheckingInput, referent: TypeReferenceHandle) -> bool {
            let mut pending = vec![referent];
            let mut visited = Vec::new();
            while let Some(reference) = pending.pop() {
                if visited.contains(&reference) {
                    continue;
                }
                visited.push(reference);
                for symbol in typed_trees::references::collect_type_symbol_dependencies(
                    program.typed_program(),
                    reference,
                ) {
                    if !symbol.is_valid() {
                        continue;
                    }
                    if matches!(
                        program.symbols.get(symbol).kind,
                        symbols::SymbolKind::TypeParameter
                            | symbols::SymbolKind::MachineParameter
                            | symbols::SymbolKind::ConformanceParameter
                            | symbols::SymbolKind::Parameter
                            | symbols::SymbolKind::Local
                    ) {
                        return false;
                    }
                    if let Some(application) = program
                        .data_definition_by_symbol(symbol)
                        .and_then(|data| data.generic_instance)
                    {
                        pending.push(application);
                    }
                }
            }
            true
        }

        fn require_context_type(
            program: &dyn CheckingInput,
            machine: &Machine,
            context: &TypeParameter,
            referent: TypeReferenceHandle,
        ) -> Result<(), &'static str> {
            if let Some(application) = program.machine_specialization(machine.symbol) {
                let Some(StaticMachineBindingValue::Type(actual)) =
                    program.specialization_binding(application, context.symbol)
                else {
                    return Err("installed context lost its exact retained type argument");
                };
                let bindings = program
                    .expression_table
                    .static_machine_application_bindings(application.binding_application);
                if bindings.len() != 1 || application.instance != machine.symbol {
                    return Err(
                        "installed context has an incomplete or foreign specialization tuple",
                    );
                }
                let owner = program
                    .expression_table
                    .static_machine_application(application.binding_application)
                    .lexical_owner;
                let expected = typed_trees::type_identity::applied_type_identity(
                    program.typed_program(),
                    *actual,
                    owner,
                    &[],
                );
                let observed = typed_trees::type_identity::applied_type_identity(
                    program.typed_program(),
                    referent,
                    owner,
                    &[],
                );
                if expected.is_none() || expected != observed {
                    return Err("installed context result differs from its exact type argument");
                }
            } else if !matches!(program.type_reference_table.type_reference(referent), TypeReferenceNode::Named { symbol, .. } if *symbol == context.symbol)
            {
                return Err("installed context result must refer to its declared Context binder");
            }
            Ok(())
        }

        fn shared_reference(
            program: &dyn CheckingInput,
            reference: TypeReferenceHandle,
        ) -> Result<(TypeReferenceHandle, Option<&typed_trees::name::Identifier>), &'static str>
        {
            match program.type_reference_table.type_reference(reference) {
                TypeReferenceNode::Reference {
                    referee,
                    access: ReferenceAccess::Shared,
                    lifetime,
                } => Ok((*referee, lifetime.as_ref())),
                _ => Err("installed helper requires an unqualified shared reference"),
            }
        }

        fn authenticate_source(
            program: &dyn CheckingInput,
            symbol: SymbolHandle,
            bytes: &[u8],
        ) -> Result<(), &'static str> {
            let source = program
                .symbols
                .symbol_source_span(symbol)
                .and_then(|span| program.symbols.source_file(span))
                .ok_or("static interrupt operation lost its core source")?;
            if source.origin != source::SourceOrigin::Toolchain
                || source.package_identity.is_some()
                || source.source.as_bytes() != bytes
            {
                return Err(
                    "static interrupt operation does not match the compiler's core declarations",
                );
            }
            Ok(())
        }

        fn require_core_data(
            program: &dyn CheckingInput,
            reference: TypeReferenceHandle,
            path: &str,
            identity: &str,
            bytes: &[u8],
        ) -> Result<(), &'static str> {
            let TypeReferenceNode::Named { symbol, .. } =
                program.type_reference_table.type_reference(reference)
            else {
                return Err("installed helper requires its exact unqualified core token type");
            };
            require_core_symbol(program, *symbol, path, identity)?;
            authenticate_source(program, *symbol, bytes)?;
            let data = program
                .data_definition_by_symbol(*symbol)
                .ok_or("installed helper token lost its data declaration")?;
            if data.supply_mode != language_semantics::DataSupplyMode::BoundaryOpaque
                || data.properties.multiplicity != language_semantics::Multiplicity::Linear
                || !data.members.is_empty()
                || !data.type_parameters.is_empty()
                || !data.lifetime_parameters.is_empty()
            {
                return Err(
                    "installed helper token must retain its opaque linear core declaration",
                );
            }
            Ok(())
        }

        fn require_core_symbol(
            program: &dyn CheckingInput,
            symbol: SymbolHandle,
            path: &str,
            identity: &str,
        ) -> Result<(), &'static str> {
            if program.symbol_toolchain_relative_path(symbol) != Some(std::path::Path::new(path))
                || program
                    .normalized_hermetic_symbol_identity(symbol)
                    .ok()
                    .as_deref()
                    != Some(identity)
            {
                return Err("installed helper selected a different core declaration");
            }
            Ok(())
        }

        fn require_pending(
            program: &dyn CheckingInput,
            machine: &Machine,
            state: &typed_trees::state::State,
            acknowledgement: SymbolHandle,
        ) -> Result<(), &'static str> {
            let mut facts = program
                .machine_contracts(machine)
                .iter()
                .chain(program.state_contracts(state))
                .map(|contract| {
                    if !matches!(contract.kind, SignatureContractKind::Requires)
                        || contract.binding.is_some()
                    {
                        return Err("installed helper changed its Pending-only entry contract");
                    }
                    Ok(program.proof_facts.span_or_empty(contract.facts))
                });
            let mut count = 0;
            for contract in &mut facts {
                for fact in contract? {
                    let typed_trees::domain::ProofFact::Membership(membership) = fact else {
                        return Err(
                            "installed helper requires only its acknowledgement's Pending membership",
                        );
                    };
                    require_core_symbol(
                        program,
                        membership.domain_symbol,
                        "interrupt.omg",
                        "toolchain::InterruptAcknowledgement::Pending",
                    )?;
                    authenticate_source(program, membership.domain_symbol, INTERRUPT_DECLARATIONS)?;
                    let typed_trees::expression::ExpressionNode::Name(subject) =
                        program.expression_table.expression(membership.value)
                    else {
                        return Err(
                            "installed helper Pending contract must name its acknowledgement",
                        );
                    };
                    if subject.resolved_symbol() != Some(acknowledgement)
                        || !membership.domain_arguments.is_empty()
                    {
                        return Err(
                            "installed helper Pending contract names another value or application",
                        );
                    }
                    count += 1;
                }
            }
            if count != 1 {
                return Err("installed helper must retain exactly one Pending precondition");
            }
            Ok(())
        }

        fn require_completion_parameter(
            program: &dyn CheckingInput,
            parameters: &[TypeParameter],
        ) -> Result<(), &'static str> {
            let [completion] = parameters else {
                return Err("completion begin requires exactly one completion function");
            };
            let TypeParameterKind::Machine {
                contract:
                    MachineParameterContract::Nominal {
                        receiving_trait,
                        receiving_arguments,
                        trait_definition,
                        requirement,
                        trait_arguments,
                        instantiated,
                    },
            } = &completion.kind
            else {
                return Err("completion begin requires the core nominal completion contract");
            };
            require_core_symbol(
                program,
                *trait_definition,
                "interrupt.omg",
                "toolchain::InterruptAcknowledgementCompletion",
            )?;
            require_core_symbol(
                program,
                *requirement,
                "interrupt.omg",
                "toolchain::InterruptAcknowledgementCompletion::complete",
            )?;
            if receiving_trait != trait_definition
                || !receiving_arguments.is_empty()
                || !trait_arguments.is_empty()
                || instantiated.is_some()
            {
                return Err("completion begin changed its complete nominal completion requirement");
            }
            Ok(())
        }

        fn require_reference(
            program: &dyn CheckingInput,
            reference: typed_trees::types::TypeReferenceHandle,
            expected: ReferenceAccess,
        ) -> Result<(), &'static str> {
            let reference = typed_trees::operator::unconstrained_type_reference(
                program.typed_program(),
                reference,
            )
            .ok_or("static interrupt argument lost its reference carrier")?;
            match program.type_reference_table.type_reference(reference) {
                TypeReferenceNode::Reference { access, .. } if *access == expected => Ok(()),
                _ => Err("static interrupt argument changed its required borrowed access"),
            }
        }
    }
    pub use static_interrupts::{
        StaticInterruptContext, StaticInterruptDeclaration, static_interrupt_context,
        static_interrupt_declaration,
    };
    pub(crate) mod task_register {
        use super::descriptor_declarations::{
            array_loan, boundary_state, extent_matches_application, fixed_array_length,
            machine_control_service, plain_primitive,
        };
        use crate::CheckingInput;
        use language_core::descriptor_tables::DescriptorTableOperation;
        use language_semantics::{ReferenceAccess, ServiceReachId};
        use symbols::SymbolHandle;
        use typed_trees::data::TypeParameterKind;
        use typed_trees::machine::Machine;
        use typed_trees::types::PrimitiveType;

        #[derive(Clone, Copy)]
        pub(crate) enum TaskRegisterDeclaration {
            Activate {
                extent: SymbolHandle,
                service: ServiceReachId,
            },
            Selector {
                service: ServiceReachId,
            },
        }

        impl TaskRegisterDeclaration {
            pub(crate) const fn service(self) -> ServiceReachId {
                match self {
                    Self::Activate { service, .. } | Self::Selector { service } => service,
                }
            }
        }

        pub(crate) fn task_register_declaration(
            program: &dyn CheckingInput,
            machine: &Machine,
        ) -> Result<Option<TaskRegisterDeclaration>, &'static str> {
            let original = program
                .machine_specialization(machine.symbol)
                .map_or(machine.symbol, |application| application.template);
            if program.symbol_toolchain_relative_path(original)
                != Some(std::path::Path::new("task_register.omg"))
            {
                return Ok(None);
            }
            let identity = program
                .normalized_hermetic_symbol_identity(original)
                .map_err(|_| "task-register declaration has no exact toolchain identity")?;
            let activate = match identity.as_str() {
                "toolchain::activate_task_register" => true,
                "toolchain::read_task_register" => false,
                _ => return Ok(None),
            };
            let declaration = program
                .machines()
                .iter()
                .find(|candidate| candidate.symbol == original)
                .ok_or("task-register application lost its original declaration")?;
            if declaration.is_public {
                return Err("task-register primitive declaration must be private");
            }
            let state = boundary_state(program, declaration)?;
            let service = machine_control_service(program, declaration)?;
            if machine_control_service(program, machine)? != service {
                return Err("task-register application changed its MachineControl service");
            }
            let parameters = program.state_parameters(state);
            let telescope = program
                .machine_specialization(declaration.symbol)
                .map(|application| {
                    program
                        .data_type_parameters
                        .span_or_empty(application.template_parameters)
                })
                .unwrap_or_else(|| program.machine_type_parameters(declaration));
            if !activate {
                if !parameters.is_empty()
                    || !telescope.is_empty()
                    || !plain_primitive(program, state.return_type, PrimitiveType::U16)
                {
                    return Err(
                        "task-register read requires no arguments and an unqualified u16 result",
                    );
                }
                return Ok(Some(TaskRegisterDeclaration::Selector { service }));
            }
            let [extent] = telescope else {
                return Err("task-register activation requires one const-u64 table extent");
            };
            let TypeParameterKind::Const { type_reference } = extent.kind else {
                return Err("task-register extent must be a const-u64 parameter");
            };
            if !plain_primitive(program, type_reference, PrimitiveType::U64) {
                return Err("task-register extent must be an unqualified u64");
            }
            let [gdt, task_state, selector, scratch] = parameters else {
                return Err(
                    "task-register activation requires GDT, task-state, selector and scratch operands",
                );
            };
            if !gdt.is_mutable
                || task_state.is_mutable
                || selector.is_mutable
                || !scratch.is_mutable
            {
                return Err(
                    "task-register activation requires mutable GDT and scratch operands and read-only task-state and selector operands",
                );
            }
            let (_, length) = array_loan(
                program,
                gdt.type_reference,
                ReferenceAccess::Mutable,
                PrimitiveType::U64,
            )?;
            if !extent_matches_application(program, declaration, extent.symbol, length)?
                || fixed_array_length(
                    program,
                    task_state.type_reference,
                    ReferenceAccess::Shared,
                    PrimitiveType::U8,
                )? != 104
                || fixed_array_length(
                    program,
                    scratch.type_reference,
                    ReferenceAccess::Mutable,
                    PrimitiveType::U8,
                )? != 10
                || !plain_primitive(program, selector.type_reference, PrimitiveType::U16)
                || !plain_primitive(program, state.return_type, PrimitiveType::U8)
            {
                return Err(
                    "task-register declaration changed its table extent, 104 task bytes, ten scratch bytes, u16 selector or byte result",
                );
            }
            Ok(Some(TaskRegisterDeclaration::Activate {
                extent: extent.symbol,
                service,
            }))
        }

        pub(crate) fn task_register_operation(
            program: &dyn CheckingInput,
            machine: &Machine,
        ) -> Result<Option<(DescriptorTableOperation, ServiceReachId)>, &'static str> {
            let Some(contract) = task_register_declaration(program, machine)? else {
                return Ok(None);
            };
            let state = boundary_state(program, machine)?;
            let parameters = program.state_parameters(state);
            let operation = match contract {
                TaskRegisterDeclaration::Selector { .. } => {
                    if !parameters.is_empty()
                        || !plain_primitive(program, state.return_type, PrimitiveType::U16)
                    {
                        return Err("task-register read application changed its signature");
                    }
                    DescriptorTableOperation::ReadTask
                }
                TaskRegisterDeclaration::Activate { extent, .. } => {
                    let application = program.machine_specialization(machine.symbol).ok_or(
                        "task-register activation requires a closed table-extent application",
                    )?;
                    if !matches!(
                        program.specialization_binding(application, extent),
                        Some(typed_trees::static_applications::StaticMachineBindingValue::Const(_))
                    ) {
                        return Err(
                            "task-register activation lost its exact const-extent substitution",
                        );
                    }
                    let [gdt, task_state, selector, scratch] = parameters else {
                        return Err("task-register application changed its four runtime operands");
                    };
                    if !gdt.is_mutable
                        || task_state.is_mutable
                        || selector.is_mutable
                        || !scratch.is_mutable
                    {
                        return Err("task-register application changed its operand mutability");
                    }
                    let (_, length) = array_loan(
                        program,
                        gdt.type_reference,
                        ReferenceAccess::Mutable,
                        PrimitiveType::U64,
                    )?;
                    if !extent_matches_application(program, machine, extent, length)? {
                        return Err(
                            "task-register GDT extent differs from its retained const argument",
                        );
                    }
                    let entries = fixed_array_length(
                        program,
                        gdt.type_reference,
                        ReferenceAccess::Mutable,
                        PrimitiveType::U64,
                    )?;
                    if !(2..=8192).contains(&entries)
                        || fixed_array_length(
                            program,
                            task_state.type_reference,
                            ReferenceAccess::Shared,
                            PrimitiveType::U8,
                        )? != 104
                        || fixed_array_length(
                            program,
                            scratch.type_reference,
                            ReferenceAccess::Mutable,
                            PrimitiveType::U8,
                        )? != 10
                        || !plain_primitive(program, selector.type_reference, PrimitiveType::U16)
                        || !plain_primitive(program, state.return_type, PrimitiveType::U8)
                    {
                        return Err(
                            "task-register application requires 2..8192 GDT words, 104 task bytes, ten scratch bytes, u16 selector and byte result",
                        );
                    }
                    DescriptorTableOperation::ActivateTask {
                        entries: entries as u16,
                    }
                }
            };
            Ok(Some((operation, contract.service())))
        }
    }
    pub use build_declarations::{
        build_operation, build_product_request_intrinsic, is_build_declaration_selection,
        resolve_build_static_operand,
    };
    use language_semantics::byte_predicates::ByteSequencePredicate;
    pub fn descriptor_table_declaration(
        program: &dyn crate::CheckingInput,
        machine: &typed_trees::machine::Machine,
    ) -> Result<Option<language_semantics::ServiceReachId>, &'static str> {
        if let Some(declaration) = task_register::task_register_declaration(program, machine)? {
            return Ok(Some(declaration.service()));
        }
        Ok(segment_registers::declaration(program, machine)?
            .map(|declaration| declaration.service()))
    }

    pub fn descriptor_table_operation(
        program: &dyn crate::CheckingInput,
        machine: &typed_trees::machine::Machine,
    ) -> Result<
        Option<(
            language_core::descriptor_tables::DescriptorTableOperation,
            language_semantics::ServiceReachId,
        )>,
        &'static str,
    > {
        if let Some(operation) = task_register::task_register_operation(program, machine)? {
            return Ok(Some(operation));
        }
        segment_registers::operation(program, machine)
    }
    use typed_trees::types::TypeReferenceNode;

    use crate::frozen_program::expression_table;
    use language_semantics::declaration_selection::{
        AuthoredDeclarationSelectionIntrinsic, BuildOperation,
    };
    use symbols::SymbolHandle;
    use typed_trees::references::{StaticArgumentCall, StaticArgumentRole};

    pub fn static_argument_role(
        program: &typed_trees::TypedTrees,
        call: StaticArgumentCall<'_>,
    ) -> StaticArgumentRole {
        use language_semantics::declaration_selection::AuthoredDeclarationSelectionIntrinsic as Intrinsic;

        match static_call_intrinsic(program, call) {
            Some(
                Intrinsic::BuildProviderSelection
                | Intrinsic::BuildBoundaryAcceptance
                | Intrinsic::BuildWireCompatibilityRequest
                | Intrinsic::BuildRepresentationSelection
                | Intrinsic::BuildServiceExclusion
                | Intrinsic::BuildServiceAudit
                | Intrinsic::BuildTestGroupRegistration
                | Intrinsic::BuildAssumedBindingContract,
            ) => StaticArgumentRole::DeclarationDescription,
            _ => StaticArgumentRole::ValueDependency,
        }
    }

    pub(crate) fn static_call_intrinsic(
        program: &dyn crate::CheckingInput,
        call: StaticArgumentCall<'_>,
    ) -> Option<AuthoredDeclarationSelectionIntrinsic> {
        authored_call_intrinsic(program, call).or_else(|| match call {
            StaticArgumentCall::Expression(_, call) => {
                classify_expression_call_intrinsic(program, call)
            }
            StaticArgumentCall::Statement(call) => classify_statement_call_intrinsic(program, call),
        })
    }

    pub(crate) fn validate_declaration_operands(
        program: &dyn crate::CheckingInput,
        call: StaticArgumentCall<'_>,
    ) -> Result<(), diagnostics::Diagnostic> {
        if static_call_intrinsic(program, call)
            == Some(AuthoredDeclarationSelectionIntrinsic::BuildProviderSelection)
            && call
                .arguments(program.typed_program())
                .iter()
                .any(|argument| {
                    !typed_trees::product_operand::is_declaration_operand(
                        program.typed_program(),
                        argument,
                    )
                })
        {
            return Err(diagnostics::Diagnostic::error(
                "provider selection operands must name bare declarations, not applied types, conformance applications, values or projections",
            ));
        }
        Ok(())
    }

    pub fn authored_call_intrinsic(
        program: &dyn crate::CheckingInput,
        call: StaticArgumentCall<'_>,
    ) -> Option<AuthoredDeclarationSelectionIntrinsic> {
        use language_semantics::declaration_selection::{
            AuthoredDeclarationSelectionKind, AuthoredDeclarationSelectionTarget,
        };
        let selections = program.authored_declaration_selections();
        let selection = match call {
            StaticArgumentCall::Expression(expression, _) => expression_table(program)
                .authored_selection_occurrences(expression)
                .filter_map(|occurrence| selections.get(occurrence))
                .filter(|selection| selection.kind() == AuthoredDeclarationSelectionKind::Call),
            StaticArgumentCall::Statement(call) => {
                return call
                    .authored_call_selection
                    .and_then(|occurrence| selections.get(occurrence))
                    .filter(|selection| selection.kind() == AuthoredDeclarationSelectionKind::Call)
                    .and_then(|selection| match selection.target() {
                        AuthoredDeclarationSelectionTarget::Intrinsic(intrinsic) => Some(intrinsic),
                        _ => None,
                    });
            }
        };
        let mut occurrences = selection;
        let selection = occurrences.next()?;
        if occurrences.next().is_some() {
            return None;
        }
        match selection.target() {
            AuthoredDeclarationSelectionTarget::Intrinsic(intrinsic) => Some(intrinsic),
            _ => None,
        }
    }

    pub fn classify_statement_call_intrinsic(
        _program: &dyn crate::CheckingInput,
        call: &typed_trees::statement::TableCall,
    ) -> Option<AuthoredDeclarationSelectionIntrinsic> {
        if call.target_symbol.is_valid() {
            return None;
        }
        checked_call_intrinsic(
            call.target.as_str(),
            call.target_symbol,
            typed_trees::expression::ExpressionHandle::invalid(),
        )
    }

    pub fn statement_wire_codec_intrinsic(
        program: &dyn crate::CheckingInput,
        call: &typed_trees::statement::TableCall,
    ) -> Option<AuthoredDeclarationSelectionIntrinsic> {
        use language_semantics::declaration_selection::AuthoredDeclarationSelectionIntrinsic as Intrinsic;

        if call.target_symbol.is_valid() {
            return None;
        }
        let intrinsic = match call.target.as_str() {
            "encode" => Intrinsic::WireEncode,
            "decode" => Intrinsic::WireDecode,
            _ => return None,
        };
        let members = program.statement_table.name_path_members(call.receiver);
        let leaf = members.last()?;
        program
            .wire_schemas()
            .iter()
            .any(|schema| schema.name.as_str() == leaf.as_str())
            .then_some(intrinsic)
    }

    pub fn single_exclusive_reference_storage(
        program: &dyn crate::CheckingInput,
        machine: &typed_trees::machine::Machine,
        argument: typed_trees::expression::ExpressionHandle,
    ) -> Option<(symbols::SymbolHandle, Vec<facts::PlaceSegment>)> {
        crate::machine_calls::calls::single_exclusive_reference_storage(program, machine, argument)
    }

    pub fn classify_expression_call_intrinsic(
        program: &dyn crate::CheckingInput,
        call: &typed_trees::expression::TableCallExpression,
    ) -> Option<AuthoredDeclarationSelectionIntrinsic> {
        if build_declarations::is_provider_selection_call(program, call) {
            return Some(AuthoredDeclarationSelectionIntrinsic::BuildProviderSelection);
        }
        if let Some(
            operation @ (BuildOperation::ServiceExclusion
            | BuildOperation::ServiceAudit
            | BuildOperation::RepresentationSelection
            | BuildOperation::BoundaryAcceptance
            | BuildOperation::WireCompatibilityRequest
            | BuildOperation::TestGroupRegistration
            | BuildOperation::AssumedBindingContract),
        ) = BuildOperation::from_call_target(call.target.as_str())
            && build_declarations::has_explicit_build_argument(program, call)
            && expression_table(program)
                .expression_handles(call.arguments)
                .len()
                == 1
            && match operation {
                BuildOperation::WireCompatibilityRequest => call.machine_arguments.len() >= 5,
                BuildOperation::RepresentationSelection
                | BuildOperation::AssumedBindingContract => call.machine_arguments.len() == 2,
                _ => call.machine_arguments.len() == 1,
            }
        {
            return Some(operation.intrinsic());
        }
        checked_call_intrinsic(call.target.as_str(), call.target_symbol, call.receiver)
    }

    fn checked_call_intrinsic(
        target: &str,
        target_symbol: SymbolHandle,
        receiver: typed_trees::expression::ExpressionHandle,
    ) -> Option<language_semantics::declaration_selection::AuthoredDeclarationSelectionIntrinsic>
    {
        use language_semantics::declaration_selection::AuthoredDeclarationSelectionIntrinsic as Intrinsic;

        if target_symbol.is_valid() || receiver.is_valid() {
            None
        } else if let Some(predicate) = ByteSequencePredicate::from_name(target) {
            Some(Intrinsic::ByteSequencePredicate(predicate))
        } else if target == "address_of" {
            Some(Intrinsic::StorageAddress)
        } else if target.starts_with("asm#") {
            Some(Intrinsic::InlineAssemblyOperation)
        } else {
            None
        }
    }

    pub fn type_reference_names_exact_prelude_data(
        program: &dyn crate::CheckingInput,
        type_reference: typed_trees::types::TypeReferenceHandle,
        name: &str,
    ) -> bool {
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Reference { referee, .. } => {
                type_reference_names_exact_prelude_data(program, *referee, name)
            }
            TypeReferenceNode::Constrained { base_type, .. } => {
                type_reference_names_exact_prelude_data(program, *base_type, name)
            }
            TypeReferenceNode::Named { symbol, .. } => {
                exact_build_prelude_data(program, *symbol, name)
            }
            _ => false,
        }
    }

    pub fn exact_build_prelude_data(
        program: &dyn crate::CheckingInput,
        type_symbol: SymbolHandle,
        name: &str,
    ) -> bool {
        program
            .symbols
            .symbol_source_span(type_symbol)
            .and_then(|span| program.symbols.source_file(span))
            .is_some_and(|source| {
                source.origin == source::SourceOrigin::Toolchain
                    && source.path == std::path::Path::new("<build-prelude>")
            })
            && program
                .data_definitions()
                .iter()
                .any(|data| data.symbol == type_symbol && data.name.as_str() == name)
    }

    pub fn storage_address_operand(
        program: &dyn crate::CheckingInput,
        call: &typed_trees::expression::TableCallExpression,
    ) -> Option<typed_trees::expression::ExpressionHandle> {
        (classify_expression_call_intrinsic(program, call)
            == Some(AuthoredDeclarationSelectionIntrinsic::StorageAddress)
            && call.machine_arguments.is_empty())
        .then(|| expression_table(program).expression_handles(call.arguments))
        .filter(|arguments| arguments.len() == 1)
        .map(|arguments| arguments[0])
    }

    pub(crate) fn call_preserves_reference_slots(
        program: &dyn crate::CheckingInput,
        call: StaticArgumentCall<'_>,
    ) -> bool {
        authored_call_intrinsic(program, call)
            == Some(AuthoredDeclarationSelectionIntrinsic::BuildProviderSelection)
            || matches!(
                static_call_intrinsic(program, call),
                Some(
                    AuthoredDeclarationSelectionIntrinsic::ByteSequencePredicate(_)
                        | AuthoredDeclarationSelectionIntrinsic::StorageAddress
                )
            )
            || matches!(call, StaticArgumentCall::Expression(_, call)
            if expression_call_only_observes_operands(program, call))
    }

    pub(crate) fn expression_call_only_observes_operands(
        program: &dyn crate::CheckingInput,
        call: &typed_trees::expression::TableCallExpression,
    ) -> bool {
        call.layout_query.is_some()
            || matches!(
                classify_expression_call_intrinsic(program, call),
                Some(
                    AuthoredDeclarationSelectionIntrinsic::ByteSequencePredicate(_)
                        | AuthoredDeclarationSelectionIntrinsic::StorageAddress
                )
            )
    }

    pub fn global_descriptor_table_operand(
        program: &dyn crate::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: Option<&typed_trees::state::State>,
        operand: typed_trees::expression::ExpressionHandle,
        operation: language_core::inline_assembly::AsmGlobalDescriptorTableOperation,
    ) -> Result<typed_trees::types::TypeReferenceHandle, &'static str> {
        use language_core::inline_assembly::AsmGlobalDescriptorTableOperation as Operation;
        use language_semantics::ReferenceAccess;
        let reference = crate::expression_result_type_reference(
            program,
            machine,
            state.ok_or("descriptor instruction has no enclosing machine body")?,
            operand,
        )
        .ok_or("operand has no checked reference type")?;
        let shell = crate::reference_shell(program, reference)
            .ok_or("operand must be a borrowed ten-byte array, not a numeric address")?;
        match (operation, shell.access) {
            (Operation::Load, ReferenceAccess::WriteOnly) => {
                return Err("lgdt needs readable bytes");
            }
            (Operation::Store, ReferenceAccess::Shared) => return Err("sgdt needs writable bytes"),
            _ => {}
        }
        let TypeReferenceNode::FixedArray {
            element_type: element,
            ..
        } = program.type_reference_table.type_reference(shell.referee)
        else {
            return Err("descriptor must be an unqualified fixed byte array");
        };
        let TypeReferenceNode::Named { symbol, .. } =
            program.type_reference_table.type_reference(*element)
        else {
            return Err("descriptor bytes must be unqualified u8 values");
        };
        if program.symbols.builtin_type_atom(*symbol) != Some(symbols::BuiltinTypeAtom::U8)
            || program.closed_fixed_array_length(shell.referee) != Some(10)
        {
            return Err("descriptor must be exactly [u8; 10]");
        }
        Ok(reference)
    }

    pub fn global_descriptor_table_operation(
        program: &dyn crate::CheckingInput,
        target: symbols::SymbolHandle,
    ) -> Option<language_core::inline_assembly::AsmGlobalDescriptorTableOperation> {
        use language_core::inline_assembly::AsmGlobalDescriptorTableOperation as Operation;
        match program.symbols.builtin_function_for_symbol(target)? {
            symbols::BuiltinFunction::AsmStoreGdtr => Some(Operation::Store),
            symbols::BuiltinFunction::AsmLoadGdtr => Some(Operation::Load),
            _ => None,
        }
    }

    pub fn global_descriptor_table_operand_types(
        program: &dyn crate::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        target: symbols::SymbolHandle,
        arguments: &[typed_trees::expression::ExpressionHandle],
    ) -> Option<[typed_trees::types::TypeReferenceHandle; 1]> {
        let operation = global_descriptor_table_operation(program, target)?;
        let [argument] = arguments else {
            return None;
        };
        Some([
            global_descriptor_table_operand(program, machine, Some(state), *argument, operation)
                .ok()?,
        ])
    }
}

pub mod contract_dependencies {
    use symbols::SymbolHandle;

    pub(crate) fn independent_callee(
        program: &dyn crate::CheckingInput,
        caller: SymbolHandle,
        callee: SymbolHandle,
    ) -> bool {
        independent_application(program, caller, callee, &[])
    }

    pub(crate) fn independent_application(
        program: &dyn crate::CheckingInput,
        caller: SymbolHandle,
        callee: SymbolHandle,
        selections: &[typed_trees::expression::StaticMachineArgument],
    ) -> bool {
        let mut pending = vec![callee];
        pending.extend(
            crate::machine_calls::machine_parameters::machine_selection_dependency_symbols(
                program, selections,
            ),
        );
        independent_declarations(program, caller, pending)
    }

    pub fn independent_call_application(
        program: &dyn crate::CheckingInput,
        caller: SymbolHandle,
        callee: SymbolHandle,
        call: typed_trees::references::StaticArgumentCall<'_>,
    ) -> bool {
        let mut pending = vec![callee];
        pending.extend(
            crate::machine_calls::machine_parameters::call_selection_dependency_symbols(
                program, call,
            ),
        );
        independent_declarations(program, caller, pending)
    }

    pub fn independent_call_site(
        program: &dyn crate::CheckingInput,
        caller: SymbolHandle,
        callee: SymbolHandle,
        site: &crate::machine_calls::calls::CallSite<'_>,
    ) -> bool {
        match site {
            crate::machine_calls::calls::CallSite::Expression { expression, call } => {
                independent_call_application(
                    program,
                    caller,
                    callee,
                    typed_trees::references::StaticArgumentCall::Expression(*expression, call),
                )
            }
            crate::machine_calls::calls::CallSite::Statement(call) => independent_call_application(
                program,
                caller,
                callee,
                typed_trees::references::StaticArgumentCall::Statement(call),
            ),
            crate::machine_calls::calls::CallSite::TransitionNamed { path, .. } => {
                independent_declarations(program, caller, vec![path.symbol])
            }
        }
    }

    fn independent_declarations(
        program: &dyn crate::CheckingInput,
        caller: SymbolHandle,
        mut pending: Vec<SymbolHandle>,
    ) -> bool {
        if !caller.is_valid() {
            return false;
        }
        let Some(caller) = program
            .machine_by_symbol(caller)
            .map(|machine| machine.symbol)
            .or_else(|| {
                crate::machine_calls::calls::machine_state_by_symbol(program, caller)
                    .map(|(machine, _)| machine.symbol)
            })
            .or_else(|| {
                crate::machine_calls::call_cycles::declaration_call_dependencies(program, caller)
                    .map(|(owner, _)| owner)
            })
        else {
            return false;
        };
        let mut visited = Vec::new();
        while let Some(declaration) = pending.pop() {
            let Some((owner, dependencies)) =
                crate::machine_calls::call_cycles::declaration_call_dependencies(
                    program,
                    declaration,
                )
            else {
                return false;
            };
            if owner == caller {
                return false;
            }
            if visited.contains(&owner) {
                continue;
            }
            visited.push(owner);
            pending.extend(dependencies);
        }
        true
    }
}

pub mod contract_identities {
    use crate::contract_structure;
    use crate::digest_encoder::DomainDigestEncoder;
    use crate::frozen_program::expression_table;
    use checked_trees::CheckedSpecializationIdentities;
    use diagnostics::Diagnostic;
    use language_semantics::TerminationInterface;
    use symbols::SymbolHandle;
    use typed_trees::data::TypeParameterKind;
    use typed_trees::type_identity::TypeIdentityRequest;
    use typed_trees::typed_trees::MachineSpecializationCommitment;
    use typed_trees::typed_trees::MachineTemplateCommitment;

    pub fn canonical_template_contract_bytes(
        program: &dyn crate::CheckingInput,
        machine_index: usize,
        reach_inference: &flow_effects::ServiceReachInferencePlan,
        operational: &flow_effects::OperationalPlan,
    ) -> Result<Vec<u8>, Diagnostic> {
        let machine = &program.machines()[machine_index];
        let parameters = program.machine_type_parameters(machine);
        let captured = program
            .conformance_by_symbol(program.declaration_scope_parent(machine.symbol))
            .map(|map| program.conformance_type_parameters(map))
            .unwrap_or(&[]);
        let capture_binders = type_parameter_binders(captured, &[])
            .into_iter()
            .map(|(symbol, name)| (symbol, format!("$capture{name}")))
            .collect::<Vec<_>>();
        let binders = type_parameter_binders(parameters, &capture_binders);
        let content_conservation = crate::content_conservation_plans(program);
        let mut bytes = Vec::new();
        bytes.push(u8::from(machine.structural_type_equations_pending));
        bytes.extend(machine.name.as_str().as_bytes());
        bytes.push(0xff);
        bytes.push(match machine.supply_mode {
            language_semantics::MachineSupplyMode::CheckedBody => 1,
            language_semantics::MachineSupplyMode::Requirement => 2,
            language_semantics::MachineSupplyMode::Boundary => 3,
            language_semantics::MachineSupplyMode::AdmissionClaim => 4,
            language_semantics::MachineSupplyMode::ExternalRealization { .. } => 5,
            language_semantics::MachineSupplyMode::TopLevelRequirement => 6,
        });
        for (index, parameter) in parameters.iter().enumerate() {
            bytes.push(match parameter.kind {
                TypeParameterKind::Type => 1,
                TypeParameterKind::Const { .. } => 2,
                TypeParameterKind::Value { .. } => 5,
                TypeParameterKind::Machine { .. } => 3,
                TypeParameterKind::Proposition { .. } => 4,
                TypeParameterKind::Implementation => 6,
            });
            bytes.extend((index as u32).to_le_bytes());
            encode_data_properties(parameter.bounds, &mut bytes);
            encode_generic_parameter_contract(
                program,
                &parameter.kind,
                &binders,
                &content_conservation,
                &mut bytes,
            )?;
            bytes.push(0xfe);
        }
        let mut conformance_bounds = Vec::new();
        for equation in program
            .declared_type_equations
            .span_or_empty(machine.declared_type_equations)
        {
            for reference in [equation.parameter, equation.definition] {
                encode_identity_text(
                    &program.type_identity_request_text(TypeIdentityRequest {
                        binders: &binders,
                        ..TypeIdentityRequest::ordinary(reference)
                    }),
                    &mut bytes,
                );
            }
            bytes.push(0xf9);
        }
        for bound in &machine.conformance_bounds {
            let mut encoded = Vec::new();
            if !bound.lifetime_arguments.is_empty() {
                encoded.push(0xff);
                encoded.extend((bound.lifetime_arguments.len() as u64).to_le_bytes());
                let owner = program.symbols.get(bound.binder).parent;
                for lifetime in &bound.lifetime_arguments {
                    let identity = typed_trees::type_identity::portable_applied_lifetime_identity(
                        program.typed_program(),
                        lifetime.as_str(),
                        owner,
                        &[],
                    )
                    .and_then(|identity| program.type_identity_text(identity))
                    .ok_or_else(|| {
                        Diagnostic::error("evidence-bound commitment lost its declared region")
                    })?;
                    encode_identity_text(&identity, &mut encoded);
                }
            }
            encoded.extend(bound.trait_name.as_str().as_bytes());
            encoded.push(0);
            for argument in &bound.arguments {
                encode_identity_text(
                    &program.type_identity_request_text(TypeIdentityRequest {
                        binders: &binders,
                        ..TypeIdentityRequest::ordinary(*argument)
                    }),
                    &mut encoded,
                );
                encoded.push(0);
            }
            conformance_bounds.push(encoded);
        }
        conformance_bounds.sort();
        for bound in conformance_bounds {
            bytes.extend(bound);
            bytes.push(0xfb);
        }
        let mut state_shapes = Vec::new();
        for state in program.machine_states(machine) {
            let mut shape = Vec::new();
            encode_state_shape(program, state, &binders, &content_conservation, &mut shape)?;
            state_shapes.push(shape);
        }
        state_shapes.sort();
        for shape in state_shapes {
            bytes.extend(shape);
            bytes.push(0xfd);
        }
        let (service_row, may_suspend, may_block) = if machine.target_family_interface {
            let reach = reach_inference.for_machine(machine.symbol).ok_or_else(|| {
                Diagnostic::error("target-family template has no published service-reach summary")
            })?;
            let operational = operational
                .machine_by_symbol(machine.symbol)
                .ok_or_else(|| {
                    Diagnostic::error("target-family template has no completed operational summary")
                })?;
            (
                reach_inference.services(reach.published),
                operational.transitive_may_suspend,
                operational.transitive_may_block,
            )
        } else {
            (
                program
                    .service_reach_rows
                    .services(machine.service_reach_row),
                machine.suspends,
                machine.blocks,
            )
        };
        let mut service_reaches: Vec<_> = service_row
            .iter()
            .map(|service| {
                program
                    .service_reaches
                    .definition(*service)
                    .expect("normalized service row references a registered service")
            })
            .map(|service| service.name.as_str())
            .collect();
        service_reaches.sort_unstable();
        service_reaches.dedup();
        for service in service_reaches {
            bytes.extend(service.as_bytes());
            bytes.push(0);
        }
        bytes.push(u8::from(
            machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
                || machine.is_public
                || program
                    .authored_service_reach_rows_for(machine.symbol)
                    .next()
                    .is_some()
                || !service_row.is_empty(),
        ));
        bytes.push(u8::from(machine.service_reach_is_installation_bound));
        bytes.push(u8::from(may_suspend));
        bytes.push(u8::from(may_block));
        let contract_binders = program
            .machine_states(machine)
            .first()
            .map(|state| formal_parameter_binders(program.state_parameters(state), &binders))
            .unwrap_or_else(|| binders.clone());
        let contracts = contract_structure::declared_contracts(
            program,
            program.machine_contracts(machine),
            &contract_binders,
            &content_conservation,
            true,
        )?;
        bytes.extend(
            checked_trees::contract_declarations_commitment(&contracts, &program.type_identities)
                .as_bytes(),
        );
        let termination = contract_structure::declared_termination(
            program,
            &machine.termination_plan.interface,
            &contract_binders,
        );
        bytes.extend(checked_trees::declared_termination_commitment(&termination).as_bytes());
        bytes.extend(crate::static_machine_template_reach_contract_bytes(
            program,
            reach_inference,
            machine,
        )?);
        Ok(bytes)
    }

    fn fnv1a_report_fingerprint(bytes: &[u8]) -> u64 {
        let mut hash = semantic_vocabulary::Fnv1a::new();
        hash.bytes(bytes);
        hash.finish()
    }

    pub fn machine_template_commitment(
        canonical_template_contract_bytes: &[u8],
    ) -> MachineTemplateCommitment {
        let mut strong = DomainDigestEncoder::new(b"omega.machine-template.v3\0");
        strong.raw(canonical_template_contract_bytes);
        MachineTemplateCommitment::from_digest(strong.finish())
    }

    pub fn generic_machine_template_report_fingerprint(
        program: &dyn crate::CheckingInput,
        machine_symbol: SymbolHandle,
    ) -> Option<u64> {
        let bytes = canonical_template_bytes_for_symbol(program, machine_symbol)?;
        Some(fnv1a_report_fingerprint(&bytes))
    }

    fn canonical_template_bytes_for_symbol(
        program: &dyn crate::CheckingInput,
        machine_symbol: SymbolHandle,
    ) -> Option<Vec<u8>> {
        let machine_index = generic_machine_index(program, machine_symbol)?;
        let operational = crate::infer_operational_may(program);
        let service_reaches = crate::infer_service_reaches(program, &operational);
        canonical_template_contract_bytes(program, machine_index, &service_reaches, &operational)
            .ok()
    }

    fn generic_machine_index(
        program: &dyn crate::CheckingInput,
        machine_symbol: SymbolHandle,
    ) -> Option<usize> {
        let machine_index = program.machine_position_by_symbol(machine_symbol)?;
        (!program
            .machine_type_parameters(&program.machines()[machine_index])
            .is_empty())
        .then_some(machine_index)
    }

    pub fn generic_machine_template_commitment(
        program: &dyn crate::CheckingInput,
        machine_symbol: SymbolHandle,
    ) -> Option<MachineTemplateCommitment> {
        let bytes = canonical_template_bytes_for_symbol(program, machine_symbol)?;
        Some(machine_template_commitment(&bytes))
    }

    pub fn accepted_template_commitment(
        program: &dyn crate::CheckingInput,
        machine_index: usize,
    ) -> Option<String> {
        let machine = &program.machines()[machine_index];
        (machine.supply_mode == language_semantics::MachineSupplyMode::AdmissionClaim)
            .then(|| machine.name.as_str().to_owned())
    }

    pub fn prepare_specialization_contract_identities(
        program: &dyn crate::CheckingInput,
        contracts: &checked_trees::MachineContractPlans,
        effects: &crate::PreparedEffectPlans,
    ) -> Result<CheckedSpecializationIdentities, Vec<Diagnostic>> {
        let mut templates: Vec<(SymbolHandle, Vec<u8>, MachineTemplateCommitment)> = Vec::new();
        let mut identities = Vec::with_capacity(
            program.machine_specializations.len() + program.captured_machine_applications.len(),
        );
        for (_, capture) in program.captured_machine_applications.iter() {
            crate::machine_calls::calls::capture_binding_is_exact(program, capture).map_err(|reason| {
            let supplying_owner = if capture.supplying_application.is_valid() { {
                let source = expression_table(program).static_machine_application(capture.supplying_application);
                program.symbols.display_path(source.lexical_owner, "::").to_string()
            } } else { "<missing>".into() };
            let declaration_occurrences = program
                .closed_conformance_application_sites()
                .iter()
                .filter(|(_, _, supplying, _)| *supplying == capture.supplying_application)
                .count();
            vec![Diagnostic::error(format!(
                "capture application does not replay its complete supplying substitution: {reason}; implementation `{}`, body `{}`, supplying owner `{}`, retained source occurrences {declaration_occurrences}",
                program.symbols.display_path(capture.conformance_application.declaration, "::"),
                program.symbols.display_path(capture.template, "::"),
                supplying_owner,
            ))]
        })?;
            if program
                .executable_application_template(capture.instance)
                .map_err(|message| vec![Diagnostic::error(message)])?
                != Some(capture.template)
            {
                return Err(vec![Diagnostic::error(
                    "capture application has conflicting executable ownership",
                )]);
            }
            let template = program.machine(capture.template).ok_or_else(|| {
                vec![Diagnostic::error(
                    "capture application lost its authored template",
                )]
            })?;
            let instance = program.machine(capture.instance).ok_or_else(|| {
                vec![Diagnostic::error(
                    "capture application lost its retained executable instance",
                )]
            })?;
            let states = program
                .captured_machine_states
                .span(capture.states)
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "capture application has a stale state correspondence",
                    )]
                })?;
            let originals = program.machine_states(template);
            let applied = program.machine_states(instance);
            if states.len() != originals.len()
                || states.len() != applied.len()
                || !states.iter().zip(originals.iter().zip(applied)).all(
                    |(mapping, (original, applied))| {
                        mapping.template == original.symbol && mapping.instance == applied.symbol
                    },
                )
            {
                return Err(vec![Diagnostic::error(
                    "capture application does not retain its complete original state correspondence",
                )]);
            }
            let original_parameters = program
                .machine_type_parameters(template)
                .iter()
                .map(|parameter| parameter.symbol)
                .chain(template.conformance_bounds.iter().map(|bound| bound.binder))
                .collect::<Vec<_>>();
            let applied_parameters = program
                .machine_type_parameters(instance)
                .iter()
                .map(|parameter| parameter.symbol)
                .chain(instance.conformance_bounds.iter().map(|bound| bound.binder))
                .collect::<Vec<_>>();
            if capture.parameters.len() != original_parameters.len()
                || capture.parameters.len() != applied_parameters.len()
                || !capture
                    .parameters
                    .iter()
                    .zip(original_parameters.iter().zip(&applied_parameters))
                    .all(|((original, applied), (left, right))| {
                        original == left && applied == right
                    })
            {
                return Err(vec![Diagnostic::error(
                    "capture application lost its exact method binder correspondence",
                )]);
            }
            let index = program
                .machine_position_by_symbol(capture.template)
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "capture application lost its authored template",
                    )]
                })?;
            let template_bytes = canonical_template_contract_bytes(
                program,
                index,
                &effects.service_reaches,
                &effects.operational,
            )
            .map_err(|diagnostic| vec![diagnostic])?;
            let template_commitment = machine_template_commitment(&template_bytes);
            let mut digest = DomainDigestEncoder::new(b"omega.machine-capture-application.v1\0");
            digest.raw(&template_commitment.as_bytes());
            let bytes = canonical_capture_application_bytes(program, contracts, capture)
                .map_err(|diagnostic| vec![diagnostic])?;
            digest.raw(&bytes);
            identities.push(checked_trees::CheckedMachineSpecializationIdentity {
                instance: capture.instance,
                template_contract_commitment: template_commitment,
                machine_argument_contract_commitments: captured_machine_contracts(
                    program, contracts, capture,
                )
                .map_err(|diagnostic| vec![diagnostic])?
                .into_boxed_slice(),
                commitment: MachineSpecializationCommitment::from_digest(digest.finish()),
            });
        }
        for specialization in &program.machine_specializations {
            let template_index = match templates
                .iter()
                .position(|(symbol, _, _)| *symbol == specialization.template)
            {
                Some(template_index) => template_index,
                None => {
                    let machine_index = program
                        .machine_position_by_symbol(specialization.template)
                        .ok_or_else(|| {
                            vec![Diagnostic::error(
                                "generic specialization lost its authored template declaration",
                            )]
                        })?;
                    let bytes = canonical_template_contract_bytes(
                        program,
                        machine_index,
                        &effects.service_reaches,
                        &effects.operational,
                    )
                    .map_err(|diagnostic| vec![diagnostic])?;
                    let commitment = machine_template_commitment(&bytes);
                    let template_index = templates.len();
                    templates.push((specialization.template, bytes, commitment));
                    template_index
                }
            };
            let (_, template_bytes, template_commitment) = &templates[template_index];
            let mut composed_template_bytes = template_bytes.clone();
            if let Some(capture_identity) = identities
                .iter()
                .find(|identity| identity.instance == specialization.template)
            {
                composed_template_bytes.extend(b"omega.invocation-of-captured-method.v1\0");
                composed_template_bytes.extend(capture_identity.commitment.as_bytes());
            }
            identities.push(
                machine_specialization_identity(
                    program,
                    contracts,
                    specialization,
                    &effects.operational,
                    &composed_template_bytes,
                    *template_commitment,
                )
                .map_err(|diagnostic| vec![diagnostic])?,
            );
        }
        Ok(CheckedSpecializationIdentities::with_rows(identities))
    }

    fn captured_machine_contracts(
        program: &dyn crate::CheckingInput,
        contracts: &checked_trees::MachineContractPlans,
        capture: &typed_trees::typed_trees::CapturedMachineApplication,
    ) -> Result<Vec<[u8; 32]>, Diagnostic> {
        let application = &capture.conformance_application;
        let selected = if application.machine_arguments.is_empty() {
            &[][..]
        } else {
            if !application.callable_application.is_valid() {
                return Err(Diagnostic::error(
                    "capture lost its checked callback application",
                ));
            }
            expression_table(program)
                .static_machine_application_selected_machines(application.callable_application)
        };
        if selected.len() != application.machine_arguments.len() {
            return Err(Diagnostic::error(
                "capture callback contract roster differs from map telescope",
            ));
        }
        selected
            .iter()
            .map(|argument| {
                let selected = argument.symbol;
                let owner = program
                    .machines()
                    .iter()
                    .find(|machine| {
                        program
                            .machine_states(machine)
                            .iter()
                            .any(|state| state.symbol == selected)
                    })
                    .ok_or_else(|| {
                        Diagnostic::error("capture machine operand has no exact callable owner")
                    })?;
                contracts
                    .for_machine(owner.symbol)
                    .map(|contract| contract.commitment.as_bytes())
                    .ok_or_else(|| {
                        Diagnostic::error("capture machine operand has no checked public contract")
                    })
            })
            .collect()
    }

    fn canonical_capture_application_bytes(
        program: &dyn crate::CheckingInput,
        contracts: &checked_trees::MachineContractPlans,
        capture: &typed_trees::typed_trees::CapturedMachineApplication,
    ) -> Result<Vec<u8>, Diagnostic> {
        use typed_trees::static_applications::StaticMachineBindingValue;
        let application = &capture.conformance_application;
        let declaration = program
            .conformance_by_symbol(application.declaration)
            .ok_or_else(|| Diagnostic::error("capture application lost its map declaration"))?;
        let bindings = expression_table(program)
            .static_machine_application_bindings(capture.binding_application);
        let mut bytes = Vec::new();
        bytes.extend(application.commitment.as_bytes());
        encode_identity_text(&capture.normalized_template_identity, &mut bytes);
        let owner = expression_table(program)
            .static_machine_application(capture.supplying_application)
            .lexical_owner;
        encode_identity_text(&conformance_symbol_identity(program, owner), &mut bytes);
        let parameters = program.conformance_type_parameters(declaration);
        let binders = type_parameter_binders(parameters, &[]);
        let conservation = crate::content_conservation_plans(program);
        bytes.extend((parameters.len() as u64).to_le_bytes());
        for parameter in parameters {
            encode_data_properties(parameter.bounds, &mut bytes);
            encode_generic_parameter_contract(
                program,
                &parameter.kind,
                &binders,
                &conservation,
                &mut bytes,
            )?;
            let binding = bindings
                .iter()
                .find(|binding| binding.parameter == parameter.symbol)
                .ok_or_else(|| {
                    Diagnostic::error("capture application has an incomplete typed binder tuple")
                })?;
            match &binding.value {
                StaticMachineBindingValue::Type(reference) => {
                    bytes.push(1);
                    encode_identity_text(
                        &program.normalized_type_identity_text(*reference),
                        &mut bytes,
                    );
                }
                StaticMachineBindingValue::Const(reference) => {
                    bytes.push(2);
                    encode_identity_text(
                        &program.normalized_type_identity_text(*reference),
                        &mut bytes,
                    );
                }
                StaticMachineBindingValue::Machine(argument) => {
                    bytes.push(3);
                    encode_identity_text(
                        &conformance_symbol_identity(program, argument.symbol),
                        &mut bytes,
                    );
                }
                _ => {
                    return Err(Diagnostic::error(
                        "capture application retains a runtime or unsupported evidence operand",
                    ));
                }
            }
        }
        for contract in captured_machine_contracts(program, contracts, capture)? {
            bytes.extend(contract);
        }
        match &capture.accepted_template_commitment {
            Some(commitment) => {
                bytes.push(1);
                encode_identity_text(commitment, &mut bytes);
            }
            None => bytes.push(0),
        }
        Ok(bytes)
    }

    fn machine_specialization_identity(
        program: &dyn crate::CheckingInput,
        contracts: &checked_trees::MachineContractPlans,
        specialization: &typed_trees::typed_trees::MachineSpecialization,
        operational: &flow_effects::OperationalPlan,
        template_contract_bytes: &[u8],
        template_contract_commitment: MachineTemplateCommitment,
    ) -> Result<checked_trees::CheckedMachineSpecializationIdentity, Diagnostic> {
        let instance = program
            .machine_by_symbol(specialization.instance)
            .ok_or_else(|| {
                Diagnostic::error("generic specialization lost its concrete instance")
            })?;
        let instance_identity =
            normalized_machine_identity(program, instance).ok_or_else(|| {
                Diagnostic::error(
                    "generic specialization instance has no normalized callable identity",
                )
            })?;

        let mut machine_owner_identities = Vec::new();
        let mut machine_contract_commitments = Vec::new();
        for state_symbol in &specialization.machine_arguments {
            let owner = program
            .machines()
            .iter()
            .find(|machine| {
                typed_trees::state::state_by_symbol(program.machine_states(machine), *state_symbol).is_some()
            })
            .ok_or_else(|| Diagnostic::error(format!(
                "generic specialization references static machine symbol {:?}, but no owning machine exists",
                state_symbol
            )))?;
            let contract = contracts.for_machine(owner.symbol).ok_or_else(|| Diagnostic::error(
            format!(
                "generic specialization selected `{}`, but its normalized machine contract identity is missing",
                owner.name
            ),
        ))?;
            let owner_identity = normalized_machine_identity(program, owner).ok_or_else(|| {
            Diagnostic::error(format!(
                "generic specialization selected `{}`, but its owner identity cannot be normalized",
                owner.name
            ))
        })?;
            let selected_state_identity = conformance_symbol_identity(program, *state_symbol);
            machine_owner_identities.push(format!(
                "{owner_identity}|selected={selected_state_identity}"
            ));
            machine_contract_commitments.push(contract.commitment.as_bytes());
        }

        let mut bytes = Vec::new();
        encode_identity_bytes(template_contract_bytes, &mut bytes);
        bytes.extend(template_contract_commitment.as_bytes());
        encode_identity_text(&specialization.normalized_template_identity, &mut bytes);
        encode_identity_text(&instance_identity, &mut bytes);
        encode_identity_texts(&specialization.type_argument_identities, &mut bytes);
        encode_identity_texts(&specialization.const_argument_identities, &mut bytes);
        bytes.extend((machine_owner_identities.len() as u64).to_le_bytes());
        for (owner, commitment) in machine_owner_identities
            .iter()
            .zip(machine_contract_commitments.iter())
        {
            encode_identity_text(owner, &mut bytes);
            bytes.extend(commitment);
        }
        bytes.extend((specialization.conformance_applications.len() as u64).to_le_bytes());
        for application in &specialization.conformance_applications {
            bytes.extend(application.commitment.as_bytes());
        }
        let operator_realization_bytes = crate::canonical_closed_operator_realization_bytes(
            program,
            specialization.instance,
            &specialization.operator_realizations,
        )
        .map_err(Diagnostic::error)?;
        encode_identity_bytes(&operator_realization_bytes, &mut bytes);
        let static_call_bindings =
            crate::static_machine_call_binding_bytes(program, operational, specialization)?;
        encode_identity_bytes(&static_call_bindings, &mut bytes);
        match &specialization.accepted_template_commitment {
            Some(commitment) => {
                bytes.push(1);
                encode_identity_text(commitment, &mut bytes);
            }
            None => bytes.push(0),
        }
        let mut strong = DomainDigestEncoder::new(b"omega.machine-specialization.v2\0");
        strong.raw(&bytes);
        Ok(checked_trees::CheckedMachineSpecializationIdentity {
            instance: specialization.instance,
            template_contract_commitment,
            machine_argument_contract_commitments: machine_contract_commitments.into_boxed_slice(),
            commitment: MachineSpecializationCommitment::from_digest(strong.finish()),
        })
    }

    fn encode_identity_bytes(value: &[u8], bytes: &mut Vec<u8>) {
        bytes.extend((value.len() as u64).to_le_bytes());
        bytes.extend(value);
    }

    fn encode_identity_text(value: &str, bytes: &mut Vec<u8>) {
        encode_identity_bytes(value.as_bytes(), bytes);
    }

    fn encode_identity_texts(values: &[String], bytes: &mut Vec<u8>) {
        bytes.extend((values.len() as u64).to_le_bytes());
        for value in values {
            encode_identity_text(value, bytes);
        }
    }

    pub fn generic_parameter_contract_bytes(
        program: &dyn crate::CheckingInput,
        parameter: &typed_trees::data::TypeParameter,
        binders: &[(SymbolHandle, String)],
        content_conservation: &[crate::ContentConservationSourcePlan],
    ) -> Result<Vec<u8>, Diagnostic> {
        let mut bytes = Vec::new();
        encode_generic_parameter_contract(
            program,
            &parameter.kind,
            binders,
            content_conservation,
            &mut bytes,
        )?;
        Ok(bytes)
    }

    fn encode_generic_parameter_contract(
        program: &dyn crate::CheckingInput,
        kind: &TypeParameterKind,
        binders: &[(SymbolHandle, String)],
        content_conservation: &[crate::ContentConservationSourcePlan],
        bytes: &mut Vec<u8>,
    ) -> Result<(), Diagnostic> {
        match kind {
            TypeParameterKind::Const { type_reference }
            | TypeParameterKind::Value { type_reference } => encode_identity_text(
                &program.type_identity_request_text(TypeIdentityRequest {
                    binders,
                    ..TypeIdentityRequest::ordinary(*type_reference)
                }),
                bytes,
            ),
            TypeParameterKind::Machine { contract } => match program
                .machine_parameter_contract_view(contract)
                .ok_or_else(|| {
                    Diagnostic::error(
                        "typed machine-parameter contract has no retained requirement identity",
                    )
                })? {
                typed_trees::data::MachineParameterContractView::Structural(signature) => {
                    bytes.push(1);
                    encode_state_signature(
                        program,
                        signature,
                        binders,
                        content_conservation,
                        bytes,
                    )?;
                }
                typed_trees::data::MachineParameterContractView::Nominal {
                    trait_definition,
                    requirement,
                } => {
                    bytes.push(2);
                    let identity = program
                        .trait_requirement_callable_identity_text(trait_definition, requirement);
                    encode_identity_text(&identity, bytes);
                    let typed_trees::data::MachineParameterContract::Nominal {
                        receiving_trait,
                        receiving_arguments,
                        trait_arguments,
                        ..
                    } = contract
                    else {
                        return Err(Diagnostic::error(
                            "nominal machine-parameter view lost its authored trait application",
                        ));
                    };
                    encode_identity_text(
                        &conformance_symbol_identity(program, *receiving_trait),
                        bytes,
                    );
                    bytes.extend((receiving_arguments.len() as u64).to_le_bytes());
                    for argument in receiving_arguments {
                        encode_identity_text(
                            &program.type_identity_request_text(TypeIdentityRequest {
                                binders,
                                ..TypeIdentityRequest::ordinary(*argument)
                            }),
                            bytes,
                        );
                    }
                    bytes.extend((trait_arguments.len() as u64).to_le_bytes());
                    for argument in trait_arguments {
                        encode_identity_text(
                            &program.type_identity_request_text(TypeIdentityRequest {
                                binders,
                                ..TypeIdentityRequest::ordinary(*argument)
                            }),
                            bytes,
                        );
                    }
                    let requirement_binders = type_parameter_binders(
                        program.trait_type_parameters(trait_definition),
                        binders,
                    );
                    encode_state_signature(
                        program,
                        requirement,
                        &requirement_binders,
                        content_conservation,
                        bytes,
                    )?;
                }
            },
            TypeParameterKind::Proposition { contract } => {
                bytes.extend((contract.parameters.len() as u32).to_le_bytes());
                let proposition_parameters =
                    program.state_parameters.span_or_empty(contract.parameters);
                let proposition_binders = formal_parameter_binders(proposition_parameters, binders);
                for parameter in proposition_parameters {
                    encode_parameter(program, parameter, &proposition_binders, bytes);
                }
            }
            TypeParameterKind::Type | TypeParameterKind::Implementation => {}
        }
        Ok(())
    }

    fn encode_data_properties(properties: typed_trees::data::DataProperties, output: &mut Vec<u8>) {
        output.push(match properties.multiplicity {
            language_semantics::Multiplicity::Unrestricted => 1,
            language_semantics::Multiplicity::Affine => 2,
            language_semantics::Multiplicity::Linear => 3,
        });
        if let Some(carry) = properties.carry {
            output.extend(format!("{carry}").as_bytes());
        }
        output.push(0);
    }

    fn type_parameter_binders(
        parameters: &[typed_trees::data::TypeParameter],
        enclosing_binders: &[(SymbolHandle, String)],
    ) -> Vec<(SymbolHandle, String)> {
        let mut binders = enclosing_binders.to_vec();
        for parameter in parameters {
            if binders
                .iter()
                .any(|(symbol, _)| *symbol == parameter.symbol)
            {
                continue;
            }
            let prefix = match parameter.kind {
                TypeParameterKind::Type => "T",
                TypeParameterKind::Const { .. } => "C",
                TypeParameterKind::Value { .. } => "V",
                TypeParameterKind::Machine { .. } => "M",
                TypeParameterKind::Proposition { .. } => "P",
                TypeParameterKind::Implementation => "I",
            };
            let binder_position = binders.len();
            binders.push((parameter.symbol, format!("${prefix}{binder_position}")));
        }
        binders
    }

    fn formal_parameter_binders(
        parameters: &[typed_trees::signature::StateParameter],
        enclosing_binders: &[(SymbolHandle, String)],
    ) -> Vec<(SymbolHandle, String)> {
        let mut binders = enclosing_binders.to_vec();
        binders.extend(
            parameters
                .iter()
                .enumerate()
                .map(|(parameter_position, parameter)| {
                    (parameter.symbol, format!("$argument{parameter_position}"))
                }),
        );
        binders
    }

    fn signature_lifetime_binders(
        program: &dyn crate::CheckingInput,
        signature: &typed_trees::signature::StateSignature,
    ) -> Vec<(typed_trees::type_identity::LifetimeParameterIdentity, usize)> {
        let mut owners = Vec::new();
        let mut owner = signature.symbol;
        while owner.is_valid() && !owners.contains(&owner) {
            owners.push(owner);
            owner = program.declaration_scope_parent(owner);
        }
        let mut binders = Vec::new();
        for owner in owners.into_iter().rev() {
            let declaration = program.lifetime_declaration_owner(owner);
            if let Some(parameters) = typed_trees::type_identity::declaration_lifetime_parameters(
                program.typed_program(),
                declaration,
            ) {
                for ordinal in 0..parameters.len() {
                    let parameter = typed_trees::type_identity::LifetimeParameterIdentity {
                        declaration,
                        ordinal,
                    };
                    if !binders.iter().any(|(known, _)| *known == parameter) {
                        binders.push((parameter, binders.len()));
                    }
                }
            }
        }
        binders
    }

    fn encode_state_signature(
        program: &dyn crate::CheckingInput,
        signature: &typed_trees::signature::StateSignature,
        binders: &[(SymbolHandle, String)],
        content_conservation: &[crate::ContentConservationSourcePlan],
        output: &mut Vec<u8>,
    ) -> Result<(), Diagnostic> {
        let generic_binders = type_parameter_binders(
            program.requirement_invocation_contract_parameters(signature),
            binders,
        );
        let lifetime_binders = signature_lifetime_binders(program, signature);
        output.extend((signature.conformance_bounds.len() as u64).to_le_bytes());
        for bound in &signature.conformance_bounds {
            let ordinal = program
                .requirement_invocation_contract_parameters(signature)
                .iter()
                .position(|parameter| {
                    parameter.symbol == bound.binder
                        && parameter.kind == TypeParameterKind::Implementation
                })
                .ok_or_else(|| {
                    Diagnostic::error("callable evidence bound has no exact implementation slot")
                })?;
            output.extend((ordinal as u64).to_le_bytes());
            let identity = program
                .normalized_hermetic_symbol_identity(bound.trait_symbol)
                .map_err(|reason| {
                    Diagnostic::error(format!(
                        "callable evidence bound lost its exact trait owner: {reason}"
                    ))
                })?;
            encode_identity_text(&identity, output);
            output.extend((bound.arguments.len() as u64).to_le_bytes());
            let definition = program
                .trait_definition_by_symbol(bound.trait_symbol)
                .ok_or_else(|| {
                    Diagnostic::error("callable evidence bound lost its trait declaration")
                })?;
            let parameters = program.trait_type_parameters(definition);
            if parameters.len() != bound.arguments.len() {
                return Err(Diagnostic::error(
                    "callable evidence bound lost its complete trait application",
                ));
            }
            for (parameter, argument) in parameters.iter().zip(&bound.arguments) {
                use typed_trees::type_identity::{
                    DeclaredStaticOperandIdentityRequest, IndexIdentityArgument,
                };
                let identity = program
                    .declared_static_operand_identity(DeclaredStaticOperandIdentityRequest {
                        operand: if parameter.kind.is_const_or_value() {
                            IndexIdentityArgument::TypeReference(*argument)
                        } else {
                            IndexIdentityArgument::TypeArgument(*argument)
                        },
                        lexical_owner: signature.symbol,
                        binders: &generic_binders,
                        lifetime_binders: &lifetime_binders,
                    })
                    .and_then(|identity| program.type_identity_text(identity))
                    .ok_or_else(|| {
                        Diagnostic::error(
                            "callable evidence argument lost its exact declared schema",
                        )
                    })?;
                encode_identity_text(&identity, output);
            }
        }
        let parameters = program.state_signature_parameters(signature);
        let contract_binders = formal_parameter_binders(parameters, &generic_binders);
        output.extend((parameters.len() as u64).to_le_bytes());
        for parameter in parameters {
            encode_parameter(program, parameter, &contract_binders, output);
        }
        encode_identity_text(
            &program.type_identity_request_text(TypeIdentityRequest {
                binders: &contract_binders,
                ..TypeIdentityRequest::ordinary(signature.return_type)
            }),
            output,
        );
        for service in program
            .service_reach_rows
            .services(signature.service_reach_row)
        {
            let service = program
                .service_reaches
                .definition(*service)
                .expect("normalized signature service row references a registered service");
            output.extend(service.name.as_bytes());
            output.push(0);
        }
        output.push(u8::from(signature.service_reach_is_installation_bound));
        output.push(u8::from(signature.suspends));
        output.push(u8::from(signature.blocks));
        let contracts = contract_structure::declared_contracts(
            program,
            program.state_signature_contracts(signature),
            &contract_binders,
            content_conservation,
            true,
        )?;
        output.extend(
            checked_trees::contract_declarations_commitment(&contracts, &program.type_identities)
                .as_bytes(),
        );
        let termination = contract_structure::declared_termination(
            program,
            &TerminationInterface::Published(signature.termination_guarantee.clone()),
            &contract_binders,
        );
        output.extend(checked_trees::declared_termination_commitment(&termination).as_bytes());
        Ok(())
    }

    pub fn canonical_state_signature_bytes(
        program: &dyn crate::CheckingInput,
        signature: &typed_trees::signature::StateSignature,
    ) -> Result<Vec<u8>, Diagnostic> {
        let binders = program
            .traits()
            .iter()
            .find(|definition| {
                program
                    .trait_machine_signatures(definition)
                    .iter()
                    .any(|requirement| requirement.symbol == signature.symbol)
            })
            .map(|definition| {
                type_parameter_binders(program.trait_type_parameters(definition), &[])
            })
            .unwrap_or_default();
        let content_conservation = crate::content_conservation_plans(program);
        let mut bytes = Vec::new();
        encode_state_signature(
            program,
            signature,
            &binders,
            &content_conservation,
            &mut bytes,
        )?;
        Ok(bytes)
    }

    pub fn canonical_requirement_schema_bytes(
        program: &dyn crate::CheckingInput,
        signature: &typed_trees::signature::StateSignature,
    ) -> Result<Vec<u8>, Diagnostic> {
        let binders = crate::contract_structure::signature_generic_binders(program, signature);
        let content = crate::content_conservation_plans(program);
        let mut bytes = b"omega.requirement-schema.v3\0".to_vec();
        let signature_bytes = canonical_state_signature_bytes(program, signature)?;
        bytes.extend((signature_bytes.len() as u64).to_le_bytes());
        bytes.extend(signature_bytes);
        bytes.extend((signature.lifetime_parameters.len() as u64).to_le_bytes());
        let parameters = program.state_signature_type_parameters(signature);
        bytes.extend((parameters.len() as u64).to_le_bytes());
        for parameter in parameters {
            bytes.push(match parameter.kind {
                TypeParameterKind::Type => 1,
                TypeParameterKind::Const { .. } => 2,
                TypeParameterKind::Value { .. } => 3,
                TypeParameterKind::Machine { .. } => 4,
                TypeParameterKind::Implementation => 5,
                TypeParameterKind::Proposition { .. } => 6,
            });
            encode_data_properties(parameter.bounds, &mut bytes);
            let contract =
                generic_parameter_contract_bytes(program, parameter, &binders, &content)?;
            bytes.extend((contract.len() as u64).to_le_bytes());
            bytes.extend(contract);
        }
        Ok(bytes)
    }

    fn encode_state_shape(
        program: &dyn crate::CheckingInput,
        state: &typed_trees::state::State,
        binders: &[(SymbolHandle, String)],
        content_conservation: &[crate::ContentConservationSourcePlan],
        output: &mut Vec<u8>,
    ) -> Result<(), Diagnostic> {
        let parameters = program.state_parameters(state);
        let contract_binders = formal_parameter_binders(parameters, binders);
        output.extend((parameters.len() as u64).to_le_bytes());
        for parameter in parameters {
            encode_parameter(program, parameter, &contract_binders, output);
        }
        encode_identity_text(
            &program.type_identity_request_text(TypeIdentityRequest {
                binders: &contract_binders,
                ..TypeIdentityRequest::ordinary(state.return_type)
            }),
            output,
        );
        let contracts = contract_structure::declared_contracts(
            program,
            program.state_contracts(state),
            &contract_binders,
            content_conservation,
            true,
        )?;
        output.extend(
            checked_trees::contract_declarations_commitment(&contracts, &program.type_identities)
                .as_bytes(),
        );
        Ok(())
    }

    fn encode_parameter(
        program: &dyn crate::CheckingInput,
        parameter: &typed_trees::signature::StateParameter,
        binders: &[(SymbolHandle, String)],
        output: &mut Vec<u8>,
    ) {
        output.push(u8::from(parameter.is_mutable));
        output.push(u8::from(parameter.is_const));
        output.push(match parameter.relevance {
            language_core::BindingRelevance::Relevant => 0,
            language_core::BindingRelevance::Erased => 1,
        });
        encode_identity_text(
            &program.type_identity_request_text(TypeIdentityRequest {
                binders,
                ..TypeIdentityRequest::ordinary(parameter.type_reference)
            }),
            output,
        );
    }

    pub fn specialization_selection_report_fingerprint(
        template: &str,
        type_arguments: &[String],
        const_arguments: &[String],
        machine_arguments: &[String],
        evidence_arguments: &[String],
    ) -> u64 {
        let mut hash = semantic_vocabulary::Fnv1a::new();
        for part in std::iter::once(template)
            .chain(type_arguments.iter().map(String::as_str))
            .chain(const_arguments.iter().map(String::as_str))
            .chain(machine_arguments.iter().map(String::as_str))
            .chain(evidence_arguments.iter().map(String::as_str))
        {
            hash.bytes_terminated(part.as_bytes(), 0xff);
        }
        hash.finish()
    }

    pub(crate) fn conformance_symbol_identity(
        program: &dyn crate::CheckingInput,
        symbol: SymbolHandle,
    ) -> String {
        let path = program.symbols.display_path(symbol, "::");
        let Some(package) = program.symbols.symbol_package_identity(symbol) else {
            return format!("unmanaged::{path}");
        };
        let digest = package.digest();
        let mut owner = String::with_capacity(digest.len() * 2);
        const HEX: &[u8; 16] = b"0123456789abcdef";
        for byte in digest {
            owner.push(char::from(HEX[usize::from(byte >> 4)]));
            owner.push(char::from(HEX[usize::from(byte & 0x0f)]));
        }
        format!("package:{owner}::{path}")
    }

    pub fn normalized_machine_identity(
        program: &dyn crate::CheckingInput,
        machine: &typed_trees::machine::Machine,
    ) -> Option<String> {
        let declaration = conformance_symbol_identity(program, machine.symbol);
        let overload = program.machine_callable_identity_text(machine)?;
        Some(format!("{declaration}|{overload}"))
    }
}

pub mod denotational_calls {
    use crate::frozen_program::expression_table;
    use numerics::float_projection::FloatProjectionOperation;
    use symbols::SymbolHandle;
    use typed_trees::expression::{ExpressionHandle, ExpressionNode};
    use typed_trees::machine::Machine;
    use typed_trees::signature::SignatureContractKind;

    pub(crate) mod normal_return {
        use crate::frozen_program::expression_table;
        use symbols::SymbolHandle;
        use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCallExpression};
        use typed_trees::machine::Machine;
        use typed_trees::state::State;
        use typed_trees::statement::StatementNode;

        pub fn normal_return_call_candidate<'program>(
            program: &'program dyn crate::CheckingInput,
            call: &TableCallExpression,
            operational: &flow_effects::OperationalPlan,
            service_reaches: &flow_effects::ServiceReachInferencePlan,
        ) -> Result<(&'program Machine, &'program State), &'static str> {
            let (machine, state) = plain_value_call_target(program, call)
                .ok_or("the call is not one exact concrete free value call")?;
            if !super::has_pure_effect_closure(
                program,
                machine.symbol,
                state.symbol,
                program
                    .state_parameters(state)
                    .iter()
                    .any(|parameter| parameter.is_mutable),
                operational,
                service_reaches,
            ) || !super::has_observation_free_checked_closure(
                program,
                machine.symbol,
                operational,
            ) {
                return Err("the call closure has effects, observations, or unresolved targets");
            }
            if !primitive_value_closure(program, machine.symbol, operational) {
                return Err("the call closure lacks exact value-input or builtin operator custody");
            }
            Ok((machine, state))
        }

        pub(crate) fn plain_value_call_target<'program>(
            program: &'program dyn crate::CheckingInput,
            call: &TableCallExpression,
        ) -> Option<(&'program Machine, &'program State)> {
            if !call.target_symbol.is_valid()
                || call.receiver.is_valid()
                || !call.carries_only_positional_arguments()
                || !call.selects_only_nominal_route()
            {
                return None;
            }
            let mut targets = program.machine_states_named(call.target_symbol);
            let (machine, state) = targets.next()?;
            if targets.next().is_some()
                || program.machine_states(machine).first()?.symbol != state.symbol
                || !primitive_value_machine(program, machine)
                || program.state_parameters(state).len()
                    != expression_table(program)
                        .expression_handles(call.arguments)
                        .len()
            {
                return None;
            }
            Some((machine, state))
        }

        fn primitive_value_machine(program: &dyn crate::CheckingInput, machine: &Machine) -> bool {
            machine.symbol.is_valid()
                && machine.supply_mode == language_semantics::MachineSupplyMode::CheckedBody
                && machine.body_is_present
                && machine.attached_data.is_none()
                && machine.lifetime_parameters.is_empty()
                && program.machine_type_parameters(machine).is_empty()
                && program.machine_states(machine).iter().all(|state| {
                    state.symbol.is_valid()
                        && program
                            .primitive_type_reference(state.return_type)
                            .is_some()
                        && program.state_parameters(state).iter().all(|parameter| {
                            parameter.symbol.is_valid()
                                && !parameter.is_mutable
                                && program
                                    .primitive_type_reference(parameter.type_reference)
                                    .is_some()
                        })
                })
        }

        fn primitive_value_closure(
            program: &dyn crate::CheckingInput,
            machine_symbol: SymbolHandle,
            operational: &flow_effects::OperationalPlan,
        ) -> bool {
            let mut pending = vec![machine_symbol];
            let mut visited = Vec::new();
            while let Some(current) = pending.pop() {
                if visited.contains(&current) {
                    continue;
                }
                visited.push(current);
                let mut machines = program
                    .machines()
                    .iter()
                    .filter(|machine| machine.symbol == current);
                let Some(machine) = machines.next() else {
                    return false;
                };
                if machines.next().is_some() || !primitive_value_machine(program, machine) {
                    return false;
                }
                for state in program.machine_states(machine) {
                    let mut available = program
                        .state_parameters(state)
                        .iter()
                        .map(|parameter| parameter.symbol)
                        .collect::<Vec<_>>();
                    for statement in program.statement_table.statements(state.statement_nodes) {
                        let expression_is_closed = |expression| {
                            closed_expression(program, machine, state, expression, &available, 0)
                        };
                        let allowed = match statement {
                            StatementNode::Expression(_) | StatementNode::Transition(_) => program
                                .tables
                                .expression_roots(statement)
                                .iter()
                                .all(|(_, expression)| expression_is_closed(*expression)),
                            StatementNode::LocalData(local) => {
                                let allowed = local.symbol.is_valid()
                                    && !local.is_mutable
                                    && expression_is_closed(local.initial_value);
                                available.push(local.symbol);
                                allowed
                            }
                            StatementNode::Call(_)
                            | StatementNode::Assignment(_)
                            | StatementNode::Assembly(_)
                            | StatementNode::AssemblyFact(_) => false,
                        };
                        if !allowed {
                            return false;
                        }
                    }
                }
                let mut summaries = operational
                    .machines()
                    .iter()
                    .filter(|summary| summary.symbol == current);
                let Some(summary) = summaries.next() else {
                    return false;
                };
                if summaries.next().is_some() {
                    return false;
                }
                for state in operational.states.span_or_empty(summary.states) {
                    for call in operational.calls.span_or_empty(state.calls) {
                        if !call.target_machine_symbol.is_valid() {
                            return false;
                        }
                        pending.push(call.target_machine_symbol);
                    }
                }
            }
            true
        }

        fn closed_expression(
            program: &dyn crate::CheckingInput,
            machine: &Machine,
            state: &State,
            expression: ExpressionHandle,
            available: &[SymbolHandle],
            depth: usize,
        ) -> bool {
            if !expression.is_valid() || depth >= 128 {
                return false;
            }
            if program.static_type_predicate_operands(expression).is_some() {
                return program.static_type_predicate_value(expression).is_some();
            }
            let child = |expression| {
                closed_expression(program, machine, state, expression, available, depth + 1)
            };
            match expression_table(program).expression(expression) {
                ExpressionNode::Integer(_)
                | ExpressionNode::Boolean(_)
                | ExpressionNode::Float(_) => true,
                ExpressionNode::Name(path) => {
                    path.symbol.is_valid()
                        && path.head_symbol == path.symbol
                        && available.contains(&path.symbol)
                }
                ExpressionNode::Binary(binary) => {
                    let builtin = crate::has_builtin_bound_expression_meaning(
                        program,
                        machine,
                        Some(state),
                        expression,
                    );
                    let operand_types = [binary.left, binary.right].map(|operand| {
                        crate::expression_result_type_reference(program, machine, state, operand)
                    });
                    let sealed_comparison = crate::proof_contracts::bound_expression_meaning::
                sealed_float_comparison_declaration_for_operands(
                    program, machine.symbol, expression, binary.operator, &operand_types,
                ).is_some();
                    (builtin || sealed_comparison) && child(binary.left) && child(binary.right)
                }
                ExpressionNode::Call(call) => {
                    plain_value_call_target(program, call).is_some()
                        && expression_table(program)
                            .expression_handles(call.arguments)
                            .iter()
                            .all(|argument| child(*argument))
                }
                ExpressionNode::Borrow(borrow) => {
                    borrow.access == language_semantics::ReferenceAccess::Shared
                        && child(borrow.target)
                }
                ExpressionNode::Cast(cast) => {
                    let source = crate::expression_result_type_reference(
                        program, machine, state, cast.value,
                    );
                    (cast.form.is_recast()
                || source.is_some_and(|source| {
                    crate::proof_contracts::contract_entailment::denotation_preserving_conversion(
                        program, cast, source,
                    )
                }))
                && child(cast.value)
                }
                _ => false,
            }
        }
    }
    pub(crate) mod requirement_satisfiers {
        use std::sync::Arc;
        use symbols::SymbolHandle;
        use typed_trees::TypedTrees;

        pub(crate) struct RequirementSatisfierIndex {
            rows: Vec<(SymbolHandle, usize)>,
        }

        fn symbol_key(symbol: SymbolHandle) -> (u32, u32) {
            (symbol.arena_index(), symbol.generation())
        }

        impl RequirementSatisfierIndex {
            fn prepare(program: &TypedTrees) -> Self {
                let mut rows = Vec::new();
                let mut requirements = Vec::new();
                for (position, machine) in program.machines().iter().enumerate() {
                    requirements.clear();
                    for conformance in program.machine_trait_conformances(machine) {
                        if let Some(declaration) =
                            typed_trees::machine::resolve_satisfied_declaration(
                                program,
                                machine,
                                conformance,
                            )
                        {
                            requirements.push(declaration.symbol());
                        }
                    }
                    requirements.sort_unstable_by_key(|symbol| symbol_key(*symbol));
                    requirements.dedup();
                    rows.extend(
                        requirements
                            .iter()
                            .map(|requirement| (*requirement, position)),
                    );
                }
                rows.sort_unstable_by_key(|(requirement, position)| {
                    (symbol_key(*requirement), *position)
                });
                Self { rows }
            }

            fn satisfiers(
                &self,
                program: &TypedTrees,
                requirement: SymbolHandle,
            ) -> Vec<SymbolHandle> {
                let key = symbol_key(requirement);
                let start = self
                    .rows
                    .partition_point(|(symbol, _)| symbol_key(*symbol) < key);
                let end = self
                    .rows
                    .partition_point(|(symbol, _)| symbol_key(*symbol) <= key);
                self.rows[start..end]
                    .iter()
                    .map(|(_, position)| program.machines()[*position].symbol)
                    .collect()
            }
        }

        pub fn requirement_satisfiers(
            program: &dyn crate::CheckingInput,
            requirement: SymbolHandle,
        ) -> Vec<SymbolHandle> {
            if let Some(memos) = crate::frozen_program::frozen_program_memos(program) {
                return memos
                    .requirement_satisfiers
                    .get_or_init(|| {
                        Arc::new(RequirementSatisfierIndex::prepare(program.typed_program()))
                    })
                    .satisfiers(program.typed_program(), requirement);
            }
            direct_satisfiers(program.typed_program(), requirement)
        }

        fn direct_satisfiers(program: &TypedTrees, requirement: SymbolHandle) -> Vec<SymbolHandle> {
            program
                .machines()
                .iter()
                .filter(|machine| {
                    program
                        .machine_trait_conformances(machine)
                        .iter()
                        .any(|conformance| {
                            typed_trees::machine::resolve_satisfied_declaration(
                                program,
                                machine,
                                conformance,
                            )
                            .is_some_and(|satisfied| satisfied.symbol() == requirement)
                        })
                })
                .map(|machine| machine.symbol)
                .collect()
        }
    }
    pub(crate) use requirement_satisfiers::RequirementSatisfierIndex;
    pub use requirement_satisfiers::requirement_satisfiers;
    pub(crate) mod specification {
        use crate::frozen_program::expression_table;
        use crate::proof_contracts::contract_entailment::{
            SelectedCallContract, selected_call_contract,
        };
        use diagnostics::Diagnostic;
        use symbols::SymbolHandle;
        use typed_trees::domain::ProofFact;
        use typed_trees::expression::{ExpressionHandle, ExpressionNode};
        use typed_trees::proposition::{PropositionBody, PropositionFormula};

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct SpecificationCallRequest {
            pub call_expression: ExpressionHandle,
            pub target_machine: SymbolHandle,
            pub target_state: SymbolHandle,
        }

        pub(crate) fn validate_specification_calls(
            program: &dyn crate::CheckingInput,
            operational: &flow_effects::OperationalPlan,
            service_reaches: &flow_effects::ServiceReachInferencePlan,
            diagnostics: &mut Vec<Diagnostic>,
        ) -> Vec<SpecificationCallRequest> {
            let mut pending = Vec::new();
            for (owner, _, fact) in typed_trees::references::program_scoped_proof_facts(program) {
                match fact {
                    ProofFact::Expression(expression) => pending.push((owner, *expression)),
                    ProofFact::Membership(membership) => pending.push((owner, membership.value)),
                    ProofFact::Proposition(application) => pending.extend(
                        expression_table(program)
                            .expression_handles(application.arguments)
                            .iter()
                            .map(|expression| (owner, *expression)),
                    ),
                }
            }
            pending.extend(crate::mathematical_applications::scoped_expression_nodes(
                program,
            ));
            for machine in program.machines() {
                if let Some(custody) = program.ranking_expression_custody_for(machine.symbol) {
                    pending.extend(
                        custody
                            .subjects
                            .iter()
                            .map(|value| (machine.symbol, *value)),
                    );
                    pending.extend(
                        custody
                            .view_arguments
                            .iter()
                            .map(|value| (machine.symbol, *value)),
                    );
                    pending.extend(custody.rank_range.map(|value| (machine.symbol, value)));
                }
            }
            for declaration in program.measures() {
                pending.extend(
                    expression_table(program)
                        .expression_handles(declaration.body)
                        .iter()
                        .map(|expression| (declaration.symbol, *expression)),
                );
            }
            for proposition in program.propositions() {
                if let PropositionBody::Transparent {
                    proposition: formula,
                } = &proposition.body
                {
                    match formula {
                        PropositionFormula::BooleanExpression(expression) => {
                            pending.push((proposition.symbol, *expression))
                        }
                        PropositionFormula::Application(application) => pending.extend(
                            expression_table(program)
                                .expression_handles(application.arguments)
                                .iter()
                                .map(|expression| (proposition.symbol, *expression)),
                        ),
                    }
                }
            }
            let content =
                crate::value_custody::content_conservation::content_conservation_inputs(program);
            let mut requests = Vec::new();
            let mut visited = std::collections::HashSet::new();
            while let Some((caller, expression)) = pending.pop() {
                if !expression_table(program).expression_is_valid(expression)
                    || !visited.insert((caller, expression))
                {
                    continue;
                }
                let node = expression_table(program).expression(expression);
                crate::queries::expression_traversal::children(program, node, |child| {
                    pending.push((caller, child))
                });
                let ExpressionNode::Call(call) = node else {
                    continue;
                };
                if content.forms_projection_call(caller, expression) {
                    continue;
                }
                if program.is_deferred_contract_invocation(expression) {
                    continue;
                }
                if crate::mathematical_applications::is_mathematical_call(program, call) {
                    // Checked mathematical signature formation owns the dependent
                    // application. It has no operational closure to classify.
                    // The producer has already checked this source application at
                    // its instantiated generic telescope. Repeating an uninstantiated
                    // carrier check here would lose that exact declaration custody.
                    continue;
                }
                let (machine, state) = match selected_call_contract(program, caller, call) {
                    Some(SelectedCallContract::Concrete(machine, state)) => (machine, state),
                    Some(SelectedCallContract::Operator(operator)) => {
                        if super::selected_operator_has_closed_value_denotation(program, operator) {
                            continue;
                        }
                        let Some(body) = super::selected_operator_authored_body(program, operator)
                        else {
                            diagnostics.push(Diagnostic::error(format!(
                            "specification call `{}` has no observation-free, reach-free, crash-free value denotation",
                            program.symbols.name(operator.symbol),
                        )));
                            continue;
                        };
                        body
                    }
                    Some(SelectedCallContract::Signature(signature))
                    | Some(SelectedCallContract::DynamicSlot { signature, .. }) => {
                        if let Some(body) =
                            super::selected_requirement_authored_body(program, caller, call)
                        {
                            body
                        } else {
                            if !matches!(
                                &signature.termination_guarantee,
                                language_semantics::TerminationGuarantee::Terminates { premises }
                                    if premises.is_empty()
                            ) {
                                diagnostics.push(Diagnostic::error(format!(
                            "specification call `{}` is not denotational: the selected machine is not unconditionally terminating",
                            signature.name,
                        )));
                            }
                            if !signature_has_closed_value_effects(program, signature) {
                                diagnostics.push(Diagnostic::error(format!(
                            "specification call `{}` has no observation-free, reach-free, crash-free value denotation",
                            signature.name,
                        )));
                            }
                            if !super::signature_value_content_is_observable(program, signature) {
                                diagnostics.push(Diagnostic::error(format!(
                                    "specification call `{}` carries forbidden denotational content",
                                    signature.name,
                                )));
                            }
                            continue;
                        }
                    }
                    None => continue,
                };
                if let Err(reason) = super::checked_value_call_candidate(
                    program,
                    machine,
                    state,
                    operational,
                    service_reaches,
                ) {
                    diagnostics.push(Diagnostic::error(format!(
                    "specification call `{}` has no observation-free, reach-free, crash-free value denotation: {reason}",
                    machine.name,
                )));
                    continue;
                }
                requests.push(SpecificationCallRequest {
                    call_expression: expression,
                    target_machine: machine.symbol,
                    target_state: state.symbol,
                });
            }
            requests.sort_by_key(|request| {
                (
                    request.call_expression.arena_index(),
                    request.call_expression.generation(),
                )
            });
            requests.dedup();
            requests
        }

        pub(crate) fn signature_has_closed_value_effects(
            program: &dyn crate::CheckingInput,
            signature: &typed_trees::signature::StateSignature,
        ) -> bool {
            !signature.suspends
                && !signature.blocks
                && !signature.service_reach_is_installation_bound
                && program
                    .service_reach_rows
                    .services(signature.service_reach_row)
                    .is_empty()
                && program.state_signature_invokes(signature).is_empty()
                && program
                    .state_signature_parameters(signature)
                    .iter()
                    .all(|parameter| {
                        !parameter.is_mutable
                            && crate::reference_shell(program, parameter.type_reference).is_none_or(
                                |reference| {
                                    reference.access == language_semantics::ReferenceAccess::Shared
                                },
                            )
                    })
                && program
                    .state_signature_contracts(signature)
                    .iter()
                    .all(|contract| !contract.is_crashes())
        }

        pub fn admit_specification_calls(
            program: &dyn crate::CheckingInput,
            requests: &[SpecificationCallRequest],
            termination: &impl crate::CheckedTerminationOracle,
        ) -> Result<(), Vec<Diagnostic>> {
            let mut diagnostics = Vec::new();
            for request in requests {
                let summary = termination.checked_termination(request.target_machine);
                if !matches!(
                    &summary,
                    Some(language_semantics::TerminationGuarantee::Terminates { premises })
                        if premises.is_empty()
                ) {
                    if crate::trace_flags::struct_trace() {
                        eprintln!(
                            "DENOTATION termination request={request:?} machine={} summary={summary:?} expression={:?}",
                            program.symbols.name(request.target_machine),
                            expression_table(program).expression(request.call_expression),
                        );
                    }
                    diagnostics.push(Diagnostic::error(format!(
                "specification call `{}` is not denotational: the selected machine is not unconditionally terminating",
                program.symbols.name(request.target_machine),
            )));
                }
            }
            crate::program_validation::finish_strict_diagnostics(diagnostics)
        }
    }
    pub use normal_return::normal_return_call_candidate;
    pub(crate) use normal_return::plain_value_call_target;
    pub(crate) use specification::validate_specification_calls;
    pub use specification::{SpecificationCallRequest, admit_specification_calls};

    /// A static callable or implementation requirement's exact public contract
    /// can close a value-only leaf.
    /// This consumes the declared promise; it never inspects a selected private
    /// implementation or supplies an unresolved caller with stronger guarantees.
    pub(crate) fn abstract_value_call_signature(
        program: &dyn crate::CheckingInput,
        target: SymbolHandle,
    ) -> Option<&typed_trees::signature::StateSignature> {
        let signature = if let Some(contract) = program.retained_static_machine_contract(target) {
            contract.signature()
        } else {
            if !target.is_valid() {
                return None;
            }
            let binder = program.symbols.get(target).parent;
            if !binder.is_valid()
                || program.symbols.get(binder).kind != symbols::SymbolKind::ConformanceParameter
            {
                return None;
            }
            // Keep the exact invoked child as the value identity. Its declaring
            // evidence bound supplies the applied public signature, including
            // explicit refinements; a selected private body supplies no promise.
            crate::selected_requirement_signature(program, target)?.1
        };
        if !signature_has_closed_value_contract(program, signature) {
            return None;
        }
        Some(signature)
    }

    pub(crate) fn signature_has_closed_value_contract(
        program: &dyn crate::CheckingInput,
        signature: &typed_trees::signature::StateSignature,
    ) -> bool {
        matches!(
            &signature.termination_guarantee,
            language_semantics::TerminationGuarantee::Terminates { premises }
                if premises.is_empty()
        ) && specification::signature_has_closed_value_effects(program, signature)
            && signature_value_content_is_observable(program, signature)
    }

    pub(crate) fn signature_value_content_is_observable(
        program: &dyn crate::CheckingInput,
        signature: &typed_trees::signature::StateSignature,
    ) -> bool {
        let forbidden = |reference| {
            crate::proof_contracts::carrier_contents::type_has_forbidden_value_observation_content(
                program, reference,
            )
        };
        !program
        .state_signature_parameters(signature)
        .iter()
        .any(|parameter| {
            crate::proof_contracts::carrier_contents::type_has_forbidden_argument_observation_content(
                program,
                parameter.type_reference,
            )
        })
        && (!signature.return_type.is_valid() || !forbidden(signature.return_type))
    }

    pub fn checked_value_call_candidate(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state: &typed_trees::state::State,
        operational: &flow_effects::OperationalPlan,
        service_reaches: &flow_effects::ServiceReachInferencePlan,
    ) -> Result<(), &'static str> {
        if let Some(memos) = crate::frozen_program::frozen_program_memos(program) {
            let key = (machine.symbol, state.symbol);
            if let Ok(candidates) = memos.checked_value_call_candidates.lock()
                && let Some(result) = candidates.get(&key).copied()
            {
                return result;
            }
            let result = unchecked_value_call_candidate(
                program,
                machine,
                state,
                operational,
                service_reaches,
            );
            if let Ok(mut candidates) = memos.checked_value_call_candidates.lock() {
                candidates.insert(key, result);
            }
            return result;
        }
        unchecked_value_call_candidate(program, machine, state, operational, service_reaches)
    }

    fn unchecked_value_call_candidate(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state: &typed_trees::state::State,
        operational: &flow_effects::OperationalPlan,
        service_reaches: &flow_effects::ServiceReachInferencePlan,
    ) -> Result<(), &'static str> {
        let forbidden = |reference| {
            crate::proof_contracts::carrier_contents::type_has_forbidden_value_observation_content(
                program, reference,
            )
        };
        if (state.return_type.is_valid() && forbidden(state.return_type))
        || program
            .state_parameters(state)
            .iter()
            .any(|parameter| {
                crate::proof_contracts::carrier_contents::type_has_forbidden_argument_observation_content(
                    program,
                    parameter.type_reference,
                )
            })
    {
        return Err("the application carries forbidden denotational content");
    }
        if !has_pure_effect_closure(
            program,
            machine.symbol,
            state.symbol,
            program
                .state_parameters(state)
                .iter()
                .any(|parameter| parameter.is_mutable),
            operational,
            service_reaches,
        ) {
            return Err("the selected closure has effects, reaches, or unresolved call targets");
        }
        if !has_observation_free_checked_closure(program, machine.symbol, operational) {
            return Err("the selected closure contains runtime observations or mutable storage");
        }
        if !has_no_crash_routes(program, machine.symbol, operational) {
            return Err("the selected closure publishes a crash route");
        }
        Ok(())
    }

    pub fn expression_values_have_call_identity(
        program: &dyn crate::CheckingInput,
        expression: ExpressionHandle,
        operational: &flow_effects::OperationalPlan,
        service_reaches: &flow_effects::ServiceReachInferencePlan,
        admitted: &dyn Fn(ExpressionHandle) -> bool,
    ) -> bool {
        let mut pending = vec![expression];
        let mut seen = Vec::new();
        while let Some(expression) = pending.pop() {
            if !expression_table(program).expression_is_valid(expression) {
                return false;
            }
            if seen.contains(&expression) {
                continue;
            }
            seen.push(expression);
            let node = expression_table(program).expression(expression);
            if let ExpressionNode::Call(call) = node
            && !admitted(expression)
            && normal_return_call_candidate(program, call, operational, service_reaches).is_err()
            && !crate::machine_calls::fact_call_projections::has_checked_denotational_call_identity(
                program,
                call,
                operational,
                service_reaches,
            )
        {
            return false;
        }
            crate::queries::expression_traversal::children(program, node, |child| {
                if child.is_valid() {
                    pending.push(child);
                }
            });
        }
        true
    }

    fn each_reachable_checked_machine(
        program: &dyn crate::CheckingInput,
        machine_symbol: symbols::SymbolHandle,
        operational: &flow_effects::OperationalPlan,
        admits: impl Fn(&dyn crate::CheckingInput, &Machine) -> bool,
    ) -> bool {
        let mut pending = vec![machine_symbol];
        let mut visited = Vec::new();
        while let Some(current) = pending.pop() {
            if visited.contains(&current) {
                continue;
            }
            visited.push(current);
            let Some(machine) = program.machine(current) else {
                return false;
            };
            if !admits(program, machine)
                || !push_checked_machine_call_targets(program, operational, current, &mut pending)
            {
                return false;
            }
        }
        true
    }

    pub(crate) fn has_observation_free_checked_closure(
        program: &dyn crate::CheckingInput,
        machine_symbol: symbols::SymbolHandle,
        operational: &flow_effects::OperationalPlan,
    ) -> bool {
        has_observation_free_closure(program, machine_symbol, operational, &[])
    }

    fn has_observation_free_closure(
        program: &dyn crate::CheckingInput,
        machine_symbol: symbols::SymbolHandle,
        operational: &flow_effects::OperationalPlan,
        requirements_in_flight: &[symbols::SymbolHandle],
    ) -> bool {
        if requirements_in_flight.contains(&machine_symbol) {
            return false;
        }
        crate::note_dependency_body_read(
            program,
            machine_symbol,
            crate::DependencyBodyRead::ObservationFreedom,
        );
        let mut path = requirements_in_flight.to_vec();
        path.push(machine_symbol);
        let frames = crate::CallFrameResolver::new(program);
        each_reachable_checked_machine(program, machine_symbol, operational, |program, machine| {
            if !machine.supplies_checked_body() || !machine.body_is_present {
                let satisfiers = requirement_satisfiers(program, machine.symbol);
                if satisfiers.is_empty() {
                    if crate::trace_flags::struct_trace() {
                        eprintln!(
                            "DENOTATION observation closure machine={:?} name={} is a requirement with no satisfier",
                            machine.symbol, machine.name,
                        );
                    }
                    return false;
                }
                return satisfiers.iter().all(|satisfier| {
                    has_observation_free_closure(program, *satisfier, operational, &path)
                });
            }
            if !program.machine_owned_data(machine).is_empty()
                || !program
                    .service_reach_rows
                    .services(machine.service_reach_row)
                    .is_empty()
            {
                if crate::trace_flags::struct_trace() {
                    eprintln!(
                        "DENOTATION observation closure machine={:?} name={} supply={:?} body={} owned={} reach={}",
                        machine.symbol,
                        machine.name,
                        machine.supply_mode,
                        machine.body_is_present,
                        program.machine_owned_data(machine).len(),
                        program
                            .service_reach_rows
                            .services(machine.service_reach_row)
                            .len(),
                    );
                }
                return false;
            }
            for state in program.machine_states(machine) {
                if program
                    .state_parameters(state)
                    .iter()
                    .any(|parameter| parameter.is_mutable)
                {
                    if crate::trace_flags::struct_trace() {
                        eprintln!(
                            "DENOTATION observation closure machine={:?} name={} state={:?} has mutable parameters",
                            machine.symbol, machine.name, state.symbol,
                        );
                    }
                    return false;
                }
                let mut owned_values = program
                    .state_parameters(state)
                    .iter()
                    .filter(|parameter| {
                        frames.as_ref().is_some_and(|frames| {
                            frames.proof_value_is_caller_isolated(parameter.type_reference)
                        })
                    })
                    .map(|parameter| parameter.symbol)
                    .collect::<Vec<_>>();
                for statement in program.statement_table.statements(state.statement_nodes) {
                    let internal_borrow = |expression| {
                        let Some(frames) = frames.as_ref() else {
                            return false;
                        };
                        frames
                            .reference_expression_origins_before_statement(
                                machine, statement, expression,
                            )
                            .is_some_and(|origins| {
                                !origins.is_empty()
                                    && origins.iter().all(|(root, _)| owned_values.contains(root))
                            })
                    };
                    use typed_trees::statement::StatementNode;
                    let allowed = match statement {
                        StatementNode::Assembly(_)
                        | StatementNode::AssemblyFact(_)
                        | StatementNode::Assignment(_) => false,
                        StatementNode::LocalData(local) => !local.is_mutable,
                        StatementNode::Call(call) => !call.receiver_symbol.is_valid(),
                        StatementNode::Expression(_) | StatementNode::Transition(_) => true,
                    } && program.tables.expression_roots(statement).iter().all(
                        |(_, expression)| {
                            expression_is_fact_observation_free(
                                program,
                                *expression,
                                &internal_borrow,
                            )
                        },
                    );
                    if !allowed {
                        if crate::trace_flags::struct_trace() {
                            eprintln!(
                                "DENOTATION observation closure machine={:?} name={} state={:?} statement={statement:?}",
                                machine.symbol, machine.name, state.symbol,
                            );
                        }
                        return false;
                    }
                    if let StatementNode::LocalData(local) = statement
                        && frames.as_ref().is_some_and(|frames| {
                            frames.proof_value_is_caller_isolated(local.type_reference)
                        })
                    {
                        owned_values.push(local.symbol);
                    }
                }
            }
            true
        })
    }

    fn expression_is_fact_observation_free(
        program: &dyn crate::CheckingInput,
        expression: ExpressionHandle,
        internal_borrow: &impl Fn(ExpressionHandle) -> bool,
    ) -> bool {
        use typed_trees::SubtreeVisit;
        !program.subtree_any(
            expression,
            typed_trees::SubtreeEdges::VALUES,
            |handle, node| {
                let visit = match node {
                    ExpressionNode::SavedSubject(_) | ExpressionNode::Atomic(_) => {
                        SubtreeVisit::Found
                    }
                    ExpressionNode::Borrow(borrow)
                        if borrow.access != language_semantics::ReferenceAccess::Shared
                            || !internal_borrow(handle) =>
                    {
                        SubtreeVisit::Found
                    }
                    ExpressionNode::Call(call)
                        if call.receiver.is_valid()
                            && !program.call_has_no_runtime_receiver_for_target(call) =>
                    {
                        SubtreeVisit::Found
                    }
                    ExpressionNode::Call(call) => SubtreeVisit::InsteadEach(
                        expression_table(program)
                            .expression_handles(call.arguments)
                            .to_vec(),
                    ),
                    _ => SubtreeVisit::Descend,
                };
                if visit == SubtreeVisit::Found && crate::trace_flags::struct_trace() {
                    eprintln!("DENOTATION observation expression={handle:?} node={node:?}",);
                }
                visit
            },
        )
    }

    pub(crate) fn unconditionally_terminates(
        program: &dyn crate::CheckingInput,
        machine_symbol: SymbolHandle,
    ) -> bool {
        let Some(machine) = program.machine(machine_symbol) else {
            return false;
        };
        matches!(
            &machine.termination_plan.checked_summary,
            language_semantics::TerminationGuarantee::Terminates { premises }
                if premises.is_empty()
        )
    }

    pub(crate) fn has_pure_effect_closure(
        program: &dyn crate::CheckingInput,
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        has_mutable_parameters: bool,
        operational: &flow_effects::OperationalPlan,
        service_reaches: &flow_effects::ServiceReachInferencePlan,
    ) -> bool {
        if has_mutable_parameters {
            return false;
        }

        let machine_summaries = operational
            .machines()
            .iter()
            .filter(|summary| summary.symbol == machine_symbol)
            .collect::<Vec<_>>();
        let [machine_summary] = machine_summaries.as_slice() else {
            return false;
        };
        if machine_summary.transitive_may_suspend || machine_summary.transitive_may_block {
            return false;
        }
        if operational
            .states
            .span_or_empty(machine_summary.states)
            .iter()
            .filter(|summary| summary.symbol == state_symbol)
            .count()
            != 1
        {
            return false;
        }

        let reach_summaries = service_reaches
            .machines()
            .iter()
            .filter(|summary| summary.machine == machine_symbol)
            .collect::<Vec<_>>();
        let [reach_summary] = reach_summaries.as_slice() else {
            return false;
        };
        if !service_reaches
            .services(reach_summary.inferred_transitive)
            .is_empty()
        {
            return false;
        }

        let mut pending = vec![machine_symbol];
        let mut visited = Vec::new();
        while let Some(current) = pending.pop() {
            if visited.contains(&current) {
                continue;
            }
            visited.push(current);
            if !push_checked_machine_call_targets(program, operational, current, &mut pending) {
                return false;
            }
        }
        true
    }

    fn push_checked_machine_call_targets(
        program: &dyn crate::CheckingInput,
        operational: &flow_effects::OperationalPlan,
        current: symbols::SymbolHandle,
        pending: &mut Vec<symbols::SymbolHandle>,
    ) -> bool {
        let Some(machine) = program.machine(current) else {
            return false;
        };
        if machine.target_family_interface {
            let Some(members) = complete_family_members(program, machine) else {
                return false;
            };
            pending.extend(members.iter().map(|member| member.machine));
        }
        let summaries = operational
            .machines()
            .iter()
            .filter(|summary| summary.symbol == current)
            .collect::<Vec<_>>();
        let [summary] = summaries.as_slice() else {
            return false;
        };
        for state in operational.states.span_or_empty(summary.states) {
            for call in operational.calls.span_or_empty(state.calls) {
                if call.target_machine_symbol.is_valid() {
                    let Some(machine) = program.machine(call.target_machine_symbol) else {
                        return false;
                    };
                    if !program
                        .machine_states(machine)
                        .iter()
                        .any(|state| state.symbol == call.target_state_symbol)
                    {
                        return false;
                    }
                    pending.push(machine.symbol);
                    continue;
                }
                let parameter = if call.static_machine_parameter.is_valid() {
                    call.static_machine_parameter
                } else {
                    call.target_state_symbol
                };
                if program
                    .machine_parameter_signature_in(machine, parameter)
                    .is_some()
                {
                    if !call.static_machine_bindings.is_empty()
                        || abstract_value_call_signature(program, parameter).is_none()
                    {
                        return false;
                    }
                    continue;
                }
                if call.static_machine_parameter.is_valid()
                    || !call.static_machine_bindings.is_empty()
                    || call.direct_may_suspend
                    || call.direct_may_block
                    || call.transitive_may_suspend
                    || call.transitive_may_block
                {
                    return false;
                }
                let Some(operator) = typed_trees::operator::declaration_by_symbol(
                    program,
                    call.target_operator_symbol,
                ) else {
                    return false;
                };
                if (call.target_state_symbol.is_valid()
                    && call.target_state_symbol != operator.symbol)
                    || !selected_operator_has_closed_value_denotation(program, operator)
                {
                    return false;
                }
            }
        }
        true
    }

    pub(crate) fn has_no_crash_routes(
        program: &dyn crate::CheckingInput,
        machine_symbol: SymbolHandle,
        operational: &flow_effects::OperationalPlan,
    ) -> bool {
        each_reachable_checked_machine(program, machine_symbol, operational, |program, machine| {
            if program.machine_declares_crash(machine) {
                return false;
            }
            if !machine.target_family_interface {
                return true;
            }
            complete_family_members(program, machine).is_some_and(|members| {
                members.iter().all(|member| {
                    program
                        .machine(member.machine)
                        .is_some_and(|machine| program.machine_publishes_crash_ceiling(machine))
                })
            })
        })
    }

    fn complete_family_members<'program>(
        program: &'program dyn crate::CheckingInput,
        interface: &Machine,
    ) -> Option<&'program [typed_trees::machine::TargetMachineFamilyMember]> {
        let members = crate::machine_calls::calls::target_family_members(program, interface)?;
        for member in members {
            let machine = program.machine(member.machine)?;
            if !machine.supplies_checked_body()
                || !machine.body_is_present
                || machine.body_status != typed_trees::machine::MachineBodyStatus::Complete
                || machine.structural_type_equations_pending
            {
                return None;
            }
        }
        Some(members)
    }

    pub(crate) fn selected_operator_authored_body<'program>(
        program: &'program dyn crate::CheckingInput,
        operator: &typed_trees::operator::OperatorDefinition,
    ) -> Option<(&'program Machine, &'program typed_trees::state::State)> {
        if operator.is_boundary
            || program
                .operator_contracts(operator)
                .iter()
                .any(|contract| matches!(contract.kind, SignatureContractKind::Crashes { .. }))
        {
            return None;
        }
        let machine = program.machine(operator.symbol)?;
        if !machine.body_is_present {
            return None;
        }
        let entry = program.machine_states(machine).first()?;
        (operator.return_type == entry.return_type
            && program.operator_parameters(operator) == program.state_parameters(entry))
        .then_some((machine, entry))
    }

    pub(crate) fn selected_requirement_authored_body<'program>(
        program: &'program dyn crate::CheckingInput,
        caller: SymbolHandle,
        call: &typed_trees::expression::TableCallExpression,
    ) -> Option<(&'program Machine, &'program typed_trees::state::State)> {
        let dispatch = call.static_requirement_dispatch.as_ref()?;
        let caller = program
            .machine_holding_state(caller)
            .map_or(caller, |machine| machine.symbol);
        let target = crate::CallSite::Expression {
            expression: Default::default(),
            call,
        }
        .target_symbol(program);
        if !crate::closed_requirement_dispatch_is_valid(program, caller, target, dispatch) {
            return None;
        }
        let (machine, entry) = crate::find_state(program, target)
            .or_else(|| crate::find_machine_head(program, target))?;
        if machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
            || !machine.body_is_present
            || machine.body_status != typed_trees::machine::MachineBodyStatus::Complete
            || program.machine_states(machine).first()?.symbol != entry.symbol
        {
            return None;
        }
        Some((machine, entry))
    }

    pub fn selected_operator_has_closed_value_denotation(
        program: &dyn crate::CheckingInput,
        operator: &typed_trees::operator::OperatorDefinition,
    ) -> bool {
        use crate::proof_contracts::float_projection_bindings;
        if program
            .operator_contracts(operator)
            .iter()
            .any(|contract| matches!(contract.kind, SignatureContractKind::Crashes { .. }))
            || program.machine(operator.symbol).is_some_and(|machine| {
                machine.suspends
                    || machine.blocks
                    || program.machine_declares_crash(machine)
                    || !program
                        .service_reach_rows
                        .services(machine.service_reach_row)
                        .is_empty()
                    || !program.machine_owned_data(machine).is_empty()
            })
        {
            return false;
        }
        if program
            .operator_parameters(operator)
            .iter()
            .any(|parameter| {
                parameter.is_mutable
                    || crate::reference_shell(program, parameter.type_reference).is_some()
            })
        {
            return false;
        }
        if float_projection_bindings::semantic_operations::exact_toolchain_float_semantic_contract(
            program, operator,
        )
        .is_some()
        {
            return true;
        }
        let path = program.operator_path_members(operator.name);
        if let [namespace, name] = path
            && let Some(projection) =
                FloatProjectionOperation::from_source_identity(namespace.as_str(), name.as_str())
            && float_projection_bindings::exact_toolchain_float_projection_primitive(
                program, operator, projection,
            )
            .is_some()
        {
            return true;
        }
        float_projection_bindings::semantic_operations::is_sealed_toolchain_float_operator(
            program, operator,
        ) && (float_projection_bindings::float_result_semantic_contract(program, operator)
            .is_some()
            || float_projection_bindings::float_scalar_semantic_contract(program, operator)
                .is_some())
    }
}

pub mod effects {
    use diagnostics::Diagnostic;
    use language_semantics::MachineSupplyMode;
    use language_semantics::ServiceReachId;

    pub fn validate_behavior_plan(
        program: &dyn crate::CheckingInput,
        operational: &flow_effects::OperationalPlan,
    ) -> Result<(), Vec<Diagnostic>> {
        let mut diagnostics = Vec::new();
        let service_reaches = crate::infer_service_reaches(program, operational);

        validate_pure_discards(program, operational, &service_reaches, &mut diagnostics);
        validate_asm_intrinsic_declarations(program, &service_reaches, &mut diagnostics);
        validate_service_reach_ceilings(program, &service_reaches, &mut diagnostics);

        for machine_summary in operational.machines() {
            let Some(machine) = program.machine(machine_summary.symbol) else {
                continue;
            };

            if crate::value_custody::cleanup::is_reserved_cleanup(program, machine)
                && (machine.suspends
                    || machine.blocks
                    || machine_summary.transitive_may_suspend
                    || machine_summary.transitive_may_block)
            {
                diagnostics.push(Diagnostic::error(format!(
                    "cleanup machine `{}` must be transitively non-suspending and nonblocking",
                    machine.name
                )));
            }

            if machine.is_public || machine.supply_mode != MachineSupplyMode::CheckedBody {
                if machine_summary.body_may_suspend && !machine.suspends {
                    diagnostics.push(Diagnostic::error(format!(
                    "machine `{}` omits `suspends` from its published contract, but its body may suspend",
                    machine.name
                )));
                }
                if machine_summary.body_may_block && !machine.blocks {
                    diagnostics.push(Diagnostic::error(format!(
                    "machine `{}` omits `blocks` from its published contract, but its body may block",
                    machine.name
                )));
                }
            }
        }

        crate::program_validation::finish_diagnostics(diagnostics)
    }

    fn validate_service_reach_ceilings(
        program: &dyn crate::CheckingInput,
        service_reaches: &flow_effects::ServiceReachInferencePlan,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        for machine in program.machines() {
            if machine.target_family_interface {
                continue;
            }
            let Some(summary) = service_reaches.for_machine(machine.symbol) else {
                continue;
            };
            let declared = program
                .service_reach_rows
                .services(machine.service_reach_row);
            let ceiling_present = program
                .authored_service_reach_rows_for(machine.symbol)
                .next()
                .is_some();
            if machine.supply_mode == MachineSupplyMode::CheckedBody && !ceiling_present {
                continue;
            }
            let required = service_reaches.services(summary.inferred_transitive);
            let missing = required
                .iter()
                .copied()
                .filter(|service| !declared.contains(service))
                .collect::<Vec<_>>();
            if missing.is_empty() {
                continue;
            }
            let names = missing
                .iter()
                .map(|service| {
                    program
                        .service_reaches
                        .definition(*service)
                        .map(|definition| definition.name.as_str())
                        .unwrap_or("<unknown canonical service>")
                })
                .collect::<Vec<_>>();
            diagnostics.push(Diagnostic::error(format!(
            "machine `{}` publishes service reach `{}` but its checked body reaches undeclared service{} `{}`",
            machine.name,
            format_service_row(program, declared),
            if names.len() == 1 { "" } else { "s" },
            names.join(" + "),
        )));
        }
    }

    fn format_service_row(
        program: &dyn crate::CheckingInput,
        services: &[ServiceReachId],
    ) -> String {
        if services.is_empty() {
            return "<none>".to_owned();
        }
        services
            .iter()
            .map(|service| {
                program
                    .service_reaches
                    .definition(*service)
                    .map(|definition| definition.name.as_str())
                    .unwrap_or("<unknown canonical service>")
            })
            .collect::<Vec<_>>()
            .join(" + ")
    }

    pub(crate) mod asm_discharge {
        use diagnostics::Diagnostic;
        use language_core::inline_assembly::AsmAuthorityRequirement;
        use typed_trees::statement::StatementNode;

        #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
        pub struct AsmAuthorityAdmission {
            machine_owner: bool,
            port_io: bool,
            idt_control: bool,
        }

        impl AsmAuthorityAdmission {
            pub const HOSTED: Self = Self {
                machine_owner: false,
                port_io: false,
                idt_control: false,
            };

            pub const MACHINE_OWNER: Self = Self {
                machine_owner: true,
                port_io: true,
                idt_control: true,
            };

            pub const fn from_freestanding(freestanding: bool) -> Self {
                if freestanding {
                    Self::MACHINE_OWNER
                } else {
                    Self::HOSTED
                }
            }

            pub const fn with_grants(self, port_io: bool, interrupt_table: bool) -> Self {
                Self {
                    machine_owner: self.machine_owner,
                    port_io: self.port_io || port_io,
                    idt_control: self.idt_control || interrupt_table,
                }
            }

            pub const fn admits(&self, requirement: AsmAuthorityRequirement) -> bool {
                match requirement {
                    AsmAuthorityRequirement::None => true,
                    AsmAuthorityRequirement::MachineOwner => self.machine_owner,
                    AsmAuthorityRequirement::PortIo => self.port_io,
                    AsmAuthorityRequirement::IdtControl => self.idt_control,
                }
            }

            fn description(requirement: AsmAuthorityRequirement) -> &'static str {
                match requirement {
                    AsmAuthorityRequirement::None => "no authority",
                    AsmAuthorityRequirement::MachineOwner => "machine-owner authority",
                    AsmAuthorityRequirement::PortIo => "port-I/O authority",
                    AsmAuthorityRequirement::IdtControl => "interrupt-table publication authority",
                }
            }
        }

        fn authored_admission_fix(requirement: AsmAuthorityRequirement) -> &'static str {
            match requirement {
                AsmAuthorityRequirement::PortIo => {
                    "Set `b.freestanding = true` or grant exactly this class with \
             `b.privileged_services.port_io = true` in build.omg, \
             or remove the asm block"
                }
                AsmAuthorityRequirement::IdtControl => {
                    "Set `b.freestanding = true` or grant exactly this class with \
             `b.privileged_services.interrupt_table = true` in build.omg, \
             or remove the asm block"
                }
                AsmAuthorityRequirement::None | AsmAuthorityRequirement::MachineOwner => {
                    "Set `b.freestanding = true` in build.omg, or remove the asm block"
                }
            }
        }

        pub fn validate_asm_discharge(
            program: &dyn crate::CheckingInput,
            admission: AsmAuthorityAdmission,
        ) -> Result<(), Vec<Diagnostic>> {
            if admission == AsmAuthorityAdmission::MACHINE_OWNER {
                return Ok(());
            }

            let mut diagnostics = Vec::new();
            for machine in program.machines() {
                for state in program.machine_states(machine) {
                    for statement in program.statement_table.statements(state.statement_nodes) {
                        let Some((instruction, _, required_authority)) =
                            statement_asm_intrinsic(program, statement)
                        else {
                            continue;
                        };
                        if admission.admits(required_authority) {
                            continue;
                        }
                        diagnostics.push(Diagnostic::error(format!(
                            "machine `{}` uses asm instruction `{}`, which requires a FREESTANDING \
                     boundary root (the contract requires {}, and this build's \
                     privileged-service admission does not supply it). {}",
                            machine.name,
                            instruction,
                            AsmAuthorityAdmission::description(required_authority),
                            authored_admission_fix(required_authority)
                        )));
                    }
                }
            }

            crate::program_validation::finish_diagnostics(diagnostics)
        }

        pub(crate) fn validate_asm_intrinsic_declarations(
            program: &dyn crate::CheckingInput,
            service_reaches: &flow_effects::ServiceReachInferencePlan,
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            let assembly_services =
                crate::machine_calls::assembly_services::AssemblyServices::new(program);
            for machine in program.machines() {
                let mut direct_asm_services = Vec::new();

                for state in program.machine_states(machine) {
                    for statement in program.statement_table.statements(state.statement_nodes) {
                        let Some((instruction, service_name, _)) =
                            statement_asm_intrinsic(program, statement)
                        else {
                            continue;
                        };
                        let Some(service_name) = service_name else {
                            continue;
                        };
                        if !direct_asm_services.contains(&service_name) {
                            direct_asm_services.push(service_name);
                        }
                        if let Err(reason) =
                            assembly_services.resolve(program, machine.symbol, service_name)
                        {
                            diagnostics.push(Diagnostic::error(format!(
                                "machine `{}` uses asm instruction `{}`, which reaches canonical \
                         service `{service_name}`, but that service identity is unavailable: \
                         {reason}; add `use omega::language::core::assembly;`",
                                machine.name, instruction
                            )));
                        }
                    }
                }

                let Some(machine_services) = service_reaches.for_machine(machine.symbol) else {
                    continue;
                };
                for service_name in direct_asm_services {
                    let Ok(service) =
                        assembly_services.resolve(program, machine.symbol, service_name)
                    else {
                        continue;
                    };
                    debug_assert!(
                        service_reaches
                            .services(machine_services.inferred_transitive)
                            .contains(&service)
                    );
                }
            }
        }

        fn statement_asm_intrinsic(
            program: &dyn crate::CheckingInput,
            statement: &StatementNode,
        ) -> Option<(
            &'static str,
            Option<&'static str>,
            language_core::inline_assembly::AsmAuthorityRequirement,
        )> {
            let target_symbol = match statement {
                StatementNode::Call(call) => call.target_symbol,
                StatementNode::Assignment(assignment) => {
                    let typed_trees::expression::ExpressionNode::Call(call) =
                        program.expression_table.expression(assignment.value)
                    else {
                        return None;
                    };
                    call.target_symbol
                }
                _ => return None,
            };

            let function = program
                .symbols
                .builtin_function_for_symbol(target_symbol)
                .filter(|function| function.is_asm_intrinsic())?;
            let service_name = function.asm_intrinsic_service_name();
            let instruction =
                language_core::inline_assembly::asm_intrinsic_mnemonic(function.name())?;
            let language_core::inline_assembly::AsmCatalogEntry::Contract(contract) =
                language_core::inline_assembly::asm_catalog_entry(instruction)?
            else {
                return None;
            };
            Some((instruction, service_name, contract.required_authority))
        }
    }
    use asm_discharge::validate_asm_intrinsic_declarations;
    pub use asm_discharge::{AsmAuthorityAdmission, validate_asm_discharge};

    pub(crate) mod pure_discards {
        use diagnostics::Diagnostic;
        use symbols::SymbolHandle;

        pub(crate) fn validate_pure_discards(
            program: &dyn crate::CheckingInput,
            operational: &flow_effects::OperationalPlan,
            service_reaches: &flow_effects::ServiceReachInferencePlan,
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            let classification = crate::proof_only_classification(program);
            for machine in program.machines() {
                if classification.is_proof_machine(program, machine) {
                    continue;
                }
                for state in program.machine_states(machine) {
                    for statement in program.statement_table.statements(state.statement_nodes) {
                        let Some(call) = statement.as_call() else {
                            continue;
                        };
                        if !call.discards_result {
                            continue;
                        }

                        if machine_owning_state(program, call.target_symbol)
                            .is_some_and(|callee| classification.is_proof_machine(program, callee))
                        {
                            continue;
                        }

                        let Some(callee) = resolve_discard_callee(
                            program,
                            operational,
                            service_reaches,
                            call.target_symbol,
                        ) else {
                            continue;
                        };

                        if !callee.has_service_reach
                            && !callee.may_suspend
                            && !callee.may_block
                            && !callee.has_mutable_parameter
                        {
                            diagnostics.push(Diagnostic::warning(format!(
                        "`_ = {target}(...);` discards a provably pure call: `{target}` has no service reach, suspension, blocking, or mutable out-parameters, so the discard is dead code; remove the statement or use the result",
                        target = call.target.as_str()
                    )));
                        }
                    }
                }
            }
        }

        pub(crate) struct DiscardCallee {
            has_service_reach: bool,
            may_suspend: bool,
            may_block: bool,
            has_mutable_parameter: bool,
        }

        fn machine_owning_state(
            program: &dyn crate::CheckingInput,
            target_symbol: SymbolHandle,
        ) -> Option<&typed_trees::machine::Machine> {
            if !target_symbol.is_valid() {
                return None;
            }
            program.machine_containing_state(target_symbol)
        }

        fn resolve_discard_callee(
            program: &dyn crate::CheckingInput,
            operational: &flow_effects::OperationalPlan,
            service_reaches: &flow_effects::ServiceReachInferencePlan,
            target_symbol: SymbolHandle,
        ) -> Option<DiscardCallee> {
            if !target_symbol.is_valid() {
                return None;
            }

            for machine in program.machines() {
                let Some(state) = program.state_in_machine(machine, target_symbol) else {
                    continue;
                };

                let operational_may = operational
                    .machines()
                    .iter()
                    .find(|entry| entry.symbol == machine.symbol)
                    .map(|entry| (entry.transitive_may_suspend, entry.transitive_may_block))?;

                return Some(DiscardCallee {
                    has_service_reach: service_reaches.for_machine(machine.symbol).is_some_and(
                        |summary| {
                            !service_reaches
                                .services(summary.inferred_transitive)
                                .is_empty()
                                || !service_reaches.services(summary.effective).is_empty()
                        },
                    ),
                    may_suspend: operational_may.0,
                    may_block: operational_may.1,
                    has_mutable_parameter: program
                        .state_parameters(state)
                        .iter()
                        .any(|parameter| parameter.is_mutable),
                });
            }

            for trait_definition in program.traits() {
                if let Some(signature) =
                    program.trait_machine_signature_by_symbol(trait_definition, target_symbol)
                {
                    return Some(signature_discard_callee(
                        program,
                        trait_definition,
                        signature,
                    ));
                }
            }

            None
        }

        fn signature_discard_callee(
            program: &dyn crate::CheckingInput,
            trait_definition: &typed_trees::trait_definition::TraitDefinition,
            signature: &typed_trees::signature::StateSignature,
        ) -> DiscardCallee {
            DiscardCallee {
                has_service_reach: trait_definition.is_boundary
                    || !program
                        .service_reach_rows
                        .services(signature.service_reach_row)
                        .is_empty(),
                may_suspend: signature.suspends,
                may_block: signature.blocks,
                has_mutable_parameter: program
                    .state_signature_parameters(signature)
                    .iter()
                    .any(|parameter| parameter.is_mutable),
            }
        }
    }
    use pure_discards::validate_pure_discards;
}

pub mod fact_call_projections {
    use crate::CheckingInput;
    use crate::frozen_program::expression_table;
    use diagnostics::Diagnostic;
    use language_semantics::MachineSupplyMode;
    use typed_trees::data::DataDefinition;
    use typed_trees::data::DataShapeKind;
    use typed_trees::domain::ProofFact;
    use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCallExpression};
    use typed_trees::machine::Machine;
    use typed_trees::proposition::{PropositionBody, PropositionFormula};
    use typed_trees::signature::SignatureContractKind;
    use typed_trees::types::TypeReferenceNode;

    pub(crate) fn validate_fact_call_projections(
        program: &dyn CheckingInput,
        operational: &flow_effects::OperationalPlan,
        service_reaches: &flow_effects::ServiceReachInferencePlan,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Vec<ValidatedFactCallProjection> {
        let mut roots = Vec::new();
        let mut proposition_applications = Vec::new();
        for (_, fact) in typed_trees::references::program_proof_facts(program) {
            match fact {
                ProofFact::Expression(expression) => roots.push(*expression),
                ProofFact::Membership(membership) => roots.push(membership.value),
                ProofFact::Proposition(application) => {
                    proposition_applications.push(application);
                    roots.extend_from_slice(
                        expression_table(program).expression_handles(application.arguments),
                    );
                }
            }
        }
        for proposition in program.propositions() {
            if let PropositionBody::Transparent { proposition } = &proposition.body {
                match proposition {
                    PropositionFormula::Application(application) => roots.extend_from_slice(
                        expression_table(program).expression_handles(application.arguments),
                    ),
                    PropositionFormula::BooleanExpression(expression) => roots.push(*expression),
                }
            }
        }

        let substituted = proposition_applications
            .into_iter()
            .flat_map(|application| substituted_projection_requests(program, application))
            .collect::<Vec<_>>();
        let has_projection = !substituted.is_empty()
            || roots
                .iter()
                .any(|root| expression_contains_call_projection(program, *root));
        if !has_projection {
            return Vec::new();
        }
        let mut visited = Vec::new();
        let mut admitted = Vec::new();
        for root in roots {
            validate_expression(
                program,
                root,
                operational,
                service_reaches,
                diagnostics,
                &mut visited,
                &mut admitted,
            );
        }
        for (projection_expression, call_expression) in substituted {
            let Some(member) = expression_table(program).member_node(projection_expression) else {
                continue;
            };
            let Some(call) = expression_table(program).call_node(call_expression) else {
                continue;
            };
            if let Some(row) = validate_direct_projection(
                program,
                projection_expression,
                call_expression,
                call,
                member,
                operational,
                service_reaches,
                diagnostics,
            ) {
                admitted.push(row);
            }
        }
        admitted.sort_by_key(|row| {
            (
                row.projection_expression.arena_index(),
                row.call_expression.arena_index(),
                row.field.arena_index(),
            )
        });
        admitted.dedup();
        admitted
    }

    fn substituted_projection_requests(
        program: &dyn CheckingInput,
        application: &typed_trees::proposition::PropositionApplication,
    ) -> Vec<(ExpressionHandle, ExpressionHandle)> {
        let Some(declaration) = program.proposition_definition_by_symbol(application.proposition)
        else {
            return Vec::new();
        };
        let PropositionBody::Transparent {
            proposition: PropositionFormula::BooleanExpression(formula),
        } = declaration.body
        else {
            return Vec::new();
        };
        let parameters = program.proposition_parameters(declaration);
        let arguments = expression_table(program).expression_handles(application.arguments);
        if parameters.len() != arguments.len() {
            return Vec::new();
        }
        let bindings = parameters
            .iter()
            .zip(arguments)
            .map(|(parameter, argument)| (parameter.symbol, *argument))
            .collect::<Vec<_>>();
        let mut requests = Vec::new();
        append_substituted_projection_requests(program, formula, &bindings, &mut requests);
        requests
    }

    fn append_substituted_projection_requests(
        program: &dyn CheckingInput,
        expression: ExpressionHandle,
        bindings: &[(symbols::SymbolHandle, ExpressionHandle)],
        requests: &mut Vec<(ExpressionHandle, ExpressionHandle)>,
    ) {
        if !expression.is_valid() {
            return;
        }
        match expression_table(program).expression(expression) {
            ExpressionNode::Member(member) => {
                if let Some(path) = expression_table(program).name_path_node(member.receiver)
                    && let Some(actual) =
                        crate::queries::substitutions::actual_for(bindings, path.symbol)
                    && matches!(
                        expression_table(program).expression(actual),
                        ExpressionNode::Call(_)
                    )
                {
                    requests.push((expression, actual));
                }
                append_substituted_projection_requests(
                    program,
                    member.receiver,
                    bindings,
                    requests,
                );
            }
            other => crate::queries::expression_traversal::children(program, other, |child| {
                append_substituted_projection_requests(program, child, bindings, requests);
            }),
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ValidatedFactCallProjection {
        pub projection_expression: ExpressionHandle,
        pub call_expression: ExpressionHandle,
        pub target_machine: symbols::SymbolHandle,
        pub target_state: symbols::SymbolHandle,
        pub machine_arguments: Box<[typed_trees::expression::StaticMachineArgument]>,
        pub result_type: typed_trees::types::TypeReferenceHandle,
        pub field: symbols::SymbolHandle,
    }

    fn validate_expression(
        program: &dyn CheckingInput,
        expression: ExpressionHandle,
        operational: &flow_effects::OperationalPlan,
        service_reaches: &flow_effects::ServiceReachInferencePlan,
        diagnostics: &mut Vec<Diagnostic>,
        visited: &mut Vec<ExpressionHandle>,
        admitted: &mut Vec<ValidatedFactCallProjection>,
    ) {
        if !expression.is_valid() || visited.contains(&expression) {
            return;
        }
        visited.push(expression);
        macro_rules! recurse {
            ($child:expr, $diagnostics:expr, $visited:expr) => {
                validate_expression(
                    program,
                    $child,
                    operational,
                    service_reaches,
                    $diagnostics,
                    $visited,
                    admitted,
                )
            };
        }
        match expression_table(program).expression(expression) {
            ExpressionNode::Member(member) => {
                match expression_table(program).expression(member.receiver) {
                ExpressionNode::Call(call) => {
                    if let Some(row) = validate_direct_projection(
                        program,
                        expression,
                        member.receiver,
                        call,
                        member,
                        operational,
                        service_reaches,
                        diagnostics,
                    ) {
                        admitted.push(row);
                    }
                }
                _ if expression_contains_call(program, member.receiver) => diagnostics.push(
                    Diagnostic::error(
                        "fact-position call projection must be one direct plain-record field; nested or adapted projection is not admitted",
                    ),
                ),
                _ => {}
            }
                recurse!(member.receiver, diagnostics, visited);
            }
            other => crate::queries::expression_traversal::children(program, other, |child| {
                recurse!(child, diagnostics, visited);
            }),
        }
    }

    fn validate_direct_projection(
        program: &dyn CheckingInput,
        projection_expression: ExpressionHandle,
        call_expression: ExpressionHandle,
        call: &TableCallExpression,
        member: &typed_trees::expression::TableMemberExpression,
        operational: &flow_effects::OperationalPlan,
        service_reaches: &flow_effects::ServiceReachInferencePlan,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Option<ValidatedFactCallProjection> {
        let reject = |reason: &str, diagnostics: &mut Vec<Diagnostic>| {
            diagnostics.push(Diagnostic::error(format!(
                "fact-position projection from call `{}` is not denotational: {reason}",
                call.target
            )));
        };
        if (call.receiver.is_valid() && !program.call_has_no_runtime_receiver_for_target(call))
            || call.quotient_operation.is_some()
            || !call.evidence_arguments.is_empty()
            || expression_table(program)
                .expression_handles(call.arguments)
                .iter()
                .any(|argument| expression_contains_call(program, *argument))
        {
            reject(
                "only one direct free call with non-call runtime arguments is admitted",
                diagnostics,
            );
            return None;
        }

        let abstract_signature =
            crate::machine_calls::denotational_calls::abstract_value_call_signature(
                program,
                call.target_symbol,
            );
        let concrete = if abstract_signature.is_some() {
            None
        } else {
            Some(validate_checked_call_candidate(
                program,
                call,
                operational,
                service_reaches,
                &reject,
                diagnostics,
            )?)
        };
        let result_type = abstract_signature
            .map(|signature| signature.return_type)
            .or_else(|| concrete.map(|(_, state)| state.return_type))?;

        let (data_symbol, qualified) =
            match program.type_reference_table.type_reference(result_type) {
                TypeReferenceNode::Named { symbol, .. } => (*symbol, false),
                TypeReferenceNode::Generic { base_symbol, .. } => (*base_symbol, false),
                TypeReferenceNode::Constrained { .. } => (symbols::SymbolHandle::invalid(), true),
                _ => (symbols::SymbolHandle::invalid(), false),
            };
        if qualified {
            reject(
                "content-bearing or otherwise qualified results are not admitted",
                diagnostics,
            );
            return None;
        }
        let Some(data) =
            crate::declarations::data_queries::data_definition_by_symbol(program, data_symbol)
        else {
            reject("the result is not one nominal plain record", diagnostics);
            return None;
        };
        if data.supply_mode != language_semantics::DataSupplyMode::CheckedShape
            || DataShapeKind::Record
                != DataDefinition::shape_kind_from_members(program.data_members(data))
        {
            reject(
                "sum, mixed, empty, and opaque result shapes are not admitted",
                diagnostics,
            );
            return None;
        }
        let field = (member.case_variant.is_none())
            .then(|| {
                program.data_members(data).iter().find_map(|candidate| {
                    let field = candidate.as_field()?;
                    (field.name.as_str() == member.member.as_str()).then_some(field.symbol)
                })
            })
            .flatten();
        let Some(field) = field else {
            reject(
                "the selected member is not one exact direct record field",
                diagnostics,
            );
            return None;
        };
        let (machine, state) = concrete?;
        Some(ValidatedFactCallProjection {
            projection_expression,
            call_expression,
            target_machine: machine.symbol,
            target_state: state.symbol,
            machine_arguments: call.machine_arguments.clone(),
            result_type,
            field,
        })
    }

    pub fn has_checked_denotational_call_identity(
        program: &dyn CheckingInput,
        call: &TableCallExpression,
        operational: &flow_effects::OperationalPlan,
        service_reaches: &flow_effects::ServiceReachInferencePlan,
    ) -> bool {
        validate_checked_call_candidate(
            program,
            call,
            operational,
            service_reaches,
            &|_, _| {},
            &mut Vec::new(),
        )
        .is_some()
    }

    pub(crate) fn validate_checked_call_candidate<'program>(
        program: &'program dyn CheckingInput,
        call: &TableCallExpression,
        operational: &flow_effects::OperationalPlan,
        service_reaches: &flow_effects::ServiceReachInferencePlan,
        reject: &impl Fn(&str, &mut Vec<Diagnostic>),
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Option<(&'program Machine, &'program typed_trees::state::State)> {
        checked_call_target_candidate(
            program,
            call.target_symbol,
            operational,
            service_reaches,
            reject,
            diagnostics,
        )
    }

    pub(crate) fn checked_call_target_candidate<'program>(
        program: &'program dyn CheckingInput,
        target: symbols::SymbolHandle,
        operational: &flow_effects::OperationalPlan,
        service_reaches: &flow_effects::ServiceReachInferencePlan,
        reject: &impl Fn(&str, &mut Vec<Diagnostic>),
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Option<(&'program Machine, &'program typed_trees::state::State)> {
        let matches = program
            .machines()
            .iter()
            .flat_map(|machine| {
                program
                    .machine_states(machine)
                    .iter()
                    .filter(|state| state.symbol == target)
                    .map(move |state| (machine, state))
            })
            .collect::<Vec<_>>();
        let [(machine, state)] = matches.as_slice() else {
            reject(
                "the call target does not resolve to one exact entry",
                diagnostics,
            );
            return None;
        };
        if machine.supply_mode != MachineSupplyMode::CheckedBody || !machine.body_is_present {
            reject(
                "bodyless, boundary, accepted, and external targets are not admitted",
                diagnostics,
            );
            return None;
        }
        let has_requires = program
            .machine_contracts(machine)
            .iter()
            .chain(program.state_contracts(state))
            .any(|contract| {
                matches!(contract.kind, SignatureContractKind::Requires)
                    && !program.proof_facts.span_or_empty(contract.facts).is_empty()
            });
        if has_requires {
            reject("the selected entry has preconditions", diagnostics);
            return None;
        }
        if program.state_parameters(state).iter().any(|parameter| {
            crate::proof_contracts::carrier_contents::type_has_forbidden_value_observation_content(
                program,
                parameter.type_reference,
            )
        }) {
            reject(
                "runtime arguments contain borrowed, opaque, linear or routed custody",
                diagnostics,
            );
            return None;
        }
        if !crate::machine_calls::denotational_calls::has_pure_effect_closure(
            program,
            machine.symbol,
            state.symbol,
            program
                .state_parameters(state)
                .iter()
                .any(|parameter| parameter.is_mutable),
            operational,
            service_reaches,
        ) {
            reject(
                "the selected call closure is effectful, reaching, blocking, suspending, mutable, or unresolved",
                diagnostics,
            );
            return None;
        }
        if !crate::machine_calls::denotational_calls::has_observation_free_checked_closure(
            program,
            machine.symbol,
            operational,
        ) {
            reject(
                "the selected call closure observes or mutates hidden state, uses atomic/external machinery, or contains a non-checked target",
                diagnostics,
            );
            return None;
        }
        if !crate::machine_calls::denotational_calls::has_no_crash_routes(
            program,
            machine.symbol,
            operational,
        ) {
            reject("the selected call closure has a crash route", diagnostics);
            return None;
        }
        Some((*machine, *state))
    }

    pub(crate) fn expression_contains_call_projection(
        program: &dyn CheckingInput,
        expression: ExpressionHandle,
    ) -> bool {
        use typed_trees::SubtreeVisit;
        program.subtree_any(
            expression,
            typed_trees::SubtreeEdges::STRUCTURAL,
            |_, node| match node {
                ExpressionNode::Member(member)
                    if expression_contains_call(program, member.receiver) =>
                {
                    SubtreeVisit::Found
                }
                ExpressionNode::Member(_) | ExpressionNode::Match(_) => SubtreeVisit::Prune,
                _ => SubtreeVisit::Descend,
            },
        )
    }

    fn expression_contains_call(program: &dyn CheckingInput, expression: ExpressionHandle) -> bool {
        if !expression.is_valid() {
            return false;
        }
        match expression_table(program).expression(expression) {
            ExpressionNode::Call(_) => true,
            ExpressionNode::Member(member) => expression_contains_call(program, member.receiver),
            ExpressionNode::Borrow(inner) => expression_contains_call(program, inner.target),
            ExpressionNode::Cast(cast) => expression_contains_call(program, cast.value),
            _ => false,
        }
    }
}

pub mod fact_only_citations {
    use crate::frozen_program::expression_table;
    use symbols::SymbolHandle;
    use typed_trees::data::TypeParameterKind;
    use typed_trees::expression::{ExpressionHandle, ExpressionNode, StaticMachineArgument};
    use typed_trees::machine::Machine;
    use typed_trees::signature::SignatureContractKind;
    use typed_trees::signature::StateParameter;
    use typed_trees::statement::{StatementNode, TableCall};
    use typed_trees::types::TypeReferenceNode;

    fn machine_by_target(
        program: &dyn crate::CheckingInput,
        target: SymbolHandle,
    ) -> Option<&Machine> {
        if !target.is_valid() {
            return None;
        }
        program.machines().iter().find(|machine| {
            machine.symbol == target
                || program
                    .machine_states(machine)
                    .first()
                    .is_some_and(|entry| entry.symbol == target)
        })
    }

    fn has_checked_effect_and_termination_closure(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        entry: &typed_trees::state::State,
        termination: &impl crate::CheckedTerminationOracle,
    ) -> bool {
        if !termination.unconditionally_terminates(machine.symbol) {
            return false;
        }
        if let Some(plans) = program.prepared_effect_plans() {
            return crate::machine_calls::denotational_calls::checked_value_call_candidate(
                program,
                machine,
                entry,
                &plans.operational,
                &plans.service_reaches,
            )
            .is_ok();
        }
        let operational = crate::infer_operational_may(program);
        let service_reaches = crate::infer_service_reaches(program, &operational);
        crate::machine_calls::denotational_calls::checked_value_call_candidate(
            program,
            machine,
            entry,
            &operational,
            &service_reaches,
        )
        .is_ok()
    }

    pub fn fact_only_theorem<'program>(
        program: &'program dyn crate::CheckingInput,
        target: SymbolHandle,
        termination: &impl crate::CheckedTerminationOracle,
    ) -> Option<&'program Machine> {
        let machine = machine_by_target(program, target)?;
        if crate::mathematical_applications::is_indexed_logical_machine(program, machine) {
            return Some(machine);
        }
        let entry = program.machine_states(machine).first()?;
        let theorem_only = machine.attached_data.is_none()
            && machine.supply_mode == language_semantics::MachineSupplyMode::CheckedBody
            && machine.body_is_present
            && !machine.suspends
            && !machine.blocks
            && !program.machine_declares_crash(machine)
            && program.machine_type_parameters(machine).len()
                == program.machine_kind_type_parameters(machine).count()
            && program.machine_owned_data(machine).is_empty()
            && program
                .service_reach_rows
                .services(machine.service_reach_row)
                .is_empty()
            && matches!(
                program
                    .type_reference_table
                    .type_reference(entry.return_type),
                TypeReferenceNode::Unit
            )
            && program.machine_contracts(machine).iter().all(|contract| {
                matches!(
                    contract.kind,
                    SignatureContractKind::Requires | SignatureContractKind::Ensures
                ) && contract.binding.is_none()
            })
            && program.machine_states(machine).iter().all(|state| {
                program.state_contracts(state).iter().all(|contract| {
                    matches!(
                        contract.kind,
                        SignatureContractKind::Requires | SignatureContractKind::Ensures
                    ) && contract.binding.is_none()
                })
            })
            && (crate::proof_only_classification(program).is_proof_machine(program, machine)
                || program
                    .machine_contracts(machine)
                    .iter()
                    .any(|contract| contract.kind == SignatureContractKind::Ensures)
                || program
                    .state_contracts(entry)
                    .iter()
                    .any(|contract| contract.kind == SignatureContractKind::Ensures))
            && program
                .state_parameters(entry)
                .iter()
                .all(|parameter| read_only_parameter(program, parameter))
            && has_checked_effect_and_termination_closure(program, machine, entry, termination);
        theorem_only.then_some(machine)
    }

    fn read_only_parameter(program: &dyn crate::CheckingInput, parameter: &StateParameter) -> bool {
        if parameter.is_mutable {
            return false;
        }
        if matches!(
            program
                .type_reference_table
                .type_reference(parameter.type_reference),
            TypeReferenceNode::Reference {
                access: language_core::ReferenceAccess::Shared,
                ..
            }
        ) {
            return true;
        }
        !crate::type_carries_linear_obligation(program, parameter.type_reference)
            && (program
                .primitive_type_reference(parameter.type_reference)
                .is_some()
                || proof_value_parameter(program, parameter.type_reference)
                || crate::CallFrameResolver::new(program).is_some_and(|frames| {
                    frames.proof_value_is_caller_isolated(parameter.type_reference)
                }))
    }

    fn proof_value_parameter(
        program: &dyn crate::CheckingInput,
        reference: typed_trees::types::TypeReferenceHandle,
    ) -> bool {
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Named { symbol, .. } => {
                crate::proof_only_classification(program).is_proof_only(*symbol)
            }
            TypeReferenceNode::Constrained { base_type, .. } => {
                proof_value_parameter(program, *base_type)
            }
            _ => false,
        }
    }

    fn static_machine_argument_is_eligible(
        program: &dyn crate::CheckingInput,
        argument: &StaticMachineArgument,
        termination: &impl crate::CheckedTerminationOracle,
    ) -> bool {
        if let Some(application) = &argument.application
            && !application
                .arguments
                .iter()
                .all(|nested| static_machine_argument_is_eligible(program, nested, termination))
        {
            return false;
        }
        if argument.type_reference.is_valid() || argument.const_literal.is_some() {
            return true;
        }
        if argument.evidence_projection.is_some() {
            return false;
        }
        if !argument.symbol.is_valid() {
            return false;
        }
        if argument.names_implementation(&program.symbols) {
            return program.symbols.get(argument.symbol).kind == symbols::SymbolKind::Conformance;
        }
        let Some(selected) = machine_by_target(program, argument.symbol) else {
            return false;
        };
        let Some(entry) = program.machine_states(selected).first() else {
            return false;
        };
        has_checked_effect_and_termination_closure(program, selected, entry, termination)
    }

    fn static_machine_argument_terminates(
        program: &dyn crate::CheckingInput,
        argument: &StaticMachineArgument,
        termination: &impl crate::CheckedTerminationOracle,
    ) -> bool {
        if let Some(application) = &argument.application
            && !application
                .arguments
                .iter()
                .all(|nested| static_machine_argument_terminates(program, nested, termination))
        {
            return false;
        }
        if argument.type_reference.is_valid()
            || argument.const_literal.is_some()
            || argument.evidence_projection.is_some()
            || !argument.symbol.is_valid()
            || argument.names_implementation(&program.symbols)
        {
            return true;
        }
        let Some(selected) = machine_by_target(program, argument.symbol) else {
            return true;
        };
        termination.unconditionally_terminates(selected.symbol)
    }

    pub fn validate_runtime_proof_citations(
        program: &dyn crate::CheckingInput,
        termination: &impl crate::CheckedTerminationOracle,
    ) -> Result<(), Vec<diagnostics::Diagnostic>> {
        let proof_only = crate::proof_only_classification(program);
        let mut diagnostics = Vec::new();
        for machine in program.machines() {
            if proof_only.is_proof_machine(program, machine)
                || crate::value_custody::content_projections::is_content_projection_machine(
                    program, machine,
                )
            {
                continue;
            }
            for state in program.machine_states(machine) {
                for statement in program.statement_table.statements(state.statement_nodes) {
                    let Some(call) = statement.as_call() else {
                        continue;
                    };
                    if !call.receiver.is_empty() {
                        continue;
                    }
                    let Some(callee) = machine_by_target(program, call.target_symbol) else {
                        continue;
                    };
                    if !proof_only.is_proof_machine(program, callee) {
                        continue;
                    }
                    if is_fact_only_citation_call(program, call, termination) {
                        continue;
                    }
                    let Some(entry) = program.machine_states(callee).first() else {
                        continue;
                    };
                    if callee.suspends
                        || callee.blocks
                        || program.machine_declares_crash(callee)
                        || !program.machine_owned_data(callee).is_empty()
                        || !program
                            .service_reach_rows
                            .services(callee.service_reach_row)
                            .is_empty()
                    {
                        diagnostics.push(diagnostics::Diagnostic::error(format!(
                        "theorem citation `{}` must be effect-free; proof citations cannot declare or infer service reach, suspension, blocking, crashes, or mutable state",
                        call.target
                    )));
                    } else if !termination.unconditionally_terminates(callee.symbol)
                        || !call.machine_arguments.iter().all(|argument| {
                            static_machine_argument_terminates(program, argument, termination)
                        })
                    {
                        diagnostics.push(diagnostics::Diagnostic::error(format!(
                        "theorem citation `{}` requires unconditional termination across its call and static machine arguments",
                        call.target
                    )));
                    } else if !has_checked_effect_and_termination_closure(
                        program,
                        callee,
                        entry,
                        termination,
                    ) || !call.machine_arguments.iter().all(|argument| {
                        static_machine_argument_is_eligible(program, argument, termination)
                    }) {
                        diagnostics.push(diagnostics::Diagnostic::error(format!(
                        "theorem citation `{}` must be effect-free; proof citations cannot declare or infer service reach, suspension, blocking, crashes, or mutable state",
                        call.target
                    )));
                    } else if !program
                        .state_parameters(entry)
                        .iter()
                        .all(|parameter| read_only_parameter(program, parameter))
                    {
                        diagnostics.push(diagnostics::Diagnostic::error(format!(
                        "theorem citation `{}` cannot take mutable or linear parameters; proof citations erase before execution and cannot transfer linear custody or write storage",
                        call.target
                    )));
                    } else {
                        diagnostics.push(diagnostics::Diagnostic::error(format!(
                        "theorem citation `{}` is not eligible for fact-only erasure; a runtime statement call to a proof machine must be a read-only, work-free theorem citation",
                        call.target
                    )));
                    }
                }
            }
        }
        crate::program_validation::finish_strict_diagnostics(diagnostics)
    }

    pub fn is_fact_only_citation(
        program: &dyn crate::CheckingInput,
        statement: &StatementNode,
        termination: &impl crate::CheckedTerminationOracle,
    ) -> bool {
        let Some(call) = statement.as_call() else {
            return false;
        };
        is_fact_only_citation_call(program, call, termination)
    }

    pub fn is_fact_only_citation_call(
        program: &dyn crate::CheckingInput,
        call: &TableCall,
        termination: &impl crate::CheckedTerminationOracle,
    ) -> bool {
        if call.authored_machine_application.is_valid()
            && is_fact_only_requirement_citation(
                program,
                expression_table(program)
                    .static_machine_application(call.authored_machine_application)
                    .lexical_owner,
                call,
            )
        {
            return true;
        }
        call.receiver.is_empty()
            && !call.receiver_symbol.is_valid()
            && !call.static_machine_parameter.is_valid()
            && call.static_requirement_dispatch.is_none()
            && (call.machine_arguments.iter().all(|argument| {
                static_machine_argument_is_eligible(program, argument, termination)
            }) || fact_only_theorem(program, call.target_symbol, termination).is_some_and(
                |theorem| {
                    crate::mathematical_applications::is_indexed_logical_machine(program, theorem)
                        && program
                            .machine_type_parameters(theorem)
                            .iter()
                            .all(|parameter| {
                                matches!(parameter.kind, TypeParameterKind::Value { .. })
                            })
                },
            ))
            && call.evidence_arguments.is_empty()
            && !call.operational_acknowledgement.acknowledges_suspend
            && !call.operational_acknowledgement.acknowledges_block
            && !call.discards_result
            && fact_only_theorem(program, call.target_symbol, termination).is_some_and(|theorem| {
                program
                    .machine_states(theorem)
                    .first()
                    .is_some_and(|entry| {
                        program.state_parameters(entry).len()
                            == program
                                .statement_table
                                .expression_handles(call.arguments)
                                .len()
                    })
            })
            && program
                .statement_table
                .expression_handles(call.arguments)
                .iter()
                .all(|argument| work_free_argument(program, *argument))
    }

    pub fn is_fact_only_requirement_citation(
        program: &dyn crate::CheckingInput,
        owner: SymbolHandle,
        call: &TableCall,
    ) -> bool {
        let namespace = call.receiver_symbol.is_valid()
            && (program
                .conformance_by_symbol(call.receiver_symbol)
                .is_some()
                || program.symbols.get(call.receiver_symbol).kind
                    == symbols::SymbolKind::ConformanceParameter);
        let closed_selection = call.static_requirement_dispatch.is_some()
            && call.receiver.is_empty()
            && !call.receiver_symbol.is_valid();
        if !owner.is_valid()
            || !(namespace || closed_selection)
            || !call.evidence_arguments.is_empty()
            || !call.machine_arguments.is_empty()
            || call.static_machine_parameter.is_valid()
            || call.operational_acknowledgement.acknowledges_suspend
            || call.operational_acknowledgement.acknowledges_block
            || call.discards_result
        {
            return false;
        }
        let Some((_, signature)) = crate::requirement_contract_signature(
            program,
            owner,
            &crate::CallSite::Statement(call),
        ) else {
            return false;
        };
        let contracts = program.state_signature_contracts(signature);
        let parameters = program.state_signature_parameters(signature);
        let arguments = program.statement_table.expression_handles(call.arguments);
        matches!(
            program
                .type_reference_table
                .type_reference(signature.return_type),
            TypeReferenceNode::Unit
        ) && crate::machine_calls::denotational_calls::signature_has_closed_value_contract(
            program, signature,
        ) && contracts
            .iter()
            .any(|contract| contract.kind == SignatureContractKind::Ensures)
            && contracts.iter().all(|contract| {
                matches!(
                    contract.kind,
                    SignatureContractKind::Requires | SignatureContractKind::Ensures
                ) && contract.binding.is_none()
            })
            && parameters.len() == arguments.len()
            && parameters.iter().all(|parameter| {
                if parameter.relevance.is_erased()
                    || parameter.is_mutable
                    || crate::type_carries_linear_obligation(program, parameter.type_reference)
                {
                    return false;
                }
                if read_only_parameter(program, parameter) {
                    return true;
                }
                let reference = program
                    .type_reference_table
                    .unconstrained_type_reference(parameter.type_reference);
                let TypeReferenceNode::Named { symbol, .. } =
                    program.type_reference_table.type_reference(reference)
                else {
                    return false;
                };
                program
                    .data_type_parameter_by_symbol(*symbol)
                    .is_some_and(|parameter| parameter.kind == TypeParameterKind::Type)
            })
            && arguments
                .iter()
                .all(|argument| work_free_argument(program, *argument))
    }

    fn work_free_argument(program: &dyn crate::CheckingInput, argument: ExpressionHandle) -> bool {
        if !expression_table(program).expression_is_valid(argument) {
            return false;
        }
        match expression_table(program).expression(argument) {
            ExpressionNode::Integer(_) | ExpressionNode::Boolean(_) | ExpressionNode::Float(_) => {
                true
            }
            ExpressionNode::Borrow(borrow) => {
                borrow.access == language_core::ReferenceAccess::Shared
                    && place_spelling(program, borrow.target)
            }
            ExpressionNode::StructLiteral(literal) => {
                (crate::proof_only_classification(program).is_proof_only(literal.type_symbol)
                    || program.prepared_mathematics().is_some_and(|view| {
                        view.source_evidence(argument).is_some_and(|evidence| {
                            evidence.type_identity.is_valid()
                                && evidence.type_identity
                                    == program.normalized_type_identity(evidence.source_type)
                        })
                    }))
                    && expression_table(program)
                        .struct_fields(literal.fields)
                        .iter()
                        .all(|field| work_free_argument(program, field.value))
            }
            _ => place_spelling(program, argument),
        }
    }

    fn place_spelling(program: &dyn crate::CheckingInput, expression: ExpressionHandle) -> bool {
        if !expression_table(program).expression_is_valid(expression) {
            return false;
        }
        match expression_table(program).expression(expression) {
            ExpressionNode::Name(_) => true,
            ExpressionNode::Member(member) => {
                member.case_variant.is_none() && place_spelling(program, member.receiver)
            }
            _ => false,
        }
    }
}

pub mod graph_components {
    pub(crate) fn strongly_connected_components(adjacency: &[Vec<usize>]) -> Vec<Vec<usize>> {
        fn finish_order(
            node: usize,
            adjacency: &[Vec<usize>],
            visited: &mut [bool],
            order: &mut Vec<usize>,
        ) {
            if std::mem::replace(&mut visited[node], true) {
                return;
            }
            for next in &adjacency[node] {
                finish_order(*next, adjacency, visited, order);
            }
            order.push(node);
        }

        fn collect_component(
            node: usize,
            reverse: &[Vec<usize>],
            visited: &mut [bool],
            component: &mut Vec<usize>,
        ) {
            if std::mem::replace(&mut visited[node], true) {
                return;
            }
            component.push(node);
            for next in &reverse[node] {
                collect_component(*next, reverse, visited, component);
            }
        }

        let mut order = Vec::with_capacity(adjacency.len());
        let mut visited = vec![false; adjacency.len()];
        for node in 0..adjacency.len() {
            finish_order(node, adjacency, &mut visited, &mut order);
        }
        let mut reverse = vec![Vec::new(); adjacency.len()];
        for (source, targets) in adjacency.iter().enumerate() {
            for target in targets {
                reverse[*target].push(source);
            }
        }
        for targets in &mut reverse {
            targets.sort_unstable();
            targets.dedup();
        }
        visited.fill(false);
        let mut components = Vec::new();
        while let Some(node) = order.pop() {
            if visited[node] {
                continue;
            }
            let mut component = Vec::new();
            collect_component(node, &reverse, &mut visited, &mut component);
            component.sort_unstable();
            components.push(component);
        }
        components
    }

    pub(crate) fn component_ids(adjacency: &[Vec<usize>]) -> Vec<usize> {
        let mut ids = vec![usize::MAX; adjacency.len()];
        for (component, members) in strongly_connected_components(adjacency)
            .into_iter()
            .enumerate()
        {
            for member in members {
                ids[member] = component;
            }
        }
        ids
    }
}

pub mod invocations {
    use crate::CheckingInput;
    use crate::machine_calls::effect_inference::full_parameter_invocation;
    use diagnostics::Diagnostic;
    use typed_trees::machine::Machine;
    use typed_trees::signature::AuthoredInvocationTarget;
    use typed_trees::signature::StateParameter;

    pub(crate) fn validate_invocation_contracts(
        program: &dyn CheckingInput,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        validate_declared_targets(program, diagnostics);
        let plan = crate::infer_synchronous_invocations(program);

        for machine in program.machines() {
            let Some(summary) = plan.for_machine(machine.symbol) else {
                continue;
            };
            let publishes = machine.supply_mode
                != language_semantics::MachineSupplyMode::CheckedBody
                || machine.is_public
                || !program.machine_invokes(machine).is_empty();
            if publishes {
                let parameters = program
                    .machine_states(machine)
                    .first()
                    .map(|state| program.state_parameters(state))
                    .unwrap_or_default();
                report_missing_targets(
                    program,
                    parameters,
                    &summary.inferred_transitive,
                    &summary.published,
                    |binding, ceiling| {
                        format!(
                            "machine `{}` omits `invokes {binding}` from its published synchronous-invocation ceiling, but its body may invoke that binding before returning (published invokes: [{ceiling}])",
                            machine.name,
                        )
                    },
                    diagnostics,
                );
            }
            validate_conformance_refinement(program, machine, summary, diagnostics);
        }
    }

    fn validate_declared_targets(program: &dyn CheckingInput, diagnostics: &mut Vec<Diagnostic>) {
        for trait_definition in program.traits() {
            for signature in program.trait_machine_signatures(trait_definition) {
                validate_target_names(
                    program,
                    &format!("{}::{}", trait_definition.name, signature.name),
                    program.state_signature_invokes(signature),
                    program.state_signature_parameters(signature),
                    diagnostics,
                );
            }
        }
        for machine in program.machines() {
            let parameters = program
                .machine_states(machine)
                .first()
                .map(|state| program.state_parameters(state))
                .unwrap_or_default();
            validate_target_names(
                program,
                machine.name.as_str(),
                program.machine_invokes(machine),
                parameters,
                diagnostics,
            );
        }
    }

    fn validate_target_names(
        program: &dyn CheckingInput,
        owner: &str,
        targets: &[typed_trees::signature::AuthoredInvocation],
        parameters: &[StateParameter],
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let mut seen = Vec::new();
        for target in targets {
            if target.target != AuthoredInvocationTarget::Unresolved
                && seen.contains(&target.target)
            {
                diagnostics.push(Diagnostic::error(format!(
                    "callable `{owner}` declares `invokes {}` more than once",
                    target.name,
                )));
                continue;
            }
            seen.push(target.target);

            match target.target {
                AuthoredInvocationTarget::Unresolved => {
                    diagnostics.push(Diagnostic::error(format!(
                    "callable `{owner}` declares `invokes {}`, but `{}` is neither one of its boundary-binding parameters nor a known boundary trait",
                    target.name,
                    target.name,
                )));
                }
                AuthoredInvocationTarget::Parameter { ordinal, symbol } => {
                    let parameter = parameters.get(ordinal as usize);
                    if parameter.is_none_or(|parameter| {
                        parameter.symbol != symbol
                            || boundary_trait_for_type(program, parameter.type_reference).is_none()
                    }) {
                        diagnostics.push(Diagnostic::error(format!(
                        "callable `{owner}` retains `invokes {}` with a stale or non-boundary parameter target",
                        target.name,
                    )));
                    }
                }
                AuthoredInvocationTarget::Service(symbol) => {
                    if !program
                        .traits()
                        .iter()
                        .any(|definition| definition.symbol == symbol && definition.is_boundary)
                    {
                        diagnostics.push(Diagnostic::error(format!(
                        "callable `{owner}` retains `invokes {}` with a stale or non-boundary service target",
                        target.name,
                    )));
                    }
                }
            }
        }
    }

    fn validate_conformance_refinement(
        program: &dyn CheckingInput,
        machine: &Machine,
        summary: &flow_effects::MachineInvocationInference,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        for conformance in program.machine_trait_conformances(machine) {
            let Some(trait_definition) = program.trait_definition_by_symbol(conformance.symbol)
            else {
                continue;
            };
            let requirements = program.trait_machine_signatures(trait_definition);
            let matching = requirements
                .iter()
                .filter(|requirement| {
                    conformance
                        .requirement
                        .as_ref()
                        .map(|name| name.as_str() == requirement.name.as_str())
                        .unwrap_or_else(|| {
                            machine
                                .name
                                .as_str()
                                .rsplit("::")
                                .next()
                                .is_some_and(|name| name == requirement.name.as_str())
                        })
                })
                .collect::<Vec<_>>();
            for requirement in matching {
                let parameters = program.state_signature_parameters(requirement);
                let allowed = crate::declared_signature_invocations(program, requirement)
                    .into_iter()
                    .map(|target| full_parameter_invocation(target, parameters))
                    .collect::<Option<Vec<_>>>();
                let actual_parameters = program
                    .machine_states(machine)
                    .first()
                    .map(|state| program.state_parameters(state))
                    .unwrap_or_default();
                let self_forwarded = trait_definition.is_boundary
                    && crate::has_self_forwarded_boundary_parameter(
                        program,
                        machine,
                        trait_definition.symbol,
                        parameters.len(),
                    );
                let actual = summary
                    .inferred_transitive
                    .iter()
                    .filter_map(|target| match *target {
                        flow_effects::InvocationTarget::Parameter(0) if self_forwarded => None,
                        flow_effects::InvocationTarget::Parameter(index) if self_forwarded => {
                            Some(flow_effects::InvocationTarget::Parameter(index - 1))
                        }
                        target => Some(target),
                    })
                    .map(|target| {
                        full_parameter_invocation(
                            target,
                            if self_forwarded {
                                parameters
                            } else {
                                actual_parameters
                            },
                        )
                    })
                    .collect::<Option<Vec<_>>>();
                let (Some(actual), Some(allowed)) = (actual, allowed) else {
                    diagnostics.push(Diagnostic::error(format!(
                    "machine `{}` conformance to `{}::{}` retains an invocation outside its declared parameter roster",
                    machine.name, trait_definition.name, requirement.name,
                )));
                    continue;
                };
                report_missing_targets(
                    program,
                    parameters,
                    &actual,
                    &allowed,
                    |binding, ceiling| {
                        format!(
                            "machine `{}` does not refine `{}::{}`: its body may synchronously invoke binding `{binding}`, but the requirement omits `invokes {binding}` (requirement invokes: [{ceiling}])",
                            machine.name, trait_definition.name, requirement.name,
                        )
                    },
                    diagnostics,
                );
            }
        }
    }

    fn report_missing_targets(
        program: &dyn CheckingInput,
        parameters: &[StateParameter],
        actual: &[flow_effects::InvocationTarget],
        allowed: &[flow_effects::InvocationTarget],
        message: impl Fn(&str, &str) -> String,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let ceiling = allowed
            .iter()
            .map(|target| invocation_target_label(program, parameters, *target))
            .collect::<Vec<_>>()
            .join(", ");
        for target in actual {
            if allowed.contains(target) {
                continue;
            }
            let binding = invocation_target_label(program, parameters, *target);
            diagnostics.push(Diagnostic::error(message(&binding, &ceiling)));
        }
    }

    fn invocation_target_label(
        program: &dyn CheckingInput,
        parameters: &[StateParameter],
        target: flow_effects::InvocationTarget,
    ) -> String {
        match target {
            flow_effects::InvocationTarget::Parameter(index) => parameters
                .get(index as usize)
                .map(|parameter| parameter.name.as_str().to_owned())
                .unwrap_or_else(|| format!("parameter#{index}")),
            flow_effects::InvocationTarget::Service(symbol) => program
                .traits()
                .iter()
                .find(|definition| definition.is_boundary && definition.symbol == symbol)
                .map(|definition| definition.name.as_str().to_owned())
                .unwrap_or_else(|| format!("service#{}", symbol.arena_index())),
        }
    }

    pub(crate) fn boundary_requirement_symbol(
        program: &dyn CheckingInput,
        type_reference: typed_trees::types::TypeReferenceHandle,
    ) -> symbols::SymbolHandle {
        typed_trees::invocations::boundary_requirement_symbol(
            program.typed_program(),
            type_reference,
        )
    }

    fn boundary_trait_for_type(
        program: &dyn CheckingInput,
        type_reference: typed_trees::types::TypeReferenceHandle,
    ) -> Option<&typed_trees::trait_definition::TraitDefinition> {
        let symbol = boundary_requirement_symbol(program, type_reference);
        program
            .traits()
            .iter()
            .find(|definition| definition.is_boundary && definition.symbol == symbol)
    }
}

pub mod machine_data {
    use crate::declarations::symbols::TopLevelSymbols;
    use crate::value_custody::expression_types::{
        argument_matches_type_reference_handle, expression_type_name_handle,
    };
    use crate::value_custody::type_references::{
        TypeParameterScope, TypeReferenceOwner, validate_type_reference_handle,
    };
    use diagnostics::Diagnostic;
    use std::fmt;
    use typed_trees::expression::ExpressionHandle;
    use typed_trees::machine::Machine;
    use typed_trees::types::TypeReferenceHandle;

    pub(crate) fn validate_owned_data(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        symbols: &TopLevelSymbols<'_>,
        diagnostics: &mut Vec<Diagnostic>,
        formation_obligations: &mut Vec<checked_trees::IntegerRangeObligation>,
    ) {
        for owned_data in program.machine_owned_data(machine) {
            validate_type_reference_handle(
                program,
                owned_data.type_reference,
                symbols,
                diagnostics,
                TypeReferenceOwner::MachineOwnedData {
                    machine: machine.name.as_str(),
                    data: owned_data.name.as_str(),
                    generic_depth: 0,
                },
                TypeParameterScope::authored(
                    machine.symbol,
                    program.machine_type_parameters(machine),
                    &machine.lifetime_parameters,
                ),
                formation_obligations,
            );

            if owned_data.initial_value.is_valid() {
                validate_initial_value_handle(
                    program,
                    owned_data.type_reference,
                    owned_data.initial_value,
                    diagnostics,
                    InitialValueOwner::MachineOwnedData {
                        machine: machine.name.as_str(),
                        data: owned_data.name.as_str(),
                    },
                );
            }
        }
    }

    #[derive(Debug, Clone, Copy)]
    pub(crate) enum InitialValueOwner<'program> {
        MachineOwnedData {
            machine: &'program str,
            data: &'program str,
        },
    }

    impl fmt::Display for InitialValueOwner<'_> {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                Self::MachineOwnedData { machine, data } => {
                    write!(formatter, "machine `{machine}` owned data `{data}`")
                }
            }
        }
    }

    fn validate_initial_value_handle(
        program: &dyn crate::CheckingInput,
        type_reference: TypeReferenceHandle,
        initial_value: ExpressionHandle,
        diagnostics: &mut Vec<Diagnostic>,
        owner: InitialValueOwner<'_>,
    ) {
        if !argument_matches_type_reference_handle(program, initial_value, type_reference) {
            diagnostics.push(Diagnostic::error(format!(
                "{owner} initializer expects `{}`, got `{}`",
                program.display_type_reference_with_constraints(type_reference),
                expression_type_name_handle(program, initial_value)
            )));
        }
    }
}

pub mod machine_specialization_identity {
    pub fn machine_specialization_matches_template_identity(
        typed: &dyn crate::CheckingInput,
        specialization: &typed_trees::typed_trees::MachineSpecialization,
        machine_identity: &str,
        machine_package_identity: Option<semantic_vocabulary::PackageKeyIdentity>,
    ) -> bool {
        typed_trees::machine_specialization_matches_template_identity(
            typed.typed_program(),
            specialization,
            machine_identity,
            machine_package_identity,
        )
    }
}

pub mod result_overloads {
    pub(crate) type HashMap<K, V> = std::collections::HashMap<K, V, symbols::BuildSymbolHasher>;

    use crate::frozen_program::expression_table;
    use diagnostics::Diagnostic;
    use source::SourceSpan;
    use symbols::{SymbolHandle, SymbolKind};
    use typed_trees::TypedTrees;
    use typed_trees::expression::{
        ExpressionHandle, ExpressionNode, TableCallExpression, TableNamePath,
    };
    use typed_trees::statement::TransitionTargetHandle;
    use typed_trees::statement::TransitionTargetNode;
    use typed_trees::statement::{StatementNode, TableLocalData};
    use typed_trees::type_identity::{CallableIdentityId, CallableParametersId, CallablePathId};
    use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

    pub(crate) struct MachineOverloadGroup {
        provisional_identity: CallableIdentityId,
        overloads: Vec<(SymbolHandle, CallableIdentityId)>,
    }

    pub(crate) struct MachineOverloadIndex {
        entry_to_group: HashMap<u32, usize>,
        groups: Vec<MachineOverloadGroup>,
    }

    impl MachineOverloadIndex {
        fn new(program: &dyn crate::CheckingInput) -> Self {
            let mut entry_to_group = HashMap::default();
            let mut family_to_group: HashMap<(CallablePathId, CallableParametersId), usize> =
                HashMap::default();
            let mut groups: Vec<MachineOverloadGroup> = Vec::new();

            for machine in program.machines() {
                let Some(entry) = program.machine_states(machine).first() else {
                    continue;
                };
                let identity = program.machine_callable_identity(machine);
                if !identity.is_valid() {
                    continue;
                }
                let family = (
                    program.callable_identity_path(identity),
                    program.callable_identity_parameters(identity),
                );
                let group_index = if let Some(index) = family_to_group.get(&family).copied() {
                    index
                } else {
                    let index = groups.len();
                    family_to_group.insert(family, index);
                    groups.push(MachineOverloadGroup {
                        provisional_identity: identity,
                        overloads: Vec::new(),
                    });
                    index
                };
                groups[group_index].overloads.push((entry.symbol, identity));
                entry_to_group
                    .entry(entry.symbol.arena_index())
                    .or_insert(group_index);
            }

            Self {
                entry_to_group,
                groups,
            }
        }
    }

    #[derive(Clone, Copy)]
    pub(crate) enum CallOwner {
        State {
            machine: SymbolHandle,
            state: SymbolHandle,
            source_span: SourceSpan,
        },
        Declaration {
            symbol: SymbolHandle,
            source_span: SourceSpan,
        },
    }

    impl CallOwner {
        fn symbol(self) -> SymbolHandle {
            match self {
                Self::State { machine, .. } => machine,
                Self::Declaration { symbol, .. } => symbol,
            }
        }

        fn source_span(self) -> SourceSpan {
            match self {
                Self::State { source_span, .. } | Self::Declaration { source_span, .. } => {
                    source_span
                }
            }
        }

        fn operand_type(
            self,
            program: &dyn crate::CheckingInput,
            argument: ExpressionHandle,
        ) -> Option<TypeReferenceHandle> {
            match self {
                Self::State { machine, state, .. } => crate::expression_result_type_reference(
                    program,
                    program.machine(machine)?,
                    program.state_by_symbol(state)?,
                    argument,
                ),
                Self::Declaration { symbol, .. } => {
                    crate::parameter_expression_result_type_reference(
                        program,
                        symbol,
                        &[],
                        argument,
                    )
                }
            }
        }

        fn type_arguments(
            self,
            program: &dyn crate::CheckingInput,
            callee: &typed_trees::machine::Machine,
            entry: &typed_trees::state::State,
            operands: CallOperands<'_>,
        ) -> Option<Vec<(SymbolHandle, TypeReferenceHandle)>> {
            match self {
                Self::State { machine, state, .. } => crate::call_type_arguments(
                    program,
                    program.machine(machine)?,
                    Some(program.state_by_symbol(state)?),
                    callee,
                    entry,
                    operands.static_arguments,
                    operands.arguments,
                ),
                Self::Declaration { symbol, .. } => {
                    crate::machine_calls::calls::declaration_call_type_arguments(
                        program,
                        symbol,
                        callee,
                        entry,
                        operands.static_arguments,
                        operands.arguments,
                    )
                }
            }
        }
    }

    #[derive(Clone, Copy)]
    pub(crate) struct CallOperands<'call> {
        owner: Option<CallOwner>,
        static_arguments: &'call [typed_trees::expression::StaticMachineArgument],
        arguments: &'call [ExpressionHandle],
    }

    fn expression_owners(program: &dyn crate::CheckingInput) -> HashMap<u32, CallOwner> {
        let mut owners = HashMap::default();
        for machine in program.machines() {
            let Some(source_span) = program
                .symbols
                .symbol_provenance_source_span(machine.symbol)
            else {
                continue;
            };
            for (ordinal, state) in program.machine_states(machine).iter().enumerate() {
                let owner = CallOwner::State {
                    machine: machine.symbol,
                    state: state.symbol,
                    source_span,
                };
                let mut contract_roots = Vec::new();
                let machine_contracts = if ordinal == 0 {
                    program.machine_contracts(machine)
                } else {
                    &[]
                };
                for contract in machine_contracts
                    .iter()
                    .chain(program.state_contracts(state))
                {
                    crate::queries::expression_traversal::collect_contract_facts(
                        program,
                        contract.facts,
                        &mut contract_roots,
                    );
                }
                for root in contract_roots {
                    record_expression_owner(program, root, owner, &mut owners);
                }
                for statement in program.statement_table.statements(state.statement_nodes) {
                    for root in crate::machine_calls::calls::statement_value_expression_roots(
                        program, statement,
                    ) {
                        record_expression_owner(program, root, owner, &mut owners);
                    }
                }
            }
        }
        for declaration in program.const_declarations() {
            let Some(source_span) = program
                .symbols
                .symbol_provenance_source_span(declaration.symbol)
            else {
                continue;
            };
            let owner = CallOwner::Declaration {
                symbol: declaration.symbol,
                source_span,
            };
            for root in [
                declaration.authored_initializer,
                declaration.materialized_initializer,
            ] {
                if expression_table(program).expression_is_valid(root) {
                    record_expression_owner(program, root, owner, &mut owners);
                }
            }
        }
        owners
    }

    fn record_expression_owner(
        program: &dyn crate::CheckingInput,
        expression: ExpressionHandle,
        owner: CallOwner,
        owners: &mut HashMap<u32, CallOwner>,
    ) {
        if !expression.is_valid() || owners.contains_key(&expression.arena_index()) {
            return;
        }
        owners.insert(expression.arena_index(), owner);
        crate::queries::expression_traversal::children(
            program,
            expression_table(program).expression(expression),
            |child| record_expression_owner(program, child, owner, owners),
        );
    }

    pub fn resolve_named_result_overloads(
        program: &mut TypedTrees,
    ) -> Result<Vec<SymbolHandle>, Vec<Diagnostic>> {
        let boolean_equalities = crate::static_type_boolean_equalities(program);
        let case_memberships = crate::case_membership_type_facts(program);
        program.prepare_static_type_contexts(&boolean_equalities, case_memberships.clone());
        program.elide_unreachable_case_arms();
        let conditional = {
            let context = crate::CheckingContext::new(program);
            crate::proof_contracts::contract_entailment::predicate_case_type_contexts(&context)
        };
        program.prepare_static_type_contexts(
            &boolean_equalities,
            case_memberships.into_iter().chain(conditional).collect(),
        );
        let predicate_contexts = {
            let context = crate::CheckingContext::new(program);
            crate::proof_contracts::contract_entailment::predicate_type_contexts(&context)
        };
        program.retain_proven_type_contexts(predicate_contexts);
        let expected_calls: std::collections::HashMap<ExpressionHandle, TypeReferenceHandle> =
            collect_expected_expression_calls(program)
                .into_iter()
                .collect();
        let expression_owners = expression_owners(program);
        let machine_overloads = MachineOverloadIndex::new(program);
        let mut diagnostics = Vec::new();

        let expression_updates = expression_table(program)
            .expression_entries()
            .filter_map(|(handle, expression)| {
                let ExpressionNode::Call(call) = expression else {
                    return None;
                };
                let expected = expected_calls.get(&handle).copied();
                selected_named_expression_callable_symbol(
                    program,
                    &machine_overloads,
                    call,
                    expression_owners.get(&handle.arena_index()).copied(),
                    expression_owners
                        .get(&handle.arena_index())
                        .map(|owner| owner.source_span())
                        .unwrap_or_else(|| expression_table(program).source_span(handle)),
                    expected,
                    &mut diagnostics,
                )
                .map(|selected| (handle, selected))
            })
            .collect::<Vec<_>>();

        let mut statement_updates = Vec::new();
        let mut operator_statement_updates = Vec::new();
        for machine in program.machines() {
            for state in program.machine_states(machine) {
                for (index, statement) in program
                    .statement_table
                    .indexed_statements(state.statement_nodes)
                {
                    let Some(call) = statement.as_call() else {
                        continue;
                    };
                    if let Some(selected) = selected_named_statement_callable_symbol(
                        program,
                        &machine_overloads,
                        call,
                        Some(CallOwner::State {
                            machine: machine.symbol,
                            state: state.symbol,
                            source_span: call.source_span,
                        }),
                        program
                            .symbols
                            .symbol_source_span(machine.symbol)
                            .unwrap_or(call.source_span),
                        None,
                        &mut diagnostics,
                    ) {
                        if let Some(operator) = program.operator_definition_by_symbol(selected) {
                            if !operator.return_type.is_valid() {
                                statement_updates.push((
                                    machine.symbol,
                                    state.statement_nodes,
                                    index,
                                    selected,
                                ));
                                continue;
                            }
                            let returns_non_unit = !matches!(
                                program
                                    .type_reference_table
                                    .type_reference(operator.return_type),
                                TypeReferenceNode::Unit
                            );
                            if returns_non_unit && !call.discards_result {
                                let path = program.operator_path_string(operator.name);
                                diagnostics.push(Diagnostic::error(format!(
                                "call to named requirement `{path}` discards its non-unit `{}` result; consume the value or discard it explicitly with `_ = {path}(...);`",
                                program.display_type_reference_with_constraints(operator.return_type)
                            )));
                            } else {
                                operator_statement_updates.push((
                                    state.symbol,
                                    state.statement_nodes,
                                    index,
                                    call.clone(),
                                    selected,
                                    returns_non_unit,
                                ));
                            }
                        } else {
                            statement_updates.push((
                                machine.symbol,
                                state.statement_nodes,
                                index,
                                selected,
                            ));
                        }
                    }
                }
            }
        }

        let transition_updates = selected_transition_call_symbols(program, &mut diagnostics);

        if !diagnostics.is_empty() {
            return Err(diagnostics);
        }
        let mut changed_owners = Vec::new();
        for (target, selected, owner) in transition_updates {
            let TransitionTargetNode::Named { path, .. } =
                program.statement_table.transition_target_mut(target)
            else {
                unreachable!("collected transition target changed shape");
            };
            if path.symbol != selected {
                path.symbol = selected;
                changed_owners.push(owner);
            }
        }
        for (handle, selected) in expression_updates {
            let ExpressionNode::Call(call) = program.expression_table.expression_mut(handle) else {
                unreachable!("collected expression call changed shape");
            };
            if call.target_symbol != selected {
                call.target_symbol = selected;
                if let Some(owner) = expression_owners.get(&handle.arena_index()) {
                    changed_owners.push(owner.symbol());
                }
            }
            let application = call.authored_machine_application;
            if application.is_valid() {
                let retained = program
                    .expression_table
                    .static_machine_application_mut(application);
                if !retained.registration_operation.is_valid() {
                    retained.registration_operation = selected;
                    if let Some(owner) = expression_owners.get(&handle.arena_index()) {
                        changed_owners.push(owner.symbol());
                    }
                }
            }
        }
        for (owner, statements, index, selected) in statement_updates {
            let StatementNode::Call(call) =
                &mut program.statement_table.statements_mut(statements)[index]
            else {
                unreachable!("collected statement call changed shape");
            };
            if call.target_symbol != selected {
                call.target_symbol = selected;
                changed_owners.push(owner);
            }
            let application = call.authored_machine_application;
            if application.is_valid() {
                let retained = program
                    .expression_table
                    .static_machine_application_mut(application);
                if !retained.registration_operation.is_valid() {
                    retained.registration_operation = selected;
                    changed_owners.push(owner);
                }
            }
        }
        for (state_symbol, statements, index, call, selected, returns_value) in
            operator_statement_updates
        {
            if let Some(owner) = program.machine_holding_state(state_symbol) {
                changed_owners.push(owner.symbol);
            }
            let source_span = call.source_span;
            let authored_call_selection = call.authored_call_selection;
            let receiver_members = program
                .statement_table
                .name_path_members(call.receiver)
                .to_vec();
            let arguments = program
                .statement_table
                .expression_handles(call.arguments)
                .to_vec();
            let receiver = if receiver_members.is_empty() {
                ExpressionHandle::invalid()
            } else {
                let mut members = arena::HandleSpan::empty();
                for member in receiver_members {
                    program
                        .expression_table
                        .push_name_path_member(&mut members, member);
                }
                program
                    .expression_table
                    .insert(ExpressionNode::Name(TableNamePath {
                        members,
                        member_symbols: arena::HandleSpan::empty(),
                        head_symbol: call.receiver_root_symbol,
                        symbol: call.receiver_symbol,
                        reserved_result_of: SymbolHandle::invalid(),
                    }))
            };
            let arguments = program
                .expression_table
                .insert_expression_handles(arguments);
            let expression =
                program
                    .expression_table
                    .insert(ExpressionNode::Call(TableCallExpression {
                        authored_call_selection,
                        authored_call_binding: None,
                        authored_machine_application: call.authored_machine_application,
                        requirement_application: Default::default(),
                        exact_coercion_target: typed_trees::types::TypeReferenceHandle::invalid(),
                        receiver,
                        target_symbol: selected,
                        target: call.target,
                        static_machine_parameter: call.static_machine_parameter,
                        static_requirement_dispatch: None,
                        machine_arguments: call.machine_arguments,
                        quotient_operation: None,
                        private_layout_operation: None,
                        installed_interrupt_operation: None,
                        layout_query: None,
                        native_code_layout: None,
                        build_selection: Default::default(),
                        arguments,
                        evidence_arguments: call.evidence_arguments,
                        operational_acknowledgement: call.operational_acknowledgement,
                    }));
            program
                .expression_table
                .set_source_span(expression, source_span);
            if let Some(occurrence) = authored_call_selection {
                program
                    .expression_table
                    .attach_authored_selection_occurrences(expression, [occurrence]);
            }
            if !returns_value {
                program.statement_table.statements_mut(statements)[index] =
                    StatementNode::Expression(expression);
                continue;
            }
            let type_reference = program
                .operator_definition_by_symbol(selected)
                .expect("selected statement operator disappeared")
                .return_type;
            let generated_name = format!(
                "__discarded_named_requirement#{}#{index}",
                statements.start().arena_index()
            );
            let symbol = program.symbols.insert_generated_root_from(
                state_symbol,
                SymbolKind::Local,
                &generated_name,
            );
            program.statement_table.statements_mut(statements)[index] =
                StatementNode::LocalData(TableLocalData {
                    role: language_core::LocalBindingRole::Value,
                    symbol,
                    name: typed_trees::name::Identifier::generated(generated_name),
                    type_reference,
                    initial_value: expression,
                    is_mutable: false,
                    type_is_inferred: true,
                    is_operand_capture: false,
                    relevance: language_core::BindingRelevance::Relevant,
                    reflected_query_origin: None,
                    ..Default::default()
                });
        }
        changed_owners.sort_unstable_by_key(|owner| (owner.arena_index(), owner.generation()));
        changed_owners.dedup();
        Ok(changed_owners)
    }

    fn selected_transition_call_symbols(
        program: &dyn crate::CheckingInput,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Vec<(TransitionTargetHandle, SymbolHandle, SymbolHandle)> {
        let mut updates = Vec::new();
        for machine in program.machines() {
            for state in program.machine_states(machine) {
                for statement in program.statement_table.statements(state.statement_nodes) {
                    let Some(transition) = statement.as_transition() else {
                        continue;
                    };
                    for target in transition.targets() {
                        let Some((name, source_span)) =
                            typed_trees::operator::named_transition_family_path(program, target)
                        else {
                            continue;
                        };
                        let TransitionTargetNode::Named { arguments, .. } =
                            program.statement_table.transition_target(target)
                        else {
                            continue;
                        };
                        let source_span = &source_span;
                        let Some(entry) = named_machine_overload_family_symbol(
                            program,
                            name,
                            CallOperands {
                                owner: Some(CallOwner::State {
                                    machine: machine.symbol,
                                    state: state.symbol,
                                    source_span: *source_span,
                                }),
                                static_arguments: &[],
                                arguments: program.statement_table.expression_handles(*arguments),
                            },
                            *source_span,
                            state.return_type.is_valid().then_some(state.return_type),
                            diagnostics,
                        ) else {
                            continue;
                        };
                        if let Some(owner) = program.machine_holding_state(entry) {
                            updates.push((target, owner.symbol, machine.symbol));
                        }
                    }
                }
            }
        }
        updates
    }

    fn family_boundary_overloads<'program>(
        program: &'program dyn crate::CheckingInput,
        candidates: impl Iterator<Item = &'program typed_trees::operator::OperatorDefinition>,
        source_span: SourceSpan,
        provisional_identity: CallableIdentityId,
    ) -> Vec<(SymbolHandle, CallableIdentityId)> {
        candidates
            .filter(|operator| operator.is_boundary && operator.spelling.is_none())
            .filter(|operator| {
                program
                    .symbols
                    .source_reference_can_see_symbol(source_span, operator.symbol)
            })
            .filter_map(|operator| {
                let identity = program.operator_callable_identity(operator);
                program
                    .callable_identities_share_family(identity, provisional_identity)
                    .then_some((operator.symbol, identity))
            })
            .collect()
    }

    fn visible_boundary_overload_family<'program>(
        program: &'program dyn crate::CheckingInput,
        candidates: &[&'program typed_trees::operator::OperatorDefinition],
        source_span: SourceSpan,
        provisional: &'program typed_trees::operator::OperatorDefinition,
    ) -> (Vec<(SymbolHandle, CallableIdentityId)>, usize) {
        let provisional_identity = program.operator_callable_identity(provisional);
        let visible_candidate_count = candidates
            .iter()
            .filter(|operator| {
                program
                    .symbols
                    .source_reference_can_see_symbol(source_span, operator.symbol)
            })
            .count();
        let overloads = family_boundary_overloads(
            program,
            candidates.iter().copied(),
            source_span,
            provisional_identity,
        );
        (overloads, visible_candidate_count)
    }

    fn selected_named_statement_callable_symbol(
        program: &dyn crate::CheckingInput,
        machine_overloads: &MachineOverloadIndex,
        call: &typed_trees::statement::TableCall,
        owner: Option<CallOwner>,
        source_span: SourceSpan,
        expected_result: Option<TypeReferenceHandle>,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Option<SymbolHandle> {
        let path = typed_trees::operator::named_statement_call_family_path(program, call);
        if let Some(resolution) = named_callable_resolution(
            program,
            machine_overloads,
            call.target_symbol,
            path.as_deref(),
            CallOperands {
                owner,
                static_arguments: &call.machine_arguments,
                arguments: program.statement_table.expression_handles(call.arguments),
            },
            source_span,
            expected_result,
            diagnostics,
        ) {
            return resolution;
        }
        if let Some(provisional) = program.operators().iter().find(|operator| {
            operator.symbol == call.target_symbol
                && operator.is_boundary
                && operator.spelling.is_none()
        }) {
            let provisional_identity = program.operator_callable_identity(provisional);
            let overloads = family_boundary_overloads(
                program,
                program.operators().iter(),
                source_span,
                provisional_identity,
            );
            if overloads.len() == 1 {
                return Some(provisional.symbol);
            }
            return select_overload_symbol(
                program,
                provisional_identity,
                &overloads,
                expected_result,
                diagnostics,
                "requirement",
            );
        }

        let candidates = typed_trees::operator::named_statement_call_candidates(program, call);
        if let Some(provisional) = candidates.first() {
            let (overloads, visible_candidate_count) =
                visible_boundary_overload_family(program, &candidates, source_span, provisional);
            if overloads.len() == visible_candidate_count {
                return select_unbound_overload_family_symbol(
                    program,
                    &overloads,
                    expected_result,
                    diagnostics,
                    "requirement",
                );
            }
        }

        selected_named_callable_symbol(
            program,
            machine_overloads,
            call.target_symbol,
            source_span,
            expected_result,
            diagnostics,
        )
    }

    fn selected_named_expression_callable_symbol(
        program: &dyn crate::CheckingInput,
        machine_overloads: &MachineOverloadIndex,
        call: &TableCallExpression,
        owner: Option<CallOwner>,
        source_span: SourceSpan,
        expected_result: Option<TypeReferenceHandle>,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Option<SymbolHandle> {
        let path = typed_trees::operator::named_expression_call_family_path(program, call);
        if let Some(resolution) = named_callable_resolution(
            program,
            machine_overloads,
            call.target_symbol,
            path.as_deref(),
            CallOperands {
                owner,
                static_arguments: &call.machine_arguments,
                arguments: expression_table(program).expression_handles(call.arguments),
            },
            source_span,
            expected_result,
            diagnostics,
        ) {
            return resolution;
        }
        if program.operators().iter().any(|operator| {
            operator.symbol == call.target_symbol
                && operator.is_boundary
                && operator.spelling.is_none()
        }) {
            return selected_named_callable_symbol(
                program,
                machine_overloads,
                call.target_symbol,
                source_span,
                expected_result,
                diagnostics,
            );
        }

        let candidates = typed_trees::operator::named_expression_call_candidates(program, call);
        let Some(provisional) = candidates.first() else {
            return selected_named_callable_symbol(
                program,
                machine_overloads,
                call.target_symbol,
                source_span,
                expected_result,
                diagnostics,
            );
        };
        let (overloads, visible_candidate_count) =
            visible_boundary_overload_family(program, &candidates, source_span, provisional);
        if overloads.len() != visible_candidate_count {
            return selected_named_callable_symbol(
                program,
                machine_overloads,
                call.target_symbol,
                source_span,
                expected_result,
                diagnostics,
            );
        }
        select_unbound_overload_family_symbol(
            program,
            &overloads,
            expected_result,
            diagnostics,
            "requirement",
        )
    }

    fn named_callable_resolution(
        program: &dyn crate::CheckingInput,
        machine_overloads: &MachineOverloadIndex,
        target_symbol: SymbolHandle,
        path: Option<&str>,
        operands: CallOperands<'_>,
        source_span: SourceSpan,
        expected_result: Option<TypeReferenceHandle>,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Option<Option<SymbolHandle>> {
        if let Some(path) = path {
            let candidates =
                typed_trees::operator::named_call_family_members(program, path, source_span);
            let provisional_parameters = candidates.iter().find_map(|symbol| {
                let machine = program.machine(*symbol)?;
                (program.machine_states(machine).first()?.symbol == target_symbol).then(|| {
                    program.callable_identity_parameters(program.machine_callable_identity(machine))
                })
            });
            let needs_parameter_selection = provisional_parameters.is_some_and(|parameters| {
                candidates.iter().any(|symbol| {
                    program.machine(*symbol).is_some_and(|machine| {
                        program.callable_identity_parameters(
                            program.machine_callable_identity(machine),
                        ) != parameters
                    })
                })
            });
            if !candidates.is_empty() && (!target_symbol.is_valid() || needs_parameter_selection) {
                return Some(named_machine_overload_family_symbol(
                    program,
                    path,
                    operands,
                    source_span,
                    expected_result,
                    diagnostics,
                ));
            }
        }
        if provisional_target_is_named_non_operator_callable(
            program,
            machine_overloads,
            target_symbol,
        ) {
            return Some(selected_named_callable_symbol(
                program,
                machine_overloads,
                target_symbol,
                source_span,
                expected_result,
                diagnostics,
            ));
        }
        None
    }

    fn provisional_target_is_named_non_operator_callable(
        program: &dyn crate::CheckingInput,
        machine_overloads: &MachineOverloadIndex,
        target: SymbolHandle,
    ) -> bool {
        machine_overloads
            .entry_to_group
            .contains_key(&target.arena_index())
            || program.traits().iter().any(|trait_definition| {
                program
                    .trait_machine_signatures(trait_definition)
                    .iter()
                    .any(|requirement| requirement.symbol == target)
            })
    }

    fn selected_named_callable_symbol(
        program: &dyn crate::CheckingInput,
        machine_overloads: &MachineOverloadIndex,
        provisional_target: SymbolHandle,
        source_span: SourceSpan,
        expected_result: Option<TypeReferenceHandle>,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Option<SymbolHandle> {
        if let Some(group) = machine_overloads
            .entry_to_group
            .get(&provisional_target.arena_index())
            .and_then(|index| machine_overloads.groups.get(*index))
        {
            let visible_overloads = group
                .overloads
                .iter()
                .filter(|(symbol, _)| {
                    program
                        .symbols
                        .source_reference_can_see_symbol(source_span, *symbol)
                })
                .cloned()
                .collect::<Vec<_>>();
            return select_overload_symbol(
                program,
                group.provisional_identity,
                &visible_overloads,
                expected_result,
                diagnostics,
                "machine",
            );
        }

        if let Some(provisional_operator) = program.operators().iter().find(|operator| {
            operator.symbol == provisional_target
                && operator.is_boundary
                && operator.spelling.is_none()
        }) {
            let provisional_identity = program.operator_callable_identity(provisional_operator);
            let overloads = family_boundary_overloads(
                program,
                program.operators().iter(),
                source_span,
                provisional_identity,
            );
            return select_overload_symbol(
                program,
                provisional_identity,
                &overloads,
                expected_result,
                diagnostics,
                "requirement",
            );
        }

        let (trait_definition, provisional_requirement) =
            program.trait_machine_signature(provisional_target)?;
        let provisional_identity =
            program.trait_requirement_callable_identity(trait_definition, provisional_requirement);
        let overloads = program
            .trait_machine_signatures(trait_definition)
            .iter()
            .filter(|requirement| {
                program
                    .symbols
                    .source_reference_can_see_symbol(source_span, requirement.symbol)
            })
            .filter_map(|requirement| {
                let identity =
                    program.trait_requirement_callable_identity(trait_definition, requirement);
                program
                    .callable_identities_share_family(identity, provisional_identity)
                    .then_some((requirement.symbol, identity))
            })
            .collect::<Vec<_>>();
        select_overload_symbol(
            program,
            provisional_identity,
            &overloads,
            expected_result,
            diagnostics,
            "requirement",
        )
    }

    fn select_overload_symbol(
        program: &dyn crate::CheckingInput,
        provisional_identity: CallableIdentityId,
        overloads: &[(SymbolHandle, CallableIdentityId)],
        expected_result: Option<TypeReferenceHandle>,
        diagnostics: &mut Vec<Diagnostic>,
        kind: &str,
    ) -> Option<SymbolHandle> {
        if overloads.len() <= 1 {
            return None;
        }

        let requested = program.callable_result_dispatch_identity(
            &expected_result
                .filter(|expected| expected.is_valid())
                .map(|expected| program.normalized_result_dispatch_set(expected))
                .unwrap_or_default(),
        );
        let matching = overloads
            .iter()
            .filter(|(_, identity)| {
                program.callable_identity_result_dispatch(*identity) == requested
            })
            .collect::<Vec<_>>();
        match matching.as_slice() {
            [(symbol, _)] => Some(*symbol),
            [] => {
                let requested = if program.callable_result_dispatch_is_empty(requested) {
                    "<empty>".to_owned()
                } else {
                    program.callable_result_dispatch_text(requested)
                };
                diagnostics.push(Diagnostic::error(format!(
                "named {kind} call `{}` has no overload for result dispatch set `{requested}` and parameter signature `{}`",
                program.callable_path_text(program.callable_identity_path(provisional_identity)),
                program
                    .callable_parameters_text(program.callable_identity_parameters(provisional_identity)),
            )));
                None
            }
            _ => None,
        }
    }

    fn named_machine_overload_family_symbol(
        program: &dyn crate::CheckingInput,
        target_name: &str,
        operands: CallOperands<'_>,
        source_span: SourceSpan,
        expected_result: Option<TypeReferenceHandle>,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Option<SymbolHandle> {
        let mut overloads =
            typed_trees::operator::named_call_family_members(program, target_name, source_span)
                .into_iter()
                .filter_map(|symbol| {
                    let machine = program.machine(symbol)?;
                    let identity = program.machine_callable_identity(machine);
                    let callable = program.machine_states(machine).first()?;
                    Some((callable.symbol, identity))
                })
                .collect::<Vec<_>>();
        if overloads.first().is_some_and(|(_, first)| {
            overloads.iter().any(|(_, candidate)| {
                program.callable_identity_path(*candidate) != program.callable_identity_path(*first)
            })
        }) {
            diagnostics.push(Diagnostic::error(format!(
                "named call `{target_name}` is ambiguous between declaration namespaces"
            )));
            return None;
        }
        let distinct_parameters = overloads.first().is_some_and(|(_, first)| {
            overloads.iter().any(|(_, candidate)| {
                program.callable_identity_parameters(*candidate)
                    != program.callable_identity_parameters(*first)
            })
        });
        if distinct_parameters {
            let owner = operands.owner?;
            overloads.retain(|(target, _)| {
            let Some((callee, entry)) = crate::find_state(program, *target) else {
                return false;
            };
            let parameters = program.state_parameters(entry);
            if parameters.len() != operands.arguments.len() {
                return false;
            }
            let Some(substitutions) = owner.type_arguments(program, callee, entry, operands) else {
                return false;
            };
            parameters
                .iter()
                .zip(operands.arguments)
                .all(|(parameter, argument)| {
                    match owner.operand_type(program, *argument) {
                        Some(actual) => {
                            crate::value_custody::expression_types::completed_call_operand_matches(
                                program,
                                actual,
                                parameter.type_reference,
                                &substitutions,
                            )
                        }
                        None => {
                            crate::value_custody::literals::has_anonymous_numeric_results(program, *argument)
                                && crate::value_custody::expression_types::argument_matches_call_carrier(
                                program,
                                owner.symbol(),
                                *argument,
                                parameter.type_reference,
                                &substitutions,
                            )
                        }
                    }
                })
        });
            if overloads.first().is_some_and(|(_, first)| {
                overloads.iter().any(|(_, candidate)| {
                    program.callable_identity_parameters(*candidate)
                        != program.callable_identity_parameters(*first)
                })
            }) {
                return None;
            }
        }
        select_unbound_overload_family_symbol(
            program,
            &overloads,
            expected_result,
            diagnostics,
            "machine",
        )
    }

    fn select_unbound_overload_family_symbol(
        program: &dyn crate::CheckingInput,
        overloads: &[(SymbolHandle, CallableIdentityId)],
        expected_result: Option<TypeReferenceHandle>,
        diagnostics: &mut Vec<Diagnostic>,
        kind: &str,
    ) -> Option<SymbolHandle> {
        match overloads {
            [] => None,
            [(symbol, _)] => Some(*symbol),
            [(_, provisional_identity), ..] => select_overload_symbol(
                program,
                *provisional_identity,
                overloads,
                expected_result,
                diagnostics,
                kind,
            ),
        }
    }

    pub fn collect_expected_expression_calls(
        program: &dyn crate::CheckingInput,
    ) -> Vec<(ExpressionHandle, TypeReferenceHandle)> {
        let mut expected = Vec::new();

        for (_, expression) in expression_table(program).expression_entries() {
            match expression {
                ExpressionNode::StructLiteral(literal) => {
                    let Some(data_definition) =
                        program.data_definitions().iter().find(|definition| {
                            definition.symbol == literal.type_symbol
                                || (!program.symbols.has_source_metadata()
                                    && definition.name.as_str() == literal.type_name.as_str())
                        })
                    else {
                        continue;
                    };
                    for field in expression_table(program).struct_fields(literal.fields) {
                        let Some(field_type) =
                            crate::value_custody::struct_literals::construction_field_type(
                                program,
                                data_definition,
                                literal.case_name.as_ref().map(|name| name.as_str()),
                                field.name.as_str(),
                            )
                        else {
                            continue;
                        };
                        push_expected_call(program, field.value, field_type, &mut expected);
                    }
                }
                ExpressionNode::Call(call) => {
                    collect_call_argument_expectations(
                        program,
                        call.target_symbol,
                        expression_table(program).expression_handles(call.arguments),
                        &mut expected,
                    );
                }
                _ => {}
            }
        }

        for machine in program.machines() {
            for state in program.machine_states(machine) {
                let statements = program.statement_table.statements(state.statement_nodes);
                for (index, statement) in statements.iter().enumerate() {
                    match statement {
                        StatementNode::Assignment(assignment) => {
                            if let Some(declared) =
                                crate::value_custody::places::declared_place_type_raw(
                                    program,
                                    machine,
                                    Some(state),
                                    assignment.target,
                                )
                            {
                                push_expected_call(
                                    program,
                                    assignment.value,
                                    declared,
                                    &mut expected,
                                );
                            }
                        }
                        StatementNode::LocalData(local)
                            if local.initial_value.is_valid()
                                && local.type_reference.is_valid()
                                && !local.type_is_inferred =>
                        {
                            push_expected_call(
                                program,
                                local.initial_value,
                                local.type_reference,
                                &mut expected,
                            );
                        }
                        StatementNode::Call(call) => {
                            collect_call_argument_expectations(
                                program,
                                call.target_symbol,
                                program.statement_table.expression_handles(call.arguments),
                                &mut expected,
                            );
                        }
                        StatementNode::Transition(transition) => {
                            for target in transition.targets() {
                                match program.statement_table.transition_target(target) {
                                    TransitionTargetNode::Named {
                                        path, arguments, ..
                                    } => collect_call_argument_expectations(
                                        program,
                                        path.symbol,
                                        program.statement_table.expression_handles(*arguments),
                                        &mut expected,
                                    ),
                                    TransitionTargetNode::Value(value)
                                        if state.return_type.is_valid() =>
                                    {
                                        push_expected_call(
                                            program,
                                            *value,
                                            state.return_type,
                                            &mut expected,
                                        );
                                    }
                                    _ => {}
                                }
                            }
                        }
                        StatementNode::Expression(value)
                            if index + 1 == statements.len() && state.return_type.is_valid() =>
                        {
                            push_expected_call(program, *value, state.return_type, &mut expected);
                        }
                        StatementNode::Assembly(_)
                        | StatementNode::AssemblyFact(_)
                        | StatementNode::Expression(_)
                        | StatementNode::LocalData(_) => {}
                    }
                }
            }
        }
        expected
    }

    fn collect_call_argument_expectations(
        program: &dyn crate::CheckingInput,
        target: SymbolHandle,
        arguments: &[ExpressionHandle],
        expected: &mut Vec<(ExpressionHandle, TypeReferenceHandle)>,
    ) {
        let Some(parameters) = crate::call_target_parameters(program, target) else {
            return;
        };
        for (parameter, argument) in parameters.iter().zip(arguments) {
            push_expected_call(program, *argument, parameter.type_reference, expected);
        }
    }

    fn push_expected_call(
        program: &dyn crate::CheckingInput,
        value: ExpressionHandle,
        expected_type: TypeReferenceHandle,
        expected: &mut Vec<(ExpressionHandle, TypeReferenceHandle)>,
    ) {
        match expression_table(program).expression(value) {
            ExpressionNode::Match(dispatch) => {
                for arm in expression_table(program).match_arms(dispatch.arms) {
                    push_expected_call(program, arm.value, expected_type, expected);
                }
            }
            ExpressionNode::Borrow(inner) => {
                let reference = crate::peel_declared_constraints(program, expected_type);
                if let TypeReferenceNode::Reference { referee, .. } =
                    program.type_reference_table.type_reference(reference)
                {
                    push_expected_call(program, inner.target, *referee, expected);
                }
            }
            ExpressionNode::Atomic(atomic) => {
                if crate::value_custody::atomic_operations::atomic_once_operands(program, atomic)
                    .is_none()
                {
                    push_expected_call(program, atomic.value, expected_type, expected);
                }
            }
            ExpressionNode::Call(_) => {
                if let Some((_, existing)) = expected
                    .iter_mut()
                    .find(|(candidate, _)| *candidate == value)
                {
                    *existing = expected_type;
                } else {
                    expected.push((value, expected_type));
                }
            }
            ExpressionNode::ArrayLiteral(values) => {
                let Some(element_type) = array_element_type(program, expected_type) else {
                    return;
                };
                for element in expression_table(program)
                    .expression_handles(*values)
                    .iter()
                    .copied()
                {
                    push_expected_call(program, element, element_type, expected);
                }
            }
            ExpressionNode::Binary(_)
            | ExpressionNode::Boolean(_)
            | ExpressionNode::Cast(_)
            | ExpressionNode::Float(_)
            | ExpressionNode::UndeterminedNaN(_)
            | ExpressionNode::Indexed(_)
            | ExpressionNode::Integer(_)
            | ExpressionNode::Member(_)
            | ExpressionNode::Name(_)
            | ExpressionNode::SavedSubject(_)
            | ExpressionNode::Range(_)
            | ExpressionNode::StructLiteral(_)
            | ExpressionNode::String(_)
            | ExpressionNode::Unary(_)
            | ExpressionNode::TypeExpression(_)
            | ExpressionNode::ZeroValue(_) => {}
        }
    }

    fn array_element_type(
        program: &dyn crate::CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> Option<TypeReferenceHandle> {
        match program.type_reference_table.type_reference(type_reference) {
            typed_trees::types::TypeReferenceNode::Reference { referee, .. }
            | typed_trees::types::TypeReferenceNode::Constrained {
                base_type: referee, ..
            } => array_element_type(program, *referee),
            typed_trees::types::TypeReferenceNode::FixedArray { element_type, .. }
            | typed_trees::types::TypeReferenceNode::Slice { element_type } => Some(*element_type),
            typed_trees::types::TypeReferenceNode::Generic { .. }
            | typed_trees::types::TypeReferenceNode::DynamicTrait { .. }
            | typed_trees::types::TypeReferenceNode::Named { .. }
            | typed_trees::types::TypeReferenceNode::ConstExpression(_)
            | typed_trees::types::TypeReferenceNode::Unit => None,
        }
    }

    pub(crate) mod float_conversion_casts {
        use crate::CheckingInput;
        use diagnostics::Diagnostic;
        use language_semantics::CallOperationalAcknowledgement;
        use language_semantics::declaration_selection::{
            AuthoredDeclarationSelectionExposure, AuthoredDeclarationSelectionKind,
        };
        use symbols::SymbolHandle;
        use typed_trees::TypedTrees;
        use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCallExpression};
        use typed_trees::machine::Machine;
        use typed_trees::name::Identifier;
        use typed_trees::state::State;
        use typed_trees::types::PrimitiveType;

        pub(crate) struct ConversionCastBinding {
            exact_coercion_target: typed_trees::types::TypeReferenceHandle,
            cast: ExpressionHandle,
            operand: ExpressionHandle,
            requirement: SymbolHandle,
            requirement_member: Identifier,
        }

        pub fn bind_float_conversion_casts(
            program: &mut TypedTrees,
        ) -> Result<usize, Vec<Diagnostic>> {
            let mut diagnostics = Vec::new();
            let bindings = collect_conversion_cast_bindings(program);
            for binding in &bindings {
                let source_span = program.expression_table.source_span(binding.cast);
                let occurrence = match program.record_resolved_authored_declaration_selection_once(
                    source_span,
                    AuthoredDeclarationSelectionExposure::PrivateImplementation,
                    AuthoredDeclarationSelectionKind::Call,
                    binding.requirement,
                ) {
                    Ok(occurrence) => occurrence,
                    Err(error) => {
                        diagnostics.push(
                    Diagnostic::error(format!(
                        "numeric cast cannot record its selected float conversion: {error:?}"
                    ))
                    .with_source_span(source_span),
                );
                        continue;
                    }
                };
                let arguments = program
                    .expression_table
                    .insert_expression_handles([binding.operand]);
                let conversion = program.expression_table.insert(ExpressionNode::Call(
                    TableCallExpression {
                        authored_call_selection: None,
                        authored_call_binding: None,
                        authored_machine_application: Default::default(),
                        requirement_application: Default::default(),
                        exact_coercion_target: binding.exact_coercion_target,
                        receiver: ExpressionHandle::invalid(),
                        target_symbol: binding.requirement,
                        static_machine_parameter: SymbolHandle::invalid(),
                        target: binding.requirement_member.clone(),
                        static_requirement_dispatch: None,
                        machine_arguments: Box::default(),
                        quotient_operation: None,
                        private_layout_operation: None,
                        installed_interrupt_operation: None,
                        layout_query: None,
                        native_code_layout: None,
                        build_selection: Default::default(),
                        arguments,
                        evidence_arguments: Box::default(),
                        operational_acknowledgement: CallOperationalAcknowledgement {
                            origin:
                                language_semantics::CallOperationalAcknowledgementOrigin::Source,
                            ..Default::default()
                        },
                    },
                ));
                program
                    .expression_table
                    .set_source_span(conversion, source_span);
                program
                    .expression_table
                    .attach_authored_selection_occurrences(conversion, [occurrence]);
                let ExpressionNode::Cast(cast) =
                    program.expression_table.expression_mut(binding.cast)
                else {
                    diagnostics.push(
                        Diagnostic::error("numeric conversion binding lost its authored cast")
                            .with_source_span(source_span),
                    );
                    continue;
                };
                cast.value = conversion;
            }
            if diagnostics.is_empty() {
                Ok(bindings.len())
            } else {
                Err(diagnostics)
            }
        }

        fn collect_conversion_cast_bindings(
            program: &dyn CheckingInput,
        ) -> Vec<ConversionCastBinding> {
            let mut bindings = Vec::new();
            let mut visited = Vec::new();
            for machine in program.machines() {
                for state in program.machine_states(machine) {
                    for statement in program.statement_table.statements(state.statement_nodes) {
                        for root in crate::machine_calls::calls::statement_value_expression_roots(
                            program, statement,
                        ) {
                            collect_expression_bindings(
                                program,
                                machine,
                                state,
                                root,
                                &mut visited,
                                &mut bindings,
                            );
                        }
                    }
                }
            }
            bindings
        }

        fn collect_expression_bindings(
            program: &dyn CheckingInput,
            machine: &Machine,
            state: &State,
            expression: ExpressionHandle,
            visited: &mut Vec<bool>,
            bindings: &mut Vec<ConversionCastBinding>,
        ) {
            if !expression.is_valid() {
                return;
            }
            let index = expression.arena_index() as usize;
            if visited.len() <= index {
                visited.resize(index + 1, false);
            }
            if std::mem::replace(&mut visited[index], true) {
                return;
            }
            let node = program.expression_table.expression(expression);
            if let ExpressionNode::Cast(cast) = node
                && let Some((requirement, requirement_member)) =
                    selected_conversion(program, machine, state, expression, cast)
            {
                bindings.push(ConversionCastBinding {
                    exact_coercion_target: cast.target_type,
                    cast: expression,
                    operand: cast.value,
                    requirement,
                    requirement_member,
                });
            }
            crate::queries::expression_traversal::children(program, node, |child| {
                collect_expression_bindings(program, machine, state, child, visited, bindings)
            });
        }

        fn selected_conversion(
            program: &dyn CheckingInput,
            machine: &Machine,
            state: &State,
            expression: ExpressionHandle,
            cast: &typed_trees::expression::TableCastExpression,
        ) -> Option<(SymbolHandle, Identifier)> {
            if cast.form.is_recast() || !cast.result_type.is_valid() {
                return None;
            }
            let destination = program.primitive_type_reference(cast.target_type)?;
            let destination_owner = destination_conversion_owner(destination)?;
            let source = crate::value_custody::expression_types::expression_result_type_reference(
                program, machine, state, cast.value,
            )
            .and_then(|reference| crate::unwrapped_type_reference(program, reference))
            .and_then(|reference| program.primitive_type_reference(reference))?;
            let source_is_float = matches!(source, PrimitiveType::F32 | PrimitiveType::F64);
            let destination_is_float =
                matches!(destination, PrimitiveType::F32 | PrimitiveType::F64);
            let supported = if destination_is_float {
                (source_is_float && source != destination)
                    || numerics::float_semantics_catalog::IntegerCarrier::from_spelling(
                        source.name(),
                    )
                    .is_some()
            } else {
                source_is_float
            };
            if !supported {
                return None;
            }
            let source_span = program.expression_table.source_span(expression);
            if source_span.span.start >= source_span.span.end {
                return None;
            }
            let requested = program.callable_result_dispatch_identity(&Default::default());
            let requirement_member = format!("from_{}", source.name());
            let mut selected = program.operators().iter().filter(|operator| {
                let path = program.operator_path_members(operator.name);
                let [owner, member] = path else {
                    return false;
                };
                let [parameter] = program.operator_parameters(operator) else {
                    return false;
                };
                operator.is_boundary
                    && operator.spelling.is_none()
                    && program.operator_type_parameters(operator).is_empty()
                    && owner.as_str() == destination_owner
                    && member.as_str() == requirement_member
                    && !parameter.is_mutable
                    && !parameter.is_const
                    && program.primitive_type_reference(parameter.type_reference) == Some(source)
                    && crate::unwrapped_type_reference(program, operator.return_type)
                        .and_then(|reference| program.primitive_type_reference(reference))
                        == Some(destination)
                    && program.callable_identity_result_dispatch(
                        program.operator_callable_identity(operator),
                    ) == requested
                    && program
                        .symbols
                        .source_reference_may_select_symbol(source_span, operator.symbol)
            });
            let operator = selected.next()?;
            if selected.next().is_some() {
                return None;
            }
            if destination_is_float
                && crate::numeric_float_conversion_contract(program, operator)
                    != Some((source, destination))
            {
                return None;
            }
            if source_is_float
                && !destination_is_float
                && crate::float_integer_conversion_contract(program, operator).is_none()
            {
                return None;
            }
            Some((operator.symbol, Identifier::generated(requirement_member)))
        }

        fn destination_conversion_owner(destination: PrimitiveType) -> Option<&'static str> {
            Some(match destination {
                PrimitiveType::I8 => "I8",
                PrimitiveType::I16 => "I16",
                PrimitiveType::I32 => "I32",
                PrimitiveType::I64 => "I64",
                PrimitiveType::U8 => "U8",
                PrimitiveType::U16 => "U16",
                PrimitiveType::U32 => "U32",
                PrimitiveType::U64 => "U64",
                PrimitiveType::F32 => "F32",
                PrimitiveType::F64 => "F64",
                _ => return None,
            })
        }
    }
    pub use float_conversion_casts::bind_float_conversion_casts;
}

pub mod static_machine_call_contracts {
    use crate::CheckingInput;
    use diagnostics::Diagnostic;
    use flow_effects::OperationalPlan;
    use symbols::SymbolHandle;
    use typed_trees::data::TypeParameterKind;

    pub fn static_machine_template_reach_contract_bytes(
        program: &dyn CheckingInput,
        inferred: &flow_effects::ServiceReachInferencePlan,
        machine: &typed_trees::machine::Machine,
    ) -> Result<Vec<u8>, Diagnostic> {
        let mut summaries = inferred
            .machines()
            .iter()
            .filter(|summary| summary.machine == machine.symbol);
        let summary = summaries.next().ok_or_else(|| {
            Diagnostic::error("generic template has no inferred service-reach dependency")
        })?;
        if summaries.next().is_some() {
            return Err(Diagnostic::error(
                "generic template has ambiguous reach dependencies",
            ));
        }
        let parameters = program.machine_type_parameters(machine);
        if parameters.len() != machine.type_parameters.len() {
            return Err(Diagnostic::error(
                "generic reach dependency lost its template telescope",
            ));
        }
        let mut bytes = b"omega.static-machine-reach-dependency.v1\0".to_vec();
        let mut services = Vec::new();
        for service in inferred.services(summary.dependency.concrete) {
            let definition = program
                .service_reaches
                .definition(*service)
                .ok_or_else(|| {
                    Diagnostic::error("generic reach dependency has an unknown service")
                })?;
            let mut encoded = Vec::new();
            encode_declaration(program, definition.symbol, &mut encoded);
            services.push(encoded);
        }
        services.sort();
        services.dedup();
        bytes.extend((services.len() as u64).to_le_bytes());
        for service in services {
            bytes.extend(service);
        }
        let dependencies = inferred
            .dependency_parameters
            .span_or_empty(summary.dependency.parameters);
        if dependencies.len() != summary.dependency.parameters.len() {
            return Err(Diagnostic::error(
                "generic reach dependency lost its binder span",
            ));
        }
        let mut ordinals = Vec::with_capacity(dependencies.len());
        let capture =
            program.conformance_by_symbol(program.declaration_scope_parent(machine.symbol));
        let captured_parameters = capture
            .map(|declaration| program.conformance_type_parameters(declaration))
            .unwrap_or(&[]);
        if capture.is_some_and(|declaration| {
            captured_parameters.len() != declaration.type_parameters.len()
        }) {
            return Err(Diagnostic::error(
                "capture reach dependency lost its map telescope",
            ));
        }
        let mut captured_ordinals = Vec::new();
        for dependency in dependencies {
            let (scope, scope_parameters) = if parameters
                .iter()
                .any(|parameter| parameter.symbol == *dependency)
            {
                (false, parameters)
            } else {
                (true, captured_parameters)
            };
            let mut matches = scope_parameters
                .iter()
                .enumerate()
                .filter(|(_, parameter)| parameter.symbol == *dependency);
            let Some((ordinal, parameter)) = matches.next() else {
                return Err(Diagnostic::error(
                    "generic reach variable is outside its invocation and capture telescopes",
                ));
            };
            if matches.next().is_some()
                || !matches!(
                    parameter.kind,
                    TypeParameterKind::Machine {
                        contract: typed_trees::data::MachineParameterContract::Nominal { .. }
                    }
                )
            {
                return Err(Diagnostic::error(
                    "generic reach variable is not one exact nominal binder",
                ));
            }
            if scope {
                captured_ordinals.push(ordinal as u64);
            } else {
                ordinals.push(ordinal as u64);
            }
        }
        ordinals.sort_unstable();
        if ordinals.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(Diagnostic::error(
                "generic reach dependency repeats a binder",
            ));
        }
        bytes.extend((ordinals.len() as u64).to_le_bytes());
        for ordinal in ordinals {
            bytes.extend(ordinal.to_le_bytes());
        }
        bytes.extend(static_machine_parameter_contract_bytes(program, parameters));
        if let Some(capture) = capture {
            bytes.extend(b"omega.static-machine-capture-reach.v1\0");
            encode_declaration(program, capture.symbol, &mut bytes);
            captured_ordinals.sort_unstable();
            if captured_ordinals.windows(2).any(|pair| pair[0] == pair[1]) {
                return Err(Diagnostic::error(
                    "capture reach dependency repeats a binder",
                ));
            }
            bytes.extend((captured_ordinals.len() as u64).to_le_bytes());
            for ordinal in captured_ordinals {
                bytes.extend(ordinal.to_le_bytes());
            }
            bytes.extend(static_machine_parameter_contract_bytes(
                program,
                captured_parameters,
            ));
        }
        Ok(bytes)
    }

    pub fn static_machine_parameter_contract_bytes(
        program: &dyn CheckingInput,
        parameters: &[typed_trees::data::TypeParameter],
    ) -> Vec<u8> {
        let mut bytes = b"omega.static-machine-operational-contracts.v1\0".to_vec();
        bytes.extend((parameters.len() as u64).to_le_bytes());
        for (ordinal, parameter) in parameters.iter().enumerate() {
            let TypeParameterKind::Machine { contract } = &parameter.kind else {
                continue;
            };
            bytes.extend((ordinal as u64).to_le_bytes());
            let Some(contract) = program.machine_parameter_contract_view(contract) else {
                bytes.push(0);
                continue;
            };
            match contract {
                typed_trees::data::MachineParameterContractView::Structural(_) => bytes.push(1),
                typed_trees::data::MachineParameterContractView::Nominal {
                    trait_definition,
                    requirement,
                } => {
                    bytes.push(2);
                    encode_declaration(program, trait_definition.symbol, &mut bytes);
                    let identity = program
                        .trait_requirement_callable_identity_text(trait_definition, requirement);
                    bytes.extend((identity.len() as u64).to_le_bytes());
                    bytes.extend(identity.as_bytes());
                }
            }
            let signature = contract.signature();
            let mut services = Vec::new();
            for service in program
                .service_reach_rows
                .services(signature.service_reach_row)
            {
                let mut encoded = Vec::new();
                if let Some(definition) = program.service_reaches.definition(*service) {
                    encoded.push(1);
                    encode_declaration(program, definition.symbol, &mut encoded);
                } else {
                    encoded.push(0);
                }
                services.push(encoded);
            }
            services.sort();
            services.dedup();
            bytes.extend((services.len() as u64).to_le_bytes());
            for service in services {
                bytes.extend(service);
            }
            bytes.push(u8::from(signature.service_reach_is_installation_bound));
            bytes.push(u8::from(signature.suspends));
            bytes.push(u8::from(signature.blocks));
            let mut invocations = Vec::new();
            for invocation in crate::machine_calls::effect_inference::declared_signature_invocations(
                program, signature,
            ) {
                let mut encoded = Vec::new();
                match invocation {
                    flow_effects::InvocationTarget::Parameter(ordinal) => {
                        encoded.push(1);
                        encoded.extend(ordinal.to_le_bytes());
                    }
                    flow_effects::InvocationTarget::Service(service) => {
                        encoded.push(2);
                        encode_declaration(program, service, &mut encoded);
                    }
                }
                invocations.push(encoded);
            }
            invocations.sort();
            invocations.dedup();
            bytes.extend((invocations.len() as u64).to_le_bytes());
            for invocation in invocations {
                bytes.extend(invocation);
            }
        }
        bytes
    }

    fn encode_declaration(program: &dyn CheckingInput, symbol: SymbolHandle, bytes: &mut Vec<u8>) {
        if let Some(package) = program.symbols.symbol_package_identity(symbol) {
            bytes.push(1);
            bytes.extend(package.digest());
        } else {
            bytes.push(0);
        }
        let path = program.symbols.display_path(symbol, "::");
        bytes.extend((path.len() as u64).to_le_bytes());
        bytes.extend(path.as_bytes());
    }

    pub fn static_machine_call_binding_bytes(
        program: &dyn CheckingInput,
        operational: &OperationalPlan,
        specialization: &typed_trees::typed_trees::MachineSpecialization,
    ) -> Result<Vec<u8>, Diagnostic> {
        let mut bytes = b"omega.static-machine-call-bindings.v2\0".to_vec();
        let mut owners = operational
            .machines()
            .iter()
            .filter(|machine| machine.symbol == specialization.instance);
        let Some(owner) = owners.next() else {
            return Err(Diagnostic::error(
                "static machine binding footprint lost its caller",
            ));
        };
        if owners.next().is_some() {
            return Err(Diagnostic::error(
                "static machine binding footprint has ambiguous callers",
            ));
        }
        let parameters = program
            .data_type_parameters
            .span_or_empty(specialization.template_parameters);
        for (state_ordinal, state) in operational
            .states
            .span_or_empty(owner.states)
            .iter()
            .enumerate()
        {
            for call in operational.calls.span_or_empty(state.calls) {
                if !call.static_machine_parameter.is_valid() {
                    continue;
                }
                let local = typed_trees::data::type_parameter_position(
                    parameters,
                    call.static_machine_parameter,
                );
                let capture = program
                    .captured_machine_applications
                    .iter()
                    .filter(|(_, capture)| capture.instance == specialization.template)
                    .map(|(_, capture)| capture)
                    .collect::<Vec<_>>();
                let scoped_binder = if let Some(position) = local {
                    (0_u8, position)
                } else if let [capture] = capture.as_slice() {
                    let declaration = program
                        .conformance_by_symbol(capture.conformance_application.declaration)
                        .ok_or_else(|| {
                            Diagnostic::error("static call lost its capture declaration")
                        })?;
                    let position = typed_trees::data::type_parameter_position(
                        program.conformance_type_parameters(declaration),
                        call.static_machine_parameter,
                    )
                    .ok_or_else(|| {
                        Diagnostic::error(
                            "static machine binding footprint lost its captured binder",
                        )
                    })?;
                    (1_u8, position)
                } else {
                    return Err(Diagnostic::error(
                        "static machine binding footprint lost its binder",
                    ));
                };
                let Some((_, selected)) =
                    crate::declarations::transitions::resolved_transition_target_state(
                        program,
                        call.target_state_symbol,
                    )
                else {
                    return Err(Diagnostic::error(
                        "static machine binding footprint lost its selected entry",
                    ));
                };
                bytes.extend((state_ordinal as u64).to_le_bytes());
                bytes.extend((call.statement_index as u64).to_le_bytes());
                bytes.extend((call.call_ordinal as u64).to_le_bytes());
                bytes.push(scoped_binder.0);
                bytes.extend((scoped_binder.1 as u64).to_le_bytes());
                encode_declaration(program, selected.symbol, &mut bytes);
            }
        }
        Ok(bytes)
    }
}

pub mod terminal_transitions {
    use symbols::SymbolHandle;
    use symbols::SymbolKind;
    use typed_trees::TypedTrees;
    use typed_trees::expression::{
        ExpressionHandle, ExpressionNode, TableCallExpression, TableNamePath,
    };
    use typed_trees::statement::TransitionTargetNode;

    pub fn normalize_terminal_transition_calls(
        program: &mut TypedTrees,
        first_machine: usize,
        completed_machines: &[SymbolHandle],
        captured_arguments: impl Fn(SymbolHandle) -> Vec<typed_trees::expression::StaticMachineArgument>,
    ) {
        let mut targets = Vec::new();
        for (index, machine) in program.machines().iter().enumerate() {
            if index < first_machine && !completed_machines.contains(&machine.symbol) {
                continue;
            }
            for state in program.machine_states(machine) {
                for statement in program.statement_table.statements(state.statement_nodes) {
                    let Some(transition) = statement.as_transition() else {
                        continue;
                    };
                    for target in transition.targets() {
                        let TransitionTargetNode::Named { path, .. } =
                            program.statement_table.transition_target(target)
                        else {
                            continue;
                        };
                        let members = program.statement_table.name_path_members(path.members);
                        if members.is_empty() {
                            continue;
                        }
                        let callable =
                            crate::terminal_callable_target(program, machine, path.symbol)
                                .is_some();
                        let implementation_namespace = path.receiver_symbol.is_valid()
                            && matches!(
                                program.symbols.get(path.receiver_symbol).kind,
                                SymbolKind::Conformance | SymbolKind::ConformanceParameter
                            )
                            && (!path.symbol.is_valid()
                                || (program.symbols.get(path.symbol).kind == SymbolKind::State
                                    && program.symbols.get(path.symbol).parent
                                        == path.receiver_symbol));
                        if path.symbol.is_valid()
                            && !callable
                            && !implementation_namespace
                            && program.operator_definition_by_symbol(path.symbol).is_none()
                        {
                            continue;
                        }
                        if members.len() > 1
                            && path.head_symbol.is_valid()
                            && !matches!(
                                program.symbols.get(path.head_symbol).kind,
                                SymbolKind::Module
                                    | SymbolKind::BuiltinType
                                    | SymbolKind::Data
                                    | SymbolKind::Domain
                                    | SymbolKind::Machine
                                    | SymbolKind::Trait
                                    | SymbolKind::Conformance
                                    | SymbolKind::ConformanceParameter
                            )
                        {
                            continue;
                        }
                        if (callable
                            || implementation_namespace
                            || program.operators().iter().any(|operator| {
                                operator.is_boundary
                                    && operator.spelling.is_none()
                                    && program.operator_path_members(operator.name) == members
                            }))
                            && !targets.iter().any(|(previous, _)| *previous == target)
                        {
                            targets.push((target, machine.symbol));
                        }
                    }
                }
            }
        }
        for (target, lexical_owner) in targets {
            let TransitionTargetNode::Named {
                path,
                arguments,
                evidence_arguments,
                source_span,
                authored_call_selection,
                static_machine_parameter,
                operational_acknowledgement,
            } = program.statement_table.transition_target(target).clone()
            else {
                unreachable!("collected boundary transition changed shape");
            };
            let mut members = program
                .statement_table
                .name_path_members(path.members)
                .to_vec();
            let leaf = members.pop().expect("boundary declaration has a path");
            let namespace_head = path.head_symbol;
            let namespace_endpoint = path.receiver_symbol;
            let receiver = if members.is_empty() {
                ExpressionHandle::invalid()
            } else {
                let mut namespace = arena::HandleSpan::empty();
                for member in members {
                    program
                        .expression_table
                        .push_name_path_member(&mut namespace, member);
                }
                let receiver =
                    program
                        .expression_table
                        .insert(ExpressionNode::Name(TableNamePath {
                            members: namespace,
                            member_symbols: arena::HandleSpan::empty(),
                            head_symbol: namespace_head,
                            symbol: namespace_endpoint,
                            reserved_result_of: SymbolHandle::invalid(),
                        }));
                program
                    .expression_table
                    .set_source_span(receiver, source_span);
                receiver
            };
            let operands = program
                .statement_table
                .expression_handles(arguments)
                .to_vec();
            let arguments = program.expression_table.insert_expression_handles(operands);
            let target_symbol = crate::call_target_entry_symbol(program, path.symbol);
            let declaration = crate::find_machine_head(program, path.symbol)
                .or_else(|| crate::find_state(program, path.symbol))
                .map_or(path.symbol, |(machine, _)| machine.symbol);
            let machine_arguments = captured_arguments(path.symbol);
            let application = program.expression_table.retain_static_machine_application(
                path.symbol,
                leaf.clone(),
                &machine_arguments,
            );
            let retained = program
                .expression_table
                .static_machine_application_mut(application);
            retained.lexical_owner = lexical_owner;
            retained.binding_declaration = declaration;
            let call = program
                .expression_table
                .insert(ExpressionNode::Call(TableCallExpression {
                    authored_call_selection,
                    authored_call_binding: None,
                    authored_machine_application: application,
                    requirement_application: Default::default(),
                    exact_coercion_target: Default::default(),
                    receiver,
                    target_symbol,
                    static_machine_parameter,
                    target: leaf,
                    static_requirement_dispatch: None,
                    machine_arguments: machine_arguments.into_boxed_slice(),
                    quotient_operation: None,
                    private_layout_operation: None,
                    installed_interrupt_operation: None,
                    layout_query: None,
                    native_code_layout: None,
                    build_selection: Default::default(),
                    arguments,
                    evidence_arguments,
                    operational_acknowledgement,
                }));
            program.expression_table.set_source_span(call, source_span);
            if let Some(occurrence) = authored_call_selection {
                program
                    .expression_table
                    .attach_authored_selection_occurrences(call, [occurrence]);
            }
            *program.statement_table.transition_target_mut(target) =
                TransitionTargetNode::Value(call);
        }
    }
}

pub(crate) fn record_reference<T: PartialEq>(recorded: &mut Vec<T>, reference: T) -> bool {
    let fresh = !recorded.contains(&reference);
    if fresh {
        recorded.push(reference);
    }
    fresh
}

/*
State-body statement lowering (`expressions::statement`): calls, transitions,
root bindings, inline assembly windows, and initializer types.
*/

pub(crate) mod statement {
    /// Types already produced in this state, without any permission to read or
    /// move their values. Later initializers reuse the exact type handles.
    pub(crate) struct LocalTypes {
        rows: Vec<(
            symbols::SymbolHandle,
            Option<typed_trees::types::TypeReferenceHandle>,
        )>,
    }

    impl LocalTypes {
        pub(crate) fn new(
            program: &typed_trees::TypedTrees,
            state: &typed_trees::state::State,
            source: &symbol_resolved_trees::SymbolResolvedTrees,
            statements: &[symbol_resolved_trees::statement::StatementNode],
        ) -> Result<Self, diagnostics::Diagnostic> {
            use symbol_resolved_trees::statement::StatementNode;
            let binding = |statement: &StatementNode| match statement {
                StatementNode::LocalData(local) if local.role.has_ordinary_initializer() => {
                    Some((local.symbol, None))
                }
                _ => None,
            };
            // Prepare only this body's binders. A slot exists before its
            // initializer runs, but has no type available until publication.
            let mut rows = program
                .state_parameters(state)
                .iter()
                .map(|parameter| (parameter.symbol, None))
                .collect::<Vec<_>>();
            rows.extend(statements.iter().filter_map(binding));
            let bodies = crate::tables::resolved_statements(source);
            let declarations = &source.tables.declarations;
            for statement in statements {
                if let StatementNode::Assembly(handle) = statement {
                    let graph = declarations.assembly_graphs.get(*handle);
                    for block in declarations.assembly_blocks.span_or_empty(graph.blocks) {
                        rows.extend(
                            bodies
                                .statements(block.statement_nodes)
                                .iter()
                                .filter_map(binding),
                        );
                    }
                }
            }
            rows.retain(|(symbol, _)| symbol.is_valid());
            if rows
                .iter()
                .any(|(symbol, _)| !program.symbols.symbols().nodes().is_valid(*symbol))
            {
                return Err(diagnostics::Diagnostic::error(
                    "local type information names an unavailable binder",
                ));
            }
            rows.sort_unstable_by_key(|(symbol, _)| (symbol.arena_index(), symbol.generation()));
            rows.dedup_by_key(|(symbol, _)| *symbol);
            let mut types = Self { rows };
            for parameter in program.state_parameters(state) {
                types.insert(parameter.symbol, parameter.type_reference)?;
            }
            Ok(types)
        }

        fn insert(
            &mut self,
            symbol: symbols::SymbolHandle,
            reference: typed_trees::types::TypeReferenceHandle,
        ) -> Result<(), diagnostics::Diagnostic> {
            if !symbol.is_valid() {
                return Ok(());
            }
            let position = self.position(symbol).ok_or_else(|| {
                diagnostics::Diagnostic::error("local type information has no declared binder")
            })?;
            if self.rows[position].1.replace(reference).is_some() {
                return Err(diagnostics::Diagnostic::error(
                    "local type information repeats a binder",
                ));
            }
            Ok(())
        }

        fn position(&self, symbol: symbols::SymbolHandle) -> Option<usize> {
            self.rows
                .binary_search_by_key(
                    &(symbol.arena_index(), symbol.generation()),
                    |(symbol, _)| (symbol.arena_index(), symbol.generation()),
                )
                .ok()
        }

        fn record(
            &mut self,
            statement: &typed_trees::statement::StatementNode,
        ) -> Result<(), diagnostics::Diagnostic> {
            if let Some(local) = statement.as_local_data()
                && local.role.has_ordinary_initializer()
            {
                // An initializer not yet typed supplies no receiving type.
                // A produced Unit result still has its own valid handle.
                if local.type_reference.is_valid() {
                    self.insert(local.symbol, local.type_reference)?;
                }
            }
            Ok(())
        }

        fn get(
            &self,
            symbol: symbols::SymbolHandle,
        ) -> Option<typed_trees::types::TypeReferenceHandle> {
            self.position(symbol)
                .and_then(|position| self.rows[position].1)
        }
    }
    pub(crate) mod arguments {
        use crate::expressions::expression::lower_executable_expression_handle;
        use crate::lowerer::Lowerer;
        use crate::tables::{resolved_statements, statement_table_mut};
        use arena::HandleSpan;
        use diagnostics::Diagnostic;
        use symbol_resolved_trees as resolved;
        use typed_trees as typed;

        pub(crate) fn lower_statement_expression(
            lowerer: &mut Lowerer,
            expression: resolved::expression::ExpressionHandle,
        ) -> Result<typed::expression::ExpressionHandle, Diagnostic> {
            lower_executable_expression_handle(lowerer, expression)
        }

        pub(crate) fn lower_statement_argument_span(
            lowerer: &mut Lowerer,
            arguments: HandleSpan<resolved::expression::ExpressionHandle>,
        ) -> Result<HandleSpan<typed::expression::ExpressionHandle>, Diagnostic> {
            let arguments = resolved_statements(lowerer.source_trees)
                .expression_handles(arguments)
                .to_vec();
            let lowered_arguments = arguments
                .into_iter()
                .map(|argument| lower_statement_expression(lowerer, argument))
                .collect::<Result<Vec<_>, _>>()?;

            Ok(statement_table_mut(&mut lowerer.typed_trees)
                .insert_expression_handles(lowered_arguments))
        }

        pub(crate) fn lower_statement_path_members(
            lowerer: &mut Lowerer,
            path: HandleSpan<resolved::name::DiagnosticName>,
        ) -> HandleSpan<typed::name::Identifier> {
            let mut lowered_path = HandleSpan::empty();

            for member in resolved_statements(lowerer.source_trees).name_path_members(path) {
                statement_table_mut(&mut lowerer.typed_trees).push_name_path_member(
                    &mut lowered_path,
                    crate::lowerer::name::lower_name(member),
                );
            }

            lowered_path
        }
    }
    pub(crate) mod calls {
        use crate::expressions::proposition::is_proposition_call_target;
        use crate::lowerer::Lowerer;
        use crate::tables::{expression_table_mut, resolved_statements};
        use diagnostics::Diagnostic;
        use symbol_resolved_trees as resolved;
        use typed::statement::StatementNode;
        use typed_trees as typed;

        use super::arguments::{lower_statement_argument_span, lower_statement_path_members};

        pub(crate) fn lower_call_statement(
            lowerer: &mut Lowerer,
            call: &resolved::statement::TableCall,
        ) -> Result<StatementNode, Diagnostic> {
            if crate::runtime_callables::callee_is_value(
                Some(lowerer.source_trees),
                &lowerer.source_trees.tables.bodies.expressions,
                call.callee,
            ) {
                return Ok(StatementNode::Call(lower_ordinary_call(lowerer, call)?));
            }
            let table = resolved_statements(lowerer.source_trees);
            let arguments = table.expression_handles(call.arguments);
            let operand = if arguments.len() == 1 {
                Some(
                    crate::tables::resolved_expressions(lowerer.source_trees)
                        .expression(arguments[0]),
                )
            } else {
                None
            };
            let members = match operand {
                Some(resolved::expression::ExpressionNode::Name(path)) => {
                    crate::tables::resolved_expressions(lowerer.source_trees)
                        .name_path_members(path.members)
                }
                _ => &[],
            };
            if call.machine_arguments.is_empty()
                && call.evidence_arguments.is_empty()
                && let Some(request) = crate::expressions::product_queries::description_request(
                    lowerer.source_trees,
                    call.target_symbol,
                    (!call.receiver.is_empty()).then_some(call.receiver_symbol),
                    operand,
                    members,
                    call.target.source_span(),
                )
            {
                let expression = crate::expressions::build_selections::lower(
                    lowerer.source_trees,
                    &mut lowerer.typed_trees,
                    request,
                    Vec::new(),
                    lowerer.source_owner,
                    Vec::new(),
                    call.operational_acknowledgement,
                )?;
                if let Some(occurrence) = call.authored_call_selection
                    && let typed::expression::ExpressionNode::Call(generated) =
                        lowerer.typed_trees.expression_table.expression(expression)
                {
                    let selected = generated.target_symbol;
                    crate::call_and_product_selections::bind_generated_call_selection(
                        &mut lowerer.typed_trees,
                        occurrence,
                        selected,
                    )?;
                }
                return Ok(StatementNode::Expression(expression));
            }
            if let Some(operation) = crate::expressions::product_queries::operation(
                lowerer.source_trees,
                call.target_symbol,
                (!call.receiver.is_empty()).then_some(call.receiver_symbol),
            ) {
                if !call.machine_arguments.is_empty() || !call.evidence_arguments.is_empty() {
                    return Err(Diagnostic::error(
                        "Product queries accept only their ordinary value arguments",
                    ));
                }
                let mut lowered = lower_ordinary_call(lowerer, call)?;
                let protocol = lowerer
                    .source_trees
                    .traits
                    .iter()
                    .find(|definition| definition.symbol == operation.namespace)
                    .ok_or_else(|| {
                        Diagnostic::error("Product query lost its toolchain protocol")
                    })?;
                lowered.receiver = Default::default();
                lowerer.typed_trees.statement_table.push_name_path_member(
                    &mut lowered.receiver,
                    crate::lowerer::name::lower_name(&protocol.name),
                );
                lowered.receiver_root_symbol = operation.namespace;
                lowered.receiver_symbol = operation.namespace;
                lowered.target_symbol = operation.target;
                let signature = lowerer
                    .source_trees
                    .trait_machine_signatures(protocol.machines)
                    .iter()
                    .find(|signature| signature.symbol == operation.target)
                    .ok_or_else(|| Diagnostic::error("Product query lost its exact operation"))?;
                lowered.target = crate::lowerer::name::lower_name(&signature.name);
                if let Some(occurrence) = call.authored_call_selection
                    && !crate::call_and_product_selections::bind_generated_call_selection(
                        &mut lowerer.typed_trees,
                        occurrence,
                        operation.target,
                    )?
                {
                    // The author named the declared query of the prelude; the
                    // protocol call typing spells is not that call.
                    lowered.authored_call_selection = None;
                }
                return Ok(StatementNode::Call(lowered));
            }
            if let Some(request) = crate::expressions::product_selections::request(
                lowerer.source_trees,
                &mut lowerer.typed_trees,
                lowerer.type_reference_exposure,
                &call.authored_static_argument_selections,
                call.receiver_symbol,
                call.target_symbol,
                call.target.as_str(),
                &call.machine_arguments,
                call.arguments.len(),
                call.evidence_arguments.len(),
                call.target.source_span(),
            )? {
                let mut occurrences = call.authored_call_selection.into_iter().collect::<Vec<_>>();
                occurrences.extend(call.authored_static_argument_selections.iter().copied());
                let arguments = resolved_statements(lowerer.source_trees)
                    .expression_handles(call.arguments)
                    .to_vec()
                    .into_iter()
                    .map(|argument| super::arguments::lower_statement_expression(lowerer, argument))
                    .collect::<Result<Vec<_>, Diagnostic>>()?;
                let expression = crate::expressions::build_selections::lower(
                    lowerer.source_trees,
                    &mut lowerer.typed_trees,
                    request.request,
                    request.inputs,
                    lowerer.source_owner,
                    arguments,
                    call.operational_acknowledgement,
                )?;
                if let Some(occurrence) = call.authored_call_selection
                    && let typed::expression::ExpressionNode::Call(generated) =
                        lowerer.typed_trees.expression_table.expression(expression)
                {
                    let selected = generated.target_symbol;
                    crate::call_and_product_selections::bind_generated_call_selection(
                        &mut lowerer.typed_trees,
                        occurrence,
                        selected,
                    )?;
                }
                expression_table_mut(&mut lowerer.typed_trees)
                    .attach_authored_selection_occurrences(expression, occurrences);
                return Ok(StatementNode::Expression(expression));
            }
            Ok(StatementNode::Call(lower_ordinary_call(lowerer, call)?))
        }

        fn lower_ordinary_call(
            lowerer: &mut Lowerer,
            call: &resolved::statement::TableCall,
        ) -> Result<typed::statement::TableCall, Diagnostic> {
            if crate::runtime_callables::callee_is_value(
                Some(lowerer.source_trees),
                &lowerer.source_trees.tables.bodies.expressions,
                call.callee,
            ) {
                let callee = super::arguments::lower_statement_expression(lowerer, call.callee)?;
                let arguments = lower_statement_argument_span(lowerer, call.arguments)?;
                let machine_arguments = crate::signatures::lexical_context::call_arguments(
                    Some(lowerer.source_trees),
                    &mut lowerer.typed_trees,
                    lowerer.type_reference_exposure,
                    symbols::SymbolHandle::invalid(),
                    &call.machine_arguments,
                    &call.authored_static_argument_selections,
                )?;
                return Ok(typed::statement::TableCall {
                    authored_call_selection: call.authored_call_selection,
                    runtime_call: Some(typed::callable_values::RuntimeCallBinding {
                        callee,
                        contract: Default::default(),
                    }),
                    target: crate::lowerer::name::lower_name(&call.target),
                    machine_arguments,
                    arguments,
                    evidence_arguments: call
                        .evidence_arguments
                        .iter()
                        .map(crate::lowerer::name::lower_name)
                        .collect::<Vec<_>>()
                        .into_boxed_slice(),
                    operational_acknowledgement: call.operational_acknowledgement,
                    discards_result: call.discards_result,
                    source_span: call.target.source_span(),
                    ..Default::default()
                });
            }
            if is_proposition_call_target(&lowerer.source_trees.symbols, call.target_symbol) {
                return Err(Diagnostic::error(
                    "a proposition application is proof-only and cannot appear as an executable call statement",
                ));
            }
            let installed_interrupt_operation =
                crate::expressions::installed_interrupts::lower_request(
                    Some(lowerer.source_trees),
                    &mut lowerer.typed_trees,
                    lowerer.type_reference_exposure,
                    &call.authored_static_argument_selections,
                    call.target_symbol,
                    &call.machine_arguments,
                )?;
            let ordinary_arguments = if installed_interrupt_operation.is_some() {
                &call.machine_arguments[2..]
            } else {
                &call.machine_arguments
            };
            let arguments = lower_statement_argument_span(lowerer, call.arguments)?;

            Ok(typed::statement::TableCall {
                runtime_call: None,
                unresolved_name: call.unresolved_name,
                installed_interrupt_operation,
                authored_machine_application: Default::default(),
                receiver_root_symbol: call.receiver_root_symbol,
                receiver_symbol: call.receiver_symbol,
                target_symbol: call.target_symbol,
                receiver: lower_statement_path_members(lowerer, call.receiver),
                target: crate::lowerer::name::lower_name(&call.target),
                static_machine_parameter: symbols::SymbolHandle::invalid(),
                static_requirement_dispatch: None,
                machine_arguments: crate::signatures::lexical_context::call_arguments(
                    Some(lowerer.source_trees),
                    &mut lowerer.typed_trees,
                    lowerer.type_reference_exposure,
                    call.target_symbol,
                    ordinary_arguments,
                    &call.authored_static_argument_selections,
                )?,
                arguments,
                evidence_arguments: call
                    .evidence_arguments
                    .iter()
                    .map(crate::lowerer::name::lower_name)
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
                operational_acknowledgement: call.operational_acknowledgement,
                discards_result: call.discards_result,
                source_span: call.target.source_span(),
                authored_call_selection: call.authored_call_selection,
                reflected_field_origin: None,
            })
        }
    }
    pub(crate) mod initializer_type {
        use super::LocalTypes;
        use crate::expressions::call_results::declared_call_return;
        use crate::lowerer::Lowerer;
        use crate::tables::resolved_expressions;
        use crate::type_reference::lower_type_reference_into_table;
        use diagnostics::Diagnostic;
        use symbol_resolved_trees as resolved;
        use symbol_resolved_trees::types::TypeReference;
        use typed_trees as typed;

        use resolved::expression::{ExpressionHandle, ExpressionNode};

        pub(crate) fn infer_initializer_type(
            lowerer: &mut Lowerer,
            state: &resolved::state::State,
            types: &LocalTypes,
            initial_value: ExpressionHandle,
        ) -> Result<Option<typed::types::TypeReferenceHandle>, Diagnostic> {
            let expressions = &lowerer.source_trees.tables.bodies.expressions;

            if let ExpressionNode::Borrow(borrow) = expressions.expression(initial_value) {
                let (target, access) = (borrow.target, borrow.access);
                let Some(referee) = infer_initializer_type(lowerer, state, types, target)? else {
                    return Ok(None);
                };
                return Ok(Some(lowerer.typed_trees.type_reference_table.insert(
                    typed::types::TypeReferenceNode::Reference {
                        referee,
                        access,
                        lifetime: None,
                    },
                )));
            }

            // Collection lengths keep their selected u64 observation type
            // when read from an initializer.
            if matches!(
                expressions.expression(initial_value),
                ExpressionNode::Member(_)
            ) {
                let lowered = crate::expressions::expression::lower_executable_expression_handle(
                    lowerer,
                    initial_value,
                )?;
                if lowerer
                    .typed_trees
                    .expression_table
                    .member_node(lowered)
                    .is_some_and(|member| {
                        member.selection
                            == typed::expression::MemberSelection::CollectionMeasure(
                                typed::expression::CollectionMeasure::Length,
                            )
                    })
                {
                    return builtin_primitive_type(lowerer, symbols::BuiltinTypeAtom::U64, "u64");
                }
            }

            match expressions.expression(initial_value) {
                ExpressionNode::StructLiteral(literal) => {
                    let reference = if literal.application_type.is_valid() {
                        let declared = lowerer
                            .source_trees
                            .child_type_reference(literal.application_type);
                        lower_type_reference_into_table(lowerer, declared)?
                    } else if literal.type_symbol.is_valid() {
                        lower_type_reference_into_table(
                            lowerer,
                            &TypeReference::Named {
                                symbol: literal.type_symbol,
                                name: literal.type_name.clone(),
                                supplied_operand_owner: Default::default(),
                            },
                        )?
                    } else {
                        return Ok(None);
                    };
                    return Ok(Some(reference));
                }
                ExpressionNode::Call(call)
                    if language_core::data_layout_queries::is_data_layout_query_spelling(
                        call.target.as_str(),
                        call.target_symbol.is_valid(),
                        call.receiver.is_valid(),
                    ) =>
                {
                    return builtin_primitive_type(lowerer, symbols::BuiltinTypeAtom::U64, "u64");
                }
                ExpressionNode::Call(call) => {
                    let target_symbol = if call.target_symbol.is_valid() {
                        call.target_symbol
                    } else {
                        crate::expressions::expression::declared_receiver_method_target(
                            lowerer.source_trees,
                            expressions,
                            call.receiver,
                            &call.target,
                        )
                    };
                    let mut machine_substitutions = Vec::new();
                    let declared_return = declared_call_return(lowerer.source_trees, target_symbol);
                    let declared_return = declared_return
                    .cloned()
                    .or_else(|| {
                        static_conformance_requirement_return(lowerer, state, call)
                    })
                    .or_else(|| {
                        lowerer
                            .source_trees
                            .roots
                            .machines
                            .iter()
                            .find_map(|machine| {
                                lowerer
                                    .source_trees
                                    .machine_type_parameters(machine)
                                    .iter()
                                    .find_map(|parameter| match &parameter.kind {
                                        resolved::data::TypeParameterKind::Machine { contract }
                                            if parameter.symbol == target_symbol =>
                                        {
                                            let contract = lowerer
                                                .source_trees
                                                .machine_parameter_contract_view(contract)?
                                                .signature();
                                            machine_substitutions.extend(
                                                lowerer
                                                    .source_trees
                                                    .data_type_parameters(contract.type_parameters)
                                                    .iter()
                                                    .filter(|parameter| {
                                                        matches!(
                                                            parameter.kind,
                                                            resolved::data::TypeParameterKind::Machine { .. }
                                                        )
                                                    })
                                                    .zip(call.machine_arguments.iter())
                                                    .filter_map(|(parameter, argument)| {
                                                        argument.path.last().map(|name| {
                                                            (
                                                                parameter.symbol,
                                                                argument.symbol,
                                                                crate::lowerer::name::lower_name(name),
                                                            )
                                                        })
                                                    }),
                                            );
                                            contract.return_type.clone()
                                        }
                                        _ => None,
                                    })
                            })
                    });
                    let Some(declared_return) = declared_return else {
                        return Ok(None);
                    };
                    let declared_return =
                        lower_type_reference_into_table(lowerer, &declared_return)?;
                    let machine_substitutions = machine_substitutions
                        .into_iter()
                        .map(|(parameter, symbol, name)| {
                            let reference = lowerer
                                .typed_trees
                                .type_reference_table
                                .insert(typed::types::TypeReferenceNode::Named { symbol, name });
                            (parameter, reference)
                        })
                        .collect::<Vec<_>>();
                    substitute_call_type_arguments(
                        &mut lowerer.typed_trees,
                        declared_return,
                        &machine_substitutions,
                        target_symbol,
                        state.symbol,
                    )?;
                    instantiate_declared_call_result(
                        lowerer,
                        state,
                        types,
                        call,
                        target_symbol,
                        declared_return,
                    )?;
                    return Ok(Some(declared_return));
                }
                _ => {}
            }

            if matches!(
                expressions.expression(initial_value),
                ExpressionNode::Name(_) | ExpressionNode::Member(_) | ExpressionNode::Indexed(_)
            ) {
                return declared_projection_type(lowerer, state, types, initial_value);
            }

            // A captured Boolean result has the same type as an authored
            // local. Membership and negation need no operand-type guess;
            // lowering still checks the selected case/domain and operands.
            let boolean_result = match expressions.expression(initial_value) {
                ExpressionNode::Boolean(_) | ExpressionNode::Membership(_) => true,
                ExpressionNode::Unary(unary) => {
                    unary.operator == resolved::expression::UnaryOperator::LogicalNot
                }
                ExpressionNode::Binary(binary) => binary.operator.result_is_boolean(),
                _ => false,
            };
            if boolean_result {
                return builtin_primitive_type(lowerer, symbols::BuiltinTypeAtom::Bool, "bool");
            }

            Ok(None)
        }

        fn declared_projection_type(
            lowerer: &mut Lowerer,
            state: &resolved::state::State,
            types: &LocalTypes,
            place: ExpressionHandle,
        ) -> Result<Option<typed::types::TypeReferenceHandle>, Diagnostic> {
            enum ProjectionStep<'source> {
                Element(ExpressionHandle),
                Field(&'source resolved::expression::TableMemberExpression),
            }
            let expressions = &lowerer.source_trees.tables.bodies.expressions;
            let mut steps = Vec::new();
            let mut root = place;
            loop {
                match expressions.expression(root) {
                    ExpressionNode::Indexed(indexed) => {
                        if matches!(
                            expressions.expression(indexed.index),
                            ExpressionNode::Range(_)
                        ) {
                            return Ok(None);
                        }
                        steps.push(ProjectionStep::Element(indexed.index));
                        root = indexed.collection;
                    }
                    ExpressionNode::Member(member) => {
                        steps.push(ProjectionStep::Field(member));
                        root = member.receiver;
                    }
                    ExpressionNode::Borrow(borrow) => root = borrow.target,
                    _ => break,
                }
            }
            let ExpressionNode::Name(path) = expressions.expression(root) else {
                return Ok(None);
            };
            if expressions.name_path_members(path.members).len() != 1 {
                return Ok(None);
            }
            let binders = [path.symbol, path.head_symbol];
            let root_type =
                if let Some(reference) = binders.into_iter().find_map(|symbol| types.get(symbol)) {
                    reference
                } else {
                    // A parameter supplies its declared type before statement lowering
                    // publishes any locals. An unavailable local cannot borrow a
                    // same-spelled parameter's type.
                    let parameters = lowerer.source_trees.state_parameters(state.parameters);
                    let Some(parameter) = binders
                        .into_iter()
                        .filter(|symbol| symbol.is_valid())
                        .find_map(|symbol| {
                            parameters
                                .iter()
                                .find(|parameter| parameter.symbol == symbol)
                        })
                    else {
                        return Ok(None);
                    };
                    lower_type_reference_into_table(lowerer, &parameter.type_reference)?
                };
            lowerer
                .typed_trees
                .prepare_data_application_view(root_type)
                .map_err(Diagnostic::error)?;
            let mut current = root_type;
            for step in steps.into_iter().rev() {
                let projected = match step {
                    ProjectionStep::Element(selector) => {
                        // Retain the exact authored operand even though reading
                        // an element type supplies no index/bounds evidence.
                        crate::expressions::expression::lower_executable_expression_handle(
                            lowerer, selector,
                        )?;
                        crate::typing_queries::collection_element_type(
                            &lowerer.typed_trees,
                            current,
                        )
                        .ok()
                    }
                    ProjectionStep::Field(member) => {
                        crate::typing_queries::declared_projection_field(
                            &lowerer.typed_trees,
                            current,
                            member.member.as_str(),
                            member.member_symbol,
                        )
                        .ok()
                        .filter(|_| member.case_variant.is_none())
                        .and_then(|field| {
                            let symbol = field.symbol;
                            crate::typing_queries::collection_field_type(
                                &lowerer.typed_trees,
                                current,
                                symbol,
                            )
                            .ok()
                        })
                    }
                };
                let Some(projected) = projected else {
                    // Applied field views are prepared after declaration
                    // lowering. Keep this initializer for ordinary capture
                    // inference and judgment once its declaration is read.
                    return Ok(None);
                };
                current = projected;
            }
            Ok(Some(current))
        }

        fn static_conformance_requirement_return(
            lowerer: &Lowerer,
            state: &resolved::state::State,
            call: &resolved::expression::TableCallExpression,
        ) -> Option<TypeReference> {
            let ExpressionNode::Name(receiver) =
                resolved_expressions(lowerer.source_trees).expression(call.receiver)
            else {
                return None;
            };
            let owner = lowerer.source_trees.roots.machines.iter().find(|machine| {
                lowerer
                    .source_trees
                    .machine_state_handles(machine.states)
                    .iter()
                    .any(|handle| {
                        lowerer.source_trees.machine_state(*handle).symbol == state.symbol
                    })
            })?;
            let mut bounds = owner
                .conformance_bounds
                .iter()
                .filter(|bound| bound.binder == receiver.symbol);
            let bound = bounds.next()?;
            if bounds.next().is_some() {
                return None;
            }
            let trait_definition = lowerer
                .source_trees
                .roots
                .trait_definition(bound.trait_symbol)?;
            let mut requirements = Vec::new();
            let mut pending = vec![trait_definition];
            let mut visited = Vec::new();
            while let Some(definition) = pending.pop() {
                if visited.contains(&definition.symbol) {
                    continue;
                }
                visited.push(definition.symbol);
                requirements.extend(
                    lowerer
                        .source_trees
                        .trait_machine_signatures(definition.machines)
                        .iter()
                        .filter(|requirement| requirement.name.as_str() == call.target.as_str()),
                );
                pending.extend(
                    lowerer
                        .source_trees
                        .trait_requirements(definition.requires)
                        .iter()
                        .filter_map(|required| {
                            lowerer.source_trees.roots.trait_definition(required.symbol)
                        }),
                );
            }
            let [requirement] = requirements.as_slice() else {
                return None;
            };
            requirement.storage.return_type.clone()
        }

        fn instantiate_declared_call_result(
            lowerer: &mut Lowerer,
            state: &resolved::state::State,
            types: &LocalTypes,
            call: &resolved::expression::TableCallExpression,
            target: symbols::SymbolHandle,
            result: typed::types::TypeReferenceHandle,
        ) -> Result<(), Diagnostic> {
            use crate::expressions::call_results::{
                declared_call_parameters, declared_call_type_parameters,
            };
            let parameters = declared_call_type_parameters(lowerer.source_trees, target).to_vec();
            if parameters.is_empty() {
                return Ok(());
            }
            let type_parameters: Vec<_> = parameters
                .iter()
                .filter(|parameter| {
                    matches!(parameter.kind, resolved::data::TypeParameterKind::Type)
                })
                .map(|parameter| parameter.symbol)
                .collect();
            let mut substitutions = Vec::new();
            for (parameter, argument) in parameters.iter().zip(call.machine_arguments.iter()) {
                if !matches!(parameter.kind, resolved::data::TypeParameterKind::Type) {
                    continue;
                }
                let selected = crate::expressions::expression::lower_static_machine_argument(
                    Some(lowerer.source_trees),
                    &mut lowerer.typed_trees,
                    lowerer.type_reference_exposure,
                    argument,
                )?;
                let concrete = lowerer
                    .typed_trees
                    .static_operand_type_reference(&selected)
                    .map_err(Diagnostic::error)?;
                substitutions.push((parameter.symbol, concrete));
            }
            let formal_parameters = declared_call_parameters(lowerer.source_trees, target).to_vec();
            let arguments = resolved_expressions(lowerer.source_trees)
                .expression_handles(call.arguments)
                .to_vec();
            for (formal, argument) in formal_parameters.iter().zip(arguments) {
                let Some(concrete) = infer_initializer_type(lowerer, state, types, argument)?
                else {
                    continue;
                };
                let formal = lower_type_reference_into_table(lowerer, &formal.type_reference)?;
                infer_declared_type_arguments(
                    &lowerer.typed_trees.type_reference_table,
                    formal,
                    concrete,
                    &type_parameters,
                    &mut substitutions,
                );
            }
            substitute_call_type_arguments(
                &mut lowerer.typed_trees,
                result,
                &substitutions,
                target,
                state.symbol,
            )
        }

        fn infer_declared_type_arguments(
            table: &typed::types::TypeReferenceTable,
            formal: typed::types::TypeReferenceHandle,
            concrete: typed::types::TypeReferenceHandle,
            parameters: &[symbols::SymbolHandle],
            substitutions: &mut Vec<(symbols::SymbolHandle, typed::types::TypeReferenceHandle)>,
        ) {
            use typed::types::TypeReferenceNode as Node;
            match (table.type_reference(formal), table.type_reference(concrete)) {
                (Node::Named { symbol, .. }, _) if parameters.contains(symbol) => {
                    if !substitutions
                        .iter()
                        .any(|(parameter, _)| parameter == symbol)
                    {
                        substitutions.push((*symbol, concrete));
                    }
                }
                (Node::Constrained { base_type, .. }, _) => infer_declared_type_arguments(
                    table,
                    *base_type,
                    concrete,
                    parameters,
                    substitutions,
                ),
                (_, Node::Constrained { base_type, .. }) => infer_declared_type_arguments(
                    table,
                    formal,
                    *base_type,
                    parameters,
                    substitutions,
                ),
                (
                    Node::Reference {
                        referee: formal, ..
                    },
                    Node::Reference {
                        referee: concrete, ..
                    },
                )
                | (
                    Node::Slice {
                        element_type: formal,
                    },
                    Node::Slice {
                        element_type: concrete,
                    },
                )
                | (
                    Node::FixedArray {
                        element_type: formal,
                        ..
                    },
                    Node::FixedArray {
                        element_type: concrete,
                        ..
                    },
                ) => infer_declared_type_arguments(
                    table,
                    *formal,
                    *concrete,
                    parameters,
                    substitutions,
                ),
                (
                    Node::Generic {
                        base_symbol: formal_symbol,
                        arguments: formal,
                        ..
                    },
                    Node::Generic {
                        base_symbol: concrete_symbol,
                        arguments: concrete,
                        ..
                    },
                ) if formal_symbol == concrete_symbol => {
                    for (formal, concrete) in table
                        .type_reference_handles(*formal)
                        .iter()
                        .zip(table.type_reference_handles(*concrete))
                    {
                        infer_declared_type_arguments(
                            table,
                            *formal,
                            *concrete,
                            parameters,
                            substitutions,
                        );
                    }
                }
                _ => {}
            }
        }

        fn substitute_call_type_arguments(
            program: &mut typed::TypedTrees,
            reference: typed::types::TypeReferenceHandle,
            substitutions: &[(symbols::SymbolHandle, typed::types::TypeReferenceHandle)],
            declaration: symbols::SymbolHandle,
            supplier: symbols::SymbolHandle,
        ) -> Result<(), Diagnostic> {
            // Apply the whole declared type once, including callable contracts
            // and alias obligations, without mutating shared child rows.
            let applied = program
                .apply_callable_type_in_scope(
                    reference,
                    substitutions,
                    &[],
                    declaration,
                    supplier,
                    &[],
                )
                .map_err(Diagnostic::error)?;
            program
                .type_reference_table
                .substitute_applied_from(reference, applied)
                .map_err(Diagnostic::error)
        }

        fn builtin_primitive_type(
            lowerer: &mut Lowerer,
            atom: symbols::BuiltinTypeAtom,
            spelling: &'static str,
        ) -> Result<Option<typed::types::TypeReferenceHandle>, Diagnostic> {
            let symbols = &lowerer.source_trees.symbols;
            let Some(symbol) = symbols
                .child_handles(symbols.root())
                .and_then(|mut children| {
                    children.find(|symbol| symbols.builtin_type_atom(*symbol) == Some(atom))
                })
            else {
                return Ok(None);
            };
            Ok(Some(lowerer.typed_trees.type_reference_table.insert(
                typed::types::TypeReferenceNode::Named {
                    symbol,
                    name: typed::name::Identifier::generated_static(spelling),
                },
            )))
        }
    }
    pub(crate) mod root_bindings {
        use super::arguments::lower_statement_expression;
        use crate::lowerer::Lowerer;
        use diagnostics::Diagnostic;
        use language_semantics::declaration_selection::{
            BuildSelectionRequest, RootBindingOperand, RootBindingRequest,
        };
        use typed_trees::statement::StatementNode;

        pub(crate) fn lower(
            lowerer: &mut Lowerer,
            binding: &symbol_resolved_trees::statement::RootBinding,
        ) -> Result<StatementNode, Diagnostic> {
            let described = binding.implementation_operand.is_valid();
            let mut arguments = vec![lower_statement_expression(lowerer, binding.receiver)?];
            let implementation = if described {
                arguments.push(lower_statement_expression(
                    lowerer,
                    binding.implementation_operand,
                )?);
                RootBindingOperand::Description
            } else {
                RootBindingOperand::AuthoredPath(product_path(&binding.implementation))
            };
            let expression = crate::expressions::build_selections::lower(
                lowerer.source_trees,
                &mut lowerer.typed_trees,
                BuildSelectionRequest::Entry(RootBindingRequest {
                    source_span: binding.source_span,
                    slot: product_path(&binding.slot),
                    implementation,
                }),
                Vec::new(),
                lowerer.source_owner,
                arguments,
                Default::default(),
            )?;
            Ok(StatementNode::Expression(expression))
        }

        fn product_path(members: &[symbol_resolved_trees::name::DiagnosticName]) -> String {
            let mut path = String::new();
            for member in members {
                if !path.is_empty() {
                    path.push_str("::");
                }
                path.push_str(member.as_str());
            }
            path
        }
    }
    pub(crate) mod transitions {
        use crate::expressions::exhaustiveness::closed_boolean_guard;
        use crate::lowerer::Lowerer;
        use crate::tables::{resolved_statements, statement_table_mut};
        use diagnostics::Diagnostic;
        use symbol_resolved_trees as resolved;
        use typed_trees as typed;

        use super::arguments::{
            lower_statement_argument_span, lower_statement_expression, lower_statement_path_members,
        };

        pub(crate) fn lower_transition_statement(
            lowerer: &mut Lowerer,
            transition: &resolved::statement::TableTransition,
            closes_run: bool,
        ) -> Result<typed::statement::TableTransition, Diagnostic> {
            let patterns = resolved_statements(lowerer.source_trees)
                .transition_patterns(transition.patterns)
                .iter()
                .map(|pattern| {
                    crate::expressions::expression::table::lower_transition_pattern(
                        lowerer.source_trees,
                        crate::tables::resolved_expressions(lowerer.source_trees),
                        &mut lowerer.typed_trees,
                        *pattern,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            let patterns =
                statement_table_mut(&mut lowerer.typed_trees).insert_transition_patterns(patterns);
            Ok(typed::statement::TableTransition {
                patterns,
                target: lower_transition_target(lowerer, transition.target)?,
                continuation: transition
                    .continuation
                    .is_valid()
                    .then(|| lower_transition_target(lowerer, transition.continuation))
                    .transpose()?
                    .unwrap_or_else(typed::statement::TransitionTargetHandle::invalid),
                guard: lower_transition_guard(
                    lowerer,
                    &transition.guard,
                    closes_run && !transition.continuation.is_valid(),
                )?,
                proof_selectors: statement_table_mut(&mut lowerer.typed_trees)
                    .insert_outcome_proof_selectors(
                        resolved_statements(lowerer.source_trees)
                            .outcome_proof_selectors(transition.proof_selectors)
                            .iter()
                            .map(|selector| typed::statement::OutcomeProofSelector {
                                output_field: crate::lowerer::name::lower_name(
                                    &selector.output_field,
                                ),
                                binding: crate::lowerer::name::lower_name(&selector.binding),
                            }),
                    ),
                exit: match transition.exit {
                    resolved::statement::TransitionExit::Ordinary => {
                        typed::statement::TransitionExit::Ordinary
                    }
                    resolved::statement::TransitionExit::Crash(cause) => {
                        typed::statement::TransitionExit::Crash(match cause {
                            resolved::signature::CrashCause::Trap => {
                                typed::signature::CrashCause::Trap
                            }
                            resolved::signature::CrashCause::Abort => {
                                typed::signature::CrashCause::Abort
                            }
                        })
                    }
                },
                source_span: transition.source_span,
            })
        }

        fn lower_transition_guard(
            lowerer: &mut Lowerer,
            guard: &resolved::statement::TransitionGuardNode,
            closes_run: bool,
        ) -> Result<typed::statement::TransitionGuardNode, Diagnostic> {
            match guard {
                resolved::statement::TransitionGuardNode::Always => {
                    Ok(typed::statement::TransitionGuardNode::Always)
                }
                resolved::statement::TransitionGuardNode::When(expression)
                    if closes_run
                        && closed_boolean_guard(lowerer.source_trees, *expression)
                            == Some(true) =>
                {
                    Ok(typed::statement::TransitionGuardNode::Always)
                }
                resolved::statement::TransitionGuardNode::When(expression) => {
                    Ok(typed::statement::TransitionGuardNode::When(
                        lower_statement_expression(lowerer, *expression)?,
                    ))
                }
            }
        }

        fn lower_transition_target(
            lowerer: &mut Lowerer,
            target: resolved::statement::TransitionTargetHandle,
        ) -> Result<typed::statement::TransitionTargetHandle, Diagnostic> {
            let target = match resolved_statements(lowerer.source_trees).transition_target(target) {
                resolved::statement::TransitionTargetNode::Named {
                    path,
                    arguments,
                    evidence_arguments,
                    source_span,
                    authored_call_selection,
                    operational_acknowledgement,
                } => {
                    let lowered_arguments = lower_statement_argument_span(lowerer, *arguments)?;

                    typed::statement::TransitionTargetNode::Named {
                        static_machine_parameter: symbols::SymbolHandle::invalid(),
                        path: typed::statement::TableNamePath {
                            unresolved_name: path.unresolved_name,
                            receiver_symbol: path.receiver_symbol,
                            members: lower_statement_path_members(lowerer, path.members),
                            head_symbol: path.head_symbol,
                            symbol: path.symbol,
                        },
                        arguments: lowered_arguments,
                        evidence_arguments: evidence_arguments
                            .iter()
                            .map(crate::lowerer::name::lower_name)
                            .collect::<Vec<_>>()
                            .into_boxed_slice(),
                        source_span: *source_span,
                        authored_call_selection: *authored_call_selection,
                        operational_acknowledgement: *operational_acknowledgement,
                    }
                }
                resolved::statement::TransitionTargetNode::Value(expression) => {
                    typed::statement::TransitionTargetNode::Value(lower_statement_expression(
                        lowerer,
                        *expression,
                    )?)
                }
                resolved::statement::TransitionTargetNode::Terminal => {
                    typed::statement::TransitionTargetNode::Terminal
                }
            };

            Ok(statement_table_mut(&mut lowerer.typed_trees).insert_transition_target(target))
        }
    }

    use crate::contracts::proof_facts::lower_statement_fact;
    use crate::expressions::expression::native_code_layout::result_type;
    use crate::lowerer::Lowerer;
    use crate::tables::{expression_table_mut, type_references};
    use crate::type_reference::lower_type_reference_handle_from_table;
    use diagnostics::Diagnostic;
    use symbol_resolved_trees as resolved;
    use typed::statement::TableLocalData;
    use typed_trees as typed;

    use self::arguments::lower_statement_expression;
    use self::calls::lower_call_statement;
    use self::initializer_type::infer_initializer_type;
    use self::transitions::lower_transition_statement;
    pub(crate) mod assembly {
        use super::{LocalTypes, lower_statement_node};
        use crate::contracts::proof_facts::lower_statement_fact;
        use crate::lowerer::Lowerer;
        use crate::tables::resolved_statements;
        use arena::Handle;
        use diagnostics::Diagnostic;
        use symbol_resolved_trees as resolved;
        use typed_trees::statement::{
            AssemblyBlock, AssemblyControlFlow, AssemblyControlPoint, AssemblyExit,
            AssemblyFactKind, StatementNode, TableAssemblyFact,
        };

        pub(crate) fn lower(
            lowerer: &mut Lowerer,
            state: &resolved::state::State,
            types: &mut LocalTypes,
            handle: Handle<resolved::statement::AssemblyControlFlow>,
        ) -> Result<Vec<StatementNode>, Diagnostic> {
            let declarations = &lowerer.source_trees.tables.declarations;
            let graph = declarations.assembly_graphs.get(handle);
            let mut blocks = Vec::new();
            let mut body = Vec::new();
            for block in declarations.assembly_blocks.span_or_empty(graph.blocks) {
                let start = body.len();
                let requires = declarations
                    .assembly_expressions
                    .span_or_empty(block.requires)
                    .iter()
                    .map(|expression| lower_statement_fact(lowerer, *expression))
                    .collect::<Result<Vec<_>, _>>()?;
                let ensures = declarations
                    .assembly_expressions
                    .span_or_empty(block.ensures)
                    .iter()
                    .map(|expression| lower_statement_fact(lowerer, *expression))
                    .collect::<Result<Vec<_>, _>>()?;
                body.push(StatementNode::Assembly(AssemblyControlPoint::Label(
                    Handle::invalid(),
                )));
                body.extend(requires.into_iter().map(|fact| {
                    StatementNode::AssemblyFact(TableAssemblyFact {
                        kind: AssemblyFactKind::Requires,
                        fact,
                    })
                }));
                let mut instructions = Vec::new();
                for instruction in
                    resolved_statements(lowerer.source_trees).statements(block.statement_nodes)
                {
                    let instruction =
                        lower_statement_node(lowerer, state, types, instruction, true)?;
                    types.record(&instruction)?;
                    instructions.push(instruction);
                }
                let transfer = if matches!(
                    block.exit,
                    resolved::statement::AssemblyExit::EnclosingTransition
                ) {
                    instructions.pop()
                } else {
                    None
                };
                body.extend(instructions);
                body.extend(ensures.into_iter().map(|fact| {
                    StatementNode::AssemblyFact(TableAssemblyFact {
                        kind: AssemblyFactKind::Ensures,
                        fact,
                    })
                }));
                body.extend(transfer);
                blocks.push(AssemblyBlock {
                    label: crate::lowerer::name::lower_name(&block.label),
                    start: u32::try_from(start)
                        .map_err(|_| Diagnostic::error("assembly statement window exceeds u32"))?,
                    count: u32::try_from(body.len() - start)
                        .map_err(|_| Diagnostic::error("assembly block exceeds u32"))?,
                    exit: match block.exit {
                        resolved::statement::AssemblyExit::Fallthrough => AssemblyExit::Fallthrough,
                        resolved::statement::AssemblyExit::Jump(_) => {
                            AssemblyExit::Jump(Handle::invalid())
                        }
                        resolved::statement::AssemblyExit::ZeroFlag(_, when_set) => {
                            AssemblyExit::ZeroFlag(Handle::invalid(), when_set)
                        }
                        resolved::statement::AssemblyExit::EnclosingTransition => {
                            AssemblyExit::EnclosingTransition
                        }
                    },
                });
            }
            let target_requirement = graph.target_requirement;
            let source_primitives = declarations
                .assembly_expressions
                .span_or_empty(graph.primitives)
                .to_vec();
            let source_scratch = graph.scratch;
            let source_blocks = graph.blocks;
            let mut scratch = [typed_trees::expression::ExpressionHandle::invalid();
                language_core::inline_assembly::ASM_SCRATCH_SLOT_COUNT];
            for (target, source) in scratch.iter_mut().zip(source_scratch) {
                if source.is_valid() {
                    *target = super::arguments::lower_statement_expression(lowerer, source)?;
                }
            }
            let primitives = source_primitives
                .into_iter()
                .map(|expression| super::arguments::lower_statement_expression(lowerer, expression))
                .collect::<Result<Vec<_>, _>>()?;
            let declarations = &lowerer.source_trees.tables.declarations;
            let table = &mut lowerer.typed_trees.statement_table;
            let primitives = table.insert_expression_handles(primitives);
            let blocks = table.insert_assembly_blocks(blocks);
            for (source_handle, target_handle) in source_blocks.handles().zip(blocks.handles()) {
                body[table.assembly_block(target_handle).start as usize] =
                    StatementNode::Assembly(AssemblyControlPoint::Label(target_handle));
                if let resolved::statement::AssemblyExit::Jump(target)
                | resolved::statement::AssemblyExit::ZeroFlag(target, _) =
                    declarations.assembly_blocks.get(source_handle).exit
                {
                    let Some(index) = source_blocks.handles().position(|handle| handle == target)
                    else {
                        return Err(Diagnostic::error(
                            "assembly jump escaped its declaring graph",
                        ));
                    };
                    table.assembly_block_mut(target_handle).exit =
                        if let resolved::statement::AssemblyExit::ZeroFlag(_, when_set) =
                            declarations.assembly_blocks.get(source_handle).exit
                        {
                            AssemblyExit::ZeroFlag(blocks.handle_at(index as u32), when_set)
                        } else {
                            AssemblyExit::Jump(blocks.handle_at(index as u32))
                        };
                }
            }
            let statement_count = u32::try_from(body.len())
                .map_err(|_| Diagnostic::error("assembly statement window exceeds u32"))?;
            let graph = table.insert_assembly(AssemblyControlFlow {
                target_requirement,
                blocks,
                statement_count,
                scratch,
                primitives,
            });
            let mut statements = Vec::with_capacity(body.len() + 1);
            statements.push(StatementNode::Assembly(AssemblyControlPoint::Entry(graph)));
            statements.extend(body);
            Ok(statements)
        }
    }

    pub(crate) fn lower_statement(
        lowerer: &mut Lowerer,
        state: &resolved::state::State,
        types: &mut LocalTypes,
        statement: &resolved::statement::StatementNode,
        closes_run: bool,
    ) -> Result<Vec<typed::statement::StatementNode>, Diagnostic> {
        if let resolved::statement::StatementNode::LocalData(local) = statement
            && let Some(operation) =
                crate::atomic_calls::update_initializer(lowerer.source_trees, local.initial_value)?
        {
            let zero = lowerer.typed_trees.expression_table.insert(
                typed::expression::ExpressionNode::Integer(
                    numerics::literals::IntegerLiteral::zero(),
                ),
            );
            let binding = lower_local(lowerer, state, types, local, Some(zero))?;
            let assignment = operation.assignment(lowerer, &binding, local.initial_value)?;
            let binding = typed::statement::StatementNode::LocalData(binding);
            types.record(&binding)?;
            return Ok(vec![binding, assignment]);
        }
        if let resolved::statement::StatementNode::Assembly(handle) = statement {
            assembly::lower(lowerer, state, types, *handle)
        } else {
            let statement = lower_statement_node(lowerer, state, types, statement, closes_run)?;
            types.record(&statement)?;
            Ok(vec![statement])
        }
    }

    fn lower_local(
        lowerer: &mut Lowerer,
        state: &resolved::state::State,
        types: &LocalTypes,
        local_data: &resolved::statement::TableLocalData,
        observed_result_slot: Option<typed::expression::ExpressionHandle>,
    ) -> Result<TableLocalData, Diagnostic> {
        let role = if observed_result_slot.is_some() {
            language_core::LocalBindingRole::AtomicObservedResult
        } else {
            local_data.role
        };
        let declared_is_unit = matches!(
            type_references(lowerer.source_trees).type_reference(local_data.type_reference),
            resolved::types::TypeReferenceNode::Unit
        );
        let type_reference = if declared_is_unit
            && local_data.initial_value.is_valid()
            && local_data.type_is_inferred
            && role.has_ordinary_initializer()
        {
            infer_initializer_type(lowerer, state, types, local_data.initial_value)?
                .unwrap_or_else(typed::types::TypeReferenceHandle::invalid)
        } else {
            lower_type_reference_handle_from_table(lowerer, local_data.type_reference)?
        };
        let roster = crate::tables::resolved_expressions(lowerer.source_trees)
            .match_fields(local_data.pattern_fields)
            .iter()
            .map(|field| {
                use resolved::expression::MatchFieldAction as Authored;
                use typed::expression::{MatchFieldAction, TableMatchField};
                TableMatchField {
                    component: field.component,
                    member: crate::lowerer::name::lower_name(&field.member),
                    field_symbol: field.field_symbol,
                    action: match &field.action {
                        Authored::Bind(binding) => {
                            MatchFieldAction::Bind(crate::lowerer::name::lower_name(binding))
                        }
                        Authored::Waive | Authored::Equal(_) => MatchFieldAction::Waive,
                    },
                }
            })
            .collect::<Vec<_>>();
        let pattern_fields = crate::tables::expression_table_mut(&mut lowerer.typed_trees)
            .insert_match_fields(roster);
        Ok(TableLocalData {
            pattern_fields,
            pattern_subject: symbols::SymbolHandle::invalid(),
            role,
            symbol: local_data.symbol,
            name: crate::lowerer::name::lower_name(&local_data.name),
            type_reference,
            initial_value: match observed_result_slot {
                Some(value) => value,
                None => local_data
                    .initial_value
                    .is_valid()
                    .then(|| lower_statement_expression(lowerer, local_data.initial_value))
                    .transpose()?
                    .unwrap_or_else(typed::expression::ExpressionHandle::invalid),
            },
            is_mutable: local_data.is_mutable,
            type_is_inferred: local_data.type_is_inferred,
            is_operand_capture: local_data.is_operand_capture,
            relevance: local_data.relevance,
            reflected_query_origin: None,
        })
    }

    fn lower_statement_node(
        lowerer: &mut Lowerer,
        state: &resolved::state::State,
        types: &LocalTypes,
        statement: &resolved::statement::StatementNode,
        closes_run: bool,
    ) -> Result<typed::statement::StatementNode, Diagnostic> {
        if let resolved::statement::StatementNode::Call(call) = statement
            && let Some(assignment) = crate::atomic_calls::store(lowerer, call)?
        {
            return Ok(assignment);
        }
        match statement {
            resolved::statement::StatementNode::Assembly(_) => Err(Diagnostic::error(
                "assembly typing requires its complete enclosing statement window",
            )),
            resolved::statement::StatementNode::RootBinding(binding) => {
                root_bindings::lower(lowerer, binding)
            }
            resolved::statement::StatementNode::AssemblyFact(fact) => {
                Ok(typed::statement::StatementNode::AssemblyFact(
                    typed::statement::TableAssemblyFact {
                        kind: match fact.kind {
                            resolved::statement::AssemblyFactKind::Requires => {
                                typed::statement::AssemblyFactKind::Requires
                            }
                            resolved::statement::AssemblyFactKind::Ensures => {
                                typed::statement::AssemblyFactKind::Ensures
                            }
                        },
                        fact: lower_statement_fact(lowerer, fact.expression)?,
                    },
                ))
            }
            resolved::statement::StatementNode::Assignment(assignment) => Ok(
                typed::statement::StatementNode::Assignment(typed::statement::TableAssignment {
                    target: lower_statement_expression(lowerer, assignment.target)?,
                    value: lower_statement_expression(lowerer, assignment.value)?,
                }),
            ),
            resolved::statement::StatementNode::Call(call)
                if !crate::runtime_callables::callee_is_value(
                    Some(lowerer.source_trees),
                    &lowerer.source_trees.tables.bodies.expressions,
                    call.callee,
                ) && lowerer
                    .source_trees
                    .symbols
                    .builtin_function_for_symbol(call.target_symbol)
                    == Some(symbols::BuiltinFunction::CodeLayout)
                    && call.receiver.is_empty() =>
            {
                let mut request =
                    crate::expressions::expression::native_code_layout::lower_request(
                        Some(lowerer.source_trees),
                        &mut lowerer.typed_trees,
                        lowerer.type_reference_exposure,
                        &call.machine_arguments,
                        call.arguments.len(),
                        call.evidence_arguments.len(),
                    )?;
                request.discards_result = true;
                let result = result_type(Some(lowerer.source_trees), &mut lowerer.typed_trees)?;
                let expression = lowerer.typed_trees.expression_table.insert(
                    typed::expression::ExpressionNode::Call(
                        typed::expression::TableCallExpression {
                            runtime_call: None,
                            authored_call_selection: call.authored_call_selection,
                            authored_call_binding: None,
                            authored_machine_application: Default::default(),
                            exact_coercion_target: Default::default(),
                            requirement_application: Default::default(),
                            receiver: Default::default(),
                            target_symbol: call.target_symbol,
                            static_machine_parameter: Default::default(),
                            target: crate::lowerer::name::lower_name(&call.target),
                            static_requirement_dispatch: None,
                            machine_arguments: Box::default(),
                            quotient_operation: None,
                            private_layout_operation: None,
                            installed_interrupt_operation: None,
                            layout_query: None,
                            native_code_layout: Some(request),
                            build_selection: Default::default(),
                            arguments: Default::default(),
                            evidence_arguments: Box::default(),
                            operational_acknowledgement: call.operational_acknowledgement,
                        },
                    ),
                );
                expression_table_mut(&mut lowerer.typed_trees)
                    .set_constructed_value_type_reference(expression, result);
                expression_table_mut(&mut lowerer.typed_trees)
                    .set_source_span(expression, call.target.source_span());
                Ok(typed::statement::StatementNode::Expression(expression))
            }
            resolved::statement::StatementNode::Call(call) => lower_call_statement(lowerer, call),
            resolved::statement::StatementNode::ProofOutputBindingStatement(_) => {
                unreachable!(
                    "evidence packages are classified out before runtime statement lowering"
                )
            }
            resolved::statement::StatementNode::Expression(expression) => {
                Ok(typed::statement::StatementNode::Expression(
                    lower_statement_expression(lowerer, *expression)?,
                ))
            }
            resolved::statement::StatementNode::LocalData(local) => {
                lower_local(lowerer, state, types, local, None)
                    .map(typed::statement::StatementNode::LocalData)
            }
            resolved::statement::StatementNode::Transition(transition) => {
                Ok(typed::statement::StatementNode::Transition(
                    lower_transition_statement(lowerer, transition, closes_run)?,
                ))
            }
        }
    }
}

#[cfg(test)]
mod boolean_result_tests {
    #[test]
    fn boolean_locals_and_inferred_dispatch_subjects_keep_their_real_type() {
        let source = r#"
            data Choice { case A; case B; }
            domain u64::Small requires self <= 8;
            machine case_subject(value: Choice) -> u64 {
                transition value in Choice::A { true -> 41u64 _ -> 0u64 }
            }
            machine domain_subject(value: u64) -> u64 {
                transition value in Small { true -> 41u64 _ -> 0u64 }
            }
            machine negated_subject(value: bool) -> u64 {
                transition !value { true -> 41u64 _ -> 0u64 }
            }
            machine ordinary(value: Choice, bit: bool, count: u64) -> bool {
                let membership: bool = value in Choice::A;
                let domain_membership: bool = count in Small;
                let literal: bool = true;
                let negated: bool = !bit;
                let comparison: bool = count <= 8u64;
                comparison
            }
        "#;
        let tokens = crate::frontend_fixture::Lexer::new(source)
            .tokenize()
            .unwrap();
        let syntax = crate::frontend_fixture::parse_syntax_trees(&tokens).unwrap();
        let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
            syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(syntax),
        )
        .unwrap();
        let typed = crate::lower_symbol_resolved_trees(&resolved).unwrap();
        let mut captured = 0;
        let mut authored = 0;
        for machine in typed.machines() {
            for state in typed.machine_states(machine) {
                for statement in typed.statement_table.statements(state.statement_nodes) {
                    let typed_trees::statement::StatementNode::LocalData(local) = statement else {
                        continue;
                    };
                    if local.role == language_core::LocalBindingRole::DispatchSubject {
                        captured += 1;
                    } else if [
                        "membership",
                        "domain_membership",
                        "literal",
                        "negated",
                        "comparison",
                    ]
                    .contains(&local.name.as_str())
                    {
                        authored += 1;
                    } else {
                        continue;
                    }
                    let typed_trees::types::TypeReferenceNode::Named { symbol, .. } = typed
                        .type_reference_table
                        .type_reference(local.type_reference)
                    else {
                        panic!("{} retains its actual Bool type", local.name.as_str());
                    };
                    assert_eq!(
                        typed.symbols.builtin_type_atom(*symbol),
                        Some(symbols::BuiltinTypeAtom::Bool)
                    );
                }
            }
        }
        assert_eq!(captured, 3);
        assert_eq!(authored, 5);
    }
}

#[cfg(test)]
mod scoped_field_tests {
    use crate::qualified_operator_home_tests::packages;
    use typed_trees::statement::StatementNode;
    use typed_trees::types::TypeReferenceNode;

    #[test]
    fn field_initializer_uses_the_explicit_receivers_package() {
        let left = "data Packet { value: u32; } machine read_left(packet: &Packet) -> u32 { let from_left = packet.value; from_left }";
        let right = "data Packet { value: u64; } machine read_right(packet: &Packet) -> u64 { let from_right = packet.value; from_right }";
        let program =
            packages(&[(1, "left.omg", left), (2, "right.omg", right)], &[], &[]).unwrap();
        let locals = program
            .machines()
            .iter()
            .flat_map(|machine| program.machine_states(machine))
            .flat_map(|state| program.statement_table.statements(state.statement_nodes))
            .filter_map(|node| match node {
                StatementNode::LocalData(local) => Some(local),
                _ => None,
            });
        let mut observed = locals
            .filter_map(|local| {
                let expected = match local.name.as_str() {
                    "from_left" => "u32",
                    "from_right" => "u64",
                    _ => return None,
                };
                let TypeReferenceNode::Named { name, .. } = program
                    .type_reference_table
                    .type_reference(local.type_reference)
                else {
                    panic!("{} lost the selected field's type", local.name.as_str());
                };
                assert_eq!(name.as_str(), expected);
                Some(local.name.as_str())
            })
            .collect::<Vec<_>>();
        observed.sort_unstable();
        assert_eq!(observed, ["from_left", "from_right"]);
    }
}

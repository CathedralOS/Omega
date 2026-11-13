/*
Call result typing, captured product operands, construction destinations, build
and product selections, installed interrupts, and qualification casts.
*/

pub(crate) mod build_selections {
    use diagnostics::Diagnostic;
    use language_semantics::declaration_selection::{
        BuildOperation, BuildSelectionRequest, RootBindingOperand,
    };
    use typed_trees::expression::{
        ExpressionHandle, ExpressionNode, TableCallExpression, TableNamePath,
    };

    pub(crate) fn lower(
        source: &symbol_resolved_trees::SymbolResolvedTrees,
        target: &mut typed_trees::TypedTrees,
        request: typed_trees::expression::BuildSelectionRequest,
        mut arguments: Vec<ExpressionHandle>,
        acknowledgement: language_semantics::CallOperationalAcknowledgement,
    ) -> Result<ExpressionHandle, Diagnostic> {
        let source_span = request.source_span();
        let mut operand_selections = Vec::new();
        for operand in request.designated_operands() {
            for (_, selection) in operand
                .application
                .as_ref()
                .into_iter()
                .flat_map(|graph| graph.selections.iter())
            {
                let kind = selection.kind.ok_or_else(|| {
                    Diagnostic::error("designated operand lost its authored selection kind")
                })?;
                operand_selections.push(
                    target
                        .authored_declaration_selections()
                        .source_coordinate_occurrence(
                            selection.source_span,
                            selection.exposure,
                            kind,
                        )
                        .map_err(|reason| {
                            Diagnostic::error(format!(
                            "Build operand cannot rejoin its {kind:?} selection ({:?}): {reason}",
                            selection.exposure
                        ))
                        .with_source_span(selection.source_span)
                        })?,
                );
            }
        }
        let operation = match (&request, arguments.len()) {
            (BuildSelectionRequest::ProviderReadiness(_), 1) => "await_provider",
            (BuildSelectionRequest::Exclusion(_), 2) => "exclude",
            (BuildSelectionRequest::Entry(binding), 1)
                if matches!(binding.implementation, RootBindingOperand::AuthoredPath(_)) =>
            {
                "bind"
            }
            (BuildSelectionRequest::Entry(binding), 2)
                if matches!(binding.implementation, RootBindingOperand::Description) =>
            {
                "bind_description"
            }
            (BuildSelectionRequest::Declaration(binding), count) => binding
                .operation
                .product_recording_operation()
                .filter(|(_, arity)| {
                    count == *arity
                        || (binding.operation == BuildOperation::ProviderSelection
                            && count.checked_add(1) == Some(*arity))
                })
                .map(|(name, _)| name)
                .ok_or_else(|| {
                    Diagnostic::error("Build selection has unexpected ordinary operands")
                        .with_source_span(source_span)
                })?,
            _ => {
                return Err(
                    Diagnostic::error("Build selection has unexpected ordinary operands")
                        .with_source_span(source_span),
                );
            }
        };
        let owner = source
            .traits
            .iter()
            .find(|definition| {
                definition.is_boundary
                    && definition.name.as_str() == "BuildBindingOperations"
                    && source
                        .symbols
                        .symbol_source_span(definition.symbol)
                        .and_then(|span| source.symbols.source_file(span))
                        .is_some_and(|file| {
                            file.origin == source::SourceOrigin::Toolchain
                                && file.path == std::path::Path::new("<build-prelude>")
                        })
            })
            .ok_or_else(|| {
                Diagnostic::error("Build selection requires its exact toolchain protocol")
                    .with_source_span(source_span)
            })?;
        let find = |name| {
            source
                .trait_machine_signatures(owner.machines)
                .iter()
                .find(|signature| signature.name.as_str() == name)
                .ok_or_else(|| {
                    Diagnostic::error(format!(
                        "Build selection protocol has no `{name}` operation"
                    ))
                    .with_source_span(source_span)
                })
        };
        // Test-group registration is authorized by the Build's own `tests`
        // facet, the operand it was authored on; every other selection by the
        // original Build value.
        let preparation = find(match &request {
            BuildSelectionRequest::Declaration(binding) if binding.operation.uses_tests_facet() => {
                "prepare_tests"
            }
            _ => "prepare",
        })?;
        let commit = find(operation)?;
        let value_helpers = match &request {
            BuildSelectionRequest::Exclusion(request) => vec![(1, request.vocabulary.helper())],
            _ if operation == "bind_native" => vec![(1, "build_selection_target")],
            _ if operation == "select_provider" => {
                let mut helpers = vec![(1, "build_selection_target")];
                if arguments.len() == 3 {
                    helpers.push((2, "build_selection_independent"));
                }
                helpers
            }
            _ => Vec::new(),
        };
        let table = &mut target.expression_table;
        let mut members = Default::default();
        let mut member_symbols = Default::default();
        table.push_name_path_member(&mut members, crate::lowerer::name::lower_name(&owner.name));
        table.push_name_path_member_symbol(&mut member_symbols, owner.symbol);
        let receiver = table.insert(ExpressionNode::Name(TableNamePath::new(
            members,
            member_symbols,
            owner.symbol,
            owner.symbol,
        )));
        let build_selection = table.insert_build_selection_request(request);
        if operation == "select_provider" && arguments.len() == 2 {
            arguments.push(table.insert(ExpressionNode::Boolean(false)));
        }
        let mut emit = |symbol,
                        name: typed_trees::name::Identifier,
                        receiver,
                        arguments: Vec<ExpressionHandle>,
                        context,
                        operational_acknowledgement| {
            let arguments = table.insert_expression_handles(arguments);
            let expression = table.insert(ExpressionNode::Call(TableCallExpression {
                runtime_call: None,
                authored_call_selection: None,
                authored_call_binding: None,
                authored_machine_application: Default::default(),
                exact_coercion_target: Default::default(),
                requirement_application: Default::default(),
                receiver,
                target_symbol: symbol,
                static_machine_parameter: Default::default(),
                target: name,
                static_requirement_dispatch: None,
                machine_arguments: Box::default(),
                quotient_operation: None,
                private_layout_operation: None,
                installed_interrupt_operation: None,
                layout_query: None,
                native_code_layout: None,
                build_selection: context,
                arguments,
                evidence_arguments: Box::default(),
                operational_acknowledgement,
            }));
            table.set_source_span(expression, source_span);
            expression
        };
        for (position, helper_name) in value_helpers {
            let helper = source
                .machines
                .iter()
                .find(|machine| {
                    machine.name.as_str() == helper_name
                        && source.symbols.is_build_prelude_symbol(machine.symbol)
                })
                .and_then(|machine| source.machine_state_handles(machine.states).first())
                .map(|state| source.machine_state(*state))
                .ok_or_else(|| {
                    Diagnostic::error(format!(
                        "Build selection has no checked value helper `{helper_name}`"
                    ))
                    .with_source_span(source_span)
                })?;
            arguments[position] = emit(
                helper.symbol,
                crate::lowerer::name::lower_name(&helper.name),
                ExpressionHandle::invalid(),
                vec![arguments[position]],
                Default::default(),
                Default::default(),
            );
        }
        arguments[0] = emit(
            preparation.symbol,
            crate::lowerer::name::lower_name(&preparation.name),
            receiver,
            vec![arguments[0]],
            build_selection,
            Default::default(),
        );
        let committed = emit(
            commit.symbol,
            crate::lowerer::name::lower_name(&commit.name),
            receiver,
            arguments,
            build_selection,
            acknowledgement,
        );
        table.attach_authored_selection_occurrences(committed, operand_selections);
        Ok(committed)
    }
}

pub(crate) mod call_results {
    use resolved::types::TypeReference;
    use symbol_resolved_trees as resolved;
    use symbols::{SymbolHandle, SymbolKind};
    use typed_trees::expression::{ExpressionHandle, ExpressionNode, ExpressionTable};

    pub(crate) fn declared_call_state(
        program: &resolved::SymbolResolvedTrees,
        target: SymbolHandle,
    ) -> Option<&resolved::state::State> {
        if !target.is_valid() || program.symbols.get(target).kind != SymbolKind::State {
            return None;
        }
        program
            .tables
            .declarations
            .machine_states
            .iter()
            .find(|(_, state)| state.symbol == target)
            .map(|(_, state)| state)
    }

    pub(crate) fn declared_call_return(
        program: &resolved::SymbolResolvedTrees,
        target: SymbolHandle,
    ) -> Option<&TypeReference> {
        if !target.is_valid() {
            return None;
        }
        match program.symbols.get(target).kind {
            SymbolKind::State => declared_call_state(program, target)
                .and_then(|state| state.return_type.as_ref())
                .or_else(|| {
                    program.traits.iter().find_map(|definition| {
                        program
                            .trait_machine_signatures(definition.machines)
                            .iter()
                            .find(|signature| signature.symbol == target)
                            .and_then(|signature| signature.return_type.as_ref())
                    })
                }),
            SymbolKind::Operator => program
                .operators
                .iter()
                .find(|operator| operator.symbol == target)?
                .return_type
                .as_ref(),
            _ => None,
        }
    }

    pub(crate) fn declared_call_type_parameters(
        program: &resolved::SymbolResolvedTrees,
        target: SymbolHandle,
    ) -> &[resolved::data::TypeParameter] {
        if !target.is_valid() {
            return &[];
        }
        match program.symbols.get(target).kind {
            SymbolKind::State => program
                .machine(program.symbols.get(target).parent)
                .map(|machine| program.machine_type_parameters(machine))
                .unwrap_or_default(),
            SymbolKind::Operator => program
                .operators
                .iter()
                .find(|operator| operator.symbol == target)
                .map(|operator| program.data_type_parameters(operator.type_parameters))
                .unwrap_or_default(),
            _ => &[],
        }
    }

    pub(crate) fn declared_call_parameters(
        program: &resolved::SymbolResolvedTrees,
        target: SymbolHandle,
    ) -> &[resolved::signature::StateParameter] {
        if !target.is_valid() {
            return &[];
        }
        match program.symbols.get(target).kind {
            SymbolKind::State => declared_call_state(program, target)
                .map(|state| program.state_parameters(state.parameters))
                .unwrap_or_default(),
            SymbolKind::Operator => program
                .operators
                .iter()
                .find(|operator| operator.symbol == target)
                .map(|operator| program.state_parameters(operator.parameters))
                .unwrap_or_default(),
            _ => &[],
        }
    }

    pub(crate) fn peel<'program>(
        program: &'program resolved::SymbolResolvedTrees,
        mut reference: &'program TypeReference,
    ) -> &'program TypeReference {
        loop {
            reference = match reference {
                TypeReference::Reference(reference) => {
                    program.child_type_reference(reference.referee)
                }
                TypeReference::Constrained(reference) => {
                    program.child_type_reference(reference.base_type)
                }
                _ => return reference,
            };
        }
    }

    pub(crate) fn computed_receiver_method_target(
        program: &resolved::SymbolResolvedTrees,
        expressions: &ExpressionTable,
        mut receiver: ExpressionHandle,
        target: &resolved::name::DiagnosticName,
    ) -> SymbolHandle {
        let mut resolve = || {
            let mut projections = Vec::new();
            let call = loop {
                receiver = match expressions.expression(receiver) {
                    node @ ExpressionNode::Member(member) => {
                        projections.push(node);
                        member.receiver
                    }
                    node @ ExpressionNode::Indexed(indexed) => {
                        projections.push(node);
                        indexed.collection
                    }
                    ExpressionNode::Borrow(borrow) => borrow.target,
                    ExpressionNode::Call(call) => break call,
                    _ => return None,
                };
            };
            let state = declared_call_state(program, call.target_symbol)?;
            let owner = program.symbols.get(state.symbol).parent;
            let producer = program.machine(owner)?;
            if call.receiver.is_valid() != producer.attached_data.is_some() {
                return None;
            }
            let mut reference = state.return_type.as_ref()?;
            for projection in projections.into_iter().rev() {
                reference = match projection {
                    ExpressionNode::Indexed(_) => {
                        let element = match peel(program, reference) {
                            TypeReference::FixedArray(array) => array.element_type,
                            TypeReference::Slice(slice) => slice.element_type,
                            _ => return None,
                        };
                        if !element.is_valid() {
                            return None;
                        }
                        program.child_type_reference(element)
                    }
                    ExpressionNode::Member(member) => {
                        let TypeReference::Named { symbol, .. } = peel(program, reference) else {
                            return None;
                        };
                        let definition = program
                            .data_definitions
                            .iter()
                            .find(|definition| definition.symbol == *symbol && symbol.is_valid())?;
                        let mut fields = program
                            .data_members(definition.storage.members)
                            .iter()
                            .filter_map(|node| match node {
                                resolved::data::DataMember::Field(field)
                                    if field.name.as_str() == member.member.as_str() =>
                                {
                                    Some(field)
                                }
                                _ => None,
                            });
                        let field = fields.next()?;
                        if fields.next().is_some()
                            || (member.member_symbol.is_valid()
                                && member.member_symbol != field.symbol)
                        {
                            return None;
                        }
                        &field.type_reference
                    }
                    _ => return None,
                };
            }
            Some(declared_type_method_target(program, reference, target))
        };
        resolve().unwrap_or_else(SymbolHandle::invalid)
    }

    pub(crate) fn declared_type_method_target(
        program: &resolved::SymbolResolvedTrees,
        reference: &TypeReference,
        target: &resolved::name::DiagnosticName,
    ) -> SymbolHandle {
        let TypeReference::Named { symbol: owner, .. } = peel(program, reference) else {
            return SymbolHandle::invalid();
        };
        if !owner.is_valid() || program.symbols.get(*owner).kind != SymbolKind::Data {
            return SymbolHandle::invalid();
        }
        let qualified = format!("{}::{}", program.symbols.name(*owner), target.as_str());
        let symbols::SymbolLookup::Unique(machine_symbol) = program
            .symbols
            .lookup_signature_free_top_level_from_source_matching(
                &qualified,
                &[SymbolKind::Machine],
                target.source_span(),
                |candidate| {
                    program.machines.iter().any(|machine| {
                        machine.symbol == candidate && machine.attached_data_symbol == *owner
                    })
                },
            )
        else {
            return SymbolHandle::invalid();
        };
        let Some(machine) = program.machines.iter().find(|machine| {
            machine.symbol == machine_symbol && machine.attached_data_symbol == *owner
        }) else {
            return SymbolHandle::invalid();
        };
        let mut states = program
            .machine_state_handles(machine.states)
            .iter()
            .map(|handle| program.machine_state(*handle))
            .filter(|state| state.name.as_str() == target.as_str());
        let Some(state) = states.next() else {
            return SymbolHandle::invalid();
        };
        if states.next().is_none() {
            state.symbol
        } else {
            SymbolHandle::invalid()
        }
    }
}

pub(crate) mod capture_product_operands {

    use crate::tables::{resolved_expressions, type_constraints};
    use arena::{Handle, HandleSpan};
    use diagnostics::Diagnostic;
    use language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure as SelectionExposure;
    use language_semantics::declaration_selection::product_operands::*;
    use symbol_resolved_trees::name::DiagnosticName;
    use symbol_resolved_trees::{
        SymbolResolvedTrees, expression as source_expression, types as source_types,
    };
    use symbols::SymbolHandle;

    pub(crate) fn capture(
        source: &SymbolResolvedTrees,
        typed: &mut typed_trees::TypedTrees,
        exposure: Option<SelectionExposure>,
        occurrences: &[language_semantics::declaration_selection::AuthoredDeclarationSelectionOccurrenceId],
        argument: &source_expression::StaticMachineArgument,
    ) -> Result<ProductStaticOperandGraph, Diagnostic> {
        let mut capture = Capture {
            source,
            typed,
            exposure,
            graph: ProductStaticOperandGraph::default(),
            expressions: Vec::new(),
            captures: Vec::new(),
            active: Vec::new(),
        };
        capture.graph.root = capture.operand(argument, occurrences, true)?;
        Ok(capture.graph)
    }
    struct Capture<'a> {
        source: &'a SymbolResolvedTrees,
        typed: &'a mut typed_trees::TypedTrees,
        exposure: Option<SelectionExposure>,
        graph: ProductStaticOperandGraph,
        expressions: Vec<(
            source_expression::ExpressionHandle,
            Handle<ProductOperandExpression>,
        )>,
        captures: Vec<(
            Handle<source_expression::MatchSubjectCapture>,
            Handle<ProductMatchSubject>,
        )>,
        active: Vec<source_expression::ExpressionHandle>,
    }
    impl Capture<'_> {
        fn retain_coordinate(
            &mut self,
            occurrence: language_semantics::declaration_selection::AuthoredDeclarationSelectionOccurrenceId,
            from_type_producer: bool,
        ) -> Result<Handle<ProductOperandSelection>, Diagnostic> {
            let ledger = if from_type_producer {
                self.typed.authored_declaration_selections()
            } else {
                self.source.authored_declaration_selections()
            };
            let selection = ledger.get(occurrence).ok_or_else(|| {
                Diagnostic::error("original static input retains an unknown selection")
            })?;
            let rejoined = ledger
                .source_coordinate_occurrence(
                    selection.source_span(),
                    selection.exposure(),
                    selection.kind(),
                )
                .map_err(|reason| {
                    Diagnostic::error(format!(
                        "captured static operand cannot rejoin its {:?} selection ({:?}): {reason}",
                        selection.kind(),
                        selection.exposure()
                    ))
                    .with_source_span(selection.source_span())
                })?;
            if rejoined != occurrence {
                return Err(Diagnostic::error(
                    "original static input changed its exact selection attachment",
                ));
            }
            Ok(self.graph.selections.insert(ProductOperandSelection {
                source_span: selection.source_span(),
                exposure: selection.exposure(),
                kind: Some(selection.kind()),
            }))
        }
        fn type_name(
            &mut self,
            name: &DiagnosticName,
            symbol: SymbolHandle,
            kind: language_semantics::declaration_selection::AuthoredDeclarationSelectionKind,
        ) -> Result<Handle<ProductOperandName>, Diagnostic> {
            let occurrence = crate::type_reference::retain_type_reference_recipe_selection(
                self.source,
                self.typed,
                name,
                symbol,
                self.exposure,
                kind,
            )?;
            let mut retained = self.named(name, symbol);
            if let Some(occurrence) = occurrence {
                retained.authored_selection = self.retain_coordinate(occurrence, true)?;
            }
            Ok(self.graph.names.insert(retained))
        }
        fn operand_selection(
            &mut self,
            argument: &source_expression::StaticMachineArgument,
            occurrences: &[language_semantics::declaration_selection::AuthoredDeclarationSelectionOccurrenceId],
        ) -> Result<Handle<ProductOperandSelection>, Diagnostic> {
            use language_semantics::declaration_selection::AuthoredDeclarationSelectionKind as Kind;
            if argument.path.is_empty() || !argument.path.iter().any(|name| name.is_source_backed())
            {
                return Ok(Handle::invalid());
            }
            if argument.symbol.is_valid()
                && matches!(
                    self.source.symbols.get(argument.symbol).kind,
                    symbols::SymbolKind::Local
                        | symbols::SymbolKind::Parameter
                        | symbols::SymbolKind::TypeParameter
                        | symbols::SymbolKind::ConformanceParameter
                        | symbols::SymbolKind::MachineParameter
                        | symbols::SymbolKind::PropositionParameter
                        | symbols::SymbolKind::PropositionMachineParameter
                )
            {
                return Ok(Handle::invalid());
            }
            if !argument.symbol.is_valid()
                && argument.application.is_none()
                && argument.evidence_projection.is_none()
                && let [name] = argument.path.as_ref()
                && language_semantics::const_value::boolean_literal_spelling(name.as_str())
                    .is_some()
            {
                return Ok(Handle::invalid());
            }
            let first = argument.path.first().expect("nonempty path").source_span();
            let last = argument.path.last().expect("nonempty path").source_span();
            if first.source_id != last.source_id {
                return Err(Diagnostic::error(
                    "original static input path crosses source owners",
                ));
            }
            let span = source::SourceSpan::new(
                first.source_id,
                source::Span::new(first.span.start, last.span.end),
            );
            let mut matching = occurrences.iter().copied().filter(|occurrence| {
                self.source
                    .authored_declaration_selections()
                    .get(*occurrence)
                    .is_some_and(|selection| {
                        selection.source_span() == span
                            && matches!(selection.kind(), Kind::StaticArgument | Kind::Conformance)
                    })
            });
            let occurrence = matching.next().ok_or_else(|| {
                Diagnostic::error(
                    "original static input has no exact original-call selection attachment",
                )
                .with_source_span(span)
            })?;
            if matching.next().is_some() {
                return Err(Diagnostic::error(
                    "original static input has ambiguous original-call selection attachments",
                )
                .with_source_span(span));
            }
            self.retain_coordinate(occurrence, false)
        }
        fn declaration(&self, symbol: SymbolHandle) -> Option<source::SourceSpan> {
            symbol
                .is_valid()
                .then(|| self.source.symbols.symbol_provenance_source_span(symbol))
                .flatten()
        }
        fn named(&self, name: &DiagnosticName, symbol: SymbolHandle) -> ProductOperandName {
            ProductOperandName {
                authored_selection: Handle::invalid(),
                text: name.as_str().to_owned(),
                source_span: name.source_span(),
                declaration: self.declaration(symbol),
                builtin_type: symbol
                    .is_valid()
                    .then(|| self.source.symbols.builtin_type_atom(symbol))
                    .flatten(),
                builtin_function: symbol
                    .is_valid()
                    .then(|| self.source.symbols.builtin_function_for_symbol(symbol))
                    .flatten(),
            }
        }
        fn name(
            &mut self,
            name: &DiagnosticName,
            symbol: SymbolHandle,
        ) -> Handle<ProductOperandName> {
            let value = self.named(name, symbol);
            self.graph.names.insert(value)
        }
        fn names(
            &mut self,
            names: &[DiagnosticName],
            last_symbol: SymbolHandle,
        ) -> HandleSpan<ProductOperandName> {
            let names = names
                .iter()
                .enumerate()
                .map(|(index, name)| {
                    self.named(
                        name,
                        if index + 1 == names.len() {
                            last_symbol
                        } else {
                            SymbolHandle::invalid()
                        },
                    )
                })
                .collect::<Vec<_>>();
            self.graph.names.insert_many(names)
        }
        fn operand(
            &mut self,
            argument: &source_expression::StaticMachineArgument,
            occurrences: &[language_semantics::declaration_selection::AuthoredDeclarationSelectionOccurrenceId],
            descriptor: bool,
        ) -> Result<Handle<ProductStaticOperand>, Diagnostic> {
            let path = self.names(&argument.path, argument.symbol);
            let structural_type = if argument.type_reference.is_valid() {
                self.ty_handle(argument.type_reference)?
            } else {
                Handle::invalid()
            };
            let application = if let Some(application) = &argument.application {
                let source = self.source;
                let lifetimes =
                    self.names(&application.lifetime_arguments, SymbolHandle::invalid());
                let arguments = application
                    .arguments
                    .iter()
                    .map(|argument| self.operand(argument, occurrences, false))
                    .collect::<Result<Vec<_>, _>>()?;
                let arguments = self.graph.static_arguments.insert_many(arguments);
                let member = if application.member.is_valid() {
                    let member = resolved_expressions(source)
                        .static_member(application.member)
                        .ok_or_else(|| {
                            Diagnostic::error("designated operand has a foreign member handle")
                        })?;
                    self.operand(member, occurrences, false)?
                } else {
                    Handle::invalid()
                };
                self.graph.applications.insert(ProductStaticApplication {
                    lifetimes,
                    arguments,
                    member,
                })
            } else {
                Handle::invalid()
            };
            let projection = argument.evidence_projection.as_ref().map(|projection| {
                (
                    self.name(&projection.term, SymbolHandle::invalid()),
                    self.name(&projection.member, SymbolHandle::invalid()),
                )
            });
            let authored_selection = if descriptor {
                Handle::invalid()
            } else {
                self.operand_selection(argument, occurrences)?
            };
            Ok(self.graph.operands.insert(ProductStaticOperand {
                authored_selection,
                path,
                application,
                structural_type,
                integer: argument.const_literal.clone(),
                projection,
                declaration: self.declaration(argument.symbol),
            }))
        }
        fn ty_handle(
            &mut self,
            handle: Handle<source_types::TypeReference>,
        ) -> Result<Handle<ProductOperandType>, Diagnostic> {
            let source = self.source;
            self.ty(source.child_type_reference(handle))
        }
        fn types(
            &mut self,
            span: HandleSpan<source_types::TypeReference>,
        ) -> Result<HandleSpan<Handle<ProductOperandType>>, Diagnostic> {
            let source = self.source;
            let values = source
                .child_type_references(span)
                .iter()
                .map(|reference| self.ty(reference))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(self.graph.type_arguments.insert_many(values))
        }
        fn ty(
            &mut self,
            reference: &source_types::TypeReference,
        ) -> Result<Handle<ProductOperandType>, Diagnostic> {
            use source_types::TypeReference as Source;
            let source = self.source;
            let value = match reference {
            Source::Unit => ProductOperandType::Unit,
            Source::Named { name, symbol, .. } => {
                ProductOperandType::Named(self.type_name(name, *symbol, language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::TypeReference)?)
            }
            Source::Generic(value) => ProductOperandType::Generic {
                name: self.type_name(&value.base_name, value.base_symbol, language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::TypeReference)?,
                lifetimes: self.names(&value.lifetime_arguments, SymbolHandle::invalid()),
                arguments: self.types(value.arguments)?,
            },
            Source::Reference(value) => ProductOperandType::Reference {
                referee: self.ty_handle(value.referee)?,
                access: value.access,
                lifetime: value
                    .lifetime
                    .as_ref()
                    .map(|name| self.name(name, SymbolHandle::invalid())),
            },
            Source::Slice(value) => ProductOperandType::Slice(self.ty_handle(value.element_type)?),
            Source::FixedArray(value) => ProductOperandType::Array {
                element: self.ty_handle(value.element_type)?,
                length: match &value.length {
                    source_types::FixedArrayLength::Literal(value) => {
                        ProductArrayLength::Literal(*value)
                    }
                    source_types::FixedArrayLength::Expression(value) => {
                        ProductArrayLength::Expression(self.ty_handle(*value)?)
                    }
                    source_types::FixedArrayLength::ConstParameter { symbol, name } => {
                        ProductArrayLength::Parameter(self.type_name(name, *symbol, language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::StaticPathSegment)?)
                    }
                    source_types::FixedArrayLength::ConstCall { machine, name } => {
                        ProductArrayLength::Call(self.type_name(name, *machine, language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::StaticPathSegment)?)
                    }
                },
            },
            Source::Constrained(value) => {
                let base = self.ty_handle(value.base_type)?;
                let constraints = type_constraints(source).span_or_empty(value.constraints).iter().map(|constraint| self.constraint(constraint)).collect::<Result<Vec<_>, _>>()?;
                ProductOperandType::Constrained {
                    base,
                    constraints: self.graph.constraints.insert_many(constraints),
                }
            }
            Source::DynamicTrait {
                symbol,
                name,
                arguments,
                ..
            } => ProductOperandType::Dynamic {
                name: self.type_name(name, *symbol, language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::TypeReference)?,
                arguments: self.types(*arguments)?,
            },
            Source::ConstExpression(value) => {
                ProductOperandType::ConstExpression(self.expression(value.expression)?)
            }
            Source::Callable { .. } => {
                return Err(Diagnostic::error("captured Product static type cannot yet retain a complete scoped callable contract"));
            }
            Source::AliasUse { .. } => {
                return Err(Diagnostic::error("captured Product static type cannot yet retain transparent alias application obligations"));
            }
        };
            Ok(self.graph.types.insert(value))
        }
        fn constraint(
            &mut self,
            value: &source_types::TypeConstraint,
        ) -> Result<ProductOperandConstraint, Diagnostic> {
            Ok(match value {
                source_types::TypeConstraint::Named(name) => {
                    ProductOperandConstraint::Named(self.name(name, SymbolHandle::invalid()))
                }
                source_types::TypeConstraint::ArithmeticDomain(policy) => {
                    ProductOperandConstraint::Arithmetic(*policy)
                }
                source_types::TypeConstraint::Domain(value) => ProductOperandConstraint::Domain {
                    name: self.name(&value.name, SymbolHandle::invalid()),
                    arguments: self.types(value.arguments)?,
                },
            })
        }
        fn expression_span(
            &mut self,
            span: HandleSpan<source_expression::ExpressionHandle>,
        ) -> Result<HandleSpan<Handle<ProductOperandExpression>>, Diagnostic> {
            let source = self.source;
            let values = resolved_expressions(source)
                .expression_handles(span)
                .iter()
                .map(|value| self.expression(*value))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(self.graph.expression_arguments.insert_many(values))
        }
        fn capture_subject(
            &mut self,
            capture: Handle<source_expression::MatchSubjectCapture>,
        ) -> Result<Handle<ProductMatchSubject>, Diagnostic> {
            if let Some((_, value)) = self
                .captures
                .iter()
                .find(|(original, _)| *original == capture)
            {
                return Ok(*value);
            }
            let subject = resolved_expressions(self.source)
                .match_subject(capture)
                .subject;
            let subject = self.expression(subject)?;
            let value = self
                .graph
                .match_subjects
                .insert(ProductMatchSubject { subject });
            self.captures.push((capture, value));
            Ok(value)
        }
        fn expression_selections(
            &mut self,
            expression: source_expression::ExpressionHandle,
        ) -> Result<HandleSpan<ProductOperandSelection>, Diagnostic> {
            let source = self.source;
            let ledger = source.authored_declaration_selections();
            let mut selections = Vec::new();
            for occurrence in
                resolved_expressions(source).authored_selection_occurrences(expression)
            {
                let selection = ledger.get(occurrence).ok_or_else(|| {
                    Diagnostic::error(
                        "designated source expression retains an unknown authored selection",
                    )
                    .with_source_span(source.tables.bodies.expressions.source_span(expression))
                })?;
                let rejoined = ledger
                    .source_coordinate_occurrence(
                        selection.source_span(),
                        selection.exposure(),
                        selection.kind(),
                    )
                    .map_err(|reason| {
                        Diagnostic::error(format!(
                        "captured static operand cannot rejoin its {:?} selection ({:?}): {reason}",
                        selection.kind(),
                        selection.exposure()
                    ))
                    .with_source_span(selection.source_span())
                    })?;
                if rejoined != occurrence {
                    return Err(Diagnostic::error(
                    "designated source expression selection changed its exact ledger association",
                )
                .with_source_span(selection.source_span()));
                }
                selections.push(ProductOperandSelection {
                    source_span: selection.source_span(),
                    exposure: selection.exposure(),
                    kind: Some(selection.kind()),
                });
            }
            Ok(self.graph.selections.insert_many(selections))
        }

        fn expression(
            &mut self,
            expression: source_expression::ExpressionHandle,
        ) -> Result<Handle<ProductOperandExpression>, Diagnostic> {
            if !expression.is_valid() {
                return Ok(Handle::invalid());
            }
            if let Some((_, value)) = self
                .expressions
                .iter()
                .find(|(original, _)| *original == expression)
            {
                return Ok(*value);
            }
            let source = self.source;
            let span = source.tables.bodies.expressions.source_span(expression);
            if self.active.contains(&expression) {
                return Err(Diagnostic::error(
                    "designated static operand has a cyclic source expression",
                )
                .with_source_span(span));
            }
            self.active.push(expression);
            use source_expression::ExpressionNode as Source;
            let kind = match source.tables.bodies.expressions.expression(expression) {
                Source::Integer(value) => ProductExpressionKind::Integer(value.clone()),
                Source::Float(value) => ProductExpressionKind::Float(value.clone()),
                Source::UndeterminedNaN(value) => ProductExpressionKind::UndeterminedNaN(*value),
                Source::Boolean(value) => ProductExpressionKind::Boolean(*value),
                Source::String(value) => ProductExpressionKind::Text(value.to_vec()),
                Source::Name(value) => ProductExpressionKind::Name(self.names(
                    resolved_expressions(source).name_path_members(value.members),
                    value.symbol,
                )),
                Source::ArrayLiteral(values) => {
                    ProductExpressionKind::Array(self.expression_span(*values)?)
                }
                Source::Binary(value) => ProductExpressionKind::Binary {
                    left: self.expression(value.left)?,
                    operator: binary(value.operator),
                    right: self.expression(value.right)?,
                },
                Source::Unary(value) => ProductExpressionKind::Unary {
                    operand: self.expression(value.operand)?,
                    operator: match value.operator {
                        source_expression::UnaryOperator::BitwiseNot => {
                            ProductUnaryOperator::BitwiseNot
                        }
                        source_expression::UnaryOperator::LogicalNot => {
                            ProductUnaryOperator::LogicalNot
                        }
                    },
                },
                Source::Member(value) => ProductExpressionKind::Member {
                    receiver: self.expression(value.receiver)?,
                    member: self.name(&value.member, value.member_symbol),
                },
                Source::Indexed(value) => ProductExpressionKind::Indexed {
                    receiver: self.expression(value.collection)?,
                    index: self.expression(value.index)?,
                },
                Source::Borrow(value) => ProductExpressionKind::Borrow {
                    target: self.expression(value.target)?,
                    access: value.access,
                },
                Source::Atomic(value) => ProductExpressionKind::Atomic {
                    value: self.expression(value.value)?,
                    result: self.expression(value.result)?,
                    ordering: value.ordering,
                    result_custody: value.result_custody,
                },
                Source::Range(value) => ProductExpressionKind::Range {
                    start: self.expression(value.start)?,
                    end: self.expression(value.end)?,
                    inclusive: value.end_inclusive,
                },
                Source::Membership(value) => ProductExpressionKind::Membership {
                    value: self.expression(value.value)?,
                    domain: self.names(
                        resolved_expressions(source).name_path_members(value.domain),
                        value.domain_symbol,
                    ),
                    declaration: self.declaration(value.domain_symbol),
                    case_type: self.declaration(value.case_type_symbol),
                    case: self.declaration(value.case_symbol),
                },
                Source::ZeroValue(value) => {
                    ProductExpressionKind::ZeroValue(self.ty_handle(*value)?)
                }
                Source::TypeExpression(value) => {
                    ProductExpressionKind::TypeExpression(self.ty_handle(*value)?)
                }
                Source::Call(value) => {
                    let receiver = self.expression(value.receiver)?;
                    let target = self.name(&value.target, value.target_symbol);
                    let owner = if value.requirement_application.is_valid() {
                        self.ty_handle(value.requirement_application)?
                    } else {
                        Handle::invalid()
                    };
                    let static_arguments = value
                        .machine_arguments
                        .iter()
                        .map(|value| {
                            self.operand(
                                value,
                                &resolved_expressions(source)
                                    .authored_selection_occurrences(expression)
                                    .collect::<Vec<_>>(),
                                false,
                            )
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    let static_arguments =
                        self.graph.static_arguments.insert_many(static_arguments);
                    let arguments = self.expression_span(value.arguments)?;
                    let evidence = self.names(&value.evidence_arguments, SymbolHandle::invalid());
                    ProductExpressionKind::Call {
                        receiver,
                        target,
                        owner,
                        static_arguments,
                        arguments,
                        evidence,
                        acknowledgement: value.operational_acknowledgement,
                    }
                }
                Source::Cast(value) => ProductExpressionKind::Cast {
                    value: self.expression(value.value)?,
                    target: self.ty_handle(value.target_type)?,
                    policy: value.domain,
                    domain: self.names(
                        resolved_expressions(source).name_path_members(value.semantic_domain),
                        value.semantic_domain_symbol,
                    ),
                    domain_arguments: self.types(value.semantic_domain_arguments)?,
                    form: value.form,
                },
                Source::StructLiteral(value) => {
                    let name = self.name(&value.type_name, value.type_symbol);
                    let case = value
                        .case_name
                        .as_ref()
                        .map(|name| self.name(name, value.case_symbol.unwrap_or_default()));
                    let fields = resolved_expressions(source)
                        .struct_fields(value.fields)
                        .iter()
                        .map(|field| {
                            Ok(ProductOperandField {
                                name: self.name(&field.name, field.field_symbol),
                                value: self.expression(field.value)?,
                            })
                        })
                        .collect::<Result<Vec<_>, Diagnostic>>()?;
                    ProductExpressionKind::Record {
                        name,
                        case,
                        fields: self.graph.fields.insert_many(fields),
                    }
                }
                Source::SavedSubject(value) => ProductExpressionKind::SavedSubject {
                    component: value.component,
                    capture: self.capture_subject(value.capture)?,
                    pattern_head: self.expression(value.pattern_head)?,
                },
                Source::Match(value) => {
                    let subject = self.expression(value.subject)?;
                    let capture = self.capture_subject(value.capture)?;
                    let arms = resolved_expressions(source)
                        .match_arms(value.arms)
                        .iter()
                        .map(|arm| {
                            let pattern = self.pattern(arm.pattern)?;
                            Ok(ProductMatchArm {
                                pattern,
                                value: self.expression(arm.value)?,
                                source_span: arm.source_span,
                            })
                        })
                        .collect::<Result<Vec<_>, Diagnostic>>()?;
                    ProductExpressionKind::Match {
                        subject,
                        capture,
                        arms: self.graph.match_arms.insert_many(arms),
                    }
                }
            };
            let authored_selections = self.expression_selections(expression)?;
            let value = self.graph.expressions.insert(ProductOperandExpression {
                source_span: span,
                authored_selections,
                kind,
            });
            self.active.pop();
            self.expressions.push((expression, value));
            Ok(value)
        }
        fn pattern(
            &mut self,
            pattern: source_expression::MatchPattern,
        ) -> Result<ProductMatchPattern, Diagnostic> {
            let source = self.source;
            Ok(match pattern {
                source_expression::MatchPattern::Wildcard => ProductMatchPattern::Wildcard,
                source_expression::MatchPattern::Value(value) => {
                    ProductMatchPattern::Value(self.expression(value)?)
                }
                source_expression::MatchPattern::Destructure(value) => {
                    let head = self.expression(value.head)?;
                    let fields = resolved_expressions(source)
                        .match_fields(value.fields)
                        .iter()
                        .map(|field| {
                            Ok(ProductMatchField {
                                component: field.component,
                                member: self.name(&field.member, field.field_symbol),
                                action: match &field.action {
                                    source_expression::MatchFieldAction::Bind(name) => {
                                        ProductMatchFieldAction::Bind(
                                            self.name(name, SymbolHandle::invalid()),
                                        )
                                    }
                                    source_expression::MatchFieldAction::Equal(value) => {
                                        ProductMatchFieldAction::Equal(self.expression(*value)?)
                                    }
                                    source_expression::MatchFieldAction::Waive => {
                                        ProductMatchFieldAction::Waive
                                    }
                                },
                            })
                        })
                        .collect::<Result<Vec<_>, Diagnostic>>()?;
                    let fields = self.graph.match_fields.insert_many(fields);
                    let selectors = resolved_expressions(source)
                        .match_proof_selectors(value.proof_selectors)
                        .iter()
                        .map(|selector| ProductProofSelector {
                            output: self.name(&selector.output_field, SymbolHandle::invalid()),
                            binding: self.name(&selector.binding, SymbolHandle::invalid()),
                        })
                        .collect::<Vec<_>>();
                    ProductMatchPattern::Destructure {
                        head,
                        fields,
                        has_rest: value.has_rest,
                        guard: self.expression(value.guard)?,
                        condition: self.expression(value.condition)?,
                        proof_selectors: self.graph.proof_selectors.insert_many(selectors),
                    }
                }
            })
        }
    }
    fn binary(value: source_expression::BinaryOperator) -> ProductBinaryOperator {
        use source_expression::BinaryOperator as Source;
        match value {
            Source::Add => ProductBinaryOperator::Add,
            Source::And => ProductBinaryOperator::And,
            Source::BitwiseAnd => ProductBinaryOperator::BitwiseAnd,
            Source::BitwiseOr => ProductBinaryOperator::BitwiseOr,
            Source::BitwiseXor => ProductBinaryOperator::BitwiseXor,
            Source::Divide => ProductBinaryOperator::Divide,
            Source::Equal => ProductBinaryOperator::Equal,
            Source::Greater => ProductBinaryOperator::Greater,
            Source::GreaterOrEqual => ProductBinaryOperator::GreaterOrEqual,
            Source::Less => ProductBinaryOperator::Less,
            Source::LessOrEqual => ProductBinaryOperator::LessOrEqual,
            Source::Modulo => ProductBinaryOperator::Modulo,
            Source::Multiply => ProductBinaryOperator::Multiply,
            Source::NotEqual => ProductBinaryOperator::NotEqual,
            Source::Or => ProductBinaryOperator::Or,
            Source::ShiftLeft => ProductBinaryOperator::ShiftLeft,
            Source::ShiftRight => ProductBinaryOperator::ShiftRight,
            Source::Subtract => ProductBinaryOperator::Subtract,
        }
    }
}

pub(crate) mod construction_destinations {
    use crate::tables::{expression_table, expression_table_mut};
    use crate::typing_queries::selected_construction_field_type;
    use diagnostics::Diagnostic;
    use symbols::SymbolHandle;
    use typed_trees::TypedTrees;
    use typed_trees::expression::{ExpressionHandle, ExpressionNode, StaticMachineArgument};
    use typed_trees::statement::{StatementNode, TransitionTargetNode};
    use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

    struct ConstructionCall {
        target: SymbolHandle,
        runtime_contract: Option<typed_trees::callable_values::RuntimeCallableContractHandle>,
        static_arguments: Vec<StaticMachineArgument>,
        arguments: Vec<ExpressionHandle>,
    }

    pub(crate) fn prepare(
        program: &mut TypedTrees,
        expression_frontier: usize,
        machine_frontier: usize,
        completed_machines: &[SymbolHandle],
    ) -> Result<Vec<TypeReferenceHandle>, Diagnostic> {
        let mut destinations = Vec::<(ExpressionHandle, TypeReferenceHandle, SymbolHandle)>::new();
        let mut calls = Vec::new();
        let mut constructors = Vec::new();
        let mut unlanded = Vec::new();

        for (handle, expression) in program.expression_table.expression_entries() {
            if usize::try_from(handle.arena_index()).is_ok_and(|index| index <= expression_frontier)
            {
                continue;
            }
            match expression {
                ExpressionNode::StructLiteral(_) => constructors.push(handle),
                ExpressionNode::Call(call) => {
                    let arguments = program.expression_table.expression_handles(call.arguments);
                    if program
                        .mathematical_definitions()
                        .iter()
                        .any(|definition| definition.symbol == call.target_symbol)
                        && arguments
                            .iter()
                            .any(|argument| needs_destination(program, *argument))
                    {
                        return Err(crate::unavailable_judgment(
                            "mathematical call receiving types",
                        ));
                    }
                    collect_call_destinations(
                        program,
                        call.target_symbol,
                        call.runtime_call.map(|binding| binding.contract),
                        &call.machine_arguments,
                        arguments,
                        &mut calls,
                    );
                }
                _ => {}
            }
        }

        let mut completed = completed_machines
            .iter()
            .map(|symbol| crate::lowerer::symbol_key(*symbol))
            .collect::<Vec<_>>();
        completed.sort_unstable();
        for (ordinal, machine) in program.machines().iter().enumerate() {
            if ordinal < machine_frontier
                && completed
                    .binary_search(&crate::lowerer::symbol_key(machine.symbol))
                    .is_err()
            {
                continue;
            }
            for owned in program.machine_owned_data(machine) {
                if owned.initial_value.is_valid() && owned.type_reference.is_valid() {
                    destinations.push((owned.initial_value, owned.type_reference, machine.symbol));
                }
            }
            for state in program.machine_states(machine) {
                let statements = program.statement_table.statements(state.statement_nodes);
                for (statement_index, statement) in statements.iter().enumerate() {
                    match statement {
                        StatementNode::Assembly(_) => {}
                        StatementNode::LocalData(local)
                            if local.initial_value.is_valid()
                                && local.type_reference.is_valid() =>
                        {
                            destinations.push((
                                local.initial_value,
                                local.type_reference,
                                machine.symbol,
                            ));
                        }
                        StatementNode::Assignment(assignment) => {
                            if needs_destination(program, assignment.value) {
                                // One declared-place query supplies the exact
                                // constructor application. Bounds, access and
                                // establishment remain lowered judgments.
                                match assigned_place_type(program, state, assignment.target) {
                                    Some(receiving) => destinations.push((
                                        assignment.value,
                                        receiving,
                                        machine.symbol,
                                    )),
                                    None => unlanded.push((
                                        machine.symbol,
                                        program.expression_table.source_span(assignment.value),
                                    )),
                                }
                            }
                        }
                        StatementNode::Call(call) => collect_call_destinations(
                            program,
                            call.target_symbol,
                            call.runtime_call.map(|binding| binding.contract),
                            &call.machine_arguments,
                            program.statement_table.expression_handles(call.arguments),
                            &mut calls,
                        ),
                        StatementNode::Expression(value)
                            if state.return_type.is_valid()
                                && statement_index + 1 == statements.len() =>
                        {
                            destinations.push((*value, state.return_type, machine.symbol));
                        }
                        StatementNode::Transition(transition) => {
                            for target in transition.targets() {
                                match program.statement_table.transition_target(target) {
                                    TransitionTargetNode::Value(value)
                                        if state.return_type.is_valid() =>
                                    {
                                        destinations.push((
                                            *value,
                                            state.return_type,
                                            machine.symbol,
                                        ));
                                    }
                                    TransitionTargetNode::Named {
                                        path, arguments, ..
                                    } => {
                                        collect_call_destinations(
                                            program,
                                            path.symbol,
                                            None,
                                            &[],
                                            program.statement_table.expression_handles(*arguments),
                                            &mut calls,
                                        );
                                    }
                                    TransitionTargetNode::Value(_)
                                    | TransitionTargetNode::Terminal => {}
                                }
                            }
                        }
                        StatementNode::AssemblyFact(_)
                        | StatementNode::Expression(_)
                        | StatementNode::LocalData(_) => {}
                    }
                }
            }
        }

        for (machine, source_span) in unlanded {
            program.record_deferred_refusal(
                machine,
                typed_trees::deferred_refusals::DeferredRequest::AssignmentLiteralDestination,
                source_span,
            );
        }
        for call in calls {
            let binders = if let Some(contract) = call.runtime_contract {
                program
                    .state_signature_type_parameters(
                        &program.runtime_callable_contracts.get(contract).signature,
                    )
                    .to_vec()
            } else {
                program.declaration_type_parameters(call.target).to_vec()
            };
            if binders.len() != call.static_arguments.len() {
                continue;
            }
            let mut substitutions = Vec::with_capacity(binders.len());
            for (binder, argument) in binders.iter().zip(&call.static_arguments) {
                let actual = program
                    .static_operand_type_reference(argument)
                    .map_err(Diagnostic::error)?;
                substitutions.push((binder.symbol, actual));
            }
            let parameters = if let Some(contract) = call.runtime_contract {
                program.state_signature_parameters(
                    &program
                        .declared_callable_contract(contract)
                        .map_err(Diagnostic::error)?
                        .signature,
                )
            } else {
                crate::typing_queries::call_target_parameters(program, call.target)?
            };
            let required: Vec<_> = parameters
                .iter()
                .map(|parameter| parameter.type_reference)
                .collect();
            for (argument, reference) in call.arguments.into_iter().zip(required) {
                if !needs_destination(program, argument) {
                    continue;
                }
                let reference = if !substitutions.is_empty() {
                    program
                        .apply_callable_type(reference, &substitutions, &[], call.target)
                        .map_err(Diagnostic::error)?
                } else if callee_owns_nominal_lifetimes(program, reference, call.target) {
                    let occurrence = program.type_reference_table.fresh_occurrence(reference);
                    program
                        .type_reference_table
                        .retain_operand_lifetime_owner(occurrence, call.target)
                        .map_err(Diagnostic::error)?;
                    occurrence
                } else {
                    reference
                };
                destinations.push((argument, reference, call.target));
            }
        }
        let mut cast_destinations = Vec::new();
        let mut completed_destinations = Vec::new();
        let mut unselected = Vec::new();
        for (value, destination, lifetime_owner) in destinations {
            land_one(
                program,
                value,
                destination,
                lifetime_owner,
                &mut cast_destinations,
                &mut completed_destinations,
                &mut unselected,
            )?;
        }
        // A child constructor may get its complete application from an
        // enclosing field. Land explicit receiving contexts first; querying
        // every child before its parent would discard that application.
        for value in constructors {
            let position = value.arena_index() as usize;
            if completed_destinations
                .get(position)
                .is_some_and(|contexts| !contexts.is_empty())
            {
                continue;
            }
            let ExpressionNode::StructLiteral(literal) =
                program.expression_table.expression(value).clone()
            else {
                continue;
            };
            let known_application = [
                program
                    .expression_table
                    .construction_required_type_reference(value),
                program
                    .expression_table
                    .constructed_value_type_reference(value),
                literal.application_type,
            ]
            .into_iter()
            .any(|reference| reference.is_valid());
            let monomorphic = program
                .unique_data_definition_by_symbol(literal.type_symbol)
                .is_some_and(|data| {
                    data.type_parameters.is_empty() && data.generic_instance.is_none()
                });
            if !known_application && !monomorphic {
                // Keep the unresolved constructor intact. The checked reader
                // refuses it if demanded without an exact receiving tuple;
                // its payload cannot infer a different application here.
                continue;
            }
            let fields = program
                .expression_table
                .struct_fields(literal.fields)
                .to_vec();
            for field in fields {
                let Some(field_type) =
                    selected_field_type(program, value, &literal, field.field_symbol)
                else {
                    unselected.push(value);
                    break;
                };
                land_one(
                    program,
                    field.value,
                    field_type,
                    literal.type_symbol,
                    &mut cast_destinations,
                    &mut completed_destinations,
                    &mut unselected,
                )?;
            }
        }
        defer_unselected_constructors(program, unselected)?;
        Ok(cast_destinations)
    }

    /// The type a constructor's field is built at, or `None` when typing does
    /// not select it. The usual reason is a constructor of generic data
    /// applied at arguments: its fields have the types of that application,
    /// which exists once the data is specialized, and typing specializes
    /// nothing.
    fn selected_field_type(
        program: &TypedTrees,
        constructor: ExpressionHandle,
        literal: &typed_trees::expression::TableStructLiteral,
        field: SymbolHandle,
    ) -> Option<TypeReferenceHandle> {
        selected_construction_field_type(program, constructor, literal, field).ok()
    }

    /// Records each constructor whose field types typing did not select on
    /// the declaration that holds it. The constructor keeps the fields it was
    /// written with; its declaration is refused where it is demanded. One
    /// that no declaration owns has no reader to raise it and refuses here.
    fn defer_unselected_constructors(
        program: &mut TypedTrees,
        unselected: Vec<ExpressionHandle>,
    ) -> Result<(), Diagnostic> {
        use typed_trees::deferred_refusals::DeferredRequest;
        if unselected.is_empty() {
            return Ok(());
        }
        let owners = typed_trees::references::collect_program_operand_owners(program);
        let mut rows = Vec::new();
        for constructor in unselected {
            let source_span = program.expression_table.source_span(constructor);
            let before = rows.len();
            rows.extend(
                owners
                    .expression_owners(constructor)
                    .filter(|owner| owner.is_valid())
                    .map(|owner| (owner, source_span)),
            );
            if rows.len() == before {
                return Err(crate::unavailable_judgment(
                    "selected constructor field type outside every declaration",
                )
                .with_source_span(source_span));
            }
        }
        for (owner, source_span) in rows {
            program.record_deferred_refusal(
                owner,
                DeferredRequest::ConstructorFieldSelection,
                source_span,
            );
        }
        Ok(())
    }

    fn assigned_place_type(
        program: &TypedTrees,
        state: &typed_trees::state::State,
        target: ExpressionHandle,
    ) -> Option<TypeReferenceHandle> {
        let declared = crate::typing_queries::declared_place_type(program, state, target).ok()?;
        let mut carrier = declared;
        while let TypeReferenceNode::Constrained { base_type, .. } =
            program.type_reference_table.type_reference(carrier)
        {
            carrier = *base_type;
        }
        (!matches!(
            program.type_reference_table.type_reference(carrier),
            TypeReferenceNode::Reference { .. }
        ))
        .then_some(declared)
    }

    fn collect_call_destinations(
        program: &TypedTrees,
        target_symbol: SymbolHandle,
        runtime_contract: Option<typed_trees::callable_values::RuntimeCallableContractHandle>,
        static_arguments: &[StaticMachineArgument],
        arguments: &[ExpressionHandle],
        calls: &mut Vec<ConstructionCall>,
    ) {
        if (!target_symbol.is_valid() && runtime_contract.is_none())
            || !arguments
                .iter()
                .any(|argument| needs_destination(program, *argument))
        {
            return;
        }
        calls.push(ConstructionCall {
            target: target_symbol,
            runtime_contract,
            static_arguments: static_arguments.to_vec(),
            arguments: arguments.to_vec(),
        });
    }

    fn needs_destination(program: &TypedTrees, value: ExpressionHandle) -> bool {
        value.is_valid()
            && match program.expression_table.expression(value) {
                ExpressionNode::Name(_)
                | ExpressionNode::MachineValue(_)
                | ExpressionNode::StructLiteral(_)
                | ExpressionNode::String(_)
                | ExpressionNode::ArrayLiteral(_)
                | ExpressionNode::Match(_) => true,
                ExpressionNode::Cast(cast) => cast.form.entry().is_some(),
                _ => false,
            }
    }

    fn land_one(
        program: &mut TypedTrees,
        value: ExpressionHandle,
        destination: TypeReferenceHandle,
        lifetime_owner: SymbolHandle,
        cast_destinations: &mut Vec<TypeReferenceHandle>,
        completed: &mut Vec<Vec<(TypeReferenceHandle, SymbolHandle)>>,
        unselected: &mut Vec<ExpressionHandle>,
    ) -> Result<bool, Diagnostic> {
        if !value.is_valid() || !destination.is_valid() {
            return Ok(false);
        }
        if !program.expression_table.expression_is_valid(value) {
            return Err(Diagnostic::error(
                "constructor destination refers to a missing expression occurrence",
            ));
        }
        if !program
            .type_reference_table
            .contains_type_reference(destination)
        {
            return Err(Diagnostic::error(
                "constructor destination refers to a missing receiving type occurrence",
            ));
        }
        // A receiving occurrence includes its complete type handle and lexical
        // region owner. Revisit a different context, but do not walk the same
        // descendants again through another root or the constructor census.
        let position = value.arena_index() as usize;
        if completed
            .get(position)
            .is_some_and(|contexts| contexts.contains(&(destination, lifetime_owner)))
        {
            return Ok(true);
        }
        let landed = land_destination(
            program,
            value,
            destination,
            lifetime_owner,
            cast_destinations,
            completed,
            unselected,
        )?;
        if landed {
            if completed.len() <= position {
                completed.resize_with(position + 1, Vec::new);
            }
            completed[position].push((destination, lifetime_owner));
        }
        Ok(landed)
    }

    fn land_destination(
        program: &mut TypedTrees,
        value: ExpressionHandle,
        destination: TypeReferenceHandle,
        mut lifetime_owner: SymbolHandle,
        cast_destinations: &mut Vec<TypeReferenceHandle>,
        completed: &mut Vec<Vec<(TypeReferenceHandle, SymbolHandle)>>,
        unselected: &mut Vec<ExpressionHandle>,
    ) -> Result<bool, Diagnostic> {
        if crate::runtime_callables::prepare_named_value(
            program,
            value,
            destination,
            lifetime_owner,
        )? {
            return Ok(true);
        }
        if let Some(dispatch) = program.expression_table.match_node(value) {
            let arms = program.expression_table.match_arms(dispatch.arms).to_vec();
            let mut landed = true;
            for arm in arms {
                landed &= land_one(
                    program,
                    arm.value,
                    destination,
                    lifetime_owner,
                    cast_destinations,
                    completed,
                    unselected,
                )?;
            }
            return Ok(landed);
        }
        // An expected type supplies a literal's carrier, never its domains.
        // Retain the destination at the complete cast, not at its operand:
        // accounting still checks every written addition, keep and removal.
        if let Some(cast) = program
            .expression_table
            .cast_node(value)
            .copied()
            .filter(|cast| cast.form.entry().is_some())
        {
            // Expression handles are dense. Keep lookup independent of the
            // number of casts in a compiler-sized source cohort.
            let index = value.arena_index() as usize;
            if cast_destinations.len() <= index {
                cast_destinations.resize(index + 1, TypeReferenceHandle::invalid());
            }
            cast_destinations[index] = destination;
            // Domain-only casts preserve the receiving carrier through an
            // inner cast or match. An explicit carrier cast is a boundary:
            // land_one does not descend through an ordinary value cast.
            return land_one(
                program,
                cast.value,
                destination,
                lifetime_owner,
                cast_destinations,
                completed,
                unselected,
            );
        }
        let mut carrier = destination;
        loop {
            let supplied = program.type_reference_table.operand_lifetime_owner(carrier);
            if supplied.is_valid() {
                lifetime_owner = supplied;
            }
            match program.type_reference_table.type_reference(carrier) {
                TypeReferenceNode::Constrained { base_type, .. } => carrier = *base_type,
                _ => break,
            }
        }
        if let ExpressionNode::StructLiteral(literal) =
            program.expression_table.expression(value).clone()
        {
            // Runtime value indices still need their actual source argument
            // materialization. This type-only landing does not snapshot or
            // invent those values and grants no construction premise.
            let exact_template = matches!(program.type_reference_table.type_reference(carrier),
            TypeReferenceNode::Generic { base_symbol, .. } if *base_symbol == literal.type_symbol)
                && !program
                    .type_application_value_indices(carrier)
                    .has_runtime_values();
            if !exact_template
                && program
                    .closed_data_constructor_definition(destination, literal.type_symbol)
                    .is_none()
            {
                return Ok(false);
            }
            let prior = expression_table(program).construction_required_type_reference(value);
            let carrier = if (!prior.is_valid() || !literal.application_type.is_valid())
                && lifetime_owner.is_valid()
                && !program
                    .type_reference_table
                    .operand_lifetime_owner(carrier)
                    .is_valid()
            {
                let occurrence = program.type_reference_table.fresh_occurrence(carrier);
                program
                    .type_reference_table
                    .retain_operand_lifetime_owner(occurrence, lifetime_owner)
                    .map_err(Diagnostic::error)?;
                occurrence
            } else {
                carrier
            };
            // Normalization is complete before landing. Reuse its exact view;
            // only a fresh receiving occurrence needs another metadata row.
            if program.data_application_definition(carrier).is_none() {
                program
                    .prepare_data_application_view(carrier)
                    .map_err(Diagnostic::error)?;
            }
            if !prior.is_valid() {
                expression_table_mut(program)
                    .set_construction_required_type_reference(value, carrier);
            }
            if !literal.application_type.is_valid()
                && let ExpressionNode::StructLiteral(retained) =
                    expression_table_mut(program).expression_mut(value)
            {
                retained.application_type = carrier;
            }
            let fields = expression_table(program)
                .struct_fields(literal.fields)
                .to_vec();
            let mut landed = true;
            for field in fields {
                let Some(field_type) =
                    selected_field_type(program, value, &literal, field.field_symbol)
                else {
                    unselected.push(value);
                    return Ok(false);
                };
                landed &= land_one(
                    program,
                    field.value,
                    field_type,
                    literal.type_symbol,
                    cast_destinations,
                    completed,
                    unselected,
                )?;
            }
            return Ok(landed);
        }
        if let (
            ExpressionNode::ArrayLiteral(elements),
            TypeReferenceNode::FixedArray { element_type, .. },
        ) = (
            program.expression_table.expression(value),
            program.type_reference_table.type_reference(carrier),
        ) {
            let elements = expression_table(program)
                .expression_handles(*elements)
                .to_vec();
            let element_type = *element_type;
            let mut landed = true;
            for element in elements {
                landed &= land_one(
                    program,
                    element,
                    element_type,
                    lifetime_owner,
                    cast_destinations,
                    completed,
                    unselected,
                )?;
            }
            return Ok(landed);
        }
        crate::typing_queries::land_owned_fixed_byte_literal(program, value, destination)
            .map(|()| true)
    }

    fn callee_owns_nominal_lifetimes(
        program: &TypedTrees,
        reference: TypeReferenceHandle,
        callee: SymbolHandle,
    ) -> bool {
        if program
            .type_reference_table
            .operand_lifetime_owner(reference)
            .is_valid()
        {
            return false;
        }
        let TypeReferenceNode::Generic {
            lifetime_arguments, ..
        } = program.type_reference_table.type_reference(reference)
        else {
            return false;
        };
        !lifetime_arguments.is_empty()
            && lifetime_arguments.iter().all(|lifetime| {
                typed_trees::type_identity::lifetime_parameter_identity(
                    program,
                    callee,
                    lifetime.as_str(),
                )
                .is_some()
            })
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn assignment_receivers_preserve_declared_types_without_inventing_unknown_types() {
            use symbols::{SymbolKind, SymbolNameRef, SymbolTableBuilder};
            use typed_trees::{
                data::DataDefinition,
                expression::{TableNamePath, TableStructLiteral},
                machine::Machine,
                signature::StateParameter,
                state::State,
                statement::TableAssignment,
            };

            let mut symbols = SymbolTableBuilder::default();
            let data = symbols.insert_root(SymbolKind::Data, SymbolNameRef::Static("Record"));
            let owner = symbols.insert_root(SymbolKind::Machine, SymbolNameRef::Static("owner"));
            let state = symbols
                .insert_children(owner, [(SymbolKind::State, SymbolNameRef::Static("entry"))])
                .handles()
                .next()
                .unwrap();
            let formal = symbols
                .insert_children(
                    state,
                    [(SymbolKind::Parameter, SymbolNameRef::Static("target"))],
                )
                .handles()
                .next()
                .unwrap();
            let mut program = TypedTrees {
                symbols: symbols.finish(),
                ..Default::default()
            };
            program.push_data_definition(DataDefinition {
                symbol: data,
                name: "Record".into(),
                ..Default::default()
            });
            let declared = program
                .type_reference_table
                .insert(TypeReferenceNode::Named {
                    symbol: data,
                    name: "Record".into(),
                });
            program
                .type_reference_table
                .retain_operand_lifetime_owner(declared, state)
                .expect("the formal's declared type retains its actual state scope");
            let parameters = program.state_parameters.insert_many([StateParameter {
                symbol: formal,
                name: "target".into(),
                type_reference: declared,
                is_mutable: true,
                ..Default::default()
            }]);
            let mut names = Default::default();
            program
                .expression_table
                .push_name_path_member(&mut names, "target".into());
            let target = program
                .expression_table
                .insert(ExpressionNode::Name(TableNamePath {
                    members: names,
                    head_symbol: formal,
                    symbol: formal,
                    ..Default::default()
                }));
            let mut missing_names = Default::default();
            program
                .expression_table
                .push_name_path_member(&mut missing_names, "missing".into());
            let missing = program
                .expression_table
                .insert(ExpressionNode::Name(TableNamePath {
                    members: missing_names,
                    ..Default::default()
                }));
            let mut literal = || {
                program
                    .expression_table
                    .insert(ExpressionNode::StructLiteral(TableStructLiteral {
                        type_symbol: data,
                        type_name: "Record".into(),
                        ..Default::default()
                    }))
            };
            let value = literal();
            let unknown_value = literal();
            let body = program.statement_table.insert_statements([
                StatementNode::Assignment(TableAssignment { target, value }),
                StatementNode::Assignment(TableAssignment {
                    target: missing,
                    value: unknown_value,
                }),
            ]);
            let states = program.machine_states.insert_many([State {
                symbol: state,
                name: "entry".into(),
                parameters,
                statement_nodes: body,
                ..Default::default()
            }]);
            program.push_machine(Machine {
                symbol: owner,
                name: "owner".into(),
                states,
                ..Default::default()
            });

            prepare(&mut program, 0, 0, &[])
                .expect("retaining source destinations grants no assignment verdict");
            let statements = program.statement_table.statements(body);
            let StatementNode::Assignment(known) = statements[0] else {
                panic!("known assignment changed kind");
            };
            let StatementNode::Assignment(unknown) = statements[1] else {
                panic!("unknown assignment changed kind");
            };
            assert_eq!((known.target, known.value), (target, value));
            assert_eq!(
                crate::typing_queries::declared_place_type(
                    &program,
                    &program.machine_states(&program.machines()[0])[0],
                    target
                )
                .unwrap(),
                declared
            );
            assert_eq!((unknown.target, unknown.value), (missing, unknown_value));
            assert!(
                !program
                    .expression_table
                    .construction_required_type_reference(unknown.value)
                    .is_valid(),
                "an unresolved destination must not invent its constructor application"
            );
            assert_eq!(
                program
                    .deferred_refusal(owner)
                    .map(|refusal| refusal.request),
                Some(typed_trees::deferred_refusals::DeferredRequest::AssignmentLiteralDestination),
                "the unresolved destination remains its owner's unserved request"
            );
            let ExpressionNode::StructLiteral(literal) = program.expression_table.expression(value)
            else {
                panic!("source constructor changed kind");
            };
            assert_eq!(literal.application_type, declared);
            assert_eq!(
                program
                    .type_reference_table
                    .operand_lifetime_owner(literal.application_type),
                state,
                "constructor landing preserves the supplying formal's lifetime scope"
            );
            assert_eq!(
                program
                    .expression_table
                    .construction_required_type_reference(value),
                declared
            );
        }

        #[test]
        fn an_assigned_literal_lands_on_its_binding_or_refuses_its_own_machine() {
            use symbols::{SymbolKind, SymbolNameRef, SymbolTableBuilder};
            use typed_trees::deferred_refusals::DeferredRequest;
            use typed_trees::expression::{TableNamePath, TableStructLiteral};
            use typed_trees::machine::{Machine, MachineBodyStatus};
            use typed_trees::statement::{TableAssignment, TableLocalData};
            use typed_trees::{data::DataDefinition, state::State};

            let mut symbols = SymbolTableBuilder::default();
            let root = symbols.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
            let declarations = symbols.insert_children(
                root,
                [
                    (SymbolKind::Data, SymbolNameRef::Static("Record")),
                    (SymbolKind::Machine, SymbolNameRef::Static("writer")),
                ],
            );
            let (data, writer) = (declarations.handle_at(0), declarations.handle_at(1));
            let inner = symbols.insert_children(
                writer,
                [
                    (SymbolKind::State, SymbolNameRef::Static("run")),
                    (SymbolKind::Local, SymbolNameRef::Static("owned")),
                    (SymbolKind::Local, SymbolNameRef::Static("borrowed")),
                ],
            );
            let [state, owned, borrowed] = [0, 1, 2].map(|position| inner.handle_at(position));
            let mut program = TypedTrees {
                symbols: symbols.finish(),
                ..Default::default()
            };
            program.push_data_definition(DataDefinition {
                symbol: data,
                name: "Record".into(),
                ..Default::default()
            });
            let record = program
                .type_reference_table
                .insert(TypeReferenceNode::Named {
                    symbol: data,
                    name: "Record".into(),
                });
            let reference = program
                .type_reference_table
                .insert(TypeReferenceNode::Reference {
                    referee: record,
                    access: language_core::ReferenceAccess::Mutable,
                    lifetime: None,
                });
            let name = |program: &mut TypedTrees, symbol, spelled: &[&'static str]| {
                let mut members = Default::default();
                for member in spelled {
                    program.expression_table.push_name_path_member(
                        &mut members,
                        typed_trees::name::Identifier::generated(*member),
                    );
                }
                program
                    .expression_table
                    .insert(ExpressionNode::Name(TableNamePath {
                        members,
                        symbol,
                        head_symbol: symbol,
                        ..Default::default()
                    }))
            };
            let literal = |program: &mut TypedTrees| {
                program
                    .expression_table
                    .insert(ExpressionNode::StructLiteral(TableStructLiteral {
                        type_symbol: data,
                        type_name: "Record".into(),
                        ..Default::default()
                    }))
            };
            let local = |symbol, type_reference| {
                StatementNode::LocalData(TableLocalData {
                    symbol,
                    type_reference,
                    ..Default::default()
                })
            };
            let target = name(&mut program, owned, &["owned"]);
            let value = literal(&mut program);
            let statement_nodes = program.statement_table.insert_statements([
                local(owned, record),
                local(borrowed, reference),
                StatementNode::Assignment(TableAssignment { target, value }),
            ]);
            let states = program.machine_states.insert_many([State {
                symbol: state,
                statement_nodes,
                ..Default::default()
            }]);
            program.push_machine(Machine {
                symbol: writer,
                states,
                body_is_present: true,
                body_status: MachineBodyStatus::Complete,
                ..Default::default()
            });

            // `owned = Record {}` is built at the type written on `owned`.
            // Landing gives the literal its own occurrence of that type,
            // which records the machine that owns its lifetimes
            // (`land_one`, `fresh_occurrence`), so the two are the same
            // type under different handles.
            let mut landed = program.clone();
            prepare(&mut landed, 0, 0, &[]).unwrap();
            assert!(landed.deferred_refusals.is_empty());
            let required = landed
                .expression_table
                .construction_required_type_reference(value);
            assert!(required.is_valid());
            assert_eq!(
                landed.type_reference_table.type_reference(required),
                landed.type_reference_table.type_reference(record)
            );
            assert_eq!(
                landed.type_reference_table.operand_lifetime_owner(required),
                writer
            );

            // The twins differ only in the place assigned: a field of the
            // binding, and a binding of reference type. Neither has a type
            // typing computes, so the literal stays as written and the
            // machine, not the program, is refused.
            for place in [
                name(&mut program, owned, &["owned", "field"]),
                name(&mut program, borrowed, &["borrowed"]),
            ] {
                let mut unlanded = program.clone();
                let assignment = unlanded.statement_table.statements(statement_nodes).len() - 1;
                let handle = statement_nodes.handles().nth(assignment).unwrap();
                let StatementNode::Assignment(assignment) =
                    unlanded.statement_table.statement_mut(handle)
                else {
                    panic!("assignment")
                };
                assignment.target = place;
                prepare(&mut unlanded, 0, 0, &[]).unwrap();
                assert_eq!(
                    unlanded
                        .deferred_refusal(writer)
                        .map(|refusal| refusal.request),
                    Some(DeferredRequest::AssignmentLiteralDestination)
                );
                assert!(
                    !unlanded
                        .expression_table
                        .construction_required_type_reference(value)
                        .is_valid()
                );
            }
        }

        #[test]
        fn a_constructor_with_an_unselected_field_refuses_its_own_machine() {
            use symbols::{SymbolKind, SymbolNameRef, SymbolTableBuilder};
            use typed_trees::data::{DataDefinition, DataField, DataMember};
            use typed_trees::deferred_refusals::DeferredRequest;
            use typed_trees::expression::{TableStructLiteral, TableStructLiteralField};
            use typed_trees::machine::{Machine, MachineBodyStatus};
            use typed_trees::state::State;

            let mut symbols = SymbolTableBuilder::default();
            let root = symbols.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
            let declarations = symbols.insert_children(
                root,
                [
                    (SymbolKind::Data, SymbolNameRef::Static("Record")),
                    (SymbolKind::Data, SymbolNameRef::Static("Other")),
                    (SymbolKind::Machine, SymbolNameRef::Static("builder")),
                ],
            );
            let [record, other, builder] =
                [0, 1, 2].map(|position| declarations.handle_at(position));
            let [own_field, foreign_field] = [record, other].map(|data| {
                symbols
                    .insert_children(data, [(SymbolKind::Field, SymbolNameRef::Static("a"))])
                    .handle_at(0)
            });
            let state = symbols
                .insert_children(builder, [(SymbolKind::State, SymbolNameRef::Static("run"))])
                .handle_at(0);
            let mut program = TypedTrees {
                symbols: symbols.finish(),
                ..Default::default()
            };
            for (symbol, name, field) in [
                (record, "Record", own_field),
                (other, "Other", foreign_field),
            ] {
                let members =
                    program
                        .tables
                        .data_members
                        .insert_many([DataMember::Field(DataField {
                            symbol: field,
                            name: "a".into(),
                            ..Default::default()
                        })]);
                program.push_data_definition(DataDefinition {
                    symbol,
                    name: name.into(),
                    members,
                    ..Default::default()
                });
            }
            let payload = program
                .expression_table
                .insert(ExpressionNode::Boolean(true));
            let constructor = |program: &mut TypedTrees, field_symbol| {
                let fields =
                    program
                        .expression_table
                        .insert_struct_fields([TableStructLiteralField {
                            name: "a".into(),
                            field_symbol,
                            value: payload,
                        }]);
                program
                    .expression_table
                    .insert(ExpressionNode::StructLiteral(TableStructLiteral {
                        type_symbol: record,
                        type_name: "Record".into(),
                        fields,
                        ..Default::default()
                    }))
            };
            let build = |program: &TypedTrees, field_symbol| {
                let mut program = program.clone();
                let value = constructor(&mut program, field_symbol);
                let statement_nodes = program
                    .statement_table
                    .insert_statements([StatementNode::Expression(value)]);
                let states = program.machine_states.insert_many([State {
                    symbol: state,
                    statement_nodes,
                    ..Default::default()
                }]);
                program.push_machine(Machine {
                    symbol: builder,
                    states,
                    body_is_present: true,
                    body_status: MachineBodyStatus::Complete,
                    ..Default::default()
                });
                prepare(&mut program, 0, 0, &[]).unwrap();
                program
            };
            // `Record { a: true }` names a field of the record it builds.
            assert!(build(&program, own_field).deferred_refusals.is_empty());
            // The twin names a field typing cannot place in that record. The
            // machine holding the constructor is refused, not the program.
            let refused = build(&program, foreign_field);
            assert_eq!(
                refused
                    .deferred_refusal(builder)
                    .map(|refusal| refusal.request),
                Some(DeferredRequest::ConstructorFieldSelection)
            );
            assert_eq!(refused.deferred_refusals.len(), 1);
        }

        #[test]
        fn deferred_destinations_do_not_complete_or_hide_expression_identity() {
            use symbols::{SymbolKind, SymbolNameRef, SymbolTableBuilder};
            use typed_trees::{data::DataDefinition, expression::TableStructLiteral};

            let mut symbols = SymbolTableBuilder::default();
            let data = symbols.insert_root(SymbolKind::Data, SymbolNameRef::Static("Record"));
            let owner = symbols.insert_root(SymbolKind::Machine, SymbolNameRef::Static("owner"));
            let mut program = TypedTrees {
                symbols: symbols.finish(),
                ..Default::default()
            };
            program.push_data_definition(DataDefinition {
                symbol: data,
                name: "Record".into(),
                ..Default::default()
            });
            let value = program
                .expression_table
                .insert(ExpressionNode::StructLiteral(TableStructLiteral {
                    type_symbol: data,
                    type_name: "Record".into(),
                    ..Default::default()
                }));
            let unrelated = program.type_reference_table.insert(TypeReferenceNode::Unit);
            let exact = program
                .type_reference_table
                .insert(TypeReferenceNode::Named {
                    symbol: data,
                    name: "Record".into(),
                });
            let mut casts = Vec::new();
            let mut completed = Vec::new();
            assert!(
                !land_one(
                    &mut program,
                    value,
                    unrelated,
                    owner,
                    &mut casts,
                    &mut completed,
                    &mut Vec::new(),
                )
                .unwrap()
            );
            assert!(
                completed.is_empty(),
                "a deferred constructor stays eligible for the census"
            );
            assert!(
                land_one(
                    &mut program,
                    value,
                    exact,
                    owner,
                    &mut casts,
                    &mut completed,
                    &mut Vec::new(),
                )
                .unwrap()
            );
            assert!(
                land_one(
                    &mut program,
                    value,
                    exact,
                    owner,
                    &mut casts,
                    &mut completed,
                    &mut Vec::new(),
                )
                .unwrap()
            );
            assert_eq!(completed[value.arena_index() as usize], [(exact, owner)]);
            assert!(
                land_one(
                    &mut program,
                    value,
                    exact,
                    data,
                    &mut casts,
                    &mut completed,
                    &mut Vec::new()
                )
                .unwrap()
            );
            assert_eq!(
                completed[value.arena_index() as usize],
                [(exact, owner), (exact, data)]
            );

            let stale = ExpressionHandle::from_parts(value.arena_index(), value.generation() + 1);
            let error = land_one(
                &mut program,
                stale,
                exact,
                owner,
                &mut casts,
                &mut completed,
                &mut Vec::new(),
            )
            .unwrap_err();
            assert!(
                error.message.contains("missing expression occurrence"),
                "{error:?}"
            );
            let stale_type =
                TypeReferenceHandle::from_parts(exact.arena_index(), exact.generation() + 1);
            let error = land_one(
                &mut program,
                value,
                stale_type,
                owner,
                &mut casts,
                &mut completed,
                &mut Vec::new(),
            )
            .unwrap_err();
            assert!(
                error.message.contains("missing receiving type occurrence"),
                "{error:?}"
            );
        }
    }
}

pub(crate) mod installed_interrupts {
    use crate::expressions::expression::lower_static_machine_argument;
    use diagnostics::Diagnostic;
    use language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure as SelectionExposure;
    use symbol_resolved_trees as resolved;
    use symbols::SymbolKind;
    use typed_trees::expression::InstalledInterruptOperationRequest;

    pub(crate) fn lower_request(
        program: Option<&resolved::SymbolResolvedTrees>,
        target_trees: &mut typed_trees::TypedTrees,
        exposure: Option<SelectionExposure>,
        occurrences: &[language_semantics::declaration_selection::AuthoredDeclarationSelectionOccurrenceId],
        target_symbol: symbols::SymbolHandle,
        machine_arguments: &[resolved::expression::StaticMachineArgument],
    ) -> Result<Option<InstalledInterruptOperationRequest>, Diagnostic> {
        let Some(program) = program else {
            return Ok(None);
        };
        if !target_symbol.is_valid() {
            return Ok(None);
        }
        let target = program.symbols.get(target_symbol);
        if target.kind != SymbolKind::State {
            return Ok(None);
        }
        let Some(owner) = program.machines.unique_by_symbol(target.parent) else {
            return Ok(None);
        };
        if program.symbols.name(target_symbol) != "run"
            || !owner.attached_data_symbol.is_valid()
            || program.symbols.name(owner.attached_data_symbol) != "X86StaticInterruptScope"
            || program.symbols.symbol_source_origin(target_symbol)
                != Some(source::SourceOrigin::Toolchain)
            || program
                .symbols
                .symbol_source_origin(owner.attached_data_symbol)
                != Some(source::SourceOrigin::Toolchain)
        {
            return Ok(None);
        }
        if !program
            .machine_state_handles(owner.states)
            .iter()
            .any(|state| program.machine_state(*state).symbol == target_symbol)
        {
            return Err(Diagnostic::error(
                "interrupt installation lost its exact declaration's entry",
            ));
        }
        let [registrar, timer_slot, ..] = machine_arguments else {
            return Err(Diagnostic::error(
                "interrupt installation needs its exact registrar application and timer slot",
            ));
        };
        if registrar.const_literal.is_some() || registrar.evidence_projection.is_some() {
            return Err(Diagnostic::error(
                "interrupt installation requires a registrar declaration, not a value or evidence projection",
            ));
        }
        let mut declaration_path = registrar.path.to_vec();
        let mut continuation = registrar;
        while let Some(application) = &continuation.application
            && application.member.is_valid()
        {
            continuation = crate::tables::resolved_expressions(program)
                .static_member(application.member)
                .ok_or_else(|| {
                    Diagnostic::error("interrupt registrar member has no owning resolved record")
                })?;
            declaration_path.extend_from_slice(&continuation.path);
        }
        let registrar_selection = if let Some((first, last)) =
            continuation.path.first().zip(continuation.path.last())
        {
            let span = source::SourceSpan::new(
                first.source_span().source_id,
                source::Span::new(first.source_span().span.start, last.source_span().span.end),
            );
            let mut matches = occurrences.iter().copied().filter(|occurrence| {
            program.authored_declaration_selections().get(*occurrence).is_some_and(|selection| {
                selection.source_span() == span
                    && selection.kind() == language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::StaticArgument
            })
        });
            let selected = matches.next();
            if matches.next().is_some() || (first.is_source_backed() && selected.is_none()) {
                return Err(Diagnostic::error(
                    "interrupt registrar lost its unique authored requirement selection",
                )
                .with_source_span(span));
            }
            selected
        } else {
            None
        };
        let selected = syntax_trees_to_symbol_resolved_trees::declaration_selection::resolve_signature_free_requirement(program, &declaration_path)
            .map_err(|reason| Diagnostic::error(format!("interrupt installation registrar declaration cannot be selected: {reason:?}")))?;
        let registrar_trait = selected.trait_definition.symbol;
        let mut registrar =
            lower_static_machine_argument(Some(program), target_trees, exposure, registrar)?;
        let mut member_handle = registrar
            .application
            .as_ref()
            .map(|application| application.member)
            .unwrap_or_default();
        if !member_handle.is_valid() {
            registrar.symbol = selected.requirement.symbol;
        } else {
            loop {
                let member = target_trees
                    .expression_table
                    .static_member_mut(member_handle)
                    .ok_or_else(|| {
                        Diagnostic::error("interrupt registrar member has no owning typed record")
                    })?;
                let next = member
                    .application
                    .as_ref()
                    .map(|application| application.member)
                    .unwrap_or_default();
                if !next.is_valid() {
                    member.symbol = selected.requirement.symbol;
                    break;
                }
                member_handle = next;
            }
        }
        if !timer_slot.symbol.is_valid()
            || program.symbols.get(timer_slot.symbol).kind != SymbolKind::Conformance
            || timer_slot.const_literal.is_some()
            || timer_slot.evidence_projection.is_some()
        {
            return Err(Diagnostic::error(
                "interrupt installation must identify one exact private callback slot conformance",
            ));
        }
        Ok(Some(InstalledInterruptOperationRequest {
            registrar_application: Default::default(),
            registrar_target: Default::default(),
            registrar_receiving_scope: Default::default(),
            registrar_selection,
            registrar_trait,
            registrar,
            timer_slot: lower_static_machine_argument(
                Some(program),
                target_trees,
                exposure,
                timer_slot,
            )?,
        }))
    }
}

pub(crate) mod product_queries {
    use crate::lowerer::name::lower_name;
    use diagnostics::Diagnostic;
    use language_semantics::build_requests::is_build_protocol;
    use symbol_resolved_trees::SymbolResolvedTrees;
    use symbols::SymbolHandle;
    use typed_trees::expression::{
        ExpressionHandle, ExpressionNode, TableCallExpression, TableNamePath,
    };

    pub(crate) struct ProductQueryOperation {
        pub namespace: SymbolHandle,
        pub target: SymbolHandle,
    }

    pub(crate) fn operation(
        source: &SymbolResolvedTrees,
        namespace: SymbolHandle,
        target: SymbolHandle,
        name: &str,
    ) -> Option<ProductQueryOperation> {
        if !source.symbols.is_build_prelude_symbol(namespace)
            || (target.is_valid() && !source.symbols.is_build_prelude_symbol(target))
        {
            return None;
        }
        let name = match (source.symbols.name(namespace), name) {
            ("BuildProduct", "entry" | "schema" | "provider") => name,
            ("ProductTypeSchema", "path") => "schema_path",
            ("ProductProviderRef", "path") => "provider_path",
            _ => return None,
        };
        let protocol = source.traits.iter().find(|definition| {
            definition.is_boundary
                && is_build_protocol(&source.symbols, definition.symbol, "BuildProductOperations")
        })?;
        source
            .trait_machine_signatures(protocol.machines)
            .iter()
            .find(|signature| signature.name.as_str() == name)
            .map(|signature| ProductQueryOperation {
                namespace: protocol.symbol,
                target: signature.symbol,
            })
    }

    pub(crate) fn lower(
        source: &SymbolResolvedTrees,
        target: &mut typed_trees::TypedTrees,
        operation: ProductQueryOperation,
        arguments: Vec<ExpressionHandle>,
        span: source::SourceSpan,
        acknowledgement: language_semantics::CallOperationalAcknowledgement,
    ) -> Result<ExpressionHandle, Diagnostic> {
        let protocol = source
            .traits
            .iter()
            .find(|definition| definition.symbol == operation.namespace)
            .ok_or_else(|| {
                Diagnostic::error("Product query lost its toolchain protocol")
                    .with_source_span(span)
            })?;
        let signature = source
            .trait_machine_signatures(protocol.machines)
            .iter()
            .find(|signature| signature.symbol == operation.target)
            .ok_or_else(|| {
                Diagnostic::error("Product query lost its exact operation").with_source_span(span)
            })?;
        let table = &mut target.expression_table;
        let mut members = Default::default();
        let mut symbols = Default::default();
        table.push_name_path_member(&mut members, lower_name(&protocol.name));
        table.push_name_path_member_symbol(&mut symbols, protocol.symbol);
        let receiver = table.insert(ExpressionNode::Name(TableNamePath::new(
            members,
            symbols,
            protocol.symbol,
            protocol.symbol,
        )));
        let arguments = table.insert_expression_handles(arguments);
        let expression = table.insert(ExpressionNode::Call(TableCallExpression {
            runtime_call: None,
            authored_call_selection: None,
            authored_call_binding: None,
            authored_machine_application: Default::default(),
            exact_coercion_target: Default::default(),
            requirement_application: Default::default(),
            receiver,
            target_symbol: operation.target,
            static_machine_parameter: Default::default(),
            target: lower_name(&signature.name),
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
            operational_acknowledgement: acknowledgement,
        }));
        table.set_source_span(expression, span);
        Ok(expression)
    }
}

pub(crate) mod product_selections {
    use crate::expressions::expression::lower_static_machine_argument;
    use diagnostics::Diagnostic;
    use language_semantics::declaration_selection;
    use language_semantics::declaration_selection::{
        BuildOperation, BuildSelectionRequest, ProductDeclarationOperand,
        ProductDeclarationRequest, ProductOperandRole,
    };
    use symbols::JoinedPath;

    /*
        `builder.tests.group<Requirement>()` is the testing facet's spelling of
        a designated Build registration (wiki/spec/build/testing.md). It shares
        the one designated-operand carrier with `Build::<operation><…>(builder)`:
        the `tests` facet becomes the one explicit argument and the Build
        prelude data the namespace, so typing, checking, execution and admission
        never see a second receiver-shaped route. A resolved user method keeps
        ordinary resolution; only the unresolved toolchain spelling with no
        ordinary arguments takes this route. Its preparation (`prepare_tests`)
        is typed against `BuildTests` and authenticated by the evaluator as the
        activation's original Build facet, never a copied or authored value.
    */
    pub(crate) fn test_group_facet(
        source: &symbol_resolved_trees::SymbolResolvedTrees,
        expressions: &symbol_resolved_trees::expression::ExpressionTable,
        receiver: symbol_resolved_trees::expression::ExpressionHandle,
        target: symbols::SymbolHandle,
        name: &str,
        ordinary_count: usize,
    ) -> Option<symbols::SymbolHandle> {
        use symbol_resolved_trees::expression::ExpressionNode;
        if !BuildOperation::from_call_target(name).is_some_and(BuildOperation::uses_tests_facet)
            || target.is_valid()
            || ordinary_count != 0
            || !receiver.is_valid()
        {
            return None;
        }
        let facet = match expressions.expression(receiver) {
            ExpressionNode::Member(member) => {
                member.case_variant.is_none() && member.member.as_str() == "tests"
            }
            ExpressionNode::Name(path) => {
                path.members.count() >= 2
                    && expressions
                        .name_path_members(path.members)
                        .last()
                        .is_some_and(|member| member.as_str() == "tests")
            }
            _ => false,
        };
        if !facet {
            return None;
        }
        source
            .data_definitions
            .iter()
            .find(|data| {
                data.name.as_str() == "Build" && source.symbols.is_build_prelude_symbol(data.symbol)
            })
            .map(|data| data.symbol)
    }

    pub(crate) fn request(
        source: &symbol_resolved_trees::SymbolResolvedTrees,
        typed: &mut typed_trees::TypedTrees,
        exposure: Option<declaration_selection::AuthoredDeclarationSelectionExposure>,
        occurrences: &[declaration_selection::AuthoredDeclarationSelectionOccurrenceId],
        namespace: symbols::SymbolHandle,
        target: symbols::SymbolHandle,
        name: &str,
        operands: &[symbol_resolved_trees::expression::StaticMachineArgument],
        ordinary_count: usize,
        evidence_count: usize,
        span: source::SourceSpan,
    ) -> Result<Option<typed_trees::expression::BuildSelectionRequest>, Diagnostic> {
        if target.is_valid() && source.symbols.is_build_prelude_symbol(target) {
            use language_semantics::declaration_selection::{
                EnumExclusionRequest, ExclusionVocabulary,
            };
            let vocabulary = match name {
                "exclude_crash" => Some(ExclusionVocabulary::CrashCause),
                "exclude_physical_authority" => Some(ExclusionVocabulary::PhysicalAuthorityClass),
                _ => None,
            };
            if let Some(vocabulary) = vocabulary {
                if ordinary_count != 2 || evidence_count != 0 || !operands.is_empty() {
                    return Err(Diagnostic::error(
                    "behavior exclusion requires its mutable Build and one evaluated enum value",
                )
                .with_source_span(span));
                }
                return Ok(Some(BuildSelectionRequest::Exclusion(
                    EnumExclusionRequest {
                        source_span: span,
                        vocabulary,
                    },
                )));
            }
        }
        let Some(operation) = BuildOperation::from_call_target(name)
            .filter(|operation| operation.has_designated_operands())
        else {
            return Ok(None);
        };
        if target.is_valid()
            || !namespace.is_valid()
            || !source.symbols.is_build_prelude_symbol(namespace)
            || !source
                .data_definitions
                .iter()
                .any(|data| data.symbol == namespace && data.name.as_str() == "Build")
        {
            return Ok(None);
        }
        let operand_count_valid = if operation == BuildOperation::ProviderReadiness {
            operands.len() == 2
        } else {
            operation.product_operand_count_valid(operands.len())
        };
        if !operand_count_valid
            || evidence_count != 0
            || if operation == BuildOperation::NativeBinding {
                ordinary_count != 3
            } else if operation == BuildOperation::ProviderSelection {
                !(2..=3).contains(&ordinary_count)
            } else {
                ordinary_count != 1
            }
        {
            return Err(Diagnostic::error(format!(
            "{} requires {} and {}",
            operation.authored_spelling(),
            match operation {
                BuildOperation::NativeBinding => "one exact requirement operand",
                BuildOperation::ServiceExclusion
                | BuildOperation::ServiceAudit
                | BuildOperation::BoundaryAcceptance => "one bare declaration operand",
                BuildOperation::TestGroupRegistration => "one exact trait machine requirement operand",
                BuildOperation::TestGroupService =>
                    "a registered group's trait machine requirement and one boundary service slot operand",
                BuildOperation::WireCompatibilityRequest =>
                    "four schema declaration operands and compatibility facts",
                _ => "two bare declaration operands",
            },
            if operation == BuildOperation::NativeBinding {
                "its explicit mutable Build, catalog target and owned native locator arguments"
            } else if operation == BuildOperation::ProviderSelection {
                "its explicit mutable Build and catalog target arguments (plus an optional composition mode)"
            } else {
                "its explicit mutable Build argument"
            },
        ))
        .with_source_span(span));
        }
        let requirement = if operation == BuildOperation::ProviderReadiness {
            let operand =
                lower_static_machine_argument(Some(source), typed, exposure, &operands[0])?;
            let reference = typed
                .static_operand_type_reference(&operand)
                .map_err(|reason| Diagnostic::error(reason).with_source_span(span))?;
            let requirement = typed.type_reference_table.type_symbol(reference);
            if !source
                .traits
                .iter()
                .any(|definition| definition.symbol == requirement && definition.is_boundary)
            {
                return Err(Diagnostic::error("provider readiness requires a pre-known exact boundary requirement; only the backing may be deferred")
                .with_source_span(span));
            }
            Some(reference)
        } else {
            None
        };
        let operands = operands.iter().skip(usize::from(requirement.is_some())).enumerate().map(|(position, operand)| {
        let nominal_application = operand.application.is_some() && (matches!(operation.product_operand_role(position), Some(ProductOperandRole::BoundarySubject | ProductOperandRole::Provider)) || operation == BuildOperation::NativeBinding);
        if operand.type_reference.is_valid() || (operand.application.is_some() && !nominal_application)
            || operand.const_literal.is_some() || operand.evidence_projection.is_some()
            || operand.path.is_empty() {
            return Err(Diagnostic::error("Build selection operands must name bare declarations, not applied types, conformance applications, values or projections")
                .with_source_span(span));
        }
        let first = operand.path.first().expect("nonempty path checked").source_span();
        let last = operand.path.last().expect("nonempty path checked").source_span();
        if first.source_id != last.source_id || first.span.start >= last.span.end {
            return Err(Diagnostic::error("Product operand has no single authored source coordinate")
                .with_source_span(span));
        }
        let application = nominal_application.then(|| crate::expressions::capture_product_operands::capture(source, typed, exposure, occurrences, operand).map(std::sync::Arc::new)).transpose()?;
        let leading = source::SourceSpan::new(first.source_id, source::Span::new(first.span.start, last.span.end));
        let source_span = application.as_ref().map_or(leading, |graph| graph.authored_operand_span(leading));
        Ok(ProductDeclarationOperand {
            application,
            path: operand.path.joined_path(),
            source_span,
        })
    }).collect::<Result<Vec<_>, Diagnostic>>()?.into_boxed_slice();
        if let Some(requirement) = requirement {
            return Ok(Some(BuildSelectionRequest::ProviderReadiness(
                declaration_selection::ProviderReadinessRequest {
                    source_span: span,
                    requirement,
                    backing: operands[0].clone(),
                },
            )));
        }
        Ok(Some(BuildSelectionRequest::Declaration(
            std::sync::Arc::new(ProductDeclarationRequest {
                source_span: span,
                operation,
                operands,
            }),
        )))
    }
}

pub(crate) mod qualification_casts {
    use crate::tables::expression_table;
    use crate::type_reference::domain_constraints::normalize_domain_constraints_for_type;
    use diagnostics::Diagnostic;
    use language_semantics::declaration_selection::{
        AuthoredDeclarationSelectionKind, AuthoredDeclarationSelectionLateBinding,
        AuthoredDeclarationSelectionTarget,
    };
    use symbols::SymbolHandle;
    use typed_trees::TypedTrees;
    use typed_trees::domain::indexed_domain_instance_name;
    use typed_trees::expression::ExpressionNode;
    use typed_trees::types::{
        DomainConstraint, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
    };

    pub(crate) fn normalize_qualification_casts(
        source: &symbol_resolved_trees::SymbolResolvedTrees,
        program: &mut TypedTrees,
        expression_frontier: usize,
        cast_destinations: &[TypeReferenceHandle],
    ) -> Result<(), Diagnostic> {
        crate::domain_cast_accounting::account(
            source,
            program,
            expression_frontier,
            cast_destinations,
        )?;
        let sites = expression_table(program)
            .expression_entries()
            .filter_map(|(handle, expression)| {
                if usize::try_from(handle.arena_index())
                    .is_ok_and(|index| index <= expression_frontier)
                {
                    return None;
                }
                let ExpressionNode::Cast(cast) = expression else {
                    return None;
                };
                Some((handle, *cast))
            })
            .collect::<Vec<_>>();
        let mut updates = Vec::with_capacity(sites.len());

        for (handle, cast) in sites {
            // A run typing deferred to its declaration has no types to settle.
            if cast.form.entry().is_some() && !cast.result_type.is_valid() {
                continue;
            }
            if cast.semantic_domain.is_empty() {
                // An accounted policy entry already carries its exact result.
                if cast.form.entry().is_some() {
                    continue;
                }
                let result = if cast.domain == numerics::arithmetic::ArithmeticDomain::Exact {
                    Some(cast.target_type)
                } else if let TypeReferenceNode::Named { symbol, .. } = program
                    .type_reference_table
                    .type_reference(cast.target_type)
                {
                    program
                        .type_reference_table
                        .find_arithmetic_result_type_reference(*symbol, cast.domain)
                } else {
                    program
                        .type_reference_table
                        .find_policy_qualified_type_reference(cast.target_type, cast.domain)
                };
                if let ExpressionNode::Cast(target) =
                    program.expression_table.expression_mut(handle)
                {
                    target.result_type = result.unwrap_or_default();
                }
                continue;
            }
            let matching_occurrences = expression_table(program)
                .authored_selection_occurrences(handle)
                .filter(|occurrence| {
                    program
                        .authored_declaration_selections()
                        .get(*occurrence)
                        .is_some_and(|selection| {
                            selection.kind() == AuthoredDeclarationSelectionKind::DomainMembership
                        })
                })
                .collect::<Vec<_>>();
            let [authored_occurrence] = matching_occurrences.as_slice() else {
                return Err(Diagnostic::error(format!(
                    "semantic qualification cast retains {} authored domain-selection occurrences; expected exactly one",
                    matching_occurrences.len(),
                )));
            };
            let authored_occurrence = *authored_occurrence;
            let Some(authored_selection) = program
                .authored_declaration_selections()
                .get(authored_occurrence)
            else {
                return Err(Diagnostic::error(
                    "semantic qualification cast retains an unknown authored domain-selection occurrence",
                ));
            };
            if authored_selection.source_span().span.start
                >= authored_selection.source_span().span.end
                || !matches!(
                    authored_selection.target(),
                    AuthoredDeclarationSelectionTarget::LateBound(
                        AuthoredDeclarationSelectionLateBinding::CheckedDomainMembership
                    ) | AuthoredDeclarationSelectionTarget::Resolved(_)
                )
            {
                return Err(
                Diagnostic::error(
                    "semantic qualification cast authored domain custody is empty or has the wrong binding family",
                )
                .with_source_span(authored_selection.source_span()),
            );
            }
            let name = expression_table(program).name_path_string(cast.semantic_domain);
            let arguments = program
                .type_reference_table
                .type_reference_handles(cast.semantic_domain_arguments)
                .to_vec();
            // An entry's target already carries the kept domains; selection reads the carrier.
            let carrier = if cast.form.entry().is_some() {
                program
                    .type_reference_table
                    .unconstrained_type_reference(cast.target_type)
            } else {
                cast.target_type
            };
            let matches = crate::type_reference::domain_constraints::select_domain_candidates(
                source,
                program,
                carrier,
                &name,
                Some(authored_selection.source_span()),
                SymbolHandle::invalid(),
                &arguments,
            );
            let [domain] = matches.as_slice() else {
                updates.push((
                    handle,
                    authored_occurrence,
                    SymbolHandle::invalid(),
                    language_semantics::SemanticDomainId::NULL,
                ));
                continue;
            };
            let index_parameters = typed_trees::domain::index_parameters(program, domain);
            if arguments.len() != index_parameters.len() {
                return Err(Diagnostic::error(format!(
                    "domain family `{}` requires {} closed index argument(s), but {} were supplied by `as ... in {name}`",
                    domain.name,
                    index_parameters.len(),
                    arguments.len(),
                )));
            }
            let instance_name =
                indexed_domain_instance_name(program, domain, index_parameters, &arguments)?;
            let semantic_id = if arguments.is_empty() {
                domain.semantic_id
            } else {
                program.semantic_domains.intern(&instance_name)
            };
            updates.push((handle, authored_occurrence, domain.symbol, semantic_id));
        }

        let mut selections = program.authored_declaration_selections().clone();
        for (_, authored_occurrence, domain, _) in &updates {
            if !domain.is_valid() {
                continue;
            }
            if let Some(resolved) = selections
                .get(*authored_occurrence)
                .and_then(|selection| selection.target().as_resolved().copied())
            {
                if resolved.selected_symbol() != *domain {
                    return Err(Diagnostic::error(
                        "copied qualification cast changes its authored domain declaration",
                    ));
                }
                continue;
            }
            selections
                .finalize_late_bound(
                    *authored_occurrence,
                    AuthoredDeclarationSelectionLateBinding::CheckedDomainMembership,
                    *domain,
                )
                .map_err(|error| {
                    Diagnostic::error(format!(
                        "failed to finalize semantic qualification-cast domain custody: {error:?}",
                    ))
                })?;
        }
        program.retain_authored_declaration_selections(selections);

        for (handle, _, domain, semantic_id) in updates {
            let Some(authored) = program.expression_table.cast_node(handle) else {
                continue;
            };
            let authored = *authored;
            if let Some(entry) = authored.form.entry() {
                /*
                    Accounting fixed this entry's target and result. Only an
                    added domain stays on the node, as the membership the cast
                    establishes; a kept or removed one was named for the
                    accounting and its selection is recorded above.
                */
                let ExpressionNode::Cast(cast) = program.expression_table.expression_mut(handle)
                else {
                    continue;
                };
                if entry.kind == language_core::cast_form::CastEntryKind::Add {
                    cast.semantic_domain_symbol = domain;
                    cast.semantic_domain_id = semantic_id;
                } else {
                    cast.semantic_domain = arena::HandleSpan::empty();
                    cast.semantic_domain_arguments = arena::HandleSpan::empty();
                }
                continue;
            }
            let result_type = if let Some(declaration) = program.domain_definition(domain) {
                let qualification = DomainConstraint {
                    name: declaration.name.clone(),
                    symbol: domain,
                    arguments: program
                        .type_reference_table
                        .type_reference_handles(authored.semantic_domain_arguments)
                        .to_vec(),
                    ..DomainConstraint::default()
                };
                let constraints = program
                    .type_reference_table
                    .insert_constraints([TypeConstraintNode::Domain(qualification)]);
                let result = program
                    .type_reference_table
                    .insert(TypeReferenceNode::Constrained {
                        base_type: authored.target_type,
                        constraints,
                    });
                normalize_domain_constraints_for_type(source, program, result)?;
                result
            } else {
                TypeReferenceHandle::invalid()
            };
            let ExpressionNode::Cast(cast) = program.expression_table.expression_mut(handle) else {
                continue;
            };
            cast.semantic_domain_symbol = domain;
            cast.semantic_domain_id = semantic_id;
            cast.result_type = result_type;
            if cast.form.is_recast() && result_type.is_valid() {
                cast.target_type = result_type;
            }
        }
        Ok(())
    }
}

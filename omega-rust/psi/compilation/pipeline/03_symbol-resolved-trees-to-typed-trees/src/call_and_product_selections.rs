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
            (BuildSelectionRequest::Declaration(binding), 3)
                if binding.operation == BuildOperation::NativeBinding =>
            {
                "bind_native"
            }
            (BuildSelectionRequest::Declaration(binding), 1)
                if binding.operation.selects_product_declarations()
                    && binding.operation != BuildOperation::ProviderSelection =>
            {
                "select"
            }
            (BuildSelectionRequest::Declaration(binding), 2 | 3)
                if binding.operation == BuildOperation::ProviderSelection =>
            {
                "select_provider"
            }
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
    use diagnostics::Diagnostic;
    use symbols::SymbolHandle;
    use typed_trees::TypedTrees;
    use typed_trees::expression::{ExpressionHandle, ExpressionNode, StaticMachineArgument};
    use typed_trees::statement::{StatementNode, TransitionTargetNode};
    use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};
    use validation::selected_construction_field_type;

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

        for (handle, expression) in program.expression_table.expression_entries() {
            if usize::try_from(handle.arena_index()).is_ok_and(|index| index <= expression_frontier)
            {
                continue;
            }
            match expression {
                ExpressionNode::StructLiteral(literal) => {
                    for field in program.expression_table.struct_fields(literal.fields) {
                        if let Some(field_type) = selected_construction_field_type(
                            program,
                            handle,
                            literal,
                            field.field_symbol,
                        ) {
                            destinations.push((field.value, field_type, literal.type_symbol));
                        }
                    }
                }
                ExpressionNode::Call(call) => {
                    let arguments = program.expression_table.expression_handles(call.arguments);
                    if let Some(required) =
                        validation::mathematical_applications::argument_type_references(
                            program, call,
                        )
                    {
                        // Mathematical declarations own their value-slot types too.
                        // The common landing walk still checks the constructor and
                        // prepares each field under that complete receiving type.
                        for (argument, reference) in arguments.iter().zip(required) {
                            if let Some(reference) = reference
                                && needs_destination(program, *argument)
                            {
                                destinations.push((*argument, reference, call.target_symbol));
                            }
                        }
                    } else {
                        collect_call_destinations(
                            program,
                            call.target_symbol,
                            call.runtime_call.map(|binding| binding.contract),
                            &call.machine_arguments,
                            arguments,
                            &mut calls,
                        );
                    }
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
                            if let Some(destination) = validation::assignment_stored_value_type(
                                program, machine, state, assignment,
                            ) {
                                destinations.push((
                                    assignment.value,
                                    destination,
                                    SymbolHandle::invalid(),
                                ));
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
                    &program.runtime_callable_contracts.get(contract).signature,
                )
            } else if let Some(parameters) =
                validation::call_target_parameters(program, call.target)
            {
                parameters
            } else {
                continue;
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
        for (value, destination, lifetime_owner) in destinations {
            land_one(
                program,
                value,
                destination,
                lifetime_owner,
                &mut cast_destinations,
            )?;
        }
        Ok(cast_destinations)
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
        mut lifetime_owner: SymbolHandle,
        cast_destinations: &mut Vec<TypeReferenceHandle>,
    ) -> Result<(), Diagnostic> {
        if !value.is_valid() || !destination.is_valid() {
            return Ok(());
        }
        if crate::runtime_callables::prepare_named_value(
            program,
            value,
            destination,
            lifetime_owner,
        )? {
            return Ok(());
        }
        if let Some(dispatch) = program.expression_table.match_node(value) {
            let arms = program.expression_table.match_arms(dispatch.arms).to_vec();
            for arm in arms {
                land_one(
                    program,
                    arm.value,
                    destination,
                    lifetime_owner,
                    cast_destinations,
                )?;
            }
            return Ok(());
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
            // An application with a runtime value index is not final here.
            // Stage 04 snapshots a computed operand into its own local and
            // replaces a specialized one, then records the application it
            // produced. Recording the authored application now would leave
            // the literal checked against operands that no longer exist.
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
                return Ok(());
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
            for field in fields {
                if let Some(field_type) =
                    selected_construction_field_type(program, value, &literal, field.field_symbol)
                {
                    land_one(
                        program,
                        field.value,
                        field_type,
                        literal.type_symbol,
                        cast_destinations,
                    )?;
                }
            }
            return Ok(());
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
            for element in elements {
                land_one(
                    program,
                    element,
                    element_type,
                    lifetime_owner,
                    cast_destinations,
                )?;
            }
            return Ok(());
        }
        validation::land_owned_fixed_byte_literal(program, value, destination)
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
        let nominal_application = operand.application.is_some() && matches!(operation.product_operand_role(position), Some(ProductOperandRole::BoundarySubject | ProductOperandRole::Provider));
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
        super::domain_cast_accounting::account(
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
        super::domain_cast_accounting::settle(program, expression_frontier);
        Ok(())
    }
}

/*
    Domain casts: `value as =Keep -Remove +Add` (domains.md, "Domain casts").

    The parser leaves one entry node per named domain, innermost first. This
    pass reads the run as the one simultaneous cast the source wrote: it finds
    the operand's declared type, checks that the entries account for every
    domain that type carries, and then gives each node the target and result an
    ordinary qualification cast would have. A kept or removed domain becomes a
    cast to the remaining set; an added one becomes the existing single-domain
    addition over that set, so establishment, custody and lowering are the
    paths every other cast already takes.
*/
pub(crate) mod domain_cast_accounting {
    use crate::tables::expression_table;
    use crate::type_reference::domain_constraints::{
        normalize_domain_constraints_for_type, select_domain_candidates,
        select_domain_candidates_where,
    };
    use diagnostics::Diagnostic;
    use language_core::cast_form::{CastEntry, CastEntryKind, CastForm};
    use language_semantics::declaration_selection::AuthoredDeclarationSelectionKind;
    use numerics::arithmetic::ArithmeticDomain;
    use symbols::SymbolHandle;
    use typed_trees::TypedTrees;
    use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCastExpression};
    use typed_trees::types::{
        DomainConstraint, DomainConstraintSubject, TypeConstraintNode, TypeReferenceHandle,
        TypeReferenceNode,
    };

    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    enum Atom {
        Declared(SymbolHandle, language_semantics::SemanticDomainId),
        Policy(ArithmeticDomain),
    }

    type Spelled = (Atom, String, Option<TypeConstraintNode>);

    struct Member {
        atom: Atom,
        label: String,
        node: Option<TypeConstraintNode>, // The operand type's own constraint, when it spells this domain.
        attached: bool, // Listed by the type itself, not only implied as an ancestor.
    }

    struct Named {
        handle: ExpressionHandle,
        entry: CastEntry,
        atoms: Vec<Spelled>,
        label: String,
    }

    type Run = Vec<(ExpressionHandle, TableCastExpression, CastEntry)>;

    fn is_new(handle: ExpressionHandle, frontier: usize) -> bool {
        !usize::try_from(handle.arena_index()).is_ok_and(|index| index <= frontier)
    }

    fn entry_sites(
        program: &TypedTrees,
        frontier: usize,
    ) -> Vec<(ExpressionHandle, TableCastExpression)> {
        expression_table(program)
            .expression_entries()
            .filter_map(|(handle, expression)| {
                let ExpressionNode::Cast(cast) = expression else {
                    return None;
                };
                (is_new(handle, frontier) && cast.form.entry().is_some()).then_some((handle, *cast))
            })
            .collect()
    }

    /* Every entry is an ordinary value cast once its run has been accounted for. */
    pub(crate) fn settle(program: &mut TypedTrees, frontier: usize) {
        for (handle, _) in entry_sites(program, frontier) {
            if let ExpressionNode::Cast(cast) = program.expression_table.expression_mut(handle) {
                cast.form = CastForm::Value;
            }
        }
    }

    pub(crate) fn account(
        source: &symbol_resolved_trees::SymbolResolvedTrees,
        program: &mut TypedTrees,
        frontier: usize,
        cast_destinations: &[TypeReferenceHandle],
    ) -> Result<(), Diagnostic> {
        let sites = entry_sites(program, frontier);
        if sites.is_empty() {
            return Ok(());
        }
        // A non-first entry's operand is the previous entry of the same run.
        let inner: Vec<ExpressionHandle> = sites
            .iter()
            .filter(|(_, cast)| !cast.form.entry().is_some_and(|entry| entry.first))
            .map(|(_, cast)| cast.value)
            .collect();
        let owners = owners(program, &sites);
        let mut runs: Vec<Run> = Vec::new();
        for (outermost, _) in sites.iter().filter(|(handle, _)| !inner.contains(handle)) {
            let mut run = Vec::new();
            let mut cursor = *outermost;
            while let Some(cast) = program.expression_table.cast_node(cursor).copied() {
                let Some(entry) = cast.form.entry() else {
                    break;
                };
                run.push((cursor, cast, entry));
                if entry.first {
                    break;
                }
                cursor = cast.value;
            }
            run.reverse();
            runs.push(run);
        }
        // Operands are allocated before the casts over them, so inner runs settle first.
        runs.sort_by_key(|run| run[0].0.arena_index());
        for run in runs {
            account_run(source, program, &owners, &run, cast_destinations)?;
        }
        Ok(())
    }

    fn owners(
        program: &TypedTrees,
        sites: &[(ExpressionHandle, TableCastExpression)],
    ) -> Vec<(ExpressionHandle, usize, usize)> {
        use typed_trees::signature::SignatureContractKind;
        let mut owners = Vec::new();
        for (machine_position, machine) in program.machines().iter().enumerate() {
            for (state_position, state) in program.machine_states(machine).iter().enumerate() {
                let mut roots = Vec::new();
                for statement in program.statement_table.statements(state.statement_nodes) {
                    roots.extend(
                        program
                            .tables
                            .expression_roots(statement)
                            .into_iter()
                            .map(|(_, root)| root),
                    );
                }
                for kind in [
                    SignatureContractKind::Requires,
                    SignatureContractKind::Ensures,
                ] {
                    roots.extend(
                        program.contract_expressions(
                            program
                                .machine_contracts(machine)
                                .iter()
                                .chain(program.state_contracts(state)),
                            kind,
                        ),
                    );
                }
                for root in roots {
                    program.subtree_for_each(
                        root,
                        typed_trees::SubtreeEdges::STRUCTURAL,
                        |handle, node| {
                            if matches!(node, ExpressionNode::Cast(cast) if cast.form.entry().is_some())
                                && sites.iter().any(|(site, _)| *site == handle)
                                && !owners.iter().any(|(owned, _, _)| *owned == handle)
                            {
                                owners.push((handle, machine_position, state_position));
                            }
                        },
                    );
                }
            }
        }
        owners
    }

    fn refuse(program: &TypedTrees, handle: ExpressionHandle, message: String) -> Diagnostic {
        Diagnostic::error(message).with_source_span(expression_table(program).source_span(handle))
    }

    fn flatten(
        program: &TypedTrees,
        mut reference: TypeReferenceHandle,
    ) -> (TypeReferenceHandle, Vec<TypeConstraintNode>) {
        let mut layers = Vec::new();
        while let TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } = program.type_reference_table.type_reference(reference)
        {
            layers.push(
                program
                    .type_reference_table
                    .constraints(*constraints)
                    .to_vec(),
            );
            reference = *base_type;
        }
        layers.reverse();
        (reference, layers.into_iter().flatten().collect())
    }

    /* The domain whose path this domain's own path extends by one member. */
    fn parent(program: &TypedTrees, domain: SymbolHandle) -> Option<SymbolHandle> {
        let definition = program.domain_definition(domain)?;
        typed_trees::domain::subject_membership_facts(program, definition)
            .map(|membership| membership.domain_symbol)
            .find(|candidate| {
                *candidate != domain
                    && program.domain_definition(*candidate).is_some_and(|parent| {
                        definition
                            .name
                            .as_str()
                            .strip_prefix(parent.name.as_str())
                            .and_then(|rest| rest.strip_prefix("::"))
                            .is_some_and(|leaf| !leaf.contains("::"))
                    })
            })
    }

    fn is_ancestor(program: &TypedTrees, ancestor: SymbolHandle, mut child: SymbolHandle) -> bool {
        while let Some(next) = parent(program, child) {
            if next == ancestor {
                return true;
            }
            child = next;
        }
        false
    }

    fn implies(program: &TypedTrees, child: Atom, ancestor: Atom) -> bool {
        matches!((child, ancestor), (Atom::Declared(child, _), Atom::Declared(ancestor, _))
            if child != ancestor && is_ancestor(program, ancestor, child))
    }

    fn declared_atom(program: &TypedTrees, symbol: SymbolHandle) -> Option<(Atom, String)> {
        let definition = program.domain_definition(symbol)?;
        Some((
            Atom::Declared(symbol, definition.semantic_id),
            domain_label(program, definition),
        ))
    }

    /* A domain as a cast entry spells it: its path under the carrier it is declared on. */
    fn domain_label(
        program: &TypedTrees,
        definition: &typed_trees::domain::DomainDefinition,
    ) -> String {
        let name = definition.name.as_str();
        name.strip_prefix(
            program
                .display_type_reference(definition.target_type)
                .as_str(),
        )
        .and_then(|rest| rest.strip_prefix("::"))
        .unwrap_or(name)
        .to_owned()
    }

    fn declared_node(program: &TypedTrees, atom: Atom) -> Option<TypeConstraintNode> {
        let Atom::Declared(symbol, _) = atom else {
            return None;
        };
        let definition = program.domain_definition(symbol)?;
        Some(TypeConstraintNode::Domain(DomainConstraint {
            name: definition.name.clone(),
            symbol,
            ..DomainConstraint::default()
        }))
    }

    fn is_anonymous_literal(program: &TypedTrees, expression: ExpressionHandle) -> bool {
        validation::has_anonymous_numeric_results(program, expression)
    }

    fn authored_span(program: &TypedTrees, handle: ExpressionHandle) -> Option<source::SourceSpan> {
        expression_table(program)
            .authored_selection_occurrences(handle)
            .filter_map(|occurrence| program.authored_declaration_selections().get(occurrence))
            .find(|selection| {
                selection.kind() == AuthoredDeclarationSelectionKind::DomainMembership
            })
            .map(|selection| selection.source_span())
    }

    /* The declared type of an expression inside the machine state that owns this run. */
    fn declared_type(
        program: &TypedTrees,
        owners: &[(ExpressionHandle, usize, usize)],
        run: &Run,
        expression: ExpressionHandle,
    ) -> Option<TypeReferenceHandle> {
        let outermost = run[run.len() - 1].0;
        let (_, machine, state) = owners.iter().find(|(owned, _, _)| *owned == outermost)?;
        let machine = &program.machines()[*machine];
        let state = &program.machine_states(machine)[*state];
        let checking = validation::CheckingContext::new(program);
        validation::expression_result_type_reference(&checking, machine, state, expression)
    }

    /*
        `value as T +D`: the carrier cast in front of the entries takes an
        operand with no domains (domains.md, "Carrier casts"). Without this
        check the conversion would drop whatever `value` carried without any
        entry naming it, which is the silent removal entries exist to end.
    */
    fn refuse_qualified_carrier_operand(
        program: &TypedTrees,
        owners: &[(ExpressionHandle, usize, usize)],
        run: &Run,
    ) -> Result<(), Diagnostic> {
        let (first, first_cast, entry) = run[0];
        if !entry.after_carrier {
            return Ok(());
        }
        let Some(carrier_cast) = program.expression_table.cast_node(first_cast.value) else {
            return Ok(());
        };
        let target = program.display_type_reference(carrier_cast.target_type);
        let Some(converted) = declared_type(program, owners, run, carrier_cast.value) else {
            if is_anonymous_literal(program, carrier_cast.value) {
                return Ok(());
            }
            return Err(refuse(
                program,
                first,
                format!(
                    "cannot read the declared type of the value converted by `as {target}`; a carrier cast followed by domain entries takes an operand with no domains, so bind the operand to a `let` with its type first",
                ),
            ));
        };
        let (_, constraints) = flatten(program, converted);
        let carried = constraints
            .iter()
            .filter_map(|constraint| match constraint {
                TypeConstraintNode::ArithmeticDomain(ArithmeticDomain::Exact) => None,
                TypeConstraintNode::ArithmeticDomain(policy) => Some(policy.name().to_owned()),
                TypeConstraintNode::Domain(domain) => {
                    Some(program.domain_definition(domain.symbol).map_or_else(
                        || domain.name.as_str().to_owned(),
                        |definition| domain_label(program, definition),
                    ))
                }
                TypeConstraintNode::Named(name) => Some(name.as_str().to_owned()),
            })
            .collect::<Vec<_>>();
        if carried.is_empty() {
            return Ok(());
        }
        let removal = carried
            .iter()
            .map(|label| format!("-{label}"))
            .collect::<Vec<_>>()
            .join(" ");
        Err(refuse(
            program,
            first,
            format!(
                "carrier cast `as {target}` converts a value whose type `{}` carries domain `{}`; a carrier cast takes an operand with no domains, so remove them by name first: `value as {removal} as {target} {}`",
                program.display_type_reference_with_constraints(converted),
                carried.join("`, `"),
                entry_label(program, &first_cast),
            ),
        ))
    }

    fn entry_label(program: &TypedTrees, cast: &TableCastExpression) -> String {
        expression_table(program)
            .name_path_string(cast.target_label)
            .trim()
            .to_owned()
    }

    /* An anonymous operand takes its receiving carrier, or a carrier fixed
    by a named domain when no receiving type is available. */
    fn operand_type(
        source: &symbol_resolved_trees::SymbolResolvedTrees,
        program: &TypedTrees,
        owners: &[(ExpressionHandle, usize, usize)],
        run: &Run,
        cast_destinations: &[TypeReferenceHandle],
    ) -> Result<TypeReferenceHandle, Diagnostic> {
        let (first, first_cast, _) = run[0];
        let operand = first_cast.value;
        if let Some(reference) = declared_type(program, owners, run, operand) {
            return Ok(reference);
        }
        let label = entry_label(program, &first_cast);
        if !is_anonymous_literal(program, operand) {
            return Err(refuse(
                program,
                first,
                format!(
                    "cannot read the declared type of the operand of `as {label}`; a domain cast accounts for the domains its operand's type carries, so bind the operand to a `let` with its type first",
                ),
            ));
        }
        if let Some(destination) = cast_destinations
            .get(run[run.len() - 1].0.arena_index() as usize)
            .filter(|destination| destination.is_valid())
        {
            // The anonymous operand starts unqualified. Taking the full
            // expected type here would silently insert its domains before
            // the authored entries have accounted for them.
            return Ok(flatten(program, *destination).0);
        }
        for (handle, cast, _) in run {
            if cast.semantic_domain.is_empty() {
                continue;
            }
            let name = expression_table(program).name_path_string(cast.semantic_domain);
            let candidates = select_domain_candidates_where(
                source,
                program,
                &name,
                authored_span(program, *handle),
                SymbolHandle::invalid(),
                |domain| !typed_trees::domain::has_generic_carrier(program, domain),
            );
            if let [domain] = candidates.as_slice() {
                return Ok(domain.target_type);
            }
        }
        Err(refuse(
            program,
            first,
            format!(
                "the literal in `as {label}` has no carrier that the named domain fixes; write the carrier first, as in `5 as i32 +Km`",
            ),
        ))
    }

    /* What the operand's type carries: its own constraints and each one's ancestors. */
    fn carried(
        program: &TypedTrees,
        run: &Run,
        constraints: &[TypeConstraintNode],
        operand_display: &str,
    ) -> Result<Vec<Member>, Diagnostic> {
        let (first, first_cast, _) = run[0];
        let mut members: Vec<Member> = Vec::new();
        for constraint in constraints {
            match constraint {
                TypeConstraintNode::ArithmeticDomain(ArithmeticDomain::Exact) => {}
                TypeConstraintNode::ArithmeticDomain(policy) => members.push(Member {
                    atom: Atom::Policy(*policy),
                    label: policy.name().to_owned(),
                    node: Some(constraint.clone()),
                    attached: true,
                }),
                TypeConstraintNode::Domain(domain)
                    if domain.subject == DomainConstraintSubject::Declared
                        && domain.symbol.is_valid() =>
                {
                    let atom = Atom::Declared(domain.symbol, domain.semantic_id);
                    match members.iter_mut().find(|member| member.atom == atom) {
                        Some(member) => {
                            member.attached = true;
                            member.node = Some(constraint.clone());
                        }
                        None => members.push(Member {
                            atom,
                            label: program.domain_definition(domain.symbol).map_or_else(
                                || domain.name.as_str().to_owned(),
                                |definition| domain_label(program, definition),
                            ),
                            node: Some(constraint.clone()),
                            attached: true,
                        }),
                    }
                    let mut ancestor = parent(program, domain.symbol);
                    while let Some(symbol) = ancestor {
                        if let Some((atom, label)) = declared_atom(program, symbol)
                            && !members.iter().any(|member| member.atom == atom)
                        {
                            members.push(Member {
                                atom,
                                label,
                                node: None,
                                attached: false,
                            });
                        }
                        ancestor = parent(program, symbol);
                    }
                }
                TypeConstraintNode::Domain(domain) => {
                    return Err(refuse(
                        program,
                        first,
                        format!(
                            "`as {}` cannot yet account for the compiler-owned domain `{}` carried by `{operand_display}`",
                            entry_label(program, &first_cast),
                            domain.name,
                        ),
                    ));
                }
                TypeConstraintNode::Named(name) => {
                    return Err(refuse(
                        program,
                        first,
                        format!(
                            "`as {}` cannot account for the unresolved constraint `{name}` carried by `{operand_display}`",
                            entry_label(program, &first_cast),
                        ),
                    ));
                }
            }
        }
        Ok(members)
    }

    /* Which domains the entries name, each at most once. */
    fn named_domains(
        source: &symbol_resolved_trees::SymbolResolvedTrees,
        program: &mut TypedTrees,
        run: &Run,
        carrier: TypeReferenceHandle,
    ) -> Result<Vec<Named>, Diagnostic> {
        let mut named: Vec<Named> = Vec::new();
        for (handle, cast, entry) in run {
            let label = entry_label(program, cast);
            let atoms: Vec<Spelled> = if cast.semantic_domain.is_empty() {
                vec![(
                    Atom::Policy(cast.domain),
                    cast.domain.name().to_owned(),
                    Some(TypeConstraintNode::ArithmeticDomain(cast.domain)),
                )]
            } else {
                let name = expression_table(program).name_path_string(cast.semantic_domain);
                let arguments = program
                    .type_reference_table
                    .type_reference_handles(cast.semantic_domain_arguments)
                    .to_vec();
                let candidates = select_domain_candidates(
                    source,
                    program,
                    carrier,
                    &name,
                    authored_span(program, *handle),
                    SymbolHandle::invalid(),
                    &arguments,
                );
                let [domain] = candidates.as_slice() else {
                    let reason = if candidates.is_empty() {
                        "names no visible domain"
                    } else {
                        "names more than one visible domain"
                    };
                    return Err(refuse(
                        program,
                        *handle,
                        format!(
                            "cast entry `{label}` {reason} `{name}` of `{}`. A `+` or `-` after a cast always begins a domain entry; for arithmetic on the cast result, parenthesise the cast",
                            program.display_type_reference(carrier),
                        ),
                    ));
                };
                if let Some(alias) = &domain.alias {
                    // Naming an alias names its expansion.
                    alias
                        .constituents
                        .iter()
                        .filter_map(|constituent| declared_atom(program, constituent.domain_symbol))
                        .map(|(atom, label)| (atom, label, None))
                        .collect()
                } else {
                    let semantic_id = if arguments.is_empty() {
                        domain.semantic_id
                    } else {
                        let instance = typed_trees::domain::indexed_domain_instance_name(
                            program,
                            domain,
                            typed_trees::domain::index_parameters(program, domain),
                            &arguments,
                        )?;
                        program.semantic_domains.intern(&instance)
                    };
                    vec![(
                        Atom::Declared(domain.symbol, semantic_id),
                        domain_label(program, domain),
                        Some(TypeConstraintNode::Domain(DomainConstraint {
                            name: domain.name.clone(),
                            symbol: domain.symbol,
                            arguments,
                            ..DomainConstraint::default()
                        })),
                    )]
                }
            };
            for (atom, domain_label, _) in &atoms {
                if named
                    .iter()
                    .any(|earlier| earlier.atoms.iter().any(|(seen, _, _)| seen == atom))
                {
                    return Err(refuse(
                        program,
                        *handle,
                        format!(
                            "cast entry `{label}` names domain `{domain_label}` a second time in one cast; each domain appears exactly once, as `=`, `-` or `+`",
                        ),
                    ));
                }
            }
            named.push(Named {
                handle: *handle,
                entry: *entry,
                atoms,
                label,
            });
        }
        Ok(named)
    }

    fn account_run(
        source: &symbol_resolved_trees::SymbolResolvedTrees,
        program: &mut TypedTrees,
        owners: &[(ExpressionHandle, usize, usize)],
        run: &Run,
        cast_destinations: &[TypeReferenceHandle],
    ) -> Result<(), Diagnostic> {
        let (first, first_cast, _) = run[0];
        let outermost = run[run.len() - 1].0;
        refuse_qualified_carrier_operand(program, owners, run)?;
        let operand_type = operand_type(source, program, owners, run, cast_destinations)?;
        if matches!(
            program.type_reference_table.type_reference(operand_type),
            TypeReferenceNode::Reference { .. }
        ) {
            return Err(refuse(
                program,
                first,
                format!(
                    "`as {}` applies to a reference; domain casts through a reference are not implemented yet (NAMED-DOMAIN-CASTS, mutable views)",
                    entry_label(program, &first_cast),
                ),
            ));
        }
        let (carrier, constraints) = flatten(program, operand_type);
        let operand_display = program.display_type_reference_with_constraints(operand_type);
        let members = carried(program, run, &constraints, &operand_display)?;
        let named = named_domains(source, program, run, carrier)?;

        // Each entry against what the operand carries.
        for entry in &named {
            for (atom, domain_label, _) in &entry.atoms {
                let carried = members.iter().any(|member| member.atom == *atom);
                match entry.entry.kind {
                    CastEntryKind::Add if carried => {
                        return Err(refuse(
                            program,
                            entry.handle,
                            format!(
                                "cast entry `{}` adds domain `{domain_label}`, but the operand's type `{operand_display}` already carries `{domain_label}`; write `={domain_label}` to keep it",
                                entry.label,
                            ),
                        ));
                    }
                    CastEntryKind::Keep | CastEntryKind::Remove if !carried => {
                        return Err(refuse(
                            program,
                            entry.handle,
                            format!(
                                "cast entry `{}` names domain `{domain_label}`, but the operand's type `{operand_display}` does not carry `{domain_label}`; only `+{domain_label}` can name a domain the operand lacks",
                                entry.label,
                            ),
                        ));
                    }
                    _ => {}
                }
            }
        }
        let kind_of = |atom: Atom| {
            named
                .iter()
                .find(|entry| entry.atoms.iter().any(|(named, _, _)| *named == atom))
                .map(|entry| entry.entry.kind)
        };
        for member in &members {
            if kind_of(member.atom).is_none() {
                return Err(refuse(
                    program,
                    outermost,
                    format!(
                        "domain cast leaves attached domain `{label}` unaccounted for: the operand's type `{operand_display}` carries `{label}`, so the cast names it as `={label}` to keep it or `-{label}` to remove it",
                        label = member.label,
                    ),
                ));
            }
        }
        let has = |kind| named.iter().any(|entry| entry.entry.kind == kind);
        if has(CastEntryKind::Remove) && has(CastEntryKind::Add) {
            return Err(refuse(
                program,
                outermost,
                "a cast that both removes and adds domains has no settled lowered form yet (NAMED-DOMAIN-CASTS, left open); write the removal and the addition as two casts, as in `(value as -A) as +B`".to_owned(),
            ));
        }

        // The result set: a child never outlives its parent.
        let mut result: Vec<Spelled> = members
            .iter()
            .filter(|member| kind_of(member.atom) == Some(CastEntryKind::Keep))
            .map(|member| (member.atom, member.label.clone(), member.node.clone()))
            .collect();
        for entry in &named {
            if entry.entry.kind == CastEntryKind::Add {
                result.extend(entry.atoms.iter().cloned());
            }
        }
        // One arithmetic policy per type: `Wrapping & Saturating` has no single meaning for `+`.
        let policies = result
            .iter()
            .filter(|(atom, _, _)| matches!(atom, Atom::Policy(_)))
            .map(|(_, label, _)| format!("`{label}`"))
            .collect::<Vec<_>>();
        if policies.len() > 1 {
            return Err(refuse(
                program,
                outermost,
                format!(
                    "domain cast would leave competing arithmetic policies {} on one type; remove one of them in the same cast",
                    policies.join(" and "),
                ),
            ));
        }
        for (atom, label, _) in &result {
            let Atom::Declared(symbol, _) = atom else {
                continue;
            };
            if let Some(parent) = parent(program, *symbol)
                && let Some((parent_atom, parent_label)) = declared_atom(program, parent)
                && !result.iter().any(|(member, _, _)| *member == parent_atom)
            {
                return Err(refuse(
                    program,
                    outermost,
                    format!(
                        "domain cast would leave `{label}` without its parent `{parent_label}`; a child domain travels with its ancestors, so keep or add `{parent_label}` as well, or remove `{label}`",
                    ),
                ));
            }
        }

        /*
            Give each entry the target and result of the ordinary cast it now
            is. The running set starts at what the operand's type lists and
            changes as each entry is applied. A type spells a child without
            its ancestors, so an ancestor is spelled only while no listed
            domain implies it.
        */
        let mut running: Vec<Spelled> = members
            .iter()
            .filter(|member| member.attached)
            .map(|member| (member.atom, member.label.clone(), member.node.clone()))
            .collect();
        for entry in &named {
            let before = spell(source, program, carrier, &running)?;
            match entry.entry.kind {
                CastEntryKind::Keep => {}
                CastEntryKind::Remove => {
                    for (atom, _, _) in &entry.atoms {
                        running.retain(|(member, _, _)| member != atom);
                    }
                    for member in &members {
                        if kind_of(member.atom) == Some(CastEntryKind::Keep)
                            && !running.iter().any(|(listed, _, _)| {
                                *listed == member.atom || implies(program, *listed, member.atom)
                            })
                        {
                            running.push((member.atom, member.label.clone(), member.node.clone()));
                        }
                    }
                }
                CastEntryKind::Add => {
                    for (atom, label, node) in &entry.atoms {
                        running.retain(|(listed, _, _)| !implies(program, *atom, *listed));
                        if !running
                            .iter()
                            .any(|(listed, _, _)| implies(program, *listed, *atom))
                        {
                            running.push((*atom, label.clone(), node.clone()));
                        }
                    }
                }
            }
            let after = spell(source, program, carrier, &running)?;
            let ExpressionNode::Cast(cast) = program.expression_table.expression_mut(entry.handle)
            else {
                continue;
            };
            match entry.entry.kind {
                CastEntryKind::Add => {
                    cast.target_type = before;
                    cast.result_type = after;
                }
                CastEntryKind::Keep | CastEntryKind::Remove => {
                    cast.target_type = after;
                    cast.result_type = after;
                    cast.domain = ArithmeticDomain::Exact;
                }
            }
        }
        Ok(())
    }

    fn spell(
        source: &symbol_resolved_trees::SymbolResolvedTrees,
        program: &mut TypedTrees,
        carrier: TypeReferenceHandle,
        members: &[Spelled],
    ) -> Result<TypeReferenceHandle, Diagnostic> {
        if members.is_empty() {
            return Ok(carrier);
        }
        if let [(Atom::Policy(policy), _, _)] = members
            && let TypeReferenceNode::Named { symbol, .. } =
                program.type_reference_table.type_reference(carrier)
            && let Some(retained) = program
                .type_reference_table
                .find_arithmetic_result_type_reference(*symbol, *policy)
        {
            return Ok(retained);
        }
        let constraints = members
            .iter()
            .filter_map(|(atom, _, node)| node.clone().or_else(|| declared_node(program, *atom)))
            .collect::<Vec<_>>();
        let constraints = program.type_reference_table.insert_constraints(constraints);
        let reference = program
            .type_reference_table
            .insert(TypeReferenceNode::Constrained {
                base_type: carrier,
                constraints,
            });
        normalize_domain_constraints_for_type(source, program, reference)?;
        Ok(reference)
    }
}

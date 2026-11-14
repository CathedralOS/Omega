/*
We represent designated product-operand declarations (`declarations::product_operand`)
and trait definitions with their conformance applications, regions, and
refinement views (`declarations::trait_definition`) in `TypedTrees`.
*/

pub mod product_operand {
    pub(crate) mod input_selections {
        use crate::expression::ExpressionHandle;
        use crate::{
            TypedTrees,
            expression::StaticMachineArgument,
            types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode},
        };
        use language_semantics::declaration_selection::AuthoredDeclarationSelectionOccurrenceId;
        use symbols::SymbolHandle;

        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
        pub enum ProductArgumentStep {
            #[default]
            Invalid,
            Argument(u32),
            Member,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
        pub enum ProductTypeSelectionSlot {
            Declaration,
            ArrayExtent,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub enum ProductInputSelectionTarget {
            #[default]
            Invalid,
            Argument(arena::HandleSpan<ProductArgumentStep>),
            Type {
                reference: TypeReferenceHandle,
                slot: ProductTypeSelectionSlot,
            },
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct ProductInputSelection {
            pub occurrence: Option<AuthoredDeclarationSelectionOccurrenceId>,
            pub argument_ordinal: u32,
            pub target: ProductInputSelectionTarget,
        }

        impl ProductInputSelection {
            pub fn selected_symbol(
                &self,
                program: &TypedTrees,
                arguments: &[StaticMachineArgument],
            ) -> Result<SymbolHandle, &'static str> {
                let symbol = match &self.target {
                    ProductInputSelectionTarget::Invalid => {
                        return Err("static input has no retained operand target");
                    }
                    ProductInputSelectionTarget::Argument(path) => {
                        if !matches!(program.product_input_argument_steps.span_or_empty(*path).first(), Some(ProductArgumentStep::Argument(index)) if *index == self.argument_ordinal)
                        {
                            return Err("Product input selection lost its actual argument ordinal");
                        }
                        let steps = program.product_input_argument_steps.span_or_empty(*path);
                        let mut argument = arguments
                            .get(self.argument_ordinal as usize)
                            .ok_or("static input has a foreign original argument ordinal")?;
                        for step in steps.iter().skip(1) {
                            let application = argument.application.as_ref().ok_or(
                                "Product input selection has no retained source application",
                            )?;
                            argument = match step {
                                ProductArgumentStep::Invalid => {
                                    return Err("static input has an invalid argument path step");
                                }
                                ProductArgumentStep::Argument(index) => {
                                    application.arguments.get(*index as usize).ok_or(
                                        "Product input selection has a foreign argument ordinal",
                                    )?
                                }
                                ProductArgumentStep::Member => program
                                    .expression_table
                                    .static_member(application.member)
                                    .ok_or(
                                        "Product input selection has a foreign member continuation",
                                    )?,
                            };
                        }
                        argument.symbol
                    }
                    ProductInputSelectionTarget::Type { reference, slot } => {
                        let actual = arguments.get(self.argument_ordinal as usize).ok_or(
                            "Product type input selection lost its actual argument ordinal",
                        )?;
                        #[derive(Default)]
                        struct Inputs {
                            types: std::collections::HashSet<
                                TypeReferenceHandle,
                                symbols::BuildSymbolHasher,
                            >,
                            expressions: std::collections::HashSet<
                                ExpressionHandle,
                                symbols::BuildSymbolHasher,
                            >,
                        }
                        impl Extend<SymbolHandle> for Inputs {
                            fn extend<T: IntoIterator<Item = SymbolHandle>>(&mut self, _: T) {}
                        }
                        impl crate::references::TypedReferences for Inputs {
                            fn type_reference(&mut self, reference: TypeReferenceHandle) -> bool {
                                self.types.insert(reference)
                            }
                            fn expression(&mut self, expression: ExpressionHandle) -> bool {
                                self.expressions.insert(expression)
                            }
                            fn proof_fact(
                                &mut self,
                                _: arena::Handle<crate::domain::ProofFact>,
                            ) -> bool {
                                false
                            }
                            fn mathematical_type(
                                &mut self,
                                _: crate::mathematical::MathematicalTypeHandle,
                            ) -> bool {
                                false
                            }
                        }
                        let mut inputs = Inputs::default();
                        crate::references::visit_static_argument(program, &mut inputs, actual);
                        if !inputs.types.contains(reference) {
                            return Err(
                                "Product type selection is not a retained input of its actual argument",
                            );
                        }
                        if !program.contains_type_reference(*reference) {
                            return Err("Product input selection has a foreign type reference");
                        }
                        match (
                            slot,
                            program.type_reference_table.type_reference(*reference),
                        ) {
                            (
                                ProductTypeSelectionSlot::Declaration,
                                TypeReferenceNode::Named { symbol, .. },
                            ) => *symbol,
                            (
                                ProductTypeSelectionSlot::Declaration,
                                TypeReferenceNode::Generic { base_symbol, .. },
                            ) => *base_symbol,
                            (
                                ProductTypeSelectionSlot::Declaration,
                                TypeReferenceNode::DynamicTrait { symbol, .. },
                            ) => *symbol,
                            (
                                ProductTypeSelectionSlot::ArrayExtent,
                                TypeReferenceNode::FixedArray {
                                    length: FixedArrayLength::ConstParameter { symbol, .. },
                                    ..
                                },
                            ) => *symbol,
                            (
                                ProductTypeSelectionSlot::ArrayExtent,
                                TypeReferenceNode::FixedArray {
                                    length: FixedArrayLength::ConstCall { machine, .. },
                                    ..
                                },
                            ) => *machine,
                            _ => {
                                return Err(
                                    "Product input selection no longer names its retained type declaration slot",
                                );
                            }
                        }
                    }
                };
                if !symbol.is_valid() {
                    return Err("Product input selection has no exact checked declaration");
                }
                Ok(program.symbols.authored_origin(symbol))
            }
        }
    }
    pub use input_selections::{
        ProductArgumentStep, ProductInputSelection, ProductInputSelectionTarget,
        ProductTypeSelectionSlot,
    };

    pub(crate) mod applications {
        use crate::{TypedTrees, expression::StaticMachineArgument};
        use arena::Handle;
        use language_semantics::declaration_selection::{
            ProductDeclarationOperand, ProductOperandRole,
            product_operands::ProductStaticEnvironment,
        };
        use std::sync::Arc;
        use symbols::SymbolHandle;

        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
        pub enum ProductApplicationDemand {
            #[default]
            FormationOnly,
            Selected,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct ProductOperandApplication {
            pub operand: ProductDeclarationOperand,
            pub environment: Option<Arc<ProductStaticEnvironment>>,
            pub owner: SymbolHandle,
            pub role: Option<ProductOperandRole>,
            pub declaration: SymbolHandle,
            pub declaration_parameters: arena::HandleSpan<crate::data::TypeParameter>,
            pub declaration_lifetime_count: u32,
            pub demand: ProductApplicationDemand,
            pub argument: StaticMachineArgument,
            pub input_selections: arena::HandleSpan<super::ProductInputSelection>,
            pub scope: crate::trait_definition::RetainedTraitScope,
            pub supplying_application: crate::static_applications::StaticMachineApplicationHandle,
            pub implementation: Option<crate::ClosedConformanceApplication>,
        }

        impl TypedTrees {
            pub fn product_operand_application_identity(
                &self,
                handle: Handle<ProductOperandApplication>,
            ) -> Result<Option<crate::type_identity::NormalizedTypeIdentity>, &'static str>
            {
                let application = self.product_operand_applications.get(handle);
                if !matches!(
                    application.role,
                    Some(
                        ProductOperandRole::BoundarySubject
                            | ProductOperandRole::Provider
                            | ProductOperandRole::Requirement
                    )
                ) {
                    return Err("Product application has no nominal declaration role");
                }
                let parameters = self
                    .data_type_parameters
                    .span(application.declaration_parameters)
                    .ok_or("Product application has a stale declared formal table")?;
                if parameters.is_empty()
                    && application.declaration_lifetime_count == 0
                    && application.role != Some(ProductOperandRole::Requirement)
                {
                    return Ok(None);
                }
                let identity = match application.role {
                    Some(ProductOperandRole::BoundarySubject | ProductOperandRole::Requirement) => {
                        application
                            .scope
                            .scope(self)
                            .and_then(|scope| scope.portable_application_identity(self))
                    }
                    Some(ProductOperandRole::Provider) => self
                        .portable_conformance_application_identity(
                            application
                                .implementation
                                .as_ref()
                                .ok_or("Product implementation lost its exact closure")?,
                            application.supplying_application,
                        ),
                    _ => unreachable!("nominal role checked above"),
                }
                .ok_or("Product application has no complete portable static identity")?;
                Ok(Some(identity))
            }

            pub fn prepared_product_operand(
                &self,
                operand: &ProductDeclarationOperand,
                environment: Option<&ProductStaticEnvironment>,
                owner: SymbolHandle,
                role: ProductOperandRole,
            ) -> Option<Handle<ProductOperandApplication>> {
                self.product_operand_applications
                    .iter()
                    .find_map(|(handle, application)| {
                        (application.operand == *operand
                            && application.environment.as_deref() == environment
                            && application.owner == owner
                            && application.role == Some(role))
                        .then_some(handle)
                    })
            }

            pub fn retain_dependency_product_application(
                &mut self,
                handle: Handle<ProductOperandApplication>,
            ) {
                if !self
                    .retained_product_application_handles
                    .span_or_empty(self.roots.retained_product_applications)
                    .contains(&handle)
                {
                    self.tables
                        .retained_product_application_handles
                        .append_to_span(&mut self.roots.retained_product_applications, handle);
                }
            }

            pub fn retain_product_operand_application(
                &mut self,
                application: ProductOperandApplication,
            ) -> Handle<ProductOperandApplication> {
                if let Some(existing) = self.prepared_product_operand(
                    &application.operand,
                    application.environment.as_deref(),
                    application.owner,
                    application
                        .role
                        .expect("prepared Product operand has a declaration role"),
                ) {
                    return existing;
                }
                self.product_operand_applications.insert(application)
            }
        }
    }
    pub use applications::{ProductApplicationDemand, ProductOperandApplication};

    pub(crate) mod occurrences {
        use crate::{
            TypedTrees,
            expression::{BuildSelectionRequest, ExpressionHandle},
            references::{StaticArgumentCall, TypedReferences},
        };
        use language_semantics::declaration_selection::ProductDeclarationRequest;
        use std::sync::Arc;
        use symbols::SymbolHandle;

        #[derive(Clone)]
        pub struct ProductSelectionOccurrence {
            pub machine: SymbolHandle,
            pub state: SymbolHandle,
            pub expression: ExpressionHandle,
            pub request: Arc<ProductDeclarationRequest>,
        }

        pub fn collect_product_selection_occurrences(
            program: &TypedTrees,
        ) -> Vec<ProductSelectionOccurrence> {
            let mut census = Census {
                machine: SymbolHandle::invalid(),
                state: SymbolHandle::invalid(),
                seen: Vec::new(),
                occurrences: Vec::new(),
            };
            for machine in program.machines() {
                census.machine = machine.symbol;
                for state in program.machine_states(machine) {
                    census.state = state.symbol;
                    census.seen.clear();
                    for statement in program.statement_table.statements(state.statement_nodes) {
                        for (_, root) in program.tables.expression_roots(statement) {
                            crate::references::visit_expression(program, &mut census, root);
                        }
                    }
                }
            }
            census.occurrences
        }

        struct Census {
            machine: SymbolHandle,
            state: SymbolHandle,
            seen: Vec<Vec<ExpressionHandle>>,
            occurrences: Vec<ProductSelectionOccurrence>,
        }
        impl Extend<SymbolHandle> for Census {
            fn extend<Symbols: IntoIterator<Item = SymbolHandle>>(&mut self, _: Symbols) {}
        }
        impl TypedReferences for Census {
            fn expression(&mut self, expression: ExpressionHandle) -> bool {
                let index = expression.arena_index() as usize;
                if self.seen.len() <= index {
                    self.seen.resize_with(index + 1, Vec::new);
                }
                if self.seen[index].contains(&expression) {
                    return false;
                }
                self.seen[index].push(expression);
                true
            }
            fn call_arguments(
                &mut self,
                program: &TypedTrees,
                call: StaticArgumentCall<'_>,
            ) -> bool {
                if let StaticArgumentCall::Expression(expression, call) = call
                    && let Some(BuildSelectionRequest::Declaration(request)) = program
                        .expression_table
                        .build_selection_request(call.build_selection)
                {
                    self.occurrences.push(ProductSelectionOccurrence {
                        machine: self.machine,
                        state: self.state,
                        expression,
                        request: Arc::clone(request),
                    });
                }
                true
            }
        }
    }
    pub use occurrences::{ProductSelectionOccurrence, collect_product_selection_occurrences};

    pub(crate) mod static_environment {
        use crate::static_applications::StaticMachineBindingValue;
        use crate::{TypedTrees, expression as expr, types};
        use arena::{Handle, HandleSpan};
        use language_semantics::declaration_selection::product_operands::*;
        use source::SourceSpan;
        use std::sync::Arc;
        use symbols::SymbolHandle;

        pub fn capture_static_environment(
            program: &TypedTrees,
            machine: SymbolHandle,
        ) -> Result<Option<Arc<ProductStaticEnvironment>>, String> {
            let mut specializations = program
                .machine_specializations
                .iter()
                .filter(|specialization| specialization.instance == machine);
            let Some(specialization) = specializations.next() else {
                return Ok(None);
            };
            if specializations.next().is_some() {
                return Err("Build helper has multiple exact specialization receipts".into());
            }
            if !specialization.binding_application.is_valid() {
                return Err(
                    "Build helper specialization lost its original static binding application"
                        .into(),
                );
            }
            let application =
                program.static_machine_application(specialization.binding_application);
            if application.binding_declaration != specialization.template {
                return Err(
                    "Build helper specialization binding belongs to a different exact template"
                        .into(),
                );
            }
            let owner = program
                .symbols
                .symbol_provenance_source_span(application.lexical_owner)
                .ok_or("Build helper actuals lost their original lexical owner")?;
            let mut capture = Capture {
                program,
                environment: ProductStaticEnvironment {
                    template: program
                        .symbols
                        .symbol_provenance_source_span(specialization.template),
                    lexical_owner: Some(owner),
                    ..Default::default()
                },
                owner,
                types: Vec::new(),
                active_types: Vec::new(),
                expressions: Vec::new(),
                active_expressions: Vec::new(),
            };
            for binding in
                program.static_machine_application_bindings(specialization.binding_application)
            {
                let parameter = program
                    .symbols
                    .symbol_provenance_source_span(binding.parameter)
                    .ok_or("Build helper static binder lost source identity")?;
                let actual = match &binding.value {
                    StaticMachineBindingValue::Type(reference)
                    | StaticMachineBindingValue::Const(reference) => program
                        .static_operand_from_type_reference(*reference)
                        .map_err(str::to_owned)?,
                    StaticMachineBindingValue::Machine(argument)
                    | StaticMachineBindingValue::Conformance(argument) => argument.clone(),
                    StaticMachineBindingValue::RuntimeValue { .. } => continue,
                };
                let argument = capture.operand(&actual)?;
                let declared_type = program
                    .data_type_parameters
                    .span_or_empty(specialization.template_parameters)
                    .iter()
                    .find(|formal| formal.symbol == binding.parameter)
                    .and_then(|formal| match formal.kind {
                        crate::data::TypeParameterKind::Const { type_reference } => {
                            Some(type_reference)
                        }
                        _ => None,
                    })
                    .map(|reference| capture.ty(reference))
                    .transpose()?
                    .unwrap_or_default();
                capture
                    .environment
                    .bindings
                    .insert(ProductStaticEnvironmentBinding {
                        parameter,
                        argument,
                        lexical_owner: owner,
                        declared_type,
                    });
            }
            let template = program
                .machine_by_symbol(specialization.template)
                .ok_or("Build helper lost its source template")?;
            for binding in
                program.static_machine_application_lifetimes(specialization.binding_application)
            {
                let ordinal = template
                    .lifetime_parameters
                    .iter()
                    .position(|parameter| parameter.as_str() == binding.parameter.as_str())
                    .ok_or("Build helper lifetime binding has no exact template position")?;
                let argument = capture.name(binding.argument.as_str(), SymbolHandle::invalid());
                capture
                    .environment
                    .lifetimes
                    .insert(ProductStaticEnvironmentLifetime {
                        parameter_owner: program
                            .symbols
                            .symbol_provenance_source_span(specialization.template)
                            .ok_or("Build helper lifetime binder lost its original declaration")?,
                        parameter_ordinal: ordinal as u32,
                        argument,
                        lexical_owner: owner,
                    });
            }
            let mut identity = b"omega-source-static-environment-v1\0".to_vec();
            let frame = |out: &mut Vec<u8>, value: &[u8]| {
                out.extend_from_slice(&(value.len() as u64).to_le_bytes());
                out.extend_from_slice(value);
            };
            frame(
                &mut identity,
                specialization.normalized_template_identity.as_bytes(),
            );
            for values in [
                &specialization.type_argument_identities,
                &specialization.const_argument_identities,
            ] {
                identity.extend_from_slice(&(values.len() as u64).to_le_bytes());
                for value in values {
                    frame(&mut identity, value.as_bytes());
                }
            }
            for symbols in [
                &specialization.machine_arguments,
                &specialization.conformance_arguments,
            ] {
                identity.extend_from_slice(&(symbols.len() as u64).to_le_bytes());
                for symbol in symbols {
                    frame(
                        &mut identity,
                        program
                            .normalized_hermetic_symbol_identity(*symbol)?
                            .as_bytes(),
                    );
                }
            }
            identity.extend_from_slice(
                &(specialization.conformance_applications.len() as u64).to_le_bytes(),
            );
            for application in &specialization.conformance_applications {
                identity.extend_from_slice(&application.commitment.as_bytes());
            }
            for binding in
                program.static_machine_application_lifetimes(specialization.binding_application)
            {
                let actual = crate::type_identity::portable_applied_lifetime_identity(
                    program,
                    binding.argument.as_str(),
                    application.lexical_owner,
                    &[],
                )
                .and_then(|identity| program.type_identity_text(identity))
                .ok_or("Build helper region actual lost its exact portable scope")?;
                frame(&mut identity, actual.as_bytes());
            }
            capture.environment.identity = identity;
            Ok(Some(Arc::new(capture.environment)))
        }

        pub fn capture_closed_operand(
            program: &TypedTrees,
            argument: &expr::StaticMachineArgument,
            lexical_owner: SymbolHandle,
        ) -> Result<ProductStaticOperandGraph, String> {
            let owner = program
                .symbols
                .symbol_provenance_source_span(lexical_owner)
                .ok_or("closed Product application lost its original lexical scope")?;
            let mut capture = Capture {
                program,
                environment: ProductStaticEnvironment::default(),
                owner,
                types: Vec::new(),
                active_types: Vec::new(),
                expressions: Vec::new(),
                active_expressions: Vec::new(),
            };
            capture.environment.graph.root = capture.operand(argument)?;
            Ok(capture.environment.graph)
        }

        struct Capture<'a> {
            program: &'a TypedTrees,
            environment: ProductStaticEnvironment,
            owner: SourceSpan,
            types: Vec<(types::TypeReferenceHandle, Handle<ProductOperandType>)>,
            active_types: Vec<types::TypeReferenceHandle>,
            expressions: Vec<(expr::ExpressionHandle, Handle<ProductOperandExpression>)>,
            active_expressions: Vec<expr::ExpressionHandle>,
        }
        impl Capture<'_> {
            fn name(&mut self, text: &str, symbol: SymbolHandle) -> Handle<ProductOperandName> {
                let program = self.program;
                let declaration = symbol
                    .is_valid()
                    .then(|| program.symbols.symbol_provenance_source_span(symbol))
                    .flatten();
                self.environment.graph.names.insert(ProductOperandName {
                    authored_selection: Handle::invalid(),
                    text: text.to_owned(),
                    source_span: declaration.unwrap_or(self.owner),
                    declaration,
                    builtin_type: symbol
                        .is_valid()
                        .then(|| program.symbols.builtin_type_atom(symbol))
                        .flatten(),
                    builtin_function: symbol
                        .is_valid()
                        .then(|| program.symbols.builtin_function_for_symbol(symbol))
                        .flatten(),
                })
            }
            fn names(
                &mut self,
                names: &[crate::name::Identifier],
            ) -> HandleSpan<ProductOperandName> {
                let values = names
                    .iter()
                    .map(|name| {
                        let handle = self.name(name.as_str(), SymbolHandle::invalid());
                        self.environment.graph.names.get(handle).clone()
                    })
                    .collect::<Vec<_>>();
                self.environment.graph.names.insert_many(values)
            }
            fn operand(
                &mut self,
                argument: &expr::StaticMachineArgument,
            ) -> Result<Handle<ProductStaticOperand>, String> {
                let program = self.program;
                if !argument.symbol.is_valid()
                    && !argument.type_reference.is_valid()
                    && argument.application.is_none()
                    && argument.const_literal.is_none()
                    && argument.path.len() == 1
                    && let Some(value) =
                        language_semantics::const_value::CanonicalConstValue::from_atom(
                            argument.path[0].as_str(),
                        )
                {
                    return Ok(self
                        .environment
                        .graph
                        .operands
                        .insert(ProductStaticOperand {
                            structural_type: self
                                .environment
                                .graph
                                .types
                                .insert(ProductOperandType::CanonicalConst(value)),
                            ..Default::default()
                        }));
                }
                let names = argument
                    .path
                    .iter()
                    .enumerate()
                    .map(|(index, name)| {
                        let h = self.name(
                            name.as_str(),
                            if index + 1 == argument.path.len() {
                                argument.symbol
                            } else {
                                SymbolHandle::invalid()
                            },
                        );
                        self.environment.graph.names.get(h).clone()
                    })
                    .collect::<Vec<_>>();
                let path = self.environment.graph.names.insert_many(names);
                let structural_type = if argument.type_reference.is_valid() {
                    self.ty(argument.type_reference)?
                } else {
                    Handle::invalid()
                };
                let application = if let Some(app) = &argument.application {
                    let arguments = app
                        .arguments
                        .iter()
                        .map(|argument| self.operand(argument))
                        .collect::<Result<Vec<_>, _>>()?;
                    let arguments = self
                        .environment
                        .graph
                        .static_arguments
                        .insert_many(arguments);
                    let lifetimes = self.names(&app.lifetime_arguments);
                    let member = if app.member.is_valid() {
                        let member = program
                            .expression_table
                            .static_member(app.member)
                            .ok_or("Build static actual has an invalid member continuation")?;
                        self.operand(member)?
                    } else {
                        Handle::invalid()
                    };
                    self.environment
                        .graph
                        .applications
                        .insert(ProductStaticApplication {
                            arguments,
                            lifetimes,
                            member,
                        })
                } else {
                    Handle::invalid()
                };
                let projection = argument.evidence_projection.as_ref().map(|projection| {
                    (
                        self.name(projection.term.as_str(), SymbolHandle::invalid()),
                        self.name(projection.member.as_str(), SymbolHandle::invalid()),
                    )
                });
                Ok(self
                    .environment
                    .graph
                    .operands
                    .insert(ProductStaticOperand {
                        authored_selection: Handle::invalid(),
                        path,
                        structural_type,
                        application,
                        integer: argument.const_literal.clone(),
                        projection,
                        declaration: argument
                            .symbol
                            .is_valid()
                            .then(|| {
                                program
                                    .symbols
                                    .symbol_provenance_source_span(argument.symbol)
                            })
                            .flatten(),
                    }))
            }
            fn type_arguments(
                &mut self,
                arguments: HandleSpan<types::TypeReferenceHandle>,
            ) -> Result<HandleSpan<Handle<ProductOperandType>>, String> {
                let program = self.program;
                let values = program
                    .type_reference_handles(arguments)
                    .iter()
                    .map(|reference| self.ty(*reference))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(self.environment.graph.type_arguments.insert_many(values))
            }
            fn ty(
                &mut self,
                reference: types::TypeReferenceHandle,
            ) -> Result<Handle<ProductOperandType>, String> {
                let program = self.program;
                if program
                    .type_reference_table
                    .type_alias_applications(reference)
                    .next()
                    .is_some()
                {
                    return Err("captured Build helper static type cannot yet retain transparent alias application obligations".into());
                }
                if let Some((_, value)) = self.types.iter().find(|(source, _)| *source == reference)
                {
                    return Ok(*value);
                }
                if self.active_types.contains(&reference) {
                    return Err("Build static actual type contains a cycle".into());
                }
                self.active_types.push(reference);
                let value = match program.type_reference_table.type_reference(reference) {
                    types::TypeReferenceNode::Unit => ProductOperandType::Unit,
                    types::TypeReferenceNode::Named { symbol, name } => {
                        if !symbol.is_valid()
                            && let Some(value) =
                                language_semantics::const_value::CanonicalConstValue::from_atom(
                                    name.as_str(),
                                )
                        {
                            let value = self
                                .environment
                                .graph
                                .types
                                .insert(ProductOperandType::CanonicalConst(value));
                            self.active_types.pop();
                            self.types.push((reference, value));
                            return Ok(value);
                        }
                        if let Some(application) = symbol
                            .is_valid()
                            .then(|| program.data_definition_by_symbol(*symbol))
                            .flatten()
                            .and_then(|definition| definition.generic_instance)
                        {
                            let value = self.ty(application)?;
                            self.active_types.pop();
                            self.types.push((reference, value));
                            return Ok(value);
                        }
                        if let Some(literal) =
                            program.static_const_literal_from_type_reference(reference)
                        {
                            let expression = self.environment.graph.expressions.insert(
                                ProductOperandExpression {
                                    source_span: self.owner,
                                    authored_selections: HandleSpan::empty(),
                                    kind: ProductExpressionKind::Integer(literal),
                                },
                            );
                            ProductOperandType::ConstExpression(expression)
                        } else {
                            ProductOperandType::Named(self.name(name.as_str(), *symbol))
                        }
                    }
                    types::TypeReferenceNode::Generic {
                        base_symbol,
                        base_name,
                        lifetime_arguments,
                        arguments,
                    } => ProductOperandType::Generic {
                        name: self.name(base_name.as_str(), *base_symbol),
                        lifetimes: self.names(lifetime_arguments),
                        arguments: self.type_arguments(*arguments)?,
                    },
                    types::TypeReferenceNode::DynamicTrait {
                        symbol,
                        name,
                        arguments,
                    } => ProductOperandType::Dynamic {
                        name: self.name(name.as_str(), *symbol),
                        arguments: self.type_arguments(*arguments)?,
                    },
                    types::TypeReferenceNode::Reference {
                        referee,
                        access,
                        lifetime,
                    } => ProductOperandType::Reference {
                        referee: self.ty(*referee)?,
                        access: *access,
                        lifetime: lifetime
                            .as_ref()
                            .map(|name| self.name(name.as_str(), SymbolHandle::invalid())),
                    },
                    types::TypeReferenceNode::Slice { element_type } => {
                        ProductOperandType::Slice(self.ty(*element_type)?)
                    }
                    types::TypeReferenceNode::FixedArray {
                        element_type,
                        length,
                    } => ProductOperandType::Array {
                        element: self.ty(*element_type)?,
                        length: match length {
                            types::FixedArrayLength::Literal(value) => {
                                ProductArrayLength::Literal(*value)
                            }
                            types::FixedArrayLength::Expression(value) => {
                                ProductArrayLength::Expression(self.ty(*value)?)
                            }
                            types::FixedArrayLength::ConstParameter { symbol, name } => {
                                ProductArrayLength::Parameter(self.name(name.as_str(), *symbol))
                            }
                            types::FixedArrayLength::ConstCall { machine, name, .. } => {
                                ProductArrayLength::Call(self.name(name.as_str(), *machine))
                            }
                        },
                    },
                    types::TypeReferenceNode::Constrained {
                        base_type,
                        constraints,
                    } => {
                        let base = self.ty(*base_type)?;
                        let constraints = program
                            .type_reference_table
                            .constraints(*constraints)
                            .iter()
                            .map(|constraint| self.constraint(constraint))
                            .collect::<Result<Vec<_>, _>>()?;
                        ProductOperandType::Constrained {
                            base,
                            constraints: self
                                .environment
                                .graph
                                .constraints
                                .insert_many(constraints),
                        }
                    }
                    types::TypeReferenceNode::ConstExpression(expression) => {
                        ProductOperandType::ConstExpression(self.expression(*expression)?)
                    }
                };
                let value = self.environment.graph.types.insert(value);
                self.active_types.pop();
                self.types.push((reference, value));
                Ok(value)
            }
            fn constraint(
                &mut self,
                value: &types::TypeConstraintNode,
            ) -> Result<ProductOperandConstraint, String> {
                Ok(match value {
                    types::TypeConstraintNode::Named(name) => ProductOperandConstraint::Named(
                        self.name(name.as_str(), SymbolHandle::invalid()),
                    ),
                    types::TypeConstraintNode::ArithmeticDomain(policy) => {
                        ProductOperandConstraint::Arithmetic(*policy)
                    }

                    types::TypeConstraintNode::Domain(domain) => {
                        let args = domain
                            .arguments
                            .iter()
                            .map(|reference| self.ty(*reference))
                            .collect::<Result<Vec<_>, _>>()?;
                        let arguments = self.environment.graph.type_arguments.insert_many(args);
                        ProductOperandConstraint::Domain {
                            name: self.name(domain.name.as_str(), domain.symbol),
                            arguments,
                        }
                    }
                })
            }
            fn expression_selections(
                &mut self,
                expression: expr::ExpressionHandle,
            ) -> Result<HandleSpan<ProductOperandSelection>, String> {
                let program = self.program;
                let ledger = program.authored_declaration_selections();
                let mut selections = Vec::new();
                for occurrence in program
                    .expression_table
                    .authored_selection_occurrences(expression)
                {
                    let selection = ledger
                        .get(occurrence)
                        .ok_or("Build static actual retains an unknown authored selection")?;
                    let rejoined = ledger
                        .source_coordinate_occurrence(
                            selection.source_span(),
                            selection.exposure(),
                            selection.kind(),
                        )
                        .map_err(|reason| reason.to_string())?;
                    if rejoined != occurrence {
                        return Err(
                            "Build static actual changed its exact source selection association"
                                .into(),
                        );
                    }
                    selections.push(ProductOperandSelection {
                        source_span: selection.source_span(),
                        exposure: selection.exposure(),
                        kind: Some(selection.kind()),
                    });
                }
                Ok(self.environment.graph.selections.insert_many(selections))
            }

            fn expression(
                &mut self,
                expression: expr::ExpressionHandle,
            ) -> Result<Handle<ProductOperandExpression>, String> {
                let program = self.program;
                if let Some((_, value)) = self
                    .expressions
                    .iter()
                    .find(|(source, _)| *source == expression)
                {
                    return Ok(*value);
                }
                if self.active_expressions.contains(&expression) {
                    return Err("Build static actual expression contains a cycle".into());
                }
                self.active_expressions.push(expression);
                let kind = match program.expression_table.expression(expression) {
                    expr::ExpressionNode::Integer(literal) => {
                        ProductExpressionKind::Integer(literal.clone())
                    }
                    expr::ExpressionNode::Boolean(value) => ProductExpressionKind::Boolean(*value),
                    expr::ExpressionNode::Float(value) => {
                        ProductExpressionKind::Float(value.clone())
                    }
                    expr::ExpressionNode::String(value) => {
                        ProductExpressionKind::Text(value.to_vec())
                    }
                    expr::ExpressionNode::Name(path) => {
                        let members = program.expression_table.name_path_members(path.members);
                        let values = members
                            .iter()
                            .enumerate()
                            .map(|(index, name)| {
                                let h = self.name(
                                    name.as_str(),
                                    if index + 1 == members.len() {
                                        path.symbol
                                    } else {
                                        SymbolHandle::invalid()
                                    },
                                );
                                self.environment.graph.names.get(h).clone()
                            })
                            .collect::<Vec<_>>();
                        ProductExpressionKind::Name(
                            self.environment.graph.names.insert_many(values),
                        )
                    }
                    expr::ExpressionNode::StructLiteral(literal) => {
                        let name = self.name(literal.type_name.as_str(), literal.type_symbol);
                        let case = literal.case_name.as_ref().map(|name| {
                            self.name(name.as_str(), literal.case_symbol.unwrap_or_default())
                        });
                        let fields = program
                            .expression_table
                            .struct_fields(literal.fields)
                            .iter()
                            .map(|field| {
                                Ok(ProductOperandField {
                                    name: self.name(field.name.as_str(), field.field_symbol),
                                    value: self.expression(field.value)?,
                                })
                            })
                            .collect::<Result<Vec<_>, String>>()?;
                        ProductExpressionKind::Record {
                            name,
                            case,
                            fields: self.environment.graph.fields.insert_many(fields),
                        }
                    }
                    expr::ExpressionNode::ArrayLiteral(elements) => {
                        let values = program
                            .expression_table
                            .expression_handles(*elements)
                            .iter()
                            .map(|value| self.expression(*value))
                            .collect::<Result<Vec<_>, _>>()?;
                        ProductExpressionKind::Array(
                            self.environment
                                .graph
                                .expression_arguments
                                .insert_many(values),
                        )
                    }
                    expr::ExpressionNode::UndeterminedNaN(format) => {
                        ProductExpressionKind::UndeterminedNaN(*format)
                    }
                    expr::ExpressionNode::ZeroValue(reference) => {
                        ProductExpressionKind::ZeroValue(self.ty(*reference)?)
                    }
                    expr::ExpressionNode::TypeExpression(reference) => {
                        ProductExpressionKind::TypeExpression(self.ty(*reference)?)
                    }
                    expr::ExpressionNode::Unary(unary) => ProductExpressionKind::Unary {
                        operator: match unary.operator {
                            expr::UnaryOperator::BitwiseNot => ProductUnaryOperator::BitwiseNot,
                            expr::UnaryOperator::LogicalNot => ProductUnaryOperator::LogicalNot,
                        },
                        operand: self.expression(unary.operand)?,
                    },
                    expr::ExpressionNode::Binary(binary) => ProductExpressionKind::Binary {
                        left: self.expression(binary.left)?,
                        right: self.expression(binary.right)?,
                        operator: match binary.operator {
                            expr::BinaryOperator::CaseMembership => {
                                ProductBinaryOperator::CaseMembership
                            }
                            expr::BinaryOperator::Add => ProductBinaryOperator::Add,
                            expr::BinaryOperator::And => ProductBinaryOperator::And,
                            expr::BinaryOperator::BitwiseAnd => ProductBinaryOperator::BitwiseAnd,
                            expr::BinaryOperator::BitwiseOr => ProductBinaryOperator::BitwiseOr,
                            expr::BinaryOperator::BitwiseXor => ProductBinaryOperator::BitwiseXor,
                            expr::BinaryOperator::Divide => ProductBinaryOperator::Divide,
                            expr::BinaryOperator::Equal => ProductBinaryOperator::Equal,
                            expr::BinaryOperator::Greater => ProductBinaryOperator::Greater,
                            expr::BinaryOperator::GreaterOrEqual => {
                                ProductBinaryOperator::GreaterOrEqual
                            }
                            expr::BinaryOperator::Less => ProductBinaryOperator::Less,
                            expr::BinaryOperator::LessOrEqual => ProductBinaryOperator::LessOrEqual,
                            expr::BinaryOperator::Modulo => ProductBinaryOperator::Modulo,
                            expr::BinaryOperator::Multiply => ProductBinaryOperator::Multiply,
                            expr::BinaryOperator::NotEqual => ProductBinaryOperator::NotEqual,
                            expr::BinaryOperator::Or => ProductBinaryOperator::Or,
                            expr::BinaryOperator::ShiftLeft => ProductBinaryOperator::ShiftLeft,
                            expr::BinaryOperator::ShiftRight => ProductBinaryOperator::ShiftRight,
                            expr::BinaryOperator::Subtract => ProductBinaryOperator::Subtract,
                        },
                    },
                    expr::ExpressionNode::Member(member) => ProductExpressionKind::Member {
                        receiver: self.expression(member.receiver)?,
                        member: self.name(member.member.as_str(), member.selection.field_symbol()),
                    },
                    expr::ExpressionNode::Indexed(indexed) => ProductExpressionKind::Indexed {
                        receiver: self.expression(indexed.collection)?,
                        index: self.expression(indexed.index)?,
                    },
                    expr::ExpressionNode::Cast(cast) => {
                        let arguments = program
                            .type_reference_handles(cast.semantic_domain_arguments)
                            .iter()
                            .map(|reference| self.ty(*reference))
                            .collect::<Result<Vec<_>, _>>()?;
                        let domain_arguments =
                            self.environment.graph.type_arguments.insert_many(arguments);
                        let members = program
                            .expression_table
                            .name_path_members(cast.semantic_domain);
                        let domain = self.names(members);
                        ProductExpressionKind::Cast {
                            value: self.expression(cast.value)?,
                            target: self.ty(cast.target_type)?,
                            policy: cast.domain,
                            domain,
                            domain_arguments,
                            form: cast.form,
                        }
                    }
                    expr::ExpressionNode::Call(call) => {
                        if call.native_code_layout.is_some() {
                            return Err("an executed static layout actual must retain its materialized value before Product capture".into());
                        }
                        let receiver = if call.receiver.is_valid() {
                            self.expression(call.receiver)?
                        } else {
                            Handle::invalid()
                        };
                        let target = self.name(call.target.as_str(), call.target_symbol);
                        if let Some(occurrence) = call.authored_call_selection {
                            let selection = program
                                .authored_declaration_selections()
                                .get(occurrence)
                                .ok_or("Build static call lost its authored Call occurrence")?;
                            if selection.kind()
                        != language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::Call
                        || !program
                            .expression_table
                            .authored_selection_occurrences(expression)
                            .any(|attached| attached == occurrence)
                    {
                        return Err("Build static call changed its source-owned Call association".into());
                    }
                            self.environment.graph.names.get_mut(target).source_span =
                                selection.source_span();
                        }
                        let owner = if call.requirement_application.is_valid() {
                            self.ty(call.requirement_application)?
                        } else {
                            Handle::invalid()
                        };
                        let static_arguments = call
                            .machine_arguments
                            .iter()
                            .map(|argument| self.operand(argument))
                            .collect::<Result<Vec<_>, _>>()?;
                        let static_arguments = self
                            .environment
                            .graph
                            .static_arguments
                            .insert_many(static_arguments);
                        let arguments = program
                            .expression_table
                            .expression_handles(call.arguments)
                            .iter()
                            .map(|expression| self.expression(*expression))
                            .collect::<Result<Vec<_>, _>>()?;
                        let arguments = self
                            .environment
                            .graph
                            .expression_arguments
                            .insert_many(arguments);
                        let evidence = self.names(&call.evidence_arguments);
                        ProductExpressionKind::Call {
                            receiver,
                            target,
                            owner,
                            static_arguments,
                            arguments,
                            evidence,
                            acknowledgement: call.operational_acknowledgement,
                        }
                    }
                    _ => {
                        return Err(
                    "Build specialization has a non-static or unmaterialized selected value".into(),
                );
                    }
                };
                let authored_selections = self.expression_selections(expression)?;
                let value = self
                    .environment
                    .graph
                    .expressions
                    .insert(ProductOperandExpression {
                        source_span: program.expression_table.source_span(expression),
                        authored_selections,
                        kind,
                    });
                self.active_expressions.pop();
                self.expressions.push((expression, value));
                Ok(value)
            }
        }
    }
    pub use static_environment::{capture_closed_operand, capture_static_environment};

    use crate::TypedTrees;
    use crate::expression::StaticMachineArgument;
    use symbols::{JoinedPath, SymbolHandle};

    pub use language_semantics::declaration_selection::ProductOperandRole as ProductOperandKind;

    /*
        A Build selection operand names a declaration by a bare path, such as
        `NativeConsole`. It is never an applied type, a conformance application,
        a value or an evidence projection. An authored type operand has no path,
        so it never qualifies.

        A type reference beside a path is not authored syntax. It is the record
        `retain_static_argument_contexts` keeps of an implementation operand's
        source scope. Reading that record as an authored type refused
        `select_provider<Console, NativeConsole>` and every program that selects
        a named provider.

        The record may accompany a bare path only when it names that same
        declaration with no arguments. Any other type reference still fails, so
        a record that drifted from its path cannot pass as bare.
    */
    pub fn is_declaration_operand(typed: &TypedTrees, argument: &StaticMachineArgument) -> bool {
        argument.application.is_none()
            && argument.const_literal.is_none()
            && argument.evidence_projection.is_none()
            && !argument.path.is_empty()
            && (!argument.type_reference.is_valid()
                || retained_source_names_same_declaration(typed, argument))
    }

    fn retained_source_names_same_declaration(
        typed: &TypedTrees,
        argument: &StaticMachineArgument,
    ) -> bool {
        argument.symbol.is_valid()
            && typed
                .type_reference_table
                .contains_type_reference(argument.type_reference)
            && matches!(
                typed.type_reference_table.type_reference(argument.type_reference),
                crate::types::TypeReferenceNode::Named { symbol, .. } if *symbol == argument.symbol
            )
    }

    pub fn resolve_product_operand(
        typed: &TypedTrees,
        argument: &StaticMachineArgument,
        occurrence: source::SourceSpan,
        kind: ProductOperandKind,
    ) -> Option<symbols::SymbolHandle> {
        if !is_declaration_operand(typed, argument) {
            return None;
        }
        resolve_product_path(typed, &argument.path.joined_path(), occurrence, kind)
    }

    pub fn resolve_product_path(
        typed: &TypedTrees,
        path: &str,
        occurrence: source::SourceSpan,
        kind: ProductOperandKind,
    ) -> Option<symbols::SymbolHandle> {
        let program = typed.product_declarations();
        let candidates = product_candidates(program, kind)?;
        let token_bound_machines = if kind == ProductOperandKind::BoundarySubject {
            program
                .machine_token_bindings()
                .iter()
                .filter(|binding| binding.is_boundary)
                .map(|binding| binding.symbol)
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        program.symbols.find_build_subject_declaration_from_source(
            path,
            occurrence,
            candidates,
            &token_bound_machines,
        )
    }

    fn product_candidates(
        program: &TypedTrees,
        kind: ProductOperandKind,
    ) -> Option<Vec<(SymbolHandle, bool)>> {
        let candidates = match kind {
            ProductOperandKind::WireCompatibilityFact => return None,
            ProductOperandKind::BoundarySubject => program
                .traits()
                .iter()
                .filter(|definition| definition.is_boundary)
                .map(|definition| (definition.symbol, definition.is_public))
                .chain(
                    program
                        .machines()
                        .iter()
                        .filter(|machine| {
                            machine.supply_mode
                                == language_semantics::MachineSupplyMode::TopLevelRequirement
                        })
                        .map(|machine| (machine.symbol, machine.is_public)),
                )
                .chain(
                    program
                        .all_operators()
                        .filter(|operator| operator.is_boundary)
                        .map(|operator| (operator.symbol, operator.is_public)),
                )
                .collect::<Vec<_>>(),
            ProductOperandKind::Provider => program
                .data_definitions()
                .iter()
                .map(|definition| (definition.symbol, definition.is_public))
                .chain(
                    program
                        .conformances()
                        .iter()
                        .map(|definition| (definition.symbol, definition.is_public)),
                )
                .collect(),
            ProductOperandKind::Requirement => program
                .traits()
                .iter()
                .filter(|definition| definition.is_boundary)
                .flat_map(|definition| {
                    program
                        .trait_machine_signatures(definition)
                        .iter()
                        .map(move |requirement| (requirement.symbol, definition.is_public))
                })
                .collect(),
            ProductOperandKind::TestRequirement => program
                .traits()
                .iter()
                .filter(|definition| !definition.is_boundary)
                .flat_map(|definition| {
                    program
                        .trait_machine_signatures(definition)
                        .iter()
                        .map(move |requirement| (requirement.symbol, definition.is_public))
                })
                .collect(),
            ProductOperandKind::Data => program
                .data_definitions()
                .iter()
                .map(|definition| (definition.symbol, definition.is_public))
                .collect(),
            ProductOperandKind::Representation => program
                .conformances()
                .iter()
                .map(|definition| (definition.symbol, definition.is_public))
                .collect(),
        };
        Some(candidates)
    }

    /// Resolve the source-owned semantic arrival signature without reading a
    /// body or constructing display/type-identity copies. A matching spelling
    /// outside the product's exact core vocabulary is never this declaration.
    pub fn core_arrival_requirement<'program>(
        program: &'program TypedTrees,
        requirement: &str,
    ) -> Result<
        (
            &'program crate::trait_definition::TraitDefinition,
            &'program crate::signature::StateSignature,
        ),
        String,
    > {
        let (owner, method) = requirement
            .split_once("::")
            .ok_or_else(|| "target semantic arrival has no requirement owner".to_owned())?;
        let mut declarations = program.traits().iter().filter(|definition| {
            definition.name.as_str() == owner
                && definition.is_boundary
                && program.symbols.core_vocabulary(definition.symbol)
                    == Some(language_core::core_vocabulary::CoreVocabulary::ProgramStorageEntry)
                && program
                    .symbols
                    .symbol_source_span(definition.symbol)
                    .and_then(|span| program.symbols.source_file(span))
                    .is_some_and(|file| file.dependency_scope == source::DependencyScope::Product)
        });
        let definition = declarations.next().ok_or_else(|| {
            format!("target semantic arrival `{requirement}` has no exact core declaration")
        })?;
        if declarations.next().is_some() {
            return Err(format!(
                "target semantic arrival `{requirement}` has multiple core declaration instances"
            ));
        }
        let mut signatures = program
            .trait_machine_signatures(definition)
            .iter()
            .filter(|signature| signature.name.as_str() == method);
        let signature = signatures.next().ok_or_else(|| {
            format!("target semantic arrival `{requirement}` has no exact machine declaration")
        })?;
        if signatures.next().is_some() {
            return Err(format!(
                "target semantic arrival `{requirement}` has multiple machine declarations"
            ));
        }
        Ok((definition, signature))
    }

    #[derive(Clone, Copy)]
    pub struct ProductEntryDeclaration<'program> {
        machine_symbol: SymbolHandle,
        machine_contracts: &'program [crate::signature::SignatureContract],
        entry_contracts: &'program [crate::signature::SignatureContract],
        pub name: &'program str,
        pub attached_data_symbol: SymbolHandle,
        pub type_parameters: &'program [crate::data::TypeParameter],
        pub entry: Option<ProductEntrySignature<'program>>,
    }

    #[derive(Clone, Copy)]
    pub struct ProductEntrySignature<'program> {
        pub parameters: &'program [crate::signature::StateParameter],
        pub return_type: crate::types::TypeReferenceHandle,
    }

    impl<'program> ProductEntryDeclaration<'program> {
        pub fn from_declared_owner(
            program: &'program TypedTrees,
            symbol: SymbolHandle,
        ) -> Option<Self> {
            let machine = program.machine(symbol)?;
            let contracts = program.machine_contracts(machine);
            if contracts.len() != machine.contracts.len()
                || program.machine_states(machine).len() != machine.states.len()
            {
                return None;
            }
            let entry_contracts = program
                .machine_states(machine)
                .first()
                .map_or(&[][..], |entry| program.state_contracts(entry));
            if program
                .machine_states(machine)
                .first()
                .is_some_and(|entry| entry_contracts.len() != entry.contracts.len())
            {
                return None;
            }
            if contracts.iter().chain(entry_contracts).any(|contract| {
                program.proof_facts.span_or_empty(contract.facts).len() != contract.facts.len()
            }) {
                return None;
            }
            Some(Self::from_machine(program, machine))
        }

        pub const fn machine_symbol(&self) -> SymbolHandle {
            self.machine_symbol
        }

        pub fn erased_requires_inputs(self) -> impl Iterator<Item = &'program str> + 'program {
            self.machine_contracts
                .iter()
                .chain(self.entry_contracts)
                .filter_map(|contract| {
                    (contract.kind == crate::signature::SignatureContractKind::Requires)
                        .then_some(contract.binding.as_ref())
                        .flatten()
                        .map(|binding| binding.as_str())
                })
        }

        pub fn from_machine(
            program: &'program TypedTrees,
            machine: &'program crate::machine::Machine,
        ) -> Self {
            Self {
                machine_symbol: machine.symbol,
                machine_contracts: program.machine_contracts(machine),
                entry_contracts: program
                    .machine_states(machine)
                    .first()
                    .map_or(&[][..], |entry| program.state_contracts(entry)),
                name: machine.name.as_str(),
                attached_data_symbol: machine.attached_data_symbol,
                type_parameters: program.machine_type_parameters(machine),
                entry: program
                    .machine_states(machine)
                    .first()
                    .map(|entry| ProductEntrySignature {
                        parameters: program.state_parameters(entry),
                        return_type: entry.return_type,
                    }),
            }
        }
    }

    pub fn resolve_build_request_author(
        typed: &TypedTrees,
        author: source::SourceSpan,
    ) -> Result<SymbolHandle, String> {
        let author = typed.symbols.rejoin_product_occurrence(author);
        let mut matching = typed
            .machines()
            .iter()
            .filter(|machine| typed.symbols.symbol_source_span(machine.symbol) == Some(author));
        let machine = matching
            .next()
            .ok_or("executed Build request has no authored selecting machine")?;
        if matching.next().is_some() {
            return Err("executed Build request has ambiguous authored machine custody".into());
        }
        Ok(machine.symbol)
    }

    pub fn native_binding_formation(
        typed: &TypedTrees,
        author: SymbolHandle,
        root: Option<SymbolHandle>,
        request: std::sync::Arc<
            language_semantics::declaration_selection::ProductDeclarationRequest,
        >,
    ) -> Result<language_semantics::declaration_selection::NativeBindingFormation, String> {
        use sha2::{Digest, Sha256};
        let machine = typed
            .machine_by_symbol(author)
            .ok_or("native source author has no exact checked machine")?;
        let span = typed
            .symbols
            .symbol_provenance_source_span(author)
            .ok_or("native source author lost its source")?;
        let source = typed
            .symbols
            .source_file(span)
            .ok_or("native declaration has no retained source file")?;
        let callable_identity = typed
            .machine_callable_identity_text(machine)
            .ok_or("native declaration has no checked callable identity")?;
        let [operand] = request.operands.as_ref() else {
            return Err("native formation lost its exact requirement".into());
        };
        let relative = source
            .path
            .strip_prefix(&source.package_root)
            .map_err(|_| "native source is outside its checked package")?
            .to_string_lossy();
        let mut digest = Sha256::new();
        digest.update(b"omega.native-source-occurrence.v1\0");
        for bytes in [
            relative.as_bytes(),
            source.source.as_bytes(),
            callable_identity.as_bytes(),
            operand.path.as_bytes(),
        ] {
            digest.update((bytes.len() as u64).to_le_bytes());
            digest.update(bytes);
        }
        digest.update((request.source_span.span.start as u64).to_le_bytes());
        digest.update((request.source_span.span.end as u64).to_le_bytes());
        let root_scope = root
            .and_then(|root| typed.symbols.symbol_provenance_source_span(root))
            .is_some_and(|root| typed.symbols.same_source_package(span, root));
        Ok(
            language_semantics::declaration_selection::NativeBindingFormation {
                request,
                author: span,
                author_package: typed.symbols.symbol_package_identity(author),
                root_scope,
                declaration_path: typed.symbols.display_path(author, "::"),
                callable_identity,
                source_occurrence_digest: digest.finalize().into(),
                resolution_stratum: source.resolution_stratum,
            },
        )
    }

    impl TypedTrees {
        pub fn same_product_operand_source(
            &self,
            left: &language_semantics::declaration_selection::ProductDeclarationOperand,
            right: &language_semantics::declaration_selection::ProductDeclarationOperand,
        ) -> bool {
            if left.path != right.path
                || self.symbols.rejoin_product_occurrence(left.source_span)
                    != self.symbols.rejoin_product_occurrence(right.source_span)
            {
                return false;
            }
            match (&left.application, &right.application) {
                (None, None) => true,
                (Some(left), Some(right)) => {
                    if std::sync::Arc::ptr_eq(left, right) {
                        return true;
                    }
                    let mut left = left.as_ref().clone();
                    let mut right = right.as_ref().clone();
                    left.remap_source_spans(|span| self.symbols.rejoin_product_occurrence(span));
                    right.remap_source_spans(|span| self.symbols.rejoin_product_occurrence(span));
                    left == right
                }
                _ => false,
            }
        }

        pub fn same_product_request_source(
            &self,
            left: &language_semantics::declaration_selection::ProductDeclarationRequest,
            right: &language_semantics::declaration_selection::ProductDeclarationRequest,
        ) -> bool {
            left.operation == right.operation
                && self.symbols.rejoin_product_occurrence(left.source_span)
                    == self.symbols.rejoin_product_occurrence(right.source_span)
                && left.operands.len() == right.operands.len()
                && left
                    .operands
                    .iter()
                    .zip(right.operands.iter())
                    .all(|(left, right)| self.same_product_operand_source(left, right))
        }
    }
}

pub mod trait_definition {
    /// An exact source requirement binding. Nomination supplies no proof or
    /// execution authority; the compilation must judge the realization against
    /// the original requirement before using it.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Supply {
        pub slot: symbols::SymbolHandle,
        pub provider: symbols::SymbolHandle,
    }

    impl Supply {
        /// Resolve only within the actual provider's nominal conformance rows.
        pub fn realization<'a>(
            &self,
            typed: &'a crate::TypedTrees,
            requirement: symbols::SymbolHandle,
        ) -> Result<&'a crate::machine::Machine, &'static str> {
            let mut realizing = typed.machines().iter().filter(|machine| {
                crate::provider_declarations::satisfaction_provider_declaration(typed, machine)
                    == Some(self.provider)
                    && (typed
                        .machine_trait_conformances(machine)
                        .iter()
                        .any(|clause| clause.requirement_symbol == requirement)
                        || typed
                            .owned_conformance_realization(machine.symbol)
                            .is_some_and(|(_, row)| row.requirement == requirement))
            });
            match (realizing.next(), realizing.next()) {
                (Some(machine), None) => Ok(machine),
                _ => Err("provider does not give the supplied requirement exactly one machine"),
            }
        }
    }

    pub(crate) mod applications {
        use super::TraitDefinition;
        use crate::TypedTrees;
        use crate::name::Identifier;
        use crate::type_identity::{
            NormalizedTypeIdentity, TypeIdentityQualification, TypeIdentityRequest,
        };
        use crate::types::{TypeReferenceHandle, TypeReferenceNode};
        use symbols::SymbolHandle;

        pub(crate) mod regions {
            use super::AppliedTraitScope;
            use crate::TypedTrees;
            use crate::data::TypeParameterKind;
            use crate::name::Identifier;
            use crate::type_identity::lifetime_parameter_identity;
            use crate::type_identity::{
                AppliedTypeIdentityScope, IndexIdentityArgument, NormalizedTypeIdentity,
                applied_index_identity, applied_lifetime_identity, applied_type_identity,
                local_trait_application_identity, portable_applied_index_identity,
                portable_applied_lifetime_identity, portable_applied_type_identity,
            };
            use symbols::SymbolHandle;

            #[derive(Debug, Clone, PartialEq, Eq)]
            pub(crate) struct AppliedDeclarationRegionScope {
                pub(crate) declaration: SymbolHandle,
                pub(crate) lifetimes: Vec<(String, String)>,
            }

            impl AppliedDeclarationRegionScope {
                pub(crate) fn new(
                    declaration: SymbolHandle,
                    parameters: &[Identifier],
                    arguments: &[Identifier],
                ) -> Self {
                    Self {
                        declaration,
                        lifetimes: parameters
                            .iter()
                            .zip(arguments)
                            .map(|(parameter, argument)| {
                                (parameter.as_str().to_owned(), argument.as_str().to_owned())
                            })
                            .collect(),
                    }
                }
            }

            impl AppliedTraitScope<'_> {
                pub fn under_invocation(
                    mut self,
                    program: &TypedTrees,
                    declaration: SymbolHandle,
                    argument_owner: SymbolHandle,
                    arguments: &[(SymbolHandle, crate::types::TypeReferenceHandle)],
                    bindings: &[crate::static_applications::StaticMachineLifetimeBinding],
                ) -> Option<Self> {
                    let enclosing = program.enclosing_invocation_frames(argument_owner).ok()?;
                    let region =
                        invocation_region_scope(program, declaration, argument_owner, bindings)?;
                    self.argument_owner = argument_owner;
                    self.substitutions.splice(0..0, arguments.iter().copied());
                    for start in &mut self.substitution_scope_starts {
                        *start += arguments.len();
                    }
                    self.substitution_scope_starts.insert(0, 0);
                    self.region_scopes.insert(0, region);
                    for frame in enclosing {
                        // Untyped bindings remain unresolved. They cannot close a
                        // demanded trait operand, but unrelated traits still form.
                        let arguments = frame
                            .bindings
                            .iter()
                            .filter_map(|binding| {
                                use crate::static_applications::StaticMachineBindingValue;
                                let reference = match &binding.value {
                                    StaticMachineBindingValue::Type(reference)
                                    | StaticMachineBindingValue::Const(reference) => *reference,
                                    StaticMachineBindingValue::Machine(argument)
                                    | StaticMachineBindingValue::Conformance(argument) => {
                                        argument.type_reference
                                    }
                                    StaticMachineBindingValue::RuntimeValue { .. } => return None,
                                };
                                reference
                                    .is_valid()
                                    .then_some((binding.parameter, reference))
                            })
                            .collect::<Vec<_>>();
                        let region = invocation_region_scope(
                            program,
                            frame.declaration,
                            frame.argument_owner,
                            &frame.lifetimes,
                        )?;
                        self.argument_owner = frame.argument_owner;
                        self.substitutions.splice(0..0, arguments.iter().copied());
                        for start in &mut self.substitution_scope_starts {
                            *start += arguments.len();
                        }
                        self.substitution_scope_starts.insert(0, 0);
                        self.region_scopes.insert(0, region);
                    }
                    Some(self)
                }

                /// Whether a member of this trait keeps the signature it was
                /// declared with, so no applied signature exists for it.
                ///
                /// `under_invocation` puts the invoking machine's own bindings
                /// in front. A trait member cannot name another machine's
                /// parameters, so those frames leave its signature alone; only
                /// the frames after them, which apply the trait, can change it.
                pub fn keeps_declared_signatures(&self, program: &TypedTrees) -> bool {
                    let invocation_frames = self
                        .region_scopes
                        .iter()
                        .take_while(|frame| {
                            frame.declaration.is_valid()
                                && program.symbols.get(frame.declaration).kind
                                    == symbols::SymbolKind::Machine
                        })
                        .count();
                    self.declaration.lifetime_parameters.is_empty()
                        && self
                            .substitution_scope_starts
                            .get(invocation_frames)
                            .is_some_and(|start| *start == self.substitutions.len())
                }

                pub fn portable_application_identity(
                    &self,
                    program: &TypedTrees,
                ) -> Option<NormalizedTypeIdentity> {
                    self.with_identity_scopes(program, |scopes| {
                        self.portable_application_identity_in_scopes(
                            program,
                            self.argument_owner,
                            scopes,
                            0,
                        )
                    })
                }

                pub fn portable_application_identity_under_invocation(
                    &self,
                    program: &TypedTrees,
                    declaration: SymbolHandle,
                    argument_owner: SymbolHandle,
                    arguments: &[(SymbolHandle, IndexIdentityArgument<'_>)],
                    bindings: &[crate::static_applications::StaticMachineLifetimeBinding],
                ) -> Option<NormalizedTypeIdentity> {
                    let region =
                        invocation_region_scope(program, declaration, argument_owner, bindings)?;
                    self.with_identity_scopes(program, |scopes| {
                        let mut complete = Vec::with_capacity(scopes.len() + 1);
                        complete.push(AppliedTypeIdentityScope {
                            declaration: region.declaration,
                            arguments,
                            lifetimes: &region.lifetimes,
                        });
                        complete.extend_from_slice(scopes);
                        self.portable_application_identity_in_scopes(
                            program,
                            argument_owner,
                            &complete,
                            1,
                        )
                    })
                }

                fn portable_application_identity_in_scopes(
                    &self,
                    program: &TypedTrees,
                    argument_owner: SymbolHandle,
                    scopes: &[AppliedTypeIdentityScope<'_>],
                    preceding: usize,
                ) -> Option<NormalizedTypeIdentity> {
                    let arguments = program
                        .trait_type_parameters(self.declaration)
                        .iter()
                        .map(|parameter| {
                            let (reference, outer) =
                                self.parameter_application(parameter.symbol)?;
                            let supplying = &scopes[..outer + preceding];
                            if parameter.kind.is_const_or_value()
                                || matches!(parameter.kind, TypeParameterKind::Machine { .. })
                            {
                                portable_applied_index_identity(
                                    program,
                                    IndexIdentityArgument::TypeReference(reference),
                                    argument_owner,
                                    supplying,
                                )
                            } else {
                                portable_applied_type_identity(
                                    program,
                                    reference,
                                    argument_owner,
                                    supplying,
                                )
                            }
                        })
                        .collect::<Option<Vec<_>>>()?;
                    let lifetimes = self
                        .declaration
                        .lifetime_parameters
                        .iter()
                        .map(|parameter| {
                            portable_applied_lifetime_identity(
                                program,
                                parameter.as_str(),
                                argument_owner,
                                scopes,
                            )
                        })
                        .collect::<Option<Vec<_>>>()?;
                    crate::type_identity::portable_trait_application_identity(
                        program,
                        self.declaration.symbol,
                        &arguments,
                        &lifetimes,
                    )
                }

                pub fn portable_parameter_identity(
                    &self,
                    program: &TypedTrees,
                    parameter: SymbolHandle,
                ) -> Option<NormalizedTypeIdentity> {
                    let (reference, outer) = self.parameter_application(parameter)?;
                    self.with_identity_scopes(program, |scopes| {
                        let binder = program
                            .trait_type_parameters(self.declaration)
                            .iter()
                            .find(|binder| binder.symbol == parameter)?;
                        if matches!(
                            binder.kind,
                            TypeParameterKind::Const { .. }
                                | TypeParameterKind::Value { .. }
                                | TypeParameterKind::Machine { .. }
                        ) {
                            portable_applied_index_identity(
                                program,
                                IndexIdentityArgument::TypeReference(reference),
                                self.argument_owner,
                                &scopes[..outer],
                            )
                        } else {
                            portable_applied_type_identity(
                                program,
                                reference,
                                self.argument_owner,
                                &scopes[..outer],
                            )
                        }
                    })
                }

                pub fn portable_lifetime_arguments(
                    &self,
                    program: &TypedTrees,
                ) -> Option<Vec<NormalizedTypeIdentity>> {
                    self.with_identity_scopes(program, |scopes| {
                        self.declaration
                            .lifetime_parameters
                            .iter()
                            .map(|parameter| {
                                portable_applied_lifetime_identity(
                                    program,
                                    parameter.as_str(),
                                    self.argument_owner,
                                    scopes,
                                )
                            })
                            .collect()
                    })
                }

                pub fn parameter_application(
                    &self,
                    parameter: SymbolHandle,
                ) -> Option<(crate::types::TypeReferenceHandle, usize)> {
                    let (ordinal, (_, reference)) = self
                        .substitutions
                        .iter()
                        .enumerate()
                        .rev()
                        .find(|(_, (binder, _))| *binder == parameter)?;
                    let outer = self
                        .substitution_scope_starts
                        .partition_point(|start| *start <= ordinal)
                        .checked_sub(1)?;
                    Some((*reference, outer))
                }

                pub fn argument_identity(
                    &self,
                    program: &TypedTrees,
                    parameter: SymbolHandle,
                ) -> Option<NormalizedTypeIdentity> {
                    let (reference, outer) = self.parameter_application(parameter)?;
                    let binder = program
                        .trait_type_parameters(self.declaration)
                        .iter()
                        .find(|binder| binder.symbol == parameter)?;
                    self.with_identity_scopes(program, |scopes| {
                        if binder.kind.is_const_or_value()
                            || matches!(binder.kind, TypeParameterKind::Machine { .. })
                        {
                            applied_index_identity(
                                program,
                                IndexIdentityArgument::TypeReference(reference),
                                self.argument_owner,
                                &scopes[..outer],
                            )
                        } else {
                            applied_type_identity(
                                program,
                                reference,
                                self.argument_owner,
                                &scopes[..outer],
                            )
                        }
                    })
                }

                pub fn with_identity_scopes<Result>(
                    &self,
                    program: &TypedTrees,
                    use_scopes: impl FnOnce(&[AppliedTypeIdentityScope<'_>]) -> Result,
                ) -> Result {
                    let arguments = self
                        .substitution_scope_starts
                        .iter()
                        .enumerate()
                        .map(|(ordinal, start)| {
                            let end = self
                                .substitution_scope_starts
                                .get(ordinal + 1)
                                .copied()
                                .unwrap_or(self.substitutions.len());
                            self.substitutions[*start..end]
                                .iter()
                                .map(|(parameter, argument)| {
                                    let is_type = program
                                        .data_type_parameter_by_symbol(*parameter)
                                        .is_some_and(|binder| {
                                            binder.kind == TypeParameterKind::Type
                                        });
                                    (
                                        *parameter,
                                        if is_type {
                                            IndexIdentityArgument::TypeArgument(*argument)
                                        } else {
                                            IndexIdentityArgument::TypeReference(*argument)
                                        },
                                    )
                                })
                                .collect::<Vec<_>>()
                        })
                        .collect::<Vec<_>>();
                    let scopes = self
                        .region_scopes
                        .iter()
                        .zip(&arguments)
                        .map(|(region, arguments)| AppliedTypeIdentityScope {
                            declaration: region.declaration,
                            arguments,
                            lifetimes: &region.lifetimes,
                        })
                        .collect::<Vec<_>>();
                    use_scopes(&scopes)
                }

                pub fn application_identity(
                    &self,
                    program: &TypedTrees,
                ) -> Option<NormalizedTypeIdentity> {
                    self.with_identity_scopes(program, |scopes| {
                        let arguments = program
                            .trait_type_parameters(self.declaration)
                            .iter()
                            .map(|parameter| {
                                let Some((reference, scope)) =
                                    self.parameter_application(parameter.symbol)
                                else {
                                    return program
                                        .normalized_nominal_type_identity(parameter.symbol);
                                };
                                if matches!(
                                    parameter.kind,
                                    TypeParameterKind::Const { .. }
                                        | TypeParameterKind::Value { .. }
                                        | TypeParameterKind::Machine { .. }
                                ) {
                                    applied_index_identity(
                                        program,
                                        IndexIdentityArgument::TypeReference(reference),
                                        self.argument_owner,
                                        &scopes[..scope],
                                    )
                                } else {
                                    applied_type_identity(
                                        program,
                                        reference,
                                        self.argument_owner,
                                        &scopes[..scope],
                                    )
                                }
                            })
                            .collect::<Option<Vec<_>>>()?;
                        let lifetimes = self
                            .declaration
                            .lifetime_parameters
                            .iter()
                            .map(|parameter| {
                                applied_lifetime_identity(
                                    program,
                                    parameter.as_str(),
                                    self.argument_owner,
                                    scopes,
                                )
                            })
                            .collect::<Option<Vec<_>>>()?;
                        local_trait_application_identity(
                            program,
                            self.declaration.symbol,
                            &arguments,
                            &lifetimes,
                        )
                    })
                }
            }

            fn invocation_region_scope(
                program: &TypedTrees,
                declaration: SymbolHandle,
                argument_owner: SymbolHandle,
                bindings: &[crate::static_applications::StaticMachineLifetimeBinding],
            ) -> Option<AppliedDeclarationRegionScope> {
                let declaration = program.lifetime_declaration_owner(declaration);
                let mut lifetimes = Vec::with_capacity(bindings.len());
                for binding in bindings {
                    let parameter = lifetime_parameter_identity(
                        program,
                        declaration,
                        binding.parameter.as_str(),
                    )?;
                    if parameter.declaration != declaration
                        || lifetimes
                            .iter()
                            .any(|(previous, _)| previous == binding.parameter.as_str())
                    {
                        return None;
                    }
                    lifetime_parameter_identity(
                        program,
                        argument_owner,
                        binding.argument.as_str(),
                    )?;
                    lifetimes.push((
                        binding.parameter.as_str().to_owned(),
                        binding.argument.as_str().to_owned(),
                    ));
                }
                Some(AppliedDeclarationRegionScope {
                    declaration,
                    lifetimes,
                })
            }
        }
        use regions::AppliedDeclarationRegionScope;
        pub(crate) mod enclosing {
            use super::project_type_reference;
            use super::{AppliedDeclarationRegionScope, AppliedTraitScope};
            use crate::TypedTrees;
            use crate::data::DataDefinition;
            use crate::name::Identifier;
            use crate::types::{TypeReferenceHandle, TypeReferenceNode};

            impl TypedTrees {
                pub fn project_enclosing_data_type_reference(
                    &self,
                    reference: TypeReferenceHandle,
                    applications: &[TypeReferenceHandle],
                ) -> Option<(TypeReferenceHandle, usize)> {
                    let mut substitutions = Vec::new();
                    let mut starts = Vec::new();
                    for application in applications {
                        let application = data_application(self, *application)?;
                        starts.push(substitutions.len());
                        substitutions.extend(
                            self.data_type_parameters(application.declaration)
                                .iter()
                                .zip(application.arguments)
                                .filter(|(parameter, _)| {
                                    parameter.kind == crate::data::TypeParameterKind::Type
                                })
                                .map(|(parameter, argument)| (parameter.symbol, *argument)),
                        );
                    }
                    let mut applied =
                        project_type_reference(self, reference, &substitutions, &starts);
                    let mut visited = Vec::new();
                    loop {
                        if !self.contains_type_reference(applied.reference)
                            || visited.contains(&(applied.reference, applied.scope_starts.len()))
                        {
                            return None;
                        }
                        visited.push((applied.reference, applied.scope_starts.len()));
                        let TypeReferenceNode::Named { symbol, .. } =
                            self.type_reference_table.type_reference(applied.reference)
                        else {
                            break;
                        };
                        let Some(origin) = self
                            .data_definition_by_symbol(*symbol)
                            .and_then(|definition| definition.generic_instance)
                        else {
                            break;
                        };
                        applied = project_type_reference(
                            self,
                            origin,
                            applied.substitutions,
                            applied.scope_starts,
                        );
                    }
                    Some((applied.reference, applied.scope_starts.len()))
                }

                pub fn with_enclosing_data_identity_scopes<Result>(
                    &self,
                    applications: &[TypeReferenceHandle],
                    use_scopes: impl FnOnce(
                        &[crate::type_identity::AppliedTypeIdentityScope<'_>],
                    ) -> Result,
                ) -> Option<Result> {
                    let applications = applications
                        .iter()
                        .map(|reference| data_application(self, *reference))
                        .collect::<Option<Vec<_>>>()?;
                    let arguments =
                        applications
                            .iter()
                            .map(|application| {
                                self.data_type_parameters(application.declaration)
                    .iter()
                    .zip(application.arguments)
                    .map(|(parameter, actual)| {
                        let actual = if parameter.kind == crate::data::TypeParameterKind::Type {
                            crate::type_identity::IndexIdentityArgument::TypeArgument(*actual)
                        } else {
                            crate::type_identity::IndexIdentityArgument::TypeReference(*actual)
                        };
                        (parameter.symbol, actual)
                    })
                    .collect::<Vec<_>>()
                            })
                            .collect::<Vec<_>>();
                    let lifetimes = applications
                        .iter()
                        .map(|application| {
                            application
                                .declaration
                                .lifetime_parameters
                                .iter()
                                .zip(application.lifetimes)
                                .map(|(parameter, actual)| {
                                    (parameter.as_str().to_owned(), actual.as_str().to_owned())
                                })
                                .collect::<Vec<_>>()
                        })
                        .collect::<Vec<_>>();
                    let scopes = applications
                        .iter()
                        .zip(&arguments)
                        .zip(&lifetimes)
                        .map(|((application, arguments), lifetimes)| {
                            crate::type_identity::AppliedTypeIdentityScope {
                                declaration: application.declaration.symbol,
                                arguments,
                                lifetimes,
                            }
                        })
                        .collect::<Vec<_>>();
                    Some(use_scopes(&scopes))
                }
            }

            pub(crate) struct DataApplication<'program> {
                pub declaration: &'program DataDefinition,
                pub arguments: &'program [TypeReferenceHandle],
                pub lifetimes: &'program [Identifier],
            }

            pub(crate) fn data_application(
                program: &TypedTrees,
                mut reference: TypeReferenceHandle,
            ) -> Option<DataApplication<'_>> {
                if !program.contains_type_reference(reference) {
                    return None;
                }
                if let TypeReferenceNode::Named { symbol, .. } =
                    program.type_reference_table.type_reference(reference)
                    && let Some(origin) =
                        program.data_definition_by_symbol(*symbol)?.generic_instance
                {
                    if origin == reference
                        || !program.type_reference_table.contains_type_reference(origin)
                    {
                        return None;
                    }
                    reference = origin;
                }
                let (declaration, arguments, lifetimes) =
                    match program.type_reference_table.type_reference(reference) {
                        TypeReferenceNode::Named { symbol, .. } => (
                            program.data_definition_by_symbol(*symbol)?,
                            &[][..],
                            &[][..],
                        ),
                        TypeReferenceNode::Generic {
                            base_symbol,
                            arguments,
                            lifetime_arguments,
                            ..
                        } => (
                            program.data_definition_by_symbol(*base_symbol)?,
                            program.type_reference_handles(*arguments),
                            lifetime_arguments.as_slice(),
                        ),
                        _ => return None,
                    };
                let parameters = program.data_type_parameters(declaration);
                if parameters.len() != arguments.len()
                    || declaration.lifetime_parameters.len() != lifetimes.len()
                {
                    return None;
                }
                Some(DataApplication {
                    declaration,
                    arguments,
                    lifetimes,
                })
            }

            pub(crate) fn append_data_application(
                program: &TypedTrees,
                reference: TypeReferenceHandle,
                scope: &mut AppliedTraitScope<'_>,
            ) -> Option<()> {
                let DataApplication {
                    declaration,
                    arguments,
                    lifetimes,
                } = data_application(program, reference)?;
                let parameters = program.data_type_parameters(declaration);
                scope
                    .substitution_scope_starts
                    .push(scope.substitutions.len());
                scope.substitutions.extend(
                    parameters
                        .iter()
                        .zip(arguments)
                        .map(|(parameter, argument)| (parameter.symbol, *argument)),
                );
                scope.region_scopes.push(AppliedDeclarationRegionScope::new(
                    declaration.symbol,
                    &declaration.lifetime_parameters,
                    lifetimes,
                ));
                Some(())
            }
        }
        pub(crate) mod evidence {
            use super::AppliedTraitScope;
            use crate::{TypedTrees, machine::Machine, signature::StateSignature};
            use symbols::SymbolHandle;

            #[derive(Clone)]
            pub struct GenericBoundRequirement<'program> {
                pub signature: &'program StateSignature,
                pub application: AppliedTraitScope<'program>,
            }

            impl TypedTrees {
                pub fn evidence_requirement_declaration(
                    &self,
                    target: SymbolHandle,
                ) -> Result<Option<&StateSignature>, String> {
                    let selected = self.symbols.get(target);
                    if selected.kind != symbols::SymbolKind::State
                        || !selected.parent.is_valid()
                        || self.symbols.get(selected.parent).kind
                            != symbols::SymbolKind::ConformanceParameter
                    {
                        return Ok(None);
                    }
                    let binder = selected.parent;
                    let owner = self.symbols.get(binder).parent;
                    let bounds = self
                        .declaration_conformance_bounds(owner)
                        .ok_or("evidence child lost its declaring bound owner")?;
                    let mut bounds = bounds.iter().filter(|bound| bound.binder == binder);
                    let bound = bounds
                        .next()
                        .ok_or("evidence child lost its exact authored bound")?;
                    if bounds.next().is_some() {
                        return Err("evidence child has multiple authored bounds".into());
                    }
                    let mut requirements = Vec::new();
                    collect_requirement_namespace(
                        self,
                        bound.trait_symbol,
                        &mut Vec::new(),
                        &mut requirements,
                    )?;
                    let children = self
                        .symbols
                        .child_handles(binder)
                        .ok_or("evidence binder lost its requirement children")?
                        .collect::<Vec<_>>();
                    if children.len() != requirements.len()
                        || children.iter().any(|child| {
                            self.symbols.get(*child).kind != symbols::SymbolKind::State
                        })
                    {
                        return Err(
                            "evidence namespace disagrees with its declared requirements".into(),
                        );
                    }
                    let ordinal = children
                        .iter()
                        .position(|child| *child == target)
                        .ok_or("evidence requirement is not an exact binder child")?;
                    Ok(Some(requirements[ordinal]))
                }

                pub fn requirement_callable_parameters(
                    &self,
                    requirement: &StateSignature,
                    provider: &[crate::data::TypeParameter],
                ) -> Vec<crate::data::TypeParameter> {
                    use crate::data::TypeParameterKind;
                    let mut ordinary = provider
                        .iter()
                        .filter(|parameter| parameter.kind != TypeParameterKind::Implementation);
                    let mut evidence = provider
                        .iter()
                        .filter(|parameter| parameter.kind == TypeParameterKind::Implementation);
                    let mut aligned = Vec::with_capacity(provider.len());
                    for required in self.requirement_invocation_contract_parameters(requirement) {
                        let selected = if required.kind == TypeParameterKind::Implementation {
                            evidence.next()
                        } else {
                            ordinary.next()
                        };
                        if let Some(parameter) = selected {
                            aligned.push(parameter.clone());
                        }
                    }
                    aligned.extend(ordinary.cloned());
                    aligned.extend(evidence.cloned());
                    aligned
                }

                pub fn machine_callable_parameters(
                    &self,
                    machine: &Machine,
                ) -> Vec<crate::data::TypeParameter> {
                    let mut parameters = self.machine_type_parameters(machine).to_vec();
                    for bound in &machine.conformance_bounds {
                        if !parameters
                            .iter()
                            .any(|parameter| parameter.symbol == bound.binder)
                        {
                            parameters.push(crate::data::TypeParameter {
                                symbol: bound.binder,
                                name: bound.binder_name.clone(),
                                kind: crate::data::TypeParameterKind::Implementation,
                                ..Default::default()
                            });
                        }
                    }
                    parameters
                }

                pub fn callable_conformance_bounds(
                    &self,
                    owner: SymbolHandle,
                ) -> Option<&[crate::machine::GenericConformanceBound]> {
                    if let Some(application) = self.closed_requirement_application(owner) {
                        return Some(&application.signature.conformance_bounds);
                    }
                    if let Some(machine) = self.machine_by_symbol(owner) {
                        return Some(&machine.conformance_bounds);
                    }
                    if let Some((_, signature)) = self.trait_machine_signature(owner) {
                        return Some(&signature.conformance_bounds);
                    }
                    self.retained_static_machine_contract(owner)
                        .map(|contract| contract.signature().conformance_bounds.as_slice())
                }

                fn declaration_conformance_bounds(
                    &self,
                    owner: SymbolHandle,
                ) -> Option<&[crate::machine::GenericConformanceBound]> {
                    self.callable_conformance_bounds(owner)
                        .or_else(|| {
                            self.trait_definition_by_symbol(owner)
                                .map(|declaration| declaration.conformance_bounds.as_slice())
                        })
                        .or_else(|| {
                            self.unique_data_definition_by_symbol(owner)
                                .map(|declaration| declaration.conformance_bounds.as_slice())
                        })
                        .or_else(|| {
                            self.type_aliases()
                                .iter()
                                .find(|declaration| declaration.symbol == owner)
                                .map(|declaration| declaration.conformance_bounds.as_slice())
                        })
                }

                pub fn callable_conformance_bound(
                    &self,
                    owner: SymbolHandle,
                    binder: SymbolHandle,
                ) -> Option<&crate::machine::GenericConformanceBound> {
                    if !binder.is_valid()
                        || self.symbols.get(binder).kind
                            != symbols::SymbolKind::ConformanceParameter
                    {
                        return None;
                    }
                    if let Some(bound) = self
                        .callable_conformance_bounds(owner)
                        .and_then(|bounds| bounds.iter().find(|bound| bound.binder == binder))
                    {
                        return Some(bound);
                    }
                    let parent = self.symbols.get(binder).parent;
                    self.declaration_conformance_bounds(parent)
                        .and_then(|bounds| bounds.iter().find(|bound| bound.binder == binder))
                }

                pub fn conformance_target_requirement<'program>(
                    &'program self,
                    machine: &'program Machine,
                    target: SymbolHandle,
                ) -> Result<Option<GenericBoundRequirement<'program>>, String> {
                    if !target.is_valid() {
                        return Ok(None);
                    }
                    for bound in &machine.conformance_bounds {
                        let binder = bound.binder;
                        if self
                            .symbols
                            .child_handles(binder)
                            .is_some_and(|mut children| children.any(|child| child == target))
                        {
                            return self.conformance_bound_requirement(machine, binder, target);
                        }
                    }
                    Ok(None)
                }

                pub fn conformance_bound_requirement<'program>(
                    &'program self,
                    machine: &'program Machine,
                    receiver: SymbolHandle,
                    target: SymbolHandle,
                ) -> Result<Option<GenericBoundRequirement<'program>>, String> {
                    self.evidence_bound_requirement(machine.symbol, receiver, target)
                }

                pub fn evidence_bound_requirement<'program>(
                    &'program self,
                    owner: SymbolHandle,
                    receiver: SymbolHandle,
                    target: SymbolHandle,
                ) -> Result<Option<GenericBoundRequirement<'program>>, String> {
                    let Some(bounds) = self.declaration_conformance_bounds(owner) else {
                        return Ok(None);
                    };
                    if !receiver.is_valid()
                        || self.symbols.get(receiver).kind
                            != symbols::SymbolKind::ConformanceParameter
                        || !bounds.iter().any(|bound| bound.binder == receiver)
                    {
                        return Ok(None);
                    }
                    let failure = || {
                        format!(
                            "declaration `{}` conformance-evidence call has no unique exact requirement",
                            self.symbols.name(owner)
                        )
                    };
                    let mut bounds = bounds.iter().filter(|bound| bound.binder == receiver);
                    let bound = bounds.next().ok_or_else(failure)?;
                    if bounds.next().is_some() {
                        return Err(format!(
                            "{}: its receiver has multiple authored bounds",
                            failure()
                        ));
                    }
                    if !target.is_valid()
                        || self.symbols.get(target).kind != symbols::SymbolKind::State
                    {
                        return Err(format!(
                            "{}: its selected target is not a requirement state",
                            failure()
                        ));
                    }
                    let prepared = self.closed_requirement_application(target);
                    if prepared.is_none()
                        && !self
                            .symbols
                            .child_handles(receiver)
                            .is_some_and(|mut children| children.any(|child| child == target))
                    {
                        return Err(format!(
                            "{}: its selected target is not owned by the receiver",
                            failure()
                        ));
                    }
                    let authored_requirement = if prepared.is_none() {
                        Some(
                            self.evidence_requirement_declaration(target)?
                                .ok_or_else(failure)?
                                .symbol,
                        )
                    } else {
                        None
                    };
                    let carrier = self
                        .trait_definition_by_symbol(bound.trait_symbol)
                        .ok_or_else(failure)?;
                    let root = crate::trait_definition::AppliedTraitScope::new(
                        self,
                        carrier,
                        crate::trait_definition::TraitApplicationArguments {
                            lexical_owner: owner,
                            enclosing_data_applications: &[],
                            arguments: &bound.arguments,
                            lifetimes: &bound.lifetime_arguments,
                        },
                    )
                    .ok_or_else(|| {
                        format!(
                            "{}: its complete trait application cannot be represented",
                            failure()
                        )
                    })?;
                    let mut selected: Option<GenericBoundRequirement<'program>> = None;
                    let mut ambiguous = false;
                    root.visit(self, &mut |application| {
                        for signature in self.trait_machine_signatures(application.declaration) {
                            let matches = if let Some(prepared) = &prepared {
                                prepared.template == signature.symbol
                                    && self
                                        .requirement_application_matches_scope(target, application)
                            } else {
                                Some(signature.symbol) == authored_requirement
                            };
                            if !matches {
                                continue;
                            }
                            let signature = prepared
                                .as_ref()
                                .map_or(signature, |prepared| prepared.signature);
                            if let Some(known) = &selected {
                                ambiguous |= known.signature.symbol != signature.symbol
                                    || !known.application.same_application(self, application);
                                continue;
                            }
                            selected = Some(GenericBoundRequirement {
                                signature,
                                application: application.clone(),
                            });
                        }
                    });
                    if ambiguous {
                        return Err(format!(
                            "{}: reachable declarations disagree on the requirement application",
                            failure()
                        ));
                    }
                    selected.map(Some).ok_or_else(|| {
                        format!(
                            "{}: its selected child has no reachable authored requirement",
                            failure()
                        )
                    })
                }
            }

            fn collect_requirement_namespace<'program>(
                program: &'program TypedTrees,
                trait_symbol: SymbolHandle,
                visited: &mut Vec<SymbolHandle>,
                output: &mut Vec<&'program StateSignature>,
            ) -> Result<(), String> {
                if visited.contains(&trait_symbol) {
                    return Ok(());
                }
                let declaration = program
                    .trait_definition_by_symbol(trait_symbol)
                    .ok_or("evidence namespace lost a resolved trait declaration")?;
                visited.push(trait_symbol);
                output.extend(program.trait_machine_signatures(declaration));
                for parent in declaration
                    .refines
                    .iter()
                    .chain(program.trait_requirements(declaration))
                {
                    collect_requirement_namespace(program, parent.symbol, visited, output)?;
                }
                Ok(())
            }
        }
        pub use evidence::GenericBoundRequirement;
        pub(crate) mod refinement_views {
            use super::AppliedTraitScope;
            use crate::{
                TypedTrees,
                signature::StateSignature,
                trait_definition::{TraitRefinementClause, TraitRefinementReach},
            };
            use symbols::SymbolHandle;

            impl<'program> AppliedTraitScope<'program> {
                fn refinement_prefixes<'scope>(
                    &'scope self,
                    program: &'scope TypedTrees,
                ) -> Vec<AppliedTraitScope<'scope>> {
                    self.region_scopes
                        .iter()
                        .enumerate()
                        .filter_map(|(position, frame)| {
                            let declaration =
                                program.trait_definition_by_symbol(frame.declaration)?;
                            if declaration.refines.is_none()
                                || declaration.refinement_clauses.is_empty()
                            {
                                return None;
                            }
                            let mut prefix: AppliedTraitScope<'scope> = self.clone();
                            prefix.declaration = declaration;
                            let end = self
                                .substitution_scope_starts
                                .get(position + 1)
                                .copied()
                                .unwrap_or(self.substitutions.len());
                            prefix.substitutions.truncate(end);
                            prefix.substitution_scope_starts.truncate(position + 1);
                            prefix.region_scopes.truncate(position + 1);
                            Some(prefix)
                        })
                        .collect()
                }

                pub(crate) fn callable_contract_key(
                    &self,
                    program: &TypedTrees,
                ) -> Option<(SymbolHandle, crate::type_identity::NormalizedTypeIdentity)>
                {
                    let prefixes = self.refinement_prefixes(program);
                    let owner = prefixes
                        .first()
                        .map_or(self.declaration.symbol, |scope| scope.declaration.symbol);
                    let identities = prefixes
                        .iter()
                        .map(|scope| scope.application_identity(program))
                        .collect::<Option<Vec<_>>>()?;
                    crate::type_identity::local_trait_contract_identity(
                        program,
                        self.application_identity(program)?,
                        &identities,
                    )
                    .map(|identity| (owner, identity))
                }

                pub(crate) fn callable_refinement_clauses(
                    &self,
                    program: &TypedTrees,
                    requirement: &str,
                ) -> Vec<(TraitRefinementClause, super::TraitApplicationEnvironment)>
                {
                    self.refinement_prefixes(program)
                        .into_iter()
                        .flat_map(|prefix| {
                            let environment = prefix.environment();
                            prefix
                                .declaration
                                .refinement_clauses
                                .iter()
                                .filter(|clause| {
                                    clause
                                        .requirement
                                        .as_ref()
                                        .is_none_or(|name| name.as_str() == requirement)
                                })
                                .cloned()
                                .map(|clause| (clause, environment.clone()))
                                .collect::<Vec<_>>()
                        })
                        .collect()
                }
            }

            impl TypedTrees {
                pub(crate) fn apply_callable_refinements(
                    &mut self,
                    signature: &mut StateSignature,
                    clauses: &[(TraitRefinementClause, super::TraitApplicationEnvironment)],
                ) -> Result<(), &'static str> {
                    for (clause, environment) in clauses {
                        if !clause.signature.contracts.is_empty() {
                            self.apply_refinement_postconditions(
                                signature,
                                &clause.signature,
                                environment,
                            )?;
                        }
                        if !clause.signature.suspends_keyword_source_spans.is_empty() {
                            signature.suspends &= clause.signature.suspends;
                            signature
                                .suspends_keyword_source_spans
                                .extend_from_slice(&clause.signature.suspends_keyword_source_spans);
                        }
                        if !clause.signature.blocks_keyword_source_spans.is_empty() {
                            signature.blocks &= clause.signature.blocks;
                            signature
                                .blocks_keyword_source_spans
                                .extend_from_slice(&clause.signature.blocks_keyword_source_spans);
                        }
                        if let language_semantics::TerminationGuarantee::Terminates { premises } =
                            &clause.signature.termination_guarantee
                            && (premises.is_empty()
                                || matches!(
                                    signature.termination_guarantee,
                                    language_semantics::TerminationGuarantee::NoGuarantee
                                ))
                        {
                            signature.termination_guarantee =
                                clause.signature.termination_guarantee.clone();
                        }
                        let row = match &clause.service_reach {
                            TraitRefinementReach::Inherited => continue,
                            TraitRefinementReach::Concrete(row) => *row,
                            TraitRefinementReach::IndependentBounded(rows) => rows
                                .iter()
                                .find(|row| row.requirement == signature.name)
                                .ok_or(
                                    "refinement callable view lost its independent requirement row",
                                )?
                                .row,
                        };
                        if row == language_semantics::ServiceReachRowId::NULL {
                            return Err("refinement callable view lost its authored reach row");
                        }
                        if signature.service_reach_is_installation_bound {
                            signature.service_reach_row = row;
                        } else {
                            let current = signature.service_reach_row;
                            let abstract_row = self.service_reach_rows.is_abstract(current)
                                || self.service_reach_rows.is_abstract(row);
                            let services = self
                                .service_reach_rows
                                .services(current)
                                .iter()
                                .filter(|service| {
                                    self.service_reach_rows.services(row).contains(service)
                                })
                                .copied()
                                .collect();
                            let ceiling = self.service_reach_rows.intern(services);
                            signature.service_reach_row = if abstract_row {
                                self.service_reach_rows.intern_abstract(ceiling)
                            } else {
                                ceiling
                            };
                        }
                        signature.service_reach_is_installation_bound = false;
                    }
                    Ok(())
                }
            }
        }
        pub(crate) mod retention {
            use super::{AppliedDeclarationRegionScope, AppliedTraitScope};
            use crate::CallableSubstitution;
            use crate::TypedTrees;
            use crate::name::Identifier;
            use crate::static_applications::StaticMachineLifetimeBinding;
            use crate::type_identity;
            use crate::types::TypeReferenceHandle;
            use arena::HandleSpan;
            use symbols::SymbolHandle;

            #[derive(Debug, Clone, PartialEq, Eq)]
            pub struct TraitApplicationEnvironment {
                declaration: SymbolHandle,
                argument_owner: SymbolHandle,
                substitutions: Vec<(SymbolHandle, TypeReferenceHandle)>,
                scope_starts: Vec<usize>,
                region_scopes: Vec<AppliedDeclarationRegionScope>,
            }

            #[derive(
                Debug, Clone, Copy, Default, PartialEq, Eq, Hash, compiler_storage::Record,
            )]
            pub struct RetainedTraitScope {
                declaration: SymbolHandle,
                argument_owner: SymbolHandle,
                frames: HandleSpan<RetainedTraitApplicationFrame>,
            }

            #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
            pub(crate) struct RetainedTraitApplicationFrame {
                declaration: SymbolHandle,
                arguments: HandleSpan<CallableSubstitution>,
                lifetimes: HandleSpan<StaticMachineLifetimeBinding>,
            }

            impl AppliedTraitScope<'_> {
                pub fn environment(&self) -> TraitApplicationEnvironment {
                    TraitApplicationEnvironment {
                        declaration: self.declaration.symbol,
                        argument_owner: self.argument_owner,
                        substitutions: self.substitutions.clone(),
                        scope_starts: self.substitution_scope_starts.clone(),
                        region_scopes: self.region_scopes.clone(),
                    }
                }
            }

            impl TraitApplicationEnvironment {
                pub(crate) fn lifetime_identity(
                    &self,
                    program: &TypedTrees,
                    reference: TypeReferenceHandle,
                    preceding_frames: usize,
                    name: &str,
                    lexical_owner: SymbolHandle,
                ) -> Option<(type_identity::LifetimeParameterIdentity, bool)> {
                    let scope = self.scope(program)?;
                    scope.with_identity_scopes(program, |frames| {
            let mut visible = frames.get(..preceding_frames)?;
            let recorded_owner = program
                .type_reference_table
                .operand_lifetime_owner(reference);
            let retained_owner = if recorded_owner.is_valid() {
                recorded_owner
            } else {
                lexical_owner
            };
            let owner = if retained_owner.is_valid() {
                let region_owner =
                    type_identity::lifetime_parameter_identity(program, retained_owner, name)?
                        .declaration;
                let frame = visible.iter().rposition(|frame| {
                    program.lifetime_declaration_owner(frame.declaration)
                        == program.lifetime_declaration_owner(region_owner)
                });
                if let Some(ordinal) = frame {
                    visible = &visible[..=ordinal];
                    return type_identity::applied_lifetime_parameter_identity_with_supplier(
                        program,
                        retained_owner,
                        self.argument_owner,
                        name,
                        visible,
                    );
                } else {
                    visible = &[];
                    retained_owner
                }
            } else {
                self.argument_owner
            };
            type_identity::applied_lifetime_parameter_identity_with_supplier(
                program,
                owner,
                self.argument_owner,
                name,
                visible,
            )
                    })
                }

                pub(crate) fn resolved_lifetime_identity(
                    &self,
                    program: &TypedTrees,
                    region: type_identity::LifetimeParameterIdentity,
                    preceding_frames: usize,
                ) -> Option<(type_identity::LifetimeParameterIdentity, bool)> {
                    let scope = self.scope(program)?;
                    scope.with_identity_scopes(program, |frames| {
                        type_identity::apply_resolved_lifetime_parameter_identity(
                            program,
                            region,
                            self.argument_owner,
                            frames.get(..preceding_frames)?,
                        )
                    })
                }

                pub(crate) fn frame_count(&self) -> usize {
                    self.scope_starts.len()
                }

                pub(crate) fn argument_owner(&self) -> SymbolHandle {
                    self.argument_owner
                }

                pub fn scope<'program>(
                    &self,
                    program: &'program TypedTrees,
                ) -> Option<AppliedTraitScope<'program>> {
                    Some(AppliedTraitScope {
                        declaration: program.trait_definition_by_symbol(self.declaration)?,
                        argument_owner: self.argument_owner,
                        substitutions: self.substitutions.clone(),
                        substitution_scope_starts: self.scope_starts.clone(),
                        region_scopes: self.region_scopes.clone(),
                    })
                }

                pub fn retain(
                    &self,
                    program: &mut TypedTrees,
                ) -> Result<RetainedTraitScope, &'static str> {
                    if self.scope_starts.len() != self.region_scopes.len() {
                        return Err("trait application lost its simultaneous region frames");
                    }
                    let mut frames = Vec::with_capacity(self.scope_starts.len());
                    for (ordinal, region) in self.region_scopes.iter().enumerate() {
                        let start = self.scope_starts[ordinal];
                        let end = self
                            .scope_starts
                            .get(ordinal + 1)
                            .copied()
                            .unwrap_or(self.substitutions.len());
                        let arguments = self
                            .substitutions
                            .get(start..end)
                            .ok_or("trait application has an invalid substitution frame")?;
                        let arguments =
                            program
                                .callable_substitutions
                                .insert_many(arguments.iter().map(|(parameter, actual)| {
                                    CallableSubstitution {
                                        parameter: *parameter,
                                        actual: *actual,
                                    }
                                }));
                        let lifetimes = program.callable_lifetime_substitutions.insert_many(
                            region.lifetimes.iter().map(|(parameter, actual)| {
                                StaticMachineLifetimeBinding {
                                    parameter: Identifier::generated(parameter),
                                    argument: Identifier::generated(actual),
                                }
                            }),
                        );
                        frames.push(RetainedTraitApplicationFrame {
                            declaration: region.declaration,
                            arguments,
                            lifetimes,
                        });
                    }
                    Ok(RetainedTraitScope {
                        declaration: self.declaration,
                        argument_owner: self.argument_owner,
                        frames: program.trait_application_frames.insert_many(frames),
                    })
                }
            }

            impl RetainedTraitScope {
                pub fn supplying_owner(self) -> SymbolHandle {
                    self.argument_owner
                }

                pub(crate) fn visit_references(
                    self,
                    program: &TypedTrees,
                    retained: &mut impl crate::references::TypedReferences,
                ) {
                    retained.declaration_description_dependency(self.argument_owner);
                    for frame in program.trait_application_frames.span_or_empty(self.frames) {
                        retained.declaration_description_dependency(frame.declaration);
                        for binding in program
                            .callable_substitutions
                            .span_or_empty(frame.arguments)
                        {
                            retained.declaration_description_dependency(binding.parameter);
                            crate::references::visit_type_reference(
                                program,
                                retained,
                                binding.actual,
                            );
                        }
                    }
                }

                pub fn scope<'program>(
                    self,
                    program: &'program TypedTrees,
                ) -> Option<AppliedTraitScope<'program>> {
                    let mut substitutions = Vec::new();
                    let mut scope_starts = Vec::new();
                    let mut region_scopes = Vec::new();
                    for frame in program.trait_application_frames.span_or_empty(self.frames) {
                        scope_starts.push(substitutions.len());
                        substitutions.extend(
                            program
                                .callable_substitutions
                                .span_or_empty(frame.arguments)
                                .iter()
                                .map(|binding| (binding.parameter, binding.actual)),
                        );
                        region_scopes.push(AppliedDeclarationRegionScope {
                            declaration: frame.declaration,
                            lifetimes: program
                                .callable_lifetime_substitutions
                                .span_or_empty(frame.lifetimes)
                                .iter()
                                .map(|binding| {
                                    (
                                        binding.parameter.as_str().to_owned(),
                                        binding.argument.as_str().to_owned(),
                                    )
                                })
                                .collect(),
                        });
                    }
                    Some(AppliedTraitScope {
                        declaration: program.trait_definition_by_symbol(self.declaration)?,
                        argument_owner: self.argument_owner,
                        substitutions,
                        substitution_scope_starts: scope_starts,
                        region_scopes,
                    })
                }
            }
        }
        pub(crate) mod source_fields {
            use super::{
                AppliedTraitScope, TraitApplicationArguments, enclosing, project_type_reference,
            };
            use crate::TypedTrees;
            use crate::data;
            use crate::references::TypedReferences;
            use crate::references::visit_type_reference;
            use crate::types::{TypeReferenceHandle, TypeReferenceNode};
            use data::DataMember;
            use std::collections::BTreeSet;
            use symbols::SymbolHandle;

            pub(crate) fn contract_application_roots<'program>(
                program: &'program TypedTrees,
                occurrences: &[crate::references::TypeOccurrence],
            ) -> Vec<AppliedTraitScope<'program>> {
                if !program
                    .type_reference_table
                    .references()
                    .any(|(_, node)| matches!(node, TypeReferenceNode::DynamicTrait { .. }))
                {
                    debug_assert!(
                        walk_contract_application_roots(program, occurrences).is_empty(),
                        "a contract-root walk published a root without a dynamic trait row",
                    );
                    return Vec::new();
                }
                walk_contract_application_roots(program, occurrences)
            }

            fn walk_contract_application_roots<'program>(
                program: &'program TypedTrees,
                occurrences: &[crate::references::TypeOccurrence],
            ) -> Vec<AppliedTraitScope<'program>> {
                let mut discovery = SourceFields {
                    program,
                    roots: Vec::new(),
                    seen: Vec::new(),
                    active: Vec::new(),
                    field_inventories: Vec::new(),
                    field_references: Vec::new(),
                };
                for occurrence in occurrences {
                    discovery.visit(
                        occurrence.lexical_owner,
                        occurrence.reference,
                        &[],
                        &[],
                        &[],
                    );
                }
                discovery.roots
            }

            #[derive(PartialEq, Eq, PartialOrd, Ord)]
            struct SourceFieldOccurrence {
                owner: (u32, u32),
                reference: (u32, u32),
                frames: Vec<(u32, u32)>,
            }

            struct SourceFields<'program> {
                program: &'program TypedTrees,
                roots: Vec<AppliedTraitScope<'program>>,
                seen: Vec<BTreeSet<SourceFieldOccurrence>>,
                active: Vec<(SymbolHandle, usize)>,
                field_inventories: Vec<(SymbolHandle, std::ops::Range<usize>)>,
                field_references: Vec<TypeReferenceHandle>,
            }

            impl SourceFields<'_> {
                fn visit(
                    &mut self,
                    owner: SymbolHandle,
                    reference: TypeReferenceHandle,
                    enclosing_applications: &[TypeReferenceHandle],
                    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
                    scope_starts: &[usize],
                ) {
                    let applied = project_type_reference(
                        self.program,
                        reference,
                        substitutions,
                        scope_starts,
                    );
                    let reference = applied.reference;
                    if !reference.is_valid() {
                        return;
                    }
                    let enclosing_applications =
                        &enclosing_applications[..applied.scope_starts.len()];
                    let slot = reference.arena_index() as usize;
                    if self.seen.len() <= slot {
                        self.seen.resize_with(slot + 1, BTreeSet::new);
                    }
                    let identity =
                        |handle: TypeReferenceHandle| (handle.arena_index(), handle.generation());
                    if !self.seen[slot].insert(SourceFieldOccurrence {
                        owner: (owner.arena_index(), owner.generation()),
                        reference: identity(reference),
                        frames: enclosing_applications
                            .iter()
                            .copied()
                            .map(identity)
                            .collect(),
                    }) {
                        return;
                    }

                    if let TypeReferenceNode::DynamicTrait {
                        symbol, arguments, ..
                    } = self.program.type_reference_table.type_reference(reference)
                    {
                        if let Some(declaration) = self.program.trait_definition_by_symbol(*symbol)
                            && let Some(scope) = AppliedTraitScope::new(
                                self.program,
                                declaration,
                                TraitApplicationArguments {
                                    lexical_owner: owner,
                                    enclosing_data_applications: enclosing_applications,
                                    arguments: self.program.type_reference_handles(*arguments),
                                    lifetimes: &[],
                                },
                            )
                        {
                            self.roots.push(scope);
                        }
                        return;
                    }

                    match self.program.type_reference_table.type_reference(reference) {
                        TypeReferenceNode::Reference { .. } | TypeReferenceNode::Slice { .. } => {
                            return;
                        }
                        TypeReferenceNode::Constrained { base_type, .. } => {
                            self.visit(
                                owner,
                                *base_type,
                                enclosing_applications,
                                applied.substitutions,
                                applied.scope_starts,
                            );
                            return;
                        }
                        TypeReferenceNode::FixedArray { element_type, .. } => {
                            self.visit(
                                owner,
                                *element_type,
                                enclosing_applications,
                                applied.substitutions,
                                applied.scope_starts,
                            );
                            return;
                        }
                        _ => {}
                    }
                    let Some(application) = enclosing::data_application(self.program, reference)
                    else {
                        return;
                    };
                    if self
                        .active
                        .iter()
                        .rev()
                        .find(|(ancestor, _)| *ancestor == application.declaration.symbol)
                        .is_some_and(|(_, end)| applied.scope_starts.len() >= *end)
                    {
                        return;
                    }
                    let mut frames = enclosing_applications.to_vec();
                    frames.push(reference);
                    let mut starts = applied.scope_starts.to_vec();
                    starts.push(applied.substitutions.len());
                    let mut substitutions = applied.substitutions.to_vec();
                    substitutions.extend(
                        self.program
                            .data_type_parameters(application.declaration)
                            .iter()
                            .zip(application.arguments)
                            .map(|(parameter, argument)| (parameter.symbol, *argument)),
                    );

                    let fields = self.field_inventory(application.declaration);
                    self.active
                        .push((application.declaration.symbol, starts.len()));
                    for position in fields {
                        let field = self.field_references[position];
                        self.visit(owner, field, &frames, &substitutions, &starts);
                    }
                    self.active.pop();
                }

                fn field_inventory(
                    &mut self,
                    declaration: &data::DataDefinition,
                ) -> std::ops::Range<usize> {
                    let slot = declaration.symbol.arena_index() as usize;
                    if self.field_inventories.len() <= slot {
                        self.field_inventories
                            .resize_with(slot + 1, || (SymbolHandle::invalid(), 0..0));
                    }
                    let (symbol, fields) = &self.field_inventories[slot];
                    if *symbol == declaration.symbol {
                        return fields.clone();
                    }
                    let mut fields = FieldTypes {
                        program: self.program,
                        references: Vec::new(),
                    };
                    for member in self.program.data_members(declaration) {
                        match member {
                            DataMember::Field(field) => visit_type_reference(
                                self.program,
                                &mut fields,
                                field.type_reference,
                            ),
                            DataMember::Variant(variant) => {
                                for field in self.program.data_payload_fields(variant) {
                                    visit_type_reference(
                                        self.program,
                                        &mut fields,
                                        field.type_reference,
                                    );
                                }
                            }
                        }
                    }
                    let start = self.field_references.len();
                    self.field_references.extend(fields.references);
                    let span = start..self.field_references.len();
                    self.field_inventories[slot] = (declaration.symbol, span.clone());
                    span
                }
            }

            struct FieldTypes<'program> {
                program: &'program TypedTrees,
                references: Vec<TypeReferenceHandle>,
            }

            impl Extend<SymbolHandle> for FieldTypes<'_> {
                fn extend<Symbols: IntoIterator<Item = SymbolHandle>>(&mut self, _: Symbols) {}
            }

            impl TypedReferences for FieldTypes<'_> {
                fn type_reference(&mut self, reference: TypeReferenceHandle) -> bool {
                    if self.references.contains(&reference) {
                        return false;
                    }
                    self.references.push(reference);
                    !matches!(
                        self.program.type_reference_table.type_reference(reference),
                        TypeReferenceNode::Reference { .. } | TypeReferenceNode::Slice { .. }
                    )
                }

                fn expression(&mut self, _: crate::expression::ExpressionHandle) -> bool {
                    false
                }

                fn mathematical_type(
                    &mut self,
                    _: crate::mathematical::MathematicalTypeHandle,
                ) -> bool {
                    false
                }
            }
        }
        pub(crate) use retention::RetainedTraitApplicationFrame;
        pub use retention::{RetainedTraitScope, TraitApplicationEnvironment};

        impl TypedTrees {
            pub fn lexical_conformance_scope(
                &self,
                caller: SymbolHandle,
            ) -> Option<(&super::Conformance, AppliedTraitScope<'_>)> {
                let authored = self.lifetime_declaration_owner(caller);
                let selecting =
                    self.conformance_by_symbol(self.declaration_scope_parent(caller))?;
                if !selecting
                    .rows
                    .iter()
                    .any(|row| row.source.owns_realization() && row.realization_machine == authored)
                {
                    return None;
                }
                let scope = if let Some(capture) =
                    self.implementation_capture(caller, authored, selecting.symbol)
                {
                    capture.conformance_application.trait_scope.scope(self)?
                } else {
                    self.machine_instantiation_path(caller, authored)?;
                    if caller != authored
                        && (!self.conformance_type_parameters(selecting).is_empty()
                            || !selecting.lifetime_parameters.is_empty())
                    {
                        return None;
                    }
                    AppliedTraitScope::from_conformance(self, selecting)?
                };
                Some((selecting, scope))
            }

            pub fn machine_conformance_requirement_scope(
                &self,
                machine: SymbolHandle,
                declaring_trait: SymbolHandle,
                requirement: SymbolHandle,
            ) -> Result<Option<AppliedTraitScope<'_>>, &'static str> {
                let Some((implementation, scope)) = self.lexical_conformance_scope(machine) else {
                    return Ok(None);
                };
                let authored = self.lifetime_declaration_owner(machine);
                let mut rows = implementation.rows.iter().filter(|row| {
                    row.source.owns_realization()
                        && row.realization_machine == authored
                        && row.declaring_trait == declaring_trait
                        && row.requirement == requirement
                });
                let Some(row) = rows.next() else {
                    return Ok(None);
                };
                if rows.next().is_some() {
                    return Err("inline member repeats an exact requirement row");
                }
                let entry = self
                    .machine_by_symbol(authored)
                    .and_then(|machine| self.machine_states(machine).first())
                    .ok_or("inline requirement member lost its authored entry")?;
                if row.realization_state != entry.symbol {
                    return Err("inline requirement row does not name its member entry");
                }
                scope
                    .declaring_scope(self, declaring_trait)
                    .map(Some)
                    .ok_or("inline requirement row lost its exact declaring application")
            }

            pub fn applied_trait_roots(&self) -> Vec<AppliedTraitScope<'_>> {
                let occurrences = crate::references::collect_program_application_occurrences(self);
                self.machines()
            .iter()
            .flat_map(|machine| {
                self.machine_trait_conformances(machine)
                    .iter()
                    .filter_map(move |edge| {
                        AppliedTraitScope::from_machine_conformance(self, machine, edge)
                    })
            })
            .chain(
                self.conformances()
                    .iter()
                    .filter_map(|edge| AppliedTraitScope::from_conformance(self, edge)),
            )
            .chain(self.machines().iter().map(|machine| (machine.symbol, &machine.conformance_bounds))
                .chain(self.traits().iter().map(|declaration| (declaration.symbol, &declaration.conformance_bounds)))
                .flat_map(|(owner, bounds)| {
                    bounds.iter().filter_map(move |bound| {
                        AppliedTraitScope::new(
                            self,
                            self.trait_definition_by_symbol(bound.trait_symbol)?,
                            TraitApplicationArguments {
                                lexical_owner: owner,
                                enclosing_data_applications: &[],
                                arguments: &bound.arguments,
                                lifetimes: &bound.lifetime_arguments,
                            },
                        )
                    })
                }))
            .chain(
                self.traits()
                    .iter()
                    .filter(|declaration| self.trait_type_parameters(declaration).is_empty())
                    .map(AppliedTraitScope::abstract_scope),
            )
            .chain(
                self.closed_conformance_application_sites_from_occurrences(
                    &occurrences.conformance_applications,
                )
                .into_iter()
                .filter_map(|(_, _, _, application)| application.trait_scope.scope(self)),
            )
            .chain(
                self.live_product_operand_applications().into_iter().filter_map(|handle| {
                    let application = self.product_operand_applications.get(handle);
                    matches!(application.role, Some(language_semantics::declaration_selection::ProductOperandRole::BoundarySubject | language_semantics::declaration_selection::ProductOperandRole::Requirement))
                        .then(|| application.scope.scope(self)).flatten()
                }),
            )
            .chain(source_fields::contract_application_roots(self, &occurrences.types))
            .chain(
                occurrences
                    .requirement_applications
                    .into_iter()
                    .filter_map(|occurrence| {
                        self.applied_requirement_scope(
                            occurrence.lexical_owner,
                            occurrence.reference,
                        )
                    }),
            )
            .collect()
            }

            pub fn applied_requirement_scope(
                &self,
                lexical_owner: SymbolHandle,
                application: TypeReferenceHandle,
            ) -> Option<AppliedTraitScope<'_>> {
                let TypeReferenceNode::Generic {
                    base_symbol,
                    lifetime_arguments,
                    arguments,
                    ..
                } = self.type_reference_table.type_reference(application)
                else {
                    return None;
                };
                AppliedTraitScope::new(
                    self,
                    self.trait_definition_by_symbol(*base_symbol)?,
                    TraitApplicationArguments {
                        lexical_owner,
                        enclosing_data_applications: &[],
                        arguments: self.type_reference_table.type_reference_handles(*arguments),
                        lifetimes: lifetime_arguments,
                    },
                )
            }
        }

        #[derive(Clone, Copy)]
        pub struct TraitApplicationArguments<'arguments> {
            pub lexical_owner: SymbolHandle,
            pub enclosing_data_applications: &'arguments [TypeReferenceHandle],
            pub arguments: &'arguments [TypeReferenceHandle],
            pub lifetimes: &'arguments [Identifier],
        }

        #[derive(Clone, Copy, PartialEq, Eq)]
        pub struct AppliedTypeReference<'scope> {
            pub reference: TypeReferenceHandle,
            substitutions: &'scope [(SymbolHandle, TypeReferenceHandle)],
            scope_starts: &'scope [usize],
        }

        impl<'scope> AppliedTypeReference<'scope> {
            pub fn parameter_argument(
                self,
                program: &TypedTrees,
                parameter: SymbolHandle,
            ) -> Option<Self> {
                let (ordinal, (_, reference)) = self
                    .substitutions
                    .iter()
                    .enumerate()
                    .rev()
                    .find(|(_, (binder, _))| *binder == parameter)?;
                let visible_scopes = self
                    .scope_starts
                    .partition_point(|start| *start <= ordinal)
                    .checked_sub(1)?;
                let visible = self.scope_starts.get(visible_scopes).copied()?;
                Some(project_type_reference(
                    program,
                    *reference,
                    &self.substitutions[..visible],
                    &self.scope_starts[..visible_scopes],
                ))
            }

            pub fn child(self, program: &TypedTrees, reference: TypeReferenceHandle) -> Self {
                project_type_reference(program, reference, self.substitutions, self.scope_starts)
            }

            pub fn type_identity(self, program: &TypedTrees) -> NormalizedTypeIdentity {
                program.scoped_type_identity(
                    TypeIdentityRequest {
                        substitutions: self.substitutions,
                        ..TypeIdentityRequest::ordinary(self.reference)
                    },
                    self.scope_starts,
                )
            }

            pub fn multiplicity(self, program: &TypedTrees) -> language_semantics::Multiplicity {
                if !self.reference.is_valid() {
                    return language_semantics::Multiplicity::Affine;
                }
                match program.type_reference_table.type_reference(self.reference) {
                    TypeReferenceNode::Constrained { base_type, .. } => {
                        self.child(program, *base_type).multiplicity(program)
                    }
                    TypeReferenceNode::FixedArray { element_type, .. } => {
                        self.child(program, *element_type).multiplicity(program)
                    }
                    _ => program.type_multiplicity(self.reference),
                }
            }
        }

        fn project_type_reference<'scope>(
            program: &TypedTrees,
            mut reference: TypeReferenceHandle,
            substitutions: &'scope [(SymbolHandle, TypeReferenceHandle)],
            scope_starts: &'scope [usize],
        ) -> AppliedTypeReference<'scope> {
            let mut visible = substitutions.len();
            let mut visible_scopes = scope_starts.len();
            if reference.is_valid() {
                while let TypeReferenceNode::Named { symbol, .. } =
                    program.type_reference_table.type_reference(reference)
                {
                    let Some((ordinal, (_, replacement))) = substitutions[..visible]
                        .iter()
                        .enumerate()
                        .rev()
                        .find(|(_, (parameter, _))| parameter == symbol)
                    else {
                        break;
                    };
                    visible_scopes = scope_starts
                        .partition_point(|start| *start <= ordinal)
                        .saturating_sub(1);
                    visible = scope_starts.get(visible_scopes).copied().unwrap_or(0);
                    reference = *replacement;
                }
            }
            AppliedTypeReference {
                reference,
                substitutions: &substitutions[..visible],
                scope_starts: &scope_starts[..visible_scopes],
            }
        }

        #[derive(Clone)]
        pub struct AppliedTraitScope<'program> {
            pub declaration: &'program TraitDefinition,
            pub substitutions: Vec<(SymbolHandle, TypeReferenceHandle)>,
            pub substitution_scope_starts: Vec<usize>,
            pub argument_owner: SymbolHandle,
            region_scopes: Vec<AppliedDeclarationRegionScope>,
        }

        impl<'program> AppliedTraitScope<'program> {
            pub fn from_type_reference(
                program: &'program TypedTrees,
                reference: TypeReferenceHandle,
                lexical_owner: SymbolHandle,
            ) -> Option<Self> {
                match program.type_reference_table.type_reference(reference) {
                    TypeReferenceNode::Reference { referee, .. } => {
                        Self::from_type_reference(program, *referee, lexical_owner)
                    }
                    TypeReferenceNode::Constrained { base_type, .. } => {
                        Self::from_type_reference(program, *base_type, lexical_owner)
                    }
                    TypeReferenceNode::Named { symbol, .. } => Some(Self::abstract_scope(
                        program.trait_definition_by_symbol(*symbol)?,
                    )),
                    TypeReferenceNode::Generic {
                        base_symbol,
                        arguments,
                        lifetime_arguments,
                        ..
                    } => Self::new(
                        program,
                        program.trait_definition_by_symbol(*base_symbol)?,
                        TraitApplicationArguments {
                            lexical_owner,
                            enclosing_data_applications: &[],
                            arguments: program.type_reference_handles(*arguments),
                            lifetimes: lifetime_arguments,
                        },
                    ),
                    TypeReferenceNode::DynamicTrait {
                        symbol, arguments, ..
                    } => Self::new(
                        program,
                        program.trait_definition_by_symbol(*symbol)?,
                        TraitApplicationArguments {
                            lexical_owner,
                            enclosing_data_applications: &[],
                            arguments: program.type_reference_handles(*arguments),
                            lifetimes: &[],
                        },
                    ),
                    _ => None,
                }
            }

            pub fn from_conformance_application(
                program: &'program TypedTrees,
                conformance: &super::Conformance,
                application: TraitApplicationArguments<'_>,
            ) -> Option<Self> {
                let parameters = program.conformance_type_parameters(conformance);
                if parameters.len() != application.arguments.len()
                    || conformance.lifetime_parameters.len() != application.lifetimes.len()
                {
                    return None;
                }
                let mut scope = Self::from_conformance(program, conformance)?;
                for start in &mut scope.substitution_scope_starts {
                    *start += parameters.len();
                }
                scope.substitution_scope_starts.insert(0, 0);
                scope.substitutions.splice(
                    0..0,
                    parameters
                        .iter()
                        .zip(application.arguments)
                        .map(|(parameter, actual)| (parameter.symbol, *actual)),
                );
                scope.region_scopes.insert(
                    0,
                    AppliedDeclarationRegionScope::new(
                        conformance.symbol,
                        &conformance.lifetime_parameters,
                        application.lifetimes,
                    ),
                );
                scope.argument_owner = application.lexical_owner;
                Some(scope)
            }

            pub fn from_conformance(
                program: &'program TypedTrees,
                conformance: &super::Conformance,
            ) -> Option<Self> {
                let declaration = program.trait_definition_by_symbol(conformance.trait_symbol)?;
                let lifetimes = conformance
                    .trait_lifetime_arguments
                    .iter()
                    .map(|ordinal| {
                        conformance
                            .lifetime_parameters
                            .get(usize::try_from(*ordinal).ok()?)
                            .cloned()
                    })
                    .collect::<Option<Vec<_>>>()?;
                Self::new(
                    program,
                    declaration,
                    TraitApplicationArguments {
                        lexical_owner: conformance.symbol,
                        enclosing_data_applications: &[],
                        arguments: program.type_reference_handles(conformance.arguments),
                        lifetimes: &lifetimes,
                    },
                )
            }

            pub fn from_machine_conformance(
                program: &'program TypedTrees,
                machine: &crate::machine::Machine,
                conformance: &crate::machine::TraitConformance,
            ) -> Option<Self> {
                let declaration = program.trait_definition_by_symbol(conformance.symbol)?;
                let owner = program.lifetime_declaration_owner(machine.symbol);
                let original = program.machine_by_symbol(owner)?;
                let lifetimes = conformance
                    .trait_lifetime_arguments
                    .iter()
                    .map(|ordinal| {
                        original
                            .lifetime_parameters
                            .get(usize::try_from(*ordinal).ok()?)
                            .cloned()
                    })
                    .collect::<Option<Vec<_>>>()?;
                Self::new(
                    program,
                    declaration,
                    TraitApplicationArguments {
                        lexical_owner: owner,
                        enclosing_data_applications: &[],
                        arguments: program.type_reference_handles(conformance.arguments),
                        lifetimes: &lifetimes,
                    },
                )
            }

            pub fn new(
                program: &TypedTrees,
                declaration: &'program TraitDefinition,
                application: TraitApplicationArguments<'_>,
            ) -> Option<Self> {
                let parameters = program.trait_type_parameters(declaration);
                if parameters.len() != application.arguments.len()
                    || declaration.lifetime_parameters.len() != application.lifetimes.len()
                {
                    return None;
                }
                let mut scope = Self {
                    declaration,
                    substitutions: Vec::new(),
                    substitution_scope_starts: Vec::new(),
                    argument_owner: application.lexical_owner,
                    region_scopes: Vec::new(),
                };
                for reference in application.enclosing_data_applications {
                    enclosing::append_data_application(program, *reference, &mut scope)?;
                }
                scope
                    .substitution_scope_starts
                    .push(scope.substitutions.len());
                scope.substitutions.extend(
                    parameters
                        .iter()
                        .zip(application.arguments)
                        .map(|(parameter, argument)| (parameter.symbol, *argument)),
                );
                scope.region_scopes.push(AppliedDeclarationRegionScope::new(
                    declaration.symbol,
                    &declaration.lifetime_parameters,
                    application.lifetimes,
                ));
                Some(scope)
            }

            pub fn abstract_scope(declaration: &'program TraitDefinition) -> Self {
                Self {
                    declaration,
                    substitutions: Vec::new(),
                    substitution_scope_starts: vec![0],
                    argument_owner: declaration.symbol,
                    region_scopes: vec![AppliedDeclarationRegionScope::new(
                        declaration.symbol,
                        &declaration.lifetime_parameters,
                        &[],
                    )],
                }
            }

            pub fn type_identity(
                &self,
                program: &TypedTrees,
                reference: TypeReferenceHandle,
            ) -> NormalizedTypeIdentity {
                program.scoped_type_identity(
                    TypeIdentityRequest {
                        substitutions: &self.substitutions,
                        ..TypeIdentityRequest::ordinary(reference)
                    },
                    &self.substitution_scope_starts,
                )
            }

            pub fn argument_reference(
                &self,
                program: &TypedTrees,
                reference: TypeReferenceHandle,
            ) -> TypeReferenceHandle {
                self.type_reference(program, reference).reference
            }

            pub fn type_reference(
                &self,
                program: &TypedTrees,
                reference: TypeReferenceHandle,
            ) -> AppliedTypeReference<'_> {
                project_type_reference(
                    program,
                    reference,
                    &self.substitutions,
                    &self.substitution_scope_starts,
                )
            }

            pub(crate) fn supplied_type_reference(
                &self,
                program: &TypedTrees,
                reference: TypeReferenceHandle,
                preceding_frames: usize,
            ) -> Option<AppliedTypeReference<'_>> {
                let starts = self.substitution_scope_starts.get(..preceding_frames)?;
                let end = self
                    .substitution_scope_starts
                    .get(preceding_frames)
                    .copied()
                    .unwrap_or(self.substitutions.len());
                Some(project_type_reference(
                    program,
                    reference,
                    self.substitutions.get(..end)?,
                    starts,
                ))
            }

            pub fn parameter_argument(
                &self,
                program: &TypedTrees,
                parameter: SymbolHandle,
            ) -> TypeReferenceHandle {
                let Some((ordinal, (_, reference))) = self
                    .substitutions
                    .iter()
                    .enumerate()
                    .rev()
                    .find(|(_, (binder, _))| *binder == parameter)
                else {
                    return TypeReferenceHandle::invalid();
                };
                let visible = self
                    .substitution_scope_starts
                    .iter()
                    .rev()
                    .find(|start| **start <= ordinal)
                    .copied()
                    .unwrap_or(0);
                project_type_reference(
                    program,
                    *reference,
                    &self.substitutions[..visible],
                    &self.substitution_scope_starts[..self
                        .substitution_scope_starts
                        .partition_point(|start| *start < visible)],
                )
                .reference
            }

            pub fn same_application(&self, program: &TypedTrees, other: &Self) -> bool {
                self.declaration.symbol == other.declaration.symbol
                    && self
                        .application_identity(program)
                        .zip(other.application_identity(program))
                        .is_some_and(|(left, right)| left == right)
            }

            /// Nominate the closed operation record supplied to this exact
            /// implementation binder. The retained substitution fixes source
            /// identity; this proves no refinement, law, or body property.
            pub fn named_implementation_argument(
                &self,
                program: &'program TypedTrees,
                parameter: SymbolHandle,
            ) -> Result<&'program super::Conformance, &'static str> {
                let declared = program
                    .trait_type_parameters(self.declaration)
                    .iter()
                    .find(|declared| declared.symbol == parameter)
                    .ok_or("implementation binder is not declared by this trait scope")?;
                if declared.kind != crate::data::TypeParameterKind::Implementation {
                    return Err("trait argument is not an implementation binder");
                }
                let mut bounds = self
                    .declaration
                    .conformance_bounds
                    .iter()
                    .filter(|bound| bound.binder == parameter);
                let (Some(bound), None) = (bounds.next(), bounds.next()) else {
                    return Err("implementation binder has no unique exact bound");
                };
                if !bound.arguments.is_empty() || !bound.lifetime_arguments.is_empty() {
                    return Err("applied implementation bound needs closed source preparation");
                }
                let argument = self.parameter_argument(program, parameter);
                if !program
                    .type_reference_table
                    .contains_type_reference(argument)
                {
                    return Err("implementation argument is not closed in its source scope");
                }
                let TypeReferenceNode::Named { symbol, .. } =
                    program.type_reference_table.type_reference(argument)
                else {
                    return Err("applied implementation argument needs closed source preparation");
                };
                let selected = program
                    .conformance_by_symbol(*symbol)
                    .ok_or("implementation argument does not name an exact operation record")?;
                if selected.trait_symbol != bound.trait_symbol {
                    return Err("implementation argument supplies another nominal trait");
                }
                if !selected.arguments.is_empty()
                    || !selected.trait_lifetime_arguments.is_empty()
                    || !selected.lifetime_parameters.is_empty()
                    || !program.conformance_type_parameters(selected).is_empty()
                {
                    return Err("generic implementation argument needs closed source preparation");
                }
                Ok(selected)
            }

            pub fn parameter_identity(
                &self,
                program: &TypedTrees,
                parameter: SymbolHandle,
                qualification: TypeIdentityQualification,
            ) -> NormalizedTypeIdentity {
                let Some((request, scopes)) =
                    self.parameter_identity_request(parameter, qualification)
                else {
                    return program
                        .normalized_nominal_type_identity(parameter)
                        .unwrap_or_default();
                };
                program.scoped_type_identity(request, scopes)
            }

            fn parameter_identity_request(
                &self,
                parameter: SymbolHandle,
                qualification: TypeIdentityQualification,
            ) -> Option<(TypeIdentityRequest<'_>, &[usize])> {
                let (ordinal, (_, reference)) = self
                    .substitutions
                    .iter()
                    .enumerate()
                    .rev()
                    .find(|(_, (binder, _))| *binder == parameter)?;
                let outer = self
                    .substitution_scope_starts
                    .iter()
                    .rev()
                    .find(|start| **start <= ordinal)
                    .copied()
                    .unwrap_or(0);
                Some((
                    TypeIdentityRequest {
                        substitutions: &self.substitutions[..outer],
                        qualification,
                        ..TypeIdentityRequest::ordinary(*reference)
                    },
                    &self.substitution_scope_starts[..self
                        .substitution_scope_starts
                        .partition_point(|start| *start < outer)],
                ))
            }

            pub fn declaring_scope(
                &self,
                program: &'program TypedTrees,
                declaration: SymbolHandle,
            ) -> Option<Self> {
                let mut selected: Option<Self> = None;
                let mut ambiguous = false;
                self.visit(program, &mut |scope| {
                    if scope.declaration.symbol != declaration {
                        return;
                    }
                    if let Some(known) = &selected {
                        ambiguous |= !known.same_application(program, scope);
                    } else {
                        selected = Some(scope.clone());
                    }
                });
                if ambiguous { None } else { selected }
            }

            pub fn bound_scope(
                &self,
                program: &'program TypedTrees,
                binder: SymbolHandle,
            ) -> Option<Self> {
                let mut bounds = self
                    .declaration
                    .conformance_bounds
                    .iter()
                    .filter(|bound| bound.binder == binder);
                let bound = bounds.next()?;
                if bounds.next().is_some() {
                    return None;
                }
                let nested = Self::new(
                    program,
                    program.trait_definition_by_symbol(bound.trait_symbol)?,
                    TraitApplicationArguments {
                        lexical_owner: self.argument_owner,
                        enclosing_data_applications: &[],
                        arguments: &bound.arguments,
                        lifetimes: &bound.lifetime_arguments,
                    },
                )?;
                Some(nested.preceded_by(self))
            }

            fn preceded_by(mut self, outer: &Self) -> Self {
                let mut substitutions = outer.substitutions.clone();
                self.substitution_scope_starts = outer.substitution_scope_starts.clone();
                self.substitution_scope_starts.push(substitutions.len());
                substitutions.append(&mut self.substitutions);
                self.substitutions = substitutions;
                let mut regions = outer.region_scopes.clone();
                regions.append(&mut self.region_scopes);
                self.region_scopes = regions;
                self.argument_owner = outer.argument_owner;
                self
            }

            /// A retained service needs every declared parent application.
            /// Return missing declarations or incomplete argument frames to
            /// that producer instead of silently publishing a smaller graph.
            pub fn parent_scopes(
                &self,
                program: &'program TypedTrees,
            ) -> Result<Vec<Self>, &'static str> {
                let parents = program.trait_requirements(self.declaration);
                if parents.len() != self.declaration.requires.len() {
                    return Err("applied trait lost its complete parent declaration span");
                }
                parents
                    .iter()
                    .chain(self.declaration.refines.iter())
                    .map(|parent| self.parent_scope(program, parent))
                    .collect()
            }

            fn parent_scope(
                &self,
                program: &'program TypedTrees,
                parent: &super::TraitRequirement,
            ) -> Result<Self, &'static str> {
                let declaration = program
                    .trait_definition_by_symbol(parent.symbol)
                    .ok_or("applied trait parent has no exact declaration")?;
                let scope = Self::new(
                    program,
                    declaration,
                    TraitApplicationArguments {
                        lexical_owner: self.argument_owner,
                        enclosing_data_applications: &[],
                        arguments: program.type_reference_handles(parent.arguments),
                        lifetimes: &parent.lifetime_arguments,
                    },
                )
                .ok_or("applied trait parent has no complete type and lifetime application")?;
                Ok(scope.preceded_by(self))
            }

            pub fn visit(&self, program: &'program TypedTrees, visit: &mut impl FnMut(&Self)) {
                self.visit_inherited(program, &mut Vec::new(), visit, &mut |_| {});
            }

            pub fn visit_parents_first(
                &self,
                program: &'program TypedTrees,
                visit: &mut impl FnMut(&Self),
            ) {
                self.visit_inherited(program, &mut Vec::new(), &mut |_| {}, visit);
            }

            fn visit_inherited(
                &self,
                program: &'program TypedTrees,
                active: &mut Vec<SymbolHandle>,
                visit: &mut impl FnMut(&Self),
                after: &mut impl FnMut(&Self),
            ) {
                if active.contains(&self.declaration.symbol) {
                    return;
                }
                active.push(self.declaration.symbol);
                visit(self);
                for parent in program
                    .trait_requirements(self.declaration)
                    .iter()
                    .chain(self.declaration.refines.iter())
                {
                    let Ok(scope) = self.parent_scope(program, parent) else {
                        continue;
                    };
                    scope.visit_inherited(program, active, visit, after);
                }
                after(self);
                active.pop();
            }
        }
    }

    pub(crate) use applications::RetainedTraitApplicationFrame;
    pub use applications::{
        AppliedTraitScope, AppliedTypeReference, GenericBoundRequirement, RetainedTraitScope,
        TraitApplicationArguments, TraitApplicationEnvironment,
    };

    use crate::name::Identifier;
    use crate::signature::StateSignature;
    use arena::HandleSpan;
    use symbols::SymbolHandle;

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct TraitDefinition {
        pub symbol: SymbolHandle,
        pub is_boundary: bool,
        pub is_public: bool,
        pub name: Identifier,
        pub lifetime_parameters: Vec<Identifier>,
        pub type_parameters: HandleSpan<crate::data::TypeParameter>,
        pub conformance_bounds: Vec<crate::machine::GenericConformanceBound>,
        pub requires: HandleSpan<TraitRequirement>,
        pub machines: HandleSpan<StateSignature>,
        pub refines: Option<TraitRequirement>,
        pub refinement_clauses: Vec<TraitRefinementClause>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct TraitRefinementClause {
        pub requirement: Option<Identifier>,
        pub signature: StateSignature,
        pub service_reaches: Vec<Identifier>,
        pub service_reach: TraitRefinementReach,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub enum TraitRefinementReach {
        #[default]
        Inherited,
        Concrete(language_semantics::ServiceReachRowId),
        IndependentBounded(Vec<ClauseAbstractReachRow>),
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct ClauseAbstractReachRow {
        pub requirement: Identifier,
        pub row: language_semantics::ServiceReachRowId,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct Conformance {
        pub symbol: SymbolHandle,
        pub is_public: bool,
        pub lifetime_parameters: Vec<Identifier>,
        pub type_parameters: HandleSpan<crate::data::TypeParameter>,
        pub trait_name: Identifier,
        pub trait_symbol: SymbolHandle,
        pub trait_lifetime_arguments: Vec<u32>,
        pub arguments: HandleSpan<crate::types::TypeReferenceHandle>,
        pub alias: Option<Identifier>,
        pub rows: Vec<ConformanceRow>,
        pub value_identity: ImplementationValueIdentity,
    }

    /*
        What distinguishes the value a named implementation constructs
        (conformances.md, "Resulting-value identity"): the schema application
        and the selected operations, never the constructor's name or a binder
        the schema application does not use. The default keeps the declaration
        as its own representative with every binder relevant, which is the
        answer for an implementation that owns a member: an inline or default
        body is a new machine over the whole telescope.
    */
    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct ImplementationValueIdentity {
        /* The first declaration constructing this same value; invalid means this one. */
        pub representative: SymbolHandle,
        /* One flag per binder: set when the schema application does not mention it. */
        pub unused_lifetimes: Vec<bool>,
        pub unused_parameters: Vec<bool>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct ConformanceRow {
        pub declaring_trait: SymbolHandle,
        pub declaring_trait_name: Identifier,
        pub requirement: SymbolHandle,
        pub requirement_name: Identifier,
        pub realization_machine: SymbolHandle,
        pub realization_state: SymbolHandle,
        pub realization_name: Identifier,
        pub source: ConformanceRowSource,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub enum ConformanceRowSource {
        Inline,
        Reference,
        TraitDefault,
        Synthesized,
    }

    impl ConformanceRowSource {
        pub fn owns_realization(self) -> bool {
            matches!(self, Self::Inline | Self::TraitDefault | Self::Synthesized)
        }
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct TraitRequirement {
        pub symbol: SymbolHandle,
        pub name: Identifier,
        pub lifetime_arguments: Vec<Identifier>,
        pub arguments: HandleSpan<crate::types::TypeReferenceHandle>,
        pub source_span: source::SourceSpan,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum DynamicSignatureIneligibility {
        BoundaryRequirement,
        RequirementLocalGenerics,
    }
}

/*
We represent closed callable applications (`callable_applications`), including
applied machine instances, trait and conformance scopes, domain scopes, and
requirement instances in `TypedTrees`.
*/

pub(crate) mod call_targets {
    pub(crate) mod applications {
        use super::super::CallableApplication;
        use crate::{TypedTrees, expression, name::Identifier};
        use symbols::SymbolHandle;

        impl CallableApplication {
            pub(crate) fn retain_selected_call_application(
                &self,
                program: &mut TypedTrees,
                call: &mut expression::TableCallExpression,
                target: SymbolHandle,
                name: &Identifier,
                supplied: &expression::StaticMachineArgument,
            ) -> Result<(), &'static str> {
                use crate::static_applications::{
                    StaticMachineBinding, StaticMachineBindingValue, StaticMachineLifetimeBinding,
                };
                if !self.static_argument_owner.is_valid() {
                    return Err("selected contract helper lost its supplying declaration");
                }
                let configured = supplied.application.as_deref();
                if configured.is_some_and(|application| application.member.is_valid()) {
                    return Err(
                        "selected contract helper needs its separately retained member selection",
                    );
                }
                let explicit =
                    configured.map_or(&[][..], |application| application.arguments.as_ref());
                let local_arguments = if call.authored_machine_application.is_valid()
                    && !program
                        .static_machine_application_bindings(call.authored_machine_application)
                        .is_empty()
                {
                    Self::retained_application_arguments(
                        program,
                        call.authored_machine_application,
                    )?
                } else if call.authored_machine_application.is_valid()
                    && !program
                        .static_machine_application_arguments(call.authored_machine_application)
                        .is_empty()
                {
                    program
                        .static_machine_application_arguments(call.authored_machine_application)
                        .to_vec()
                } else {
                    call.machine_arguments.to_vec()
                };
                if !explicit.is_empty() && !local_arguments.is_empty() {
                    return Err(
                        "selected contract helper has two argument lists for one declaration",
                    );
                }
                let arguments = if explicit.is_empty() {
                    local_arguments
                } else {
                    explicit.to_vec()
                };
                let (declaration, parameters, evidence, lifetime_parameters) =
                    if let Some(machine) = program
                        .machine_holding_state(target)
                        .or_else(|| program.machine_by_symbol(target))
                    {
                        (
                            machine.symbol,
                            program.machine_type_parameters(machine).to_vec(),
                            machine.conformance_bounds.clone(),
                            machine.lifetime_parameters.clone(),
                        )
                    } else if let Some((_, signature)) = program.machine_parameter_signature(target)
                    {
                        (
                            signature.symbol,
                            program.state_signature_type_parameters(signature).to_vec(),
                            signature.conformance_bounds.clone(),
                            signature.lifetime_parameters.clone(),
                        )
                    } else {
                        return Err("selected contract helper lost its parameter declaration");
                    };
                if arguments.len() != parameters.len() + evidence.len() {
                    return Err("selected contract helper needs its complete configured arguments");
                }
                let mut bindings = Vec::with_capacity(arguments.len());
                for (parameter, argument) in parameters.iter().zip(&arguments) {
                    let value = match parameter.kind {
                        crate::data::TypeParameterKind::Machine { .. } => {
                            StaticMachineBindingValue::Machine(argument.clone())
                        }
                        crate::data::TypeParameterKind::Const { .. } => {
                            StaticMachineBindingValue::Const(
                                program.static_operand_type_reference_in_scope(
                                    argument,
                                    self.static_argument_owner,
                                )?,
                            )
                        }
                        crate::data::TypeParameterKind::Value { .. } => {
                            return Err("selected contract helper has an unclosed runtime operand");
                        }
                        _ => StaticMachineBindingValue::Type(
                            program.static_operand_type_reference_in_scope(
                                argument,
                                self.static_argument_owner,
                            )?,
                        ),
                    };
                    bindings.push(StaticMachineBinding {
                        parameter: parameter.symbol,
                        value,
                    });
                }
                for (bound, argument) in evidence.iter().zip(&arguments[parameters.len()..]) {
                    bindings.push(StaticMachineBinding {
                        parameter: bound.binder,
                        value: StaticMachineBindingValue::Conformance(argument.clone()),
                    });
                }
                let explicit_lifetimes = configured.map_or(&[][..], |application| {
                    application.lifetime_arguments.as_ref()
                });
                if !explicit_lifetimes.is_empty()
                    && explicit_lifetimes.len() != lifetime_parameters.len()
                {
                    return Err("selected contract helper has an incomplete lifetime application");
                }
                let lifetimes = lifetime_parameters
                    .iter()
                    .zip(explicit_lifetimes)
                    .map(|(parameter, argument)| {
                        crate::type_identity::lifetime_parameter_identity(
                            program,
                            self.static_argument_owner,
                            argument.as_str(),
                        )
                        .ok_or("selected contract helper lifetime has no supplying declaration")?;
                        Ok(StaticMachineLifetimeBinding {
                            parameter: parameter.clone(),
                            argument: argument.clone(),
                        })
                    })
                    .collect::<Result<Vec<_>, &'static str>>()?;
                let application = program.expression_table.retain_static_machine_application(
                    target,
                    name.clone(),
                    &arguments,
                );
                let row = program
                    .expression_table
                    .static_machine_application_mut(application);
                row.lexical_owner = self.static_argument_owner;
                row.binding_declaration = declaration;
                program
                    .expression_table
                    .set_static_machine_application_bindings(application, &bindings);
                program
                    .expression_table
                    .set_static_machine_application_lifetimes(application, declaration, &lifetimes);
                call.machine_arguments = arguments.into();
                call.authored_machine_application = application;
                Ok(())
            }
        }

        impl CallableApplication {
            pub(crate) fn retain_prepared_call_application(
                &self,
                program: &mut TypedTrees,
                call: &mut expression::TableCallExpression,
                provider: crate::static_applications::StaticMachineApplicationHandle,
            ) -> Result<(), &'static str> {
                let source = program.static_machine_application(provider).clone();
                let bindings = program
                    .static_machine_application_bindings(provider)
                    .to_vec();
                if program
                    .machine_parameter_signature(source.binding_declaration)
                    .is_some()
                {
                    if !bindings.is_empty() {
                        return Err(
                            "forwarded contract helper unexpectedly contains configured parameters",
                        );
                    }
                    let supplied = expression::StaticMachineArgument {
                        symbol: source.registration_operation,
                        path: Box::from([source.target.clone()]),
                        ..Default::default()
                    };
                    return self.retain_selected_call_application(
                        program,
                        call,
                        source.registration_operation,
                        &source.target,
                        &supplied,
                    );
                }

                let lifetimes = program
                    .static_machine_application_lifetimes(provider)
                    .to_vec();
                let arguments = Self::retained_application_arguments(program, provider)?;
                if !call.machine_arguments.is_empty() {
                    return Err(
                        "determined contract helper cannot replace a universal call application",
                    );
                }
                let application = program.expression_table.retain_static_machine_application(
                    source.registration_operation,
                    source.target,
                    &arguments,
                );
                let row = program
                    .expression_table
                    .static_machine_application_mut(application);
                row.lexical_owner = source.lexical_owner;
                row.binding_declaration = source.binding_declaration;
                row.conformance_selection = source.conformance_selection;
                program
                    .expression_table
                    .set_static_machine_application_bindings(application, &bindings);
                program
                    .expression_table
                    .set_static_machine_application_lifetimes(
                        application,
                        source.lifetime_binding_declaration,
                        &lifetimes,
                    );
                call.machine_arguments = arguments.into();
                call.authored_machine_application = application;
                Ok(())
            }
        }

        impl CallableApplication {
            fn retained_application_arguments(
                program: &TypedTrees,
                application: crate::static_applications::StaticMachineApplicationHandle,
            ) -> Result<Vec<expression::StaticMachineArgument>, &'static str> {
                use crate::static_applications::StaticMachineBindingValue;
                let declaration = program
                    .static_machine_application(application)
                    .binding_declaration;
                let bindings = program.static_machine_application_bindings(application);
                let declared_parameters =
                    if let Some(machine) = program.machine_by_symbol(declaration) {
                        program
                            .machine_type_parameters(machine)
                            .iter()
                            .map(|parameter| parameter.symbol)
                            .chain(machine.conformance_bounds.iter().map(|bound| bound.binder))
                            .collect::<Vec<_>>()
                    } else if let Some((_, signature)) =
                        program.machine_parameter_signature(declaration)
                    {
                        program
                            .state_signature_type_parameters(signature)
                            .iter()
                            .map(|parameter| parameter.symbol)
                            .chain(
                                signature
                                    .conformance_bounds
                                    .iter()
                                    .map(|bound| bound.binder),
                            )
                            .collect::<Vec<_>>()
                    } else {
                        return Err("prepared contract helper lost its receiving declaration");
                    };
                let mut arguments = Vec::with_capacity(declared_parameters.len());
                for parameter in declared_parameters {
                    let binding = bindings
                        .iter()
                        .find(|binding| binding.parameter == parameter)
                        .ok_or("prepared contract helper lost one configured parameter")?;
                    arguments.push(match &binding.value {
                        StaticMachineBindingValue::Type(reference)
                        | StaticMachineBindingValue::Const(reference) => {
                            program.static_operand_from_type_reference(*reference)?
                        }
                        StaticMachineBindingValue::Machine(argument)
                        | StaticMachineBindingValue::Conformance(argument) => argument.clone(),
                        StaticMachineBindingValue::RuntimeValue { .. } => {
                            return Err("prepared contract helper has an unclosed runtime operand");
                        }
                    });
                }
                Ok(arguments)
            }
        }
    }

    use super::CallableApplication;
    use crate::{TypedTrees, expression, name::Identifier};
    use symbols::{SymbolHandle, SymbolKind};

    pub(crate) struct AppliedCallTarget {
        parameter: SymbolHandle,
        target: SymbolHandle,
        name: Identifier,
        attached_data: Option<(SymbolHandle, Identifier)>,
        supplied: Option<expression::StaticMachineArgument>,
        provider: Option<crate::static_applications::StaticMachineApplicationHandle>,
    }

    impl CallableApplication {
        pub(crate) fn selected_call_target(
            &self,
            program: &TypedTrees,
            parameter: SymbolHandle,
        ) -> Result<Option<AppliedCallTarget>, &'static str> {
            if !parameter.is_valid()
                || program.symbols.get(parameter).kind != SymbolKind::MachineParameter
            {
                return Ok(None);
            }
            let supplied = self
                .static_arguments
                .iter()
                .rev()
                .find(|(symbol, _)| *symbol == parameter)
                .map(|(_, argument)| argument.clone());
            let provider = self
                .selected_providers
                .iter()
                .find(|(binder, _)| *binder == parameter)
                .map(|(_, provider)| *provider);
            let (selected, name) = if let Some(provider) = provider {
                let record = program.static_machine_application(provider);
                (record.registration_operation, record.target.clone())
            } else if let Some(argument) = &supplied {
                let name = argument
                    .path
                    .last()
                    .cloned()
                    .ok_or("selected contract helper lost its diagnostic name")?;
                (argument.symbol, name)
            } else {
                let Some((_, reference)) = self
                    .types
                    .iter()
                    .rev()
                    .find(|(symbol, _)| *symbol == parameter)
                else {
                    return Ok(None);
                };
                let crate::types::TypeReferenceNode::Named { symbol, name } =
                    program.type_reference_table.type_reference(*reference)
                else {
                    return Err("applied contract call needs an exact named callable declaration");
                };
                (*symbol, name.clone())
            };
            if !selected.is_valid()
                || !matches!(
                    program.symbols.get(selected).kind,
                    SymbolKind::Machine | SymbolKind::State | SymbolKind::MachineParameter
                )
            {
                return Err("applied contract call lost its selected callable declaration");
            }
            if selected == parameter
                && supplied
                    .as_ref()
                    .is_none_or(|argument| argument.application.is_none())
            {
                return Ok(None);
            }
            let machine = program
                .machine_by_symbol(selected)
                .or_else(|| program.machine_holding_state(selected));
            let target = if let Some(machine) = machine
                && machine.symbol == selected
            {
                program
                    .machine_states(machine)
                    .first()
                    .ok_or("applied contract call selected a machine without its entry")?
                    .symbol
            } else {
                selected
            };
            let attached_data = match machine {
                Some(machine) if machine.attached_data_symbol.is_valid() => Some((
                    machine.attached_data_symbol,
                    machine.attached_data.clone().ok_or(
                        "applied contract call lost its attached declaration's diagnostic name",
                    )?,
                )),
                _ => None,
            };
            Ok(Some(AppliedCallTarget {
                parameter,
                target,
                name,
                attached_data,
                supplied,
                provider,
            }))
        }

        pub(crate) fn apply_call_target(
            &self,
            program: &mut TypedTrees,
            call: &mut expression::TableCallExpression,
            selected: AppliedCallTarget,
        ) -> Result<(), &'static str> {
            if !call.static_machine_parameter.is_valid() {
                call.static_machine_parameter = self.symbol(selected.parameter);
            }
            if let Some(provider) = selected.provider {
                self.retain_prepared_call_application(program, call, provider)?;
            } else if let Some(argument) = &selected.supplied {
                self.retain_selected_call_application(
                    program,
                    call,
                    selected.target,
                    &selected.name,
                    argument,
                )?;
            }
            call.target_symbol = selected.target;
            call.target = selected.name;
            if let Some((symbol, name)) = selected.attached_data {
                call.receiver =
                    program
                        .expression_table
                        .insert_tree(&expression::Expression::Name(
                            expression::NamePath::resolved_from_iter([name], symbol, symbol),
                        ));
            }
            Ok(())
        }
    }
}
pub(crate) mod closed_operands {
    use crate::{TypedTrees, types};
    use symbols::SymbolHandle;

    #[derive(Default)]
    struct ClosedOperands {
        symbols: Vec<SymbolHandle>,
        types: Vec<types::TypeReferenceHandle>,
    }

    struct OperandClosure<'program, 'scope> {
        program: &'program TypedTrees,
        operands: ClosedOperands,
        scope: Option<crate::trait_definition::AppliedTypeReference<'scope>>,
        visited_types: Vec<(
            types::TypeReferenceHandle,
            Option<crate::trait_definition::AppliedTypeReference<'scope>>,
        )>,
        visited_expressions: Vec<(
            crate::expression::ExpressionHandle,
            Option<crate::trait_definition::AppliedTypeReference<'scope>>,
        )>,
    }

    impl Extend<SymbolHandle> for OperandClosure<'_, '_> {
        fn extend<Symbols: IntoIterator<Item = SymbolHandle>>(&mut self, symbols: Symbols) {
            for symbol in symbols {
                if let Some(actual) = self
                    .scope
                    .and_then(|scope| scope.parameter_argument(self.program, symbol))
                {
                    let previous = self.scope.replace(actual);
                    crate::references::visit_type_reference(self.program, self, actual.reference);
                    self.scope = previous;
                } else {
                    self.operands.symbols.push(symbol);
                }
            }
        }
    }

    impl crate::references::TypedReferences for OperandClosure<'_, '_> {
        fn type_reference(&mut self, reference: types::TypeReferenceHandle) -> bool {
            let key = (reference, self.scope);
            if self.visited_types.contains(&key) {
                return false;
            }
            self.visited_types.push(key);
            if let Some(applied) = self.scope.map(|scope| scope.child(self.program, reference))
                && applied.reference != reference
            {
                let previous = self.scope.replace(applied);
                crate::references::visit_type_reference(self.program, self, applied.reference);
                self.scope = previous;
                return false;
            }
            self.operands.types.push(reference);
            let symbol = match self.program.type_reference_table.type_reference(reference) {
                types::TypeReferenceNode::Named { symbol, .. } => *symbol,
                types::TypeReferenceNode::Generic { base_symbol, .. } => *base_symbol,
                _ => SymbolHandle::invalid(),
            };
            if let Some(origin) = self
                .program
                .data_definition_by_symbol(symbol)
                .and_then(|definition| definition.generic_instance)
            {
                crate::references::visit_type_reference(self.program, self, origin);
            }
            true
        }
        fn expression(&mut self, expression: crate::expression::ExpressionHandle) -> bool {
            let key = (expression, self.scope);
            if self.visited_expressions.contains(&key) {
                return false;
            }
            self.visited_expressions.push(key);
            true
        }
    }

    impl TypedTrees {
        pub fn require_closed_application_operands(
            &self,
            arguments: &[types::TypeReferenceHandle],
        ) -> Result<(), &'static str> {
            self.require_application_operand_bindings(
                arguments,
                SymbolHandle::invalid(),
                false,
                None,
            )
        }

        pub fn require_declared_application_operands(
            &self,
            arguments: &[types::TypeReferenceHandle],
            lexical_owner: SymbolHandle,
        ) -> Result<(), &'static str> {
            self.require_application_operand_bindings(arguments, lexical_owner, true, None)
        }

        pub fn require_owned_application_operands(
            &self,
            arguments: &[types::TypeReferenceHandle],
            lexical_owner: SymbolHandle,
        ) -> Result<(), &'static str> {
            self.require_application_operand_bindings(arguments, lexical_owner, false, None)
        }

        pub fn require_receiving_trait_operands(
            &self,
            scope: &crate::trait_definition::AppliedTraitScope<'_>,
        ) -> Result<(), &'static str> {
            self.require_trait_operand_bindings(scope, false)
        }

        pub fn require_declared_trait_operands(
            &self,
            scope: &crate::trait_definition::AppliedTraitScope<'_>,
        ) -> Result<(), &'static str> {
            self.require_trait_operand_bindings(scope, true)
        }

        fn require_trait_operand_bindings(
            &self,
            scope: &crate::trait_definition::AppliedTraitScope<'_>,
            allow_declared_binders: bool,
        ) -> Result<(), &'static str> {
            for parameter in self.trait_type_parameters(scope.declaration) {
                let (actual, preceding_frames) = scope
                    .parameter_application(parameter.symbol)
                    .ok_or("closed requirement omits a declaring trait operand")?;
                self.require_application_operand_bindings(
                    &[actual],
                    scope.argument_owner,
                    allow_declared_binders,
                    Some((scope, preceding_frames)),
                )?;
            }
            Ok(())
        }

        fn require_application_operand_bindings(
            &self,
            arguments: &[types::TypeReferenceHandle],
            lexical_owner: SymbolHandle,
            allow_declared_binders: bool,
            scope: Option<(&crate::trait_definition::AppliedTraitScope<'_>, usize)>,
        ) -> Result<(), &'static str> {
            let mut closure = OperandClosure {
                program: self,
                operands: ClosedOperands::default(),
                scope: None,
                visited_types: Vec::new(),
                visited_expressions: Vec::new(),
            };
            for argument in arguments.iter() {
                closure.scope = scope
                    .map(|(scope, preceding_frames)| {
                        scope
                            .supplied_type_reference(self, *argument, preceding_frames)
                            .ok_or("closed requirement lost its supplying operand prefix")
                    })
                    .transpose()?;
                let supplied = closure.scope.map_or(*argument, |view| view.reference);
                crate::references::visit_type_reference(self, &mut closure, supplied);
            }
            let operands = closure.operands;
            if operands
                .symbols
                .iter()
                .filter(|symbol| symbol.is_valid())
                .any(|symbol| {
                    if allow_declared_binders
                        && self
                            .data_type_parameter_by_symbol(*symbol)
                            .is_some_and(|parameter| {
                                matches!(
                                    parameter.kind,
                                    crate::data::TypeParameterKind::Type
                                        | crate::data::TypeParameterKind::Const { .. }
                                )
                            })
                    {
                        let parent = self.symbols.get(*symbol).parent;
                        let mut owner = lexical_owner;
                        while owner.is_valid() {
                            if parent == owner {
                                return false;
                            }
                            owner = self.symbols.get(owner).parent;
                        }
                    }
                    matches!(
                        self.symbols.get(*symbol).kind,
                        symbols::SymbolKind::TypeParameter
                            | symbols::SymbolKind::MachineParameter
                            | symbols::SymbolKind::ConformanceParameter
                            | symbols::SymbolKind::PropositionParameter
                            | symbols::SymbolKind::PropositionMachineParameter
                            | symbols::SymbolKind::Local
                            | symbols::SymbolKind::Parameter
                    )
                })
            {
                return Err("open operands do not close a requirement instance");
            }
            for reference in &operands.types {
                if let types::TypeReferenceNode::Named { symbol, .. } =
                    self.type_reference_table.type_reference(*reference)
                    && let Some(definition) = self.data_definition_by_symbol(*symbol)
                    && (!definition.type_parameters.is_empty()
                        || !definition.lifetime_parameters.is_empty())
                {
                    return Err("closed requirement operand omits a nominal telescope");
                }
            }
            for argument in arguments.iter().copied() {
                let exact_regions = if let Some((scope, preceding_frames)) = scope {
                    scope
                        .with_identity_scopes(self, |frames| {
                            crate::type_identity::applied_type_identity(
                                self,
                                argument,
                                scope.argument_owner,
                                &frames[..preceding_frames],
                            )
                        })
                        .is_some()
                } else if lexical_owner.is_valid() {
                    crate::type_identity::applied_type_identity(self, argument, lexical_owner, &[])
                        .is_some()
                } else {
                    false
                };
                if exact_regions {
                    continue;
                }
                if self
                    .region_free_scoped_type_identity(
                        crate::type_identity::TypeIdentityRequest::ordinary(argument),
                        &[],
                    )
                    .is_none()
                {
                    return Err("closed requirement needs a retained exact region application");
                }
            }
            Ok(())
        }
    }
}
pub(crate) mod conformance_scopes {
    use crate::TypedTrees;
    use crate::name::Identifier;
    use crate::static_applications::StaticMachineApplicationHandle;
    use crate::trait_definition::{AppliedTraitScope, TraitApplicationArguments};
    use crate::{ClosedConformanceApplication, ClosedConformanceApplicationSite};

    impl TypedTrees {
        pub fn portable_conformance_application_identity(
            &self,
            application: &crate::ClosedConformanceApplication,
            supplying_application: crate::static_applications::StaticMachineApplicationHandle,
        ) -> Option<crate::type_identity::NormalizedTypeIdentity> {
            if !supplying_application.is_valid() {
                return None;
            }
            let source = self.static_machine_application(supplying_application);
            self.portable_conformance_application_identity_in_owner(
                application,
                source.lexical_owner,
            )
        }

        pub(crate) fn portable_conformance_application_identity_in_owner(
            &self,
            application: &crate::ClosedConformanceApplication,
            lexical_owner: symbols::SymbolHandle,
        ) -> Option<crate::type_identity::NormalizedTypeIdentity> {
            let lexical_owner = if application.trait_scope.supplying_owner().is_valid() {
                application.trait_scope.scope(self)?.argument_owner
            } else {
                lexical_owner
            };
            let selected = crate::expression::StaticMachineArgument {
                symbol: application.declaration,
                application: Some(Box::new(crate::expression::StaticSymbolApplication {
                    member: Default::default(),
                    lifetime_arguments: application
                        .lifetime_arguments
                        .iter()
                        .map(Identifier::generated)
                        .collect(),
                    arguments: application.arguments.clone(),
                })),
                ..Default::default()
            };
            crate::type_identity::portable_applied_index_identity(
                self,
                crate::type_identity::IndexIdentityArgument::StaticArgument(&selected),
                lexical_owner,
                &[],
            )
        }

        pub fn closed_conformance_applications_match(
            &self,
            left: &crate::ClosedConformanceApplication,
            right: &crate::ClosedConformanceApplication,
        ) -> bool {
            if left == right {
                return true;
            }
            if !left.same_selection_metadata(right) {
                return false;
            }
            let (Some(left_scope), Some(right_scope)) =
                (left.trait_scope.scope(self), right.trait_scope.scope(self))
            else {
                return false;
            };
            let owner = left_scope.argument_owner;
            if !owner.is_valid()
                || owner != right_scope.argument_owner
                || !left_scope.same_application(self, &right_scope)
            {
                return false;
            }
            let left_identity =
                self.portable_conformance_application_identity_in_owner(left, owner);
            left_identity.is_some()
                && left_identity
                    == self.portable_conformance_application_identity_in_owner(right, owner)
        }

        pub fn prepare_closed_conformance_scopes(&mut self) -> Result<(), String> {
            let pending = self
                .closed_conformance_application_sites()
                .into_iter()
                .map(|(site, _, supplying, application)| (site, supplying, application.clone()))
                .collect::<Vec<_>>();
            for (site, binding_application, application) in pending {
                self.prepare_closed_conformance_scope(site, binding_application, application)?;
            }
            Ok(())
        }

        pub fn prepare_closed_conformance_occurrence_scope(
            &mut self,
            application: StaticMachineApplicationHandle,
        ) -> Result<(), String> {
            if !application.is_valid() {
                return Err(
                    "closed implementation application lost its supplying source application"
                        .into(),
                );
            }
            let site = ClosedConformanceApplicationSite::Occurrence(application);
            let selected = self
                .closed_conformance_application_at_mut(site)
                .ok_or("closed implementation application lost its occurrence")?
                .clone();
            self.prepare_closed_conformance_scope(site, application, selected)
        }

        pub fn conformance_selection_region_supplier(
            &self,
            application: StaticMachineApplicationHandle,
        ) -> Result<symbols::SymbolHandle, &'static str> {
            if !application.is_valid() {
                return Err("implementation selection lost its source application");
            }
            let source = self.static_machine_application(application);
            let Some(selection) = &source.conformance_selection else {
                return Ok(source.lexical_owner);
            };
            let selected = self
                .static_machine_application_arguments(application)
                .get(selection.operand_ordinal as usize)
                .ok_or("implementation selection lost its exact operand")?;
            self.static_operand_region_supplier(selected, source.lexical_owner)
        }

        fn closed_conformance_region_supplier(
            &self,
            site: ClosedConformanceApplicationSite,
            binding_application: StaticMachineApplicationHandle,
            selected: &ClosedConformanceApplication,
        ) -> Result<symbols::SymbolHandle, &'static str> {
            use crate::static_applications::StaticMachineBindingValue;

            let ClosedConformanceApplicationSite::Invocation {
                specialization,
                application: ordinal,
            } = site
            else {
                return self.conformance_selection_region_supplier(binding_application);
            };
            let receipt = self
                .machine_specializations
                .get(specialization)
                .ok_or("implementation capture lost its specialization receipt")?;
            let source = self.static_machine_application(binding_application);
            if receipt.binding_application != binding_application
                || source.binding_declaration != receipt.template
                || receipt.conformance_applications.get(ordinal) != Some(selected)
                || receipt.conformance_arguments.get(ordinal) != Some(&selected.declaration)
            {
                return Err("implementation capture changed its retained specialization");
            }
            let parameter = self
                .machine_by_symbol(receipt.template)
                .and_then(|template| template.conformance_bounds.get(ordinal))
                .ok_or("implementation capture lost its exact declaration parameter")?;
            let mut bindings = self
                .static_machine_application_bindings(binding_application)
                .iter()
                .filter(|binding| binding.parameter == parameter.binder);
            let binding = bindings
                .next()
                .ok_or("implementation capture lost its exact supplied binding")?;
            let StaticMachineBindingValue::Conformance(operand) = &binding.value else {
                return Err("implementation capture changed its supplied operand role");
            };
            let configured = operand.application.as_deref();
            if bindings.next().is_some()
                || operand.symbol != selected.declaration
                || configured.map_or(&[][..], |application| application.arguments.as_ref())
                    != selected.arguments.as_ref()
                || !configured
                    .map_or(&[][..], |application| {
                        application.lifetime_arguments.as_ref()
                    })
                    .iter()
                    .map(|lifetime| lifetime.as_str())
                    .eq(selected.lifetime_arguments.iter().map(String::as_str))
            {
                return Err("implementation capture changed its complete selected tuple");
            }
            // The instance header owns the call. Its captured implementation
            // keeps the region supplier of the exact original operand.
            self.static_operand_region_supplier(operand, source.lexical_owner)
        }

        fn prepare_closed_conformance_scope(
            &mut self,
            site: ClosedConformanceApplicationSite,
            binding_application: StaticMachineApplicationHandle,
            mut application: ClosedConformanceApplication,
        ) -> Result<(), String> {
            if !binding_application.is_valid() {
                return Err(
                    "closed implementation application lost its supplying source application"
                        .into(),
                );
            }
            let owner = self
                .static_machine_application(binding_application)
                .lexical_owner;
            let supplier =
                self.closed_conformance_region_supplier(site, binding_application, &application)?;
            let arguments = application
                .arguments
                .iter()
                .map(|argument| self.static_operand_type_reference_in_scope(argument, supplier))
                .collect::<Result<Vec<_>, _>>()?;
            let lifetimes = application
                .lifetime_arguments
                .iter()
                .map(Identifier::generated)
                .collect::<Vec<_>>();
            let declaration = self
            .conformance_by_symbol(application.declaration)
            .ok_or_else(|| {
                let source_owner = match site {
                    ClosedConformanceApplicationSite::TraitArgument(handle) => self
                        .trait_implementation_applications
                        .get(handle)
                        .source_scope
                        .supplying_owner(),
                    _ => symbols::SymbolHandle::invalid(),
                };
                format!(
                    "closed implementation application lost its declaration: site={site:?}, declaration={:?} ({:?}), supplying_application={binding_application:?}, generated_owner={owner:?} ({:?}), retained_source_owner={source_owner:?} ({:?})",
                    application.declaration,
                    self.symbols.display_path(application.declaration, "::"),
                    self.symbols.display_path(owner, "::"),
                    self.symbols.display_path(source_owner, "::"),
                )
            })?;
            let scope = AppliedTraitScope::from_conformance_application(
                self,
                declaration,
                TraitApplicationArguments {
                    lexical_owner: supplier,
                    enclosing_data_applications: &[],
                    arguments: &arguments,
                    lifetimes: &lifetimes,
                },
            )
            .ok_or("closed implementation application lost its complete source telescope")?;
            let scope = scope.environment().retain(self)?;
            application.trait_scope = scope;
            self.closed_conformance_application_at_mut(site)
                .ok_or("closed implementation application lost its occurrence")?
                .trait_scope = scope;
            self.prepare_realization_contracts(&application, owner, scope)
                .map_err(|reason| {
                    format!(
                        "{reason}: selected={}, call_owner={}, region_supplier={}, lifetimes={:?}, captures={:?}, site={site:?}",
                        self.symbols.display_path(application.declaration, "::"),
                        self.symbols.display_path(owner, "::"),
                        self.symbols.display_path(supplier, "::"),
                        application.lifetime_arguments,
                        application
                            .arguments
                            .iter()
                            .map(crate::expression::StaticMachineArgument::display_name)
                            .collect::<Vec<_>>(),
                    )
                })?;
            Ok(())
        }
    }
}
pub(crate) mod data_cleanup_scopes {
    use super::CallableApplication;
    use crate::{TypedTrees, data, domain, types};
    use symbols::SymbolHandle;

    impl TypedTrees {
        pub fn prepare_closed_data_cleanup_application(
            &mut self,
            reference: types::TypeReferenceHandle,
        ) -> Result<bool, &'static str> {
            let types::TypeReferenceNode::Generic {
                base_symbol,
                arguments,
                ..
            } = self.type_reference_node(reference).clone()
            else {
                return Ok(false);
            };
            let Some(template) = self.unique_data_definition_by_symbol(base_symbol) else {
                return Ok(false);
            };
            let Some(selection) = template.cleanup_selection else {
                return Ok(false);
            };
            let declaration = template.symbol;
            let parameters = self.data_type_parameters(template).to_vec();
            let arguments = self
                .type_reference_table
                .type_reference_handles(arguments)
                .to_vec();
            if parameters.is_empty() || parameters.len() != arguments.len() {
                return Ok(false);
            }
            if parameters
                .iter()
                .zip(&arguments)
                .any(|(parameter, argument)| {
                    !matches!(parameter.kind, data::TypeParameterKind::Value { .. })
                        && !domain::index_argument_is_closed(self, *argument)
                })
            {
                return Ok(false);
            }
            if self
                .applied_data_cleanup_selection(declaration, reference)
                .is_some()
            {
                return Ok(false);
            }
            let substitutions = parameters
                .iter()
                .zip(&arguments)
                .map(|(parameter, argument)| (parameter.symbol, *argument))
                .collect::<Vec<_>>();
            let mut application = CallableApplication {
                owned_signature: SymbolHandle::invalid(),
                runtime_values: Vec::new(),
                region_scope: declaration,
                trait_region_application: None,
                lifetime_declaration: declaration,
                lifetimes: Vec::new(),
                elided_lifetimes: Vec::new(),
                types: substitutions,
                static_arguments: Vec::new(),
                static_argument_owner: SymbolHandle::invalid(),
                selected_providers: Vec::new(),
                symbols: Vec::new(),
                active_requirements: Vec::new(),
                source_binders: parameters
                    .iter()
                    .map(|parameter| (parameter.name.clone(), parameter.symbol))
                    .collect(),
            };
            let selection =
                self.apply_cleanup_selection(&mut application, selection, declaration)?;
            self.applied_data_cleanup_selections
                .push(domain::AppliedDataCleanupSelection {
                    declaration,
                    selection,
                });
            Ok(true)
        }
    }
}
pub(crate) mod domain_scopes {
    use super::CallableApplication;
    use crate::{TypedTrees, domain, types};
    use symbols::SymbolHandle;

    impl CallableApplication {
        pub(crate) fn domain_identity(
            program: &mut TypedTrees,
            declaration: SymbolHandle,
            arguments: &[types::TypeReferenceHandle],
            original: language_semantics::SemanticDomainId,
        ) -> Result<language_semantics::SemanticDomainId, &'static str> {
            if arguments.is_empty() || !declaration.is_valid() {
                return Ok(original);
            }
            let Some(domain) = program.domain_definition(declaration) else {
                return Ok(original);
            };
            let identity = domain::indexed_domain_instance_name(
                program,
                domain,
                domain::index_parameters(program, domain),
                arguments,
            )
            .map_err(|_| "applied domain arguments have no canonical instance identity")?;
            Ok(program.semantic_domains.intern(&identity))
        }
    }

    impl TypedTrees {
        pub fn prepare_closed_domain_application(
            &mut self,
            declaration: SymbolHandle,
            arguments: &[types::TypeReferenceHandle],
            closed_arguments: &[types::TypeReferenceHandle],
            identity: language_semantics::SemanticDomainId,
        ) -> Result<bool, &'static str> {
            if self.applied_domain_applications.iter().any(|application| {
                application.declaration == declaration && application.theory.semantic_id == identity
            }) {
                return Ok(false);
            }
            let mut theory = self
                .domain_definition(declaration)
                .ok_or("applied domain lost its declaration")?
                .clone();
            if domain::has_generic_carrier(self, &theory) {
                return Err("applied domain requires its exact generic carrier coordinate");
            }
            let parameters = self.domain_type_parameters(&theory).to_vec();
            if parameters.len() != arguments.len() || parameters.len() != closed_arguments.len() {
                return Err("applied domain has an incomplete explicit index telescope");
            }
            let substitutions = parameters
                .iter()
                .zip(closed_arguments)
                .map(|(parameter, argument)| (parameter.symbol, *argument))
                .collect::<Vec<_>>();
            let source_subject = theory.subject_symbol;
            let subject = if source_subject.is_valid() {
                self.symbols.insert_generated_root_from(
                    source_subject,
                    self.symbols.get(source_subject).kind,
                    theory.subject_name.as_str(),
                )
            } else {
                source_subject
            };
            let mut application = CallableApplication {
                owned_signature: SymbolHandle::invalid(),
                runtime_values: Vec::new(),
                region_scope: declaration,
                trait_region_application: None,
                lifetime_declaration: declaration,
                lifetimes: Vec::new(),
                elided_lifetimes: Vec::new(),
                types: substitutions,
                static_arguments: Vec::new(),
                static_argument_owner: SymbolHandle::invalid(),
                selected_providers: Vec::new(),
                symbols: source_subject
                    .is_valid()
                    .then_some((source_subject, subject))
                    .into_iter()
                    .collect(),
                active_requirements: Vec::new(),
                source_binders: parameters
                    .iter()
                    .map(|parameter| (parameter.name.clone(), parameter.symbol))
                    .collect(),
            };
            theory.target_type = application.type_reference(self, theory.target_type)?;
            theory.subject_symbol = subject;
            theory.facts = application.facts(self, theory.facts)?;
            theory.type_parameters = Default::default();
            theory.index_arguments.clear();
            theory.semantic_id = identity;
            theory.semantic_roles.denotation_dimension =
                theory.semantic_roles.denotation_dimension.map(|_| identity);
            theory.semantic_roles.arithmetic_policy =
                theory.semantic_roles.arithmetic_policy.map(|_| identity);
            if let Some(selection) = theory.cleanup_selection {
                theory.cleanup_selection =
                    Some(self.apply_cleanup_selection(&mut application, selection, declaration)?);
            }
            self.applied_domain_applications
                .push(domain::AppliedDomainApplication {
                    declaration,
                    arguments: arguments.to_vec(),
                    theory,
                });
            Ok(true)
        }

        pub(crate) fn apply_cleanup_selection(
            &mut self,
            application: &mut CallableApplication,
            selection: domain::CleanupSelection,
            lexical_owner: SymbolHandle,
        ) -> Result<domain::CleanupSelection, &'static str> {
            let original = self
                .static_machine_application(selection.application)
                .clone();
            let mut arguments = self
                .static_machine_application_arguments(selection.application)
                .to_vec();
            application.static_arguments(self, &mut arguments)?;
            let selected = self.expression_table.retain_static_machine_application(
                original.registration_operation,
                original.target,
                &arguments,
            );
            let lifetimes = self
                .static_machine_application_lifetimes(selection.application)
                .to_vec();
            self.expression_table
                .set_static_machine_application_lifetimes(
                    selected,
                    original.lifetime_binding_declaration,
                    &lifetimes,
                );
            let row = self
                .expression_table
                .static_machine_application_mut(selected);
            row.lexical_owner = lexical_owner;
            row.binding_declaration = original.binding_declaration;
            Ok(domain::CleanupSelection {
                application: selected,
                authorization: selection.authorization,
                subject: application.type_reference(self, selection.subject)?,
                executable: SymbolHandle::invalid(),
            })
        }
    }
    #[cfg(test)]
    pub(crate) mod tests {
        use super::*;
        use crate::data::{TypeParameter, TypeParameterKind};
        use crate::name::Identifier;
        use crate::types::{DomainConstraint, TypeConstraintNode, TypeReferenceNode};
        use symbols::{SymbolKind, SymbolNameRef, SymbolTableBuilder};

        #[test]
        fn applied_domain_carrier_matches_its_exact_argument_telescope() {
            let mut symbols = SymbolTableBuilder::new();
            let pair = symbols.insert_root(SymbolKind::Data, SymbolNameRef::Static("Pair"));
            let other = symbols.insert_root(SymbolKind::Data, SymbolNameRef::Static("Other"));
            let first = symbols.insert_root(SymbolKind::TypeParameter, SymbolNameRef::Static("T"));
            let second = symbols.insert_root(SymbolKind::TypeParameter, SymbolNameRef::Static("U"));
            let mut program = TypedTrees {
                symbols: symbols.finish(),
                ..Default::default()
            };
            let operands = [first, second].map(|symbol| {
                program
                    .type_reference_table
                    .insert(TypeReferenceNode::Named {
                        symbol,
                        name: Identifier::generated("parameter"),
                    })
            });
            let parameters = program
                .data_type_parameters
                .insert_many([first, second].map(|symbol| TypeParameter {
                    symbol,
                    name: Identifier::generated("parameter"),
                    kind: TypeParameterKind::Type,
                    ..Default::default()
                }));
            let application =
                |program: &mut TypedTrees,
                 base_symbol,
                 arguments: &[types::TypeReferenceHandle]| {
                    let arguments = program
                        .type_reference_table
                        .insert_type_reference_handles(arguments.iter().copied());
                    program
                        .type_reference_table
                        .insert(TypeReferenceNode::Generic {
                            base_symbol,
                            base_name: Identifier::generated("Pair"),
                            lifetime_arguments: Vec::new(),
                            arguments,
                        })
                };
            let target = application(&mut program, pair, &operands);
            let reversed = application(&mut program, pair, &[operands[1], operands[0]]);
            let unrelated = application(&mut program, other, &operands);
            let domain = domain::DomainDefinition {
                type_parameters: parameters,
                target_type: target,
                ..Default::default()
            };
            assert!(domain::domain_accepts_carrier(
                &program, &domain, target, &operands
            ));
            assert!(domain::domain_accepts_carrier(
                &program,
                &domain,
                reversed,
                &[operands[1], operands[0]]
            ));
            assert!(!domain::domain_accepts_carrier(
                &program, &domain, reversed, &operands
            ));
            assert!(!domain::domain_accepts_carrier(
                &program, &domain, unrelated, &operands
            ));
        }

        #[test]
        fn applied_domain_constraint_keeps_arguments_identity_and_roles_together() {
            let mut symbols = SymbolTableBuilder::new();
            let domain_symbol =
                symbols.insert_root(SymbolKind::Domain, SymbolNameRef::Static("Marked"));
            let parameter =
                symbols.insert_root(SymbolKind::TypeParameter, SymbolNameRef::Static("T"));
            let mut program = TypedTrees {
                symbols: symbols.finish(),
                ..Default::default()
            };
            let carrier = program.type_reference_table.insert(TypeReferenceNode::Unit);
            let operand = program
                .type_reference_table
                .insert(TypeReferenceNode::Named {
                    symbol: parameter,
                    name: Identifier::generated("T"),
                });
            let parameters = program.data_type_parameters.insert_many([TypeParameter {
                symbol: parameter,
                name: Identifier::generated("T"),
                kind: TypeParameterKind::Type,
                ..Default::default()
            }]);
            let declaration_id = program.semantic_domains.intern("Marked");
            program.push_domain_definition(domain::DomainDefinition {
                symbol: domain_symbol,
                name: Identifier::generated("Marked"),
                type_parameters: parameters,
                target_type: carrier,
                semantic_id: declaration_id,
                ..Default::default()
            });
            let original = CallableApplication::domain_identity(
                &mut program,
                domain_symbol,
                &[operand],
                declaration_id,
            )
            .unwrap();
            let constraints =
                program
                    .type_reference_table
                    .insert_constraints([TypeConstraintNode::Domain(DomainConstraint {
                        symbol: domain_symbol,
                        arguments: vec![operand],
                        semantic_id: original,
                        semantic_roles: language_semantics::DomainSemanticRoles {
                            denotation_dimension: Some(original),
                            arithmetic_policy: Some(original),
                        },
                        ..Default::default()
                    })]);
            let source = program
                .type_reference_table
                .insert(TypeReferenceNode::Constrained {
                    base_type: carrier,
                    constraints,
                });
            let applied = program
                .apply_callable_type(source, &[(parameter, carrier)], &[], parameter)
                .unwrap();
            let TypeReferenceNode::Constrained {
                constraints: applied_constraints,
                ..
            } = program.type_reference_node(applied)
            else {
                panic!("lost domain constraint");
            };
            let TypeConstraintNode::Domain(applied) = &program
                .type_reference_table
                .constraints(*applied_constraints)[0]
            else {
                panic!("lost domain instance");
            };
            let applied = applied.clone();
            let expected = CallableApplication::domain_identity(
                &mut program,
                domain_symbol,
                &[carrier],
                declaration_id,
            )
            .unwrap();
            assert_ne!(original, expected);
            assert_eq!(applied.arguments, [carrier]);
            assert_eq!(applied.semantic_id, expected);
            assert_eq!(applied.semantic_roles.denotation_dimension, Some(expected));
            assert_eq!(applied.semantic_roles.arithmetic_policy, Some(expected));
            let TypeConstraintNode::Domain(source) =
                &program.type_reference_table.constraints(constraints)[0]
            else {
                panic!("source declaration changed kind");
            };
            assert_eq!(source.arguments, [operand]);
            assert_eq!(
                source.semantic_id, original,
                "application must not mutate its source contract"
            );
        }
    }
}
pub(crate) mod machine_instances {
    use crate::{
        TypedTrees,
        static_applications::{StaticMachineBinding, StaticMachineLifetimeBinding},
        typed_trees::{CapturedMachineApplication, MachineSpecialization},
    };
    use symbols::SymbolHandle;

    #[derive(Clone, Copy)]
    pub enum MachineInstantiation<'program> {
        Specialization(&'program MachineSpecialization),
        Capture(&'program CapturedMachineApplication),
    }

    pub struct EnclosingInvocationFrame<'program> {
        pub declaration: SymbolHandle,
        pub argument_owner: SymbolHandle,
        pub bindings: &'program [StaticMachineBinding],
        pub lifetimes: Vec<StaticMachineLifetimeBinding>,
    }
    impl MachineInstantiation<'_> {
        pub fn template(self) -> SymbolHandle {
            match self {
                Self::Specialization(receipt) => receipt.template,
                Self::Capture(receipt) => receipt.template,
            }
        }
        pub fn instance(self) -> SymbolHandle {
            match self {
                Self::Specialization(receipt) => receipt.instance,
                Self::Capture(receipt) => receipt.instance,
            }
        }
    }
    impl TypedTrees {
        /// Read enclosing applications from their exact instance receipts, from
        /// the immediate caller out to the original supplying declaration.
        pub fn enclosing_invocation_frames(
            &self,
            mut owner: SymbolHandle,
        ) -> Result<Vec<EnclosingInvocationFrame<'_>>, &'static str> {
            let mut frames = Vec::new();
            let mut visited = Vec::new();
            while owner.is_valid() {
                if visited.contains(&owner) {
                    return Err("enclosing invocation applications contain a cycle");
                }
                visited.push(owner);
                let Some(receipt) = self.machine_instantiation(owner)? else {
                    break;
                };
                let (application, declaration, argument_owner, lifetimes) = match receipt {
                    MachineInstantiation::Specialization(receipt) => {
                        let application = receipt.binding_application;
                        if !application.is_valid() {
                            return Err("enclosing specialization lost its binding application");
                        }
                        let source = self.static_machine_application(application);
                        (
                            application,
                            receipt.template,
                            source.lexical_owner,
                            self.static_application_lifetimes_for_declaration(
                                application,
                                receipt.template,
                            )?
                            .into_owned(),
                        )
                    }
                    MachineInstantiation::Capture(receipt) => {
                        let application = receipt.binding_application;
                        let selected = &receipt.conformance_application;
                        let scope = selected
                            .trait_scope
                            .scope(self)
                            .ok_or("enclosing capture lost its supplying scope")?;
                        let declaration = self
                            .conformance_by_symbol(selected.declaration)
                            .ok_or("enclosing capture lost its implementation declaration")?;
                        if !application.is_valid()
                            || scope.declaration.symbol != selected.trait_definition
                            || declaration.lifetime_parameters.len()
                                != selected.lifetime_arguments.len()
                        {
                            return Err("enclosing capture changed its complete application");
                        }
                        let lifetimes = declaration
                            .lifetime_parameters
                            .iter()
                            .zip(&selected.lifetime_arguments)
                            .map(|(parameter, argument)| StaticMachineLifetimeBinding {
                                parameter: parameter.clone(),
                                argument: crate::name::Identifier::generated(argument),
                            })
                            .collect();
                        (
                            application,
                            selected.declaration,
                            scope.argument_owner,
                            lifetimes,
                        )
                    }
                };
                let source = self.static_machine_application(application);
                if source.binding_declaration != declaration || !argument_owner.is_valid() {
                    return Err("enclosing invocation changed its declaration or supplier");
                }
                frames.push(EnclosingInvocationFrame {
                    declaration,
                    argument_owner,
                    bindings: self.static_machine_application_bindings(application),
                    lifetimes,
                });
                // Inferred instances without an authored outer application
                // retain themselves as the terminal supplying owner.
                if source.lexical_owner == owner {
                    break;
                }
                owner = source.lexical_owner;
            }
            Ok(frames)
        }

        pub fn machine_instantiation(
            &self,
            instance: SymbolHandle,
        ) -> Result<Option<MachineInstantiation<'_>>, &'static str> {
            let mut receipts = self
                .machine_specializations
                .iter()
                .filter(|receipt| receipt.instance == instance)
                .map(MachineInstantiation::Specialization)
                .chain(
                    self.captured_machine_applications
                        .iter()
                        .filter(|(_, receipt)| receipt.instance == instance)
                        .map(|(_, receipt)| MachineInstantiation::Capture(receipt)),
                );
            let selected = receipts.next();
            if receipts.next().is_some() {
                return Err("callable has multiple capture or specialization receipts");
            }
            Ok(selected)
        }

        pub fn machine_instantiation_path(
            &self,
            instance: SymbolHandle,
            template: SymbolHandle,
        ) -> Option<Vec<MachineInstantiation<'_>>> {
            let mut current = instance;
            let mut visited = Vec::new();
            let mut path = Vec::new();
            while current != template {
                if !current.is_valid() || visited.contains(&current) {
                    return None;
                }
                visited.push(current);
                let receipt = self.machine_instantiation(current).ok()??;
                current = receipt.template();
                path.push(receipt);
            }
            Some(path)
        }

        pub fn implementation_capture(
            &self,
            instance: SymbolHandle,
            template: SymbolHandle,
            implementation: SymbolHandle,
        ) -> Option<&CapturedMachineApplication> {
            let path = self.machine_instantiation_path(instance, template)?;
            let mut captures = path.into_iter().filter_map(|receipt| match receipt {
                MachineInstantiation::Capture(capture)
                    if capture.conformance_application.declaration == implementation =>
                {
                    Some(capture)
                }
                _ => None,
            });
            let selected = captures.next()?;
            captures.next().is_none().then_some(selected)
        }

        pub fn instantiated_machine_state_matches(
            &self,
            instance: SymbolHandle,
            template: SymbolHandle,
            source_state: SymbolHandle,
            instance_state: SymbolHandle,
        ) -> bool {
            let Some(path) = self.machine_instantiation_path(instance, template) else {
                return false;
            };
            let mut current = instance_state;
            for receipt in path {
                let Some(source) = self.machine(receipt.template()) else {
                    return false;
                };
                let Some(realized) = self.machine(receipt.instance()) else {
                    return false;
                };
                let source_states = self.machine_states(source);
                let instance_states = self.machine_states(realized);
                if source_states.len() != instance_states.len() {
                    return false;
                }
                current = match receipt {
                    MachineInstantiation::Specialization(_) => {
                        let mut selected = instance_states
                            .iter()
                            .enumerate()
                            .filter(|(_, state)| state.symbol == current);
                        let Some((ordinal, _)) = selected.next() else {
                            return false;
                        };
                        if selected.next().is_some() {
                            return false;
                        }
                        source_states[ordinal].symbol
                    }
                    MachineInstantiation::Capture(capture) => {
                        let states = self.captured_machine_states.span_or_empty(capture.states);
                        if states.len() != source_states.len() {
                            return false;
                        }
                        let mut selected = states.iter().filter(|state| state.instance == current);
                        let Some(mapping) = selected.next() else {
                            return false;
                        };
                        if selected.next().is_some()
                            || !source_states
                                .iter()
                                .any(|state| state.symbol == mapping.template)
                        {
                            return false;
                        }
                        mapping.template
                    }
                };
            }
            current == source_state
        }
    }

    impl TypedTrees {
        pub fn static_machine_binding_selects_target(
            &self,
            mut owner: SymbolHandle,
            binder: SymbolHandle,
            target: SymbolHandle,
        ) -> Result<bool, &'static str> {
            if !binder.is_valid() || self.retained_static_machine_contract(binder).is_none() {
                return Ok(false);
            }
            let entry = |symbol| {
                if self.symbols.get(symbol).kind == symbols::SymbolKind::Machine {
                    self.machine_by_symbol(symbol)
                        .and_then(|machine| self.machine_states(machine).first())
                        .map(|state| state.symbol)
                } else {
                    (self.symbols.get(symbol).kind == symbols::SymbolKind::State).then_some(symbol)
                }
            };
            let mut visited = Vec::new();
            loop {
                if visited.contains(&owner) {
                    return Err("static call has cyclic invocation ownership");
                }
                visited.push(owner);
                let mut instances = self
                    .machine_specializations
                    .iter()
                    .filter(|instance| instance.instance == owner);
                let Some(instance) = instances.next() else {
                    return Ok(false);
                };
                if instances.next().is_some() {
                    return Err("static call has ambiguous invocation ownership");
                }
                if let Some(crate::static_applications::StaticMachineBindingValue::Machine(
                    argument,
                )) = self.specialization_binding(instance, binder)
                {
                    let declared = self
                        .data_type_parameters
                        .span_or_empty(instance.template_parameters)
                        .iter()
                        .any(|parameter| {
                            parameter.symbol == binder
                                && matches!(
                                    parameter.kind,
                                    crate::data::TypeParameterKind::Machine { .. }
                                )
                        });
                    return Ok(declared
                        && argument.application.is_none()
                        && entry(argument.symbol)
                            .is_some_and(|selected| entry(target) == Some(selected)));
                }
                owner = instance.template;
            }
        }
    }
}
pub(crate) mod machine_selections {
    pub(crate) mod queries {
        use super::super::{AppliedCallableOwner, AppliedMachineCallable};
        use crate::type_identity::{
            IndexIdentityArgument, applied_index_identity, applied_type_identity,
        };
        use crate::{
            TypedTrees, signature,
            static_applications::{
                StaticMachineApplicationHandle, StaticMachineBinding, StaticMachineBindingValue,
            },
            types::TypeReferenceHandle,
        };
        use symbols::SymbolHandle;

        impl TypedTrees {
            pub(crate) fn same_selected_provider(
                &self,
                left: StaticMachineApplicationHandle,
                right: StaticMachineApplicationHandle,
            ) -> bool {
                let a = self.static_machine_application(left);
                let b = self.static_machine_application(right);
                a.registration_operation == b.registration_operation
                    && a.binding_declaration == b.binding_declaration
                    && a.lexical_owner == b.lexical_owner
                    && a.lifetime_binding_declaration == b.lifetime_binding_declaration
                    && a.conformance_selection == b.conformance_selection
                    && self.static_machine_application_lifetimes(left)
                        == self.static_machine_application_lifetimes(right)
                    && self.same_selected_bindings(
                        self.static_machine_application_bindings(left),
                        self.static_machine_application_bindings(right),
                        a.lexical_owner,
                    )
            }

            fn same_selected_bindings(
                &self,
                left: &[StaticMachineBinding],
                right: &[StaticMachineBinding],
                owner: SymbolHandle,
            ) -> bool {
                left.len() == right.len()
                    && left.iter().zip(right).all(|(previous, current)| {
                        previous.parameter == current.parameter
                            && match (&previous.value, &current.value) {
                                (
                                    StaticMachineBindingValue::Type(a),
                                    StaticMachineBindingValue::Type(b),
                                ) => self.same_callable_operand(*a, *b, owner),
                                (
                                    StaticMachineBindingValue::Const(a),
                                    StaticMachineBindingValue::Const(b),
                                ) => self.same_callable_index(*a, *b, owner),
                                (
                                    StaticMachineBindingValue::Machine(a),
                                    StaticMachineBindingValue::Machine(b),
                                )
                                | (
                                    StaticMachineBindingValue::Conformance(a),
                                    StaticMachineBindingValue::Conformance(b),
                                ) => {
                                    a == b
                                        || applied_index_identity(
                                            self,
                                            IndexIdentityArgument::StaticArgument(a),
                                            owner,
                                            &[],
                                        )
                                        .zip(applied_index_identity(
                                            self,
                                            IndexIdentityArgument::StaticArgument(b),
                                            owner,
                                            &[],
                                        ))
                                        .is_some_and(|(a, b)| a == b)
                                }
                                _ => previous.value == current.value,
                            }
                    })
            }

            pub(crate) fn machine_selection_lifetimes_are_current(
                &self,
                owner: &AppliedCallableOwner,
            ) -> bool {
                let AppliedCallableOwner::MachineSelection {
                    invocation,
                    provider,
                    caller_lifetimes,
                    caller_bindings,
                    caller_lifetime_declaration,
                    caller_lifetime_owner,
                    provider_lifetime_declaration,
                    provider_lifetime_owner,
                    provider_lifetimes,
                    ..
                } = owner
                else {
                    return false;
                };
                let caller = self.static_machine_application(*invocation);
                let selected = self.static_machine_application(*provider);
                let saved = self
                    .expression_table
                    .static_machine_binding_snapshot(*caller_bindings);
                let current = self.static_machine_application_bindings(*invocation);
                self.same_selected_bindings(saved, current, caller.lexical_owner)
                    && *caller_lifetime_declaration == caller.lifetime_binding_declaration
                    && *caller_lifetime_owner == caller.lexical_owner
                    && *provider_lifetime_declaration == selected.lifetime_binding_declaration
                    && *provider_lifetime_owner == selected.lexical_owner
                    && self
                        .callable_lifetime_substitutions
                        .span_or_empty(*caller_lifetimes)
                        == self.static_machine_application_lifetimes(*invocation)
                    && self
                        .callable_lifetime_substitutions
                        .span_or_empty(*provider_lifetimes)
                        == self.static_machine_application_lifetimes(*provider)
            }

            pub fn has_applied_machine_selection(
                &self,
                invocation: StaticMachineApplicationHandle,
                ordinal: usize,
                selected: SymbolHandle,
            ) -> bool {
                self.applied_callable_signatures
                    .machine_rows(invocation)
                    .any(|(_, prepared)| {
                        matches!(&prepared.owner, AppliedCallableOwner::MachineSelection {
                invocation: source, ordinal: position, selected: entry, ..
            } if *source == invocation && *position == ordinal && *entry == selected)
                    })
            }

            pub fn applied_machine_callable(
                &self,
                invocation: StaticMachineApplicationHandle,
                ordinal: usize,
                selected: SymbolHandle,
                demand: SymbolHandle,
                expected: &signature::StateSignature,
            ) -> Option<AppliedMachineCallable<'_>> {
                self.applied_callable_signatures
                    .machine_rows(invocation)
                    .find_map(|(handle, _)| {
                        let view = self.applied_machine_callable_at(
                            super::super::AppliedMachineCallableId(handle),
                        )?;
                        (view.ordinal == ordinal
                            && view.selected_entry == selected
                            && view.demand == demand
                            && view.source_required == expected)
                            .then_some(view)
                    })
            }

            pub fn applied_machine_callable_at(
                &self,
                identity: super::super::AppliedMachineCallableId,
            ) -> Option<AppliedMachineCallable<'_>> {
                let prepared = self.applied_callable_signatures.get(identity.0);
                let AppliedCallableOwner::MachineSelection {
                    invocation,
                    ordinal,
                    demand,
                    selected,
                    provider,
                    required,
                    source_required,
                    caller_operands,
                    caller_lifetimes,
                    ..
                } = &prepared.owner
                else {
                    return None;
                };
                Some(AppliedMachineCallable {
                    identity,
                    invocation: *invocation,
                    ordinal: *ordinal,
                    demand: *demand,
                    lexical_owner: self.static_machine_application(*provider).lexical_owner,
                    provider: *provider,
                    selected_entry: *selected,
                    required: required.as_ref()?,
                    selected: &prepared.signature,
                    source_required,
                    caller_operands: self.callable_substitutions.span_or_empty(*caller_operands),
                    caller_lifetimes: self
                        .callable_lifetime_substitutions
                        .span_or_empty(*caller_lifetimes),
                })
            }

            pub fn applied_machine_callable_matches(
                &self,
                invocation: StaticMachineApplicationHandle,
                ordinal: usize,
                selected: SymbolHandle,
                demand: SymbolHandle,
                provider_bindings: &[StaticMachineBinding],
                provider_lifetimes: &[crate::static_applications::StaticMachineLifetimeBinding],
                required: &signature::StateSignature,
                caller_substitutions: &[(SymbolHandle, TypeReferenceHandle)],
                caller_lifetime_bindings: &[crate::static_applications::StaticMachineLifetimeBinding],
            ) -> bool {
                self.applied_machine_callable_mismatch(
                    invocation,
                    ordinal,
                    selected,
                    demand,
                    provider_bindings,
                    provider_lifetimes,
                    required,
                    caller_substitutions,
                    caller_lifetime_bindings,
                )
                .is_none()
            }

            pub fn applied_machine_callable_mismatch(
                &self,
                invocation: StaticMachineApplicationHandle,
                ordinal: usize,
                selected: SymbolHandle,
                demand: SymbolHandle,
                provider_bindings: &[StaticMachineBinding],
                provider_lifetimes: &[crate::static_applications::StaticMachineLifetimeBinding],
                required: &signature::StateSignature,
                caller_substitutions: &[(SymbolHandle, TypeReferenceHandle)],
                caller_lifetime_bindings: &[crate::static_applications::StaticMachineLifetimeBinding],
            ) -> Option<&'static str> {
                let mut mismatch = "no checking view names this exact function and requirement";
                let matches = self
                    .applied_callable_signatures
                    .machine_rows(invocation)
                    .any(|(_, prepared)| {
                        let AppliedCallableOwner::MachineSelection {
                            invocation: source,
                            ordinal: position,
                            demand: prepared_demand,
                            selected: entry,
                            provider,
                            source_required,
                            caller_operands,
                            caller_lifetimes,
                            caller_lifetime_declaration,
                            caller_lifetime_owner,
                            provider_lifetime_declaration,
                            provider_lifetime_owner,
                            provider_lifetimes: prepared_provider_lifetimes,
                            ..
                        } = &prepared.owner
                        else {
                            return false;
                        };
                        let operands = self.callable_substitutions.span_or_empty(*caller_operands);
                        let owner = self.static_machine_application(invocation).lexical_owner;
                        let previous_bindings = self.static_machine_application_bindings(*provider);
                        if *source != invocation
                            || *position != ordinal
                            || *entry != selected
                            || *prepared_demand != demand
                            || source_required != required
                        {
                            return false;
                        }
                        let checks = [
                            (
                                self.machine_selection_lifetimes_are_current(&prepared.owner),
                                "retained caller arguments or lifetime bindings changed",
                            ),
                            (*source == invocation, "source invocation changed"),
                            (*position == ordinal, "machine argument position changed"),
                            (*entry == selected, "selected function changed"),
                            (
                                *prepared_demand == demand,
                                "required machine binder changed",
                            ),
                            (
                                source_required == required,
                                "source requirement signature changed",
                            ),
                            (
                                *caller_lifetime_declaration
                                    == self
                                        .static_machine_application(invocation)
                                        .lifetime_binding_declaration,
                                "caller lifetime declaration changed",
                            ),
                            (
                                *caller_lifetime_owner == owner,
                                "caller lexical owner changed",
                            ),
                            (
                                *provider_lifetime_declaration
                                    == self
                                        .static_machine_application(*provider)
                                        .lifetime_binding_declaration,
                                "provider lifetime declaration changed",
                            ),
                            (
                                *provider_lifetime_owner
                                    == self.static_machine_application(*provider).lexical_owner,
                                "provider lexical owner changed",
                            ),
                            (
                                self.callable_lifetime_substitutions
                                    .span_or_empty(*prepared_provider_lifetimes)
                                    == provider_lifetimes,
                                "provider lifetime arguments changed",
                            ),
                            (
                                self.callable_lifetime_substitutions
                                    .span_or_empty(*caller_lifetimes)
                                    == caller_lifetime_bindings,
                                "caller lifetime arguments changed",
                            ),
                            (
                                self.static_machine_application(*provider).lexical_owner
                                    == self.static_machine_application(invocation).lexical_owner,
                                "provider and caller have different lexical owners",
                            ),
                            (
                                self.static_machine_application_lifetimes(*provider)
                                    == provider_lifetimes,
                                "provider lifetime arguments changed",
                            ),
                            (
                                previous_bindings.len() == provider_bindings.len(),
                                "provider argument count changed",
                            ),
                            (
                                previous_bindings.iter().zip(provider_bindings).all(
                                    |(previous, current)| {
                                        previous.parameter == current.parameter
                                            && match (&previous.value, &current.value) {
                                                (
                                                    StaticMachineBindingValue::Type(left),
                                                    StaticMachineBindingValue::Type(right),
                                                ) => {
                                                    self.same_callable_operand(*left, *right, owner)
                                                }
                                                (
                                                    StaticMachineBindingValue::Const(left),
                                                    StaticMachineBindingValue::Const(right),
                                                ) => self.same_callable_index(*left, *right, owner),
                                                _ => previous.value == current.value,
                                            }
                                    },
                                ),
                                "provider arguments changed",
                            ),
                            (
                                operands.len() == caller_substitutions.len(),
                                "caller argument count changed",
                            ),
                            (
                                operands.iter().zip(caller_substitutions).all(
                                    |(operand, (parameter, actual))| {
                                        operand.parameter == *parameter
                                            && self.same_caller_operand(
                                                *parameter,
                                                operand.actual,
                                                *actual,
                                                owner,
                                            )
                                    },
                                ),
                                "caller arguments changed",
                            ),
                        ];
                        if let Some((_, reason)) = checks.iter().find(|(matches, _)| !matches) {
                            mismatch = reason;
                            false
                        } else {
                            true
                        }
                    });
                (!matches).then_some(mismatch)
            }

            fn same_caller_operand(
                &self,
                parameter: SymbolHandle,
                left: TypeReferenceHandle,
                right: TypeReferenceHandle,
                owner: SymbolHandle,
            ) -> bool {
                let Some(parameter) = self.data_type_parameter_by_symbol(parameter) else {
                    return false;
                };
                match parameter.kind {
                    crate::data::TypeParameterKind::Type => {
                        self.same_callable_operand(left, right, owner)
                    }
                    crate::data::TypeParameterKind::Const { .. }
                    | crate::data::TypeParameterKind::Value { .. } => {
                        self.same_callable_index(left, right, owner)
                    }
                    _ => false,
                }
            }

            fn same_callable_index(
                &self,
                left: TypeReferenceHandle,
                right: TypeReferenceHandle,
                owner: SymbolHandle,
            ) -> bool {
                left == right
                    || applied_index_identity(
                        self,
                        IndexIdentityArgument::TypeReference(left),
                        owner,
                        &[],
                    )
                    .zip(applied_index_identity(
                        self,
                        IndexIdentityArgument::TypeReference(right),
                        owner,
                        &[],
                    ))
                    .is_some_and(|(left, right)| left == right)
            }

            fn same_callable_operand(
                &self,
                left: TypeReferenceHandle,
                right: TypeReferenceHandle,
                owner: SymbolHandle,
            ) -> bool {
                left == right
                    || applied_type_identity(self, left, owner, &[])
                        .zip(applied_type_identity(self, right, owner, &[]))
                        .is_some_and(|(left, right)| left == right)
            }

            pub fn applied_machine_callable_origin(
                &self,
                invocation: StaticMachineApplicationHandle,
            ) -> Option<StaticMachineApplicationHandle> {
                if !invocation.is_valid() {
                    return None;
                }
                let retained = self.static_machine_application(invocation);
                if !retained.registration_operation.is_valid() {
                    return None;
                }
                let source = retained.callable_contract_source;
                if !source.is_valid() {
                    return Some(invocation);
                }
                let original = self.static_machine_application(source);
                let lifetimes = self.static_machine_application_lifetimes(invocation);
                (!original.callable_contract_source.is_valid()
                    && original.registration_operation.is_valid()
                    && self.same_registered_machine_operation(
                        retained.registration_operation,
                        original.registration_operation,
                    )
                    && original.binding_declaration.is_valid()
                    && original.lexical_owner.is_valid()
                    && retained.binding_declaration == original.binding_declaration
                    && retained.lexical_owner == original.lexical_owner
                    && (lifetimes.is_empty()
                        || retained.lifetime_binding_declaration
                            == original.lifetime_binding_declaration)
                    && retained.conformance_selection == original.conformance_selection
                    && lifetimes == self.static_machine_application_lifetimes(source)
                    && self.same_selected_bindings(
                        self.static_machine_application_bindings(invocation),
                        self.static_machine_application_bindings(source),
                        original.lexical_owner,
                    ))
                .then_some(source)
            }

            fn same_registered_machine_operation(
                &self,
                left: SymbolHandle,
                right: SymbolHandle,
            ) -> bool {
                let head_and_entry = |head, entry| {
                    self.machine_by_symbol(head).is_some_and(|machine| {
                        self.unique_machine_by_entry_state_symbol(entry)
                            .is_some_and(|(owner, _)| owner.symbol == machine.symbol)
                    })
                };
                left == right || head_and_entry(left, right) || head_and_entry(right, left)
            }

            pub fn applied_machine_callables(
                &self,
                invocation: StaticMachineApplicationHandle,
            ) -> impl Iterator<Item = AppliedMachineCallable<'_>> {
                self.applied_machine_callable_origin(invocation)
                    .into_iter()
                    .flat_map(|origin| self.applied_callable_signatures.machine_rows(origin))
                    .filter_map(|(handle, _)| {
                        self.applied_machine_callable_at(super::super::AppliedMachineCallableId(
                            handle,
                        ))
                    })
            }
        }
    }

    use super::{
        AppliedCallableOwner, AppliedCallableSignature, CallableApplication,
        CallableLifetimeBinding, CallableLifetimeOrigin, CallableSubstitution,
    };
    use crate::{
        TypedTrees, signature,
        static_applications::{StaticMachineApplicationHandle, StaticMachineBindingValue},
        types::TypeReferenceHandle,
    };
    use symbols::SymbolHandle;

    impl TypedTrees {
        pub fn prepare_applied_machine_callable(
            &mut self,
            invocation: StaticMachineApplicationHandle,
            ordinal: usize,
            provider: StaticMachineApplicationHandle,
            selected_entry: SymbolHandle,
            demand: SymbolHandle,
            required: &signature::StateSignature,
            caller_substitutions: &[(SymbolHandle, TypeReferenceHandle)],
            caller_lifetime_bindings: &[crate::static_applications::StaticMachineLifetimeBinding],
        ) -> Result<(), &'static str> {
            let application = self.static_machine_application(provider);
            let selected = application.registration_operation;
            let machine = self.machine_by_symbol(application.binding_declaration);
            let forwarded = if machine.is_none() {
                self.machine_parameter_signature(selected)
                    .map(|(_, signature)| signature.clone())
            } else {
                None
            };
            if machine.is_none() && forwarded.is_none() {
                return Err("applied callable lost its original declaration or Machine binder");
            }
            let substitutions = self
                .static_machine_application_bindings(provider)
                .iter()
                .filter_map(|binding| match binding.value {
                    StaticMachineBindingValue::Type(actual)
                    | StaticMachineBindingValue::Const(actual) => {
                        Some(Ok((binding.parameter, actual)))
                    }
                    StaticMachineBindingValue::Conformance(_)
                    | StaticMachineBindingValue::Machine(_) => None,
                    StaticMachineBindingValue::RuntimeValue { .. } => Some(Err(
                        "applied machine callable has an unclosed runtime operand",
                    )),
                })
                .collect::<Result<Vec<_>, _>>()?;
            let static_arguments = self
                .static_machine_application_bindings(provider)
                .iter()
                .filter_map(|binding| match &binding.value {
                    StaticMachineBindingValue::Conformance(argument)
                    | StaticMachineBindingValue::Machine(argument) => {
                        Some((binding.parameter, argument.clone()))
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            let lifetimes = self
                .static_application_lifetimes_for_declaration(
                    provider,
                    application.binding_declaration,
                )?
                .iter()
                .map(|binding| (binding.parameter.clone(), binding.argument.clone()))
                .collect::<Vec<_>>();
            let declared = if let Some(machine) = machine {
                let state = self
                    .machine_states(machine)
                    .iter()
                    .find(|state| state.symbol == selected)
                    .ok_or("applied callable lost its original entry state")?;
                if !lifetimes.is_empty()
                    && (lifetimes.len() != machine.lifetime_parameters.len()
                        || machine
                            .lifetime_parameters
                            .iter()
                            .zip(&lifetimes)
                            .any(|(parameter, (supplied, _))| parameter != supplied))
                {
                    return Err("applied machine callable has an incomplete lifetime tuple");
                }
                if self
                    .machine_type_parameters(machine)
                    .iter()
                    .any(|parameter| {
                        let supplied = match &parameter.kind {
                            crate::data::TypeParameterKind::Machine { .. } => {
                                static_arguments.iter().any(|(symbol, argument)| {
                                    *symbol == parameter.symbol && argument.symbol.is_valid()
                                })
                            }
                            _ => substitutions
                                .iter()
                                .any(|(symbol, _)| *symbol == parameter.symbol),
                        };
                        !supplied
                    })
                {
                    return Err("applied machine callable has an incomplete provider tuple");
                }
                if machine.conformance_bounds.iter().any(|bound| {
                    !static_arguments
                        .iter()
                        .any(|(symbol, _)| *symbol == bound.binder)
                }) {
                    return Err("applied machine callable has an incomplete evidence tuple");
                }
                signature::StateSignature {
                    symbol: selected,
                    name: state.name.clone(),
                    lifetime_parameters: machine.lifetime_parameters.clone(),
                    parameters: state.parameters,
                    return_type: state.return_type,
                    contracts: machine.contracts,
                    invokes: machine.invokes,
                    service_reach_row: machine.service_reach_row,
                    service_reach_is_installation_bound: machine
                        .service_reach_is_installation_bound,
                    suspends: machine.suspends,
                    blocks: machine.blocks,
                    termination_guarantee: match &machine.termination_plan.interface {
                        language_semantics::TerminationInterface::Published(guarantee) => {
                            guarantee.clone()
                        }
                        _ => language_semantics::TerminationGuarantee::NoGuarantee,
                    },
                    ..signature::StateSignature::default()
                }
            } else {
                if !substitutions.is_empty()
                    || !static_arguments.is_empty()
                    || !lifetimes.is_empty()
                {
                    return Err("forwarded callable provider has an unexpected declaration tuple");
                }
                forwarded.ok_or("applied callable lost its forwarded contract")?
            };
            let provider_lifetime_declaration = application.lifetime_binding_declaration;
            let provider_lifetime_owner = application.lexical_owner;
            let signature = CallableApplication {
                region_scope: application.binding_declaration,
                lifetime_declaration: selected,
                lifetimes: lifetimes
                    .into_iter()
                    .map(|(parameter, argument)| {
                        let region = crate::type_identity::lifetime_parameter_identity(
                            self,
                            application.lexical_owner,
                            argument.as_str(),
                        )
                        .ok_or("applied provider region lost its supplying declaration")?;
                        Ok(CallableLifetimeBinding {
                            parameter,
                            argument,
                            region,
                            origin: CallableLifetimeOrigin::Lexical,
                        })
                    })
                    .collect::<Result<Vec<_>, &'static str>>()?,
                types: substitutions,
                static_arguments,
                static_argument_owner: application.lexical_owner,
                ..CallableApplication::default()
            }
            .signature(self, &declared)?;
            let provider_region_rows = self.static_machine_application_lifetimes(provider).to_vec();
            let provider_lifetimes = self
                .callable_lifetime_substitutions
                .insert_many(provider_region_rows);
            let invocation_source = self.static_machine_application(invocation).clone();
            let caller_lifetime_declaration = invocation_source.lifetime_binding_declaration;
            let caller_lifetime_owner = invocation_source.lexical_owner;
            let source_required = required.clone();
            let caller_snapshot = self
                .expression_table
                .snapshot_static_machine_bindings(invocation);
            let caller_operands =
                self.callable_substitutions
                    .insert_many(caller_substitutions.iter().map(|(parameter, actual)| {
                        CallableSubstitution {
                            parameter: *parameter,
                            actual: *actual,
                        }
                    }));
            let caller_lifetimes = self
                .callable_lifetime_substitutions
                .insert_many(caller_lifetime_bindings.iter().cloned());
            let previous =
                self.applied_callable_signatures
                    .iter()
                    .find_map(|(handle, previous)| {
                        matches!(&previous.owner, AppliedCallableOwner::MachineSelection {
                invocation: source,
                ordinal: position,
                demand: previous_demand,
                selected: previous_entry,
                source_required: previous_required,
                ..
            } if *source == invocation
                && *position == ordinal
                && *previous_demand == demand
                && *previous_entry == selected_entry
                && previous_required == &source_required)
                        .then_some(handle)
                    });
            let prepared = AppliedCallableSignature {
                owner: AppliedCallableOwner::MachineSelection {
                    invocation,
                    ordinal,
                    demand,
                    selected: selected_entry,
                    provider,
                    required: None,
                    source_required,
                    caller_operands,
                    caller_bindings: caller_snapshot,
                    caller_lifetimes,
                    caller_lifetime_declaration,
                    caller_lifetime_owner,
                    provider_lifetime_declaration,
                    provider_lifetime_owner,
                    provider_lifetimes,
                },
                signature,
            };
            if let Some(previous) = previous {
                self.applied_callable_signatures
                    .replace(previous, prepared)?;
            } else {
                self.applied_callable_signatures.insert(prepared);
            }
            Ok(())
        }
    }

    impl TypedTrees {
        pub fn prepare_selected_machine_contracts(
            &mut self,
            invocation: StaticMachineApplicationHandle,
        ) -> Result<(), &'static str> {
            let selected = self
                .expression_table
                .static_machine_application_selected_machines(invocation);
            let mut providers = Vec::new();
            for (_, prepared) in self.applied_callable_signatures.iter() {
                let AppliedCallableOwner::MachineSelection {
                    invocation: source,
                    ordinal,
                    selected: entry,
                    demand,
                    provider,
                    ..
                } = &prepared.owner
                else {
                    continue;
                };
                if *source != invocation
                    || selected
                        .get(*ordinal)
                        .is_none_or(|argument| argument.symbol != *entry)
                    || !self.machine_selection_lifetimes_are_current(&prepared.owner)
                {
                    continue;
                }
                if let Some((_, previous)) =
                    providers.iter().find(|(parameter, _)| parameter == demand)
                {
                    if !self.same_selected_provider(*previous, *provider) {
                        return Err(
                            "current contract helper has conflicting retained provider applications",
                        );
                    }
                } else {
                    providers.push((*demand, *provider));
                }
            }
            let pending = self
                .applied_callable_signatures
                .iter()
                .filter_map(|(handle, prepared)| match &prepared.owner {
                    AppliedCallableOwner::MachineSelection {
                        invocation: source,
                        required: None,
                        ordinal,
                        selected: entry,
                        ..
                    } if *source == invocation
                        && selected
                            .get(*ordinal)
                            .is_some_and(|argument| argument.symbol == *entry)
                        && self.machine_selection_lifetimes_are_current(&prepared.owner) =>
                    {
                        Some(handle)
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            for handle in pending {
                let AppliedCallableOwner::MachineSelection {
                    source_required,
                    caller_operands,
                    caller_bindings,
                    caller_lifetimes,
                    caller_lifetime_owner,
                    ..
                } = self.applied_callable_signatures.get(handle).owner.clone()
                else {
                    unreachable!()
                };
                let arguments = self
                    .expression_table
                    .static_machine_binding_snapshot(caller_bindings)
                    .iter()
                    .filter_map(|binding| match &binding.value {
                        StaticMachineBindingValue::Machine(argument)
                        | StaticMachineBindingValue::Conformance(argument) => {
                            Some((binding.parameter, argument.clone()))
                        }
                        _ => None,
                    })
                    .collect();
                let lifetimes = self
                    .callable_lifetime_substitutions
                    .span_or_empty(caller_lifetimes)
                    .iter()
                    .map(|binding| {
                        let region = crate::type_identity::lifetime_parameter_identity(
                            self,
                            caller_lifetime_owner,
                            binding.argument.as_str(),
                        )
                        .ok_or("applied requirement region lost its supplying declaration")?;
                        Ok(CallableLifetimeBinding {
                            parameter: binding.parameter.clone(),
                            argument: binding.argument.clone(),
                            region,
                            origin: CallableLifetimeOrigin::Lexical,
                        })
                    })
                    .collect::<Result<Vec<_>, &'static str>>()?;
                let types = self
                    .callable_substitutions
                    .span_or_empty(caller_operands)
                    .iter()
                    .map(|operand| (operand.parameter, operand.actual))
                    .collect();
                let required = CallableApplication {
                    lifetimes,
                    types,
                    static_arguments: arguments,
                    static_argument_owner: caller_lifetime_owner,
                    selected_providers: providers.clone(),
                    ..CallableApplication::default()
                }
                .signature(self, &source_required)?;
                self.applied_callable_signatures
                    .set_required(handle, required)?;
            }
            Ok(())
        }

        pub fn applied_machine_provider(
            &self,
            invocation: StaticMachineApplicationHandle,
            ordinal: usize,
            selected: SymbolHandle,
            demand: SymbolHandle,
            expected: &signature::StateSignature,
        ) -> Option<StaticMachineApplicationHandle> {
            self.applied_callable_signatures
                .iter()
                .find_map(|(_, prepared)| match &prepared.owner {
                    AppliedCallableOwner::MachineSelection {
                        invocation: source,
                        ordinal: position,
                        selected: entry,
                        demand: parameter,
                        source_required,
                        provider,
                        ..
                    } if *source == invocation
                        && *position == ordinal
                        && *entry == selected
                        && *parameter == demand
                        && source_required == expected =>
                    {
                        Some(*provider)
                    }
                    _ => None,
                })
        }
    }
    #[cfg(test)]
    pub(crate) mod record_tests {
        use super::*;
        use crate::expression::{StaticMachineArgument, StaticSymbolApplication};
        use crate::name::Identifier;
        use crate::static_applications::{StaticMachineBinding, StaticMachineBindingValue};
        use arena::HandleSpan;
        use symbols::{SymbolKind, SymbolNameRef, SymbolTableBuilder};

        #[test]
        fn selected_contract_records_keep_caller_bindings_and_completion_state() {
            let mut symbols = SymbolTableBuilder::default();
            let caller = symbols.insert_root(SymbolKind::Machine, SymbolNameRef::Static("caller"));
            let helper = symbols.insert_root(SymbolKind::Machine, SymbolNameRef::Static("helper"));
            let binder = symbols.insert_root(
                SymbolKind::MachineParameter,
                SymbolNameRef::Static("Required"),
            );
            let mut program = TypedTrees {
                symbols: symbols.finish(),
                ..Default::default()
            };
            let original_member =
                program
                    .expression_table
                    .insert_static_member(StaticMachineArgument {
                        symbol: helper,
                        path: Box::from([Identifier::from("member")]),
                        ..Default::default()
                    });
            let supplied = StaticMachineArgument {
                symbol: helper,
                path: Box::from([Identifier::from("helper")]),
                application: Some(Box::new(StaticSymbolApplication {
                    member: original_member,
                    lifetime_arguments: Box::from([Identifier::from("borrow")]),
                    arguments: Box::from([StaticMachineArgument {
                        const_literal: Some(
                            numerics::literals::IntegerLiteral::from_parts(
                                false,
                                numerics::literals::IntegerRadix::Decimal,
                                "37",
                            )
                            .unwrap(),
                        ),
                        ..Default::default()
                    }]),
                })),
                ..Default::default()
            };
            let invocation = program.expression_table.retain_static_machine_application(
                caller,
                Identifier::from("caller"),
                std::slice::from_ref(&supplied),
            );
            let binding = StaticMachineBinding {
                parameter: binder,
                value: StaticMachineBindingValue::Machine(supplied),
            };
            program
                .expression_table
                .set_static_machine_application_bindings(
                    invocation,
                    std::slice::from_ref(&binding),
                );
            let snapshot = program
                .expression_table
                .snapshot_static_machine_bindings(invocation);
            let snapshot_rows = program
                .expression_table
                .static_machine_binding_snapshot(snapshot)
                .to_vec();
            let StaticMachineBindingValue::Machine(snapshot_argument) = &snapshot_rows[0].value
            else {
                unreachable!()
            };
            let copied_member = snapshot_argument.application.as_ref().unwrap().member;
            assert_ne!(copied_member, original_member);
            assert_eq!(
                program.expression_table.static_member(copied_member),
                program.expression_table.static_member(original_member)
            );

            let signature = signature::StateSignature {
                symbol: helper,
                name: Identifier::from("required helper"),
                ..Default::default()
            };
            let mut records = Vec::new();
            for required in [None, Some(signature.clone())] {
                let record = AppliedCallableSignature {
                    owner: AppliedCallableOwner::MachineSelection {
                        invocation,
                        ordinal: 1,
                        demand: binder,
                        selected: helper,
                        provider: invocation,
                        required,
                        source_required: signature.clone(),
                        caller_operands: HandleSpan::empty(),
                        caller_bindings: snapshot,
                        caller_lifetimes: HandleSpan::empty(),
                        caller_lifetime_declaration: caller,
                        caller_lifetime_owner: caller,
                        provider_lifetime_declaration: helper,
                        provider_lifetime_owner: caller,
                        provider_lifetimes: HandleSpan::empty(),
                    },
                    signature: signature.clone(),
                };
                let handle = program.applied_callable_signatures.insert(record.clone());
                records.push((handle, record));
            }
            let bytes =
                compiler_storage::encode(&program, compiler_storage::RecordLimits::default())
                    .unwrap();
            let restored: TypedTrees =
                compiler_storage::decode(&bytes, compiler_storage::RecordLimits::default())
                    .unwrap();
            for (handle, original) in records {
                assert_eq!(restored.applied_callable_signatures.get(handle), &original);
                let AppliedCallableOwner::MachineSelection {
                    caller_bindings, ..
                } = original.owner
                else {
                    unreachable!()
                };
                assert_eq!(
                    restored
                        .expression_table
                        .static_machine_binding_snapshot(caller_bindings),
                    snapshot_rows.as_slice()
                );
            }
            assert_eq!(
                restored.static_machine_application(invocation),
                program.static_machine_application(invocation)
            );
        }
    }
}
pub(crate) mod namespaces {
    use super::CallableApplication;
    use crate::{
        TypedTrees, expression::TableCallExpression, signature, trait_definition::AppliedTraitScope,
    };
    use symbols::{SymbolHandle, SymbolKind};

    impl CallableApplication {
        pub(crate) fn namespace_symbols(
            &self,
            program: &mut TypedTrees,
            binder: SymbolHandle,
            expressions: &[crate::expression::ExpressionHandle],
        ) -> Result<Vec<(SymbolHandle, SymbolHandle)>, &'static str> {
            let selected = if let Some((_, argument)) = self
                .static_arguments
                .iter()
                .rev()
                .find(|(source, _)| *source == binder)
            {
                argument.clone()
            } else if let Some((_, argument)) = self
                .types
                .iter()
                .rev()
                .find(|(source, _)| *source == binder)
            {
                program.static_operand_from_type_reference(*argument)?
            } else {
                return Ok(Vec::new());
            };
            if !selected.names_implementation(&program.symbols) {
                return Err("applied requirement namespace needs an implementation operand");
            }
            if selected.application.is_none() {
                return program.selected_implementation_namespace_symbols(binder, &selected);
            }
            let owner = program.symbols.get(binder).parent;
            let mut calls = Vec::new();
            for handle in expressions {
                let Some(call) = program.expression_table.call_node(*handle) else {
                    continue;
                };
                if !program
                    .expression_table
                    .name_path_node(call.receiver)
                    .is_some_and(|path| path.head_symbol == binder)
                {
                    continue;
                }
                program
                    .evidence_bound_requirement(owner, binder, call.target_symbol)
                    .map_err(|_| "applied namespace call lost its source bound")?
                    .ok_or("applied namespace call lost its exact requirement")?;
                calls.push((*handle, call.receiver));
            }
            calls.sort_unstable_by_key(|(_, receiver)| {
                (receiver.arena_index(), receiver.generation())
            });
            if calls.is_empty() || expressions.iter().any(|handle| {
                matches!(program.expression_table.expression(*handle), crate::expression::ExpressionNode::Name(path) if path.head_symbol == binder)
                    && calls.binary_search_by_key(&(handle.arena_index(), handle.generation()), |(_, receiver)| (receiver.arena_index(), receiver.generation())).is_err()
            }) {
                return Err("applied namespace needs a complete qualified call");
            }
            let qualifier = if selected.type_reference.is_valid() {
                selected.type_reference
            } else {
                let owner = self
                    .trait_region_application
                    .as_ref()
                    .map_or(self.static_argument_owner, |(scope, _)| {
                        scope.argument_owner()
                    });
                program.static_operand_type_reference_in_scope(&selected, owner)?
            };
            // Keep the complete selected operand on each call. Psi04's ordinary
            // qualified-selection owner checks its application and public row.
            for (handle, _) in calls {
                let crate::expression::ExpressionNode::Call(call) =
                    program.expression_table.expression_mut(handle)
                else {
                    unreachable!();
                };
                call.requirement_application = qualifier;
            }
            program.implementation_namespace_symbols(binder, selected.symbol)
        }
    }

    impl TypedTrees {
        pub fn selected_implementation_namespace_symbols(
            &self,
            binder: SymbolHandle,
            selected: &crate::expression::StaticMachineArgument,
        ) -> Result<Vec<(SymbolHandle, SymbolHandle)>, &'static str> {
            let program = self;
            if !selected.names_implementation(&program.symbols) {
                return Err("applied requirement namespace needs an implementation operand");
            }
            if selected.application.is_some() {
                return Err(
                    "applied requirement namespace needs its complete selected application",
                );
            }
            self.implementation_namespace_symbols(binder, selected.symbol)
        }

        fn implementation_namespace_symbols(
            &self,
            binder: SymbolHandle,
            selected: SymbolHandle,
        ) -> Result<Vec<(SymbolHandle, SymbolHandle)>, &'static str> {
            let program = self;
            let mut symbols = vec![(binder, selected)];
            let owner = program.symbols.get(binder).parent;
            for child in program.symbols.child_handles(binder).into_iter().flatten() {
                if program.symbols.get(child).kind != SymbolKind::State {
                    continue;
                }
                let requirement = program
                    .evidence_bound_requirement(owner, binder, child)
                    .map_err(|_| "applied requirement namespace lost its authored bound")?
                    .ok_or("applied requirement namespace lost its requirement")?;
                let mut candidates = Vec::new();
                if let Some(conformance) = program.conformance_by_symbol(selected) {
                    candidates.extend(
                        conformance
                            .rows
                            .iter()
                            .filter(|row| {
                                row.declaring_trait == requirement.application.declaration.symbol
                                    && row.requirement == requirement.signature.symbol
                            })
                            .map(|row| row.requirement),
                    );
                } else {
                    let selected_owner = program.symbols.get(selected).parent;
                    for target in program
                        .symbols
                        .child_handles(selected)
                        .into_iter()
                        .flatten()
                    {
                        if let Some(bound) = program
                            .evidence_bound_requirement(selected_owner, selected, target)
                            .map_err(|_| "selected namespace lost its authored bound")?
                            && bound.signature.symbol == requirement.signature.symbol
                            && bound.application.declaration.symbol
                                == requirement.application.declaration.symbol
                        {
                            candidates.push(target);
                        }
                    }
                }
                let [target] = candidates.as_slice() else {
                    return Err("selected namespace has no unique exact requirement row");
                };
                symbols.push((child, *target));
            }
            Ok(symbols)
        }
    }

    impl TypedTrees {
        pub fn named_conformance_call_signature(
            &self,
            call: &TableCallExpression,
        ) -> Option<&signature::StateSignature> {
            self.resolve_named_conformance_call_signature(call)
                .ok()
                .flatten()
        }

        /// Separate an ordinary receiver from a named map whose selected public
        /// view refuses. An absent or ambiguous map must not borrow an ambient
        /// trait application after losing its own selection.
        pub(super) fn resolve_named_conformance_call_signature(
            &self,
            call: &TableCallExpression,
        ) -> Result<Option<&signature::StateSignature>, &'static str> {
            if call.static_requirement_dispatch.is_some() || call.requirement_application.is_valid()
            {
                return Ok(None);
            }
            let Some(receiver) = self.expression_table.name_path_node(call.receiver) else {
                return Ok(None);
            };
            let Some(selected) = self.conformance_by_symbol(receiver.symbol) else {
                return if receiver.symbol.is_valid()
                    && self.symbols.get(receiver.symbol).kind == SymbolKind::Conformance
                {
                    Err("named public view lost its selected implementation")
                } else {
                    Ok(None)
                };
            };
            // Applied generic maps retain their independently supplied tuple in
            // the ordinary dispatch path above; a bare declaration cannot supply it.
            if !selected.type_parameters.is_empty() || !selected.lifetime_parameters.is_empty() {
                return Err("named public view has no complete selected application");
            }
            let closed = self.closed_requirement_application(call.target_symbol);
            let template = closed
                .as_ref()
                .map_or(call.target_symbol, |selected| selected.template);
            let mut rows = selected
                .rows
                .iter()
                .filter(|row| row.requirement == template);
            let row = rows
                .next()
                .ok_or("named public view has no exact requirement row")?;
            if rows.next().is_some() {
                return Err("named public view has ambiguous requirement rows");
            }
            let scope = AppliedTraitScope::from_conformance(self, selected)
                .ok_or("named public view lost its complete trait application")?
                .declaring_scope(self, row.declaring_trait)
                .ok_or("named public view lost its declaring trait scope")?;
            if closed.is_some()
                && !self.requirement_application_matches_scope(call.target_symbol, &scope)
            {
                return Err("named public view disagrees with its selected trait application");
            }
            self.applied_callable_signature(&scope, template)
                .or_else(|| {
                    (scope.substitutions.is_empty()
                        && scope.declaration.lifetime_parameters.is_empty())
                    .then(|| self.trait_machine_signature_by_symbol(scope.declaration, template))
                    .flatten()
                })
                .map(Some)
                .ok_or("named public view has no prepared applied signature")
        }
    }
}
pub use machine_instances::MachineInstantiation;
pub(crate) mod payloads {
    use super::{CallableApplication, CallableLifetimeOrigin};
    use crate::{TypedTrees, expression, static_applications, types};
    use arena::HandleSpan;

    impl CallableApplication {
        pub(crate) fn type_reference(
            &mut self,
            program: &mut TypedTrees,
            original: types::TypeReferenceHandle,
        ) -> Result<types::TypeReferenceHandle, &'static str> {
            let obligations = program
                .type_reference_table
                .type_alias_applications(original)
                .cloned()
                .collect::<Vec<_>>();
            let applied = self.type_reference_with_owner(program, original)?;
            if obligations.is_empty() {
                return Ok(applied);
            }
            let applied = program.type_reference_table.fresh_occurrence(applied);
            for obligation in obligations {
                let application = self.type_reference(program, obligation.application)?;
                let (declaration, arguments, lifetime_arguments) =
                    match program.type_reference_node(application) {
                        types::TypeReferenceNode::Generic {
                            base_symbol,
                            arguments,
                            lifetime_arguments,
                            ..
                        } => (*base_symbol, *arguments, lifetime_arguments.clone()),
                        types::TypeReferenceNode::Named { symbol, .. } => {
                            (*symbol, HandleSpan::empty(), Vec::new())
                        }
                        _ => return Err("applied alias obligation lost its declaration head"),
                    };
                if declaration != obligation.declaration {
                    return Err("applied alias obligation changed its declaration");
                }
                let supplied_operand_owner = program
                    .type_reference_table
                    .operand_lifetime_owner(application);
                program.type_reference_table.retain_type_alias_application(
                    crate::type_aliases::TypeAliasApplication {
                        instance: applied,
                        application,
                        arguments,
                        lifetime_arguments,
                        supplied_operand_owner,
                        ..obligation
                    },
                );
            }
            Ok(applied)
        }

        fn type_reference_with_owner(
            &mut self,
            program: &mut TypedTrees,
            original: types::TypeReferenceHandle,
        ) -> Result<types::TypeReferenceHandle, &'static str> {
            if !original.is_valid() {
                return Ok(original);
            }
            let owner = program
                .type_reference_table
                .operand_lifetime_owner(original);
            if owner.is_valid()
                && !program
                    .type_reference_table
                    .has_resolved_operand_lifetimes(original)
                && program.lifetime_declaration_owner(owner)
                    != program.lifetime_declaration_owner(self.region_scope)
            {
                let lifetimes = std::mem::take(&mut self.lifetimes);
                let elided_lifetimes = std::mem::take(&mut self.elided_lifetimes);
                let applied = self.type_payload(program, original);
                self.lifetimes = lifetimes;
                self.elided_lifetimes = elided_lifetimes;
                return applied;
            }
            self.type_payload(program, original)
        }

        fn type_payload(
            &mut self,
            program: &mut TypedTrees,
            original: types::TypeReferenceHandle,
        ) -> Result<types::TypeReferenceHandle, &'static str> {
            let mut applied_owner = program
                .type_reference_table
                .operand_lifetime_owner(original);
            let mut applied_lifetimes = Vec::new();
            let mut supplied_origins = Vec::new();
            let mut supplied_extent_argument = None;
            let node = match program.type_reference_node(original).clone() {
                types::TypeReferenceNode::Named { symbol, name } => {
                    if let Some((_, argument)) = self
                        .static_arguments
                        .iter()
                        .rev()
                        .find(|(binder, _)| *binder == symbol)
                    {
                        return Self::selected_argument_reference(program, argument);
                    }
                    if let Some((_, argument)) = self
                        .types
                        .iter()
                        .rev()
                        .find(|(binder, _)| *binder == symbol)
                    {
                        return Ok(*argument);
                    }
                    types::TypeReferenceNode::Named {
                        symbol: self.symbol(symbol),
                        name,
                    }
                }
                types::TypeReferenceNode::Reference {
                    referee,
                    access,
                    lifetime,
                } => {
                    let lifetime = lifetime
                        .as_ref()
                        .filter(|lifetime| !self.elided_lifetimes.contains(lifetime))
                        .map(|lifetime| self.scoped_lifetime(program, original, 0, lifetime))
                        .transpose()?;
                    if let Some(lifetime) = &lifetime {
                        applied_lifetimes.push(lifetime.region);
                        if lifetime.region.declaration.is_valid() {
                            applied_owner = lifetime.region.declaration;
                        }
                    }
                    types::TypeReferenceNode::Reference {
                        referee: self.type_reference(program, referee)?,
                        access,
                        lifetime: lifetime.map(|lifetime| lifetime.name),
                    }
                }
                types::TypeReferenceNode::Constrained {
                    base_type,
                    constraints,
                } => {
                    let base_type = self.type_reference(program, base_type)?;
                    let original_constraints = program
                        .type_reference_table
                        .constraints(constraints)
                        .to_vec();
                    let mut constraints = Vec::with_capacity(original_constraints.len());
                    for constraint in original_constraints {
                        constraints.push(match constraint {
                            types::TypeConstraintNode::Domain(mut domain) => {
                                domain.arguments = domain
                                    .arguments
                                    .into_iter()
                                    .map(|argument| self.type_reference(program, argument))
                                    .collect::<Result<Vec<_>, _>>()?;
                                domain.semantic_id = Self::domain_identity(
                                    program,
                                    domain.symbol,
                                    &domain.arguments,
                                    domain.semantic_id,
                                )?;
                                domain.semantic_roles.denotation_dimension = domain
                                    .semantic_roles
                                    .denotation_dimension
                                    .map(|_| domain.semantic_id);
                                domain.semantic_roles.arithmetic_policy = domain
                                    .semantic_roles
                                    .arithmetic_policy
                                    .map(|_| domain.semantic_id);
                                types::TypeConstraintNode::Domain(domain)
                            }

                            constraint => constraint,
                        });
                    }
                    let constraints = program.type_reference_table.insert_constraints(constraints);
                    types::TypeReferenceNode::Constrained {
                        base_type,
                        constraints,
                    }
                }
                types::TypeReferenceNode::FixedArray {
                    element_type,
                    length,
                } => {
                    let length = match length {
                        types::FixedArrayLength::ConstParameter { symbol, name } => {
                            if let Some((_, argument)) = self
                                .types
                                .iter()
                                .rev()
                                .find(|(binder, _)| *binder == symbol)
                            {
                                supplied_extent_argument = Some(*argument);
                                match program.type_reference_node(*argument) {
                                    types::TypeReferenceNode::Named { symbol, name } => {
                                        if let Some(value) = program
                                            .type_reference_table
                                            .anonymous_numeric_literal_value(*argument)
                                        {
                                            types::FixedArrayLength::Literal(value)
                                        } else if let Some(value) =
                                            crate::integer_const_declaration_extent(
                                                program, *symbol,
                                            )
                                        {
                                            supplied_origins.extend(
                                                program
                                                    .type_reference_table
                                                    .constant_origins(*argument)?
                                                    .iter()
                                                    .copied(),
                                            );
                                            types::FixedArrayLength::Literal(value)
                                        } else {
                                            types::FixedArrayLength::ConstParameter {
                                                symbol: *symbol,
                                                name: name.clone(),
                                            }
                                        }
                                    }
                                    types::TypeReferenceNode::ConstExpression(expression) => {
                                        match program.expression_table.expression(*expression) {
                                            expression::ExpressionNode::Integer(value) => {
                                                let value = value.value_u64().and_then(|value| usize::try_from(value).ok())
                                                .ok_or("nominal callable array extent is outside the host extent range")?;
                                                types::FixedArrayLength::Literal(value)
                                            }
                                            _ => types::FixedArrayLength::Expression(*argument),
                                        }
                                    }
                                    _ => {
                                        return Err(
                                            "nominal callable array extent argument is not a const index",
                                        );
                                    }
                                }
                            } else {
                                types::FixedArrayLength::ConstParameter {
                                    symbol: self.symbol(symbol),
                                    name,
                                }
                            }
                        }
                        types::FixedArrayLength::Expression(reference) => {
                            types::FixedArrayLength::Expression(
                                self.type_reference(program, reference)?,
                            )
                        }
                        length => length,
                    };
                    types::TypeReferenceNode::FixedArray {
                        element_type: self.type_reference(program, element_type)?,
                        length,
                    }
                }
                types::TypeReferenceNode::Slice { element_type } => {
                    types::TypeReferenceNode::Slice {
                        element_type: self.type_reference(program, element_type)?,
                    }
                }
                types::TypeReferenceNode::Generic {
                    base_symbol,
                    base_name,
                    lifetime_arguments,
                    arguments,
                } => {
                    let nominal_owner = self.symbol(base_symbol);
                    if !applied_owner.is_valid()
                        && !lifetime_arguments.is_empty()
                        && lifetime_arguments.iter().all(|lifetime| {
                            !self
                                .lifetimes
                                .iter()
                                .any(|binding| binding.parameter == *lifetime)
                                && crate::type_identity::lifetime_parameter_identity(
                                    program,
                                    self.region_scope,
                                    lifetime.as_str(),
                                )
                                .is_some()
                        })
                    {
                        applied_owner = self.region_scope;
                    }
                    if !applied_owner.is_valid()
                        && lifetime_arguments
                            .iter()
                            .any(|lifetime| self.elided_lifetimes.contains(lifetime))
                    {
                        applied_owner = self.region_scope;
                    }
                    let lifetimes = lifetime_arguments
                    .iter()
                    .enumerate()
                    .map(|(slot, lifetime)| {
                        if self.elided_lifetimes.contains(lifetime) {
                            if crate::type_identity::lifetime_parameter_identity(
                                program,
                                applied_owner,
                                lifetime.as_str(),
                            )
                            .is_none()
                            {
                                return Err(
                                    "elided nominal result lost its supplying lifetime telescope",
                                );
                            }
                            return Ok(super::ScopedCallableLifetime {
                                name: lifetime.clone(),
                                region: Default::default(),
                                origin: super::CallableLifetimeOrigin::Lexical,
                            });
                        }
                        self.scoped_lifetime(program, original, slot, lifetime)
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                    for lifetime in &lifetimes {
                        if lifetime.region.declaration.is_valid()
                            && crate::type_identity::lifetime_parameter_name(
                                program,
                                lifetime.region,
                            ) != Some(&lifetime.name)
                        {
                            return Err(
                                "applied nominal region lost its exact declaration ordinal",
                            );
                        }
                    }
                    if let Some((environment, _)) = &self.trait_region_application {
                        let supplier = environment.argument_owner();
                        if lifetimes.iter().all(|lifetime| {
                            lifetime.origin == CallableLifetimeOrigin::Captured
                                && crate::type_identity::lifetime_parameter_identity(
                                    program,
                                    supplier,
                                    lifetime.name.as_str(),
                                ) == Some(lifetime.region)
                        }) {
                            applied_owner = supplier;
                        } else if !applied_owner.is_valid() {
                            applied_owner = self.region_scope;
                        }
                    }
                    applied_lifetimes.extend(lifetimes.iter().map(|lifetime| lifetime.region));
                    types::TypeReferenceNode::Generic {
                        base_symbol: nominal_owner,
                        base_name,
                        lifetime_arguments: lifetimes
                            .into_iter()
                            .map(|lifetime| lifetime.name)
                            .collect(),
                        arguments: self.type_arguments(program, arguments)?,
                    }
                }
                types::TypeReferenceNode::DynamicTrait {
                    symbol,
                    name,
                    arguments,
                } => types::TypeReferenceNode::DynamicTrait {
                    symbol: self.symbol(symbol),
                    name,
                    arguments: self.type_arguments(program, arguments)?,
                },
                types::TypeReferenceNode::ConstExpression(expression) => {
                    types::TypeReferenceNode::ConstExpression(self.expression(program, expression)?)
                }
                types::TypeReferenceNode::Unit => types::TypeReferenceNode::Unit,
            };
            let mut origins = program
                .type_reference_table
                .constant_origins(original)?
                .to_vec();
            origins.extend(supplied_origins);
            let extent_argument = match (&node, supplied_extent_argument) {
                (types::TypeReferenceNode::FixedArray { .. }, Some(argument)) => Some(argument),
                (
                    types::TypeReferenceNode::FixedArray {
                        length: types::FixedArrayLength::Expression(argument),
                        ..
                    },
                    None,
                ) => Some(*argument),
                (types::TypeReferenceNode::FixedArray { .. }, None) => {
                    let argument = program.type_reference_table.array_extent_argument(original);
                    if argument.is_valid() {
                        Some(self.type_reference(program, argument)?)
                    } else {
                        None
                    }
                }
                _ => None,
            };
            let applied = program.type_reference_table.insert(node);
            if let Some(argument) = extent_argument {
                program
                    .type_reference_table
                    .retain_array_extent_argument(applied, argument)?;
            }
            let origin = program.type_reference_table.operand_origin(original);
            program
                .type_reference_table
                .set_operand_origin(applied, origin);
            program
                .type_reference_table
                .attach_constant_origins(applied, origins)?;
            program
                .type_reference_table
                .retain_operand_lifetime_owner(applied, applied_owner)?;
            program
                .type_reference_table
                .retain_operand_lifetimes(applied, applied_lifetimes)?;
            Ok(applied)
        }

        pub(crate) fn type_arguments(
            &mut self,
            program: &mut TypedTrees,
            original: HandleSpan<types::TypeReferenceHandle>,
        ) -> Result<HandleSpan<types::TypeReferenceHandle>, &'static str> {
            let arguments = program.type_reference_handles(original).to_vec();
            if arguments.len() != original.len() {
                return Err("applied callable has a stale type-argument span");
            }
            let applied = arguments
                .into_iter()
                .map(|argument| self.type_reference(program, argument))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(program
                .type_reference_table
                .insert_type_reference_handles(applied))
        }

        pub(crate) fn expression(
            &mut self,
            program: &mut TypedTrees,
            original: expression::ExpressionHandle,
        ) -> Result<expression::ExpressionHandle, &'static str> {
            if !original.is_valid() {
                return Ok(original);
            }
            let first = program.expression_table.expression_count() + 1;
            let copied = program.expression_table.insert_copy(original);
            let rows = program
                .expression_table
                .iter_expressions()
                .filter(|(handle, _)| handle.arena_index() as usize >= first)
                .map(|(handle, _)| handle)
                .collect::<Vec<_>>();
            for handle in &rows {
                let expression::ExpressionNode::Name(mut path) =
                    program.expression_table.expression(*handle).clone()
                else {
                    continue;
                };
                if path.head_symbol.is_valid() || path.symbol.is_valid() {
                    continue;
                }
                let [member] = program.expression_table.name_path_members(path.members) else {
                    continue;
                };
                if let Some((_, symbol)) = self
                    .source_binders
                    .iter()
                    .rev()
                    .find(|(name, _)| name == member)
                {
                    let mut member_symbols = HandleSpan::empty();
                    program
                        .expression_table
                        .push_name_path_member_symbol(&mut member_symbols, *symbol);
                    path.member_symbols = member_symbols;
                    path.head_symbol = *symbol;
                    path.symbol = *symbol;
                    *program.expression_table.expression_mut(*handle) =
                        expression::ExpressionNode::Name(path);
                }
            }
            let mut symbols = self.symbols.clone();
            for handle in &rows {
                if let expression::ExpressionNode::Name(path) =
                    program.expression_table.expression(*handle)
                {
                    let binder = path.head_symbol;
                    if binder.is_valid()
                        && program.symbols.get(binder).kind
                            == symbols::SymbolKind::ConformanceParameter
                        && !symbols.iter().any(|(source, _)| *source == binder)
                    {
                        symbols.extend(self.namespace_symbols(program, binder, &rows)?);
                    }
                }
            }
            let call_targets = rows
                .iter()
                .map(
                    |handle| match program.expression_table.expression(*handle) {
                        expression::ExpressionNode::Call(call) => {
                            self.selected_call_target(program, call.target_symbol)
                        }
                        _ => Ok(None),
                    },
                )
                .collect::<Result<Vec<_>, _>>()?;
            program.expression_table.remap_symbols_in(copied, &symbols);
            for (handle, call_target) in rows.into_iter().zip(call_targets) {
                let node = program.expression_table.expression(handle).clone();
                let applied = match node {
                expression::ExpressionNode::Name(path) => {
                    if path.head_symbol.is_valid()
                        && program.symbols.get(path.head_symbol).kind
                            == symbols::SymbolKind::ConformanceParameter
                    {
                        continue;
                    }
                    if self
                        .static_arguments
                        .iter()
                        .any(|(binder, _)| *binder == path.head_symbol)
                    {
                        return Err(
                            "applied evidence expression requires an exact requirement namespace mapping",
                        );
                    }
                    if let Some((_, argument)) = self
                        .types
                        .iter()
                        .rev()
                        .find(|(binder, _)| *binder == path.head_symbol)
                    {
                        let argument = *argument;
                        let carrier = program
                            .data_type_parameter_by_symbol(path.head_symbol)
                            .and_then(|parameter| match parameter.kind {
                                crate::data::TypeParameterKind::Const { type_reference }
                                | crate::data::TypeParameterKind::Value { type_reference } => {
                                    Some(type_reference)
                                }
                                _ => None,
                            });
                        let carrier = match carrier {
                            Some(carrier) => self.type_reference(program, carrier)?,
                            None => types::TypeReferenceHandle::invalid(),
                        };
                        let replacement =
                            Self::selected_value_expression(program, argument, carrier)?;
                        Some(program.expression_table.expression(replacement).clone())
                    } else {
                        None
                    }
                }
                expression::ExpressionNode::StructLiteral(mut literal) if literal.application_type.is_valid() => {
                    literal.application_type =
                        self.type_reference(program, literal.application_type)?;
                    Some(expression::ExpressionNode::StructLiteral(literal))
                }
                expression::ExpressionNode::Match(mut dispatch) if program.expression_table.match_arms(dispatch.arms).iter().any(|arm|
                    matches!(arm.pattern, expression::MatchPattern::Destructure(pattern) if pattern.application_type.is_valid())) => {
                    let mut arms = program.expression_table.match_arms(dispatch.arms).to_vec();
                    for arm in &mut arms {
                        if let expression::MatchPattern::Destructure(ref mut pattern) = arm.pattern {
                            pattern.application_type = self.type_reference(program, pattern.application_type)?;
                        }
                    }
                    dispatch.arms = program.expression_table.insert_match_arms(arms);
                    Some(expression::ExpressionNode::Match(dispatch))
                }
                expression::ExpressionNode::Cast(mut cast) => {
                    cast.target_type = self.type_reference(program, cast.target_type)?;
                    cast.result_type = self.type_reference(program, cast.result_type)?;
                    cast.semantic_domain_arguments =
                        self.type_arguments(program, cast.semantic_domain_arguments)?;
                    let arguments = program.type_reference_table
                        .type_reference_handles(cast.semantic_domain_arguments).to_vec();
                    cast.semantic_domain_id = Self::domain_identity(
                        program,
                        cast.semantic_domain_symbol,
                        &arguments,
                        cast.semantic_domain_id,
                    )?;
                    Some(expression::ExpressionNode::Cast(cast))
                }
                expression::ExpressionNode::TypeExpression(reference) => {
                    Some(expression::ExpressionNode::TypeExpression(
                        self.type_reference(program, reference)?,
                    ))
                }
                expression::ExpressionNode::ZeroValue(reference) => Some(
                    expression::ExpressionNode::ZeroValue(self.type_reference(program, reference)?),
                ),
                expression::ExpressionNode::Call(mut call) => {
                    call.requirement_application =
                        self.type_reference(program, call.requirement_application)?;
                    call.exact_coercion_target =
                        self.type_reference(program, call.exact_coercion_target)?;
                    if let Some(request) = &mut call.installed_interrupt_operation {
                        request.invalidate_preparation();
                        self.static_arguments(program, std::slice::from_mut(&mut request.registrar))?;
                        self.static_arguments(program, std::slice::from_mut(&mut request.timer_slot))?;
                    }
                    self.static_arguments(program, &mut call.machine_arguments)?;
                    if let Some(query) = &mut call.native_code_layout {
                        self.static_arguments(program, std::slice::from_mut(&mut query.operand))?;
                    }
                    for application in
                        crate::references::StaticArgumentCall::Expression(handle, &call).applications()
                    {
                        let lifetimes = program
                            .static_machine_application_lifetimes(application)
                            .iter()
                            .map(
                                |binding| static_applications::StaticMachineLifetimeBinding {
                                    parameter: binding.parameter.clone(),
                                    argument: self.lifetime(&binding.argument),
                                },
                            )
                            .collect::<Vec<_>>();
                        let declaration = program
                            .static_machine_application(application)
                            .lifetime_binding_declaration;
                        program
                            .expression_table
                            .set_static_machine_application_lifetimes(
                                application,
                                declaration,
                                &lifetimes,
                            );
                        let mut arguments = program
                            .static_machine_application_arguments(application)
                            .to_vec();
                        self.static_arguments(program, &mut arguments)?;
                        program
                            .expression_table
                            .static_machine_application_arguments_mut(application)
                            .clone_from_slice(&arguments);
                        let mut selected = program
                            .expression_table
                            .static_machine_application_selected_machines(application)
                            .to_vec();
                        self.static_arguments(program, &mut selected)?;
                        program
                            .expression_table
                            .static_machine_application_selected_machines_mut(application)
                            .clone_from_slice(&selected);
                        let mut bindings = program
                            .static_machine_application_bindings(application)
                            .to_vec();
                        for binding in &mut bindings {
                            match &mut binding.value {
                                static_applications::StaticMachineBindingValue::Type(reference)
                                | static_applications::StaticMachineBindingValue::Const(
                                    reference,
                                ) => *reference = self.type_reference(program, *reference)?,
                                static_applications::StaticMachineBindingValue::RuntimeValue {
                                    type_reference,
                                    ..
                                } => {
                                    *type_reference =
                                        self.type_reference(program, *type_reference)?
                                }
                                static_applications::StaticMachineBindingValue::Machine(
                                    argument,
                                )
                                | static_applications::StaticMachineBindingValue::Conformance(
                                    argument,
                                ) => {
                                    self.static_arguments(program, std::slice::from_mut(argument))?
                                }
                            }
                        }
                        program
                            .expression_table
                            .static_machine_application_bindings_mut(application)
                            .clone_from_slice(&bindings);
                    }
                    if let Some(target) = call_target {
                        self.apply_call_target(program, &mut call, target)?;
                    }
                    Some(expression::ExpressionNode::Call(call))
                }
                _ => None,
            };
                if let Some(applied) = applied {
                    *program.expression_table.expression_mut(handle) = applied;
                }
                if let Some(mut authority) = program
                    .expression_table
                    .index_expression_authority(handle)
                    .cloned()
                {
                    if let Some(normalization) = &mut authority.normalization {
                        normalization.index_type =
                            self.type_reference(program, normalization.index_type)?;
                    }
                    if let Some(operation) = &mut authority.operation {
                        operation.operand_type =
                            self.type_reference(program, operation.operand_type)?;
                        if let Some(application) = &mut operation.application {
                            self.static_arguments(program, std::slice::from_mut(application))?;
                        }
                        if let Some((_, selected)) = self
                            .static_arguments
                            .iter()
                            .rev()
                            .find(|(binder, _)| *binder == operation.provider)
                        {
                            let alias = if program.symbols.get(selected.symbol).kind
                                == symbols::SymbolKind::ConformanceParameter
                            {
                                program.symbols.name(selected.symbol).to_owned()
                            } else {
                                let conformance = program.conformance_by_symbol(selected.symbol)
                                .ok_or("applied index authority lost its selected evidence declaration")?;
                                conformance
                                    .alias
                                    .as_ref()
                                    .map_or("<unnamed-conformance>", |alias| alias.as_str())
                                    .to_owned()
                            };
                            operation.apply_provider(selected, &alias);
                        }
                    }
                    *program
                        .expression_table
                        .index_expression_authority_mut(handle) = authority;
                }
            }
            Ok(copied)
        }

        pub(crate) fn static_arguments(
            &mut self,
            program: &mut TypedTrees,
            arguments: &mut [expression::StaticMachineArgument],
        ) -> Result<(), &'static str> {
            for argument in arguments {
                let selected = self
                    .static_arguments
                    .iter()
                    .rev()
                    .find(|(symbol, _)| *symbol == argument.symbol)
                    .map(|(_, replacement)| Ok(replacement.clone()))
                    .or_else(|| {
                        self.types
                            .iter()
                            .rev()
                            .find(|(symbol, _)| *symbol == argument.symbol)
                            .map(|(_, replacement)| {
                                program.static_operand_from_type_reference(*replacement)
                            })
                    });
                if let Some(selected) = selected {
                    if argument.application.is_some() {
                        return Err(
                            "an applied callable binder cannot be replaced as a bare static operand",
                        );
                    }
                    *argument = selected?;
                    continue;
                }
                argument.type_reference = self.type_reference(program, argument.type_reference)?;
                if let Some(application) = &mut argument.application {
                    for lifetime in &mut application.lifetime_arguments {
                        *lifetime = self.lifetime(lifetime);
                    }
                    self.static_arguments(program, &mut application.arguments)?;
                    if application.member.is_valid() {
                        let mut member = program
                            .expression_table
                            .static_member(application.member)
                            .ok_or("qualified static member has no owning expression record")?
                            .clone();
                        self.static_arguments(program, std::slice::from_mut(&mut member))?;
                        *program
                            .expression_table
                            .static_member_mut(application.member)
                            .ok_or("qualified static member lost its owning expression record")? =
                            member;
                    }
                }
            }
            Ok(())
        }
    }
}
pub(crate) mod realization_scopes {
    use super::{AppliedCallableOwner, AppliedCallableSignature};
    use crate::{TypedTrees, signature, trait_definition::RetainedTraitScope};
    use symbols::SymbolHandle;

    impl TypedTrees {
        pub(crate) fn prepare_realization_contracts(
            &mut self,
            selected: &crate::ClosedConformanceApplication,
            lexical_owner: SymbolHandle,
            scope: RetainedTraitScope,
        ) -> Result<(), &'static str> {
            let conformance = self
                .conformance_by_symbol(selected.declaration)
                .ok_or("realization contract lost its selected implementation")?;
            if conformance.type_parameters.is_empty() && conformance.lifetime_parameters.is_empty()
            {
                return Ok(());
            }
            let application = self
                .portable_conformance_application_identity_in_owner(selected, lexical_owner)
                .ok_or("realization contract lost its complete capture identity")?;
            let owner = AppliedCallableOwner::Realization {
                declaration: selected.declaration,
                application,
                lexical_owner,
            };
            let rows = conformance
                .rows
                .iter()
                .filter(|row| row.source.owns_realization())
                .map(|row| (row.realization_machine, row.realization_state))
                .collect::<Vec<_>>();
            let environment = scope
                .scope(self)
                .ok_or("realization contract lost its supplying scope")?
                .environment();
            for (machine_symbol, entry) in rows {
                if self
                    .applied_callable_signatures
                    .iter()
                    .any(|(_, prepared)| {
                        prepared.owner == owner && prepared.signature.symbol == entry
                    })
                {
                    continue;
                }
                let machine = self
                    .machine_by_symbol(machine_symbol)
                    .ok_or("realization contract lost its owned machine")?;
                let state = self
                    .state_in_machine(machine, entry)
                    .ok_or("realization contract lost its exact entry")?;
                let declared = signature::StateSignature {
                    symbol: entry,
                    name: state.name.clone(),
                    lifetime_parameters: machine.lifetime_parameters.clone(),
                    type_parameters: machine.type_parameters,
                    parameters: state.parameters,
                    return_type: state.return_type,
                    contracts: machine.contracts,
                    invokes: machine.invokes,
                    service_reach_row: machine.service_reach_row,
                    service_reach_is_installation_bound: machine
                        .service_reach_is_installation_bound,
                    suspends: machine.suspends,
                    blocks: machine.blocks,
                    termination_guarantee: match &machine.termination_plan.interface {
                        language_semantics::TerminationInterface::Published(guarantee) => {
                            guarantee.clone()
                        }
                        _ => language_semantics::TerminationGuarantee::NoGuarantee,
                    },
                    ..Default::default()
                };
                let mut substitution = self.materialize_trait_application(&environment)?;
                let signature = substitution.signature(self, &declared)?;
                self.applied_callable_signatures
                    .insert(AppliedCallableSignature {
                        owner: owner.clone(),
                        signature,
                    });
            }
            Ok(())
        }

        pub fn realization_call_signature(
            &self,
            lexical_owner: SymbolHandle,
            call: &crate::expression::TableCallExpression,
        ) -> Option<&signature::StateSignature> {
            let dispatch = call.static_requirement_dispatch.as_ref()?;
            if call.target_symbol != dispatch.realization_state {
                return None;
            }
            let selections = self.selected_conformance_applications(
                lexical_owner,
                dispatch.selection_application,
                dispatch.evidence_parameter,
                dispatch.requirement,
                dispatch.application_report_fingerprint,
                dispatch.application_commitment,
            );
            let [selection] = selections.as_slice() else {
                return None;
            };
            let application =
                self.portable_conformance_application_identity_in_owner(selection, lexical_owner)?;
            let owner = AppliedCallableOwner::Realization {
                declaration: selection.declaration,
                application,
                lexical_owner,
            };
            self.applied_callable_signatures
                .iter()
                .find_map(|(_, prepared)| {
                    (prepared.owner == owner && prepared.signature.symbol == call.target_symbol)
                        .then_some(&prepared.signature)
                })
        }
    }
}
pub(crate) mod requirement_instances {
    use super::{AppliedCallableOwner, AppliedCallableSignature, CallableApplication};
    use crate::type_identity;
    use crate::{TypedTrees, data, signature, static_applications, types};
    use arena::HandleSpan;
    use symbols::SymbolHandle;
    use type_identity::applied_type_identity;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub enum RequirementLocalArgument {
        Static(types::TypeReferenceHandle),
        RuntimeValue { carrier: types::TypeReferenceHandle },
    }

    impl Default for RequirementLocalArgument {
        fn default() -> Self {
            Self::Static(types::TypeReferenceHandle::invalid())
        }
    }

    impl RequirementLocalArgument {
        pub fn type_reference(self) -> types::TypeReferenceHandle {
            match self {
                Self::Static(reference) | Self::RuntimeValue { carrier: reference } => reference,
            }
        }

        pub fn static_operand(self) -> Option<types::TypeReferenceHandle> {
            match self {
                Self::Static(reference) => Some(reference),
                Self::RuntimeValue { .. } => None,
            }
        }

        fn portable_identity(
            self,
            program: &TypedTrees,
            binding: &static_applications::StaticMachineBindingValue,
            lexical_owner: SymbolHandle,
        ) -> Option<type_identity::NormalizedTypeIdentity> {
            // A selected machine and its entry state invoke the same callable.
            // Keep its complete checked application rather than its type spelling.
            if matches!(
                binding,
                static_applications::StaticMachineBindingValue::Machine(_)
            ) {
                type_identity::portable_applied_index_identity(
                    program,
                    type_identity::IndexIdentityArgument::TypeReference(self.type_reference()),
                    lexical_owner,
                    &[],
                )
            } else {
                type_identity::portable_applied_type_identity(
                    program,
                    self.type_reference(),
                    lexical_owner,
                    &[],
                )
            }
        }
    }

    #[derive(Clone, Copy)]
    pub struct ClosedRequirementApplication<'program> {
        pub declaring_trait: SymbolHandle,
        pub template: SymbolHandle,
        pub trait_application_identity: type_identity::NormalizedTypeIdentity,
        pub portable_trait_application_identity: type_identity::NormalizedTypeIdentity,
        pub portable_local_arguments: &'program [type_identity::NormalizedTypeIdentity],
        pub trait_scope: crate::trait_definition::RetainedTraitScope,
        pub trait_arguments: &'program [types::TypeReferenceHandle],
        pub local_arguments: &'program [RequirementLocalArgument],
        pub invocation_contract_parameters: &'program [data::TypeParameter],
        pub application: static_applications::StaticMachineApplicationHandle,
        pub signature: &'program signature::StateSignature,
    }

    impl TypedTrees {
        pub fn requirement_invocation_contract_parameters<'program>(
            &'program self,
            signature: &'program signature::StateSignature,
        ) -> &'program [data::TypeParameter] {
            self.closed_requirement_application(signature.symbol)
                .map(|application| application.invocation_contract_parameters)
                .unwrap_or_else(|| self.state_signature_type_parameters(signature))
        }

        pub fn closed_requirement_admission_signature(
            &self,
            instance: SymbolHandle,
        ) -> Option<signature::StateSignature> {
            let prepared = self.closed_requirement_signature(instance)?;
            let AppliedCallableOwner::RequirementInstance {
                invocation_contract_parameters,
                ..
            } = prepared.owner
            else {
                return None;
            };
            let mut admission = prepared.signature.clone();
            admission.type_parameters = invocation_contract_parameters;
            Some(admission)
        }

        pub fn closed_requirement_applications(
            &self,
        ) -> impl Iterator<Item = ClosedRequirementApplication<'_>> {
            self.applied_callable_signatures
                .iter()
                .filter_map(|(_, prepared)| self.closed_requirement_record(prepared))
        }

        pub fn closed_requirement_application(
            &self,
            instance: SymbolHandle,
        ) -> Option<ClosedRequirementApplication<'_>> {
            self.closed_requirement_record(self.closed_requirement_signature(instance)?)
        }

        fn closed_requirement_record<'program>(
            &'program self,
            prepared: &'program AppliedCallableSignature,
        ) -> Option<ClosedRequirementApplication<'program>> {
            let AppliedCallableOwner::RequirementInstance {
                declaration,
                template,
                trait_application_identity,
                portable_trait_application_identity,
                portable_local_arguments,
                trait_scope,
                trait_arguments,
                local_arguments,
                invocation_contract_parameters,
                application,
            } = prepared.owner
            else {
                return None;
            };
            Some(ClosedRequirementApplication {
                declaring_trait: declaration,
                template,
                trait_application_identity,
                portable_trait_application_identity,
                portable_local_arguments: self
                    .callable_argument_identities
                    .span_or_empty(portable_local_arguments),
                trait_scope,
                trait_arguments: self.type_reference_handles(trait_arguments),
                local_arguments: self.callable_arguments.span_or_empty(local_arguments),
                invocation_contract_parameters: self
                    .data_type_parameters
                    .span_or_empty(invocation_contract_parameters),
                application,
                signature: &prepared.signature,
            })
        }

        pub fn requirement_application_matches_scope<'program>(
            &'program self,
            requirement: SymbolHandle,
            scope: &crate::trait_definition::AppliedTraitScope<'program>,
        ) -> bool {
            let Some(application) = self.closed_requirement_application(requirement) else {
                return false;
            };
            let Some(declaration) = self.trait_definition_by_symbol(application.declaring_trait)
            else {
                return false;
            };
            scope
                .declaring_scope(self, declaration.symbol)
                .and_then(|actual| actual.application_identity(self))
                == Some(application.trait_application_identity)
        }

        pub fn requirement_contract_scopes_match(
            &self,
            left: &crate::trait_definition::AppliedTraitScope<'_>,
            right: &crate::trait_definition::AppliedTraitScope<'_>,
        ) -> bool {
            left.callable_contract_key(self)
                .zip(right.callable_contract_key(self))
                .is_some_and(|(left, right)| left == right)
        }

        pub fn requirement_application_matches_contract_scope<'program>(
            &'program self,
            requirement: SymbolHandle,
            scope: &crate::trait_definition::AppliedTraitScope<'program>,
        ) -> bool {
            self.requirement_application_matches_scope(requirement, scope)
                && self
                    .closed_requirement_application(requirement)
                    .and_then(|application| application.trait_scope.scope(self))
                    .is_some_and(|retained| {
                        self.requirement_contract_scopes_match(&retained, scope)
                    })
        }

        pub fn requirement_realization_specialization(
            &self,
            requirement: SymbolHandle,
            realization: SymbolHandle,
            provider: SymbolHandle,
        ) -> Option<&crate::commitments::MachineSpecialization> {
            let application = self.closed_requirement_application(requirement)?;
            let arguments = self.static_machine_application_arguments(application.application);
            let source_owner = self
                .static_machine_application(application.application)
                .lexical_owner;
            let mut matches = self.machine_specializations.iter().filter(|receipt| {
            if receipt.template != realization
                || receipt.instance != provider
                || !receipt.binding_application.is_valid()
            {
                return false;
            }
            let Some(declaration) = self
                .machine_declaration_for_lexical_scope(realization)
                .filter(|declaration| declaration.type_parameters == receipt.template_parameters)
            else {
                return false;
            };
            let declared_parameters = self.machine_callable_parameters(declaration);
            let parameters =
                self.requirement_callable_parameters(application.signature, &declared_parameters);
            let bindings = self.static_machine_application_bindings(receipt.binding_application);
            let binding_owner = self
                .static_machine_application(receipt.binding_application)
                .lexical_owner;
            let same_regions = self
                .static_application_lifetimes_for_declaration(application.application, realization)
                .ok()
                .zip(
                    self.static_application_lifetimes_for_declaration(
                        receipt.binding_application,
                        realization,
                    )
                    .ok(),
                )
                .is_some_and(|(source, selected)| {
                    source == selected && (source.is_empty() || source_owner == binding_owner)
                });
            same_regions
                && parameters.len() == arguments.len()
                && bindings.len() == parameters.len()
                && parameters
                    .iter()
                    .zip(arguments)
                    .enumerate()
                    .all(|(ordinal, (parameter, argument))| {
                        let mut actual = bindings
                            .iter()
                            .filter(|binding| binding.parameter == parameter.symbol);
                        let Some(binding) = actual.next() else {
                            return false;
                        };
                        actual.next().is_none()
                            && match application.local_arguments.get(ordinal) {
                                Some(RequirementLocalArgument::RuntimeValue { carrier }) => {
                                    let static_applications::StaticMachineBindingValue::RuntimeValue {
                                        type_reference, subject, expression,
                                    } = binding.value else { return false; };
                                    matches!(parameter.kind, data::TypeParameterKind::Value { .. })
                                        && !subject.is_valid() && !expression.is_valid()
                                        && applied_type_identity(self, *carrier, source_owner, &[])
                                            .zip(applied_type_identity(self, type_reference, binding_owner, &[]))
                                            .is_some_and(|(expected, actual)| expected == actual)
                                        && self.machine_by_symbol(provider)
                                            .and_then(|machine| self.machine_states(machine).first())
                                            .is_some_and(|state| {
                                                self.state_parameters(state).iter()
                                                    .filter(|formal| self.symbols.get(formal.symbol).generated_from == parameter.symbol)
                                                    .count() == 1
                                            })
                                }
                                _ => self.static_binding_matches_argument(
                                &parameter.kind,
                                &binding.value,
                                argument,
                                source_owner,
                                binding_owner,
                            ),
                            }
                    })
        });
            let receipt = matches.next()?;
            matches.next().is_none().then_some(receipt)
        }

        pub fn requirement_realization_matches(
            &self,
            requirement: SymbolHandle,
            realization: SymbolHandle,
            provider: SymbolHandle,
        ) -> bool {
            let Some(application) = self.closed_requirement_application(requirement) else {
                return false;
            };
            let Some(path) = self.machine_instantiation_path(provider, realization) else {
                return false;
            };
            for receipt in &path {
                if let super::MachineInstantiation::Capture(capture) = receipt {
                    let Some(scope) = capture.conformance_application.trait_scope.scope(self)
                    else {
                        return false;
                    };
                    if !self.requirement_application_matches_scope(requirement, &scope) {
                        return false;
                    }
                }
            }
            if application.local_arguments.is_empty() {
                return !path.iter().any(|receipt| {
                    matches!(receipt, super::MachineInstantiation::Specialization(_))
                }) && self
                    .machine(provider)
                    .is_some_and(|machine| self.machine_type_parameters(machine).is_empty());
            }
            let mut substitutions = path.iter().filter_map(|receipt| match receipt {
                super::MachineInstantiation::Specialization(receipt) => Some(*receipt),
                _ => None,
            });
            let Some(substitution) = substitutions.next() else {
                return false;
            };
            substitutions.next().is_none()
                && self
                    .requirement_realization_specialization(
                        requirement,
                        substitution.template,
                        substitution.instance,
                    )
                    .is_some()
        }

        pub fn instantiate_requirement_application(
            &mut self,
            environment: &crate::trait_definition::TraitApplicationEnvironment,
            template: SymbolHandle,
            trait_arguments: &[types::TypeReferenceHandle],
            application: static_applications::StaticMachineApplicationHandle,
        ) -> Result<SymbolHandle, &'static str> {
            if !application.is_valid() {
                return Err("closed requirement has no retained source application");
            }
            let source = self.static_machine_application(application);
            if source.registration_operation != template || source.binding_declaration != template {
                return Err("closed requirement application names a different declaration");
            }
            let lexical_owner = source.lexical_owner;
            let bindings = self
                .static_machine_application_bindings(application)
                .to_vec();
            let local_arguments = bindings
                .iter()
                .map(|binding| match &binding.value {
                    static_applications::StaticMachineBindingValue::Type(actual)
                    | static_applications::StaticMachineBindingValue::Const(actual) => self
                        .static_operand_type_reference_in_scope(
                            &crate::expression::StaticMachineArgument {
                                type_reference: *actual,
                                ..Default::default()
                            },
                            lexical_owner,
                        )
                        .map(RequirementLocalArgument::Static),
                    static_applications::StaticMachineBindingValue::Machine(actual)
                    | static_applications::StaticMachineBindingValue::Conformance(actual) => self
                        .static_operand_type_reference_in_scope(actual, lexical_owner)
                        .map(RequirementLocalArgument::Static),
                    static_applications::StaticMachineBindingValue::RuntimeValue {
                        type_reference,
                        ..
                    } => Ok(RequirementLocalArgument::RuntimeValue {
                        carrier: *type_reference,
                    }),
                })
                .collect::<Result<Vec<_>, _>>()?;
            let receiving_scope = environment
                .scope(self)
                .ok_or("closed requirement lost its declaring trait scope")?;
            let declaring_trait = receiving_scope.declaration.symbol;
            let refinements =
                receiving_scope.callable_refinement_clauses(self, self.symbols.name(template));
            self.require_receiving_trait_operands(&receiving_scope)?;
            self.require_owned_application_operands(
                &local_arguments
                    .iter()
                    .map(|argument| argument.type_reference())
                    .collect::<Vec<_>>(),
                lexical_owner,
            )?;
            let trait_application_identity = receiving_scope
                .application_identity(self)
                .ok_or("closed requirement has no complete declaring application identity")?;
            let portable_trait_application_identity = receiving_scope
                .portable_application_identity(self)
                .ok_or("closed requirement has no complete portable declaring application")?;
            let portable_local_arguments = local_arguments
                .iter()
                .zip(&bindings)
                .map(|(argument, binding)| {
                    argument
                        .portable_identity(self, &binding.value, lexical_owner)
                        .ok_or("closed requirement has no complete portable local operand")
                })
                .collect::<Result<Vec<_>, _>>()?;
            if let Some(existing) = self.closed_requirement_applications().find(|existing| {
                existing.declaring_trait == declaring_trait
                    && existing.template == template
                    && self.requirement_application_matches_contract_scope(
                        existing.signature.symbol,
                        &receiving_scope,
                    )
                    && existing.trait_application_identity == trait_application_identity
                    && existing.portable_trait_application_identity
                        == portable_trait_application_identity
                    && existing.portable_local_arguments == portable_local_arguments
                    && existing
                        .local_arguments
                        .iter()
                        .zip(&local_arguments)
                        .all(|(left, right)| {
                            std::mem::discriminant(left) == std::mem::discriminant(right)
                        })
                    && self
                        .static_machine_application(existing.application)
                        .lifetime_binding_declaration
                        == self
                            .static_machine_application(application)
                            .lifetime_binding_declaration
                    && self.static_machine_application_lifetimes(existing.application)
                        == self.static_machine_application_lifetimes(application)
                    && (self
                        .static_machine_application_lifetimes(application)
                        .is_empty()
                        || self
                            .static_machine_application(existing.application)
                            .lexical_owner
                            == lexical_owner)
                    && self
                        .static_machine_application_bindings(existing.application)
                        .len()
                        == bindings.len()
                    && self
                        .static_machine_application_bindings(existing.application)
                        .iter()
                        .zip(&bindings)
                        .all(|(left, right)| {
                            left.parameter == right.parameter && match (&left.value, &right.value) {
                                (
                                    static_applications::StaticMachineBindingValue::Type(left),
                                    static_applications::StaticMachineBindingValue::Type(right),
                                )
                                | (
                                    static_applications::StaticMachineBindingValue::Const(left),
                                    static_applications::StaticMachineBindingValue::Const(right),
                                ) => {
                                    let existing_owner = self
                                        .static_machine_application(existing.application)
                                        .lexical_owner;
                                    applied_type_identity(self, *left, existing_owner, &[])
                                        .zip(applied_type_identity(
                                            self,
                                            *right,
                                            lexical_owner,
                                            &[],
                                        ))
                                        .is_some_and(|(left, right)| left == right)
                                }
                                (
                                    static_applications::StaticMachineBindingValue::RuntimeValue {
                                        type_reference: left,
                                        ..
                                    },
                                    static_applications::StaticMachineBindingValue::RuntimeValue {
                                        type_reference: right,
                                        ..
                                    },
                                ) => applied_type_identity(self, *left, lexical_owner, &[])
                                    .zip(applied_type_identity(self, *right, lexical_owner, &[]))
                                    .is_some_and(|(left, right)| left == right),
                                // Without this arm two calls selecting one operand
                                // each get their own instance under one canonical
                                // identity, which lowering rejects as a duplicate.
                                (
                                    static_applications::StaticMachineBindingValue::Machine(left),
                                    static_applications::StaticMachineBindingValue::Machine(right),
                                )
                                | (
                                    static_applications::StaticMachineBindingValue::Conformance(
                                        left,
                                    ),
                                    static_applications::StaticMachineBindingValue::Conformance(
                                        right,
                                    ),
                                ) => self.same_static_machine_operand(
                                    left,
                                    self.static_machine_application(existing.application)
                                        .lexical_owner,
                                    right,
                                    lexical_owner,
                                ),
                                (left, right) => left == right,
                            }
                        })
            }) {
                return Ok(existing.signature.symbol);
            }
            let mut declared = self
                .trait_machine_signature(template)
                .filter(|(owner, _)| owner.symbol == declaring_trait)
                .map(|(_, signature)| signature.clone())
                .ok_or("closed requirement lost its authored signature")?;
            self.apply_callable_refinements(&mut declared, &refinements)?;
            let outer_parameters = self
                .trait_definition_by_symbol(declaring_trait)
                .map(|owner| self.trait_type_parameters(owner).to_vec())
                .ok_or("closed requirement lost its declaring trait")?;
            if outer_parameters.len() != trait_arguments.len() {
                return Err("closed requirement has an incomplete trait application");
            }
            let parameters = self.state_signature_type_parameters(&declared).to_vec();
            if parameters.len() != bindings.len()
                || parameters
                    .iter()
                    .zip(&bindings)
                    .any(|(parameter, binding)| parameter.symbol != binding.parameter)
            {
                return Err(
                    "closed requirement needs exactly one binding per authored local parameter",
                );
            }
            let mut substitutions = CallableApplication {
                region_scope: template,
                static_argument_owner: lexical_owner,
                trait_region_application: Some((environment.clone(), environment.frame_count())),
                source_binders: outer_parameters
                    .iter()
                    .map(|parameter| (parameter.name.clone(), parameter.symbol))
                    .collect(),
                types: outer_parameters
                    .iter()
                    .zip(trait_arguments)
                    .map(|(parameter, argument)| (parameter.symbol, *argument))
                    .collect(),
                ..CallableApplication::default()
            };
            for (parameter, binding) in parameters.iter().zip(&bindings) {
                match (&parameter.kind, &binding.value) {
                    (
                        data::TypeParameterKind::Type,
                        static_applications::StaticMachineBindingValue::Type(actual),
                    )
                    | (
                        data::TypeParameterKind::Const { .. }
                        | data::TypeParameterKind::Value { .. },
                        static_applications::StaticMachineBindingValue::Const(actual),
                    ) if actual.is_valid() => {
                        substitutions.types.push((parameter.symbol, *actual));
                    }
                    (
                        data::TypeParameterKind::Machine { .. },
                        static_applications::StaticMachineBindingValue::Machine(actual),
                    )
                    | (
                        data::TypeParameterKind::Implementation,
                        static_applications::StaticMachineBindingValue::Conformance(actual),
                    ) => {
                        substitutions
                            .static_arguments
                            .push((parameter.symbol, actual.clone()));
                    }
                    (
                        data::TypeParameterKind::Value { .. },
                        static_applications::StaticMachineBindingValue::RuntimeValue { .. },
                    ) => substitutions.runtime_values.push(parameter.symbol),
                    _ => {
                        return Err(
                            "closed requirement binding has no represented complete static operand",
                        );
                    }
                }
            }
            let instance = self.symbols.insert_generated_root_from(
                template,
                self.symbols.get(template).kind,
                declared.name.as_str(),
            );
            substitutions.symbols.push((template, instance));
            substitutions.owned_signature = instance;
            let mut closed = substitutions.signature(self, &declared)?;
            let invocation_contract_parameters = closed.type_parameters;
            closed.type_parameters = HandleSpan::empty();
            let trait_arguments = self
                .type_reference_table
                .insert_type_reference_handles(trait_arguments.iter().copied());
            let local_arguments = self.callable_arguments.insert_many(local_arguments);
            let trait_scope = environment.retain(self)?;
            let portable_local_arguments = self
                .callable_argument_identities
                .insert_many(portable_local_arguments);
            self.applied_callable_signatures
                .insert(AppliedCallableSignature {
                    owner: AppliedCallableOwner::RequirementInstance {
                        declaration: declaring_trait,
                        template,
                        trait_application_identity,
                        portable_trait_application_identity,
                        portable_local_arguments,
                        trait_scope,
                        trait_arguments,
                        local_arguments,
                        invocation_contract_parameters,
                        application,
                    },
                    signature: closed,
                });
            Ok(instance)
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::{expression, machine::Machine, name::Identifier, state::State};
        use symbols::{SymbolKind, SymbolNameRef, SymbolTableBuilder};

        #[test]
        fn requirement_machine_operands_keep_callable_identity_across_entry_selection() {
            let mut symbols = SymbolTableBuilder::default();
            let selected =
                symbols.insert_root(SymbolKind::Machine, SymbolNameRef::Static("Selected"));
            let other = symbols.insert_root(SymbolKind::Machine, SymbolNameRef::Static("Other"));
            let missing =
                symbols.insert_root(SymbolKind::Machine, SymbolNameRef::Static("Missing"));
            let symbolic =
                symbols.insert_root(SymbolKind::TypeParameter, SymbolNameRef::Static("Callback"));
            let states = symbols
                .insert_children(
                    selected,
                    [
                        (SymbolKind::State, SymbolNameRef::Static("entry")),
                        (SymbolKind::State, SymbolNameRef::Static("later")),
                    ],
                )
                .handles()
                .collect::<Vec<_>>();
            let other_entry = symbols
                .insert_children(other, [(SymbolKind::State, SymbolNameRef::Static("entry"))])
                .start();
            let orphan = symbols.insert_root(SymbolKind::State, SymbolNameRef::Static("orphan"));
            let mut program = TypedTrees {
                symbols: symbols.finish(),
                ..Default::default()
            };
            let unit = program
                .type_reference_table
                .insert(types::TypeReferenceNode::Unit);
            for (symbol, name, state_symbols) in [
                (selected, "Selected", states.as_slice()),
                (other, "Other", std::slice::from_ref(&other_entry)),
            ] {
                let mut machine = Machine {
                    symbol,
                    name: Identifier::generated(name),
                    ..Default::default()
                };
                for state in state_symbols {
                    program.push_machine_state(
                        &mut machine,
                        State {
                            symbol: *state,
                            name: Identifier::from(program.symbols.name(*state)),
                            return_type: unit,
                            ..Default::default()
                        },
                    );
                }
                program.push_machine(machine);
            }
            let machine_binding = static_applications::StaticMachineBindingValue::Machine(
                expression::StaticMachineArgument::default(),
            );
            let references = [
                selected,
                states[0],
                states[1],
                other,
                other_entry,
                missing,
                orphan,
                symbolic,
            ]
            .map(|symbol| {
                let name = Identifier::from(program.symbols.name(symbol));
                program
                    .type_reference_table
                    .insert(types::TypeReferenceNode::Named { symbol, name })
            });
            let identities = references.map(|reference| {
                RequirementLocalArgument::Static(reference).portable_identity(
                    &program,
                    &machine_binding,
                    selected,
                )
            });
            assert!(identities[0].is_some());
            assert_eq!(identities[0], identities[1]);
            assert_ne!(identities[0], identities[2]);
            assert_ne!(identities[0], identities[3]);
            assert_eq!(identities[3], identities[4]);
            assert!(
                identities[5].is_none(),
                "a missing machine is not an executable callable"
            );
            assert!(
                identities[6].is_none(),
                "a state without its machine must refuse"
            );
            assert!(
                identities[7].is_some(),
                "symbolic machine binders retain their identity"
            );
            assert_ne!(identities[0], identities[7]);

            let application = |program: &mut TypedTrees, argument| {
                let arguments = program
                    .type_reference_table
                    .insert_type_reference_handles([argument]);
                program
                    .type_reference_table
                    .insert(types::TypeReferenceNode::Generic {
                        base_symbol: selected,
                        base_name: Identifier::generated("Selected"),
                        lifetime_arguments: Vec::new(),
                        arguments,
                    })
            };
            let first = application(&mut program, unit);
            let second = application(&mut program, references[3]);
            let identity = |reference| {
                RequirementLocalArgument::Static(reference).portable_identity(
                    &program,
                    &machine_binding,
                    selected,
                )
            };
            assert!(identity(first).is_some());
            assert_ne!(identity(first), identity(second));
            assert_ne!(identity(first), identities[0]);
            let ordinary = static_applications::StaticMachineBindingValue::Type(references[0]);
            assert_eq!(
                RequirementLocalArgument::Static(references[0])
                    .portable_identity(&program, &ordinary, selected),
                type_identity::portable_applied_type_identity(
                    &program,
                    references[0],
                    selected,
                    &[]
                )
            );
        }
    }
}
pub(crate) mod selected_operands {
    use super::CallableApplication;
    use crate::{TypedTrees, expression, types};

    impl TypedTrees {
        pub fn static_operand_region_supplier(
            &self,
            selected: &expression::StaticMachineArgument,
            lexical_owner: symbols::SymbolHandle,
        ) -> Result<symbols::SymbolHandle, &'static str> {
            use crate::type_identity::{IndexIdentityArgument, applied_index_identity};

            let reference = selected.type_reference;
            if !reference.is_valid() {
                return Ok(lexical_owner);
            }
            if !self.contains_type_reference(reference) {
                return Err("static operand lost its retained source type");
            }
            let supplier = self.type_reference_table.operand_lifetime_owner(reference);
            if !supplier.is_valid() {
                if !self
                    .type_reference_table
                    .operand_lifetimes(reference)
                    .is_empty()
                    || !matches!(
                        self.type_reference_table.operand_origin(reference),
                        types::TypeOperandOrigin::Lexical
                    )
                {
                    return Err("captured static operand lost its supplying region owner");
                }
                return Ok(lexical_owner);
            }

            // A copied call can use lifetimes from the declaration that supplied
            // its selected implementation. Check that the retained selection
            // still matches before resolving those lifetimes there.
            let mut explicit = selected.clone();
            explicit.type_reference = Default::default();
            let retained = applied_index_identity(
                self,
                IndexIdentityArgument::StaticArgument(selected),
                supplier,
                &[],
            );
            let supplied = applied_index_identity(
                self,
                IndexIdentityArgument::StaticArgument(&explicit),
                supplier,
                &[],
            );
            if retained.is_none() || retained != supplied {
                return Err("static operand changed its retained selected application");
            }
            Ok(supplier)
        }

        pub fn static_operand_from_type_reference(
            &self,
            reference: types::TypeReferenceHandle,
        ) -> Result<expression::StaticMachineArgument, &'static str> {
            use expression::{ExpressionNode, StaticMachineArgument, StaticSymbolApplication};
            use types::TypeReferenceNode;

            if !reference.is_valid()
                || !self.type_reference_table.contains_type_reference(reference)
            {
                return Err("static operand has no retained type payload");
            }
            if self
                .type_reference_table
                .type_alias_applications(reference)
                .next()
                .is_some()
            {
                return Ok(StaticMachineArgument {
                    type_reference: reference,
                    ..StaticMachineArgument::default()
                });
            }
            if let Some(literal) = self.static_const_literal_from_type_reference(reference) {
                return Ok(StaticMachineArgument {
                    const_literal: Some(literal),
                    ..StaticMachineArgument::default()
                });
            }
            match self.type_reference_table.type_reference(reference) {
                TypeReferenceNode::Named { symbol, name } => Ok(StaticMachineArgument {
                    symbol: *symbol,
                    path: Box::from([name.clone()]),
                    ..StaticMachineArgument::default()
                }),
                TypeReferenceNode::Generic {
                    base_symbol,
                    base_name,
                    lifetime_arguments,
                    arguments,
                } if self.symbols.get(*base_symbol).kind != symbols::SymbolKind::Data => {
                    let arguments = self
                        .type_reference_handles(*arguments)
                        .iter()
                        .map(|argument| self.static_operand_from_type_reference(*argument))
                        .collect::<Result<Vec<_>, _>>()?;
                    Ok(StaticMachineArgument {
                        symbol: *base_symbol,
                        path: Box::from([base_name.clone()]),
                        application: Some(Box::new(StaticSymbolApplication {
                            member: arena::Handle::invalid(),
                            lifetime_arguments: lifetime_arguments.clone().into_boxed_slice(),
                            arguments: arguments.into_boxed_slice(),
                        })),
                        ..StaticMachineArgument::default()
                    })
                }
                TypeReferenceNode::ConstExpression(value) => {
                    match self.expression_table.expression(*value) {
                        ExpressionNode::Name(path) => Ok(StaticMachineArgument {
                            symbol: path.resolved_symbol().unwrap_or_default(),
                            path: self
                                .expression_table
                                .name_path_members(path.members)
                                .to_vec()
                                .into_boxed_slice(),
                            ..StaticMachineArgument::default()
                        }),
                        ExpressionNode::Boolean(value) => Ok(StaticMachineArgument {
                            path: Box::from([crate::name::Identifier::generated(
                                language_semantics::const_value::CanonicalConstValue::boolean(
                                    *value,
                                )
                                .atom(),
                            )]),
                            ..StaticMachineArgument::default()
                        }),
                        _ => Ok(StaticMachineArgument {
                            type_reference: reference,
                            ..StaticMachineArgument::default()
                        }),
                    }
                }
                _ => Ok(StaticMachineArgument {
                    type_reference: reference,
                    ..StaticMachineArgument::default()
                }),
            }
        }

        pub fn static_const_literal_from_type_reference(
            &self,
            reference: types::TypeReferenceHandle,
        ) -> Option<numerics::literals::IntegerLiteral> {
            use numerics::literals::{IntegerLiteral, IntegerRadix};
            match self.type_reference_table.type_reference(reference) {
                types::TypeReferenceNode::ConstExpression(value) => {
                    let expression::ExpressionNode::Integer(literal) =
                        self.expression_table.expression(*value)
                    else {
                        return None;
                    };
                    Some(literal.clone())
                }
                types::TypeReferenceNode::Named { symbol, name } if !symbol.is_valid() => {
                    let mut spelling = name.as_str();
                    let negative = spelling.starts_with('-');
                    if negative {
                        spelling = &spelling[1..];
                    }
                    let (radix, digits) = if let Some(digits) = spelling.strip_prefix("0b") {
                        (IntegerRadix::Binary, digits)
                    } else if let Some(digits) = spelling.strip_prefix("0o") {
                        (IntegerRadix::Octal, digits)
                    } else if let Some(digits) = spelling.strip_prefix("0x") {
                        (IntegerRadix::Hexadecimal, digits)
                    } else {
                        (IntegerRadix::Decimal, spelling)
                    };
                    IntegerLiteral::from_parts(negative, radix, digits).ok()
                }
                _ => None,
            }
        }

        pub fn static_operand_type_reference(
            &mut self,
            selected: &expression::StaticMachineArgument,
        ) -> Result<types::TypeReferenceHandle, &'static str> {
            CallableApplication::selected_argument_reference(self, selected)
        }

        pub fn static_operand_type_reference_in_scope(
            &mut self,
            selected: &expression::StaticMachineArgument,
            owner: symbols::SymbolHandle,
        ) -> Result<types::TypeReferenceHandle, &'static str> {
            if !owner.is_valid() {
                return Err("static operand lost its supplying declaration");
            }
            let start = self.type_reference_table.type_reference_count() + 1;
            let supplied = CallableApplication::selected_argument_reference(self, selected)?;
            let copied = self.apply_callable_type(supplied, &[], &[], owner)?;
            let references = self
                .type_reference_table
                .references()
                .filter(|(handle, _)| handle.arena_index() as usize >= start)
                .map(|(handle, _)| handle)
                .collect::<Vec<_>>();
            for reference in references {
                if !self
                    .type_reference_table
                    .operand_lifetime_owner(reference)
                    .is_valid()
                {
                    self.type_reference_table
                        .retain_operand_lifetime_owner(reference, owner)?;
                }
            }
            Ok(copied)
        }
    }

    impl CallableApplication {
        pub(crate) fn selected_value_expression(
            program: &mut TypedTrees,
            reference: types::TypeReferenceHandle,
            carrier: types::TypeReferenceHandle,
        ) -> Result<expression::ExpressionHandle, &'static str> {
            if let Some(literal) = program.static_const_literal_from_type_reference(reference) {
                return Ok(program
                    .expression_table
                    .insert(expression::ExpressionNode::Integer(literal)));
            }
            match program.type_reference_node(reference).clone() {
                types::TypeReferenceNode::ConstExpression(value) => {
                    Ok(program.expression_table.insert_copy(value))
                }
                types::TypeReferenceNode::Named { symbol, name } if !symbol.is_valid() => {
                    let value = language_semantics::const_value::CanonicalConstValue::from_atom(
                        name.as_str(),
                    )
                    .ok_or("callable expression argument has no represented canonical value")?;
                    let decoded = value
                        .decode_encoding()
                        .ok_or("callable canonical value is malformed")?;
                    if let language_semantics::const_value::DecodedCanonicalConstValue::Boolean(
                        value,
                    ) = decoded
                    {
                        return Ok(program
                            .expression_table
                            .insert(expression::ExpressionNode::Boolean(value)));
                    }
                    if !carrier.is_valid()
                        || program.normalized_type_identity_text(carrier) != value.type_name
                    {
                        return Err("callable canonical value lost its exact declared carrier");
                    }
                    let value = program
                        .materialize_canonical_const_value(carrier, &decoded)
                        .ok_or("callable canonical value does not match its declared schema")?;
                    Ok(program.expression_table.insert(value))
                }
                types::TypeReferenceNode::Named { symbol, name } if symbol.is_valid() => {
                    if let Some(declaration) = program.const_declaration_by_symbol(symbol) {
                        if declaration.evaluated_value.is_some()
                            && declaration.materialized_initializer.is_valid()
                        {
                            let initializer = declaration.materialized_initializer;
                            return Ok(program.expression_table.insert_copy(initializer));
                        }
                    } else if !program
                        .data_type_parameter_by_symbol(symbol)
                        .is_some_and(|parameter| parameter.kind.is_const_or_value())
                    {
                        return Err(
                            "callable expression argument does not name a const or value declaration",
                        );
                    }
                    Ok(program
                        .expression_table
                        .insert_tree(&expression::Expression::Name(
                            expression::NamePath::resolved_from_iter([name], symbol, symbol),
                        )))
                }
                _ => Err(
                    "nominal callable expression application needs an expression-valued argument",
                ),
            }
        }

        pub(crate) fn selected_argument_reference(
            program: &mut TypedTrees,
            selected: &expression::StaticMachineArgument,
        ) -> Result<types::TypeReferenceHandle, &'static str> {
            if selected
                .application
                .as_ref()
                .is_some_and(|application| application.member.is_valid())
            {
                return Err(
                    "qualified static member must resolve in its exact applied owner before type materialization",
                );
            }
            if selected.type_reference.is_valid() {
                return Ok(selected.type_reference);
            }
            if let Some(literal) = &selected.const_literal {
                let expression = program
                    .expression_table
                    .insert(expression::ExpressionNode::Integer(literal.clone()));
                return Ok(program
                    .type_reference_table
                    .insert(types::TypeReferenceNode::ConstExpression(expression)));
            }
            if let Some(spelling) = crate::static_const_argument_spelling(program, selected)
                && let Some(value) =
                    language_semantics::const_value::CanonicalConstValue::from_atom(&spelling)
            {
                let node = match value
                    .decode_encoding()
                    .ok_or("applied callable canonical value is malformed")?
                {
                    language_semantics::const_value::DecodedCanonicalConstValue::Boolean(value) => {
                        let value = program
                            .expression_table
                            .insert(expression::ExpressionNode::Boolean(value));
                        types::TypeReferenceNode::ConstExpression(value)
                    }
                    _ => types::TypeReferenceNode::Named {
                        symbol: symbols::SymbolHandle::invalid(),
                        name: crate::name::Identifier::generated(value.atom()),
                    },
                };
                return Ok(program.type_reference_table.insert(node));
            }
            if !selected.symbol.is_valid() || selected.evidence_projection.is_some() {
                return Err("applied callable has an unrepresented static type operand");
            }
            let name = selected
                .path
                .last()
                .cloned()
                .ok_or("applied callable lost its selected operand's diagnostic name")?;
            let node = if let Some(application) = &selected.application {
                let arguments = application
                    .arguments
                    .iter()
                    .map(|argument| Self::selected_argument_reference(program, argument))
                    .collect::<Result<Vec<_>, _>>()?;
                types::TypeReferenceNode::Generic {
                    base_symbol: selected.symbol,
                    base_name: name,
                    lifetime_arguments: application.lifetime_arguments.to_vec(),
                    arguments: program
                        .type_reference_table
                        .insert_type_reference_handles(arguments),
                }
            } else {
                types::TypeReferenceNode::Named {
                    symbol: selected.symbol,
                    name,
                }
            };
            Ok(program.type_reference_table.insert(node))
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn supplied_operand_does_not_acquire_preexisting_region_node() {
            let mut program = TypedTrees::default();
            let unit = program
                .type_reference_table
                .insert(types::TypeReferenceNode::Unit);
            let incoming =
                program
                    .type_reference_table
                    .insert(types::TypeReferenceNode::Reference {
                        referee: unit,
                        access: language_semantics::ReferenceAccess::Shared,
                        lifetime: Some(crate::name::Identifier::generated("incoming")),
                    });
            let unrelated =
                program
                    .type_reference_table
                    .insert(types::TypeReferenceNode::Reference {
                        referee: unit,
                        access: language_semantics::ReferenceAccess::Shared,
                        lifetime: Some(crate::name::Identifier::generated("unrelated")),
                    });
            let owner = symbols::SymbolHandle::from_parts(1, 1);
            let selected = expression::StaticMachineArgument {
                type_reference: incoming,
                ..Default::default()
            };
            let copied = program
                .static_operand_type_reference_in_scope(&selected, owner)
                .unwrap();
            assert!(
                !program
                    .type_reference_table
                    .operand_lifetime_owner(unrelated)
                    .is_valid()
            );
            assert!(
                !program
                    .type_reference_table
                    .operand_lifetime_owner(incoming)
                    .is_valid()
            );
            assert_eq!(
                program.type_reference_table.operand_lifetime_owner(copied),
                owner
            );
        }
    }
}
pub(crate) mod signatures {
    use super::{CallableApplication, CallableLifetimeBinding, CallableLifetimeOrigin};
    use crate::{TypedTrees, data, domain, signature};
    use arena::{Handle, HandleSpan};

    impl TypedTrees {
        pub fn applied_signature_authored_parameters<'program>(
            &'program self,
            declared: &signature::StateSignature,
            applied: &'program signature::StateSignature,
        ) -> Option<&'program [signature::StateParameter]> {
            let count = self.state_signature_parameters(declared).len();
            let parameters = self.state_signature_parameters(applied);
            if count == parameters.len() {
                return Some(parameters);
            }
            let closed = self.closed_requirement_application(applied.symbol)?;
            if closed.template != declared.symbol {
                return None;
            }
            let runtime = closed
                .local_arguments
                .iter()
                .zip(closed.invocation_contract_parameters)
                .filter_map(|(argument, binder)| {
                    matches!(
                        argument,
                        super::RequirementLocalArgument::RuntimeValue { .. }
                    )
                    .then_some(binder.symbol)
                });
            if !parameters
                .get(count..)?
                .iter()
                .map(|parameter| parameter.symbol)
                .eq(runtime)
            {
                return None;
            }
            parameters.get(..count)
        }

        pub fn applied_signature_parameter(
            &self,
            declared: &signature::StateSignature,
            applied: &signature::StateSignature,
            parameter: symbols::SymbolHandle,
        ) -> Option<symbols::SymbolHandle> {
            let declared_types = self.requirement_invocation_contract_parameters(declared);
            let applied_types = self.requirement_invocation_contract_parameters(applied);
            if declared_types.len() == applied_types.len()
                && let Some(position) = declared_types
                    .iter()
                    .position(|candidate| candidate.symbol == parameter)
            {
                return Some(applied_types[position].symbol);
            }
            let declared_values = self.state_signature_parameters(declared);
            let applied_values = self.applied_signature_authored_parameters(declared, applied)?;
            declared_values
                .iter()
                .position(|candidate| candidate.symbol == parameter)
                .map(|position| applied_values[position].symbol)
        }

        pub fn applied_signature_contract_fact(
            &self,
            declared: &signature::StateSignature,
            applied: &signature::StateSignature,
            fact: Handle<domain::ProofFact>,
        ) -> Option<&domain::ProofFact> {
            let declared_contracts = self.state_signature_contracts(declared);
            let applied_contracts = self.state_signature_contracts(applied);
            if declared_contracts.len() != applied_contracts.len() {
                return None;
            }
            for (source, selected) in declared_contracts.iter().zip(applied_contracts) {
                if source.kind != selected.kind || source.facts.len() != selected.facts.len() {
                    return None;
                }
                for offset in 0..source.facts.len() {
                    let offset = u32::try_from(offset).ok()?;
                    if source.facts.handle_at(offset) == fact {
                        return self
                            .proof_facts
                            .span_or_empty(selected.facts)
                            .get(offset as usize);
                    }
                }
            }
            None
        }
    }

    impl CallableApplication {
        pub(crate) fn signature(
            &mut self,
            program: &mut TypedTrees,
            declared: &signature::StateSignature,
        ) -> Result<signature::StateSignature, &'static str> {
            let type_parameters = program.state_signature_type_parameters(declared).to_vec();
            let parameters = program.state_signature_parameters(declared).to_vec();
            let previous_symbols = self.symbols.len();
            let previous_lifetimes = self.lifetimes.len();
            let previous_binders = self.source_binders.len();
            let previous_region_scope = self.region_scope;
            self.region_scope = declared.symbol;
            let result = (|| {
                self.source_binders.extend(
                    type_parameters
                        .iter()
                        .map(|parameter| (parameter.name.clone(), parameter.symbol))
                        .chain(
                            parameters
                                .iter()
                                .map(|parameter| (parameter.name.clone(), parameter.symbol)),
                        ),
                );
                let selected_lifetime_telescope = declared.symbol == self.lifetime_declaration;
                if !selected_lifetime_telescope {
                    self.lifetimes
                        .extend(declared.lifetime_parameters.iter().enumerate().map(
                            |(ordinal, parameter)| CallableLifetimeBinding {
                                parameter: parameter.clone(),
                                argument: parameter.clone(),
                                region: crate::type_identity::LifetimeParameterIdentity {
                                    declaration: declared.symbol,
                                    ordinal,
                                },
                                origin: CallableLifetimeOrigin::Rigid,
                            },
                        ));
                }
                let pending = type_parameters
                    .iter()
                    .map(|parameter| (parameter.symbol, parameter.name.as_str()))
                    .chain(
                        parameters
                            .iter()
                            .map(|parameter| (parameter.symbol, parameter.name.as_str())),
                    )
                    .filter(|(symbol, _)| {
                        symbol.is_valid()
                            && !self.types.iter().any(|(parameter, _)| parameter == symbol)
                            && !self
                                .static_arguments
                                .iter()
                                .any(|(parameter, _)| parameter == symbol)
                    })
                    .map(|(symbol, name)| {
                        let kind = if self.runtime_values.contains(&symbol) {
                            symbols::SymbolKind::Parameter
                        } else {
                            program.symbols.get(symbol).kind
                        };
                        (symbol, kind, name.to_owned())
                    })
                    .collect::<Vec<_>>();
                let owner = self.symbol(declared.symbol);
                if owner.is_valid() && owner == self.owned_signature {
                    let children = program.symbols.insert_generated_children_from(
                        owner,
                        pending
                            .iter()
                            .map(|(symbol, kind, name)| (*symbol, *kind, name.as_str())),
                    );
                    for (offset, (symbol, _, _)) in (0u32..).zip(pending.iter()) {
                        self.symbols.push((*symbol, children.handle_at(offset)));
                    }
                } else {
                    for (symbol, kind, name) in pending {
                        let fresh = program
                            .symbols
                            .insert_generated_root_from(symbol, kind, &name);
                        self.symbols.push((symbol, fresh));
                    }
                }
                let mut applied = declared.clone();
                if selected_lifetime_telescope {
                    applied.lifetime_parameters.retain(|parameter| {
                        !self
                            .lifetimes
                            .iter()
                            .any(|binding| binding.parameter == *parameter)
                    });
                }
                applied.symbol = self.symbol(declared.symbol);
                let mut applied_parameters = Vec::with_capacity(type_parameters.len());
                for mut parameter in type_parameters {
                    parameter.symbol = self.symbol(parameter.symbol);
                    parameter.kind = self.parameter_kind(program, parameter.kind)?;
                    applied_parameters.push(parameter);
                }
                applied.type_parameters =
                    program.data_type_parameters.insert_many(applied_parameters);
                for bound in &mut applied.conformance_bounds {
                    bound.binder = self.symbol(bound.binder);
                    bound.arguments = bound
                        .arguments
                        .iter()
                        .map(|argument| self.type_reference(program, *argument))
                        .collect::<Result<Vec<_>, _>>()?;
                }
                applied.parameters = self.parameters(program, &parameters)?;
                for (declared, applied_parameter) in program
                    .state_signature_type_parameters(declared)
                    .iter()
                    .cloned()
                    .zip(
                        program
                            .state_signature_type_parameters(&applied)
                            .iter()
                            .cloned(),
                    )
                    .collect::<Vec<_>>()
                {
                    if !self.runtime_values.contains(&declared.symbol) {
                        continue;
                    }
                    let data::TypeParameterKind::Value { type_reference } = applied_parameter.kind
                    else {
                        return Err("runtime requirement projection is not a Value binder");
                    };
                    program.state_parameters.append_to_span(
                        &mut applied.parameters,
                        signature::StateParameter {
                            symbol: applied_parameter.symbol,
                            name: applied_parameter.name,
                            type_reference,
                            is_const: false,
                            is_mutable: false,
                            relevance: language_core::BindingRelevance::Relevant,
                        },
                    );
                }
                applied.return_type = self.type_reference(program, declared.return_type)?;
                applied.contracts = HandleSpan::empty();
                for mut contract in program.state_signature_contracts(declared).to_vec() {
                    contract.facts = self.facts(program, contract.facts)?;
                    program
                        .signature_contracts
                        .append_to_span(&mut applied.contracts, contract);
                }
                applied.invokes = HandleSpan::empty();
                for mut invocation in program.state_signature_invokes(declared).to_vec() {
                    invocation.target = match invocation.target {
                        signature::AuthoredInvocationTarget::Parameter { ordinal, symbol } => {
                            signature::AuthoredInvocationTarget::Parameter {
                                ordinal,
                                symbol: self.symbol(symbol),
                            }
                        }
                        signature::AuthoredInvocationTarget::Service(symbol) => {
                            signature::AuthoredInvocationTarget::Service(self.symbol(symbol))
                        }
                        signature::AuthoredInvocationTarget::Unresolved => {
                            signature::AuthoredInvocationTarget::Unresolved
                        }
                    };
                    program
                        .signature_invokes
                        .append_to_span(&mut applied.invokes, invocation);
                }
                if let language_semantics::TerminationGuarantee::Terminates { premises } =
                    &mut applied.termination_guarantee
                {
                    for premise in premises {
                        premise.subject.root = self.symbol(premise.subject.root);
                        for projection in &mut premise.subject.projections {
                            *projection = self.symbol(*projection);
                        }
                    }
                }
                Ok(applied)
            })();
            self.symbols.truncate(previous_symbols);
            self.lifetimes.truncate(previous_lifetimes);
            self.source_binders.truncate(previous_binders);
            self.region_scope = previous_region_scope;
            result
        }

        pub(crate) fn parameter_kind(
            &mut self,
            program: &mut TypedTrees,
            kind: data::TypeParameterKind,
        ) -> Result<data::TypeParameterKind, &'static str> {
            Ok(match kind {
                data::TypeParameterKind::Const { type_reference } => {
                    data::TypeParameterKind::Const {
                        type_reference: self.type_reference(program, type_reference)?,
                    }
                }
                data::TypeParameterKind::Value { type_reference } => {
                    data::TypeParameterKind::Value {
                        type_reference: self.type_reference(program, type_reference)?,
                    }
                }
                data::TypeParameterKind::Machine { contract } => {
                    let contract = match contract {
                        data::MachineParameterContract::Structural(declared) => {
                            data::MachineParameterContract::Structural(
                                self.signature(program, &declared)?,
                            )
                        }
                        data::MachineParameterContract::Nominal {
                            receiving_trait,
                            receiving_arguments,
                            trait_definition,
                            requirement,
                            trait_arguments,
                            instantiated,
                        } => {
                            let receiving_arguments = receiving_arguments
                                .into_iter()
                                .map(|argument| self.type_reference(program, argument))
                                .collect::<Result<Vec<_>, _>>()?;
                            let arguments = trait_arguments
                                .into_iter()
                                .map(|argument| self.type_reference(program, argument))
                                .collect::<Result<Vec<_>, _>>()?;
                            if let Some(signature) = instantiated {
                                data::MachineParameterContract::Nominal {
                                    receiving_trait,
                                    receiving_arguments,
                                    trait_definition,
                                    requirement,
                                    trait_arguments: arguments,
                                    instantiated: Some(Box::new(
                                        self.signature(program, &signature)?,
                                    )),
                                }
                            } else {
                                self.nominal_contract(
                                    program,
                                    receiving_trait,
                                    requirement,
                                    receiving_arguments,
                                )?
                            }
                        }
                        data::MachineParameterContract::RequirementIdentity => {
                            data::MachineParameterContract::RequirementIdentity
                        }
                    };
                    data::TypeParameterKind::Machine { contract }
                }
                data::TypeParameterKind::Proposition { mut contract } => {
                    let parameters = program
                        .state_parameters
                        .span_or_empty(contract.parameters)
                        .to_vec();
                    contract.parameters = self.parameters(program, &parameters)?;
                    data::TypeParameterKind::Proposition { contract }
                }
                data::TypeParameterKind::Type => data::TypeParameterKind::Type,
                data::TypeParameterKind::Implementation => data::TypeParameterKind::Implementation,
            })
        }

        fn parameters(
            &mut self,
            program: &mut TypedTrees,
            declared: &[signature::StateParameter],
        ) -> Result<HandleSpan<signature::StateParameter>, &'static str> {
            let mut parameters = HandleSpan::empty();
            let previous_symbols = self.symbols.len();
            for parameter in declared {
                if parameter.symbol.is_valid() && self.symbol(parameter.symbol) == parameter.symbol
                {
                    let kind = program.symbols.get(parameter.symbol).kind;
                    let fresh = program.symbols.insert_generated_root_from(
                        parameter.symbol,
                        kind,
                        parameter.name.as_str(),
                    );
                    self.symbols.push((parameter.symbol, fresh));
                }
            }
            for mut parameter in declared.iter().cloned() {
                parameter.symbol = self.symbol(parameter.symbol);
                parameter.type_reference =
                    self.type_reference(program, parameter.type_reference)?;
                program
                    .state_parameters
                    .append_to_span(&mut parameters, parameter);
            }
            self.symbols.truncate(previous_symbols);
            Ok(parameters)
        }

        pub(crate) fn facts(
            &mut self,
            program: &mut TypedTrees,
            declared: HandleSpan<domain::ProofFact>,
        ) -> Result<HandleSpan<domain::ProofFact>, &'static str> {
            let original = program.proof_facts.span_or_empty(declared).to_vec();
            if original.len() != declared.len() {
                return Err("applied callable has a stale proof-fact span");
            }
            let mut copied = Vec::with_capacity(original.len());
            for (offset, fact) in original.into_iter().enumerate() {
                let source_index = declared
                    .start()
                    .arena_index()
                    .checked_add(
                        u32::try_from(offset).map_err(|_| "applied proof-fact offset overflow")?,
                    )
                    .ok_or("applied proof-fact handle overflow")?;
                let source_fact = Handle::from_parts(source_index, declared.start().generation());
                let span = program.proof_fact_source_span(source_fact);
                let fact = match fact {
                    domain::ProofFact::Expression(expression) => {
                        domain::ProofFact::Expression(self.expression(program, expression)?)
                    }
                    domain::ProofFact::Membership(mut membership) => {
                        membership.value = self.expression(program, membership.value)?;
                        membership.domain_arguments =
                            self.type_arguments(program, membership.domain_arguments)?;
                        membership.domain_symbol = self.symbol(membership.domain_symbol);
                        let arguments = program
                            .type_reference_table
                            .type_reference_handles(membership.domain_arguments)
                            .to_vec();
                        membership.semantic_domain = Self::domain_identity(
                            program,
                            membership.domain_symbol,
                            &arguments,
                            membership.semantic_domain,
                        )?;
                        domain::ProofFact::Membership(membership)
                    }
                    domain::ProofFact::Proposition(mut application) => {
                        let arguments = program
                            .expression_table
                            .expression_handles(application.arguments)
                            .to_vec();
                        let arguments = arguments
                            .into_iter()
                            .map(|argument| self.expression(program, argument))
                            .collect::<Result<Vec<_>, _>>()?;
                        application.arguments = program
                            .expression_table
                            .insert_expression_handles(arguments);
                        if let Some((_, replacement)) = self
                            .types
                            .iter()
                            .rev()
                            .find(|(symbol, _)| *symbol == application.proposition)
                        {
                            let crate::types::TypeReferenceNode::Named { symbol, name } =
                                program.type_reference_table.type_reference(*replacement)
                            else {
                                return Err(
                                    "applied callable proposition head needs a named proposition operand",
                                );
                            };
                            application.proposition = *symbol;
                            application.name = name.clone();
                        } else {
                            application.proposition = self.symbol(application.proposition);
                        }
                        for argument in &mut application.binder_arguments {
                            if let Some((_, replacement)) = self
                                .types
                                .iter()
                                .rev()
                                .find(|(symbol, _)| *symbol == argument.symbol)
                            {
                                match program.type_reference_table.type_reference(*replacement) {
                                    crate::types::TypeReferenceNode::Named { symbol, name } => {
                                        argument.symbol = *symbol;
                                        argument.path = vec![name.clone()].into_boxed_slice();
                                    }
                                    crate::types::TypeReferenceNode::ConstExpression(
                                        expression,
                                    ) => {
                                        let crate::expression::ExpressionNode::Integer(value) =
                                            program.expression_table.expression(*expression)
                                        else {
                                            return Err(
                                                "nominal callable proposition operand needs a normalized const literal",
                                            );
                                        };
                                        argument.const_literal = Some(value.clone());
                                        argument.symbol = symbols::SymbolHandle::invalid();
                                        argument.path = Box::new([]);
                                    }
                                    _ => {
                                        return Err(
                                            "compound nominal callable proposition operands are not yet represented",
                                        );
                                    }
                                }
                            } else {
                                argument.symbol = self.symbol(argument.symbol);
                            }
                        }
                        domain::ProofFact::Proposition(application)
                    }
                };
                copied.push((fact, span));
            }
            let facts = program
                .proof_facts
                .insert_many(copied.iter().map(|(fact, _)| fact.clone()));
            for (offset, (_, source_span)) in copied.into_iter().enumerate() {
                if let Some(source_span) = source_span {
                    let index = facts
                        .start()
                        .arena_index()
                        .checked_add(
                            u32::try_from(offset)
                                .map_err(|_| "applied proof-fact offset overflow")?,
                        )
                        .ok_or("applied proof-fact handle overflow")?;
                    program.set_proof_fact_source_span(
                        Handle::from_parts(index, facts.start().generation()),
                        source_span,
                    );
                }
            }
            Ok(facts)
        }
    }
}
pub(crate) mod table {
    //! Repeated signature lookups should visit only the rows for that call.
    //! Producers keep separate groups for machine selections and requirement
    //! invocations, including each requirement's exact lexical owner. Inserts,
    //! replacement and retirement maintain these links before returning. Handles
    //! keep their generations, and row links preserve arena iteration order.

    use super::{AppliedCallableOwner, AppliedCallableSignature};
    use crate::signature::StateSignature;
    use crate::static_applications::StaticMachineApplicationHandle;
    use arena::{Arena, Handle};
    use symbols::SymbolHandle;

    type CallableRow = Handle<AppliedCallableSignature>;

    #[derive(Clone, Copy, Default, PartialEq, Eq)]
    enum InvocationPurpose {
        #[default]
        MachineSelection,
        Requirement(SymbolHandle),
    }

    fn invocation_owner(
        row: &AppliedCallableSignature,
    ) -> Option<(StaticMachineApplicationHandle, InvocationPurpose)> {
        match row.owner {
            AppliedCallableOwner::MachineSelection { invocation, .. } => {
                Some((invocation, InvocationPurpose::MachineSelection))
            }
            AppliedCallableOwner::RequirementInvocation {
                application,
                lexical_owner,
                ..
            } => Some((application, InvocationPurpose::Requirement(lexical_owner))),
            _ => None,
        }
    }

    #[derive(Clone, Default)]
    struct InvocationGroup {
        invocation: StaticMachineApplicationHandle,
        purpose: InvocationPurpose,
        next: Handle<InvocationGroup>,
        first: CallableRow,
        last: CallableRow,
    }

    #[derive(Clone, Copy, Default)]
    struct RowLink {
        row: CallableRow,
        next: CallableRow,
    }

    #[derive(Clone, Default)]
    pub(crate) struct AppliedCallableTable {
        arena: Arena<AppliedCallableSignature>,
        invocation_heads: Vec<Handle<InvocationGroup>>,
        invocation_groups: Arena<InvocationGroup>,
        row_links: Vec<RowLink>,
    }

    impl From<Arena<AppliedCallableSignature>> for AppliedCallableTable {
        fn from(arena: Arena<AppliedCallableSignature>) -> Self {
            let layout = AssociationLayout::measure(&arena)
                .expect("produced callable association must fit the host's storage sizes");
            Self::recover(arena, layout)
        }
    }

    impl PartialEq for AppliedCallableTable {
        fn eq(&self, other: &Self) -> bool {
            self.arena == other.arena
        }
    }
    impl Eq for AppliedCallableTable {}
    impl std::fmt::Debug for AppliedCallableTable {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            self.arena.fmt(formatter)
        }
    }
    impl std::ops::Deref for AppliedCallableTable {
        type Target = Arena<AppliedCallableSignature>;
        fn deref(&self) -> &Self::Target {
            &self.arena
        }
    }

    impl compiler_storage::Record for AppliedCallableTable {
        fn write(
            &self,
            output: &mut compiler_storage::RecordWriter,
        ) -> Result<(), compiler_storage::RecordError> {
            output.record(|output| compiler_storage::Record::write(&self.arena, output))
        }
        fn read<Context>(
            input: &mut compiler_storage::RecordReader<'_, Context>,
        ) -> Result<Self, compiler_storage::RecordError> {
            input.record(|input| {
                let arena =
                    <Arena<AppliedCallableSignature> as compiler_storage::Record>::read(input)?;
                let layout = AssociationLayout::measure(&arena)
                    .ok_or_else(|| input.error("applied callable association size overflow"))?;
                let allocation = layout.allocation_bytes().ok_or_else(|| {
                    input.error("applied callable association allocation overflow")
                })?;
                input.allocate(allocation)?;
                Ok(Self::recover(arena, layout))
            })
        }
    }

    struct AssociationLayout {
        invocation_slots: usize,
        row_slots: usize,
        associated_rows: usize,
    }

    impl AssociationLayout {
        fn measure(arena: &Arena<AppliedCallableSignature>) -> Option<Self> {
            let mut layout = Self {
                invocation_slots: 0,
                row_slots: 0,
                associated_rows: 0,
            };
            for (handle, row) in arena.iter() {
                if let Some((invocation, _)) = invocation_owner(row) {
                    layout.invocation_slots = layout
                        .invocation_slots
                        .max((invocation.arena_index() as usize).checked_add(1)?);
                    layout.row_slots = layout
                        .row_slots
                        .max((handle.arena_index() as usize).checked_add(1)?);
                    layout.associated_rows = layout.associated_rows.checked_add(1)?;
                }
            }
            Some(layout)
        }

        fn allocation_bytes(&self) -> Option<usize> {
            self.invocation_slots
                .checked_mul(std::mem::size_of::<Handle<InvocationGroup>>())?
                .checked_add(self.row_slots.checked_mul(std::mem::size_of::<RowLink>())?)?
                .checked_add(
                    self.associated_rows
                        .checked_mul(std::mem::size_of::<InvocationGroup>())?,
                )?
                .checked_add(
                    self.associated_rows
                        .checked_mul(std::mem::size_of::<CallableRow>())?,
                )
        }
    }

    impl AppliedCallableTable {
        fn recover(arena: Arena<AppliedCallableSignature>, layout: AssociationLayout) -> Self {
            let mut rows = Vec::with_capacity(layout.associated_rows);
            for (handle, row) in arena.iter() {
                if invocation_owner(row).is_some() {
                    rows.push(handle);
                }
            }
            let mut table = Self {
                arena,
                invocation_heads: vec![Handle::default(); layout.invocation_slots],
                invocation_groups: Arena::with_capacity(layout.associated_rows),
                row_links: vec![RowLink::default(); layout.row_slots],
            };
            for row in rows {
                table.publish_row(row);
            }
            table
        }

        pub(crate) fn insert(&mut self, row: AppliedCallableSignature) -> CallableRow {
            let handle = self.arena.insert(row);
            self.publish_row(handle);
            handle
        }

        pub(crate) fn replace(
            &mut self,
            handle: CallableRow,
            row: AppliedCallableSignature,
        ) -> Result<(), &'static str> {
            if !self.arena.is_valid(handle) {
                return Err("applied callable replacement names a retired row");
            }
            self.remove_row(handle);
            *self.arena.get_mut(handle) = row;
            self.publish_row(handle);
            Ok(())
        }

        pub(crate) fn free(&mut self, handle: CallableRow) -> bool {
            if !self.arena.is_valid(handle) {
                return false;
            }
            self.remove_row(handle);
            self.arena.free(handle)
        }

        pub(crate) fn replace_signature(
            &mut self,
            handle: CallableRow,
            signature: StateSignature,
        ) -> Result<(), &'static str> {
            if !self.arena.is_valid(handle) {
                return Err("applied callable signature replacement names a retired row");
            }
            self.arena.get_mut(handle).signature = signature;
            Ok(())
        }

        pub(crate) fn set_required(
            &mut self,
            handle: CallableRow,
            required: StateSignature,
        ) -> Result<(), &'static str> {
            if !self.arena.is_valid(handle) {
                return Err("applied callable requirement names a retired row");
            }
            let AppliedCallableOwner::MachineSelection {
                required: destination,
                ..
            } = &mut self.arena.get_mut(handle).owner
            else {
                return Err("applied callable requirement names a non-selection row");
            };
            *destination = Some(required);
            Ok(())
        }

        fn group(
            &self,
            invocation: StaticMachineApplicationHandle,
            purpose: InvocationPurpose,
        ) -> Handle<InvocationGroup> {
            let mut handle = self
                .invocation_heads
                .get(invocation.arena_index() as usize)
                .copied()
                .unwrap_or_default();
            while handle.is_valid() {
                let group = self.invocation_groups.get(handle);
                if group.invocation == invocation && group.purpose == purpose {
                    return handle;
                }
                handle = group.next;
            }
            Handle::default()
        }

        fn next_row(&self, row: CallableRow) -> CallableRow {
            let link = self.row_links[row.arena_index() as usize];
            debug_assert_eq!(link.row, row);
            link.next
        }

        fn publish_row(&mut self, row: CallableRow) {
            let Some((invocation, purpose)) = invocation_owner(self.arena.get(row)) else {
                return;
            };
            let mut group = self.group(invocation, purpose);
            if !group.is_valid() {
                let slot = invocation.arena_index() as usize;
                self.invocation_heads
                    .resize(self.invocation_heads.len().max(slot + 1), Handle::default());
                group = self.invocation_groups.insert(InvocationGroup {
                    invocation,
                    purpose,
                    next: self.invocation_heads[slot],
                    ..Default::default()
                });
                self.invocation_heads[slot] = group;
            }
            let slot = row.arena_index() as usize;
            self.row_links
                .resize(self.row_links.len().max(slot + 1), RowLink::default());
            self.row_links[slot] = RowLink {
                row,
                next: CallableRow::default(),
            };
            let tail = self.invocation_groups.get(group).last;
            if !tail.is_valid() {
                let group = self.invocation_groups.get_mut(group);
                group.first = row;
                group.last = row;
            } else if tail.arena_index() < row.arena_index() {
                self.row_links[tail.arena_index() as usize].next = row;
                self.invocation_groups.get_mut(group).last = row;
            } else {
                let mut previous = CallableRow::default();
                let mut next = self.invocation_groups.get(group).first;
                while next.is_valid() && next.arena_index() < row.arena_index() {
                    previous = next;
                    next = self.next_row(next);
                }
                self.row_links[slot].next = next;
                if previous.is_valid() {
                    self.row_links[previous.arena_index() as usize].next = row;
                } else {
                    self.invocation_groups.get_mut(group).first = row;
                }
            }
        }

        fn remove_row(&mut self, row: CallableRow) {
            let Some((invocation, purpose)) = invocation_owner(self.arena.get(row)) else {
                return;
            };
            let group = self.group(invocation, purpose);
            let mut previous = CallableRow::default();
            let mut current = self.invocation_groups.get(group).first;
            while current.is_valid() && current != row {
                previous = current;
                current = self.next_row(current);
            }
            debug_assert_eq!(current, row);
            let next = self.next_row(row);
            if previous.is_valid() {
                self.row_links[previous.arena_index() as usize].next = next;
            } else {
                self.invocation_groups.get_mut(group).first = next;
            }
            if self.invocation_groups.get(group).last == row {
                self.invocation_groups.get_mut(group).last = previous;
            }
            self.row_links[row.arena_index() as usize] = RowLink::default();
            if self.invocation_groups.get(group).first.is_valid() {
                return;
            }
            let slot = invocation.arena_index() as usize;
            let successor = self.invocation_groups.get(group).next;
            let mut prior = Handle::<InvocationGroup>::default();
            let mut current = self.invocation_heads[slot];
            while current != group {
                prior = current;
                current = self.invocation_groups.get(current).next;
            }
            if prior.is_valid() {
                self.invocation_groups.get_mut(prior).next = successor;
            } else {
                self.invocation_heads[slot] = successor;
            }
            self.invocation_groups.free(group);
        }

        pub(crate) fn machine_rows(
            &self,
            invocation: StaticMachineApplicationHandle,
        ) -> impl Iterator<Item = (CallableRow, &AppliedCallableSignature)> {
            self.invocation_rows(invocation, InvocationPurpose::MachineSelection)
        }

        pub(crate) fn requirement_rows(
            &self,
            invocation: StaticMachineApplicationHandle,
            lexical_owner: SymbolHandle,
        ) -> impl Iterator<Item = (CallableRow, &AppliedCallableSignature)> {
            self.invocation_rows(invocation, InvocationPurpose::Requirement(lexical_owner))
        }

        fn invocation_rows(
            &self,
            invocation: StaticMachineApplicationHandle,
            purpose: InvocationPurpose,
        ) -> impl Iterator<Item = (CallableRow, &AppliedCallableSignature)> {
            let group = self.group(invocation, purpose);
            let mut next = if group.is_valid() {
                self.invocation_groups.get(group).first
            } else {
                CallableRow::default()
            };
            std::iter::from_fn(move || {
                if !next.is_valid() {
                    return None;
                }
                let row = next;
                next = self.next_row(row);
                Some((row, self.arena.get(row)))
            })
        }
    }

    #[cfg(test)]
    pub(crate) mod tests {
        use super::*;

        fn selection(invocation: StaticMachineApplicationHandle) -> AppliedCallableSignature {
            AppliedCallableSignature {
                owner: AppliedCallableOwner::MachineSelection {
                    invocation,
                    ordinal: 0,
                    demand: Default::default(),
                    selected: Default::default(),
                    provider: Default::default(),
                    required: None,
                    source_required: Default::default(),
                    caller_operands: Default::default(),
                    caller_bindings: Default::default(),
                    caller_lifetimes: Default::default(),
                    caller_lifetime_declaration: Default::default(),
                    caller_lifetime_owner: Default::default(),
                    provider_lifetime_declaration: Default::default(),
                    provider_lifetime_owner: Default::default(),
                    provider_lifetimes: Default::default(),
                },
                signature: Default::default(),
            }
        }

        fn rows(
            table: &AppliedCallableTable,
            invocation: StaticMachineApplicationHandle,
        ) -> Vec<CallableRow> {
            table
                .machine_rows(invocation)
                .map(|(handle, _)| handle)
                .collect()
        }

        #[test]
        fn invocation_generations_keep_separate_rows_in_arena_order() {
            let older = StaticMachineApplicationHandle::from_parts(17, 2);
            let newer = StaticMachineApplicationHandle::from_parts(17, 3);
            let unrelated = StaticMachineApplicationHandle::from_parts(18, 2);
            let mut table = AppliedCallableTable::default();
            let first = table.insert(selection(older));
            let second = table.insert(selection(newer));
            table.insert(selection(unrelated));
            let last = table.insert(selection(older));
            assert_eq!(rows(&table, older), [first, last]);
            assert_eq!(rows(&table, newer), [second]);
            assert!(rows(&table, StaticMachineApplicationHandle::from_parts(17, 4)).is_empty());
            table.free(second);
            assert_eq!(rows(&table, older), [first, last]);
            assert!(rows(&table, newer).is_empty());
        }

        #[test]
        fn recycled_row_slots_return_to_their_arena_position() {
            let invocation = StaticMachineApplicationHandle::from_parts(1, 1);
            let other = StaticMachineApplicationHandle::from_parts(2, 1);
            let mut table = AppliedCallableTable::default();
            let first = table.insert(selection(invocation));
            let middle = table.insert(selection(other));
            let last = table.insert(selection(invocation));
            assert!(table.free(first));
            assert!(!table.free(first));
            let recycled = table.insert(selection(invocation));
            assert_eq!(recycled.arena_index(), first.arena_index());
            assert_ne!(recycled.generation(), first.generation());
            assert_eq!(rows(&table, invocation), [recycled, last]);
            assert_eq!(rows(&table, other), [middle]);
            table.replace(last, selection(other)).unwrap();
            assert_eq!(rows(&table, invocation), [recycled]);
            assert_eq!(rows(&table, other), [middle, last]);
            assert!(table.replace(first, selection(other)).is_err());
        }

        #[test]
        fn clones_replacement_and_record_recovery_keep_their_own_associations() {
            let first = StaticMachineApplicationHandle::from_parts(3, 1);
            let second = StaticMachineApplicationHandle::from_parts(3, 2);
            let mut table = AppliedCallableTable::default();
            let one = table.insert(selection(first));
            let two = table.insert(selection(first));
            let clone = table.clone();
            table.free(one);
            assert_eq!(rows(&clone, first), [one, two]);
            assert_eq!(rows(&table, first), [two]);
            let mut replacement = Arena::default();
            let replacement_one = replacement.insert(selection(second));
            let replacement_two = replacement.insert(selection(second));
            assert_eq!(replacement.revision(), clone.revision());
            table = replacement.into();
            assert!(rows(&table, first).is_empty());
            assert_eq!(rows(&table, second), [replacement_one, replacement_two]);
            let limits = compiler_storage::RecordLimits::default();
            let encoded = compiler_storage::encode(&table, limits).unwrap();
            assert_eq!(
                encoded,
                compiler_storage::encode(&table.arena, compiler_storage::RecordLimits::default())
                    .unwrap()
            );
            let restored: AppliedCallableTable =
                compiler_storage::decode(&encoded, compiler_storage::RecordLimits::default())
                    .unwrap();
            assert_eq!(restored, table);
            assert_eq!(rows(&restored, second), [replacement_one, replacement_two]);
            assert!(rows(&restored, first).is_empty());
            #[derive(compiler_storage::Record)]
            struct PriorTableRecord {
                arena: Arena<AppliedCallableSignature>,
            }
            let prior = PriorTableRecord {
                arena: table.arena.clone(),
            };
            assert_eq!(
                compiler_storage::encode(&prior, compiler_storage::RecordLimits::default())
                    .unwrap(),
                encoded
            );
            let mut saw_write_success = false;
            let mut saw_read_success = false;
            for depth in 0..=compiler_storage::RecordLimits::default().depth {
                let limits = compiler_storage::RecordLimits {
                    depth,
                    ..Default::default()
                };
                let old_write = compiler_storage::encode(&prior, limits);
                let new_write = compiler_storage::encode(&table, limits);
                assert_eq!(old_write.is_ok(), new_write.is_ok(), "write depth {depth}");
                if let Ok(old_bytes) = old_write {
                    saw_write_success = true;
                    assert_eq!(old_bytes, new_write.unwrap());
                }
                let old_read = compiler_storage::decode::<PriorTableRecord>(&encoded, limits);
                let new_read = compiler_storage::decode::<AppliedCallableTable>(&encoded, limits);
                assert_eq!(old_read.is_ok(), new_read.is_ok(), "read depth {depth}");
                if let Ok(restored) = new_read {
                    saw_read_success = true;
                    assert_eq!(rows(&restored, second), [replacement_one, replacement_two]);
                }
                if depth == 0 {
                    assert!(
                        compiler_storage::encode(&table, limits)
                            .unwrap_err()
                            .reason
                            .contains("nesting")
                    );
                    assert!(
                        compiler_storage::decode::<AppliedCallableTable>(&encoded, limits)
                            .unwrap_err()
                            .reason
                            .contains("nesting")
                    );
                }
            }
            assert!(saw_write_success && saw_read_success);
            // Both purposes share the same links without borrowing each other's rows.
            // Lexical-owner generations remain part of a requirement's exact identity.
            let owner = SymbolHandle::from_parts(29, 1);
            let other_owner = SymbolHandle::from_parts(29, 2);
            let requirement = |application, lexical_owner| AppliedCallableSignature {
                owner: AppliedCallableOwner::RequirementInvocation {
                    declaration: Default::default(),
                    application,
                    lexical_owner,
                },
                signature: Default::default(),
            };
            let requirement_rows = |table: &AppliedCallableTable, application, lexical_owner| {
                table
                    .requirement_rows(application, lexical_owner)
                    .map(|(handle, _)| handle)
                    .collect::<Vec<_>>()
            };
            let mut mixed = AppliedCallableTable::default();
            let machine = mixed.insert(selection(first));
            let early = mixed.insert(requirement(first, owner));
            let separate = mixed.insert(requirement(first, other_owner));
            let late = mixed.insert(requirement(first, owner));
            assert_eq!(rows(&mixed, first), [machine]);
            assert_eq!(requirement_rows(&mixed, first, owner), [early, late]);
            assert_eq!(requirement_rows(&mixed, first, other_owner), [separate]);
            assert!(requirement_rows(&mixed, second, owner).is_empty());
            let unchanged = mixed.clone();
            mixed.free(early);
            let recycled = mixed.insert(requirement(first, owner));
            assert_eq!(recycled.arena_index(), early.arena_index());
            assert_ne!(recycled.generation(), early.generation());
            assert_eq!(requirement_rows(&mixed, first, owner), [recycled, late]);
            assert_eq!(requirement_rows(&unchanged, first, owner), [early, late]);
            mixed.replace(late, requirement(second, owner)).unwrap();
            assert_eq!(requirement_rows(&mixed, first, owner), [recycled]);
            assert_eq!(requirement_rows(&mixed, second, owner), [late]);
            assert_eq!(requirement_rows(&mixed, first, other_owner), [separate]);
            let encoded = compiler_storage::encode(&mixed, Default::default()).unwrap();
            let restored: AppliedCallableTable =
                compiler_storage::decode(&encoded, Default::default()).unwrap();
            assert_eq!(restored, mixed);
            assert_eq!(rows(&restored, first), [machine]);
            assert_eq!(requirement_rows(&restored, first, owner), [recycled]);
            assert_eq!(requirement_rows(&restored, second, owner), [late]);
            assert_eq!(requirement_rows(&restored, first, other_owner), [separate]);
        }

        #[test]
        fn signature_and_requirement_updates_cannot_change_invocation_ownership() {
            let invocation = StaticMachineApplicationHandle::from_parts(1, 1);
            let other = StaticMachineApplicationHandle::from_parts(1, 2);
            let mut table = AppliedCallableTable::default();
            let handle = table.insert(selection(invocation));
            let signature = StateSignature {
                symbol: symbols::SymbolHandle::from_arena_index(9),
                ..Default::default()
            };
            table.replace_signature(handle, signature.clone()).unwrap();
            table.set_required(handle, signature.clone()).unwrap();
            assert_eq!(rows(&table, invocation), [handle]);
            assert!(rows(&table, other).is_empty());
            assert_eq!(table.get(handle).signature, signature);
            assert!(
                matches!(&table.get(handle).owner, AppliedCallableOwner::MachineSelection { required: Some(required), .. } if *required == signature)
            );
            let non_selection = table.insert(AppliedCallableSignature::default());
            assert!(
                table
                    .set_required(non_selection, StateSignature::default())
                    .is_err()
            );
            assert_eq!(rows(&table, invocation), [handle]);
        }

        #[test]
        fn absent_invocations_and_trait_only_tables_have_no_machine_rows() {
            let invocation = StaticMachineApplicationHandle::from_parts(99, 4);
            let mut table = AppliedCallableTable::default();
            assert!(rows(&table, invocation).is_empty());
            table.insert(AppliedCallableSignature {
                owner: AppliedCallableOwner::Trait {
                    declaration: Default::default(),
                    application: Default::default(),
                    scope: Default::default(),
                },
                signature: Default::default(),
            });
            assert!(rows(&table, invocation).is_empty());
            assert!(rows(&table, StaticMachineApplicationHandle::default()).is_empty());
        }

        #[test]
        fn record_recovery_budgets_association_before_allocating_for_handles() {
            let mut arena = Arena::default();
            arena.insert(selection(StaticMachineApplicationHandle::from_parts(
                u32::MAX,
                1,
            )));
            let bytes = compiler_storage::encode(&arena, compiler_storage::RecordLimits::default())
                .unwrap();
            let error = compiler_storage::decode::<AppliedCallableTable>(
                &bytes,
                compiler_storage::RecordLimits::default(),
            )
            .unwrap_err();
            assert!(error.reason.contains("allocation"));
        }

        #[test]
        fn small_record_recovery_charges_every_preallocated_association_buffer() {
            let mut arena = Arena::default();
            arena.insert(selection(StaticMachineApplicationHandle::from_parts(1, 1)));
            let bytes = compiler_storage::encode(&arena, compiler_storage::RecordLimits::default())
                .unwrap();
            let mut low = 0;
            let mut high = compiler_storage::RecordLimits::default().allocation;
            while low < high {
                let middle = low + (high - low) / 2;
                let limits = compiler_storage::RecordLimits {
                    allocation: middle,
                    ..Default::default()
                };
                if compiler_storage::decode::<Arena<AppliedCallableSignature>>(&bytes, limits)
                    .is_ok()
                {
                    high = middle;
                } else {
                    low = middle + 1;
                }
            }
            let association = AssociationLayout::measure(&arena)
                .unwrap()
                .allocation_bytes()
                .unwrap();
            assert!(association > 0);
            let tight = compiler_storage::RecordLimits {
                allocation: low + association - 1,
                ..Default::default()
            };
            assert!(
                compiler_storage::decode::<Arena<AppliedCallableSignature>>(&bytes, tight).is_ok()
            );
            let error =
                compiler_storage::decode::<AppliedCallableTable>(&bytes, tight).unwrap_err();
            assert!(error.reason.contains("allocation"));
            let exact = compiler_storage::RecordLimits {
                allocation: low + association,
                ..Default::default()
            };
            let restored: AppliedCallableTable = compiler_storage::decode(&bytes, exact).unwrap();
            assert_eq!(
                rows(&restored, StaticMachineApplicationHandle::from_parts(1, 1)).len(),
                1
            );
        }
    }
}
pub(crate) mod trait_scopes {
    use super::{AppliedCallableOwner, AppliedCallableSignature, CallableApplication};
    use crate::{TypedTrees, signature, trait_definition::AppliedTraitScope};
    use symbols::SymbolHandle;

    impl TypedTrees {
        /// The selected map authenticates execution; the caller's evidence
        /// bound supplies the public contract, including its refinements.
        pub fn selected_requirement_contract_scope<'program>(
            &'program self,
            owner: SymbolHandle,
            dispatch: &crate::StaticRequirementDispatch,
        ) -> Result<AppliedTraitScope<'program>, &'static str> {
            use crate::static_applications::StaticMachineBindingValue;
            use crate::trait_definition::TraitApplicationArguments;

            let applications = self.selected_conformance_applications(
                owner,
                dispatch.selection_application,
                dispatch.evidence_parameter,
                dispatch.requirement,
                dispatch.application_report_fingerprint,
                dispatch.application_commitment,
            );
            let [application] = applications.as_slice() else {
                return Err("selected contract has no unique supplying map application");
            };
            let selected = application
                .trait_scope
                .scope(self)
                .and_then(|scope| scope.declaring_scope(self, dispatch.declaring_trait))
                .ok_or("selected contract lost its declaring map scope")?;
            if !dispatch.evidence_parameter.is_valid() {
                return Ok(selected);
            }
            let mut receipts = self
                .machine_specializations
                .iter()
                .filter(|receipt| receipt.instance == owner);
            let receipt = receipts
                .next()
                .ok_or("selected contract lost its supplying specialization")?;
            if receipts.next().is_some() || !receipt.binding_application.is_valid() {
                return Err("selected contract has an ambiguous supplying specialization");
            }
            let source = self.static_machine_application(receipt.binding_application);
            if source.binding_declaration != receipt.template {
                return Err("selected contract changed its supplying declaration");
            }
            let mut bounds = self
                .callable_conformance_bounds(receipt.template)
                .ok_or("selected contract lost its authored evidence bounds")?
                .iter()
                .filter(|bound| bound.binder == dispatch.evidence_parameter);
            let bound = bounds
                .next()
                .ok_or("selected contract lost its exact evidence binder")?;
            if bounds.next().is_some()
                || !matches!(
                    self.specialization_binding(receipt, bound.binder),
                    Some(StaticMachineBindingValue::Conformance(_))
                )
            {
                return Err("selected contract has no unique implementation binding");
            }
            let declaration = self
                .trait_definition_by_symbol(bound.trait_symbol)
                .ok_or("selected contract lost its bound declaration")?;
            let root = AppliedTraitScope::new(
                self,
                declaration,
                TraitApplicationArguments {
                    lexical_owner: receipt.template,
                    enclosing_data_applications: &[],
                    arguments: &bound.arguments,
                    lifetimes: &bound.lifetime_arguments,
                },
            )
            .ok_or("selected contract cannot form its complete evidence bound")?;
            let arguments = self
                .static_machine_application_bindings(receipt.binding_application)
                .iter()
                .filter_map(|binding| {
                    let reference = match &binding.value {
                        StaticMachineBindingValue::Type(reference)
                        | StaticMachineBindingValue::Const(reference) => *reference,
                        StaticMachineBindingValue::Machine(argument)
                        | StaticMachineBindingValue::Conformance(argument) => {
                            argument.type_reference
                        }
                        StaticMachineBindingValue::RuntimeValue { .. } => return None,
                    };
                    Some((binding.parameter, reference))
                })
                .collect::<Vec<_>>();
            let lifetimes = self.static_application_lifetimes_for_declaration(
                receipt.binding_application,
                receipt.template,
            )?;
            let required = root
                .under_invocation(
                    self,
                    receipt.template,
                    source.lexical_owner,
                    &arguments,
                    &lifetimes,
                )
                .and_then(|scope| scope.declaring_scope(self, dispatch.declaring_trait))
                .ok_or("selected contract lost its complete invocation scope")?;
            if !required.same_application(self, &selected) {
                return Err("selected map disagrees with its supplying evidence bound");
            }
            Ok(required)
        }

        pub fn requirement_call_signature(
            &self,
            lexical_owner: SymbolHandle,
            call: &crate::expression::TableCallExpression,
        ) -> Option<&signature::StateSignature> {
            self.resolve_requirement_call_signature(lexical_owner, call)
                .ok()
        }

        /// Copied contracts retain the full outer application even though their
        /// logical owner and bare sibling targets keep the declaration symbols.
        /// Explicit selections still own their result; only an implicit sibling
        /// may use the exact supplying scope, including inherited/nullary members.
        pub fn requirement_call_signature_in_scope(
            &self,
            lexical_owner: SymbolHandle,
            call: &crate::expression::TableCallExpression,
            supplying_scope: crate::trait_definition::RetainedTraitScope,
        ) -> Option<&signature::StateSignature> {
            if let Some(signature) = self.resolve_named_conformance_call_signature(call).ok()? {
                return Some(signature);
            }
            if !call.is_ordinary_positional_call()
                || call.requirement_application.is_valid()
                || call.exact_coercion_target.is_valid()
                || call.build_selection.is_valid()
                || call.operational_acknowledgement.acknowledges_suspend
                || call.operational_acknowledgement.acknowledges_block
                || self
                    .closed_requirement_application(call.target_symbol)
                    .is_some_and(|application| !application.local_arguments.is_empty())
            {
                return self.requirement_call_signature(lexical_owner, call);
            }
            let requirement = self
                .closed_requirement_application(call.target_symbol)
                .map_or(call.target_symbol, |application| application.template);
            let Some((declaration, declared)) = self.trait_machine_signature(requirement) else {
                return self.requirement_call_signature(lexical_owner, call);
            };
            let Some(scope) = supplying_scope
                .scope(self)?
                .declaring_scope(self, declaration.symbol)
            else {
                return self.requirement_call_signature(lexical_owner, call);
            };
            // A copied call can still name the original member. Use the contract
            // and result type from the trait application supplying it. The member
            // keeps its declared lifetimes for ordinary call checking.
            self.applied_callable_signature(&scope, declared.symbol)
                .filter(|signature| signature.type_parameters.is_empty())
        }

        /// The same logical view with the reason its exact source cannot supply it.
        /// Optional queries above keep their existing refusal behavior.
        pub fn resolve_requirement_call_signature(
            &self,
            lexical_owner: SymbolHandle,
            call: &crate::expression::TableCallExpression,
        ) -> Result<&signature::StateSignature, &'static str> {
            self.requirement_invocation_signature(
                lexical_owner,
                call.target_symbol,
                call.static_requirement_dispatch.as_ref(),
                call.requirement_application,
                call.authored_machine_application,
            )
            .or_else(|reason| self.named_conformance_call_signature(call).ok_or(reason))
        }

        pub fn requirement_statement_call_signature(
            &self,
            lexical_owner: SymbolHandle,
            call: &crate::statement::TableCall,
        ) -> Option<&signature::StateSignature> {
            self.requirement_invocation_signature(
                lexical_owner,
                call.target_symbol,
                call.static_requirement_dispatch.as_ref(),
                Default::default(),
                call.authored_machine_application,
            )
            .ok()
        }

        fn requirement_invocation_signature(
            &self,
            lexical_owner: SymbolHandle,
            target: SymbolHandle,
            dispatch: Option<&crate::StaticRequirementDispatch>,
            requirement_application: crate::types::TypeReferenceHandle,
            authored_application: crate::static_applications::StaticMachineApplicationHandle,
        ) -> Result<&signature::StateSignature, &'static str> {
            let target = dispatch.map_or(target, |dispatch| dispatch.contract_symbol());
            if let Some(closed) = self.closed_requirement_application(target) {
                if dispatch.is_none() && authored_application.is_valid() {
                    let source = self.static_machine_application(authored_application);
                    let original = source.registration_operation;
                    let binder = if original.is_valid() {
                        self.symbols.get(original).parent
                    } else {
                        SymbolHandle::invalid()
                    };
                    if source.lexical_owner == lexical_owner
                        && binder.is_valid()
                        && self.symbols.get(binder).kind
                            == symbols::SymbolKind::ConformanceParameter
                    {
                        let mut owner = lexical_owner;
                        let mut visited = Vec::new();
                        while owner.is_valid() && !visited.contains(&owner) {
                            visited.push(owner);
                            if let Some(bound) = self
                                .evidence_bound_requirement(owner, binder, target)
                                .map_err(|_| "closed public view has unresolved source evidence")?
                            {
                                if bound
                                    .application
                                    .callable_refinement_clauses(
                                        self,
                                        bound.signature.name.as_str(),
                                    )
                                    .is_empty()
                                {
                                    break;
                                }
                                if let Some(signature) = self.applied_callable_signature(
                                    &bound.application,
                                    bound.signature.symbol,
                                ) && signature.type_parameters.is_empty()
                                    && signature.lifetime_parameters.is_empty()
                                {
                                    return Ok(signature);
                                }
                                break;
                            }
                            owner = self.declaration_scope_parent(owner);
                        }
                    }
                }
                if requirement_application.is_valid() {
                    let scope = self
                        .applied_requirement_scope(lexical_owner, requirement_application)
                        .ok_or("closed public view has no exact source application scope")?;
                    if !self.requirement_application_matches_scope(target, &scope) {
                        return Err("closed public view does not match its source application");
                    }
                }
                return Ok(closed.signature);
            }
            if let Some(prepared) = self.find_requirement_invocation(
                authored_application,
                lexical_owner,
                |prepared| {
                        matches!(prepared.owner, AppliedCallableOwner::RequirementInvocation {
                    application,
                    lexical_owner: owner,
                    ..
                } if application == authored_application
                    && application.is_valid()
                    && owner == lexical_owner
                    && (prepared.signature.symbol == target
                        || {
                            let source = self.expression_table.static_machine_application(application);
                            source.lexical_owner == lexical_owner
                                && source.registration_operation == target
                                && source.binding_declaration == prepared.signature.symbol
                        }))
                },
            ) {
                return Ok(&prepared.signature);
            }
            let outer = self.requirement_outer_signature(
                lexical_owner,
                target,
                dispatch,
                requirement_application,
            )?;
            (dispatch.is_none() || outer.type_parameters.is_empty())
                .then_some(outer)
                .ok_or("selected public view still has an unbound member telescope")
        }

        fn requirement_outer_signature(
            &self,
            lexical_owner: SymbolHandle,
            target: SymbolHandle,
            dispatch: Option<&crate::StaticRequirementDispatch>,
            requirement_application: crate::types::TypeReferenceHandle,
        ) -> Result<&signature::StateSignature, &'static str> {
            let binder = self.symbols.get(target).parent;
            let mut owner = lexical_owner;
            let mut visited = Vec::new();
            let bound = loop {
                if !owner.is_valid() || visited.contains(&owner) {
                    break None;
                }
                visited.push(owner);
                if let Some(bound) = self
                    .evidence_bound_requirement(owner, binder, target)
                    .map_err(|_| "public view has unresolved source evidence")?
                {
                    break Some(bound);
                }
                owner = self.declaration_scope_parent(owner);
            };
            if let Some(bound) = bound {
                if requirement_application.is_valid() {
                    let written = self
                        .applied_requirement_scope(lexical_owner, requirement_application)
                        .ok_or("evidence public view has no exact written application scope")?
                        .declaring_scope(self, bound.application.declaration.symbol)
                        .ok_or("evidence public view has no declaring trait scope")?;
                    if !bound.application.same_application(self, &written) {
                        return Err("evidence public view disagrees with its written application");
                    }
                }
                return self
                    .applied_callable_signature(&bound.application, bound.signature.symbol)
                    .or_else(|| {
                        (bound.application.substitutions.is_empty()
                            && bound.application.declaration.lifetime_parameters.is_empty())
                        .then_some(bound.signature)
                    })
                    .ok_or("evidence public view has no prepared applied signature");
            }
            let (declaration, signature) = self
                .trait_machine_signature(target)
                .ok_or("public view has no source requirement declaration")?;
            if let Some(dispatch) = dispatch {
                let scope = self
                    .selected_requirement_contract_scope(lexical_owner, dispatch)?
                    .declaring_scope(self, declaration.symbol)
                    .ok_or("selected public view has no declaring trait scope")?;
                return self
                    .applied_callable_signature(&scope, signature.symbol)
                    .or_else(|| scope.keeps_declared_signatures(self).then_some(signature))
                    .ok_or("selected public view has no prepared applied signature");
            }
            if requirement_application.is_valid() {
                let scope = self
                    .applied_requirement_scope(lexical_owner, requirement_application)
                    .ok_or("public view has no exact written application scope")?
                    .declaring_scope(self, declaration.symbol)
                    .ok_or("public view has no declaring trait scope")?;
                return self
                    .applied_callable_signature(&scope, signature.symbol)
                    .ok_or("public view has no prepared applied signature");
            }
            (declaration.type_parameters.is_empty() && declaration.lifetime_parameters.is_empty())
                .then_some(signature)
                .ok_or("public view has no complete outer application")
        }

        pub fn prepare_requirement_invocation(
            &mut self,
            lexical_owner: SymbolHandle,
            call: &crate::expression::TableCallExpression,
            bindings: &[crate::static_applications::StaticMachineBinding],
            selected: &signature::StateSignature,
        ) -> Result<Option<signature::StateSignature>, &'static str> {
            let target = call
                .static_requirement_dispatch
                .as_ref()
                .map_or(call.target_symbol, |dispatch| dispatch.contract_symbol());
            self.prepare_requirement_invocation_application(
                lexical_owner,
                target,
                call.authored_machine_application,
                bindings,
                selected,
            )
        }

        pub fn prepare_requirement_statement_invocation(
            &mut self,
            lexical_owner: SymbolHandle,
            call: &crate::statement::TableCall,
            bindings: &[crate::static_applications::StaticMachineBinding],
            selected: &signature::StateSignature,
        ) -> Result<Option<signature::StateSignature>, &'static str> {
            let target = call
                .static_requirement_dispatch
                .as_ref()
                .map_or(call.target_symbol, |dispatch| dispatch.contract_symbol());
            self.prepare_requirement_invocation_application(
                lexical_owner,
                target,
                call.authored_machine_application,
                bindings,
                selected,
            )
        }

        fn prepare_requirement_invocation_application(
            &mut self,
            lexical_owner: SymbolHandle,
            target: SymbolHandle,
            authored_application: crate::static_applications::StaticMachineApplicationHandle,
            bindings: &[crate::static_applications::StaticMachineBinding],
            selected: &signature::StateSignature,
        ) -> Result<Option<signature::StateSignature>, &'static str> {
            if self.closed_requirement_application(target).is_some()
                || !authored_application.is_valid()
            {
                return Ok(None);
            }
            let Some((declaration, declared)) = self.trait_machine_signature(selected.symbol)
            else {
                return Ok(None);
            };
            if declared.type_parameters.is_empty() {
                return Ok(None);
            }
            let declaration = declaration.symbol;
            let declared = declared.clone();
            let mut application = CallableApplication {
                region_scope: lexical_owner,
                static_argument_owner: lexical_owner,
                ..CallableApplication::default()
            };
            for binding in bindings {
                let Some(parameter) =
                    self.applied_signature_parameter(&declared, selected, binding.parameter)
                else {
                    return Err("requirement invocation lost its prepared member binder");
                };
                match &binding.value {
                    crate::static_applications::StaticMachineBindingValue::Type(actual)
                    | crate::static_applications::StaticMachineBindingValue::Const(actual) => {
                        application.types.push((parameter, *actual));
                    }
                    crate::static_applications::StaticMachineBindingValue::Machine(actual)
                    | crate::static_applications::StaticMachineBindingValue::Conformance(actual) => {
                        application
                            .static_arguments
                            .push((parameter, actual.clone()));
                    }
                    crate::static_applications::StaticMachineBindingValue::RuntimeValue {
                        ..
                    } => {}
                }
            }
            let signature = application.signature(self, selected)?;
            let owner = AppliedCallableOwner::RequirementInvocation {
                declaration,
                application: authored_application,
                lexical_owner,
            };
            let previous =
                self.applied_callable_signatures
                    .iter()
                    .find_map(|(handle, previous)| {
                        (previous.owner == owner && previous.signature.symbol == signature.symbol)
                            .then_some(handle)
                    });
            if let Some(previous) = previous {
                self.applied_callable_signatures
                    .replace_signature(previous, signature.clone())?;
            } else {
                self.applied_callable_signatures
                    .insert(AppliedCallableSignature {
                        owner,
                        signature: signature.clone(),
                    });
            }
            Ok(Some(signature))
        }

        pub fn applied_callable_signature(
            &self,
            application: &AppliedTraitScope<'_>,
            requirement: SymbolHandle,
        ) -> Option<&signature::StateSignature> {
            if let Some(closed) = self.closed_requirement_application(requirement)
                && self.requirement_application_matches_contract_scope(requirement, application)
            {
                return Some(closed.signature);
            }
            let (owner, identity) = application.callable_contract_key(self)?;
            self.applied_callable_signatures
                .iter()
                .find_map(|(_, prepared)| {
                    let AppliedCallableOwner::Trait {
                        declaration,
                        application: prepared_identity,
                        ..
                    } = &prepared.owner
                    else {
                        return None;
                    };
                    if *declaration != owner
                        || *prepared_identity != identity
                        || prepared.signature.symbol != requirement
                    {
                        return None;
                    }
                    Some(&prepared.signature)
                })
        }

        pub fn prepare_applied_callable_signatures(
            &mut self,
        ) -> Result<Vec<signature::StateSignature>, &'static str> {
            self.prepare_applied_callable_signatures_reporting_change()
                .map(|(produced, _)| produced)
        }

        pub fn prepare_applied_callable_signatures_reporting_change(
            &mut self,
        ) -> Result<(Vec<signature::StateSignature>, bool), &'static str> {
            let invocation_scopes = self.refinement_invocation_scopes()?;
            let mut roots = self.applied_trait_roots();
            roots.extend(
                invocation_scopes
                    .iter()
                    .filter_map(|scope| scope.scope(self)),
            );
            let mut bound_roots = Vec::new();
            for root in &roots {
                root.visit(self, &mut |scope| {
                    for bound in &scope.declaration.conformance_bounds {
                        if let Some(bound) = scope.bound_scope(self, bound.binder) {
                            bound_roots.push(bound);
                        }
                    }
                });
            }
            roots.extend(bound_roots);
            let mut pending = Vec::new();
            for scope in roots {
                scope.visit(self, &mut |scope| {
                    for signature in self.trait_machine_signatures(scope.declaration) {
                        let clauses =
                            scope.callable_refinement_clauses(self, signature.name.as_str());
                        if signature.type_parameters.is_empty()
                            && scope.substitutions.is_empty()
                            && scope.declaration.lifetime_parameters.is_empty()
                            && clauses.is_empty()
                        {
                            continue;
                        }
                        let Some((owner, identity)) = scope.callable_contract_key(self) else {
                            continue;
                        };
                        if !clauses.is_empty() {
                            for closed in self.closed_requirement_applications().filter(|closed| {
                                closed.template == signature.symbol
                                    && self.requirement_application_matches_scope(
                                        closed.signature.symbol,
                                        scope,
                                    )
                                    && !self.requirement_application_matches_contract_scope(
                                        closed.signature.symbol,
                                        scope,
                                    )
                            }) {
                                pending.push((
                                    scope.declaration.symbol,
                                    owner,
                                    identity,
                                    closed.signature.symbol,
                                    scope.environment(),
                                    clauses.clone(),
                                ));
                            }
                        }
                        pending.push((
                            scope.declaration.symbol,
                            owner,
                            identity,
                            signature.symbol,
                            scope.environment(),
                            clauses,
                        ));
                    }
                });
            }
            let row_key =
                |requirement: SymbolHandle,
                 declaration: SymbolHandle,
                 identity: crate::type_identity::NormalizedTypeIdentity| {
                    (
                        (requirement.arena_index(), requirement.generation()),
                        (declaration.arena_index(), declaration.generation()),
                        identity,
                    )
                };
            let index_rows = |program: &Self| {
                let mut rows = program
                    .applied_callable_signatures
                    .iter()
                    .filter_map(|(_, prepared)| match prepared.owner {
                        AppliedCallableOwner::Trait {
                            declaration,
                            application,
                            ..
                        } => Some(row_key(prepared.signature.symbol, declaration, application)),
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                rows.sort_unstable();
                (rows, program.applied_callable_signatures.revision())
            };
            let (mut prepared_rows, mut indexed_revision) = index_rows(self);
            let mut wrote = false;
            let mut produced = Vec::new();
            for (trait_symbol, owner, identity, requirement, environment, clauses) in pending {
                if self.applied_callable_signatures.revision() != indexed_revision {
                    (prepared_rows, indexed_revision) = index_rows(self);
                }
                if prepared_rows
                    .binary_search(&row_key(requirement, owner, identity))
                    .is_ok()
                {
                    continue;
                }
                wrote = true;
                let mut applied = if let Some(closed) =
                    self.closed_requirement_application(requirement)
                {
                    closed.signature.clone()
                } else {
                    let application = self.materialize_trait_application(&environment)?;
                    let arguments = self.trait_application_arguments(trait_symbol, &application)?;
                    let Some(applied) = CallableApplication {
                        trait_region_application: Some((
                            environment.clone(),
                            environment.frame_count(),
                        )),
                        ..CallableApplication::default()
                    }
                    .nominal(self, trait_symbol, requirement, &arguments)?
                    else {
                        continue;
                    };
                    applied
                };
                self.apply_callable_refinements(&mut applied, &clauses)?;
                let scope = environment.retain(self)?;
                let inserted = row_key(applied.symbol, owner, identity);
                produced.push(applied.clone());
                let before_insert = self.applied_callable_signatures.revision();
                self.applied_callable_signatures
                    .insert(AppliedCallableSignature {
                        owner: AppliedCallableOwner::Trait {
                            declaration: owner,
                            application: identity,
                            scope,
                        },
                        signature: applied,
                    });
                let after_insert = self.applied_callable_signatures.revision();
                if before_insert == indexed_revision && after_insert == before_insert + 1 {
                    if let Err(position) = prepared_rows.binary_search(&inserted) {
                        prepared_rows.insert(position, inserted);
                    }
                    indexed_revision = after_insert;
                } else {
                    (prepared_rows, indexed_revision) = index_rows(self);
                }
            }
            Ok((produced, wrote))
        }

        fn refinement_invocation_scopes(
            &mut self,
        ) -> Result<Vec<crate::trait_definition::TraitApplicationEnvironment>, &'static str>
        {
            use crate::static_applications::StaticMachineBindingValue;
            use crate::trait_definition::{AppliedTraitScope, TraitApplicationArguments};
            let occurrences = crate::references::collect_program_application_occurrences(self);
            let mut scopes = Vec::new();
            for invocation in occurrences.invocation_applications {
                let record = self.static_machine_application(invocation).clone();
                let Some(machine) = self.machine_by_symbol(record.binding_declaration) else {
                    continue;
                };
                // Any refinement clause changes the callable the bound supplies:
                // contracts, termination, suspension, blocking or reach. A
                // selected requirement call reads its contract from this
                // invocation scope, so each refined bound needs one prepared.
                let bounds = machine
                    .conformance_bounds
                    .iter()
                    .filter(|bound| {
                        self.trait_definition_by_symbol(bound.trait_symbol)
                            .is_some_and(|declaration| !declaration.refinement_clauses.is_empty())
                    })
                    .cloned()
                    .collect::<Vec<_>>();
                if bounds.is_empty() {
                    continue;
                }
                let mut bindings = self
                    .static_machine_application_bindings(invocation)
                    .to_vec();
                let mut arguments = Vec::with_capacity(bindings.len());
                let mut retained_types = false;
                for binding in &mut bindings {
                    let actual = match &mut binding.value {
                        StaticMachineBindingValue::Type(reference)
                        | StaticMachineBindingValue::Const(reference) => *reference,
                        StaticMachineBindingValue::Machine(argument) => {
                            if !argument.type_reference.is_valid() {
                                argument.type_reference = self
                                    .static_operand_type_reference_in_scope(
                                        argument,
                                        record.lexical_owner,
                                    )?;
                                retained_types = true;
                            }
                            argument.type_reference
                        }
                        StaticMachineBindingValue::Conformance(argument) => {
                            if !argument.type_reference.is_valid() {
                                return Err(
                                    "refinement invocation lost its retained implementation operand",
                                );
                            }
                            argument.type_reference
                        }
                        StaticMachineBindingValue::RuntimeValue { .. } => {
                            return Err(
                                "refinement postcondition invocation has an unresolved value argument",
                            );
                        }
                    };
                    arguments.push((binding.parameter, actual));
                }
                if retained_types {
                    self.expression_table
                        .set_static_machine_application_bindings(invocation, &bindings);
                }
                let lifetimes = self.static_application_lifetimes_for_declaration(
                    invocation,
                    record.binding_declaration,
                )?;
                for bound in bounds {
                    let declaration = self
                        .trait_definition_by_symbol(bound.trait_symbol)
                        .ok_or("refinement invocation lost its bound declaration")?;
                    let scope = AppliedTraitScope::new(
                        self,
                        declaration,
                        TraitApplicationArguments {
                            lexical_owner: record.binding_declaration,
                            enclosing_data_applications: &[],
                            arguments: &bound.arguments,
                            lifetimes: &bound.lifetime_arguments,
                        },
                    )
                    .and_then(|scope| {
                        scope.under_invocation(
                            self,
                            record.binding_declaration,
                            record.lexical_owner,
                            &arguments,
                            &lifetimes,
                        )
                    })
                    .ok_or("refinement invocation lost its complete bound scope")?;
                    scopes.push(scope.environment());
                }
            }
            Ok(scopes)
        }

        pub fn materialize_trait_application_arguments(
            &mut self,
            environment: &crate::trait_definition::TraitApplicationEnvironment,
        ) -> Result<Vec<crate::types::TypeReferenceHandle>, &'static str> {
            let trait_symbol = environment
                .scope(self)
                .ok_or("applied callable lost its complete source environment")?
                .declaration
                .symbol;
            let application = self.materialize_trait_application(environment)?;
            self.trait_application_arguments(trait_symbol, &application)
        }

        pub(crate) fn materialize_trait_application(
            &mut self,
            environment: &crate::trait_definition::TraitApplicationEnvironment,
        ) -> Result<CallableApplication, &'static str> {
            let scope = environment
                .scope(self)
                .ok_or("applied callable lost its complete source environment")?;
            let substitutions = scope.substitutions;
            let scope_starts = scope.substitution_scope_starts;
            let argument_owner = scope.argument_owner;
            let mut application = CallableApplication {
                trait_region_application: Some((environment.clone(), 0)),
                ..CallableApplication::default()
            };
            for (ordinal, start) in scope_starts.iter().copied().enumerate() {
                if let Some((_, preceding_frames)) = &mut application.trait_region_application {
                    *preceding_frames = ordinal;
                }
                let end = scope_starts
                    .get(ordinal + 1)
                    .copied()
                    .unwrap_or(substitutions.len());
                let group = substitutions[start..end]
                    .iter()
                    .map(|(parameter, argument)| {
                        let applied = application.type_reference(self, *argument)?;
                        if ordinal == 0
                            && applied.is_valid()
                            && !self
                                .type_reference_table
                                .operand_lifetime_owner(applied)
                                .is_valid()
                        {
                            self.type_reference_table
                                .retain_operand_lifetime_owner(applied, argument_owner)?;
                        }
                        Ok((*parameter, applied))
                    })
                    .collect::<Result<Vec<_>, &'static str>>()?;
                application.types.extend(group);
            }
            Ok(application)
        }

        fn trait_application_arguments(
            &self,
            trait_symbol: SymbolHandle,
            application: &CallableApplication,
        ) -> Result<Vec<crate::types::TypeReferenceHandle>, &'static str> {
            let parameters = self
                .trait_definition_by_symbol(trait_symbol)
                .map(|declaration| {
                    self.trait_type_parameters(declaration)
                        .iter()
                        .map(|parameter| parameter.symbol)
                        .collect::<Vec<_>>()
                })
                .ok_or("applied callable lost its declaring trait")?;
            let arguments = parameters
                .into_iter()
                .map(|symbol| {
                    application
                        .types
                        .iter()
                        .rev()
                        .find(|(parameter, _)| *parameter == symbol)
                        .map(|(_, argument)| *argument)
                        .ok_or("applied callable lost a trait argument")
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(arguments)
        }
    }
}
pub(crate) use table::AppliedCallableTable;

pub use requirement_instances::{ClosedRequirementApplication, RequirementLocalArgument};

use crate::static_applications::StaticMachineLifetimeBinding;
use crate::type_identity::lifetime_parameter_identity;
use crate::{TypedTrees, data, signature, types};
use symbols::SymbolHandle;

impl TypedTrees {
    pub fn applied_trait_signatures(
        &self,
        trait_symbol: SymbolHandle,
    ) -> impl Iterator<Item = &signature::StateSignature> {
        self.applied_trait_contracts(trait_symbol)
            .map(|contract| contract.signature)
    }

    pub fn applied_trait_contracts(
        &self,
        trait_symbol: SymbolHandle,
    ) -> impl Iterator<Item = CallableContractScope<'_>> {
        self.applied_callable_signatures
            .iter()
            .filter(|(_, prepared)| self.callable_application_owner_is_live(prepared))
            .filter_map(move |(_, prepared)| match prepared.owner {
                AppliedCallableOwner::Trait {
                    declaration, scope, ..
                }
                | AppliedCallableOwner::RequirementInstance {
                    declaration,
                    trait_scope: scope,
                    ..
                } if declaration == trait_symbol => Some(CallableContractScope {
                    signature: &prepared.signature,
                    trait_scope: Some(scope),
                }),
                _ => None,
            })
    }

    pub fn requirement_invocation_signatures(
        &self,
        caller: SymbolHandle,
    ) -> impl Iterator<Item = &signature::StateSignature> {
        self.applied_callable_signatures
            .iter()
            .filter_map(move |(_, prepared)| match prepared.owner {
                AppliedCallableOwner::RequirementInvocation { lexical_owner, .. }
                | AppliedCallableOwner::Realization { lexical_owner, .. }
                    if lexical_owner == caller =>
                {
                    Some(&prepared.signature)
                }
                _ => None,
            })
    }

    pub fn prepared_callable_signatures(&self) -> impl Iterator<Item = &signature::StateSignature> {
        self.prepared_callable_contracts()
            .map(|contract| contract.signature)
    }

    /// The copied telescope and its supplying trait application travel
    /// together. Logical declaration symbols deliberately remain unchanged
    /// when preparation substitutes a concrete outer application.
    pub fn prepared_callable_contracts(&self) -> impl Iterator<Item = CallableContractScope<'_>> {
        self.applied_callable_signatures
            .iter()
            .filter(|(_, prepared)| self.callable_application_owner_is_live(prepared))
            .flat_map(|(_, prepared)| {
                let counterparts = match &prepared.owner {
                    AppliedCallableOwner::MachineSelection {
                        required,
                        source_required,
                        ..
                    } => [required.as_ref(), Some(source_required)],
                    _ => [None, None],
                };
                let trait_scope = match prepared.owner {
                    AppliedCallableOwner::Trait { scope, .. } => Some(scope),
                    AppliedCallableOwner::RequirementInstance { trait_scope, .. } => {
                        Some(trait_scope)
                    }
                    _ => None,
                };
                [Some(&prepared.signature), counterparts[0], counterparts[1]]
                    .into_iter()
                    .flatten()
                    .map(move |signature| CallableContractScope {
                        signature,
                        trait_scope,
                    })
            })
    }

    fn callable_application_owner_is_live(&self, prepared: &AppliedCallableSignature) -> bool {
        match prepared.owner {
            AppliedCallableOwner::RequirementInvocation { lexical_owner, .. }
            | AppliedCallableOwner::Realization { lexical_owner, .. } => {
                self.machine_by_symbol(lexical_owner).is_some()
            }
            _ => true,
        }
    }

    pub fn retain_live_trait_callable_applications(&mut self) {
        let mut live = Vec::new();
        for root in self.applied_trait_roots() {
            root.visit(self, &mut |scope| {
                if let Some(key) = scope.callable_contract_key(self) {
                    live.push(key);
                }
            });
        }
        let removed = self
            .applied_callable_signatures
            .iter()
            .filter_map(|(handle, prepared)| match prepared.owner {
                AppliedCallableOwner::Trait {
                    declaration,
                    application,
                    ..
                } if !live.contains(&(declaration, application)) => Some(handle),
                _ => None,
            })
            .collect::<Vec<_>>();
        for handle in removed {
            self.applied_callable_signatures.free(handle);
        }
    }

    pub fn remove_trait_declaration_applications(
        &mut self,
        removed: impl Fn(SymbolHandle) -> bool,
    ) {
        let applications = self
            .applied_callable_signatures
            .iter()
            .filter_map(|(handle, application)| match application.owner {
                AppliedCallableOwner::Trait { declaration, .. }
                | AppliedCallableOwner::RequirementInstance { declaration, .. }
                | AppliedCallableOwner::RequirementInvocation { declaration, .. }
                | AppliedCallableOwner::Realization { declaration, .. }
                    if removed(declaration) =>
                {
                    Some(handle)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        for application in applications {
            self.applied_callable_signatures.free(application);
        }
    }
}

/// A retained callable telescope and the exact outer scope that produced it.
/// This is source typing context, never a conformance or checked proof.
#[derive(Clone, Copy)]
pub struct CallableContractScope<'a> {
    pub signature: &'a signature::StateSignature,
    pub trait_scope: Option<crate::trait_definition::RetainedTraitScope>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub(crate) struct AppliedCallableSignature {
    owner: AppliedCallableOwner,
    signature: signature::StateSignature,
}

impl AppliedCallableSignature {
    pub(crate) fn requirement_instance_symbol(&self) -> Option<SymbolHandle> {
        matches!(self.owner, AppliedCallableOwner::RequirementInstance { .. })
            .then_some(self.signature.symbol)
    }

    pub(crate) fn requirement_template_symbol(&self) -> Option<SymbolHandle> {
        match self.owner {
            AppliedCallableOwner::RequirementInstance { template, .. } => Some(template),
            _ => None,
        }
    }

    pub(crate) fn signature(&self) -> &signature::StateSignature {
        &self.signature
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub(crate) enum AppliedCallableOwner {
    #[default]
    Absent,
    Realization {
        declaration: SymbolHandle,
        application: crate::type_identity::NormalizedTypeIdentity,
        lexical_owner: SymbolHandle,
    },
    Trait {
        declaration: SymbolHandle,
        application: crate::type_identity::NormalizedTypeIdentity,
        scope: crate::trait_definition::RetainedTraitScope,
    },
    RequirementInvocation {
        declaration: SymbolHandle,
        application: crate::static_applications::StaticMachineApplicationHandle,
        lexical_owner: SymbolHandle,
    },
    RequirementInstance {
        declaration: SymbolHandle,
        template: SymbolHandle,
        trait_application_identity: crate::type_identity::NormalizedTypeIdentity,
        portable_trait_application_identity: crate::type_identity::NormalizedTypeIdentity,
        portable_local_arguments: arena::HandleSpan<crate::type_identity::NormalizedTypeIdentity>,
        trait_scope: crate::trait_definition::RetainedTraitScope,
        trait_arguments: arena::HandleSpan<types::TypeReferenceHandle>,
        local_arguments: arena::HandleSpan<RequirementLocalArgument>,
        invocation_contract_parameters: arena::HandleSpan<data::TypeParameter>,
        application: crate::static_applications::StaticMachineApplicationHandle,
    },
    MachineSelection {
        invocation: crate::static_applications::StaticMachineApplicationHandle,
        ordinal: usize,
        demand: SymbolHandle,
        selected: SymbolHandle,
        provider: crate::static_applications::StaticMachineApplicationHandle,
        required: Option<signature::StateSignature>,
        source_required: signature::StateSignature,
        caller_operands: arena::HandleSpan<CallableSubstitution>,
        caller_bindings: arena::HandleSpan<crate::static_applications::StaticMachineBinding>,
        caller_lifetimes: arena::HandleSpan<StaticMachineLifetimeBinding>,
        caller_lifetime_declaration: SymbolHandle,
        caller_lifetime_owner: SymbolHandle,
        provider_lifetime_declaration: SymbolHandle,
        provider_lifetime_owner: SymbolHandle,
        provider_lifetimes: arena::HandleSpan<StaticMachineLifetimeBinding>,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct CallableSubstitution {
    pub parameter: SymbolHandle,
    pub actual: types::TypeReferenceHandle,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AppliedMachineCallableId(arena::Handle<AppliedCallableSignature>);

impl AppliedMachineCallableId {
    pub fn is_valid(self) -> bool {
        self.0.is_valid()
    }
}

pub struct AppliedMachineCallable<'a> {
    pub identity: AppliedMachineCallableId,
    pub invocation: crate::static_applications::StaticMachineApplicationHandle,
    pub ordinal: usize,
    pub demand: SymbolHandle,
    pub lexical_owner: SymbolHandle,
    pub provider: crate::static_applications::StaticMachineApplicationHandle,
    pub selected_entry: SymbolHandle,
    pub required: &'a signature::StateSignature,
    pub selected: &'a signature::StateSignature,
    pub source_required: &'a signature::StateSignature,
    pub caller_operands: &'a [CallableSubstitution],
    pub caller_lifetimes: &'a [StaticMachineLifetimeBinding],
}

#[derive(Default)]
pub(crate) struct CallableApplication {
    owned_signature: SymbolHandle,
    runtime_values: Vec<SymbolHandle>,
    region_scope: SymbolHandle,
    trait_region_application: Option<(crate::trait_definition::TraitApplicationEnvironment, usize)>,
    lifetime_declaration: SymbolHandle,
    lifetimes: Vec<CallableLifetimeBinding>,
    elided_lifetimes: Vec<crate::name::Identifier>,
    types: Vec<(SymbolHandle, types::TypeReferenceHandle)>,
    static_arguments: Vec<(SymbolHandle, crate::expression::StaticMachineArgument)>,
    static_argument_owner: SymbolHandle,
    selected_providers: Vec<(
        SymbolHandle,
        crate::static_applications::StaticMachineApplicationHandle,
    )>,
    symbols: Vec<(SymbolHandle, SymbolHandle)>,
    active_requirements: Vec<(SymbolHandle, SymbolHandle)>,
    source_binders: Vec<(crate::name::Identifier, SymbolHandle)>,
}

struct CallableLifetimeBinding {
    parameter: crate::name::Identifier,
    argument: crate::name::Identifier,
    region: crate::type_identity::LifetimeParameterIdentity,
    origin: CallableLifetimeOrigin,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CallableLifetimeOrigin {
    Lexical,
    Rigid,
    Captured,
}

struct ScopedCallableLifetime {
    name: crate::name::Identifier,
    region: crate::type_identity::LifetimeParameterIdentity,
    origin: CallableLifetimeOrigin,
}

impl CallableApplication {
    fn scoped_lifetime(
        &self,
        program: &TypedTrees,
        reference: types::TypeReferenceHandle,
        slot: usize,
        original: &crate::name::Identifier,
    ) -> Result<ScopedCallableLifetime, &'static str> {
        if let Some(region) = program
            .type_reference_table
            .operand_lifetimes(reference)
            .get(slot)
            .copied()
            .filter(|region| region.declaration.is_valid())
        {
            if let Some(binding) = self.lifetimes.iter().rev().find(|binding| {
                lifetime_parameter_identity(program, self.region_scope, binding.parameter.as_str())
                    == Some(region)
            }) {
                return Ok(ScopedCallableLifetime {
                    name: binding.argument.clone(),
                    region: binding.region,
                    origin: binding.origin,
                });
            }
            let (region, substituted) =
                if let Some((environment, preceding_frames)) = &self.trait_region_application {
                    environment
                        .resolved_lifetime_identity(program, region, *preceding_frames)
                        .ok_or("resolved callable region lost its exact supplying frame")?
                } else {
                    (region, false)
                };
            let name = crate::type_identity::lifetime_parameter_name(program, region)
                .ok_or("resolved callable region lost its declaration ordinal")?
                .clone();
            return Ok(ScopedCallableLifetime {
                name,
                region,
                origin: if substituted {
                    CallableLifetimeOrigin::Captured
                } else {
                    CallableLifetimeOrigin::Lexical
                },
            });
        }
        if let Some(binding) = self
            .lifetimes
            .iter()
            .rev()
            .find(|binding| binding.parameter == *original)
        {
            return Ok(ScopedCallableLifetime {
                name: binding.argument.clone(),
                region: binding.region,
                origin: binding.origin,
            });
        }
        let Some((environment, preceding_frames)) = &self.trait_region_application else {
            return Ok(ScopedCallableLifetime {
                name: original.clone(),
                region: Default::default(),
                origin: CallableLifetimeOrigin::Lexical,
            });
        };
        let (region, substituted) = environment
            .lifetime_identity(
                program,
                reference,
                *preceding_frames,
                original.as_str(),
                self.region_scope,
            )
            .ok_or("applied trait operand has no exact supplying region")?;
        let name = crate::type_identity::lifetime_parameter_name(program, region)
            .ok_or("applied trait operand lost its region declaration ordinal")?
            .clone();
        Ok(ScopedCallableLifetime {
            name,
            region,
            origin: if substituted {
                CallableLifetimeOrigin::Captured
            } else {
                CallableLifetimeOrigin::Lexical
            },
        })
    }

    fn lifetime(&self, original: &crate::name::Identifier) -> crate::name::Identifier {
        self.lifetimes
            .iter()
            .rev()
            .find(|binding| binding.parameter == *original)
            .map_or_else(|| original.clone(), |binding| binding.argument.clone())
    }

    fn symbol(&self, original: SymbolHandle) -> SymbolHandle {
        self.symbols
            .iter()
            .rev()
            .find(|(symbol, _)| *symbol == original)
            .map_or(original, |(_, replacement)| *replacement)
    }

    fn nominal_contract(
        &mut self,
        program: &mut TypedTrees,
        selected_trait: SymbolHandle,
        requirement: SymbolHandle,
        mut arguments: Vec<types::TypeReferenceHandle>,
    ) -> Result<data::MachineParameterContract, &'static str> {
        use crate::trait_definition::{AppliedTraitScope, TraitApplicationArguments};

        let receiving_arguments = arguments.clone();
        let (declaration, signature) = program
            .trait_machine_signature(requirement)
            .ok_or("nominal requirement lost its declaring trait")?;
        let declaring_trait = declaration.symbol;
        let mut clauses = Vec::new();
        let environment = if declaring_trait == selected_trait {
            None
        } else {
            let selected = program
                .trait_definition_by_symbol(selected_trait)
                .ok_or("nominal requirement lost its selected trait")?;
            let scope = AppliedTraitScope::new(
                program,
                selected,
                TraitApplicationArguments {
                    lexical_owner: self.region_scope,
                    enclosing_data_applications: &[],
                    arguments: &arguments,
                    lifetimes: &[],
                },
            )
            .ok_or("inherited nominal requirement has invalid selected trait arguments")?;
            let declaring = scope
                .declaring_scope(program, declaring_trait)
                .ok_or("inherited nominal requirement has no unique declaring trait application")?;
            clauses = declaring.callable_refinement_clauses(program, signature.name.as_str());
            Some(declaring.environment())
        };
        let previous_regions = self.trait_region_application.clone();
        if let Some(environment) = &environment {
            arguments = program.materialize_trait_application_arguments(environment)?;
            self.trait_region_application = Some((environment.clone(), environment.frame_count()));
        }
        let applied = if arguments.is_empty() && environment.is_none() {
            Ok(None)
        } else {
            self.nominal(program, declaring_trait, requirement, &arguments)
        };
        self.trait_region_application = previous_regions;
        let mut applied = applied?;
        if let Some(signature) = &mut applied {
            program.apply_callable_refinements(signature, &clauses)?;
        }
        Ok(data::MachineParameterContract::Nominal {
            receiving_trait: selected_trait,
            receiving_arguments,
            trait_definition: declaring_trait,
            requirement,
            trait_arguments: arguments,
            instantiated: applied.map(Box::new),
        })
    }

    fn nominal(
        &mut self,
        program: &mut TypedTrees,
        trait_symbol: SymbolHandle,
        requirement: SymbolHandle,
        arguments: &[types::TypeReferenceHandle],
    ) -> Result<Option<signature::StateSignature>, &'static str> {
        let Some(definition) = program.trait_definition_by_symbol(trait_symbol) else {
            return Ok(None);
        };
        let parameters = program
            .trait_type_parameters(definition)
            .iter()
            .map(|parameter| (parameter.name.clone(), parameter.symbol))
            .collect::<Vec<_>>();
        if parameters.len() != arguments.len() {
            return Ok(None);
        }
        let Some(declared) = program
            .trait_machine_signature_by_symbol(definition, requirement)
            .cloned()
        else {
            return Ok(None);
        };
        if self
            .active_requirements
            .contains(&(trait_symbol, requirement))
        {
            return Err(
                "recursive nominal callable application is not yet represented by cycle-aware applied telescopes",
            );
        }
        let previous_types = self.types.len();
        let previous_binders = self.source_binders.len();
        self.source_binders.extend(parameters.iter().cloned());
        self.types.extend(
            parameters
                .into_iter()
                .map(|(_, symbol)| symbol)
                .zip(arguments.iter().copied()),
        );
        self.active_requirements.push((trait_symbol, requirement));
        let applied = self.signature(program, &declared);
        self.active_requirements.pop();
        self.types.truncate(previous_types);
        self.source_binders.truncate(previous_binders);
        applied.map(Some)
    }
}

impl TypedTrees {
    pub(crate) fn apply_refinement_postconditions(
        &mut self,
        receiving: &mut signature::StateSignature,
        declared: &signature::StateSignature,
        environment: &crate::trait_definition::TraitApplicationEnvironment,
    ) -> Result<(), &'static str> {
        let parameters = self.state_signature_parameters(declared);
        let actual_parameters = self.state_signature_parameters(receiving);
        let type_parameters = self.state_signature_type_parameters(declared);
        let actual_types = self.state_signature_type_parameters(receiving);
        if parameters.len() != actual_parameters.len()
            || type_parameters.len() != actual_types.len()
            || declared.lifetime_parameters.len() != receiving.lifetime_parameters.len()
        {
            return Err("refinement postcondition lost its inherited callable telescope");
        }
        let symbols = std::iter::once((declared.symbol, receiving.symbol))
            .chain(
                parameters
                    .iter()
                    .zip(actual_parameters)
                    .map(|(formal, actual)| (formal.symbol, actual.symbol)),
            )
            .chain(
                type_parameters
                    .iter()
                    .zip(actual_types)
                    .map(|(formal, actual)| (formal.symbol, actual.symbol)),
            )
            .collect();
        let source_binders = parameters
            .iter()
            .map(|parameter| (parameter.name.clone(), parameter.symbol))
            .chain(
                type_parameters
                    .iter()
                    .map(|parameter| (parameter.name.clone(), parameter.symbol)),
            )
            .collect();
        let lifetimes = declared
            .lifetime_parameters
            .iter()
            .zip(&receiving.lifetime_parameters)
            .enumerate()
            .map(|(ordinal, (parameter, argument))| CallableLifetimeBinding {
                parameter: parameter.clone(),
                argument: argument.clone(),
                region: crate::type_identity::LifetimeParameterIdentity {
                    declaration: receiving.symbol,
                    ordinal,
                },
                origin: CallableLifetimeOrigin::Rigid,
            })
            .collect();
        let mut application = self.materialize_trait_application(environment)?;
        application.region_scope = declared.symbol;
        application.trait_region_application =
            Some((environment.clone(), environment.frame_count()));
        application.static_argument_owner = environment.argument_owner();
        application.symbols = symbols;
        application.source_binders = source_binders;
        application.lifetimes = lifetimes;
        let contracts = self.state_signature_contracts(receiving).to_vec();
        let additions = self.state_signature_contracts(declared).to_vec();
        receiving.contracts = Default::default();
        for contract in contracts {
            self.signature_contracts
                .append_to_span(&mut receiving.contracts, contract);
        }
        for mut contract in additions {
            if contract.kind != signature::SignatureContractKind::Ensures {
                return Err("refinement postconditions cannot add caller obligations");
            }
            contract.facts = application.facts(self, contract.facts)?;
            self.signature_contracts
                .append_to_span(&mut receiving.contracts, contract);
        }
        Ok(())
    }

    pub fn apply_callable_result_type(
        &mut self,
        original: types::TypeReferenceHandle,
        lifetimes: &[(crate::name::Identifier, Option<crate::name::Identifier>)],
        declaration: SymbolHandle,
    ) -> Result<types::TypeReferenceHandle, &'static str> {
        let mut application = CallableApplication {
            region_scope: declaration,
            ..CallableApplication::default()
        };
        for (parameter, argument) in lifetimes {
            if let Some(argument) = argument {
                application.lifetimes.push(CallableLifetimeBinding {
                    parameter: parameter.clone(),
                    argument: argument.clone(),
                    region: Default::default(),
                    origin: CallableLifetimeOrigin::Lexical,
                });
            } else {
                application.elided_lifetimes.push(parameter.clone());
            }
        }
        application.type_reference(self, original)
    }

    pub fn apply_callable_type(
        &mut self,
        original: types::TypeReferenceHandle,
        substitutions: &[(SymbolHandle, types::TypeReferenceHandle)],
        lifetimes: &[(crate::name::Identifier, crate::name::Identifier)],
        declaration: SymbolHandle,
    ) -> Result<types::TypeReferenceHandle, &'static str> {
        CallableApplication {
            region_scope: declaration,
            types: substitutions.to_vec(),
            lifetimes: lifetimes
                .iter()
                .map(|(parameter, argument)| CallableLifetimeBinding {
                    parameter: parameter.clone(),
                    argument: argument.clone(),
                    region: Default::default(),
                    origin: CallableLifetimeOrigin::Lexical,
                })
                .collect(),
            ..CallableApplication::default()
        }
        .type_reference(self, original)
    }

    pub fn apply_static_application_type(
        &mut self,
        original: types::TypeReferenceHandle,
        application: crate::static_applications::StaticMachineApplicationHandle,
        declaration: SymbolHandle,
    ) -> Result<types::TypeReferenceHandle, &'static str> {
        let source = self
            .expression_table
            .static_machine_application(application);
        let lifetimes = self
            .static_application_lifetimes_for_declaration(application, declaration)?
            .iter()
            .map(|binding| {
                let region = lifetime_parameter_identity(
                    self,
                    source.lexical_owner,
                    binding.argument.as_str(),
                )
                .ok_or("static callable result region lost its supplying declaration")?;
                Ok(CallableLifetimeBinding {
                    parameter: binding.parameter.clone(),
                    argument: binding.argument.clone(),
                    region,
                    origin: CallableLifetimeOrigin::Lexical,
                })
            })
            .collect::<Result<Vec<_>, &'static str>>()?;
        CallableApplication {
            region_scope: declaration,
            lifetimes,
            ..CallableApplication::default()
        }
        .type_reference(self, original)
    }

    pub fn apply_captured_callable_type(
        &mut self,
        original: types::TypeReferenceHandle,
        environment: &crate::trait_definition::TraitApplicationEnvironment,
        local_lifetimes: &[crate::name::Identifier],
        declaration: SymbolHandle,
    ) -> Result<types::TypeReferenceHandle, &'static str> {
        CallableApplication {
            region_scope: declaration,
            trait_region_application: Some((environment.clone(), environment.frame_count())),
            lifetimes: local_lifetimes
                .iter()
                .map(|name| {
                    let region = lifetime_parameter_identity(self, declaration, name.as_str())
                        .ok_or("captured callable type lost a declared local region")?;
                    Ok(CallableLifetimeBinding {
                        parameter: name.clone(),
                        argument: name.clone(),
                        region,
                        origin: CallableLifetimeOrigin::Rigid,
                    })
                })
                .collect::<Result<Vec<_>, &'static str>>()?,
            ..CallableApplication::default()
        }
        .type_reference(self, original)
    }

    pub fn copy_callable_parameter_kind(
        &mut self,
        kind: data::TypeParameterKind,
        symbols: &[(SymbolHandle, SymbolHandle)],
        declaration: SymbolHandle,
    ) -> Result<data::TypeParameterKind, &'static str> {
        CallableApplication {
            region_scope: declaration,
            symbols: symbols.to_vec(),
            ..CallableApplication::default()
        }
        .parameter_kind(self, kind)
    }

    pub fn instantiate_nominal_machine_parameter_contracts(
        &mut self,
        first_parameter: usize,
    ) -> Result<(), &'static str> {
        let pending = self
            .data_type_parameters
            .iter_from(first_parameter)
            .filter_map(|(handle, parameter)| match &parameter.kind {
                data::TypeParameterKind::Machine {
                    contract:
                        data::MachineParameterContract::Nominal {
                            receiving_trait,
                            receiving_arguments,
                            requirement,
                            instantiated: None,
                            ..
                        },
                } => Some((
                    handle,
                    *receiving_trait,
                    *requirement,
                    receiving_arguments.clone(),
                    parameter.symbol,
                )),
                _ => None,
            })
            .collect::<Vec<_>>();
        for (handle, selected_trait, requirement, arguments, parameter) in pending {
            let lexical_owner = self.declaration_scope_parent(parameter);
            let contract = CallableApplication {
                region_scope: lexical_owner,
                ..CallableApplication::default()
            }
            .nominal_contract(self, selected_trait, requirement, arguments)?;
            self.data_type_parameters.get_mut(handle).kind =
                data::TypeParameterKind::Machine { contract };
        }
        Ok(())
    }
}

/*
We represent closed callable applications (`callable_applications`), including
applied machine instances, trait and conformance scopes, domain scopes, and
requirement instances in `TypedTrees`.
*/

impl crate::TypedTrees {
    /// Land a static scalar at its binder's carrier before either describing
    /// or specializing the application. Names arrive already resolved; only
    /// the canonical value codec, never a rendered decimal name, supplies a value.
    pub fn prepare_callable_scalar_argument(
        &self,
        primitive: crate::types::PrimitiveType,
        argument: &crate::expression::StaticMachineArgument,
    ) -> Result<crate::expression::ExpressionNode, &'static str> {
        use crate::expression::ExpressionNode;
        use crate::types::PrimitiveType;
        use language_semantics::const_value::{CanonicalConstValue, DecodedCanonicalConstValue};
        use numerics::literals::{IntegerLanding, IntegerLiteral, IntegerRadix};

        if argument.application.is_some() || argument.evidence_projection.is_some() {
            return Err("a static scalar requires a closed value operand");
        }
        if let Some(literal) = &argument.const_literal
            && (argument.symbol.is_valid()
                || !argument.path.is_empty()
                || (argument.type_reference.is_valid()
                    && self
                        .static_const_literal_from_type_reference(argument.type_reference)
                        .as_ref()
                        != Some(literal)))
        {
            return Err("a scalar literal cannot also select another operand");
        }
        let literal = argument.const_literal.clone().or_else(|| {
            argument
                .type_reference
                .is_valid()
                .then(|| self.static_const_literal_from_type_reference(argument.type_reference))
                .flatten()
        });
        if let Some(literal) = literal {
            let landed_type = primitive
                .landed_landing_fixed()
                .ok_or("an integer argument requires an integer carrier")?;
            let landing = IntegerLanding {
                landed_type,
                domain: numerics::arithmetic::ArithmeticDomain::Exact,
            };
            if literal.landing().is_some_and(|stated| stated != landing)
                || !literal
                    .value_bignum()
                    .is_some_and(|value| landed_type.value_carrier().admits(&value))
            {
                return Err("a static integer is outside its declared carrier");
            }
            return Ok(ExpressionNode::Integer(literal.with_landing(landing)));
        }
        if primitive == PrimitiveType::Bool
            && let Some(value) = argument.closed_boolean()
        {
            return Ok(ExpressionNode::Boolean(value));
        }
        let canonical = if argument.symbol.is_valid() {
            if self
                .const_declaration_by_symbol(argument.symbol)
                .is_some_and(|declaration| {
                    !matches!(
                        self.type_reference_node(declaration.declared_type),
                        crate::types::TypeReferenceNode::Named { .. }
                    ) || self.primitive_type_reference(declaration.declared_type) != Some(primitive)
                })
            {
                return Err("a named constant does not have its binder's carrier");
            }
            crate::canonical_const_declaration_value(self, argument.symbol)
        } else if argument.type_reference.is_valid() {
            match self.type_reference_node(argument.type_reference) {
                crate::types::TypeReferenceNode::Named { symbol, name } if !symbol.is_valid() => {
                    CanonicalConstValue::from_atom(name.as_str())
                }
                crate::types::TypeReferenceNode::ConstExpression(value) => {
                    if let ExpressionNode::Boolean(value) = self.expression_table.expression(*value)
                    {
                        Some(CanonicalConstValue::boolean(*value))
                    } else {
                        None
                    }
                }
                _ => None,
            }
        } else {
            argument
                .path
                .first()
                .filter(|_| argument.path.len() == 1)
                .and_then(|name| CanonicalConstValue::from_atom(name.as_str()))
        }
        .ok_or("a static scalar requires its canonical closed value")?;
        match canonical
            .decode_encoding()
            .ok_or("a static scalar has a malformed canonical value")?
        {
            DecodedCanonicalConstValue::Integer {
                integer_type,
                value,
            } if primitive.landed_landing_fixed() == Some(integer_type) => {
                let text = value.to_string();
                let literal = IntegerLiteral::from_parts(
                    text.starts_with('-'),
                    IntegerRadix::Decimal,
                    text.trim_start_matches('-'),
                )
                .map_err(|_| "a canonical integer lost its literal representation")?;
                Ok(ExpressionNode::Integer(literal.with_landing(
                    IntegerLanding {
                        landed_type: integer_type,
                        domain: numerics::arithmetic::ArithmeticDomain::Exact,
                    },
                )))
            }
            DecodedCanonicalConstValue::Boolean(value) if primitive == PrimitiveType::Bool => {
                Ok(ExpressionNode::Boolean(value))
            }
            _ => Err("a static scalar does not have its binder's carrier"),
        }
    }

    /// Form the declaration's complete supported tuple without requesting an
    /// executable instance. The generic type node retains the selected owner
    /// and arguments in that owner's exact binder order, including landed values.
    pub(crate) fn prepare_callable_operand_application(
        &mut self,
        selected: &crate::expression::StaticMachineArgument,
        owner: symbols::SymbolHandle,
    ) -> Result<
        (
            crate::types::TypeReferenceHandle,
            Vec<crate::static_applications::StaticMachineBinding>,
        ),
        &'static str,
    > {
        use crate::data::TypeParameterKind;
        use crate::static_applications::{StaticMachineBinding, StaticMachineBindingValue};
        use crate::types::TypeReferenceNode;
        let owner = self.static_operand_region_supplier(selected, owner)?;
        let machine = self
            .machine_by_symbol(selected.symbol)
            .or_else(|| self.machine_holding_state(selected.symbol))
            .ok_or("a callable application requires its selected declaration")?;
        let declaration = machine.symbol;
        let name = machine.name.clone();
        let parameters = self.machine_callable_parameters(machine);
        let application = selected.application.as_deref();
        if application.is_some_and(|application| application.member.is_valid())
            || application.map_or(0, |application| application.arguments.len()) != parameters.len()
            || application.map_or(0, |application| application.lifetime_arguments.len())
                != machine.lifetime_parameters.len()
            || parameters
                .iter()
                .any(|parameter| parameter.bounds != crate::data::DataProperties::default())
            || (application.is_some()
                && (!machine.conformance_bounds.is_empty()
                    || !machine.declared_type_equations.is_empty()))
        {
            return Err("a callable application requires its complete admitted tuple and premises");
        }
        let Some(application) = application else {
            let reference = self.type_reference_table.insert(TypeReferenceNode::Named {
                symbol: declaration,
                name,
            });
            self.type_reference_table
                .retain_operand_lifetime_owner(reference, owner)?;
            return Ok((reference, Vec::new()));
        };
        let mut arguments = Vec::with_capacity(parameters.len());
        let mut bindings = Vec::with_capacity(parameters.len());
        for (parameter, argument) in parameters.iter().zip(&application.arguments) {
            let reference = match parameter.kind {
                TypeParameterKind::Type => {
                    if argument.const_literal.is_some()
                        || argument.evidence_projection.is_some()
                        || (!argument.type_reference.is_valid()
                            && (!self.symbols.symbols().nodes().is_valid(argument.symbol)
                                || !matches!(
                                    self.symbols.get(argument.symbol).kind,
                                    symbols::SymbolKind::BuiltinType
                                        | symbols::SymbolKind::Data
                                        | symbols::SymbolKind::TypeParameter
                                )))
                    {
                        return Err("a callable type argument requires its resolved type");
                    }
                    let reference = self.static_operand_type_reference_in_scope(argument, owner)?;
                    if matches!(
                        self.type_reference_node(reference),
                        TypeReferenceNode::ConstExpression(_)
                    ) {
                        return Err("a callable type argument cannot be a value");
                    }
                    reference
                }
                TypeParameterKind::Const { type_reference }
                | TypeParameterKind::Value { type_reference } => {
                    if !matches!(
                        self.type_reference_node(type_reference),
                        TypeReferenceNode::Named { .. }
                    ) {
                        return Err("a callable scalar requires its admitted declared carrier");
                    }
                    let primitive = self
                        .primitive_type_reference(type_reference)
                        .ok_or("a callable scalar requires its admitted carrier")?;
                    let value = self.prepare_callable_scalar_argument(primitive, argument)?;
                    let expression = self.expression_table.insert(value);
                    self.type_reference_table
                        .insert(TypeReferenceNode::ConstExpression(expression))
                }
                _ => {
                    return Err(
                        "a callable application requires its admitted callable or conformance argument",
                    );
                }
            };
            arguments.push(reference);
            bindings.push(StaticMachineBinding {
                parameter: parameter.symbol,
                value: if parameter.kind.is_const_or_value() {
                    StaticMachineBindingValue::Const(reference)
                } else {
                    StaticMachineBindingValue::Type(reference)
                },
            });
        }
        let arguments = self
            .type_reference_table
            .insert_type_reference_handles(arguments);
        let reference = self
            .type_reference_table
            .insert(TypeReferenceNode::Generic {
                base_symbol: declaration,
                base_name: name,
                lifetime_arguments: application.lifetime_arguments.to_vec(),
                arguments,
            });
        self.type_reference_table
            .retain_operand_lifetime_owner(reference, owner)?;
        Ok((reference, bindings))
    }
}

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
    }
}
pub(crate) mod domain_scopes {
    use super::CallableApplication;
    use crate::{TypedTrees, domain, types};
    use symbols::SymbolHandle;

    impl CallableApplication {
        pub(crate) fn domain_identity(
            &mut self,
            program: &mut TypedTrees,
            declaration: SymbolHandle,
            arguments: &[types::TypeReferenceHandle],
            original: language_semantics::SemanticDomainId,
        ) -> Result<language_semantics::SemanticDomainId, &'static str> {
            if !declaration.is_valid() {
                return Ok(original);
            }
            let domain = program
                .domain_definition(declaration)
                .ok_or("applied domain lost its declaration")?;
            if program.domain_type_parameters(domain).is_empty() {
                if !arguments.is_empty() {
                    return Err("unindexed domain received index arguments");
                }
                return Ok(domain.semantic_id);
            }
            // A fixed carrier's entire telescope is already explicit in
            // this packet. Only a carrier-generic family needs the retained
            // implicit carrier coordinate from its source application.
            if !domain::has_generic_carrier(program, domain) {
                return program
                    .retain_domain_arguments(declaration, arguments)
                    .map_err(|_| "copied domain indices have no canonical instance identity");
            }
            let retained = program
                .domain_arguments_for_instance(declaration, original)
                .ok_or("applied domain lost its complete carrier/index coordinate")?
                .to_vec();
            let complete = retained
                .into_iter()
                .map(|argument| self.type_reference(program, argument))
                .collect::<Result<Vec<_>, _>>()?;
            if arguments.len()
                != domain::index_parameters(
                    program,
                    program
                        .domain_definition(declaration)
                        .ok_or("applied domain lost its declaration")?,
                )
                .len()
            {
                return Err("applied domain has an incomplete explicit index telescope");
            };
            program
                .retain_domain_arguments(declaration, &complete)
                .map_err(|_| "copied domain arguments have no canonical instance identity")
        }
    }

    impl TypedTrees {
        /// Retains a domain's complete declaration application while the
        /// producer still knows the carrier. This captures meaning, not proof.
        pub fn retain_domain_application(
            &mut self,
            declaration: SymbolHandle,
            carrier: types::TypeReferenceHandle,
            arguments: &[types::TypeReferenceHandle],
        ) -> Result<language_semantics::SemanticDomainId, diagnostics::Diagnostic> {
            let definition = self.domain_definition(declaration).ok_or_else(|| {
                diagnostics::Diagnostic::error("domain application lost its declaration")
            })?;
            if arguments.len() != domain::index_parameters(self, definition).len() {
                return Err(diagnostics::Diagnostic::error(
                    "domain application has an incomplete index telescope",
                ));
            }
            if !self.contains_type_reference(carrier) {
                return Err(diagnostics::Diagnostic::error(
                    "domain application lost its exact carrier",
                ));
            }
            let mut complete = Vec::with_capacity(self.domain_type_parameters(definition).len());
            if domain::has_generic_carrier(self, definition) {
                complete.push(
                    self.type_reference_table
                        .unconstrained_type_reference(carrier),
                );
            }
            complete.extend_from_slice(arguments);
            self.retain_domain_arguments(declaration, &complete)
        }

        fn retain_domain_arguments(
            &mut self,
            declaration: SymbolHandle,
            arguments: &[types::TypeReferenceHandle],
        ) -> Result<language_semantics::SemanticDomainId, diagnostics::Diagnostic> {
            let definition = self.domain_definition(declaration).ok_or_else(|| {
                diagnostics::Diagnostic::error("domain application lost its declaration")
            })?;
            let parameters = self.domain_type_parameters(definition);
            let identity =
                domain::indexed_domain_instance_name(self, definition, parameters, arguments)?;
            if parameters.is_empty() {
                return Ok(definition.semantic_id);
            }
            let identity = self.semantic_domains.intern(&identity);
            if self.domain_application(declaration, identity).is_none() {
                let is_closed = arguments
                    .iter()
                    .all(|argument| domain::index_argument_is_closed(self, *argument));
                self.applied_domain_applications
                    .insert(domain::AppliedDomainApplication {
                        declaration,
                        identity,
                        arguments: arguments.to_vec(),
                        is_closed,
                        theory: None,
                    });
            }
            Ok(identity)
        }

        /// Applies every retained coordinate after callable specialization.
        /// Copied facts may retain further applications; the same queue serves
        /// them before the program is frozen for the lowered checker.
        pub fn prepare_domain_applications(&mut self) {
            let mut position = 0;
            while position < self.applied_domain_applications.len() {
                if self
                    .applied_domain_applications
                    .get(position)
                    .is_some_and(|row| row.theory.is_none())
                    && let Err(reason) = self.prepare_domain_application(position)
                {
                    self.applied_domain_applications
                        .set_theory(position, Err(reason.to_owned()));
                }
                position += 1;
            }
        }

        fn prepare_domain_application(&mut self, position: usize) -> Result<(), &'static str> {
            let retained = self
                .applied_domain_applications
                .get(position)
                .ok_or("domain application has no retained coordinate")?;
            let declaration = retained.declaration;
            let identity = retained.identity;
            let arguments = retained.arguments.clone();
            let mut theory = self
                .domain_definition(declaration)
                .ok_or("applied domain lost its declaration")?
                .clone();
            if !domain::has_direct_index_telescope(self, &theory) {
                return Err("domain index declaration is not its direct parameter telescope");
            }
            let parameters = self.domain_type_parameters(&theory).to_vec();
            if parameters.len() != arguments.len() {
                return Err("applied domain has an incomplete declaration telescope");
            }
            let substitutions = parameters
                .iter()
                .zip(&arguments)
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
                invocation_elision: None,
                types: substitutions,
                static_arguments: Vec::new(),
                static_argument_owner: SymbolHandle::invalid(),
                selected_providers: Vec::new(),
                symbols: source_subject
                    .is_valid()
                    .then_some((source_subject, subject))
                    .into_iter()
                    .collect(),
                declaration_symbols: Default::default(),
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
                .set_theory(position, Ok(theory));
            Ok(())
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
        fn domain_application_lookup_is_exact_and_rebuilt_from_stored_rows() {
            use compiler_storage::{RecordLimits, decode, encode};
            use language_semantics::SemanticDomainId;

            let mut applications = domain::AppliedDomainApplications::default();
            for (declaration, identity) in [
                (SymbolHandle::from_parts(7, 2), SemanticDomainId(42)),
                (SymbolHandle::from_parts(7, 3), SemanticDomainId(42)),
                (SymbolHandle::from_parts(7, 2), SemanticDomainId(41)),
            ] {
                assert!(applications.insert(domain::AppliedDomainApplication {
                    declaration,
                    identity,
                    arguments: Vec::new(),
                    is_closed: true,
                    theory: Some(Err(format!("refusal for {}", identity.0))),
                }));
            }
            let rows = applications.iter().cloned().collect::<Vec<_>>();
            assert!(!applications.insert(rows[0].clone()));
            let bytes = encode(&applications, RecordLimits::default()).unwrap();
            let restored: domain::AppliedDomainApplications =
                decode(&bytes, RecordLimits::default()).unwrap();
            assert_eq!(restored.iter().cloned().collect::<Vec<_>>(), rows);
            for row in restored.iter() {
                assert_eq!(restored.find(row.declaration, row.identity), Some(row));
            }
            assert!(
                restored
                    .find(rows[0].declaration, SemanticDomainId(40))
                    .is_none()
            );
            let malformed = encode(
                &vec![rows[0].clone(), rows[0].clone()],
                RecordLimits::default(),
            )
            .unwrap();
            let error =
                decode::<domain::AppliedDomainApplications>(&malformed, RecordLimits::default())
                    .unwrap_err();
            assert!(
                error
                    .to_string()
                    .contains("duplicate exact domain application"),
                "{error}"
            );
        }

        #[test]
        fn a_generic_domain_carrier_is_its_binder_not_a_matching_diagnostic_name() {
            let mut symbols = SymbolTableBuilder::new();
            let nominal = symbols.insert_root(SymbolKind::Data, SymbolNameRef::Static("T"));
            let binder =
                symbols.insert_root(SymbolKind::TypeParameter, SymbolNameRef::Static("Binder"));
            let mut program = TypedTrees {
                symbols: symbols.finish(),
                ..Default::default()
            };
            let parameters = program.data_type_parameters.insert_many([TypeParameter {
                symbol: binder,
                name: Identifier::generated("T"),
                kind: TypeParameterKind::Type,
                ..Default::default()
            }]);
            let mut definition = domain::DomainDefinition {
                type_parameters: parameters,
                ..Default::default()
            };
            for (symbol, expected) in [
                (binder, true),
                (nominal, false),
                (SymbolHandle::invalid(), false),
            ] {
                definition.target_type =
                    program
                        .type_reference_table
                        .insert(TypeReferenceNode::Named {
                            symbol,
                            name: Identifier::generated("T"),
                        });
                assert_eq!(domain::has_generic_carrier(&program, &definition), expected);
            }
        }

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
            let original = program
                .retain_domain_application(domain_symbol, carrier, &[operand])
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
            let expected = program
                .retain_domain_application(domain_symbol, carrier, &[carrier])
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

    const MISSING_EXECUTABLE_TEMPLATE: &str = "executable application lost its template machine";

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
        pub fn binding_application(
            self,
        ) -> crate::static_applications::StaticMachineApplicationHandle {
            match self {
                Self::Specialization(receipt) => receipt.binding_application,
                Self::Capture(receipt) => receipt.binding_application,
            }
        }

        /// Rejoins an executable entry to the exact declaration retained when
        /// its application was produced. State order and symbol provenance do
        /// not supply missing correspondence.
        pub fn entry_template(
            self,
            program: &TypedTrees,
            entry: SymbolHandle,
        ) -> Result<SymbolHandle, &'static str> {
            let span = match self {
                Self::Specialization(receipt) => receipt.states,
                Self::Capture(receipt) => receipt.states,
            };
            let template = self.template();
            program.validate_executable_template_symbol(template)?;
            let source = program
                .machine(template)
                .ok_or(MISSING_EXECUTABLE_TEMPLATE)?;
            let instance = program
                .machine(self.instance())
                .ok_or("executable application lost its instance machine")?;
            let source_states = program.machine_states(source);
            let instance_states = program.machine_states(instance);
            let states = program
                .captured_machine_states
                .span(span)
                .ok_or("executable application lost its entry correspondence")?;
            if states.len() != source_states.len() || states.len() != instance_states.len() {
                return Err("executable application has incomplete entry correspondence");
            }
            let mut selected = states.iter().filter(|state| state.instance == entry);
            let (Some(mapping), None) = (selected.next(), selected.next()) else {
                return Err("executable application has no unique entry correspondence");
            };
            if !instance_states.iter().any(|state| state.symbol == entry)
                || !source_states
                    .iter()
                    .any(|state| state.symbol == mapping.template)
            {
                return Err("executable application entry belongs to another template");
            }
            Ok(mapping.template)
        }
    }
    impl TypedTrees {
        /// Ownership discovery can use a template before reading its entry.
        /// Both reads require the exact retained symbol generation.
        pub fn validate_executable_template_symbol(
            &self,
            template: SymbolHandle,
        ) -> Result<(), &'static str> {
            self.symbols
                .symbols()
                .nodes()
                .is_valid(template)
                .then_some(())
                .ok_or(MISSING_EXECUTABLE_TEMPLATE)
        }

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
                .for_instance(instance)
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
                let Ok(template) = receipt.entry_template(self, current) else {
                    return false;
                };
                current = template;
            }
            current == source_state
        }
    }

    impl TypedTrees {
        pub fn static_machine_binding_position(
            &self,
            mut owner: SymbolHandle,
            binder: SymbolHandle,
            target: SymbolHandle,
        ) -> Result<Option<u32>, &'static str> {
            if !binder.is_valid() || self.retained_static_machine_contract(binder).is_none() {
                return Ok(None);
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
                let mut instances = self.machine_specializations.for_instance(owner);
                let Some(instance) = instances.next() else {
                    return Ok(None);
                };
                if instances.next().is_some() {
                    return Err("static call has ambiguous invocation ownership");
                }
                if let Some(crate::static_applications::StaticMachineBindingValue::Machine(
                    argument,
                )) = self.specialization_binding(instance, binder)
                {
                    if argument.application.is_some()
                        || entry(argument.symbol)
                            .is_none_or(|selected| entry(target) != Some(selected))
                    {
                        return Ok(None);
                    }
                    return self
                        .data_type_parameters
                        .span_or_empty(instance.template_parameters)
                        .iter()
                        .position(|parameter| {
                            parameter.symbol == binder
                                && matches!(
                                    parameter.kind,
                                    crate::data::TypeParameterKind::Machine { .. }
                                )
                        })
                        .map(u32::try_from)
                        .transpose()
                        .map_err(|_| "static call has too many application binders");
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
            TypedTrees,
            static_applications::{
                StaticMachineApplicationHandle, StaticMachineBinding, StaticMachineBindingValue,
            },
            types::TypeReferenceHandle,
        };
        use symbols::SymbolHandle;

        impl TypedTrees {
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

            pub(crate) fn same_registered_machine_operation(
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

    #[cfg(test)]
    pub(crate) mod record_tests {
        use crate::callable_applications::{AppliedCallableOwner, AppliedCallableSignature};
        use crate::expression::{StaticMachineArgument, StaticSymbolApplication};
        use crate::name::Identifier;
        use crate::static_applications::{StaticMachineBinding, StaticMachineBindingValue};
        use crate::{TypedTrees, signature};
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

    type CaptureKey = (
        SymbolHandle,
        crate::type_identity::NormalizedTypeIdentity,
        SymbolHandle,
    );

    /// A transient sparse index of existing capture records. Entries retain
    /// handles only; all source and body correspondence stays in the record.
    #[derive(Default)]
    pub struct NamedConformanceCaptureIndex {
        rows: symbols::SymbolKeyMap<
            CaptureKey,
            Option<arena::Handle<crate::CapturedMachineApplication>>,
        >,
    }

    impl NamedConformanceCaptureIndex {
        pub fn new(program: &TypedTrees) -> Self {
            let mut index = Self::default();
            for (handle, _) in program.captured_machine_applications.iter() {
                // Unrelated unsupported records grant no authority. A demanded
                // named application still needs one valid, unambiguous entry.
                let _ = index.insert(program, handle);
            }
            index
        }

        pub fn insert(
            &mut self,
            program: &TypedTrees,
            handle: arena::Handle<crate::CapturedMachineApplication>,
        ) -> Result<(), &'static str> {
            let key = Self::record_key(program, handle)?;
            self.rows
                .entry(key)
                .and_modify(|known| {
                    if *known != Some(handle) {
                        *known = None;
                    }
                })
                .or_insert(Some(handle));
            Ok(())
        }

        pub fn get<'program>(
            &self,
            program: &'program TypedTrees,
            call: &TableCallExpression,
        ) -> Result<Option<&'program crate::CapturedMachineApplication>, &'static str> {
            let Some((_, row, _)) = program.named_conformance_call_member(call)? else {
                return Ok(None);
            };
            let Some((owner, identity)) = program.named_conformance_capture_key(call)? else {
                return Ok(None);
            };
            let Some(selected) = self.rows.get(&(owner, identity, row.realization_machine)) else {
                return Ok(None);
            };
            let handle = selected.ok_or("named application has ambiguous captured bodies")?;
            if !program.captured_machine_applications.is_valid(handle) {
                return Err("named application lost its captured body record");
            }
            Ok(Some(program.captured_machine_applications.get(handle)))
        }

        fn record_key(
            program: &TypedTrees,
            handle: arena::Handle<crate::CapturedMachineApplication>,
        ) -> Result<CaptureKey, &'static str> {
            use crate::expression::{StaticMachineArgument, StaticSymbolApplication};
            if !program.captured_machine_applications.is_valid(handle) {
                return Err("named capture has a foreign record");
            }
            let capture = program.captured_machine_applications.get(handle);
            if capture.template == capture.instance
                || !program
                    .expression_table
                    .static_machine_application_is_valid(capture.binding_application)
                || !program
                    .expression_table
                    .static_machine_application_is_valid(capture.supplying_application)
            {
                return Err("named capture has no actual instantiation applications");
            }
            let source = program.static_machine_application(capture.binding_application);
            let supplying = program.static_machine_application(capture.supplying_application);
            let declaration = capture.conformance_application.declaration;
            if source.registration_operation != declaration
                || source.binding_declaration != declaration
                || source.lexical_owner != supplying.lexical_owner
                || !program
                    .symbols
                    .symbols()
                    .nodes()
                    .is_valid(source.lexical_owner)
            {
                return Err("named capture changed its constructor or source owner");
            }
            let (implementation, row) = program
                .owned_conformance_realization(capture.template)
                .ok_or("named capture lost its unique owned template")?;
            if implementation.symbol != declaration
                || supplying.registration_operation != row.requirement
            {
                return Err("named capture changed its actual requirement row");
            }
            let identity = |arguments: &[StaticMachineArgument]| {
                let selected = StaticMachineArgument {
                    symbol: declaration,
                    application: Some(Box::new(StaticSymbolApplication {
                        member: arena::Handle::invalid(),
                        lifetime_arguments: Box::new([]),
                        arguments: arguments.into(),
                    })),
                    ..Default::default()
                };
                crate::type_identity::applied_index_identity(
                    program,
                    crate::type_identity::IndexIdentityArgument::StaticArgument(&selected),
                    source.lexical_owner,
                    &[],
                )
            };
            let selected = identity(&capture.conformance_application.arguments)
                .ok_or("named capture lost its complete map identity")?;
            let parameters = program.conformance_type_parameters(implementation);
            let bindings = program.static_machine_application_bindings(capture.binding_application);
            if bindings.len() != parameters.len()
                || capture.conformance_application.arguments.len() != parameters.len()
                || parameters
                    .iter()
                    .zip(bindings)
                    .zip(capture.conformance_application.arguments.iter())
                    .any(|((parameter, binding), argument)| {
                        binding.parameter != parameter.symbol
                            || !program.static_binding_matches_argument(
                                &parameter.kind,
                                &binding.value,
                                argument,
                                source.lexical_owner,
                                source.lexical_owner,
                            )
                    })
            {
                return Err("named capture changed its actual constructor bindings");
            }
            if Some(selected)
                != identity(
                    program.static_machine_application_arguments(capture.binding_application),
                )
                || program
                    .type_identity_text(selected)
                    .as_deref()
                    .map(str::as_bytes)
                    != Some(capture.conformance_application.identity_encoding.as_ref())
            {
                return Err("named capture constructor disagrees with its complete map tuple");
            }
            Ok((source.lexical_owner, selected, capture.template))
        }
    }

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
        /// The call retains a named map's independently supplied telescope in
        /// its requirement application, and the source application owns the
        /// lexical scope. Neither the receiver's name nor the expected result
        /// supplies an omitted argument.
        pub fn named_conformance_call_member(
            &self,
            call: &TableCallExpression,
        ) -> Result<
            Option<(
                &crate::trait_definition::Conformance,
                &crate::trait_definition::ConformanceRow,
                &signature::StateSignature,
            )>,
            &'static str,
        > {
            // Parameter dispatch already owns its requirement and realization.
            // A nominal receiver cannot reinterpret that independent selection.
            if call.static_requirement_dispatch.is_some() {
                return Ok(None);
            }
            let Some(receiver) = self.expression_table.name_path_node(call.receiver) else {
                return Ok(None);
            };
            if !call.requirement_application.is_valid() {
                return self.named_conformance_member(receiver.symbol, call.target_symbol);
            }
            if !receiver.symbol.is_valid()
                || !self.symbols.symbols().nodes().is_valid(receiver.symbol)
            {
                return Err("named application has no exact receiver");
            }
            if self.symbols.get(receiver.symbol).kind != SymbolKind::Conformance {
                return Ok(None);
            }
            let selected = self
                .conformance_by_symbol(receiver.symbol)
                .ok_or("named application lost its implementation")?;
            let parameters = self.conformance_type_parameters(selected);
            if parameters.is_empty() != selected.type_parameters.is_empty()
                || !selected.lifetime_parameters.is_empty()
                || parameters.iter().any(|parameter| {
                    parameter.kind != crate::data::TypeParameterKind::Type
                        || parameter.bounds != crate::data::DataProperties::default()
                })
            {
                return Err("named application needs its complete eligible type telescope");
            }
            if !self
                .type_reference_table
                .contains_type_reference(call.requirement_application)
                || !self
                    .expression_table
                    .static_machine_application_is_valid(call.authored_machine_application)
            {
                return Err("named application changed its retained map or source application");
            }
            let arguments = match self.type_reference_node(call.requirement_application) {
                crate::types::TypeReferenceNode::Named { symbol, .. }
                    if *symbol == receiver.symbol && parameters.is_empty() =>
                {
                    &[][..]
                }
                crate::types::TypeReferenceNode::Generic {
                    base_symbol,
                    lifetime_arguments,
                    arguments,
                    ..
                } if *base_symbol == receiver.symbol && lifetime_arguments.is_empty() => {
                    self.type_reference_handles(*arguments)
                }
                _ => return Err("named application changed its complete map constructor"),
            };
            if arguments.len() != parameters.len()
                || arguments.iter().any(|argument| {
                    !self.type_reference_table.contains_type_reference(*argument)
                        || self
                            .region_free_scoped_type_identity(
                                crate::type_identity::TypeIdentityRequest::ordinary(*argument),
                                &[],
                            )
                            .is_none()
                })
            {
                return Err("named application lost a complete region-free map argument");
            }
            let source = self.static_machine_application(call.authored_machine_application);
            let template = self
                .closed_requirement_application(call.target_symbol)
                .map_or(call.target_symbol, |closed| closed.template);
            // A bodyless trait requirement is no machine declaration, so
            // source retention leaves its local binding declaration absent.
            // Its registered operation and supplying owner remain exact.
            if source.registration_operation != template
                || (source.binding_declaration.is_valid() && source.binding_declaration != template)
                || !source.bindings.is_empty()
                || !source.lifetime_bindings.is_empty()
                || !source.lexical_owner.is_valid()
                || !self
                    .symbols
                    .symbols()
                    .nodes()
                    .is_valid(source.lexical_owner)
            {
                return Err("named application changed its requirement or supplying owner");
            }
            let scope = self
                .applied_requirement_scope(source.lexical_owner, call.requirement_application)
                .ok_or("named application lost its complete trait application")?;
            self.named_conformance_member_at(receiver.symbol, call.target_symbol, Some(scope))
        }

        /// Include every map argument, even one unused by the trait scope,
        /// and its actual supplying owner when reusing a captured body.
        /// This key identifies a selection; it grants no body proof.
        pub fn named_conformance_capture_key(
            &self,
            call: &TableCallExpression,
        ) -> Result<
            Option<(SymbolHandle, crate::type_identity::NormalizedTypeIdentity)>,
            &'static str,
        > {
            if self.named_conformance_call_member(call)?.is_none()
                || !call.requirement_application.is_valid()
            {
                return Ok(None);
            }
            let source = self.static_machine_application(call.authored_machine_application);
            let identity = crate::type_identity::applied_index_identity(
                self,
                crate::type_identity::IndexIdentityArgument::TypeReference(
                    call.requirement_application,
                ),
                source.lexical_owner,
                &[],
            )
            .ok_or("named application lost its complete capture identity")?;
            Ok(Some((source.lexical_owner, identity)))
        }

        /// Retain the actual map constructor bindings and source scope before
        /// its body is copied. These are source data, not checked facts or an
        /// assertion that a conformance body was accepted.
        pub fn prepare_named_conformance_application(
            &mut self,
            call: &TableCallExpression,
        ) -> Result<Option<crate::ClosedConformanceApplication>, &'static str> {
            use crate::static_applications::{StaticMachineBinding, StaticMachineBindingValue};
            use sha2::{Digest, Sha256};
            let Some((owner, identity)) = self.named_conformance_capture_key(call)? else {
                return Ok(None);
            };
            let encoding = self
                .type_identity_text(identity)
                .ok_or("named application lost its canonical encoding")?;
            let crate::types::TypeReferenceNode::Generic {
                base_symbol,
                arguments,
                ..
            } = self.type_reference_node(call.requirement_application)
            else {
                return Err("named application lost its complete constructor");
            };
            let declaration = *base_symbol;
            let arguments = self.type_reference_handles(*arguments).to_vec();
            let selected = self
                .conformance_by_symbol(declaration)
                .ok_or("named application lost its source declaration")?;
            let parameters = self
                .conformance_type_parameters(selected)
                .iter()
                .map(|parameter| parameter.symbol)
                .collect::<Vec<_>>();
            let rows = selected
                .rows
                .iter()
                .map(|row| crate::ClosedConformanceRowIdentity {
                    declaring_trait: row.declaring_trait,
                    requirement: row.requirement,
                    realization_machine: row.realization_machine,
                    realization_state: row.realization_state,
                })
                .collect();
            let scope = self
                .applied_requirement_scope(owner, call.requirement_application)
                .ok_or("named application lost its source scope")?;
            let trait_definition = scope.declaration.symbol;
            let trait_arguments = self
                .trait_type_parameters(scope.declaration)
                .iter()
                .map(|parameter| {
                    self.normalized_type_identity_text(
                        scope.parameter_argument(self, parameter.symbol),
                    )
                })
                .collect();
            let environment = scope.environment();
            let type_arguments = arguments
                .iter()
                .map(|argument| self.normalized_type_identity_text(*argument))
                .collect();
            let operands = arguments
                .iter()
                .map(|reference| {
                    let mut argument = self.static_operand_from_type_reference(*reference)?;
                    argument.type_reference = *reference;
                    Ok(argument)
                })
                .collect::<Result<Vec<_>, &'static str>>()?;
            let bindings = parameters
                .into_iter()
                .zip(&arguments)
                .map(|(parameter, argument)| StaticMachineBinding {
                    parameter,
                    value: StaticMachineBindingValue::Type(*argument),
                })
                .collect::<Vec<_>>();
            let scope = environment.retain(self)?;
            let name = crate::name::Identifier::generated(self.symbols.name(declaration));
            let application = self.expression_table.retain_static_machine_application(
                declaration,
                name,
                &operands,
            );
            self.expression_table
                .set_static_machine_application_bindings(application, &bindings);
            let source = self
                .expression_table
                .static_machine_application_mut(application);
            source.lexical_owner = owner;
            source.binding_declaration = declaration;
            let mut fingerprint = semantic_vocabulary::Fnv1a::new();
            fingerprint.string_framed(&encoding);
            let mut digest = Sha256::new();
            digest.update(b"OMEGA-TYPED-CONFORMANCE-APPLICATION\0");
            digest.update(encoding.as_bytes());
            Ok(Some(crate::ClosedConformanceApplication {
                declaration,
                trait_scope: scope,
                callable_application: application,
                arguments: operands.into_boxed_slice(),
                lifetime_arguments: Vec::new(),
                type_arguments,
                const_arguments: Vec::new(),
                machine_arguments: Vec::new(),
                trait_definition,
                trait_lifetime_arguments: Vec::new(),
                trait_arguments,
                rows,
                report_fingerprint: fingerprint.finish(),
                commitment: crate::ClosedConformanceApplicationCommitment::from_digest(
                    digest.finalize().into(),
                ),
                identity_encoding: std::sync::Arc::from(encoding.into_bytes()),
            }))
        }

        /// The named operation record, its unique requirement row and that
        /// row's public signature at the retained application. This identifies
        /// a choice; body validity and invocation authority remain separate.
        pub fn named_conformance_member(
            &self,
            namespace: SymbolHandle,
            requirement: SymbolHandle,
        ) -> Result<
            Option<(
                &crate::trait_definition::Conformance,
                &crate::trait_definition::ConformanceRow,
                &signature::StateSignature,
            )>,
            &'static str,
        > {
            self.named_conformance_member_at(namespace, requirement, None)
        }

        fn named_conformance_member_at(
            &self,
            namespace: SymbolHandle,
            requirement: SymbolHandle,
            application: Option<AppliedTraitScope<'_>>,
        ) -> Result<
            Option<(
                &crate::trait_definition::Conformance,
                &crate::trait_definition::ConformanceRow,
                &signature::StateSignature,
            )>,
            &'static str,
        > {
            if !namespace.is_valid() {
                return Ok(None);
            }
            if !self.symbols.symbols().nodes().is_valid(namespace) {
                return Err("named public view has a foreign namespace");
            }
            if self.symbols.get(namespace).kind != SymbolKind::Conformance {
                return Ok(None);
            }
            let selected = self
                .conformance_by_symbol(namespace)
                .ok_or("named public view lost its selected implementation")?;
            // Applied generic maps retain their independently supplied tuple in
            // the ordinary dispatch path above; a bare declaration cannot supply it.
            if application.is_none()
                && (!selected.type_parameters.is_empty()
                    || !selected.lifetime_parameters.is_empty())
            {
                return Err("named public view has no complete selected application");
            }
            let closed = self.closed_requirement_application(requirement);
            let template = closed
                .as_ref()
                .map_or(requirement, |selected| selected.template);
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
            let machine = self
                .machine_by_symbol(row.realization_machine)
                .ok_or("named public view lost its selected machine")?;
            let [entry] = self.machine_states(machine) else {
                return Err("named public view needs one exact selected entry");
            };
            if entry.symbol != row.realization_state {
                return Err("named public view changed its selected entry");
            }
            // An inline member belongs to this exact row. A reference row
            // instead retains an already declared machine and its own contract;
            // identifying either choice does not prove its refinement.
            if row.source.owns_realization() {
                let (owner, actual) = self
                    .owned_conformance_realization(row.realization_machine)
                    .ok_or("named public view lost its owned realization")?;
                if owner.symbol != selected.symbol || actual != row {
                    return Err("named public view changed its owned realization");
                }
            }
            let scope = application
                .or_else(|| AppliedTraitScope::from_conformance(self, selected))
                .ok_or("named public view lost its complete trait application")?
                .declaring_scope(self, row.declaring_trait)
                .ok_or("named public view lost its declaring trait scope")?;
            if closed.is_some() && !self.requirement_application_matches_scope(requirement, &scope)
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
                .map(|signature| Some((selected, row, signature)))
                .ok_or("named public view has no prepared applied signature")
        }
    }
}
pub use machine_instances::MachineInstantiation;
pub use namespaces::NamedConformanceCaptureIndex;
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
                    let count = program
                        .unique_data_definition_by_symbol(symbol)
                        .map_or(0, |data| data.lifetime_parameters.len());
                    if self.invocation_elision.is_some() && count != 0 {
                        let regions = (0..count)
                            .map(|_| self.invocation_region(program))
                            .collect::<Result<Vec<_>, _>>()?;
                        applied_owner = regions[0].region.declaration;
                        applied_lifetimes.extend(regions.iter().map(|region| region.region));
                        types::TypeReferenceNode::Generic {
                            base_symbol: self.symbol(symbol),
                            base_name: name,
                            lifetime_arguments: regions
                                .into_iter()
                                .map(|region| region.name)
                                .collect(),
                            arguments: Default::default(),
                        }
                    } else {
                        types::TypeReferenceNode::Named {
                            symbol: self.symbol(symbol),
                            name,
                        }
                    }
                }
                types::TypeReferenceNode::Reference {
                    referee,
                    access,
                    lifetime,
                } => {
                    // Reserve the outer slot before traversing its referent,
                    // exactly as signature identity and stage 05 read inputs.
                    let lifetime = if lifetime.is_none() && self.invocation_elision.is_some() {
                        Some(self.invocation_region(program)?)
                    } else {
                        lifetime
                            .as_ref()
                            .filter(|lifetime| !self.elided_lifetimes.contains(lifetime))
                            .map(|lifetime| self.scoped_lifetime(program, original, 0, lifetime))
                            .transpose()?
                    };
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
                                if domain.subject == types::DomainConstraintSubject::Declared
                                    && program.domain_definition(domain.symbol).is_some()
                                {
                                    domain.semantic_id = program.retain_domain_application(
                                        domain.symbol, base_type, &domain.arguments,
                                    ).map_err(|_| "copied qualification lost its complete domain application")?;
                                }
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
                    let lifetimes = if lifetime_arguments.is_empty()
                        && self.invocation_elision.is_some()
                    {
                        let count = program
                            .unique_data_definition_by_symbol(base_symbol)
                            .map_or(0, |data| data.lifetime_parameters.len());
                        let regions = (0..count)
                            .map(|_| self.invocation_region(program))
                            .collect::<Result<Vec<_>, _>>()?;
                        if let Some(first) = regions.first() {
                            applied_owner = first.region.declaration;
                        }
                        regions
                    } else {
                        lifetime_arguments
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
                        if self.invocation_elision.is_some() && program.type_reference_table.operand_lifetimes(original).get(slot).is_some_and(|region| !region.declaration.is_valid()) {
                            return self.invocation_region(program);
                        }
                        self.scoped_lifetime(program, original, slot, lifetime)
                    })
                    .collect::<Result<Vec<_>, _>>()?
                    };
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
            symbols.extend(self.declaration_symbols.rebinding().iter().copied());
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
            let call_targets = self.expression_call_targets(program, &rows)?;
            program.expression_table.remap_symbols_in(copied, &symbols);
            self.apply_expression_rows(program, &rows, call_targets)?;
            Ok(copied)
        }

        pub(crate) fn expression_call_targets(
            &self,
            program: &TypedTrees,
            rows: &[expression::ExpressionHandle],
        ) -> Result<Vec<Option<super::call_targets::AppliedCallTarget>>, &'static str> {
            rows.iter()
                .map(
                    |handle| match program.expression_table.expression(*handle) {
                        expression::ExpressionNode::Call(call) => {
                            self.selected_call_target(program, call.target_symbol)
                        }
                        _ => Ok(None),
                    },
                )
                .collect()
        }

        /// Specialization and contract application share the ownership of all
        /// types retained by copied expressions. Keep this exhaustive over node
        /// kinds so a new node cannot silently retain a template's type rows.
        pub(crate) fn apply_expression_rows(
            &mut self,
            program: &mut TypedTrees,
            rows: &[expression::ExpressionHandle],
            call_targets: Vec<Option<super::call_targets::AppliedCallTarget>>,
        ) -> Result<(), &'static str> {
            for (&handle, call_target) in rows.iter().zip(call_targets) {
                let carrier = program
                    .expression_table
                    .constructed_value_type_reference(handle);
                if carrier.is_valid() {
                    let carrier = self.type_reference(program, carrier)?;
                    program
                        .expression_table
                        .set_constructed_value_type_reference(handle, carrier);
                }
                let required = program
                    .expression_table
                    .construction_required_type_reference(handle);
                if required.is_valid() {
                    let required = self.type_reference(program, required)?;
                    program
                        .expression_table
                        .set_construction_required_type_reference(handle, required);
                }
                let node = program.expression_table.expression(handle).clone();
                let applied = match node {
                    expression::ExpressionNode::Name(path) => {
                        if path.static_application.is_valid() {
                            let mut argument = program
                                .expression_table
                                .static_member(path.static_application)
                                .ok_or("copied name lost its retained static operand")?
                                .clone();
                            self.static_arguments(program, std::slice::from_mut(&mut argument))?;
                            *program
                                .expression_table
                                .static_member_mut(path.static_application)
                                .ok_or("copied name lost its retained static operand")? = argument;
                        }
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
                    expression::ExpressionNode::StructLiteral(mut literal) => {
                        literal.application_type =
                            self.type_reference(program, literal.application_type)?;
                        Some(expression::ExpressionNode::StructLiteral(literal))
                    }
                    expression::ExpressionNode::Match(mut dispatch) => {
                        let mut arms = program.expression_table.match_arms(dispatch.arms).to_vec();
                        for arm in &mut arms {
                            if let expression::MatchPattern::Destructure(ref mut pattern) =
                                arm.pattern
                            {
                                pattern.application_type =
                                    self.type_reference(program, pattern.application_type)?;
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
                        let arguments = program
                            .type_reference_table
                            .type_reference_handles(cast.semantic_domain_arguments)
                            .to_vec();
                        cast.semantic_domain_id = self.domain_identity(
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
                    expression::ExpressionNode::ZeroValue(reference) => {
                        Some(expression::ExpressionNode::ZeroValue(
                            self.type_reference(program, reference)?,
                        ))
                    }
                    expression::ExpressionNode::Call(mut call) => {
                        call.requirement_application =
                            self.type_reference(program, call.requirement_application)?;
                        call.exact_coercion_target =
                            self.type_reference(program, call.exact_coercion_target)?;
                        self.static_arguments(program, &mut call.machine_arguments)?;
                        for argument in call.request_static_arguments_mut() {
                            self.static_arguments(program, std::slice::from_mut(argument))?;
                        }
                        for application in
                            crate::references::StaticArgumentCall::Expression(handle, &call)
                                .applications()
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
                    expression::ExpressionNode::SavedSubject(_)
                    | expression::ExpressionNode::ArrayLiteral(_)
                    | expression::ExpressionNode::Atomic(_)
                    | expression::ExpressionNode::Binary(_)
                    | expression::ExpressionNode::Boolean(_)
                    | expression::ExpressionNode::Float(_)
                    | expression::ExpressionNode::UndeterminedNaN(_)
                    | expression::ExpressionNode::Indexed(_)
                    | expression::ExpressionNode::Integer(_)
                    | expression::ExpressionNode::Member(_)
                    | expression::ExpressionNode::Borrow(_)
                    | expression::ExpressionNode::Range(_)
                    | expression::ExpressionNode::String(_)
                    | expression::ExpressionNode::Unary(_) => None,
                };
                if let Some(applied) = applied {
                    *program.expression_table.expression_mut(handle) = applied;
                }
            }
            Ok(())
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
mod declaration_invocations {
    use super::{
        AppliedCallableOwner, AppliedCallableSignature, CallableApplication,
        CallableLifetimeBinding, CallableLifetimeOrigin,
    };
    use crate::{
        TypedTrees,
        signature::StateSignature,
        static_applications::{
            StaticMachineApplicationHandle, StaticMachineBinding, StaticMachineBindingValue,
        },
    };
    use language_core::sealed_intrinsics::{CompilerPrimitive, SealedIntrinsic};
    use symbols::SymbolHandle;

    impl TypedTrees {
        /// A bodyless declaration publishes a contract, not an executable
        /// copy. Close that contract at its actual source application while
        /// types are still mutable; later readers borrow the retained result.
        pub fn prepare_bodyless_callable_application(
            &mut self,
            application: StaticMachineApplicationHandle,
            target: SymbolHandle,
        ) -> Result<bool, &'static str> {
            let Some((declaration, source)) = self.bodyless_declaration(target)? else {
                return Ok(false);
            };
            if !self
                .expression_table
                .static_machine_application_is_valid(application)
            {
                return Err("bodyless call has no retained source application");
            }
            let invocation = self.static_machine_application(application).clone();
            if invocation.binding_declaration != declaration
                || !self
                    .same_registered_machine_operation(invocation.registration_operation, target)
                || !self
                    .symbols
                    .symbols()
                    .nodes()
                    .is_valid(invocation.lexical_owner)
                || !invocation.input_selections.is_empty()
                || !invocation.selected_machine_arguments.is_empty()
                || invocation.conformance_selection.is_some()
                || invocation.callable_contract_source.is_valid()
            {
                return Err("bodyless call lost its exact declaration or supplying application");
            }
            if self
                .bodyless_callable_signature(application, target)?
                .is_some()
            {
                // The lookup authenticates the complete current occurrence.
                // An unchanged read must not mint new type or region rows.
                return Ok(true);
            }
            let parameters = self.state_signature_type_parameters(&source).to_vec();
            if parameters.iter().any(|parameter| {
                parameter.kind != crate::data::TypeParameterKind::Type
                    || parameter.bounds != crate::data::DataProperties::default()
            }) || !source.conformance_bounds.is_empty()
            {
                return Err(
                    "bodyless call needs its static value, evidence or eligibility judgment",
                );
            }
            let arguments = self
                .static_machine_application_arguments(application)
                .to_vec();
            if arguments.len() != parameters.len() || invocation.arguments.len() != arguments.len()
            {
                return Err("bodyless call needs exactly one argument per declared type parameter");
            }
            let mut bindings = Vec::with_capacity(parameters.len());
            for (parameter, argument) in parameters.iter().zip(&arguments) {
                let actual = self
                    .static_operand_type_reference_in_scope(argument, invocation.lexical_owner)?;
                if !self.type_reference_table.contains_type_reference(actual) {
                    return Err("bodyless call type argument has no live retained occurrence");
                }
                bindings.push(StaticMachineBinding {
                    parameter: parameter.symbol,
                    value: StaticMachineBindingValue::Type(actual),
                });
            }
            let types = bindings
                .iter()
                .map(|binding| match binding.value {
                    StaticMachineBindingValue::Type(actual) => actual,
                    _ => unreachable!(),
                })
                .collect::<Vec<_>>();
            self.require_declared_application_operands(&types, invocation.lexical_owner)?;
            if self
                .require_owned_application_operands(&types, invocation.lexical_owner)
                .is_err()
            {
                // A declared outer binder is still open. Its generated
                // callers prepare their own exact closed applications.
                return Ok(false);
            }
            let argument_identities = arguments
                .iter()
                .map(|argument| {
                    self.declared_static_operand_identity(
                        crate::type_identity::DeclaredStaticOperandIdentityRequest {
                            operand: crate::type_identity::IndexIdentityArgument::StaticArgument(
                                argument,
                            ),
                            lexical_owner: invocation.lexical_owner,
                            binders: &[],
                            lifetime_binders: &[],
                        },
                    )
                    .ok_or("bodyless call argument has no complete declared identity")
                })
                .collect::<Result<Vec<_>, &'static str>>()?;
            self.expression_table
                .set_static_machine_application_bindings(application, &bindings);
            let lifetime_rows = self
                .static_application_lifetimes_for_declaration(application, declaration)?
                .to_vec();
            let lifetimes = lifetime_rows
                .iter()
                .map(|binding| {
                    let region = crate::type_identity::lifetime_parameter_identity(
                        self,
                        invocation.lexical_owner,
                        binding.argument.as_str(),
                    )
                    .ok_or("bodyless call lifetime has no exact supplying declaration")?;
                    Ok(CallableLifetimeBinding {
                        parameter: binding.parameter.clone(),
                        argument: binding.argument.clone(),
                        region,
                        origin: CallableLifetimeOrigin::Lexical,
                    })
                })
                .collect::<Result<Vec<_>, &'static str>>()?;
            let mut existing = self
                .applied_callable_signatures
                .declaration_rows(application, invocation.lexical_owner);
            let previous = existing.next().map(|(handle, row)| (handle, row.clone()));
            if existing.next().is_some() {
                return Err("bodyless call has more than one prepared declaration contract");
            }
            drop(existing);
            if let Some((_, previous)) = &previous
                && self.bodyless_application_matches(previous, declaration, &source, application)
            {
                return Ok(true);
            }
            let mut declared = source.clone();
            // All original type binders were supplied above, including
            // unused ones; no generic eligibility premise was discarded.
            declared.type_parameters = Default::default();
            let signature = CallableApplication {
                region_scope: declaration,
                lifetime_declaration: source.symbol,
                lifetimes,
                types: parameters
                    .iter()
                    .zip(types)
                    .map(|(parameter, actual)| (parameter.symbol, actual))
                    .collect(),
                static_argument_owner: invocation.lexical_owner,
                ..Default::default()
            }
            .signature(self, &declared)?;
            let snapshot = self
                .expression_table
                .snapshot_static_machine_bindings(application);
            let lifetimes = self
                .callable_lifetime_substitutions
                .insert_many(lifetime_rows);
            let arguments = self
                .callable_argument_identities
                .insert_many(argument_identities);
            let prepared = AppliedCallableSignature {
                owner: AppliedCallableOwner::RequirementInvocation {
                    declaration,
                    application,
                    lexical_owner: invocation.lexical_owner,
                    source,
                    bindings: snapshot,
                    lifetimes,
                    arguments,
                },
                signature,
            };
            if let Some((previous, _)) = previous {
                self.applied_callable_signatures
                    .replace(previous, prepared)?;
            } else {
                self.applied_callable_signatures.insert(prepared);
            }
            Ok(true)
        }

        /// A selected body and its public requirement have distinct binders.
        /// Close the public header from the exact retained body application,
        /// then keep that occurrence beside the ordinary closed requirement.
        pub fn prepare_selected_requirement_application(
            &mut self,
            application: StaticMachineApplicationHandle,
            dispatch: &crate::StaticRequirementDispatch,
            original: SymbolHandle,
        ) -> Result<(), &'static str> {
            use crate::static_applications::{StaticMachineBinding, StaticMachineBindingValue};
            if !self
                .expression_table
                .static_machine_application_is_valid(application)
                || !self.symbols.symbols().nodes().is_valid(original)
                || !self
                    .symbols
                    .symbols()
                    .nodes()
                    .is_valid(dispatch.evidence_parameter)
            {
                return Err("selected public application has stale source coordinates");
            }
            let invocation = self.static_machine_application(application).clone();
            let receipt = self
                .machine_instantiation(invocation.lexical_owner)?
                .ok_or("selected public application lost its supplying instance")?;
            let bound = self
                .evidence_bound_requirement(
                    receipt.template(),
                    dispatch.evidence_parameter,
                    original,
                )
                .map_err(|_| "selected public application lost its exact bound")?
                .ok_or("selected public application has no requirement bound")?;
            if bound.signature.symbol != dispatch.requirement
                || bound.application.declaration.symbol != dispatch.declaring_trait
            {
                return Err("selected public application changed its requirement");
            }
            let source = self
                .applied_callable_signature(&bound.application, bound.signature.symbol)
                .or_else(|| {
                    bound
                        .application
                        .keeps_declared_signatures(self)
                        .then_some(bound.signature)
                })
                .ok_or("selected public application lost its applied source header")?
                .clone();
            let environment = bound.application.environment();
            let required = self.state_signature_type_parameters(&source).to_vec();
            if required.is_empty() {
                return Ok(());
            }
            let (_, declaration) = self
                .trait_machine_signature(source.symbol)
                .ok_or("selected public application lost its authored requirement")?;
            let authored = self.state_signature_type_parameters(declaration).to_vec();
            let provider = self
                .machine_by_symbol(invocation.binding_declaration)
                .ok_or("selected public application lost its binding declaration")?;
            if provider.symbol != dispatch.realization_machine {
                return Err("selected public application binds another realization");
            }
            let supplied = self.machine_callable_parameters(provider);
            let mut bindings = self
                .static_machine_application_bindings(application)
                .to_vec();
            if required.len() != authored.len()
                || required.len() != supplied.len()
                || supplied.len() != bindings.len()
                || !source.lifetime_parameters.is_empty()
                || !invocation.lifetime_bindings.is_empty()
            {
                return Err("selected public application needs its complete invocation telescope");
            }
            bindings.sort_unstable_by_key(|binding| {
                (
                    binding.parameter.arena_index(),
                    binding.parameter.generation(),
                )
            });
            if bindings
                .windows(2)
                .any(|pair| pair[0].parameter == pair[1].parameter)
            {
                return Err("selected realization repeats an invocation binder");
            }
            let mut public_bindings = Vec::with_capacity(required.len());
            let mut public_arguments = Vec::with_capacity(required.len());
            for ((required, authored), supplied) in required.iter().zip(authored).zip(supplied) {
                let crate::data::TypeParameterKind::Const {
                    type_reference: expected,
                } = required.kind
                else {
                    return Err("selected public application has an unserved binder kind");
                };
                let crate::data::TypeParameterKind::Const {
                    type_reference: actual,
                } = supplied.kind
                else {
                    return Err("selected realization has another binder kind");
                };
                let primitive = self
                    .primitive_type_reference(expected)
                    .filter(|expected| self.primitive_type_reference(actual) == Some(*expected))
                    .ok_or("selected realization value binder has another carrier")?;
                let Ok(position) = bindings.binary_search_by_key(
                    &(supplied.symbol.arena_index(), supplied.symbol.generation()),
                    |binding| {
                        (
                            binding.parameter.arena_index(),
                            binding.parameter.generation(),
                        )
                    },
                ) else {
                    return Err("selected realization binder has no unique argument");
                };
                let binding = &bindings[position];
                let StaticMachineBindingValue::Const(reference) = binding.value else {
                    return Err("selected realization binder has no static value");
                };
                let argument = self.static_operand_from_type_reference(reference)?;
                self.prepare_callable_scalar_argument(primitive, &argument)?;
                public_bindings.push(StaticMachineBinding {
                    parameter: authored.symbol,
                    value: StaticMachineBindingValue::Const(reference),
                });
                public_arguments.push(argument);
            }
            let mut existing = self
                .applied_callable_signatures
                .declaration_rows(application, invocation.lexical_owner);
            let previous = existing.next().map(|(handle, _)| handle);
            if existing.next().is_some() {
                return Err("selected call has several prepared public applications");
            }
            drop(existing);
            let arguments = self
                .static_machine_application_arguments(application)
                .iter()
                .map(|argument| {
                    self.declared_static_operand_identity(
                        crate::type_identity::DeclaredStaticOperandIdentityRequest {
                            operand: crate::type_identity::IndexIdentityArgument::StaticArgument(
                                argument,
                            ),
                            lexical_owner: invocation.lexical_owner,
                            binders: &[],
                            lifetime_binders: &[],
                        },
                    )
                    .ok_or("selected call argument has no exact declared identity")
                })
                .collect::<Result<Vec<_>, _>>()?;
            let public = self.expression_table.retain_static_machine_application(
                source.symbol,
                source.name.clone(),
                &public_arguments,
            );
            let row = self.expression_table.static_machine_application_mut(public);
            row.lexical_owner = invocation.lexical_owner;
            row.binding_declaration = source.symbol;
            self.expression_table
                .set_static_machine_application_bindings(public, &public_bindings);
            let trait_arguments = self.materialize_trait_application_arguments(&environment)?;
            let closed = self.instantiate_requirement_application(
                &environment,
                source.symbol,
                &trait_arguments,
                public,
            )?;
            let signature = self
                .closed_requirement_application(closed)
                .ok_or("selected public application lost its closed requirement")?
                .signature
                .clone();
            let bindings = self
                .expression_table
                .snapshot_static_machine_bindings(application);
            let arguments = self.callable_argument_identities.insert_many(arguments);
            let prepared = AppliedCallableSignature {
                owner: AppliedCallableOwner::RequirementInvocation {
                    declaration: source.symbol,
                    application,
                    lexical_owner: invocation.lexical_owner,
                    source,
                    bindings,
                    lifetimes: Default::default(),
                    arguments,
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

        pub(crate) fn selected_requirement_invocation_signature(
            &self,
            owner: SymbolHandle,
            application: StaticMachineApplicationHandle,
            source: &StateSignature,
        ) -> Result<Option<&StateSignature>, &'static str> {
            if !application.is_valid() {
                return Ok(None);
            }
            let mut rows = self
                .applied_callable_signatures
                .declaration_rows(application, owner);
            let Some((_, prepared)) = rows.next() else {
                return Ok(None);
            };
            if rows.next().is_some()
                || !self.bodyless_application_matches(prepared, source.symbol, source, application)
            {
                return Err("selected public application changed after preparation");
            }
            Ok(Some(&prepared.signature))
        }

        /// Compiler metadata identifies the declaration; the same retained
        /// bodyless occurrence still authenticates its complete application.
        /// No unprepared template or another compiler-provided family serves
        /// as a substitute for this source call's closed contract.
        pub fn compiler_primitive_callable_signature(
            &self,
            application: StaticMachineApplicationHandle,
            target: SymbolHandle,
        ) -> Result<Option<(CompilerPrimitive, &StateSignature)>, &'static str> {
            if !self.symbols.symbols().nodes().is_valid(target) {
                return Err("compiler primitive target has no live declaration");
            }
            let Some(machine) = self
                .machine_by_symbol(target)
                .or_else(|| self.machine_holding_state(target))
            else {
                return Ok(None);
            };
            let Some(operation) = self.run_custody_primitive(machine)? else {
                return Ok(None);
            };
            Ok(self
                .bodyless_callable_signature(application, target)?
                .map(|signature| (operation, signature)))
        }

        fn run_custody_primitive(
            &self,
            machine: &crate::machine::Machine,
        ) -> Result<Option<CompilerPrimitive>, &'static str> {
            if !matches!(
                self.symbols
                    .sealed_declaration(machine.symbol, machine.name.as_str()),
                Some(SealedIntrinsic::CompilerPrimitive(
                    CompilerPrimitive::RunCustody(_)
                ))
            ) {
                return Ok(None);
            }
            match self.compiler_primitive_schema_checked(machine.symbol)? {
                Some((operation @ CompilerPrimitive::RunCustody(_), _)) => Ok(Some(operation)),
                _ => Err("run custody changed its exact compiler primitive declaration"),
            }
        }

        /// Look up this occurrence, not another call with equal rendered
        /// arguments. Mutated source bindings cannot reuse an earlier header.
        pub fn bodyless_callable_signature(
            &self,
            application: StaticMachineApplicationHandle,
            target: SymbolHandle,
        ) -> Result<Option<&StateSignature>, &'static str> {
            let Some((declaration, source)) = self.bodyless_declaration(target)? else {
                return Ok(None);
            };
            if !self
                .expression_table
                .static_machine_application_is_valid(application)
            {
                return Err("bodyless call has no live source application");
            }
            let invocation = self.static_machine_application(application);
            if invocation.binding_declaration != declaration
                || !self
                    .same_registered_machine_operation(invocation.registration_operation, target)
                || !self
                    .symbols
                    .symbols()
                    .nodes()
                    .is_valid(invocation.lexical_owner)
                || !invocation.input_selections.is_empty()
                || !invocation.selected_machine_arguments.is_empty()
                || invocation.conformance_selection.is_some()
                || invocation.callable_contract_source.is_valid()
            {
                return Err("bodyless call does not belong to its prepared declaration");
            }
            let mut rows = self
                .applied_callable_signatures
                .declaration_rows(application, invocation.lexical_owner);
            let Some((_, prepared)) = rows.next() else {
                return Ok(None);
            };
            if rows.next().is_some() {
                return Err("bodyless call has ambiguous prepared declaration contracts");
            }
            if !self.bodyless_application_matches(prepared, declaration, &source, application) {
                return Err("bodyless call source or bindings changed after contract preparation");
            }
            Ok(Some(&prepared.signature))
        }

        fn bodyless_application_matches(
            &self,
            prepared: &AppliedCallableSignature,
            actual_declaration: SymbolHandle,
            actual_source: &StateSignature,
            actual_application: StaticMachineApplicationHandle,
        ) -> bool {
            let AppliedCallableOwner::RequirementInvocation {
                declaration,
                application,
                lexical_owner,
                source,
                bindings,
                lifetimes,
                arguments,
            } = &prepared.owner
            else {
                return false;
            };
            let invocation = self.static_machine_application(actual_application);
            let actual_arguments = self.static_machine_application_arguments(actual_application);
            let actual_bindings = self.static_machine_application_bindings(actual_application);
            let Some(known_arguments) = self.callable_argument_identities.span(*arguments) else {
                return false;
            };
            let Some(known_lifetimes) = self.callable_lifetime_substitutions.span(*lifetimes)
            else {
                return false;
            };
            *declaration == actual_declaration
                && *application == actual_application
                && self.symbols.symbols().nodes().is_valid(*lexical_owner)
                && *lexical_owner == invocation.lexical_owner
                && source == actual_source
                && (prepared.signature.symbol == source.symbol
                    || self
                        .closed_requirement_application(prepared.signature.symbol)
                        .is_some_and(|closed| closed.template == source.symbol))
                && bindings.len() == actual_bindings.len()
                && invocation.bindings.len() == actual_bindings.len()
                && source.type_parameters.len() == actual_bindings.len()
                && self
                    .expression_table
                    .static_machine_binding_snapshot(*bindings)
                    == actual_bindings
                && invocation.lifetime_bindings.len() == known_lifetimes.len()
                && known_lifetimes == self.static_machine_application_lifetimes(actual_application)
                && invocation.arguments.len() == actual_arguments.len()
                && known_arguments.len() == actual_arguments.len()
                && known_arguments
                    .iter()
                    .zip(actual_arguments)
                    .all(|(known, actual)| {
                        self.declared_static_operand_identity(
                            crate::type_identity::DeclaredStaticOperandIdentityRequest {
                                operand:
                                    crate::type_identity::IndexIdentityArgument::StaticArgument(
                                        actual,
                                    ),
                                lexical_owner: *lexical_owner,
                                binders: &[],
                                lifetime_binders: &[],
                            },
                        ) == Some(*known)
                    })
        }

        pub fn bodyless_declaration(
            &self,
            target: SymbolHandle,
        ) -> Result<Option<(SymbolHandle, StateSignature)>, &'static str> {
            if !self.symbols.symbols().nodes().is_valid(target) {
                return Err("bodyless call target has no live declaration");
            }
            let Some(machine) = self
                .machine_by_symbol(target)
                .or_else(|| self.machine_holding_state(target))
            else {
                return Ok(None);
            };
            // Only this exact sealed compiler family joins the existing
            // preparation path. Other compiler-provided declarations do not
            // acquire bodyless-call authority from their supply mode alone.
            if machine.supply_mode != language_semantics::MachineSupplyMode::Boundary
                && self.run_custody_primitive(machine)?.is_none()
            {
                return Ok(None);
            }
            if !machine.has_bodyless_signature() {
                return Ok(None);
            }
            let states = self
                .machine_states
                .span(machine.states)
                .ok_or("bodyless declaration has a stale state roster")?;
            let [state] = states else {
                return Err("bodyless declaration needs one exact entry state");
            };
            if (target != machine.symbol && target != state.symbol)
                || !state.statement_nodes.is_empty()
                || !state.contracts.is_empty()
                || machine.structural_type_equations_pending
                || !machine.declared_type_equations.is_empty()
            {
                return Err(
                    "bodyless declaration needs its complete entry and structural contract",
                );
            }
            if self
                .data_type_parameters
                .span(machine.type_parameters)
                .is_none()
                || self.state_parameters.span(state.parameters).is_none()
                || self.signature_contracts.span(machine.contracts).is_none()
            {
                return Err("bodyless declaration has a stale parameter or contract roster");
            }
            Ok(Some((
                machine.symbol,
                StateSignature {
                    // The machine authored the contracts and effect rows;
                    // its generated entry supplies only the runtime telescope.
                    // Keeping the entry as owner loses the machine's written
                    // reach (and mis-scopes reserved result/region bindings).
                    symbol: machine.symbol,
                    name: machine.name.clone(),
                    spelling: machine.spelling,
                    lifetime_parameters: machine.lifetime_parameters.clone(),
                    type_parameters: machine.type_parameters,
                    conformance_bounds: machine.conformance_bounds.clone(),
                    parameters: state.parameters,
                    return_type: state.return_type,
                    invokes: machine.invokes,
                    service_reach_row: machine.service_reach_row,
                    service_reach_is_installation_bound: machine
                        .service_reach_is_installation_bound,
                    suspends_keyword_source_spans: machine.suspends_keyword_source_spans.clone(),
                    blocks_keyword_source_spans: machine.blocks_keyword_source_spans.clone(),
                    suspends: machine.suspends,
                    blocks: machine.blocks,
                    contracts: machine.contracts,
                    termination_guarantee: match &machine.termination_plan.interface {
                        language_semantics::TerminationInterface::Published(guarantee) => {
                            guarantee.clone()
                        }
                        _ => language_semantics::TerminationGuarantee::NoGuarantee,
                    },
                    declares_no_crash_route: machine.declares_no_crash_route,
                    ..Default::default()
                },
            )))
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::{
            data::{TypeParameter, TypeParameterKind},
            machine::Machine,
            name::Identifier,
            signature::StateParameter,
            state::State,
            types::TypeReferenceNode,
        };
        use symbols::{BuiltinTypeAtom, SymbolKind, SymbolNameRef, SymbolTableBuilder};

        #[test]
        fn bodyless_occurrences_keep_both_closed_type_arguments_and_source_bindings() {
            let mut symbols = SymbolTableBuilder::default();
            let root = symbols.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
            let roots = symbols.insert_children(
                root,
                symbols::builtin_type_symbols().into_iter().chain([
                    (SymbolKind::Machine, SymbolNameRef::Static("transfer")),
                    (SymbolKind::Machine, SymbolNameRef::Static("caller")),
                ]),
            );
            let target = roots.handle_at(BuiltinTypeAtom::ALL.len() as u32);
            let caller = roots.handle_at(BuiltinTypeAtom::ALL.len() as u32 + 1);
            let children = symbols.insert_children(
                target,
                [
                    (SymbolKind::TypeParameter, SymbolNameRef::Static("P")),
                    (SymbolKind::TypeParameter, SymbolNameRef::Static("T")),
                    (SymbolKind::State, SymbolNameRef::Static("entry")),
                ],
            );
            let mut program = TypedTrees {
                symbols: symbols.finish(),
                ..Default::default()
            };
            let mut named = |symbol, name| {
                program
                    .type_reference_table
                    .insert(TypeReferenceNode::Named {
                        symbol,
                        name: Identifier::generated(name),
                    })
            };
            let p = named(children.handle_at(0), "P");
            let t = named(children.handle_at(1), "T");
            let u32_type = named(
                roots.handle_at(
                    BuiltinTypeAtom::Integer(numerics::integer_carrier::FixedInteger::unsigned(32))
                        .ordinal() as u32,
                ),
                "u32",
            );
            let u64_type = named(
                roots.handle_at(BuiltinTypeAtom::U64.ordinal() as u32),
                "u64",
            );
            let type_parameters = program.data_type_parameters.insert_many([
                TypeParameter {
                    symbol: children.handle_at(0),
                    name: Identifier::generated("P"),
                    kind: TypeParameterKind::Type,
                    ..Default::default()
                },
                TypeParameter {
                    symbol: children.handle_at(1),
                    name: Identifier::generated("T"),
                    kind: TypeParameterKind::Type,
                    ..Default::default()
                },
            ]);
            let parameters = program.state_parameters.insert_many([
                StateParameter {
                    type_reference: p,
                    ..Default::default()
                },
                StateParameter {
                    type_reference: t,
                    ..Default::default()
                },
            ]);
            let states = program.machine_states.insert_many([State {
                symbol: children.handle_at(2),
                name: Identifier::generated("entry"),
                parameters,
                return_type: t,
                ..Default::default()
            }]);
            program.push_machine(Machine {
                symbol: target,
                name: Identifier::generated("transfer"),
                type_parameters,
                states,
                supply_mode: language_semantics::MachineSupplyMode::Boundary,
                ..Default::default()
            });
            let application = |program: &mut TypedTrees, types: [_; 2]| {
                let arguments =
                    types.map(|ty| program.static_operand_from_type_reference(ty).unwrap());
                let application = program.expression_table.retain_static_machine_application(
                    target,
                    Identifier::generated("transfer"),
                    &arguments,
                );
                let occurrence = program
                    .expression_table
                    .static_machine_application_mut(application);
                occurrence.lexical_owner = caller;
                occurrence.binding_declaration = target;
                application
            };
            let first = application(&mut program, [u32_type, u64_type]);
            let second = application(&mut program, [u64_type, u32_type]);
            let unprepared = application(&mut program, [u32_type, u64_type]);
            assert!(
                program
                    .prepare_bodyless_callable_application(first, target)
                    .unwrap()
            );
            assert!(
                program
                    .prepare_bodyless_callable_application(second, target)
                    .unwrap()
            );
            for (application, expected) in [
                (first, [u32_type, u64_type]),
                (second, [u64_type, u32_type]),
            ] {
                let signature = program
                    .bodyless_callable_signature(application, target)
                    .unwrap()
                    .unwrap();
                assert!(signature.type_parameters.is_empty());
                assert_eq!(signature.parameters.len(), 2);
                for (formal, expected) in program
                    .state_parameters
                    .span(signature.parameters)
                    .unwrap()
                    .iter()
                    .zip(expected)
                {
                    assert_eq!(
                        program
                            .type_reference_table
                            .type_reference(formal.type_reference),
                        program.type_reference_table.type_reference(expected)
                    );
                }
                assert_eq!(
                    program
                        .type_reference_table
                        .type_reference(signature.return_type),
                    program.type_reference_table.type_reference(expected[1])
                );
            }
            assert!(
                program
                    .bodyless_callable_signature(unprepared, target)
                    .unwrap()
                    .is_none()
            );
            let replacement = [u64_type, u64_type]
                .map(|ty| program.static_operand_from_type_reference(ty).unwrap());
            program
                .expression_table
                .set_static_machine_application_arguments(first, &replacement);
            assert_eq!(
                program
                    .bodyless_callable_signature(first, target)
                    .unwrap_err(),
                "bodyless call source or bindings changed after contract preparation"
            );
            let duplicate = program
                .applied_callable_signatures
                .declaration_rows(second, caller)
                .next()
                .unwrap()
                .1
                .clone();
            program.applied_callable_signatures.insert(duplicate);
            assert_eq!(
                program
                    .bodyless_callable_signature(second, target)
                    .unwrap_err(),
                "bodyless call has ambiguous prepared declaration contracts"
            );
            // Supply mode by itself cannot promote an ordinary declaration,
            // even one with a previously prepared complete application.
            program.machine_mut(0).supply_mode =
                language_semantics::MachineSupplyMode::CompilerProvided;
            assert!(
                program
                    .compiler_primitive_callable_signature(second, target)
                    .unwrap()
                    .is_none()
            );
            assert!(
                !program
                    .prepare_bodyless_callable_application(second, target)
                    .unwrap()
            );
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
            kind: &data::TypeParameterKind,
            binding: &static_applications::StaticMachineBindingValue,
            lexical_owner: SymbolHandle,
        ) -> Option<type_identity::NormalizedTypeIdentity> {
            // A selected machine and its entry state invoke the same callable.
            // Keep its complete checked application rather than its type spelling.
            if matches!(
                binding,
                static_applications::StaticMachineBindingValue::Machine(_)
            ) && matches!(kind, data::TypeParameterKind::Machine { contract }
                if !matches!(contract, data::MachineParameterContract::RequirementIdentity))
            {
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
        pub invocation_contract_parameters: arena::RootRows<'program, data::TypeParameter>,
        pub application: static_applications::StaticMachineApplicationHandle,
        pub signature: &'program signature::StateSignature,
    }

    impl TypedTrees {
        pub fn requirement_invocation_contract_parameters<'program>(
            &'program self,
            signature: &'program signature::StateSignature,
        ) -> arena::RootRows<'program, data::TypeParameter> {
            self.closed_requirement_application(signature.symbol)
                .map(|application| application.invocation_contract_parameters)
                .unwrap_or_else(|| self.state_signature_type_parameters(signature))
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
                    let parameter = self
                        .data_type_parameter_by_symbol(binding.parameter)
                        .ok_or("closed requirement lost its declared local parameter")?;
                    argument
                        .portable_identity(self, &parameter.kind, &binding.value, lexical_owner)
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
                            let left_parameter = left.parameter;
                            let left_value = &left.value;
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
                                    static_applications::StaticMachineBindingValue::Machine(_),
                                    static_applications::StaticMachineBindingValue::Machine(right),
                                ) => self
                                    .data_type_parameter_by_symbol(left_parameter)
                                    .is_some_and(|parameter| {
                                        self.static_binding_matches_argument(
                                            &parameter.kind,
                                            left_value,
                                            right,
                                            lexical_owner,
                                            self.static_machine_application(existing.application)
                                                .lexical_owner,
                                        )
                                    }),
                                (
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
            let callable_kind = data::TypeParameterKind::Machine {
                contract: data::MachineParameterContract::Structural(signature::StateSignature {
                    return_type: unit,
                    ..Default::default()
                }),
            };
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
                    &callable_kind,
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
                    &callable_kind,
                    &machine_binding,
                    selected,
                )
            };
            assert!(identity(first).is_some());
            assert_ne!(identity(first), identity(second));
            assert_ne!(identity(first), identities[0]);
            let ordinary = static_applications::StaticMachineBindingValue::Type(references[0]);
            assert_eq!(
                RequirementLocalArgument::Static(references[0]).portable_identity(
                    &program,
                    &data::TypeParameterKind::Type,
                    &ordinary,
                    selected
                ),
                type_identity::portable_applied_type_identity(
                    &program,
                    references[0],
                    selected,
                    &[]
                )
            );

            // Intern the same real requirement twice through the two checked
            // selections. Equality of printed keys alone would miss duplicate
            // generated declarations that later lowering must refuse.
            let receiver =
                program
                    .symbols
                    .insert_generated_root_from(selected, SymbolKind::Trait, "Receiver");
            let template = program
                .symbols
                .insert_generated_children(receiver, [(SymbolKind::State, "invoke")])
                .start();
            let binder = program
                .symbols
                .insert_generated_children(template, [(SymbolKind::MachineParameter, "Callback")])
                .start();
            let parameters = program
                .data_type_parameters
                .insert_many([data::TypeParameter {
                    symbol: binder,
                    name: Identifier::generated("Callback"),
                    kind: data::TypeParameterKind::Machine {
                        contract: data::MachineParameterContract::Structural(
                            signature::StateSignature {
                                return_type: unit,
                                ..Default::default()
                            },
                        ),
                    },
                    ..Default::default()
                }]);
            let mut declaration = crate::trait_definition::TraitDefinition {
                symbol: receiver,
                name: Identifier::generated("Receiver"),
                is_boundary: true,
                ..Default::default()
            };
            program.push_trait_machine_signature(
                &mut declaration,
                signature::StateSignature {
                    symbol: template,
                    name: Identifier::generated("invoke"),
                    type_parameters: parameters,
                    return_type: unit,
                    ..Default::default()
                },
            );
            let environment =
                crate::trait_definition::AppliedTraitScope::abstract_scope(&declaration)
                    .environment();
            program.push_trait_definition(declaration);
            let mut close = |symbol| {
                let actual = expression::StaticMachineArgument {
                    symbol,
                    path: Box::from([Identifier::from(program.symbols.name(symbol))]),
                    ..Default::default()
                };
                let application = program.expression_table.retain_static_machine_application(
                    template,
                    Identifier::generated("invoke"),
                    std::slice::from_ref(&actual),
                );
                let row = program
                    .expression_table
                    .static_machine_application_mut(application);
                row.binding_declaration = template;
                row.lexical_owner = selected;
                program
                    .expression_table
                    .set_static_machine_application_bindings(
                        application,
                        &[static_applications::StaticMachineBinding {
                            parameter: binder,
                            value: static_applications::StaticMachineBindingValue::Machine(actual),
                        }],
                    );
                program
                    .expression_table
                    .set_static_machine_application_lifetimes(application, template, &[]);
                program
                    .instantiate_requirement_application(&environment, template, &[], application)
                    .unwrap()
            };
            let machine_instance = close(selected);
            assert_eq!(
                machine_instance,
                close(states[0]),
                "the entry spelling must reuse the SAME checked instance"
            );
            assert_ne!(machine_instance, close(states[1]));
            assert_ne!(machine_instance, close(other));
            let closed = program
                .closed_requirement_application(machine_instance)
                .unwrap();
            let binding = &program.static_machine_application_bindings(closed.application)[0];
            let (_, signature) = program.trait_machine_signature(template).unwrap();
            let parameter = &program.state_signature_type_parameters(signature)[0];
            for (symbol, expected) in [
                (selected, true),
                (states[0], true),
                (states[1], false),
                (other, false),
                (missing, false),
            ] {
                let actual = expression::StaticMachineArgument {
                    symbol,
                    path: Box::from([Identifier::from(program.symbols.name(symbol))]),
                    ..Default::default()
                };
                assert_eq!(
                    program.static_binding_matches_argument(
                        &parameter.kind,
                        &binding.value,
                        &actual,
                        selected,
                        selected
                    ),
                    expected,
                    "the source call must rejoin exactly its checked machine selection"
                );
            }
            let metadata_kind = data::TypeParameterKind::Machine {
                contract: data::MachineParameterContract::RequirementIdentity,
            };
            let metadata_argument = expression::StaticMachineArgument {
                symbol: template,
                path: Box::from([Identifier::generated("invoke")]),
                ..Default::default()
            };
            let metadata_binding =
                static_applications::StaticMachineBindingValue::Machine(metadata_argument.clone());
            assert!(
                program.static_binding_matches_argument(
                    &metadata_kind,
                    &metadata_binding,
                    &metadata_argument,
                    selected,
                    selected
                ),
                "declaration identity does not require an executable body"
            );
            assert!(
                !program.static_binding_matches_argument(
                    &callable_kind,
                    &metadata_binding,
                    &metadata_argument,
                    selected,
                    selected
                ),
                "a declaration-only requirement is not a concrete callable value"
            );
            let metadata_reference = program
                .static_operand_type_reference_in_scope(&metadata_argument, selected)
                .unwrap();
            assert_eq!(
                RequirementLocalArgument::Static(metadata_reference)
                    .portable_identity(&program, &metadata_kind, &metadata_binding, selected)
                    .expect(
                        "metadata-only requirements retain their complete declaration identity"
                    ),
                type_identity::portable_applied_type_identity(
                    &program,
                    metadata_reference,
                    selected,
                    &[]
                )
                .unwrap()
            );
        }
    }
}
pub(crate) mod selected_operands {
    use super::CallableApplication;
    use crate::{TypedTrees, expression, types};

    impl TypedTrees {
        /// Resolve the retained declaration operand before a description uses
        /// it. Bare generic heads share the parser's type-expression form;
        /// their resolved declaration and complete arguments decide the role.
        pub fn declaration_operand_from_expression(
            &self,
            expression: expression::ExpressionHandle,
        ) -> Result<expression::StaticMachineArgument, &'static str> {
            use expression::{ExpressionNode, StaticMachineArgument};
            if !self.expression_table.expression_is_valid(expression) {
                return Err("declaration operand lost its retained expression");
            }
            match self.expression_table.expression(expression) {
                ExpressionNode::Name(path) if path.static_application.is_valid() => self
                    .expression_table
                    .static_member(path.static_application)
                    .cloned()
                    .ok_or("declaration operand lost its selected application"),
                ExpressionNode::Name(path) => Ok(StaticMachineArgument {
                    symbol: path.symbol,
                    ..StaticMachineArgument::default()
                }),
                ExpressionNode::TypeExpression(reference) => {
                    let mut selected = self.static_operand_from_type_reference(*reference)?;
                    if !self.symbols.symbols().nodes().is_valid(selected.symbol)
                        || !matches!(
                            self.symbols.get(selected.symbol).kind,
                            symbols::SymbolKind::Machine | symbols::SymbolKind::State
                        )
                    {
                        return Err(
                            "an applied declaration operand requires a machine or requirement",
                        );
                    }
                    // Keep the original supplying scope so the existing region
                    // owner can authenticate this exact selected application.
                    selected.type_reference = *reference;
                    Ok(selected)
                }
                _ => Err("a declaration operand requires an uninvoked path, not a value or call"),
            }
        }

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
                TypeReferenceNode::Generic { base_symbol, .. }
                    if !self.symbols.symbols().nodes().is_valid(*base_symbol) =>
                {
                    Err("static operand lost its selected declaration")
                }
                TypeReferenceNode::Generic {
                    base_symbol,
                    base_name,
                    lifetime_arguments,
                    arguments,
                } if self.symbols.get(*base_symbol).kind != symbols::SymbolKind::Data => {
                    let arguments = self
                        .type_reference_handles(*arguments)
                        .iter()
                        .map(|argument| -> Result<StaticMachineArgument, &'static str> {
                            let mut operand = self.static_operand_from_type_reference(*argument)?;
                            // The application still owns this exact payload;
                            // conversion must not discard a child reference.
                            operand.type_reference = *argument;
                            Ok(operand)
                        })
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
                    match self.expression_table.expression(*value) {
                        expression::ExpressionNode::Integer(literal) => Some(literal.clone()),
                        // A calculation over literals is the literal of its
                        // value (`const_integer::closed_literal`).
                        _ => crate::const_integer::closed_literal(&self.expression_table, *value),
                    }
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
                        unresolved @ (signature::AuthoredInvocationTarget::Unresolved
                        | signature::AuthoredInvocationTarget::Undeclared) => unresolved,
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
                        membership.semantic_domain = self.domain_identity(
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

        pub(crate) fn declaration_rows(
            &self,
            invocation: StaticMachineApplicationHandle,
            owner: SymbolHandle,
        ) -> impl Iterator<Item = (CallableRow, &AppliedCallableSignature)> {
            self.invocation_rows(invocation, InvocationPurpose::Requirement(owner))
        }

        pub(crate) fn machine_rows(
            &self,
            invocation: StaticMachineApplicationHandle,
        ) -> impl Iterator<Item = (CallableRow, &AppliedCallableSignature)> {
            self.invocation_rows(invocation, InvocationPurpose::MachineSelection)
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
        use crate::signature::StateSignature;

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
            assert_ne!(replacement.revision(), clone.revision());
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
                    source: Default::default(),
                    bindings: Default::default(),
                    lifetimes: Default::default(),
                    arguments: Default::default(),
                },
                signature: Default::default(),
            };
            let requirement_rows = |table: &AppliedCallableTable, application, lexical_owner| {
                table
                    .invocation_rows(application, InvocationPurpose::Requirement(lexical_owner))
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
        fn signature_replacement_preserves_invocation_ownership() {
            let invocation = StaticMachineApplicationHandle::from_parts(1, 1);
            let other = StaticMachineApplicationHandle::from_parts(1, 2);
            let mut table = AppliedCallableTable::default();
            let handle = table.insert(selection(invocation));
            let signature = StateSignature {
                symbol: symbols::SymbolHandle::from_arena_index(9),
                ..Default::default()
            };
            let mut replacement = table.get(handle).clone();
            replacement.signature = signature.clone();
            table.replace(handle, replacement).unwrap();
            assert_eq!(rows(&table, invocation), [handle]);
            assert!(rows(&table, other).is_empty());
            assert_eq!(table.get(handle).signature, signature);
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

    // The existing installation loop owns these transient requests for both a
    // program preparation and a newly constructed call's exact application.
    type CallableSignatureRequest = (
        SymbolHandle,
        SymbolHandle,
        crate::type_identity::NormalizedTypeIdentity,
        SymbolHandle,
        crate::trait_definition::TraitApplicationEnvironment,
        Vec<(
            crate::trait_definition::TraitRefinementClause,
            crate::trait_definition::TraitApplicationEnvironment,
        )>,
    );

    impl TypedTrees {
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
            let pending = self.applied_callable_signature_requests(roots);
            self.prepare_applied_callable_signature_requests(pending)
        }

        /// Prepare the public signatures of the exact retained application
        /// before constructing its runtime actuals. The ordinary program pass
        /// uses the same request and installation operations; this grants no
        /// conformance proof or executable body.
        pub fn prepare_applied_callable_signatures_for_application(
            &mut self,
            lexical_owner: SymbolHandle,
            application: crate::types::TypeReferenceHandle,
        ) -> Result<(), &'static str> {
            if !self
                .type_reference_table
                .contains_type_reference(application)
                || !self.symbols.symbols().nodes().is_valid(lexical_owner)
            {
                return Err("applied callable lost its source application owner");
            }
            let scope = self
                .applied_requirement_scope(lexical_owner, application)
                .ok_or("applied callable lost its complete requirement application")?;
            let pending = self.applied_callable_signature_requests(vec![scope]);
            self.prepare_applied_callable_signature_requests(pending)?;
            Ok(())
        }

        fn applied_callable_signature_requests<'program>(
            &'program self,
            mut roots: Vec<AppliedTraitScope<'program>>,
        ) -> Vec<CallableSignatureRequest> {
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
            pending
        }

        fn prepare_applied_callable_signature_requests(
            &mut self,
            pending: Vec<CallableSignatureRequest>,
        ) -> Result<(Vec<signature::StateSignature>, bool), &'static str> {
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
        source: signature::StateSignature,
        bindings: arena::HandleSpan<crate::static_applications::StaticMachineBinding>,
        lifetimes: arena::HandleSpan<StaticMachineLifetimeBinding>,
        arguments: arena::HandleSpan<crate::type_identity::NormalizedTypeIdentity>,
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
    invocation_elision: Option<(
        SymbolHandle,
        usize,
        crate::type_identity::identity_context::SignatureElision,
    )>,
    types: Vec<(SymbolHandle, types::TypeReferenceHandle)>,
    static_arguments: Vec<(SymbolHandle, crate::expression::StaticMachineArgument)>,
    static_argument_owner: SymbolHandle,
    selected_providers: Vec<(
        SymbolHandle,
        crate::static_applications::StaticMachineApplicationHandle,
    )>,
    symbols: Vec<(SymbolHandle, SymbolHandle)>,
    declaration_symbols: crate::PreparedSymbolRebinding,
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
    fn invocation_region(
        &self,
        program: &mut TypedTrees,
    ) -> Result<ScopedCallableLifetime, &'static str> {
        let (owner, position, elision) = self
            .invocation_elision
            .as_ref()
            .ok_or("invocation has no elision scope")?;
        let ordinal = elision
            .region()
            .ok_or("invocation result has ambiguous lifetime elision")?;
        if !program
            .machines()
            .get(*position)
            .is_some_and(|machine| machine.symbol == *owner)
        {
            return Err("invocation lost its unique lifetime declaration");
        }
        if program.machines().at(*position).lifetime_parameters.len() == ordinal {
            program.machine_mut(*position).lifetime_parameters.push(
                crate::name::Identifier::generated(format!("@invocation-region-{ordinal}")),
            );
        }
        let name = program
            .machines()
            .at(*position)
            .lifetime_parameters
            .get(ordinal)
            .cloned()
            .ok_or("invocation elision skipped a declaration ordinal")?;
        Ok(ScopedCallableLifetime {
            name,
            region: crate::type_identity::LifetimeParameterIdentity {
                declaration: *owner,
                ordinal,
            },
            origin: CallableLifetimeOrigin::Lexical,
        })
    }

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
            .map(|(_, replacement)| *replacement)
            .or_else(|| self.declaration_symbols.execution_symbol(original))
            .unwrap_or(original)
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

    /// Close the whole invocation signature in input order. Elided inputs
    /// introduce private caller binders once; the result uses the same policy
    /// as callable schemas, before any recursive value construction occurs.
    pub fn instantiate_invocation_signature(
        &mut self,
        parameters: &[types::TypeReferenceHandle],
        result: types::TypeReferenceHandle,
        substitutions: &[(SymbolHandle, types::TypeReferenceHandle)],
        declaration: crate::callable_signature_shapes::CallableDeclaration<'_>,
        caller: &mut crate::machine::Machine,
    ) -> Result<(Vec<types::TypeReferenceHandle>, types::TypeReferenceHandle), &'static str> {
        let declared = declaration.lifetime_parameters(self)?.to_vec();
        if declared != caller.lifetime_parameters {
            return Err("generated caller has a different lifetime telescope");
        }
        let positions = self.machine_positions_for_symbol(caller.symbol);
        let [position] = positions.as_slice() else {
            return Err("invocation lost its unique caller");
        };
        let mut application = CallableApplication {
            region_scope: declaration.symbol(),
            types: substitutions.to_vec(),
            lifetimes: declared
                .iter()
                .enumerate()
                .map(|(ordinal, name)| CallableLifetimeBinding {
                    parameter: name.clone(),
                    argument: name.clone(),
                    region: crate::type_identity::LifetimeParameterIdentity {
                        declaration: caller.symbol,
                        ordinal,
                    },
                    origin: CallableLifetimeOrigin::Lexical,
                })
                .collect(),
            invocation_elision: Some((
                caller.symbol,
                *position as usize,
                crate::type_identity::identity_context::SignatureElision::inputs(declared.len()),
            )),
            ..Default::default()
        };
        let parameters = parameters
            .iter()
            .map(|reference| application.type_reference(self, *reference))
            .collect::<Result<Vec<_>, _>>()?;
        application
            .invocation_elision
            .as_ref()
            .unwrap()
            .2
            .enter_result(declared.len(), 0);
        let result = application.type_reference(self, result)?;
        caller.lifetime_parameters = self
            .machine_by_symbol(caller.symbol)
            .ok_or("invocation lost its current caller")?
            .lifetime_parameters
            .clone();
        Ok((parameters, result))
    }

    /// A selected member's lifetime parameters belong to its enclosing data
    /// application. Apply that tuple before using the member in another scope;
    /// an omitted tuple keeps ordinary lifetime elision.
    pub fn instantiate_data_member_type(
        &mut self,
        original: types::TypeReferenceHandle,
        application: types::TypeReferenceHandle,
    ) -> Result<types::TypeReferenceHandle, &'static str> {
        let definition = self
            .closed_data_definition(application)
            .ok_or("input member lost its closed data application")?;
        let declaration = definition.symbol;
        let parameters = definition.lifetime_parameters.clone();
        if parameters.is_empty() {
            // The retained member already has its static arguments applied.
            // With no enclosing lifetimes to bind, keep that occurrence.
            return Ok(original);
        }
        let application = self
            .type_reference_table
            .unconstrained_type_reference(application);
        let supplied = match self.type_reference_node(application) {
            types::TypeReferenceNode::Generic {
                lifetime_arguments, ..
            } => lifetime_arguments.clone(),
            types::TypeReferenceNode::Named { .. } => Vec::new(),
            _ => return Err("input member has no nominal enclosing application"),
        };
        if supplied.is_empty() {
            return self.apply_callable_result_type(
                original,
                &parameters
                    .into_iter()
                    .map(|parameter| (parameter, None))
                    .collect::<Vec<_>>(),
                declaration,
            );
        }
        if supplied.len() != parameters.len() {
            return Err("input member lost its complete enclosing lifetime tuple");
        }
        let owner = self
            .type_reference_table
            .operand_lifetime_owner(application);
        let retained = self.type_reference_table.operand_lifetimes(application);
        let lifetimes = parameters
            .into_iter()
            .zip(supplied)
            .enumerate()
            .map(|(slot, (parameter, argument))| {
                let region = match retained.get(slot).copied() {
                    Some(region) if region.declaration.is_valid() => Some(region),
                    Some(_) => {
                        return Err("input member lifetime still has an unmaterialized elision");
                    }
                    None => lifetime_parameter_identity(self, owner, argument.as_str()),
                }
                .ok_or("input member lifetime lost its actual supplying declaration")?;
                Ok(CallableLifetimeBinding {
                    parameter,
                    argument,
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

    /// Copies every form of callable contract fact under the same declaration
    /// and static bindings as its entry. No fact becomes checking evidence here.
    pub fn copy_callable_contract(
        &mut self,
        original: &signature::SignatureContract,
        symbols: crate::SymbolRebinding<'_>,
        substitutions: &[(SymbolHandle, types::TypeReferenceHandle)],
        declaration: SymbolHandle,
    ) -> Result<signature::SignatureContract, &'static str> {
        let mut application = CallableApplication {
            region_scope: declaration,
            types: substitutions.to_vec(),
            symbols: symbols.iter().copied().collect(),
            ..CallableApplication::default()
        };
        let mut copied = original.clone();
        copied.facts = application.facts(self, original.facts)?;
        Ok(copied)
    }

    /// Applies one declaration's types throughout a copied entry and its clauses.
    /// Signatures, local annotations, patterns, static operands and expression
    /// metadata all use the same substitution owner; applying a domain also
    /// rebuilds its semantic identity from the substituted arguments.
    pub fn apply_copied_callable_types<'state>(
        &mut self,
        entries: impl IntoIterator<Item = (SymbolHandle, &'state mut crate::state::State, usize, usize)>,
        symbols: crate::SymbolRebinding<'_>,
        substitutions: &[(SymbolHandle, types::TypeReferenceHandle)],
    ) -> Result<(), &'static str> {
        let mut application = CallableApplication {
            types: substitutions.to_vec(),
            ..CallableApplication::default()
        };
        for &(source, target) in symbols.iter() {
            application
                .declaration_symbols
                .insert_execution(source, target);
        }
        for (declaration, state, first_expression, expression_count) in entries {
            application.region_scope = declaration;
            let rows = self
                .expression_table
                .iter_expressions_from(first_expression)
                .take(expression_count)
                .map(|(handle, _)| handle)
                .collect::<Vec<_>>();
            state.return_type = application.type_reference(self, state.return_type)?;
            for parameter in state.parameters.handles() {
                let original = self.state_parameters.get(parameter).type_reference;
                let applied = application.type_reference(self, original)?;
                self.state_parameters.get_mut(parameter).type_reference = applied;
            }
            for handle in state.statement_nodes.handles() {
                let Some(local) = self.statement_table.statement(handle).as_local_data() else {
                    continue;
                };
                let original = local.type_reference;
                let applied = application.type_reference(self, original)?;
                if let crate::statement::StatementNode::LocalData(local) =
                    self.statement_table.statement_mut(handle)
                {
                    local.type_reference = applied;
                }
            }
            let call_targets = application.expression_call_targets(self, &rows)?;
            application.apply_expression_rows(self, &rows, call_targets)?;
        }
        Ok(())
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

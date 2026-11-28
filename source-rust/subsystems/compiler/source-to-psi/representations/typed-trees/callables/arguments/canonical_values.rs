impl crate::source_state::TypedTrees {
    /// Land a static scalar at its binder's carrier before either describing
    /// or specializing the application. Names arrive already resolved; only
    /// the canonical value codec, never a rendered decimal name, supplies a value.
    pub fn prepare_callable_scalar_argument(
        &self,
        primitive: crate::source_state::types::PrimitiveType,
        argument: &crate::source_state::expression::StaticMachineArgument,
    ) -> Result<crate::source_state::expression::ExpressionNode, &'static str> {
        use crate::source_state::expression::ExpressionNode;
        use crate::source_state::types::PrimitiveType;
        use application_identity::const_value::CanonicalConstValue;
        use application_identity::const_value::DecodedCanonicalConstValue;
        use numeric_literals::{IntegerLanding, IntegerLiteral};

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
                        crate::source_state::types::TypeReferenceNode::Named { .. }
                    ) || self.primitive_type_reference(declaration.declared_type) != Some(primitive)
                })
            {
                return Err("a named constant does not have its binder's carrier");
            }
            crate::source_state::canonical_const_declaration_value(self, argument.symbol)
        } else if argument.type_reference.is_valid() {
            match self.type_reference_node(argument.type_reference) {
                crate::source_state::types::TypeReferenceNode::Named { symbol, name }
                    if !symbol.is_valid() =>
                {
                    CanonicalConstValue::from_atom(name.as_str())
                }
                crate::source_state::types::TypeReferenceNode::ConstExpression(value) => {
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
                let literal = IntegerLiteral::from_integer(value);
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

    /// The checked scalar value at a callable binder's declared carrier.
    pub fn callable_scalar_value(
        &self,
        primitive: crate::source_state::types::PrimitiveType,
        argument: &crate::source_state::expression::StaticMachineArgument,
    ) -> Result<application_identity::const_value::DecodedCanonicalConstValue, &'static str> {
        use crate::source_state::expression::ExpressionNode;
        use application_identity::const_value::DecodedCanonicalConstValue;

        match self.prepare_callable_scalar_argument(primitive, argument)? {
            ExpressionNode::Integer(literal) => Ok(DecodedCanonicalConstValue::Integer {
                integer_type: primitive
                    .landed_landing_fixed()
                    .ok_or("an application's integer binding has no admitted carrier")?,
                value: literal
                    .value_bignum()
                    .ok_or("an application's integer binding has no canonical value")?,
            }),
            ExpressionNode::Boolean(value) => Ok(DecodedCanonicalConstValue::Boolean(value)),
            _ => Err("an application's value binding has no admitted scalar value"),
        }
    }

    /// The ordinary schema of a const operation-record parameter. This does
    /// not admit its construction or provide callable refinement evidence.
    pub fn static_record_schema(
        &self,
        reference: crate::source_state::types::TypeReferenceHandle,
        owner: symbols::SymbolHandle,
    ) -> Option<crate::source_state::trait_definition::AppliedTraitScope<'_>> {
        if !self.type_reference_table.contains_type_reference(reference)
            || !matches!(
                self.type_reference_table.type_reference(reference),
                crate::source_state::types::TypeReferenceNode::Named { .. }
                    | crate::source_state::types::TypeReferenceNode::Generic { .. }
            )
        {
            return None;
        }
        crate::source_state::trait_definition::AppliedTraitScope::from_type_reference(
            self, reference, owner,
        )
    }

    /// The one canonical callable atom producer. Both named operation-record
    /// fields and retained machine values supply the same exact code, static
    /// application and requirement coordinates here.
    pub fn canonical_callable_selection_value(
        &self,
        selected: &crate::source_state::expression::StaticMachineArgument,
        application: crate::source_state::static_applications::StaticMachineApplicationHandle,
        requirement: Option<String>,
        exact_sources: &[(source_text::SourceId, [u8; 32])],
    ) -> Result<application_identity::const_value::DecodedCanonicalConstValue, &'static str> {
        let (declaration, application) = crate::source_state::type_identity::portable_machine_selection_identity(
            self,
            selected,
            application,
            exact_sources,
        )
        .map_err(
            |_| "a canonical callable lost its exact selected declaration or static application",
        )?;
        let nominal = self
            .exact_owner_nominal_identity(declaration, exact_sources)
            .and_then(|identity| self.type_identity_text(identity))
            .filter(|identity| !identity.is_empty())
            .ok_or("a canonical callable lost its exact declaration owner")?;
        self.canonical_callable_atom(declaration, application, requirement, nominal)
    }

    pub(crate) fn canonical_callable_selection_value_in_context(
        &self,
        selected: &crate::source_state::expression::StaticMachineArgument,
        application: crate::source_state::static_applications::StaticMachineApplicationHandle,
        requirement: Option<String>,
        context: &crate::source_state::type_identity::identity_context::TypeIdentityContext<'_>,
    ) -> Result<application_identity::const_value::DecodedCanonicalConstValue, &'static str> {
        let (declaration, application) = crate::source_state::type_identity::portable_machine_selection_identity_in_context(
            self, selected, application, context,
        ).map_err(|_| "a canonical callable lost its exact selected declaration or static application")?;
        let nominal = context.name(self, declaration, "");
        self.canonical_callable_atom(declaration, application, requirement, nominal)
    }

    fn canonical_callable_atom(
        &self,
        declaration: symbols::SymbolHandle,
        application: Option<String>,
        requirement: Option<String>,
        nominal: String,
    ) -> Result<application_identity::const_value::DecodedCanonicalConstValue, &'static str> {
        if nominal.is_empty() {
            return Err("a canonical callable lost its declaration owner");
        }
        let callable = self
            .machine_by_symbol(declaration)
            .and_then(|machine| self.machine_callable_identity_text(machine))
            .filter(|identity| !identity.is_empty())
            .ok_or("a canonical callable lost its selected overload identity")?;
        let declaration = application_identity::type_identity::encoding::callable_declaration_path(
            &nominal, &callable,
        );
        Ok(
            application_identity::const_value::DecodedCanonicalConstValue::StaticCallable {
                declaration,
                application,
                requirement,
            },
        )
    }

    /// Read an existing checked machine-value selection as a canonical atom.
    /// Its stored requirement is checked through its exact source row; encoding
    /// the identity neither forms a new descriptor nor supplies refinement.
    pub fn canonical_static_callable_value(
        &self,
        value: &crate::source_state::callable_values::MachineValue,
        exact_sources: &[(source_text::SourceId, [u8; 32])],
    ) -> Result<application_identity::const_value::DecodedCanonicalConstValue, &'static str> {
        let machine = self
            .machine_by_symbol(value.selected.symbol)
            .or_else(|| self.machine_holding_state(value.selected.symbol))
            .ok_or("a canonical callable lost its exact selected declaration")?;
        let requirement = match (value.conformance.is_valid(), value.requirement.is_valid()) {
            (false, false) => None,
            (true, true) => {
                let (_, row, signature) = self
                    .named_conformance_member(value.conformance, value.requirement)?
                    .ok_or("a canonical callable lost its requirement binding")?;
                if row.realization_machine != machine.symbol {
                    return Err("a canonical callable changed its selected requirement member");
                }
                let owner = self
                    .trait_definition_by_symbol(row.declaring_trait)
                    .ok_or("a canonical callable lost its declaring schema")?;
                Some(self.trait_requirement_callable_identity_text(owner, signature))
            }
            _ => return Err("a canonical callable lost its complete requirement binding"),
        };
        self.canonical_callable_selection_value(
            &value.selected,
            value.application,
            requirement,
            exact_sources,
        )
    }

    pub(crate) fn is_closed_aggregate_operand(
        &self,
        carrier: crate::source_state::types::TypeReferenceHandle,
        argument: &crate::source_state::expression::StaticMachineArgument,
    ) -> bool {
        self.contains_type_reference(carrier)
            && self.primitive_type_reference(carrier).is_none()
            && argument.symbol.is_valid()
            && (self.const_declaration_by_symbol(argument.symbol).is_some()
                || self.variant_owner(argument.symbol).is_some())
    }

    /// Retain every field of an already evaluated aggregate value. The source
    /// constant's declared carrier, rather than its display name, owns typing.
    pub fn canonical_aggregate_argument(
        &self,
        declared_type: crate::source_state::types::TypeReferenceHandle,
        argument: &crate::source_state::expression::StaticMachineArgument,
    ) -> Result<application_identity::const_value::CanonicalConstValue, &'static str> {
        if !self.contains_type_reference(declared_type) {
            return Err("a static aggregate has no declared carrier");
        }
        if argument.application.is_some()
            || argument.evidence_projection.is_some()
            || argument.const_literal.is_some()
        {
            return Err("a static aggregate needs its exact closed value operand");
        }
        if let Some((owner, case)) = self.variant_owner(argument.symbol) {
            return self.canonical_case_argument(declared_type, owner, case);
        }
        let declaration = self
            .const_declaration_by_symbol(argument.symbol)
            .ok_or("a static aggregate has no evaluated declaration")?;
        if self.normalized_type_identity(declaration.declared_type)
            != self.normalized_type_identity(declared_type)
        {
            return Err("a static aggregate has another declared carrier");
        }
        let value =
            crate::source_state::canonical_const_declaration_value(self, declaration.symbol)
                .ok_or("a static aggregate lost its evaluated value")?;
        if !value.is_canonical_static_value() {
            return Err("a static aggregate has no canonical static value");
        }
        let decoded = value
            .decode_encoding()
            .ok_or("a static aggregate lost its canonical encoding")?;
        if !self.canonical_const_materialization_matches(
            declaration.declared_type,
            declaration.materialized_initializer,
            &decoded,
        ) {
            return Err("a static aggregate differs from its checked fields or selections");
        }
        Ok(value)
    }

    /// Describe the resulting named record value in schema declaration order.
    /// Construction and member refinement remain with ordinary source checking;
    /// the canonical value retains the exact selections instead of its alias.
    pub fn canonical_operation_record_value(
        &self,
        symbol: symbols::SymbolHandle,
        exact_sources: &[(source_text::SourceId, [u8; 32])],
    ) -> Result<application_identity::const_value::CanonicalConstValue, &'static str> {
        crate::source_state::type_identity::portable_machine_applications::portable_operation_record_value(
            self,
            symbol,
            exact_sources,
        )
        .map_err(|_| "a canonical operation record lost its finite selected-value identity")
    }

    pub(crate) fn canonical_operation_record_value_in_context(
        &self,
        symbol: symbols::SymbolHandle,
        context: &crate::source_state::type_identity::identity_context::TypeIdentityContext<'_>,
    ) -> Result<application_identity::const_value::CanonicalConstValue, &'static str> {
        use crate::source_state::trait_definition::AppliedTraitScope;
        use application_identity::const_value::CanonicalConstValue;
        use application_identity::const_value::DecodedCanonicalConstValue;

        let selected = self
            .conformance_by_symbol(symbol)
            .ok_or("a canonical operation record has no named value")?;
        if !selected.lifetime_parameters.is_empty()
            || !self.conformance_type_parameters(selected).is_empty()
        {
            return Err("a canonical operation record needs its complete closed application");
        }
        let root = AppliedTraitScope::from_conformance(self, selected)
            .ok_or("a canonical operation record lost its schema application")?;
        let schema =
            crate::source_state::type_identity::portable_record_schema_identity_in_context(
                self, &root, context,
            )
            .map_err(|_| "a canonical operation record lost its applied schema identity")?;
        let mut scopes = Vec::<AppliedTraitScope<'_>>::new();
        root.try_visit(self, &mut |scope| {
            if let Some(previous) = scopes
                .iter()
                .find(|previous| previous.declaration.symbol == scope.declaration.symbol)
            {
                if !previous.same_application(self, scope) {
                    return Err(
                        "a canonical operation record inherits different schema applications",
                    );
                }
            } else {
                scopes.push(scope.clone());
            }
            Ok(())
        })?;
        let mut fields = Vec::new();
        for scope in scopes {
            for requirement in self.trait_machine_signatures(scope.declaration) {
                let (_, row, applied) = self
                    .named_conformance_member(symbol, requirement.symbol)?
                    .ok_or("a canonical operation record lost its exact member selection")?;
                if row.declaring_trait != scope.declaration.symbol {
                    return Err("a canonical operation record changed its declaring schema");
                }
                let requirement =
                    self.trait_requirement_callable_identity_text(scope.declaration, applied);
                if requirement.is_empty() {
                    return Err("a canonical operation record lost its applied member contract");
                }
                let selected_machine = self
                    .machine_by_symbol(row.realization_machine)
                    .ok_or("a canonical operation record lost its selected machine")?;
                let Some([entry]) = self.machine_states(selected_machine).as_array() else {
                    return Err("a canonical operation record needs one exact selected entry");
                };
                if entry.symbol != row.realization_state {
                    return Err("a canonical operation record changed its selected entry");
                }
                let selected = crate::source_state::expression::StaticMachineArgument {
                    symbol: selected_machine.symbol,
                    ..Default::default()
                };
                let value = self.canonical_callable_selection_value_in_context(
                    &selected,
                    crate::source_state::static_applications::StaticMachineApplicationHandle::invalid(),
                    Some(requirement.clone()),
                    context,
                )?;
                fields.push((requirement, value));
            }
        }
        CanonicalConstValue::from_decoded(
            &DecodedCanonicalConstValue::Record {
                type_name: schema,
                fields,
            },
            "",
        )
        .ok_or("a canonical operation record exceeds the admitted static value profile")
    }

    /// Form the declaration's complete supported tuple without requesting an
    /// executable instance. The generic type node retains the selected owner
    /// and arguments in that owner's exact binder order, including landed values.
    pub(crate) fn prepare_callable_operand_application(
        &mut self,
        selected: &crate::source_state::expression::StaticMachineArgument,
        owner: symbols::SymbolHandle,
    ) -> Result<
        (
            crate::source_state::types::TypeReferenceHandle,
            Vec<crate::source_state::static_applications::StaticMachineBinding>,
        ),
        &'static str,
    > {
        use crate::source_state::data::TypeParameterKind;
        use crate::source_state::static_applications::{
            StaticMachineBinding, StaticMachineBindingValue,
        };
        use crate::source_state::types::TypeReferenceNode;
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
            || parameters.iter().any(|parameter| {
                parameter.bounds != crate::source_state::data::DataProperties::default()
            })
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
                    if self.type_argument_names_value(reference) {
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

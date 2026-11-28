use super::CallableApplication;
use crate::source_state::{TypedTrees, expression, types};

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
                    return Err("an applied declaration operand requires a machine or requirement");
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
        use crate::source_state::type_identity::{IndexIdentityArgument, applied_index_identity};

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

    /// Both callable and type-alias applications use the same type-reference
    /// arena for type and value arguments. A type binder cannot accept a
    /// value, including when substitution would erase the argument.
    pub fn type_argument_names_value(&self, reference: types::TypeReferenceHandle) -> bool {
        use crate::source_state::data::TypeParameterKind;
        use symbols::SymbolKind;
        let symbol = match self.type_reference_table.type_reference(reference) {
            types::TypeReferenceNode::ConstExpression(_) => return true,
            types::TypeReferenceNode::Named { symbol, .. } => *symbol,
            _ => return false,
        };
        if !self.symbols.symbols().nodes().is_valid(symbol) {
            return self
                .static_const_literal_from_type_reference(reference)
                .is_some();
        }
        match self.symbols.get(symbol).kind {
            SymbolKind::TypeParameter => {
                self.data_type_parameter_by_symbol(symbol)
                    .is_some_and(|parameter| {
                        matches!(
                            parameter.kind,
                            TypeParameterKind::Const { .. } | TypeParameterKind::Value { .. }
                        )
                    })
            }
            SymbolKind::Const
            | SymbolKind::Parameter
            | SymbolKind::Local
            | SymbolKind::Machine
            | SymbolKind::MachineParameter
            | SymbolKind::ConformanceParameter => true,
            _ => false,
        }
    }

    pub fn static_operand_from_type_reference(
        &self,
        reference: types::TypeReferenceHandle,
    ) -> Result<expression::StaticMachineArgument, &'static str> {
        use expression::{ExpressionNode, StaticMachineArgument, StaticSymbolApplication};
        use types::TypeReferenceNode;

        if !reference.is_valid() || !self.type_reference_table.contains_type_reference(reference) {
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
    ) -> Option<numeric_literals::IntegerLiteral> {
        self.type_reference_table
            .type_reference(reference)
            .integer_literal(&self.expression_table)
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
            .type_reference_rows_from(start)
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
        lexical_owner: symbols::SymbolHandle,
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
                let value = application_identity::const_value::CanonicalConstValue::from_atom(
                    name.as_str(),
                )
                .ok_or("callable expression argument has no represented canonical value")?;
                let decoded = value
                    .decode_encoding()
                    .ok_or("callable canonical value is malformed")?;
                if let application_identity::const_value::DecodedCanonicalConstValue::Boolean(
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
                } else if program.variant_owner(symbol).is_some() {
                    let argument = expression::StaticMachineArgument {
                        symbol,
                        ..Default::default()
                    };
                    let value = program.canonical_aggregate_argument(carrier, &argument)?;
                    let decoded = value
                        .decode_encoding()
                        .ok_or("a static case lost its canonical value")?;
                    let node = program
                        .materialize_canonical_const_value(carrier, &decoded)
                        .ok_or("a static case cannot be constructed at the selected carrier")?;
                    return Ok(program.expression_table.insert(node));
                } else if let Some(record) = program.conformance_by_symbol(symbol) {
                    if !record.lifetime_parameters.is_empty()
                        || !program.conformance_type_parameters(record).is_empty()
                    {
                        return Err("callable record value needs its complete closed application");
                    }
                    if !program
                        .type_reference_table
                        .contains_type_reference(carrier)
                        || !matches!(
                            program.type_reference_node(carrier),
                            types::TypeReferenceNode::Named { .. }
                                | types::TypeReferenceNode::Generic { .. }
                        )
                    {
                        return Err("callable record value lost its exact declared carrier");
                    }
                    let required =
                            crate::source_state::trait_definition::AppliedTraitScope::from_type_reference(
                                program,
                                carrier,
                                lexical_owner,
                            )
                            .ok_or("callable record value lost its exact declared carrier")?;
                    let actual =
                        crate::source_state::trait_definition::AppliedTraitScope::from_conformance(
                            program, record,
                        )
                        .ok_or("callable record value lost its complete schema application")?;
                    if !required.same_application(program, &actual) {
                        return Err("callable record value has another declared schema");
                    }
                    // Keep the exact named value. Member refinement and
                    // ordinary value-use checks still own its obligations.
                } else if !program
                    .data_type_parameter_by_symbol(symbol)
                    .is_some_and(|parameter| parameter.kind.is_const_or_value())
                {
                    return Err(
                        "callable expression argument does not name a const or value declaration",
                    );
                }
                Ok(program.expression_table.insert_name_path(
                    &expression::NamePath::resolved_from_iter([name], symbol, symbol),
                ))
            }
            _ => Err("nominal callable expression application needs an expression-valued argument"),
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
        if let Some(spelling) =
            crate::source_state::static_const_argument_spelling(program, selected)
            && let Some(value) =
                application_identity::const_value::CanonicalConstValue::from_atom(&spelling)
        {
            let node = match value
                .decode_encoding()
                .ok_or("applied callable canonical value is malformed")?
            {
                application_identity::const_value::DecodedCanonicalConstValue::Boolean(value) => {
                    let value = program
                        .expression_table
                        .insert(expression::ExpressionNode::Boolean(value));
                    types::TypeReferenceNode::ConstExpression(value)
                }
                _ => types::TypeReferenceNode::Named {
                    symbol: symbols::SymbolHandle::invalid(),
                    name: crate::source_state::name::Identifier::generated(value.atom()),
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
#[path = "../tests/selected_operands.rs"]
mod tests;

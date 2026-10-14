//! Preserve complete caller-owned static operands in signature type payloads.
//! Implementation slots may appear inside a data/domain application as type
//! references; replacing only expression arguments would leave a free provider
//! binder. This is representation transport, never evidence selection.

use super::CallableApplication;
use crate::{TypedTrees, expression, types};

impl TypedTrees {
    /// Recover one static operand without duplicating its category authority.
    /// Applied data stays a complete type payload; named code keeps its own
    /// recursive telescope. In particular, a const expression is never a type.
    pub fn static_operand_from_type_reference(
        &self,
        reference: types::TypeReferenceHandle,
    ) -> Result<expression::StaticMachineArgument, &'static str> {
        use expression::{ExpressionNode, StaticMachineArgument, StaticSymbolApplication};
        use types::TypeReferenceNode;

        if !reference.is_valid() || !self.type_reference_table.contains_type_reference(reference) {
            return Err("static operand has no retained type payload");
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
                    .type_reference_table
                    .type_reference_handles(*arguments)
                    .iter()
                    .map(|argument| self.static_operand_from_type_reference(*argument))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(StaticMachineArgument {
                    symbol: *base_symbol,
                    path: Box::from([base_name.clone()]),
                    application: Some(Box::new(StaticSymbolApplication {
                        lifetime_arguments: lifetime_arguments.clone().into_boxed_slice(),
                        arguments: arguments.into_boxed_slice(),
                    })),
                    ..StaticMachineArgument::default()
                })
            }
            TypeReferenceNode::ConstExpression(value) => match self
                .expression_table
                .expression(*value)
            {
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
                        language_semantics::const_value::CanonicalConstValue::boolean(*value)
                            .atom(),
                    )]),
                    ..StaticMachineArgument::default()
                }),
                _ => Err("static operand requires a retained declaration or evaluated const value"),
            },
            _ => Ok(StaticMachineArgument {
                type_reference: reference,
                ..StaticMachineArgument::default()
            }),
        }
    }

    /// Numeric type payloads are retained by older specialization bindings as
    /// unresolved literal names, and by callable applications as expressions.
    /// Both denote a const operand, never a second named/type selection.
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

    /// Project the complete static operand into a type payload without
    /// selecting evidence or discarding its lifetime application.
    pub fn static_operand_type_reference(
        &mut self,
        selected: &expression::StaticMachineArgument,
    ) -> Result<types::TypeReferenceHandle, &'static str> {
        CallableApplication::selected_argument_reference(self, selected)
    }

    /// Materialize an incoming operand in its retained application's lexical
    /// scope. Newly copied nodes keep that supplier; existing explicit custody
    /// is preserved. This carries representation, never a region or loan grant.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supplied_operand_does_not_acquire_preexisting_region_node() {
        let mut program = TypedTrees::default();
        let unit = program
            .type_reference_table
            .insert(types::TypeReferenceNode::Unit);
        let incoming = program
            .type_reference_table
            .insert(types::TypeReferenceNode::Reference {
                referee: unit,
                access: language_semantics::ReferenceAccess::Shared,
                lifetime: Some(crate::name::Identifier::generated("incoming")),
            });
        let unrelated = program
            .type_reference_table
            .insert(types::TypeReferenceNode::Reference {
                referee: unit,
                access: language_semantics::ReferenceAccess::Shared,
                lifetime: Some(crate::name::Identifier::generated("unrelated")),
            });
        // This test checks representation transport, not declaration admission
        // or loan authority. The owner is a distinct nonzero transport key.
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

impl CallableApplication {
    /// A supplied value can be retained as an expression or by its declaration.
    /// Copy evaluated constants, but preserve an open value binder's exact
    /// caller identity. Never apply the receiving telescope to either payload.
    pub(super) fn selected_value_expression(
        program: &mut TypedTrees,
        reference: types::TypeReferenceHandle,
    ) -> Result<expression::ExpressionHandle, &'static str> {
        if let Some(literal) = program.static_const_literal_from_type_reference(reference) {
            return Ok(program
                .expression_table
                .insert(expression::ExpressionNode::Integer(literal)));
        }
        match program
            .type_reference_table
            .type_reference(reference)
            .clone()
        {
            types::TypeReferenceNode::ConstExpression(value) => {
                Ok(program.expression_table.insert_copy(value))
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
            _ => Err("nominal callable expression application needs an expression-valued argument"),
        }
    }

    pub(super) fn selected_argument_reference(
        program: &mut TypedTrees,
        selected: &expression::StaticMachineArgument,
    ) -> Result<types::TypeReferenceHandle, &'static str> {
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
        if let Some(declaration) = program.const_declaration_by_symbol(selected.symbol)
            && declaration.evaluated_value.is_some()
            && declaration.materialized_initializer.is_valid()
        {
            let initializer = declaration.materialized_initializer;
            let value = program.expression_table.insert_copy(initializer);
            return Ok(program
                .type_reference_table
                .insert(types::TypeReferenceNode::ConstExpression(value)));
        }
        if let Some(spelling) =
            crate::typed_trees::static_const_argument_spelling(program, selected)
            && let Some(value) =
                language_semantics::const_value::CanonicalConstValue::from_atom(&spelling)
            && let Some(language_semantics::const_value::DecodedCanonicalConstValue::Boolean(value)) =
                value.decode_encoding()
        {
            let value = program
                .expression_table
                .insert(expression::ExpressionNode::Boolean(value));
            return Ok(program
                .type_reference_table
                .insert(types::TypeReferenceNode::ConstExpression(value)));
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

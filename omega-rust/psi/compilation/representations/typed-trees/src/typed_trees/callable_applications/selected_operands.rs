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
                            language_semantics::const_value::CanonicalConstValue::boolean(*value)
                                .atom(),
                        )]),
                        ..StaticMachineArgument::default()
                    }),
                    _ => Ok(StaticMachineArgument {
                        // A compound const keeps its original value graph. Its
                        // declaration-owned slot still admits the operand later.
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

    /// Numeric type payloads are retained by older specialization bindings as
    /// unresolved literal names, and by callable applications as expressions.
    /// Both denote a const operand, never a second named/type selection.
    pub fn static_const_literal_from_type_reference(
        &self,
        reference: types::TypeReferenceHandle,
    ) -> Option<numerics::literals::IntegerLiteral> {
        if self
            .type_reference_table
            .completed_const_result(reference)
            .is_some()
        {
            return crate::static_applications::static_const_literal_from_type_reference(
                self, reference,
            );
        }
        match self.type_reference_table.type_reference(reference) {
            types::TypeReferenceNode::ConstExpression(value) => {
                let expression::ExpressionNode::Integer(literal) =
                    self.expression_table.expression(*value)
                else {
                    return None;
                };
                Some(literal.clone())
            }
            types::TypeReferenceNode::Named { symbol, .. } if !symbol.is_valid() => {
                crate::static_applications::static_const_literal_from_type_reference(
                    self, reference,
                )
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
}

impl CallableApplication {
    /// Copy a supplied const into expression scratch without applying the
    /// receiving telescope. Completed operands use the same original-meaning
    /// projection as static call arguments; unavailable integer meaning is
    /// never reconstructed from the receiving parameter's carrier.
    pub(super) fn selected_const_expression(
        program: &mut TypedTrees,
        reference: types::TypeReferenceHandle,
    ) -> Result<expression::ExpressionHandle, &'static str> {
        if !reference.is_valid() || !program.type_reference_table.contains_type_reference(reference) {
            return Err("applied callable const has no retained type payload");
        }
        if let types::TypeReferenceNode::ConstExpression(value) =
            program.type_reference_table.type_reference(reference)
        {
            let value = *value;
            return Ok(program.expression_table.insert_copy(value));
        }
        let selected = program.static_operand_from_type_reference(reference)?;
        let applied = Self::selected_argument_reference(program, &selected)?;
        let types::TypeReferenceNode::ConstExpression(value) =
            program.type_reference_table.type_reference(applied)
        else {
            return Err("nominal callable expression application needs an expression-valued argument");
        };
        // This materialization already copied the declaration initializer or
        // emitted the original literal/Boolean operand into invocation scratch.
        Ok(*value)
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

//! Preserve complete caller-owned static operands in signature type payloads.
//! Implementation slots may appear inside a data/domain application as type
//! references; replacing only expression arguments would leave a free provider
//! binder. This is representation transport, never evidence selection.

use super::CallableApplication;
use crate::{TypedTrees, expression, types};

impl TypedTrees {
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

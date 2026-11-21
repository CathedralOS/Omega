//! Closed literals retained before executable initializers are checked.

use crate::resolution::declarations::declaration_selection::ConstantSelection;
use crate::resolution::declarations::generic_data::canonicalize_selected_declared_const_definition;
use crate::resolution::tables::syntax_expressions;
use diagnostics::Diagnostic;
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::ExpressionHandle;
use syntax_trees::identifier::Identifier;
use syntax_trees::item::ConstDefinition;
use syntax_trees::types::{FixedArrayLength, TypeReferenceNode};

/// A closed module literal can name a constructor in its declaring source.
/// Generic carriers validate the selected template here and close their
/// arguments later, when their exact instance exists.
pub(super) fn validate_module_literal(
    syntax: &SyntaxTrees,
    constant: &ConstDefinition,
    selection: &ConstantSelection,
) -> Result<(), Diagnostic> {
    if !selection
        .header_symbols()
        .source_module(constant.name.source_span().source_id)
        .is_valid()
        || primitive_literal_declaration(syntax, constant, Some(selection))
    {
        return Ok(());
    }
    let result = if let Some(base_name) = generic_const_carrier_leaf(syntax, constant) {
        selection.data(syntax, base_name).and_then(|definition| {
            if definition.type_parameters.is_empty() {
                Err(format!(
                    "`{base_name}` does not select a generic data template",
                    base_name = base_name.as_str(syntax)
                ))
            } else {
                Ok(())
            }
        })
    } else {
        canonicalize_selected_declared_const_definition(syntax, constant, Some(selection))
            .map(|_| ())
    };
    result.map_err(|reason| {
        Diagnostic::error(format!(
            "module-owned nominal constant `{}` is invalid: {reason}",
            constant.name.as_str(syntax)
        ))
        .with_source_span(constant.name.source_span())
    })
}

pub(super) fn primitive_literal_declaration(
    syntax: &SyntaxTrees,
    constant: &ConstDefinition,
    selection: Option<&ConstantSelection>,
) -> bool {
    use syntax_trees::expression::ExpressionNode;
    if !scalar_literal_tree(syntax, constant.value()) {
        return false;
    }
    let mut type_reference = constant.type_reference;
    let is_array = matches!(
        syntax.type_references.type_reference(type_reference),
        TypeReferenceNode::FixedArray { .. }
    );
    if !is_array
        && matches!(
            syntax.expressions.expression(constant.value()),
            ExpressionNode::ArrayLiteral(_)
        )
    {
        return false;
    }
    loop {
        match syntax.type_references.type_reference(type_reference) {
            TypeReferenceNode::Named(name) => {
                return selection
                    .and_then(|selection| selection.builtin_type(syntax, name))
                    .is_some_and(|atom| {
                        crate::resolution::constants::is_structural_scalar_carrier(atom)
                            || (!is_array
                                && matches!(
                                    atom,
                                    symbols::BuiltinTypeAtom::F32 | symbols::BuiltinTypeAtom::F64
                                ))
                    });
            }
            TypeReferenceNode::FixedArray {
                element_type,
                length: FixedArrayLength::Literal(_),
            } => type_reference = *element_type,
            _ => return false,
        }
    }
}

/*
    The base spelling of a module constant's generic carrier once fixed-array
    layers are peeled: `Box<u64>` and `[Box<u64>; 2]` share this gate. Any
    other leaf — named, constrained, builtin — keeps its existing owner.
*/
fn generic_const_carrier_leaf<'a>(
    syntax: &'a SyntaxTrees,
    constant: &ConstDefinition,
) -> Option<&'a Identifier> {
    let mut carrier = constant.type_reference;
    loop {
        carrier = match syntax.type_references.type_reference(carrier) {
            TypeReferenceNode::FixedArray { element_type, .. } => *element_type,
            TypeReferenceNode::Generic { base_name, .. } => return Some(base_name),
            _ => return None,
        };
    }
}

fn scalar_literal_tree(syntax: &SyntaxTrees, expression: ExpressionHandle) -> bool {
    use syntax_trees::expression::ExpressionNode;
    match syntax.expressions.expression(expression) {
        ExpressionNode::Boolean(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::UndeterminedNaN(_)
        | ExpressionNode::String(_) => true,
        ExpressionNode::ArrayLiteral(elements) => syntax_expressions(syntax)
            .expression_handles(*elements)
            .iter()
            .all(|element| scalar_literal_tree(syntax, *element)),
        _ => false,
    }
}

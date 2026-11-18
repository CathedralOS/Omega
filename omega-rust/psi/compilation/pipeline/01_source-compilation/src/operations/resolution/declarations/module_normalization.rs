/*
    Narrow rejection boundaries for transforms which precede module identity.

    Literal arrays of primitive scalars carry no nominal initializer names to
    normalize. They can use the same exact constant-header selection and canonical
    value route as scalars; lexical and package custody still precede publication.
    Every newly admitted array declaration is also checked by the existing
    canonicalizer, including unused private declarations: unused initializers
    never reach destination checking. Array leaves
    therefore stay within its canonical integer/Boolean subset. Scoped literals
    additionally retain their exact module-local or imported owner declaration
    at constant finalization, where complete symbols exist. The attachment owner
    may be generic; value-carrier normalization independently closes the declared
    type without replacing the owner's identity. Scalar substitution then uses
    resolved declaration identity and the declared numeric landing, just as for
    unscoped constants. Primitive arrays use this same selection for body copies,
    preserving their declared element landings and full array type at destinations.
    Nominal constants use the same structural encoder after the
    shared header resolver selects their carrier and every nested constructor
    in its declaring source. Their uses retain exact declaration custody and
    rejoin the receiving parameter after symbol allocation; equal layouts and
    encoded labels never grant nominal identity. Scoped nominal constants use
    the same exact attachment finalization as scoped scalars. A generic carrier
    — `Box<u64>` under any fixed-array layers — cannot hold a canonical
    const-index identity before its closed instance exists, so those
    declarations defer value admission to lowering exactly as root constants
    do. Their base template still selects one generic data declaration in the
    declaring source at this boundary; unselected or nongeneric spellings
    reject here rather than drifting to a later stage's weaker error.
*/

use crate::resolution::constants::validate_scalar_initializer;
use crate::resolution::declarations::generic_data::canonicalize_selected_declared_const_definition;
use crate::resolution::lowering::lowerer::ConstResolutionMode;
use crate::resolution::tables::syntax_expressions;
use crate::resolution::tables::syntax_items;
use crate::resolution::tables::syntax_type_references;
use diagnostics::Diagnostic;
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::ExpressionHandle;
use syntax_trees::identifier::Identifier;
use syntax_trees::item::ConstDefinition;
use syntax_trees::item::Item;
use syntax_trees::types::FixedArrayLength;
use syntax_trees::types::TypeReferenceNode;

pub(crate) fn validate_with_const_resolution_mode(
    syntax: &SyntaxTrees,
    selection: &crate::resolution::declarations::declaration_selection::ConstantSelection,
    mode: ConstResolutionMode,
) -> Result<(), Vec<Diagnostic>> {
    let module_sources = syntax
        .root_items()
        .filter_map(|item| {
            let Item::Module(module) = item else {
                return None;
            };
            syntax_items(syntax)
                .identifier_path_members(module.path)
                .first()
                .map(|member| member.source_span().source_id)
        })
        .collect::<Vec<_>>();
    if module_sources.is_empty() {
        return Ok(());
    }
    for item in syntax.root_items() {
        let Item::Const(constant) = item else {
            continue;
        };
        if !module_sources.contains(&constant.name.source_span().source_id) {
            continue;
        }
        if mode != ConstResolutionMode::Complete
            || crate::resolution::declarations::generic_data::const_destination_requires_proof(
                syntax,
                constant,
                Some(selection),
            )
        {
            // The retained initializer checks this destination in its
            // own module, using the same route as a root declaration.
            continue;
        }
        if module_literal_constant(syntax, constant, Some(selection)) {
            if matches!(
                syntax_type_references(syntax).type_reference(constant.type_reference),
                TypeReferenceNode::FixedArray { .. }
            ) {
                canonicalize_selected_declared_const_definition(syntax, constant, Some(selection))
                    .map_err(|reason| {
                        vec![
                            Diagnostic::error(format!(
                                "module array constant `{}` is invalid: {reason}",
                                constant.name.as_str(syntax)
                            ))
                            .with_source_span(constant.name.source_span()),
                        ]
                    })?;
            } else {
                validate_scalar_initializer(syntax, constant, Some(selection)).map_err(
                    |reason| {
                        vec![
                            Diagnostic::error(format!(
                                "module scalar constant `{}` is invalid: {reason}",
                                constant.name.as_str(syntax)
                            ))
                            .with_source_span(constant.name.source_span()),
                        ]
                    },
                )?;
            }
        } else if let Some(base_name) = generic_const_carrier_leaf(syntax, constant) {
            let selected = selection.data(syntax, base_name).and_then(|definition| {
                if definition.type_parameters.is_empty() {
                    Err(format!(
                        "`{base_name}` does not select a generic data template",
                        base_name = base_name.as_str(syntax)
                    ))
                } else {
                    Ok(())
                }
            });
            selected.map_err(|reason| {
                vec![
                    Diagnostic::error(format!(
                        "module-owned nominal constant `{}` is invalid: {reason}",
                        constant.name.as_str(syntax)
                    ))
                    .with_source_span(constant.name.source_span()),
                ]
            })?;
        } else {
            canonicalize_selected_declared_const_definition(syntax, constant, Some(selection))
                .map_err(|reason| {
                    vec![
                        Diagnostic::error(format!(
                            "module-owned nominal constant `{}` is invalid: {reason}",
                            constant.name.as_str(syntax)
                        ))
                        .with_source_span(constant.name.source_span()),
                    ]
                })?;
        }
    }

    Ok(())
}

pub(crate) fn module_literal_constant(
    syntax: &SyntaxTrees,
    constant: &ConstDefinition,
    selection: Option<&crate::resolution::declarations::declaration_selection::ConstantSelection>,
) -> bool {
    use syntax_trees::expression::ExpressionNode;
    if !scalar_literal_tree(syntax, constant.value) {
        return false;
    }
    let mut type_reference = constant.type_reference;
    let is_array = matches!(
        syntax.type_references.type_reference(type_reference),
        TypeReferenceNode::FixedArray { .. }
    );
    if !is_array
        && matches!(
            syntax.expressions.expression(constant.value),
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

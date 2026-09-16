//! Narrow rejection boundaries for transforms which precede module identity.
//!
//! Literal arrays of primitive scalars carry no nominal initializer names to
//! normalize. They can use the same exact constant-header selection and canonical
//! value route as scalars; lexical and package custody still precede publication.
//! Every newly admitted array declaration is also checked by the existing
//! canonicalizer, including unused private declarations: unused initializers
//! never reach destination checking. Array leaves
//! therefore stay within its canonical integer/Boolean subset. Scoped literals
//! additionally retain their exact nongeneric module-local carrier at constant
//! finalization, where complete symbols exist. Scalar substitution then uses
//! resolved declaration identity and the declared numeric landing, just as for
//! unscoped constants. Primitive arrays use this same selection for body copies,
//! preserving their declared element landings and full array type at destinations.
//! Nominal constants use the same structural encoder after the
//! shared header resolver selects their carrier and every nested constructor
//! in its declaring source. Their uses retain exact declaration custody and
//! rejoin the receiving parameter after symbol allocation; equal layouts and
//! encoded labels never grant nominal identity. Scoped nominal constants use
//! the same exact attachment finalization as scoped scalars. A generic carrier
//! — `Box<u64>` under any fixed-array layers — cannot hold a canonical
//! const-index identity before its closed instance exists, so those
//! declarations defer value admission to lowering exactly as root constants
//! do. Their base template still selects one generic data declaration in the
//! declaring source at this boundary; unselected or nongeneric spellings
//! reject here rather than drifting to a later stage's weaker error.

use diagnostics::Diagnostic;
use syntax_trees::SyntaxTrees;
use syntax_trees::item::Item;
use syntax_trees::types::TypeReferenceNode;

pub(crate) fn validate_module_normalization(syntax: &SyntaxTrees) -> Result<(), Vec<Diagnostic>> {
    let selection = crate::preparation::generic_data::constant_selection::ConstantSelection::new(
        syntax,
        None,
        Vec::new(),
    )?;
    validate_with_selection(syntax, &selection)
}

pub(crate) fn validate_with_selection(
    syntax: &SyntaxTrees,
    selection: &crate::preparation::generic_data::constant_selection::ConstantSelection,
) -> Result<(), Vec<Diagnostic>> {
    validate_with_const_resolution_mode(
        syntax,
        selection,
        crate::resolution::lowerer::ConstResolutionMode::Complete,
    )
}

pub(crate) fn validate_with_const_resolution_mode(
    syntax: &SyntaxTrees,
    selection: &crate::preparation::generic_data::constant_selection::ConstantSelection,
    mode: crate::resolution::lowerer::ConstResolutionMode,
) -> Result<(), Vec<Diagnostic>> {
    let module_sources = syntax
        .root_items()
        .filter_map(|item| {
            let Item::Module(module) = item else {
                return None;
            };
            syntax
                .items
                .identifier_path_members(module.path)
                .first()
                .map(|member| member.source_span().source_id)
        })
        .collect::<Vec<_>>();
    if module_sources.is_empty() {
        return Ok(());
    }
    // Generic method cloning selects attachments by exact carrier declaration
    // in their source context. Same-leaf module carriers need no spelling fence;
    // the common synthesis owner retains its ordinary eligibility restrictions.
    // Trait defaults and conformances join by the same exact source selection:
    // a module-owned template and a same-spelled sibling never share identity,
    // and generated references carry the selected owner's logical path.
    // Module domains and their operator homes resolve by the same
    // namespace rules: qualified semantic identity, module-local precedence,
    // and import-gated relative spellings. Const-fact evaluation and
    // constrained-argument identity select their exact owner through that law,
    // so same-spelled non-generic siblings no longer collide. Indexed domain
    // families intern under their complete logical path and every application
    // selects its telescope through the same name law before any argument
    // folds, so open-template indices on module-owned carriers substitute
    // only against their exact owner. Operators ride the same law: their
    // generic binders close at call-time operand application rather than
    // through the data-template normalization queue, so a module-owned
    // generic operator selects by its qualified or imported spelling exactly
    // like a non-generic sibling.
    for item in syntax.root_items() {
        let unsupported = match item {
            Item::Const(constant)
                if module_sources.contains(&constant.name.source_span().source_id) =>
            {
                if mode == crate::resolution::lowerer::ConstResolutionMode::InitializerSelection
                    && crate::constant::requires_const_initializer_evaluation(syntax, constant)
                {
                    // Only value admission is deferred. Ordinary resolution
                    // still validates the declaration's namespace and carrier.
                    None
                } else if module_literal_constant(syntax, constant) {
                    if matches!(
                        syntax
                            .type_references
                            .type_reference(constant.type_reference),
                        TypeReferenceNode::FixedArray { .. }
                    ) {
                        crate::preparation::generic_data::canonicalize_declared_const_definition(
                            syntax, constant,
                        )
                        .map_err(|reason| {
                            vec![
                                Diagnostic::error(format!(
                                    "module array constant `{}` is invalid: {reason}",
                                    constant.name.as_str()
                                ))
                                .with_source_span(constant.name.source_span()),
                            ]
                        })?;
                    } else {
                        crate::constant::validate_scalar_initializer(syntax, constant).map_err(
                            |reason| {
                                vec![
                                    Diagnostic::error(format!(
                                        "module scalar constant `{}` is invalid: {reason}",
                                        constant.name.as_str()
                                    ))
                                    .with_source_span(constant.name.source_span()),
                                ]
                            },
                        )?;
                    }
                    None
                } else if let Some(base_name) = generic_const_carrier_leaf(syntax, constant) {
                    // A `Box<u64>`-style carrier cannot hold a canonical
                    // const-index identity before its closed instance exists,
                    // so value admission defers to lowering exactly as for a
                    // root constant: the base template still selects exactly in
                    // the declaring source now, and every use destination-
                    // checks the substituted initializer.
                    let selected = selection.data(syntax, base_name).and_then(|definition| {
                        if definition.type_parameters.is_empty() {
                            Err(format!(
                                "`{base_name}` does not select a generic data template"
                            ))
                        } else {
                            Ok(())
                        }
                    });
                    selected.map_err(|reason| {
                        vec![
                            Diagnostic::error(format!(
                                "module-owned nominal constant `{}` is invalid: {reason}",
                                constant.name
                            ))
                            .with_source_span(constant.name.source_span()),
                        ]
                    })?;
                    None
                } else {
                    crate::preparation::generic_data::canonicalize_selected_declared_const_definition(syntax, constant, Some(selection))
                        .map_err(|reason| vec![Diagnostic::error(format!(
                            "module-owned nominal constant `{}` is invalid: {reason}", constant.name
                        )).with_source_span(constant.name.source_span())])?;
                    None
                }
            }
            Item::Data(data)
                if !data.type_parameters.is_empty()
                    && module_sources.contains(&data.name.source_span().source_id)
                    && matches!(data.name.as_str(), "IntervalSet" | "CountedQuantity") =>
            {
                Some((
                    &data.name,
                    "module-owned generic data requires namespace-aware template normalization",
                ))
            }
            _ => None,
        };
        if let Some((name, message)) = unsupported {
            return Err(vec![
                Diagnostic::error(message).with_source_span(name.source_span()),
            ]);
        }
    }

    Ok(())
}

pub(crate) fn module_literal_constant(
    syntax: &SyntaxTrees,
    constant: &syntax_trees::item::ConstDefinition,
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
                return matches!(
                    name.as_str(),
                    "bool" | "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" | "u64" | "addr"
                ) || (!is_array && matches!(name.as_str(), "f32" | "f64"))
                    // The legacy free-constant profile includes this spelling,
                    // but `string` is not a builtin scalar. Do not extend that
                    // exception to newly admitted scoped nominal declarations.
                    || (!is_array && constant.scope.as_str().is_empty() && name.as_str() == "string");
            }
            TypeReferenceNode::FixedArray {
                element_type,
                length: syntax_trees::types::FixedArrayLength::Literal(_),
            } => type_reference = *element_type,
            _ => return false,
        }
    }
}

/// The base spelling of a module constant's generic carrier once fixed-array
/// layers are peeled: `Box<u64>` and `[Box<u64>; 2]` share this gate. Any
/// other leaf — named, constrained, builtin — keeps its existing owner.
fn generic_const_carrier_leaf<'a>(
    syntax: &'a SyntaxTrees,
    constant: &syntax_trees::item::ConstDefinition,
) -> Option<&'a syntax_trees::identifier::Identifier> {
    let mut carrier = constant.type_reference;
    loop {
        carrier = match syntax.type_references.type_reference(carrier) {
            TypeReferenceNode::FixedArray { element_type, .. } => *element_type,
            TypeReferenceNode::Generic { base_name, .. } => return Some(base_name),
            _ => return None,
        };
    }
}

fn scalar_literal_tree(
    syntax: &SyntaxTrees,
    expression: syntax_trees::expression::ExpressionHandle,
) -> bool {
    use syntax_trees::expression::ExpressionNode;
    match syntax.expressions.expression(expression) {
        ExpressionNode::Boolean(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::String(_) => true,
        ExpressionNode::ArrayLiteral(elements) => syntax
            .expressions
            .expression_handles(*elements)
            .iter()
            .all(|element| scalar_literal_tree(syntax, *element)),
        _ => false,
    }
}

#[cfg(test)]
mod tests;

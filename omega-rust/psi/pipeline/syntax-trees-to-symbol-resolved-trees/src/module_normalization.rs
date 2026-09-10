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
//! the same exact attachment finalization as scoped scalars. Foreign/generic
//! attachments still need their full owners.

use diagnostics::Diagnostic;
use source::SourceId;
use syntax_trees::SyntaxTrees;
use syntax_trees::item::Item;
use syntax_trees::types::TypeReferenceNode;

pub(crate) fn validate_module_normalization(syntax: &SyntaxTrees) -> Result<(), Vec<Diagnostic>> {
    let selection =
        crate::generic_data::constant_selection::ConstantSelection::new(syntax, None, Vec::new())?;
    validate_with_selection(syntax, &selection)
}

pub(crate) fn validate_with_selection(
    syntax: &SyntaxTrees,
    selection: &crate::generic_data::constant_selection::ConstantSelection,
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
    let generic_data = syntax
        .root_items()
        .filter_map(|item| {
            let Item::Data(data) = item else {
                return None;
            };
            (!data.type_parameters.is_empty()).then_some(data)
        })
        .collect::<Vec<_>>();
    for item in syntax.root_items() {
        let unsupported = match item {
            Item::Trait(definition)
                if module_sources.contains(&definition.name.source_span().source_id) =>
            {
                Some((&definition.name, "module-owned traits require namespace-aware trait default normalization"))
            }
            Item::Conformance(definition)
                if module_sources.contains(&definition.trait_name.source_span().source_id) =>
            {
                Some((&definition.trait_name, "module-owned conformances require namespace-aware trait default normalization"))
            }
            Item::Domain(definition)
                if module_sources.contains(&definition.name.source_span().source_id) =>
            {
                Some((&definition.name, "module-owned domains require namespace-aware operator home normalization"))
            }
            Item::Operator(definition) => {
                syntax.items.identifier_path_members(definition.name).first()
                    .filter(|name| module_sources.contains(&name.source_span().source_id))
                    .map(|name| (name, "module-owned operators require namespace-aware operator home normalization"))
            }
            Item::Const(constant)
                if module_sources.contains(&constant.name.source_span().source_id) =>
            {
                if module_literal_constant(syntax, constant) {
                    if matches!(
                        syntax.type_references.type_reference(constant.type_reference),
                        TypeReferenceNode::FixedArray { .. }
                    ) {
                        crate::generic_data::canonicalize_declared_const_definition(syntax, constant)
                            .map_err(|reason| {
                                vec![Diagnostic::error(format!(
                                    "module array constant `{}` is invalid: {reason}",
                                    constant.name.as_str()
                                )).with_source_span(constant.name.source_span())]
                            })?;
                    } else {
                        crate::constant::validate_scalar_initializer(syntax, constant).map_err(|reason| {
                            vec![Diagnostic::error(format!(
                                "module scalar constant `{}` is invalid: {reason}",
                                constant.name.as_str()
                            )).with_source_span(constant.name.source_span())]
                        })?;
                    }
                    None
                } else {
                    crate::generic_data::canonicalize_selected_declared_const_definition(syntax, constant, Some(selection))
                        .map_err(|reason| vec![Diagnostic::error(format!(
                            "module-owned nominal constant `{}` is invalid: {reason}", constant.name
                        )).with_source_span(constant.name.source_span())])?;
                    None

                }
            }
            Item::Data(data)
                if !data.type_parameters.is_empty()
                    && module_sources.contains(&data.name.source_span().source_id) =>
            {
                Some((
                    &data.name,
                    "module-owned generic data requires namespace-aware template normalization",
                ))
            }
            Item::Machine(machine)
                if module_sources.contains(&machine.name.source_span().source_id)
                    && machine.attached_data.as_ref().is_some_and(|carrier| {
                        generic_data
                            .iter()
                            .any(|data| data.name.as_str() == carrier.as_str())
                    }) =>
            {
                Some((
                    &machine.name,
                    "module-owned attached machines sharing a generic carrier spelling require namespace-aware template normalization",
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
    // The pre-resolution instance cache deduplicates argument spellings.
    // Two module-owned nominal arguments with one bare spelling cannot yet
    // share that key, even when their template itself has no module.
    for handle in syntax.type_references.generic_nodes() {
        let TypeReferenceNode::Generic {
            base_name,
            arguments,
            ..
        } = syntax.type_references.type_reference(handle)
        else {
            continue;
        };
        if !generic_data
            .iter()
            .any(|data| data.name.as_str() == base_name.as_str())
        {
            continue;
        }
        let mut pending = syntax
            .type_references
            .type_reference_handles(*arguments)
            .to_vec();
        while let Some(argument) = pending.pop() {
            match syntax.type_references.type_reference(argument) {
                TypeReferenceNode::Named(name)
                    if !name.as_str().contains("::")
                        && nominal_argument_has_module_collision(
                            syntax,
                            &module_sources,
                            name.as_str(),
                        ) =>
                {
                    return Err(vec![Diagnostic::error("same-spelled nominal generic arguments from different modules require namespace-aware template normalization")
                        .with_source_span(name.source_span())]);
                }
                TypeReferenceNode::Reference { referee, .. } => pending.push(*referee),
                TypeReferenceNode::Constrained { base_type, .. } => pending.push(*base_type),
                TypeReferenceNode::FixedArray { element_type, .. }
                | TypeReferenceNode::Slice { element_type } => pending.push(*element_type),
                _ => {}
            }
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

fn nominal_argument_has_module_collision(
    syntax: &SyntaxTrees,
    module_sources: &[SourceId],
    name: &str,
) -> bool {
    let declarations = syntax
        .root_items()
        .filter_map(|item| {
            let Item::Data(data) = item else {
                return None;
            };
            (data.name.as_str() == name).then_some(data.name.source_span().source_id)
        })
        .collect::<Vec<_>>();
    declarations.len() > 1
        && declarations
            .iter()
            .any(|source| module_sources.contains(source))
}

#[cfg(test)]
mod tests {
    use super::*;
    use source_files_to_tokens::Lexer;
    use tokens_to_syntax_trees::parse_syntax_trees_into_with_id;

    fn parse(sources: &[&str]) -> SyntaxTrees {
        let mut syntax = SyntaxTrees::default();
        for (source_ordinal, text) in sources.iter().enumerate() {
            let tokens = Lexer::new(text)
                .tokenize()
                .expect("tokenize normalization control");
            parse_syntax_trees_into_with_id(&mut syntax, SourceId(source_ordinal), &tokens)
                .expect("parse normalization control");
        }
        syntax
    }

    #[test]
    fn module_scalar_constants_survive_pre_symbol_normalization() {
        let syntax = parse(&[
            "module first; const VALUE: u64 = 1;",
            "module second; const VALUE: u64 = 2;",
        ]);
        crate::normalize_generic_data(syntax.clone()).expect("body substitution follows symbols");
        crate::lower_syntax_trees(&syntax).expect("exact module constant identities");
    }

    #[test]
    fn unused_module_scalar_initializers_obey_their_declared_carriers() {
        for (carrier, value) in [
            ("u8", "256"),
            ("u8", "1u64"),
            ("bool", "1"),
            ("u64", "true"),
            ("f32", "1.0f64"),
        ] {
            let source = format!("module settings; const VALUE: {carrier} = {value};");
            assert!(
                crate::normalize_generic_data(parse(&[&source])).is_err(),
                "invalid unused declaration accepted: {source}"
            );
        }
    }

    #[test]
    fn module_array_declarations_are_validated_even_without_uses() {
        for visibility in ["", "pub "] {
            for (carrier, value) in [
                ("[u8; 2]", "[1]"),
                ("[u8; 2]", "[1, 2, 3]"),
                ("[u8; 2]", "[1, 256]"),
                ("[u8; 2]", "[1, true]"),
                ("[u8; 2]", "[1u64, 2]"),
                ("[[u8; 2]; 1]", "[[1]]"),
            ] {
                let source =
                    format!("module settings; {visibility}const SIZE: {carrier} = {value};");
                let diagnostics = crate::normalize_generic_data(parse(&[&source]))
                    .expect_err("an unused array declaration still owes component conformance");
                assert!(
                    diagnostics[0].message.contains("module array constant"),
                    "{diagnostics:?}"
                );
            }
            for (carrier, value) in [
                ("[u8; 2]", "[1u8, 2u8]"),
                ("[[bool; 2]; 1]", "[[true, false]]"),
                ("[u64; 0]", "[]"),
            ] {
                let source =
                    format!("module settings; {visibility}const SIZE: {carrier} = {value};");
                crate::normalize_generic_data(parse(&[&source]))
                    .expect("valid canonical array declaration");
            }
        }
    }

    #[test]
    fn structural_integer_encoding_retains_exact_landing_domain() {
        use numerics::arithmetic::ArithmeticDomain;
        use numerics::literals::{IntegerLanding, LandedIntegerType};
        use syntax_trees::expression::ExpressionNode;
        for domain in [
            ArithmeticDomain::Exact,
            ArithmeticDomain::Wrapping,
            ArithmeticDomain::Saturating,
            ArithmeticDomain::Trapping,
        ] {
            let mut syntax = parse(&["module settings; const SIZE: [u8; 1] = [1u8];"]);
            let (handle, literal) = syntax
                .expressions
                .iter_expressions()
                .find_map(|(handle, node)| {
                    if let ExpressionNode::Integer(literal) = node {
                        Some((handle, literal.clone()))
                    } else {
                        None
                    }
                })
                .expect("integer array leaf");
            syntax.expressions.replace_expression(
                handle,
                ExpressionNode::Integer(literal.with_landing(IntegerLanding {
                    landed_type: LandedIntegerType::U8,
                    domain,
                })),
            );
            let result = crate::normalize_generic_data(syntax);
            if domain == ArithmeticDomain::Exact {
                result.expect("matching exact component landing");
            } else {
                assert!(
                    result.expect_err("canonical component cannot erase arithmetic policy")[0]
                        .message
                        .contains("integer literal landing conflicts")
                );
            }
        }
    }

    #[test]
    fn module_arrays_with_unchecked_leaf_types_remain_fenced() {
        for source in [
            "module settings; const SIZE: [f32; 0] = [];",
            "module settings; const SIZE: [string; 0] = [];",
        ] {
            assert!(
                crate::normalize_generic_data(parse(&[source]))
                    .expect_err("array shape cannot bypass missing namespace or value owners")[0]
                    .message
                    .contains("runtime floating/text identity")
            );
        }
    }

    #[test]
    fn module_aggregate_constants_check_unused_initializers() {
        for source in [
            "module first; data Value { value: u64; } const VALUE: Value = Value { value: 1 };",
            "module first; data Value { value: u64; } const VALUE: [Value; 0] = [];",
        ] {
            crate::normalize_generic_data(parse(&[source])).expect("selected nominal initializer");
        }
        for initializer in [
            "Value { value: 256 }",
            "Value {}",
            "Value { value: 1, extra: 2 }",
        ] {
            let source = format!(
                "module first; data Value {{ value: u8; }} const VALUE: Value = {initializer};"
            );
            crate::normalize_generic_data(parse(&[&source]))
                .expect_err("unused nominal initializer must still check");
        }
    }

    #[test]
    fn module_generic_templates_reject_before_same_name_overwrite() {
        let syntax = parse(&[
            "module first; data Box<T> { value: T; }",
            "module second; data Box<T> { other: T; }",
        ]);
        assert!(
            crate::normalize_generic_data(syntax).expect_err("template identities are not ready")
                [0]
            .message
            .contains("template normalization")
        );
    }

    #[test]
    fn module_attached_methods_cannot_join_unrelated_generic_carriers() {
        let syntax = parse(&[
            "data Box<T> { value: T; }",
            "module other; data Box {} machine Box::act(&self) {}",
        ]);
        assert!(
            crate::normalize_generic_data(syntax)
                .expect_err("same carrier spelling has distinct owners")[0]
                .message
                .contains("attached machines")
        );
    }

    #[test]
    fn same_spelled_module_arguments_cannot_share_one_generic_instance() {
        let syntax = parse(&[
            "data Box<T> { value: T; }",
            "module first; data Point {} data Container { value: Box<Point>; }",
            "module second; data Point {}",
        ]);
        assert!(
            crate::normalize_generic_data(syntax).expect_err("nominal argument owners differ")[0]
                .message
                .contains("nominal generic arguments")
        );
    }

    #[test]
    fn unrelated_primitive_generic_instances_remain_available_with_modules() {
        let syntax = parse(&[
            "data Box<T> { value: T; }",
            "module other; data Container { value: Box<u64>; }",
        ]);
        crate::normalize_generic_data(syntax)
            .expect("module presence does not disable unrelated templates");
    }

    #[test]
    fn module_traits_reject_before_default_template_names_are_indexed() {
        let mut syntax = parse(&[
            "module first; trait Service { machine run(); }",
            "module second; trait Service { machine stop(); }",
        ]);
        assert!(
            crate::synthesize_trait_defaults(&mut syntax)
                .expect_err("trait templates need exact namespace owners")[0]
                .message
                .contains("trait default normalization")
        );
    }

    #[test]
    fn module_conformances_cannot_select_a_default_template_by_leaf_name() {
        let mut syntax = parse(&[
            "trait Service { machine run(); }",
            "module implementation; data Worker {} WorkerService: Worker satisfies Service {}",
        ]);
        assert!(
            crate::synthesize_trait_defaults(&mut syntax)
                .expect_err("conformance normalization needs exact namespace owners")[0]
                .message
                .contains("module-owned conformances")
        );
    }

    #[test]
    fn module_domain_and_operator_homes_reject_before_name_based_relocation() {
        for source in [
            "module units; domain u64::Distance;",
            "module units; operator add(left: u64, right: u64) -> u64;",
        ] {
            let syntax = parse(&[source]);
            assert!(
                crate::lower_syntax_trees(&syntax)
                    .expect_err("operator home normalization needs exact namespaces")[0]
                    .message
                    .contains("operator home normalization")
            );
        }
    }
}

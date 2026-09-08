//! Narrow rejection boundaries for transforms which precede module identity.

use diagnostics::Diagnostic;
use source::SourceId;
use syntax_trees::SyntaxTrees;
use syntax_trees::item::Item;
use syntax_trees::types::TypeReferenceNode;

pub(crate) fn validate_module_normalization(syntax: &SyntaxTrees) -> Result<(), Vec<Diagnostic>> {
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
                Some((
                    &constant.name,
                    "module-owned constants require namespace-aware constant substitution",
                ))
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
    fn module_constants_reject_before_substitution() {
        let syntax = parse(&[
            "module first; const VALUE: u64 = 1;",
            "module second; const VALUE: u64 = 2;",
        ]);
        assert!(
            crate::normalize_generic_data(syntax.clone())
                .expect_err("constant identities are not ready")[0]
                .message
                .contains("constant substitution")
        );
        assert!(crate::lower_syntax_trees(&syntax).is_err());
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

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
    // Non-generic module domains and their operator homes resolve by the same
    // namespace rules: qualified semantic identity, module-local precedence,
    // and import-gated relative spellings. Two remaining surfaces still fence:
    // generic templates (open index telescopes and carrier binders share the
    // generic-template normalization queue) and same-named domain siblings
    // (compile-time domain-fact evaluation still selects by declared spelling,
    // so a collision could discharge facts against the wrong owner).
    let domain_names_collide = |definition: &syntax_trees::item::DomainDefinition| {
        syntax.root_items().any(|item| {
            let Item::Domain(other) = item else {
                return false;
            };
            !std::ptr::eq(other, definition) && other.name.as_str() == definition.name.as_str()
        })
    };
    for item in syntax.root_items() {
        let unsupported = match item {
            Item::Domain(definition)
                if module_sources.contains(&definition.name.source_span().source_id) =>
            {
                if !definition.type_parameters.is_empty() {
                    Some((&definition.name, "module-owned generic domains require namespace-aware template normalization"))
                } else if domain_names_collide(definition) {
                    Some((&definition.name, "module-owned domains sharing a declared name with another domain require exact const-evaluation ownership"))
                } else {
                    None
                }
            }
            Item::Operator(definition) => {
                syntax.items.identifier_path_members(definition.name).first()
                    .filter(|name| module_sources.contains(&name.source_span().source_id))
                    .filter(|_| !definition.type_parameters.is_empty())
                    .map(|name| (name, "module-owned generic operators require namespace-aware template normalization"))
            }
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
                        syntax.type_references.type_reference(constant.type_reference),
                        TypeReferenceNode::FixedArray { .. }
                    ) {
                        crate::preparation::generic_data::canonicalize_declared_const_definition(syntax, constant)
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
mod tests {
    use super::*;
    use source::SourceId;
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
        crate::preparation::generic_data::normalize_generic_data(
            crate::preparation::generic_data::GenericDataRequest::new(syntax.clone()),
        )
        .expect("body substitution follows symbols");
        crate::resolve(crate::ResolutionRequest::new(&syntax))
            .expect("exact module constant identities");
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
                crate::preparation::generic_data::normalize_generic_data(
                    crate::preparation::generic_data::GenericDataRequest::new(parse(&[&source]))
                )
                .is_err(),
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
                let diagnostics = crate::preparation::generic_data::normalize_generic_data(
                    crate::preparation::generic_data::GenericDataRequest::new(parse(&[&source])),
                )
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
                crate::preparation::generic_data::normalize_generic_data(
                    crate::preparation::generic_data::GenericDataRequest::new(parse(&[&source])),
                )
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
            let result = crate::preparation::generic_data::normalize_generic_data(
                crate::preparation::generic_data::GenericDataRequest::new(syntax),
            );
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
                crate::preparation::generic_data::normalize_generic_data(
                    crate::preparation::generic_data::GenericDataRequest::new(parse(&[source]))
                )
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
            crate::preparation::generic_data::normalize_generic_data(
                crate::preparation::generic_data::GenericDataRequest::new(parse(&[source])),
            )
            .expect("selected nominal initializer");
        }
        for initializer in [
            "Value { value: 256 }",
            "Value {}",
            "Value { value: 1, extra: 2 }",
        ] {
            let source = format!(
                "module first; data Value {{ value: u8; }} const VALUE: Value = {initializer};"
            );
            crate::preparation::generic_data::normalize_generic_data(
                crate::preparation::generic_data::GenericDataRequest::new(parse(&[&source])),
            )
            .expect_err("unused nominal initializer must still check");
        }
    }

    #[test]
    fn closed_record_arguments_select_current_domains_by_exact_carrier() {
        let syntax = parse(&[
            "data Token {} data Other {} domain Token::Issued; domain Other::Issued; data Cell<T> { value: T; } data Holder { first: Cell<Token in Issued>; second: Cell<Other in Issued>; }",
        ]);
        let normalized = crate::preparation::generic_data::normalize_generic_data(
            crate::preparation::generic_data::GenericDataRequest::new(syntax),
        )
        .expect("current declared domain arguments normalize");
        let instances = normalized
            .root_items()
            .filter_map(|item| {
                let Item::Data(data) = item else {
                    return None;
                };
                data.generic_instance.map(|_| data.name.as_str())
            })
            .collect::<Vec<_>>();
        assert_eq!(
            instances.len(),
            2,
            "both exact carrier-domain applications must synthesize: {instances:?}"
        );
        assert!(instances.contains(&"Cell<Token in Issued>"));
        assert!(instances.contains(&"Cell<Other in Issued>"));
    }

    #[test]
    fn module_generic_records_do_not_overwrite_same_name_templates() {
        let syntax = parse(&[
            "module first; data Box<T> { value: T; }",
            "module second; data Box<T> { other: T; }",
        ]);
        crate::preparation::generic_data::normalize_generic_data(
            crate::preparation::generic_data::GenericDataRequest::new(syntax),
        )
        .expect("distinct module record templates remain available");
    }

    #[test]
    fn module_attached_methods_do_not_join_unrelated_generic_carriers() {
        let syntax = parse(&[
            "data Box<T> { value: T; }",
            "module other; data Box {} machine Box::act(&self) {}",
        ]);
        let syntax = crate::preparation::generic_data::normalize_generic_data(
            crate::preparation::generic_data::GenericDataRequest::new(syntax),
        )
        .expect("same carrier spelling has distinct owners");
        assert_eq!(
            syntax
                .root_items()
                .filter(|item| matches!(item, Item::Machine(_)))
                .count(),
            1
        );
    }

    #[test]
    fn same_spelled_module_arguments_select_their_own_declarations() {
        let syntax = parse(&[
            "data Box<T> { value: T; }",
            "module first; data Point {} data Container { value: Box<Point>; }",
            "module second; data Point {}",
        ]);
        crate::preparation::generic_data::normalize_generic_data(
            crate::preparation::generic_data::GenericDataRequest::new(syntax),
        )
        .expect("the argument is selected in its declaring module");
    }

    #[test]
    fn unrelated_primitive_generic_instances_remain_available_with_modules() {
        let syntax = parse(&[
            "data Box<T> { value: T; }",
            "module other; data Container { value: Box<u64>; }",
        ]);
        crate::preparation::generic_data::normalize_generic_data(
            crate::preparation::generic_data::GenericDataRequest::new(syntax),
        )
        .expect("module presence does not disable unrelated templates");
    }

    #[test]
    fn module_trait_defaults_join_the_exact_selected_template() {
        let mut syntax = parse(&[
            "module first; trait Service { machine run(&mut self) { } } data Worker {} first_membership: Worker satisfies Service;",
            "module second; trait Service { machine stop(&mut self) { } } data Worker {}",
        ]);
        crate::preparation::trait_defaults::synthesize_trait_defaults(&mut syntax)
            .expect("same-spelled module traits keep their own default templates");
        let mut machines = syntax
            .root_items()
            .filter_map(|item| match item {
                Item::Machine(machine) => Some((
                    machine.name.as_str().to_string(),
                    machine
                        .attached_data
                        .as_ref()
                        .map(|attached| attached.source_span().source_id),
                )),
                _ => None,
            })
            .collect::<Vec<_>>();
        machines.sort_by(|left, right| left.0.cmp(&right.0));
        assert_eq!(
            machines,
            [("first::Worker::run".to_string(), Some(SourceId(0)))],
            "only the declaring module's template attaches to its own carrier: {machines:?}"
        );
    }

    #[test]
    fn module_closed_conformance_rows_keep_the_exact_declaring_path() {
        use syntax_trees::item::{ConformanceBody, ConformanceMember};
        let mut syntax = parse(&[
            "module first; trait Service { machine run(&mut self) { } } data Worker {} membership: Worker satisfies Service {}",
            "module second; trait Service { machine stop(&mut self) { } }",
        ]);
        crate::preparation::trait_defaults::synthesize_trait_defaults(&mut syntax)
            .expect("a module conformance selects the same-module trait");
        let conformance = syntax
            .root_items()
            .find_map(|item| match item {
                Item::Conformance(conformance) => Some(conformance),
                _ => None,
            })
            .expect("one conformance");
        let ConformanceBody::Closed { members } = &conformance.body else {
            panic!("closed conformance retained");
        };
        let [
            ConformanceMember::TraitDefault {
                declaring_trait,
                requirement_ordinal,
                machine,
            },
        ] = syntax.items.conformance_members(*members)
        else {
            panic!("exactly one synthesized default row");
        };
        assert_eq!(declaring_trait.as_str(), "first::Service");
        assert_eq!(*requirement_ordinal, 0);
        assert_eq!(machine.name.as_str(), "run");
        assert_eq!(
            machine
                .attached_data
                .as_ref()
                .map(|attached| attached.as_str()),
            Some("Worker")
        );
    }

    #[test]
    fn ambiguous_imported_traits_synthesize_no_default() {
        let mut syntax = parse(&[
            "module first; pub trait Service { machine run(&mut self) { } }",
            "module second; pub trait Service { machine stop(&mut self) { } }",
            "use first::Service; use second::Service; data Worker {} membership: Worker satisfies Service;",
        ]);
        crate::preparation::trait_defaults::synthesize_trait_defaults(&mut syntax)
            .expect("an ambiguous authored trait defers to resolution diagnostics");
        assert!(
            !syntax
                .root_items()
                .any(|item| matches!(item, Item::Machine(_))),
            "no default is synthesized from an ambiguous trait name"
        );
    }

    #[test]
    fn module_attached_machines_override_only_their_own_carrier() {
        let mut syntax = parse(&[
            "trait Service { machine run(&mut self) { } }",
            "module first; data Worker {} machine Worker::run(&mut self) { } first_membership: Worker satisfies Service;",
            "module second; data Worker {} second_membership: Worker satisfies Service;",
        ]);
        crate::preparation::trait_defaults::synthesize_trait_defaults(&mut syntax)
            .expect("carrier identity is exact across same-spelled modules");
        let mut names = syntax
            .root_items()
            .filter_map(|item| match item {
                Item::Machine(machine) => Some(machine.name.as_str().to_string()),
                _ => None,
            })
            .collect::<Vec<_>>();
        names.sort();
        assert_eq!(
            names,
            ["Worker::run", "second::Worker::run"],
            "the authored override suppresses only its own carrier's default: {names:?}"
        );
    }

    #[test]
    fn module_domains_and_operators_lower_under_their_namespace() {
        for source in [
            "module units; domain u64::Distance;",
            "module units; operator add(left: u64, right: u64) -> u64;",
            "module units; domain u64::Distance requires self > 0; operator u64::Distance::add(left: u64 in u64::Distance, right: u64) -> u64;",
        ] {
            let syntax = parse(&[source]);
            crate::resolve(crate::ResolutionRequest::new(&syntax))
                .expect("module-owned domains and operators lower under exact namespaces");
        }
    }

    #[test]
    fn module_generic_templates_and_same_named_domain_siblings_remain_fenced() {
        for (sources, message) in [
            (
                &["module units; domain<T> T::Distance;"][..],
                "namespace-aware template normalization",
            ),
            (
                &["module units; operator copy<T>(value: T) -> T;"][..],
                "namespace-aware template normalization",
            ),
            (
                &["domain u64::Distance; module units; domain u64::Distance;"][..],
                "exact const-evaluation ownership",
            ),
            (
                &[
                    "module units; domain u64::Distance;",
                    "module rooms; domain u64::Distance;",
                ][..],
                "exact const-evaluation ownership",
            ),
        ] {
            let syntax = parse(sources);
            let diagnostics = crate::resolve(crate::ResolutionRequest::new(&syntax))
                .expect_err("fenced module declarations still reject");
            assert!(
                diagnostics[0].message.contains(message),
                "{sources:?}: {diagnostics:?}"
            );
        }
    }
}

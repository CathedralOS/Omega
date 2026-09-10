use super::*;

#[test]
fn semantic_match_arms_retain_exact_qualification_sites_and_instances() {
    for (declarations, domain) in [
        ("domain i64::Km;", "Km"),
        ("domain<T, const U: u64> T::Quantity<U>;", "Quantity<1>"),
        ("domain<T, const U: u64> T::Quantity<U>;", "Quantity<2>"),
        ("domain i64::Km; domain i64::Distance = Km;", "Distance"),
    ] {
        let source = format!(
            "{declarations}
             machine choose(flag: bool, left: i64, right: i64) -> i64 in {domain} {{
                 match flag {{
                     true -> left as i64 in {domain},
                     false -> right as i64 in {domain}
                 }}
             }}"
        );
        let checked = checked_source(&source, false);
        let plans = &checked.facts.values.scalar_computations;
        let root = plans.roots.iter().next().expect("retained return graph").1;
        let CheckedScalarComputationKind::Dispatch { arms, .. } = plans.nodes.get(root.root).kind
        else {
            panic!("qualification must remain in the selected arm: {source}");
        };
        let arms = plans.dispatch_arms.span(arms).unwrap();
        assert_eq!(arms.len(), 2);
        let mut cast_sites = Vec::new();
        for arm in arms {
            let node = plans.nodes.get(arm.value);
            let CheckedScalarComputationKind::Qualification {
                source_expression,
                operand,
                result_type,
            } = node.kind
            else {
                panic!("each selected cast has explicit semantic custody");
            };
            assert_eq!(node.primitive_type, PrimitiveType::I64);
            assert!(!node.value_source.is_valid());
            let ExpressionNode::Cast(cast) = checked.expression_table.expression(source_expression)
            else {
                panic!("exact authored cast");
            };
            assert_eq!(result_type, cast.result_type);
            assert!(result_type.is_valid());
            assert!(cast.semantic_domain_id.is_valid());
            assert_eq!(plans.nodes.get(operand).value_source, cast.value);
            assert!(matches!(
                plans.nodes.get(operand).kind,
                CheckedScalarComputationKind::Value(_)
            ));
            let uses = checked
                .facts
                .qualifications
                .vacuous_uses
                .iter()
                .filter(|usage| usage.expression == source_expression)
                .collect::<Vec<_>>();
            assert_eq!(uses.len(), 1);
            assert_eq!(uses[0].domain, cast.semantic_domain_symbol);
            assert_eq!(uses[0].semantic_domain, cast.semantic_domain_id);
            cast_sites.push(source_expression);
        }
        assert_ne!(cast_sites[0], cast_sites[1]);
    }
}

#[test]
fn scalar_qualifications_keep_distinct_indexed_instances() {
    let checked = checked_source(
        "domain<T, const U: u64> T::Quantity<U>;
         machine first(value: i64) -> i64 in Quantity<1> { value as i64 in Quantity<1> }
         machine second(value: i64) -> i64 in Quantity<2> { value as i64 in Quantity<2> }",
        false,
    );
    let plans = &checked.facts.values.scalar_computations;
    let instances = plans
        .roots
        .iter()
        .map(|(_, root)| {
            let CheckedScalarComputationKind::Qualification {
                source_expression,
                result_type,
                ..
            } = plans.nodes.get(root.root).kind
            else {
                panic!("explicit indexed qualification root");
            };
            let ExpressionNode::Cast(cast) = checked.expression_table.expression(source_expression)
            else {
                panic!("authored cast");
            };
            assert_eq!(result_type, cast.result_type);
            (cast.semantic_domain_symbol, cast.semantic_domain_id)
        })
        .collect::<Vec<_>>();
    assert_eq!(instances.len(), 2);
    assert_eq!(instances[0].0, instances[1].0);
    assert_ne!(instances[0].1, instances[1].1);
}

#[test]
fn vacuous_scalar_eligibility_does_not_claim_predicates_or_routes() {
    let source = "domain i64::Empty;
        domain i64::Positive requires self > 0;
        domain i64::Issued established by Issuer::issue;
        boundary trait Issuer {
            machine issue(value: i64) -> i64 ensures result in i64::Issued;
        }
        domain i64::Alias = Positive;
        machine identity(value: i64) -> i64 { value }";
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    let domains = typed.domain_definitions();
    assert_eq!(domains.len(), 4);
    for (index, domain) in domains.iter().enumerate() {
        assert_eq!(
            crate::facts::domain_is_vacuous(&typed, domain.symbol, &mut Vec::new()),
            index == 0,
            "only the declaration with no membership requirements is eligible"
        );
    }
}

use super::{CheckedScalarComputationKind, ExpressionNode};
use crate::tests::front_end::typed_program;
use crate::values::scalar::computations::tests::checked_source;
use crate::values::scalar::semantic_casts;

#[test]
fn bare_scalar_tag_erasure_retains_the_authored_cast() {
    for carrier in ["i64", "bool", "f64"] {
        let checked = checked_source(
            &format!(
                "domain {carrier}::Tagged;
             machine erase(value: {carrier} in Tagged) -> {carrier} {{ value as {carrier} }}"
            ),
            false,
        );
        assert!(
            checked
                .facts
                .values
                .scalar_expressions
                .expressions
                .is_empty()
        );
        let plans = &checked.facts.values.scalar_computations;
        let root = plans.roots.iter().next().expect("erasure return graph").1;
        let node = plans.nodes.get(root.root);
        let CheckedScalarComputationKind::Qualification {
            source_expression,
            operand,
            result_type,
        } = node.kind
        else {
            panic!("bare erasure has exact semantic custody");
        };
        assert!(!node.value_source.is_valid());
        let ExpressionNode::Cast(cast) = checked.expression_table.expression(source_expression)
        else {
            panic!("authored erasure cast");
        };
        assert!(cast.semantic_domain.is_empty());
        assert!(!semantic_casts::has_declared_domains(
            &checked.typed,
            result_type
        ));
        assert_eq!(plans.nodes.get(operand).value_source, cast.value);
    }
}

#[test]
fn scalar_tag_erasure_composes_inside_arithmetic_and_selected_calls() {
    for (declarations, domain) in [
        ("domain i64::Km;", "Km"),
        ("domain<T, const U: u64> T::Quantity<U>;", "Quantity<7>"),
    ] {
        for body in [
            "(value as i64) + 0i64".to_owned(),
            format!(
                "(match flag {{ true -> identity(value), false -> 9i64 as i64 in {domain} }}) as i64 + 0i64"
            ),
        ] {
            let checked = checked_source(
                &format!(
                    "{declarations}
                 machine identity(value: i64 in {domain}) -> i64 in {domain} {{ value }}
                 machine choose(flag: bool, value: i64 in {domain}) -> i64 {{ {body} }}"
                ),
                false,
            );
            let plans = &checked.facts.values.scalar_computations;
            let root = plans.roots.iter().next().expect("composed return graph").1;
            assert!(matches!(
                plans.nodes.get(root.root).kind,
                CheckedScalarComputationKind::Apply { .. }
            ));
            assert!(plans.nodes.iter().any(|(_, node)| {
                matches!(node.kind, CheckedScalarComputationKind::Qualification { source_expression, .. }
                    if matches!(checked.expression_table.expression(source_expression), ExpressionNode::Cast(cast) if cast.semantic_domain.is_empty()))
            }));
            assert!(
                checked
                    .facts
                    .values
                    .scalar_expressions
                    .expression_at(root.state, root.statement_ordinal, root.role)
                    .is_none()
            );
        }
    }
}

#[test]
fn scalar_erasure_does_not_publish_predicate_or_routed_tag_graphs() {
    for declaration in [
        "domain i64::Tagged requires self != 5;",
        "domain i64::Tagged established by Issuer::issue;
         boundary trait Issuer { machine issue(value: i64) -> i64 ensures result in i64::Tagged; }",
    ] {
        let checked = checked_source(
            &format!(
                "{declaration}
                machine erase(value: i64 in Tagged) -> i64 {{ value as i64 }}"
            ),
            false,
        );
        // Checking permits explicit non-owning erasure. This producer slice
        // transports only vacuous scalar tags, not predicate/route evidence.
        assert!(
            checked
                .facts
                .values
                .scalar_expressions
                .expressions
                .is_empty()
        );
        assert!(checked.facts.values.scalar_computations.roots.is_empty());
    }
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
    let typed = typed_program(source);
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

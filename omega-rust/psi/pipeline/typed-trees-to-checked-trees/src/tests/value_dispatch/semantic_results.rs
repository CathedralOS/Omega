use super::check;

#[test]
fn routed_scalar_match_results_still_require_a_custody_join() {
    let source = "domain i64::Issued established by Issuer::issue;
        trait Issuer { machine issue(value: i64) -> i64 in Issued; }
        machine choose(flag: bool, left: i64 in Issued, right: i64 in Issued) -> i64 in Issued {
            match flag { true -> left, false -> right }
        }";
    let errors = check(source).expect_err("routed provenance is not a plain scalar join");
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("branch custody join")),
        "{errors:?}"
    );
}

#[test]
fn scalar_match_result_casts_still_prove_domain_predicates() {
    let source = "domain i64::Positive requires self > 0;
        machine choose(flag: bool) -> i64 in Positive {
            match flag { true -> 0 as i64 in Positive, false -> 1 as i64 in Positive }
        }";
    let errors =
        check(source).expect_err("the false membership claim cannot become result evidence");
    assert!(
        errors.iter().any(|error| error.message.contains("Positive")
            && (error.message.contains("prove")
                || error.message.contains("predicate")
                || error.message.contains("requires"))),
        "{errors:?}"
    );
}

#[test]
fn indexed_cast_specializations_keep_result_and_expression_domain_ids_in_sync() {
    use typed_trees::expression::ExpressionNode;
    use typed_trees::types::{TypeConstraintNode, TypeReferenceNode};

    let checked = check(
        "domain<T, const U: u64> T::Quantity<U>;
         machine retag<const To: u64>(value: i64) -> i64 in Quantity<To> {
             transition { _ -> (value as i64 in Quantity<To>) }
         }
         data Main {}
         machine Main::run(&mut self) {
             let first: i64 in Quantity<1> = retag(70);
             let second: i64 in Quantity<2> = retag(70);
         }",
    )
    .expect("two closed indexed qualifications");
    let mut cast_domains = Vec::new();
    for (_, expression) in checked.expression_table.iter_expressions() {
        let ExpressionNode::Cast(cast) = expression else {
            continue;
        };
        if cast.semantic_domain.is_empty() {
            continue;
        }
        let TypeReferenceNode::Constrained { constraints, .. } = checked
            .type_reference_table
            .type_reference(cast.result_type)
        else {
            panic!("specialized cast retains qualified result type");
        };
        let [TypeConstraintNode::Domain(domain)] =
            checked.type_reference_table.constraints(*constraints)
        else {
            panic!("cast result carries its exact indexed domain");
        };
        assert_eq!(cast.semantic_domain_symbol, domain.symbol);
        assert_eq!(cast.semantic_domain_id, domain.semantic_id);
        assert!(
            checked
                .type_reference_table
                .contains_type_reference(cast.result_type)
        );
        cast_domains.push(domain.semantic_id);
    }
    assert_eq!(
        cast_domains.len(),
        3,
        "the authored cast and both concrete occurrences survive"
    );
    assert_ne!(
        cast_domains[0], cast_domains[1],
        "distinct const packs cannot share a domain instance"
    );
    let mut result_domains = Vec::new();
    for specialization in &checked.machine_specializations {
        let machine = checked
            .machines()
            .iter()
            .find(|machine| machine.symbol == specialization.instance)
            .expect("specialized machine");
        let state = &checked.machine_states(machine)[0];
        let TypeReferenceNode::Constrained { constraints, .. } = checked
            .type_reference_table
            .type_reference(state.return_type)
        else {
            panic!("qualified specialized return");
        };
        let [TypeConstraintNode::Domain(domain)] =
            checked.type_reference_table.constraints(*constraints)
        else {
            panic!("indexed specialized return");
        };
        assert!(cast_domains.contains(&domain.semantic_id));
        result_domains.push(domain.semantic_id);
    }
    assert_eq!(result_domains.len(), 2);
    assert_ne!(result_domains[0], result_domains[1]);
}

#[test]
fn match_checks_semantic_domains_before_outer_erasure() {
    for (declarations, first, second) in [
        ("domain i64::Km; domain i64::Miles;", "Km", "Miles"),
        (
            "domain<T, const U: u64> T::Quantity<U>;",
            "Quantity<1>",
            "Quantity<2>",
        ),
    ] {
        for (first, second) in [(first, second), (second, first)] {
            let source = format!(
                "{declarations} machine choose(flag: bool, left: i64, right: i64) -> i64 {{ (match flag {{ true -> left as i64 in {first}, false -> right as i64 in {second} }}) as i64 }}"
            );
            let errors =
                check(&source).expect_err("outer erasure cannot repair an incompatible join");
            assert!(
                errors
                    .iter()
                    .any(|error| error.message.contains("match arms produce incompatible")),
                "{source}: {errors:?}"
            );
        }
    }
}

#[test]
fn match_retains_compatible_domains_and_accepts_explicit_arm_erasure() {
    for source in [
        "domain i64::Km; machine choose(flag: bool, left: i64, right: i64) -> i64 in Km { match flag { true -> left as i64 in Km, false -> right as i64 in Km } }",
        "domain i64::Km; domain i64::Distance = i64::Km; machine choose(flag: bool, left: i64, right: i64) -> i64 in Km { match flag { true -> left as i64 in Distance, false -> right as i64 in Km } }",
        "domain<T, const U: u64> T::Quantity<U>; machine choose(flag: bool, left: i64, right: i64) -> i64 in Quantity<1> { match flag { true -> left as i64 in Quantity<1>, false -> right as i64 in Quantity<1> } }",
        "domain i64::Km; domain i64::Miles; machine choose(flag: bool, left: i64, right: i64) -> i64 { match flag { true -> (left as i64 in Km) as i64, false -> (right as i64 in Miles) as i64 } }",
    ] {
        check(source).unwrap_or_else(|errors| panic!("{source}: {errors:?}"));
    }
}

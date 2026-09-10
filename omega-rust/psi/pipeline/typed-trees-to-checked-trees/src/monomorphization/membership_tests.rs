use super::*;

fn typed(source: &str) -> TypedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokens");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("syntax");
    let resolved =
        syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).expect("resolution");
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).expect("typing")
}

#[test]
fn parameter_membership_instances_follow_single_and_multiple_specialization() {
    for second in [
        "",
        "machine other(value: i64 in Coordinate<9>) -> i64 in Coordinate<9> { relay<9>(value) }",
    ] {
        let mut program = typed(&format!(
            "domain<T, const I: u64> T::Coordinate<I>;
             machine relay<const I: u64>(value: i64 in Coordinate<I>) -> i64 in Coordinate<I> {{ value }}
             machine run(value: i64 in Coordinate<7>) -> i64 in Coordinate<7> {{ relay<7>(value) }}
             {second}"
        ));
        monomorphize_generic_machine_value_calls_with_nominal_uses(&mut program, &mut Vec::new())
            .expect("closed specializations");
        refresh_closed_domain_instance_identities(&mut program).expect("refresh memberships");
        let mut instances = Vec::new();
        for specialization in &program.machine_specializations {
            let machine = program
                .machines()
                .iter()
                .find(|machine| machine.symbol == specialization.instance)
                .expect("specialized machine");
            let state = &program.machine_states(machine)[0];
            let parameter = &program.state_parameters(state)[0];
            let TypeReferenceNode::Constrained { constraints, .. } = program
                .type_reference_table
                .type_reference(parameter.type_reference)
            else {
                panic!("qualified parameter");
            };
            let domain = program
                .type_reference_table
                .constraints(*constraints)
                .iter()
                .find_map(|constraint| match constraint {
                    TypeConstraintNode::Domain(domain) => Some(domain),
                    _ => None,
                })
                .expect("domain instance");
            let membership = program
                .signature_contracts
                .span_or_empty(state.contracts)
                .iter()
                .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts))
                .find_map(|fact| match fact {
                    ProofFact::Membership(membership)
                        if membership.domain_symbol == domain.symbol =>
                    {
                        Some(membership)
                    }
                    _ => None,
                })
                .expect("implicit membership requirement");
            let arguments = program
                .type_reference_table
                .type_reference_handles(membership.domain_arguments);
            assert_eq!(arguments.len(), 1);
            assert_eq!(arguments.len(), domain.arguments.len());
            assert_eq!(
                program.normalized_type_identity(arguments[0]),
                program.normalized_type_identity(domain.arguments[0])
            );
            assert_eq!(membership.semantic_domain, domain.semantic_id);
            assert!(membership.semantic_domain.is_valid());
            instances.push(membership.semantic_domain);
        }
        instances.sort_by_key(|identity| identity.0);
        instances.dedup();
        assert_eq!(instances.len(), if second.is_empty() { 1 } else { 2 });
    }
}

#[test]
fn membership_contract_identity_retains_normalized_indices() {
    let identity = |index: &str| {
        let program = typed(&format!(
            "domain<T, const I: u64> T::Coordinate<I>; machine run(value: i64 in Coordinate<{index}>) -> i64 in Coordinate<{index}> {{ value }}"
        ));
        let fact = program
            .proof_facts
            .iter()
            .find_map(|(_, fact)| matches!(fact, ProofFact::Membership(_)).then_some(fact))
            .expect("membership");
        contract_fact_text(&program, fact)
    };
    assert_ne!(identity("7"), identity("9"));
    assert_eq!(identity("7"), identity("(7 + 0)"));
}

#[test]
fn refresh_rejects_missing_index_arguments_instead_of_using_family_identity() {
    let mut program = typed(
        "domain<T, const I: u64> T::Coordinate<I>; machine run(value: i64 in Coordinate<7>) -> i64 in Coordinate<7> { value }",
    );
    let handle = program
        .proof_facts
        .iter()
        .find_map(|(handle, fact)| matches!(fact, ProofFact::Membership(_)).then_some(handle))
        .expect("membership");
    let ProofFact::Membership(membership) = program.proof_facts.get_mut(handle) else {
        panic!("membership");
    };
    membership.domain_arguments = HandleSpan::empty();
    assert!(refresh_closed_domain_instance_identities(&mut program).is_err());
}

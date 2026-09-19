//! Tests for proof surface collection.

use arena::HandleSpan;
use syntax_trees::SyntaxTrees;
use syntax_trees::identifier::Identifier;
use syntax_trees::item::{
    CapabilityContract, CapabilityContractKind, DomainDefinition, Item, Machine,
    OperatorDefinition, State, StateParameterNode,
};
use syntax_trees::types::{TypeConstraintNode, TypeReferenceNode};

use crate::build_proof_surface_report;

#[test]
fn integer_remainder_ranges_contain_truncating_results() {
    use super::obligations::{IntegerRange, integer_binary_range};
    use numerics::bignum::BigInt;
    use typed_trees::expression::BinaryOperator;

    for (minimum, maximum, divisor_minimum, divisor_maximum) in [
        (-7, -7, 2, 2),
        (-7, -7, -2, -2),
        (-7, 7, -3, -2),
        (0, 0, 2, 9),
        (-1, 1, 1, 1),
        (-9, -2, 2, 5),
        (2, 9, -5, -2),
        (0, 9, 2, 5),
    ] {
        let range = integer_binary_range(
            BinaryOperator::Modulo,
            IntegerRange {
                minimum: BigInt::from_i64(minimum),
                maximum: BigInt::from_i64(maximum),
            },
            IntegerRange {
                minimum: BigInt::from_i64(divisor_minimum),
                maximum: BigInt::from_i64(divisor_maximum),
            },
        )
        .expect("nonzero divisor range");
        for dividend in minimum..=maximum {
            for divisor in divisor_minimum..=divisor_maximum {
                let remainder = BigInt::from_i64(dividend % divisor);
                assert!(range.minimum <= remainder && remainder <= range.maximum);
            }
        }
        if minimum >= 0 {
            assert_eq!(range.minimum, BigInt::zero());
        }
        if maximum <= 0 {
            assert_eq!(range.maximum, BigInt::zero());
        }
    }
    for (minimum, maximum) in [(-1, 1), (0, 0), (-2, 0), (0, 2)] {
        assert!(
            integer_binary_range(
                BinaryOperator::Modulo,
                IntegerRange {
                    minimum: BigInt::from_i64(-7),
                    maximum: BigInt::from_i64(7),
                },
                IntegerRange {
                    minimum: BigInt::from_i64(minimum),
                    maximum: BigInt::from_i64(maximum),
                },
            )
            .is_none()
        );
    }
    let magnitude = BigInt::from_u128(u128::MAX).mul(&BigInt::from_i64(2));
    let dividend = magnitude.negate();
    let divisor = magnitude.sub(&BigInt::from_i64(1)).negate();
    let range = integer_binary_range(
        BinaryOperator::Modulo,
        IntegerRange {
            minimum: dividend.clone(),
            maximum: dividend.clone(),
        },
        IntegerRange {
            minimum: divisor.clone(),
            maximum: divisor.clone(),
        },
    )
    .expect("unbounded negative operands");
    let remainder = dividend.div_rem(&divisor).expect("nonzero divisor").1;
    assert!(range.minimum <= remainder && remainder <= range.maximum);
    assert_eq!(range.maximum, BigInt::zero());
}

#[test]
fn collects_domain_surface() {
    let mut syntax_trees = SyntaxTrees::new(Default::default());
    let target_type = syntax_trees
        .type_references
        .insert(TypeReferenceNode::Named(Identifier::generated("String")));

    syntax_trees.push_root_item(Item::Domain(DomainDefinition {
        name: Identifier::generated("NonEmpty"),
        type_parameters: HandleSpan::empty(),
        target_type,
        index_arguments: HandleSpan::empty(),
        is_public: false,
        alias: None,
        authored_routes: Vec::new(),
        classification: None,
        predicate_body: language_semantics::DomainPredicateBody::Bodyless,
        facts: HandleSpan::empty(),
        operators: HandleSpan::empty(),
        semantic_clause_token_count: 3,
    }));

    let report = build_proof_surface_report(&syntax_trees);

    assert_eq!(report.domains.len(), 1);
    let (_, domain) = report.domains.iter().next().expect("domain surface");
    assert_eq!(domain.name, "NonEmpty");
    assert_eq!(domain.target_type, "String");
    assert_eq!(
        domain.predicate_body,
        language_semantics::DomainPredicateBody::Bodyless
    );
    assert_eq!(domain.fact_count, 0);
    assert_eq!(domain.membership_fact_count, 0);
    assert_eq!(domain.semantic_clause_token_count, 3);
}

#[test]
fn collects_operator_contract_surface() {
    let mut syntax_trees = SyntaxTrees::new(Default::default());
    let operator_name = syntax_trees.items.insert_identifier_path_members([
        Identifier::generated("Slice"),
        Identifier::generated("index"),
    ]);
    let requires = syntax_trees
        .items
        .append_capability_contract(CapabilityContract {
            kind: CapabilityContractKind::Requires,
            keyword_source_span: None,
            binding: None,
            facts: HandleSpan::empty(),
            token_count: 3,
        });
    let ensures = syntax_trees
        .items
        .append_capability_contract(CapabilityContract {
            kind: CapabilityContractKind::Ensures,
            keyword_source_span: None,
            binding: None,
            facts: HandleSpan::empty(),
            token_count: 3,
        });
    let return_type = syntax_trees
        .type_references
        .insert_named(Identifier::generated("T"));
    syntax_trees.push_root_item(Item::Operator(OperatorDefinition {
        is_public: false,
        is_boundary: false,
        name: operator_name,
        lifetime_parameters: Vec::new(),
        type_parameters: HandleSpan::empty(),
        parameters: HandleSpan::empty(),
        return_type,
        contracts: HandleSpan::from_parts(
            requires,
            ensures
                .arena_index()
                .checked_sub(requires.arena_index())
                .expect("contracts should be contiguous")
                + 1,
        ),
        spelling: None,
        token_count: 1,
    }));

    let report = build_proof_surface_report(&syntax_trees);

    assert_eq!(report.contracts.len(), 2);
    assert!(
        report
            .contracts
            .iter()
            .all(|(_, contract)| { contract.owner == "operator `Slice::index`" })
    );
}

#[test]
fn collects_bounded_type_sites() {
    let mut syntax_trees = SyntaxTrees::new(Default::default());
    let base_type = syntax_trees
        .type_references
        .insert_named(Identifier::generated("f32"));
    let constraint = syntax_trees
        .type_references
        .append_constraint(TypeConstraintNode::Named(Identifier::generated(
            "speed_range",
        )));
    let parameter_type = syntax_trees
        .type_references
        .insert_constrained(base_type, HandleSpan::from_parts(constraint, 1));
    let parameter = syntax_trees
        .items
        .insert_state_parameter_node(StateParameterNode {
            name: Identifier::generated("speed"),
            type_reference: parameter_type,
            is_const: false,
            is_mutable: false,
            is_self: false,
            relevance: Default::default(),
        });
    let parameter_handle = syntax_trees.items.append_state_parameter_handle(parameter);
    let state = syntax_trees.items.insert_state(&State {
        name: Identifier::generated("entry"),
        parameters: HandleSpan::from_parts(parameter_handle, 1),
        return_type: syntax_trees::types::TypeReferenceHandle::invalid(),
        contracts: HandleSpan::empty(),
        statements: HandleSpan::empty(),
    });
    let state_handle = syntax_trees.items.append_state_handle(state);
    syntax_trees.push_root_item(Item::Machine(Machine {
        name: Identifier::generated("main"),
        generic_data_template: Default::default(),
        where_facts: HandleSpan::empty(),
        attached_data: None,
        spelling: None,
        is_public: false,
        is_top_level_boundary_requirement: false,
        target: None,
        boundary: false,
        bodyless: false,
        lifetime_parameters: Vec::new(),
        type_parameters: HandleSpan::empty(),
        satisfies: HandleSpan::empty(),
        conformance_bounds: Vec::new(),
        terminates_guarantee: false,
        ranking_subjects: HandleSpan::empty(),
        ranking_view: HandleSpan::empty(),
        ranking_view_arguments: HandleSpan::empty(),
        ranking_range: syntax_trees::expression::ExpressionHandle::invalid(),
        service_reach_keyword_source_spans: Vec::new(),
        service_reach_is_installation_bound: false,
        service_reaches: HandleSpan::empty(),
        invokes: HandleSpan::empty(),
        suspends_keyword_source_spans: Vec::new(),
        blocks_keyword_source_spans: Vec::new(),
        suspends: false,
        blocks: false,
        contracts: HandleSpan::empty(),
        states: HandleSpan::from_parts(state_handle, 1),
    }));

    let report = build_proof_surface_report(&syntax_trees);

    assert_eq!(report.bounded_sites.len(), 1);

    let (_, bounded_site) = report
        .bounded_sites
        .iter()
        .next()
        .expect("bounded site should be collected");
    assert_eq!(bounded_site.base_type, "f32");
    assert_eq!(bounded_site.constraints, "[speed_range]");
}

#[test]
fn collects_machine_contract_surface() {
    let mut syntax_trees = SyntaxTrees::new(Default::default());
    let requires = syntax_trees
        .items
        .append_capability_contract(CapabilityContract {
            kind: CapabilityContractKind::Requires,
            keyword_source_span: None,
            binding: None,
            facts: HandleSpan::empty(),
            token_count: 3,
        });
    let ensures = syntax_trees
        .items
        .append_capability_contract(CapabilityContract {
            kind: CapabilityContractKind::Ensures,
            keyword_source_span: None,
            binding: None,
            facts: HandleSpan::empty(),
            token_count: 3,
        });

    syntax_trees.push_root_item(Item::Machine(Machine {
        name: Identifier::generated("distinct_indices"),
        generic_data_template: Default::default(),
        where_facts: HandleSpan::empty(),
        attached_data: None,
        spelling: None,
        is_public: false,
        is_top_level_boundary_requirement: false,
        target: None,
        boundary: false,
        bodyless: false,
        lifetime_parameters: Vec::new(),
        type_parameters: HandleSpan::empty(),
        satisfies: HandleSpan::empty(),
        conformance_bounds: Vec::new(),
        terminates_guarantee: false,
        ranking_subjects: HandleSpan::empty(),
        ranking_view: HandleSpan::empty(),
        ranking_view_arguments: HandleSpan::empty(),
        ranking_range: syntax_trees::expression::ExpressionHandle::invalid(),
        service_reach_keyword_source_spans: Vec::new(),
        service_reach_is_installation_bound: false,
        service_reaches: HandleSpan::empty(),
        invokes: HandleSpan::empty(),
        suspends_keyword_source_spans: Vec::new(),
        blocks_keyword_source_spans: Vec::new(),
        suspends: false,
        blocks: false,
        contracts: HandleSpan::from_parts(
            requires,
            ensures
                .arena_index()
                .checked_sub(requires.arena_index())
                .expect("contracts should be contiguous")
                + 1,
        ),
        states: HandleSpan::empty(),
    }));

    let report = build_proof_surface_report(&syntax_trees);

    assert_eq!(report.contracts.len(), 2);
    let contracts = report
        .contracts
        .iter()
        .map(|(_, contract)| contract.kind)
        .collect::<Vec<_>>();
    assert_eq!(
        contracts,
        vec![
            super::ContractKindSurface::Requires,
            super::ContractKindSurface::Ensures
        ]
    );
}

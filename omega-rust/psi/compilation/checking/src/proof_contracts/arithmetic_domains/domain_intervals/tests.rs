use super::{declared_domain_predicate_bounds, exact_declared_domain_interval};
use numerics::bignum::BigInt;
use typed_trees::{
    TypedTrees,
    types::{DomainConstraint, TypeConstraintNode, TypeReferenceNode},
};

fn fields(predicate: &str) -> (TypedTrees, Vec<DomainConstraint>) {
    let program = crate::front_end::typed_program(&format!(
        "domain<const Bound: u64> u64::AtMost<Bound>(value) requires {predicate};
         data Holder {{ first: u64 in AtMost<256>; second: u64 in AtMost<512>; }}"
    ));
    let holder = program
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "Holder")
        .unwrap();
    let domains = program
        .data_members(holder)
        .iter()
        .map(|member| {
            let typed_trees::data::DataMember::Field(field) = member else {
                panic!("field")
            };
            let TypeReferenceNode::Constrained { constraints, .. } = program
                .type_reference_table
                .type_reference(field.type_reference)
            else {
                panic!("qualified field")
            };
            program
                .type_reference_table
                .constraints(*constraints)
                .iter()
                .find_map(|constraint| {
                    if let TypeConstraintNode::Domain(domain) = constraint {
                        Some(domain.clone())
                    } else {
                        None
                    }
                })
                .expect("indexed qualification")
        })
        .collect();
    (program, domains)
}

#[test]
fn closed_indices_supply_their_own_bounds_without_equating_instances() {
    let (program, domains) = fields("value <= Bound");
    assert_ne!(domains[0].semantic_id, domains[1].semantic_id);
    assert_eq!(
        exact_declared_domain_interval(&program, &domains[0]),
        Some((None, Some(BigInt::from_u64(256))))
    );
    assert_eq!(
        exact_declared_domain_interval(&program, &domains[1]),
        Some((None, Some(BigInt::from_u64(512))))
    );
}

#[test]
fn changing_an_index_without_its_normalized_identity_declines() {
    let (program, domains) = fields("value <= Bound");
    let mut substituted = domains[0].clone();
    substituted.arguments = domains[1].arguments.clone();
    assert!(exact_declared_domain_interval(&program, &substituted).is_none());
}

#[test]
fn an_indexed_bound_does_not_erase_an_additional_predicate() {
    let (program, domains) = fields("value <= Bound && value % 2 == 0");
    assert_eq!(
        declared_domain_predicate_bounds(&program, &domains[0]).map(|(_, maximum)| maximum),
        Some(BigInt::from_u64(256))
    );
    assert!(exact_declared_domain_interval(&program, &domains[0]).is_none());
}

#[test]
fn an_open_index_does_not_acquire_a_static_value() {
    let program = crate::front_end::typed_program(
        "domain<const Bound: u64> u64::AtMost<Bound>(value) requires value <= Bound;
         machine open<const Limit: u64>(value: u64 in AtMost<Limit>) {}",
    );
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "open")
        .unwrap();
    let state = &program.machine_states(machine)[0];
    let parameter = &program.state_parameters(state)[0];
    let TypeReferenceNode::Constrained { constraints, .. } = program
        .type_reference_table
        .type_reference(parameter.type_reference)
    else {
        panic!("qualified parameter")
    };
    let domain = program
        .type_reference_table
        .constraints(*constraints)
        .iter()
        .find_map(|constraint| {
            if let TypeConstraintNode::Domain(domain) = constraint {
                Some(domain)
            } else {
                None
            }
        })
        .unwrap();
    assert!(exact_declared_domain_interval(&program, domain).is_none());
}

use super::TypedTreesSnapshot;
use crate::TypedTrees;
use crate::domain::DomainDefinition;

#[test]
fn snapshots_empty_typed_tree_as_json() {
    let program = TypedTrees::default();
    let snapshot = TypedTreesSnapshot::from_typed_trees(&program);

    assert_eq!(snapshot.roots.data_definitions.len(), 0);
    assert!(snapshot.to_json_pretty().is_ok());
}

#[test]
fn snapshots_normalized_domain_facets() {
    let mut program = TypedTrees::default();
    program.push_domain_definition(DomainDefinition {
        semantic_id: omega_core::semantics::SemanticDomainId(23),
        facets: omega_core::semantics::DomainFacets {
            predicate: false,
            semantic: Some(omega_core::semantics::SemanticDomainId(23)),
        },
        ..Default::default()
    });

    let snapshot = TypedTreesSnapshot::from_typed_trees(&program);
    let [domain] = snapshot.roots.domain_definitions.as_slice() else {
        panic!("one domain snapshot")
    };
    assert_eq!(domain.semantic_id, 23);
    assert!(!domain.facets.predicate);
    assert_eq!(domain.facets.semantic, Some(23));
    assert!(snapshot.to_json_pretty().is_ok());
}

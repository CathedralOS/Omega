use super::{
    MachineSupplySnapshot, TypeConstraintSnapshot, TypedTreesSnapshot, type_constraint_snapshot,
};
use crate::TypedTrees;
use crate::domain::DomainDefinition;
use crate::machine::Machine;
use crate::name::Identifier;
use crate::types::{DomainConstraint, TypeConstraintNode};

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
    assert_eq!(domain.predicate_body, "bodyless");
    assert!(!domain.facets.predicate);
    assert_eq!(domain.facets.semantic, Some(23));
    assert!(snapshot.to_json_pretty().is_ok());
}

#[test]
fn snapshots_normalized_domain_constraint_identity_and_facets() {
    let program = TypedTrees::default();
    let symbol = omega_core::symbols::SymbolHandle::from_arena_index(31);
    let semantic_id = omega_core::semantics::SemanticDomainId(7);
    let snapshot = type_constraint_snapshot(
        &program,
        &TypeConstraintNode::Domain(DomainConstraint {
            name: Identifier::generated("Utf8"),
            symbol,
            semantic_id,
            facets: omega_core::semantics::DomainFacets {
                predicate: true,
                semantic: Some(semantic_id),
            },
        }),
    );

    assert!(matches!(
        snapshot,
        TypeConstraintSnapshot::Domain {
            name,
            symbol: 31,
            semantic_id: 7,
            predicate_facet: true,
            semantic_facet: Some(7),
        } if name == "Utf8"
    ));
}

#[test]
fn snapshots_normalized_machine_supply_including_external_binding_identity() {
    let mut program = TypedTrees::default();
    program.push_machine(Machine {
        name: Identifier::generated("checked"),
        ..Machine::default()
    });
    program.push_machine(Machine {
        name: Identifier::generated("leaf"),
        supply_mode: omega_core::semantics::MachineSupplyMode::ExternalRealization {
            binding: omega_core::semantics::ExternalBindingId(17),
        },
        ..Machine::default()
    });

    let snapshot = TypedTreesSnapshot::from_typed_trees(&program);
    assert_eq!(
        snapshot.roots.machines[0].supply,
        MachineSupplySnapshot::CheckedBody
    );
    assert_eq!(
        snapshot.roots.machines[1].supply,
        MachineSupplySnapshot::ExternalRealization { binding: 17 }
    );
    let json = snapshot.to_json_pretty().expect("snapshot JSON");
    assert!(json.contains("\"kind\": \"external_realization\""));
    assert!(json.contains("\"binding\": 17"));
}

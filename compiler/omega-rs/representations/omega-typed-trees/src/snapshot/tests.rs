use super::{
    MachineSupplySnapshot, TypeConstraintSnapshot, TypedTreesSnapshot, type_constraint_snapshot,
};
use crate::TypedTrees;
use crate::domain::{DomainAliasConstituent, DomainAliasDefinition, DomainDefinition};
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
fn snapshots_normalized_domain_semantic_roles() {
    let mut program = TypedTrees::default();
    let machine = omega_core::symbols::SymbolHandle::from_arena_index(25);
    program.push_domain_definition(DomainDefinition {
        semantic_id: omega_core::semantics::SemanticDomainId(23),
        semantic_roles: omega_core::semantics::DomainSemanticRoles {
            denotation_dimension: Some(omega_core::semantics::SemanticDomainId(23)),
            arithmetic_policy: None,
        },
        establishment_routes: vec![
            omega_core::semantics::DomainEstablishmentRoute::OwnerCheckedMachine { machine },
        ],
        ..Default::default()
    });

    let snapshot = TypedTreesSnapshot::from_typed_trees(&program);
    let [domain] = snapshot.roots.domain_definitions.as_slice() else {
        panic!("one domain snapshot")
    };
    assert_eq!(domain.semantic_id, 23);
    assert_eq!(domain.predicate_body, "bodyless");
    assert_eq!(domain.semantic_roles.denotation_dimension, Some(23));
    assert_eq!(domain.semantic_roles.arithmetic_policy, None);
    assert_eq!(domain.establishment_routes.len(), 1);
    assert_eq!(domain.establishment_routes[0].kind, "owner_checked_machine");
    assert_eq!(
        domain.establishment_routes[0].source_symbol,
        machine.arena_index()
    );
    assert!(snapshot.to_json_pretty().is_ok());
}

#[test]
fn snapshots_transparent_alias_theory_independently_from_facts() {
    let mut program = TypedTrees::default();
    let atom_symbol = omega_core::symbols::SymbolHandle::from_arena_index(41);
    let mut domain = omega_core::arena::HandleSpan::empty();
    program
        .domain_path_members
        .append_to_span(&mut domain, Identifier::generated("Socket"));
    program
        .domain_path_members
        .append_to_span(&mut domain, Identifier::generated("Connected"));
    program.push_domain_definition(DomainDefinition {
        name: Identifier::generated("Socket::Usable"),
        is_public: true,
        alias: Some(DomainAliasDefinition {
            constituents: vec![DomainAliasConstituent {
                domain,
                domain_symbol: atom_symbol,
            }],
        }),
        ..Default::default()
    });

    let snapshot = TypedTreesSnapshot::from_typed_trees(&program);
    let [alias] = snapshot.roots.domain_definitions.as_slice() else {
        panic!("one alias snapshot")
    };
    assert!(alias.is_public);
    assert_eq!(alias.alias.len(), 1);
    assert_eq!(alias.alias[0].domain, ["Socket", "Connected"]);
    assert_eq!(alias.alias[0].domain_symbol, atom_symbol.arena_index());
    assert!(alias.facts.is_empty());
}

#[test]
fn snapshots_normalized_domain_constraint_identity_and_roles() {
    let program = TypedTrees::default();
    let symbol = omega_core::symbols::SymbolHandle::from_arena_index(31);
    let semantic_id = omega_core::semantics::SemanticDomainId(7);
    let boundary_trait = omega_core::symbols::SymbolHandle::from_arena_index(32);
    let requirement = omega_core::symbols::SymbolHandle::from_arena_index(33);
    let snapshot = type_constraint_snapshot(
        &program,
        &TypeConstraintNode::Domain(DomainConstraint {
            name: Identifier::generated("Utf8"),
            symbol,
            semantic_id,
            predicate_body: omega_core::semantics::DomainPredicateBody::Present,
            semantic_roles: omega_core::semantics::DomainSemanticRoles {
                denotation_dimension: Some(semantic_id),
                arithmetic_policy: None,
            },
            establishment_routes: vec![
                omega_core::semantics::DomainEstablishmentRoute::BoundaryRequirement {
                    boundary_trait,
                    requirement,
                },
            ],
        }),
    );

    assert!(matches!(
        snapshot,
        TypeConstraintSnapshot::Domain {
            name,
            symbol: 31,
            semantic_id: 7,
            predicate_body: "present",
            semantic_roles: super::DomainSemanticRolesSnapshot {
                denotation_dimension: Some(7),
                arithmetic_policy: None,
            },
            establishment_routes,
        } if name == "Utf8"
            && establishment_routes == vec![super::DomainEstablishmentRouteSnapshot {
                kind: "boundary_requirement",
                source_symbol: 32,
                requirement_symbol: Some(33),
            }]
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

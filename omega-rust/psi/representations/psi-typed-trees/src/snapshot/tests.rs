use super::{
    DomainConstraintSubjectSnapshot, MachineSupplySnapshot, TypeConstraintSnapshot,
    TypedTreesSnapshot, type_constraint_snapshot,
};
use crate::TypedTrees;
use crate::data::DataDefinition;
use crate::domain::{DomainAliasConstituent, DomainAliasDefinition, DomainDefinition};
use crate::machine::Machine;
use crate::name::Identifier;
use crate::types::{
    DomainConstraint, DomainConstraintSubject, OmegaLayoutGrammar, TypeConstraintNode,
    TypeReferenceNode,
};

#[test]
fn snapshots_empty_typed_tree_as_json() {
    let program = TypedTrees::default();
    let snapshot = TypedTreesSnapshot::from_typed_trees(&program);

    assert_eq!(snapshot.roots.data_definitions.len(), 0);
    assert!(snapshot.to_json_pretty().is_ok());
}

#[test]
fn snapshots_public_data_visibility() {
    let mut program = TypedTrees::default();
    program.push_data_definition(DataDefinition {
        name: Identifier::generated("PublicRecord"),
        is_public: true,
        ..DataDefinition::default()
    });

    let snapshot = TypedTreesSnapshot::from_typed_trees(&program);
    assert!(snapshot.roots.data_definitions[0].is_public);
}

#[test]
fn snapshots_normalized_domain_semantic_roles() {
    let mut program = TypedTrees::default();
    let trait_definition = psi_symbols::SymbolHandle::from_arena_index(25);
    let requirement = psi_symbols::SymbolHandle::from_arena_index(26);
    program.push_domain_definition(DomainDefinition {
        semantic_id: psi_language_semantics::SemanticDomainId(23),
        semantic_roles: psi_language_semantics::DomainSemanticRoles {
            denotation_dimension: Some(psi_language_semantics::SemanticDomainId(23)),
            arithmetic_policy: None,
        },
        establishment_routes: vec![
            psi_language_semantics::DomainEstablishmentRoute::CheckedRequirement {
                trait_definition,
                requirement,
            },
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
    assert_eq!(domain.establishment_routes[0].kind, "checked_requirement");
    assert_eq!(
        domain.establishment_routes[0].source_symbol,
        trait_definition.arena_index()
    );
    assert_eq!(
        domain.establishment_routes[0].requirement_symbol,
        Some(requirement.arena_index())
    );
    assert!(snapshot.to_json_pretty().is_ok());
}

#[test]
fn snapshots_transparent_alias_theory_independently_from_facts() {
    let mut program = TypedTrees::default();
    let atom_symbol = psi_symbols::SymbolHandle::from_arena_index(41);
    let mut domain = psi_arena::HandleSpan::empty();
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
    let symbol = psi_symbols::SymbolHandle::from_arena_index(31);
    let semantic_id = psi_language_semantics::SemanticDomainId(7);
    let boundary_trait = psi_symbols::SymbolHandle::from_arena_index(32);
    let requirement = psi_symbols::SymbolHandle::from_arena_index(33);
    let snapshot = type_constraint_snapshot(
        &program,
        &TypeConstraintNode::Domain(DomainConstraint {
            name: Identifier::generated("Utf8"),
            arguments: Vec::new(),
            subject: DomainConstraintSubject::Declared,
            symbol,
            semantic_id,
            classification: None,
            predicate_body: psi_language_semantics::DomainPredicateBody::Present,
            semantic_roles: psi_language_semantics::DomainSemanticRoles {
                denotation_dimension: Some(semantic_id),
                arithmetic_policy: None,
            },
            establishment_routes: vec![
                psi_language_semantics::DomainEstablishmentRoute::BoundaryRequirement {
                    boundary_trait,
                    requirement,
                },
            ],
            authored_selection: None,
        }),
    );

    assert!(matches!(
        snapshot,
        TypeConstraintSnapshot::Domain {
            name,
            subject: DomainConstraintSubjectSnapshot::Declared,
            arguments,
            symbol: 31,
            semantic_id: 7,
            classification: None,
            predicate_body: "present",
            semantic_roles: super::DomainSemanticRolesSnapshot {
                denotation_dimension: Some(7),
                arithmetic_policy: None,
            },
            establishment_routes,
        } if name == "Utf8"
            && arguments.is_empty()
            && establishment_routes == vec![super::DomainEstablishmentRouteSnapshot {
                kind: "boundary_requirement",
                source_symbol: 32,
                requirement_symbol: Some(33),
            }]
    ));
}

#[test]
fn snapshots_closed_compiler_domain_subject_and_structural_schema() {
    let mut program = TypedTrees::default();
    let schema_symbol = psi_symbols::SymbolHandle::from_arena_index(41);
    let schema = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: schema_symbol,
            name: Identifier::generated("Save"),
        });
    let snapshot = type_constraint_snapshot(
        &program,
        &TypeConstraintNode::Domain(DomainConstraint {
            name: Identifier::generated("diagnostic layout name"),
            arguments: vec![schema],
            subject: DomainConstraintSubject::OmegaLayout {
                grammar: OmegaLayoutGrammar::Derived,
            },
            ..DomainConstraint::default()
        }),
    );

    assert!(matches!(
        snapshot,
        TypeConstraintSnapshot::Domain {
            name,
            subject: DomainConstraintSubjectSnapshot::OmegaLayout { grammar: "derived" },
            arguments,
            ..
        } if name == "diagnostic layout name"
            && matches!(arguments.as_slice(), [super::TypeReferenceSnapshot::Named { name }] if name == "Save")
    ));
}

#[test]
fn snapshots_normalized_machine_supply_including_external_binding_identity() {
    let mut program = TypedTrees::default();
    let binding = program
        .external_bindings
        .intern(psi_language_semantics::ExternalBindingIdentity::CompilerIntrinsic);
    program.push_machine(Machine {
        name: Identifier::generated("checked"),
        is_public: true,
        ..Machine::default()
    });
    program.push_machine(Machine {
        name: Identifier::generated("leaf"),
        supply_mode: psi_language_semantics::MachineSupplyMode::ExternalRealization {
            binding: Some(binding),
            mechanism: Some(psi_language_semantics::ExternalBindingMechanism::CompilerIntrinsic),
        },
        body_is_present: false,
        ..Machine::default()
    });
    program.push_machine(Machine {
        name: Identifier::generated("pending_leaf"),
        supply_mode: psi_language_semantics::MachineSupplyMode::ExternalRealization {
            binding: None,
            mechanism: None,
        },
        body_is_present: false,
        ..Machine::default()
    });

    let snapshot = TypedTreesSnapshot::from_typed_trees(&program);
    assert!(snapshot.roots.machines[0].is_public);
    assert!(snapshot.roots.machines[0].body_is_present);
    assert_eq!(
        snapshot.roots.machines[0].supply,
        MachineSupplySnapshot::CheckedBody
    );
    assert_eq!(
        snapshot.roots.machines[1].supply,
        MachineSupplySnapshot::ExternalRealization {
            binding: Some(1),
            mechanism: Some("compiler_intrinsic")
        }
    );
    assert!(!snapshot.roots.machines[1].body_is_present);
    assert_eq!(
        snapshot.roots.machines[2].supply,
        MachineSupplySnapshot::ExternalRealization {
            binding: None,
            mechanism: None,
        }
    );
    assert_eq!(snapshot.external_bindings.len(), 1);
    assert_eq!(snapshot.external_bindings[0].identity, 1);
    assert_eq!(
        snapshot.external_bindings[0].binding,
        super::ExternalBindingValueSnapshot::CompilerIntrinsic
    );
    let json = snapshot.to_json_pretty().expect("snapshot JSON");
    assert!(json.contains("\"kind\": \"external_realization\""));
    assert!(json.contains("\"binding\": 1"));
    assert!(json.contains("\"kind\": \"compiler_intrinsic\""));
}

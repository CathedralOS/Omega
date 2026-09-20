use super::{
    DomainConstraintSubjectSnapshot, MachineSupplySnapshot, TypeConstraintSnapshot,
    TypedTreesSnapshot,
};
use crate::TypedTrees;
use crate::data::DataDefinition;
use crate::domain::{DomainAliasConstituent, DomainAliasDefinition, DomainDefinition};
use crate::machine::Machine;
use crate::name::Identifier;
use crate::typed_trees::inspection::snapshot::type_snapshots::type_constraint_snapshot;
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
fn snapshots_measure_root_and_full_table_census() {
    let mut program = TypedTrees::default();
    program.push_const_declaration(crate::constant::ConstDeclaration {
        is_public: true,
        ..crate::constant::ConstDeclaration::default()
    });
    let mut measure = crate::measure::MeasureDefinition {
        symbol: symbols::SymbolHandle::from_arena_index(51),
        lexicographic: true,
        ..crate::measure::MeasureDefinition::default()
    };
    program.push_measure_path_member(&mut measure, Identifier::generated("Card"));
    program.push_measure_path_member(&mut measure, Identifier::generated("PowerOrder"));
    program.push_measure(measure);
    let mut definition = crate::mathematical::MathematicalDefinition {
        name: Identifier::generated("theorem"),
        is_public: true,
        ..crate::mathematical::MathematicalDefinition::default()
    };
    let result = program.insert_mathematical_type(crate::mathematical::MathematicalType::default());
    definition.result = result;
    program.push_mathematical_parameter(
        &mut definition,
        crate::mathematical::MathematicalParameter {
            name: Identifier::generated("x"),
            ty: result,
            ..crate::mathematical::MathematicalParameter::default()
        },
    );
    program.push_mathematical_definition(definition);

    let snapshot = TypedTreesSnapshot::from_typed_trees(&program);

    let [measure] = snapshot.roots.measures.as_slice() else {
        panic!("one measure snapshot")
    };
    assert!(measure.has_symbol);
    assert_eq!(measure.name, ["Card", "PowerOrder"]);
    assert!(measure.lexicographic);
    assert!(measure.parameter.is_none());
    assert!(measure.body.is_empty());
    assert_eq!(snapshot.tables.authored_declaration_selection_count, 0);
    assert_eq!(snapshot.tables.const_declaration_count, 1);
    assert_eq!(snapshot.tables.measure_count, 1);
    assert_eq!(snapshot.tables.measure_path_member_count, 2);
    assert_eq!(snapshot.tables.mathematical_definition_count, 1);
    assert_eq!(snapshot.tables.mathematical_parameter_count, 1);
    assert_eq!(snapshot.tables.mathematical_type_count, 1);
    assert!(snapshot.to_json_pretty().is_ok());
}

#[test]
fn snapshots_normalized_domain_semantic_roles() {
    let mut program = TypedTrees::default();
    let trait_definition = symbols::SymbolHandle::from_arena_index(25);
    let requirement = symbols::SymbolHandle::from_arena_index(26);
    program.push_domain_definition(DomainDefinition {
        semantic_id: language_semantics::SemanticDomainId(23),
        semantic_roles: language_semantics::DomainSemanticRoles {
            denotation_dimension: Some(language_semantics::SemanticDomainId(23)),
            arithmetic_policy: None,
        },
        establishment_routes: vec![
            language_semantics::DomainEstablishmentRoute::CheckedRequirement {
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
    let atom_symbol = symbols::SymbolHandle::from_arena_index(41);
    let mut domain = arena::HandleSpan::empty();
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
    let symbol = symbols::SymbolHandle::from_arena_index(31);
    let semantic_id = language_semantics::SemanticDomainId(7);
    let boundary_trait = symbols::SymbolHandle::from_arena_index(32);
    let requirement = symbols::SymbolHandle::from_arena_index(33);
    let snapshot = type_constraint_snapshot(
        &program,
        &TypeConstraintNode::Domain(DomainConstraint {
            name: Identifier::generated("Utf8"),
            arguments: Vec::new(),
            subject: DomainConstraintSubject::Declared,
            symbol,
            semantic_id,
            classification: None,
            predicate_body: language_semantics::DomainPredicateBody::Present,
            semantic_roles: language_semantics::DomainSemanticRoles {
                denotation_dimension: Some(semantic_id),
                arithmetic_policy: None,
            },
            establishment_routes: vec![
                language_semantics::DomainEstablishmentRoute::BoundaryRequirement {
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
    let schema_symbol = symbols::SymbolHandle::from_arena_index(41);
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
        .intern(language_semantics::ExternalBindingIdentity::CompilerIntrinsic);
    program.push_machine(Machine {
        name: Identifier::generated("checked"),
        is_public: true,
        spelling: Some(language_core::operator_spelling::OperatorSpelling::Multiply),
        ..Machine::default()
    });
    program.push_machine(Machine {
        name: Identifier::generated("leaf"),
        supply_mode: language_semantics::MachineSupplyMode::ExternalRealization {
            binding: Some(binding),
            mechanism: Some(language_semantics::ExternalBindingMechanism::CompilerIntrinsic),
        },
        body_is_present: false,
        ..Machine::default()
    });
    program.push_machine(Machine {
        name: Identifier::generated("pending_leaf"),
        supply_mode: language_semantics::MachineSupplyMode::ExternalRealization {
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
    assert_eq!(snapshot.roots.machines[0].spelling, Some("*"));
    assert_eq!(snapshot.roots.machines[1].spelling, None);
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

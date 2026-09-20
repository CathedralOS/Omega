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
fn snapshots_authored_declaration_selection_rows() {
    use language_semantics::declaration_selection::{
        AuthoredDeclarationSelectionExposure, AuthoredDeclarationSelectionKind,
    };

    let mut program = TypedTrees::default();
    let mut selections =
        language_semantics::declaration_selection::AuthoredDeclarationSelections::default();
    selections
        .record_late_bound(
            source::SourceSpan::new(source::SourceId(3), source::Span::new(20, 22)),
            AuthoredDeclarationSelectionExposure::PrivateImplementation,
            AuthoredDeclarationSelectionKind::MemberAccess,
            language_semantics::declaration_selection::AuthoredDeclarationSelectionLateBinding::CheckedMember,
        )
        .expect("late-bound selection records");
    program.retain_authored_declaration_selections(selections);
    let span = source::SourceSpan::new(source::SourceId(3), source::Span::new(10, 14));
    let occurrence = program
        .record_resolved_authored_declaration_selection_once(
            span,
            AuthoredDeclarationSelectionExposure::PublicInterface,
            AuthoredDeclarationSelectionKind::Call,
            symbols::SymbolHandle::from_arena_index(51),
        )
        .expect("resolved selection records");

    let snapshot = TypedTreesSnapshot::from_typed_trees(&program);

    let [late_bound, resolved] = snapshot.authored_declaration_selections.as_slice() else {
        panic!("two selection snapshots")
    };
    assert_eq!(resolved.occurrence_id, occurrence.ordinal());
    assert_eq!(resolved.source_id, 3);
    assert_eq!(resolved.source_start, 10);
    assert_eq!(resolved.source_end, 14);
    assert_eq!(resolved.exposure, "public_interface");
    assert_eq!(resolved.kind, "call");
    assert_eq!(resolved.compiler_partition, None);
    assert_eq!(
        resolved.target,
        super::AuthoredDeclarationSelectionTargetSnapshot::Resolved {
            selected_symbol: 51
        }
    );
    assert_eq!(late_bound.kind, "member_access");
    assert_eq!(
        late_bound.target,
        super::AuthoredDeclarationSelectionTargetSnapshot::LateBound {
            binding: "checked_member"
        }
    );
    assert_eq!(snapshot.tables.authored_declaration_selection_count, 2);
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
fn snapshots_program_level_custody_and_full_state_census() {
    let mut program = TypedTrees::default();
    let boundary = symbols::SymbolHandle::from_arena_index(61);
    let requirement = symbols::SymbolHandle::from_arena_index(62);
    program.service_reaches.intern(boundary, "Console");
    program
        .service_reaches
        .set_parents(language_semantics::ServiceReachId(1), Vec::new());
    program
        .authored_service_reach_rows
        .push(crate::signature::AuthoredServiceReachRow {
            owner: boundary,
            keyword_source_spans: vec![source::SourceSpan::new(
                source::SourceId(4),
                source::Span::new(1, 7),
            )],
            targets: vec![crate::signature::AuthoredServiceReachTarget {
                service: boundary,
                source_span: source::SourceSpan::new(source::SourceId(4), source::Span::new(9, 16)),
            }],
            installation_bound: true,
        });
    program
        .fused_service_erasures
        .push(crate::typed_trees::FusedServiceErasureAuthorization {
            requirement,
            provider_plan_digest: [9u8; 32],
        });
    program
        .boundary_calling_plans
        .push(crate::typed_trees::BoundaryCallingPlanIdentity {
            boundary_trait: boundary,
            boundary_arguments: Vec::new(),
            requirement_machine: requirement,
            report_fingerprint: 0xdead_beef,
            commitment: crate::typed_trees::BoundaryCallingPlanCommitment::from_digest([7u8; 32]),
        });
    program
        .machine_specializations
        .push(crate::typed_trees::MachineSpecialization {
            template: boundary,
            instance: requirement,
            type_arguments: vec!["u8".to_owned()],
            const_argument_identities: vec!["3".to_owned()],
            normalized_template_identity: "Main::Clock".to_owned(),
            template_contract_commitment:
                crate::typed_trees::MachineTemplateCommitment::from_digest([6u8; 32]),
            report_fingerprint: 42,
            commitment: crate::typed_trees::MachineSpecializationCommitment::from_digest([5u8; 32]),
            ..Default::default()
        });
    let subject = program
        .expression_table
        .insert(crate::expression::ExpressionNode::Boolean(true));
    program
        .ranking_expression_custody
        .push(crate::ranking::RankingExpressionCustody {
            machine: boundary,
            subjects: vec![subject],
            ..Default::default()
        });

    let snapshot = TypedTreesSnapshot::from_typed_trees(&program);

    let [reach] = snapshot.service_reaches.as_slice() else {
        panic!("one service reach")
    };
    assert_eq!(reach.id, 1);
    assert_eq!(reach.symbol, 61);
    assert_eq!(reach.name, "Console");
    assert!(reach.parents.is_empty());
    let [row] = snapshot.authored_service_reach_rows.as_slice() else {
        panic!("one authored reach row")
    };
    assert_eq!(row.owner, 61);
    assert_eq!(row.keyword_spans[0].source_id, 4);
    assert_eq!(row.keyword_spans[0].start, 1);
    assert_eq!(row.targets[0].service, 61);
    assert_eq!(row.targets[0].span.start, 9);
    assert_eq!(row.targets[0].span.end, 16);
    assert!(row.installation_bound);
    let [plan] = snapshot.boundary_calling_plans.as_slice() else {
        panic!("one boundary calling plan")
    };
    assert_eq!(plan.boundary_trait, 61);
    assert_eq!(plan.requirement_machine, 62);
    assert_eq!(plan.report_fingerprint, 0xdead_beef);
    assert_eq!(plan.commitment.len(), 64);
    let [erasure] = snapshot.fused_service_erasures.as_slice() else {
        panic!("one fused service erasure")
    };
    assert_eq!(erasure.requirement, 62);
    assert_eq!(erasure.provider_plan_digest.len(), 64);
    let [specialization] = snapshot.machine_specializations.as_slice() else {
        panic!("one machine specialization")
    };
    assert_eq!(specialization.template, 61);
    assert_eq!(specialization.instance, 62);
    assert_eq!(specialization.type_arguments, ["u8"]);
    assert_eq!(specialization.const_argument_identities, ["3"]);
    assert_eq!(specialization.normalized_template_identity, "Main::Clock");
    assert_eq!(specialization.report_fingerprint, 42);
    assert_eq!(specialization.commitment.len(), 64);
    let [custody] = snapshot.ranking_expression_custody.as_slice() else {
        panic!("one ranking expression custody")
    };
    assert_eq!(custody.machine, 61);
    assert_eq!(custody.subjects, [1]);
    assert!(custody.view_arguments.is_empty());
    assert!(custody.rank_range.is_none());
    assert_eq!(snapshot.tables.service_reach_definition_count, 1);
    assert_eq!(snapshot.tables.authored_service_reach_row_count, 1);
    assert_eq!(snapshot.tables.machine_specialization_count, 1);
    assert_eq!(snapshot.tables.boundary_calling_plan_count, 1);
    assert_eq!(snapshot.tables.fused_service_erasure_count, 1);
    assert_eq!(snapshot.tables.ranking_expression_custody_count, 1);
    assert_eq!(snapshot.tables.expression_count, 1);
    assert_eq!(snapshot.tables.pending_const_range_endpoint_count, 0);
    assert!(snapshot.to_json_pretty().is_ok());
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

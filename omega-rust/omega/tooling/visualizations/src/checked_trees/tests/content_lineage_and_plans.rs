use super::*;

fn lineage_child_plan(parent: &ContentConservationPlan) -> ContentConservationPlan {
    fn drift_first_subject(term: &ContentConservationTerm) -> ContentConservationTerm {
        match term {
            ContentConservationTerm::Projection {
                domain,
                semantic_domain,
                projection_machine,
                projection_report_fingerprint,
                subject,
            } => {
                let mut subject = subject.clone();
                subject
                    .segments
                    .push(language_semantics::content::ContentPlaceSegment::FixedIndex(7));
                ContentConservationTerm::Projection {
                    domain: *domain,
                    semantic_domain: *semantic_domain,
                    projection_machine: *projection_machine,
                    projection_report_fingerprint: *projection_report_fingerprint,
                    subject,
                }
            }
            ContentConservationTerm::Separate(terms) => {
                let mut terms = terms.clone();
                terms[0] = drift_first_subject(&terms[0]);
                ContentConservationTerm::Separate(terms)
            }
        }
    }

    let equation = ContentConservationEquation::new(
        drift_first_subject(parent.equation.left()),
        parent.equation.right().clone(),
    );
    ContentConservationPlan {
        owner_kind: parent.owner_kind,
        owner: parent.owner,
        callable: parent.callable,
        algebra: parent.algebra.clone(),
        report_fingerprint: conservation_report_fingerprint(&parent.algebra, &equation),
        equation,
    }
}

fn push_content_partition_lineage_child(program: &mut CheckedTrees, source_derivation_depth: u32) {
    let parent = program.facts.qualifications.content.partition_compositions[0].clone();
    let mut child = parent.clone();
    child.source_callable = parent.plan.callable;
    child.source_report_fingerprint = parent.plan.report_fingerprint;
    child.source_plan = parent.plan.clone();
    child.source_derivation_depth = source_derivation_depth;
    child.plan = lineage_child_plan(&parent.plan);
    program
        .facts
        .qualifications
        .content
        .partition_compositions
        .push(child);
}

#[test]
fn content_partition_lineage_manifest_accepts_exact_authored_anchor() {
    let program = content_partition_input_validation_fixture();
    validate_content_partition_lineage(&program);
    assert!(claim_outcome_manifest_json(&program).contains("\"source_derivation_depth\": 0"));
}

#[test]
fn content_partition_lineage_manifest_accepts_exact_one_hop_parent() {
    let mut program = content_partition_input_validation_fixture();
    push_content_partition_lineage_child(&mut program, 1);
    validate_content_partition_lineage(&program);
}

#[test]
#[should_panic(expected = "depth-zero source must match one exact authored plan")]
fn content_partition_lineage_manifest_rejects_missing_authored_anchor() {
    let mut program = content_partition_input_validation_fixture();
    program
        .facts
        .qualifications
        .content
        .conservation_plans
        .clear();
    validate_content_partition_lineage(&program);
}

#[test]
#[should_panic(expected = "depth-zero source must match one exact authored plan")]
fn content_partition_lineage_manifest_rejects_ambiguous_authored_anchor() {
    let mut program = content_partition_input_validation_fixture();
    let duplicate = program.facts.qualifications.content.conservation_plans[0].clone();
    program
        .facts
        .qualifications
        .content
        .conservation_plans
        .push(duplicate);
    validate_content_partition_lineage(&program);
}

#[test]
#[should_panic(expected = "must match one distinct exact parent row")]
fn content_partition_lineage_manifest_rejects_missing_parent() {
    let mut program = content_partition_input_validation_fixture();
    program.facts.qualifications.content.partition_compositions[0].source_derivation_depth = 1;
    validate_content_partition_lineage(&program);
}

#[test]
#[should_panic(expected = "must match exactly one distinct parent row")]
fn content_partition_lineage_manifest_rejects_ambiguous_parent() {
    let mut program = content_partition_input_validation_fixture();
    push_content_partition_lineage_child(&mut program, 1);
    let duplicate_parent = program.facts.qualifications.content.partition_compositions[0].clone();
    program
        .facts
        .qualifications
        .content
        .partition_compositions
        .push(duplicate_parent);
    validate_content_partition_lineage(&program);
}

#[test]
#[should_panic(expected = "must exactly increment its parent")]
fn content_partition_lineage_manifest_rejects_depth_gap() {
    let mut program = content_partition_input_validation_fixture();
    push_content_partition_lineage_child(&mut program, 2);
    validate_content_partition_lineage(&program);
}

#[test]
#[should_panic(expected = "source derivation depth must not overflow")]
fn content_partition_lineage_manifest_rejects_depth_overflow() {
    let mut program = content_partition_input_validation_fixture();
    push_content_partition_lineage_child(&mut program, u32::MAX);
    let parent = program.facts.qualifications.content.partition_compositions[0].clone();
    let child = program.facts.qualifications.content.partition_compositions[1].clone();
    let mut overflow_parent = parent;
    overflow_parent.source_derivation_depth = u32::MAX;
    program.facts.qualifications.content.partition_compositions = vec![child, overflow_parent];
    validate_content_partition_lineage(&program);
}

#[test]
#[should_panic(expected = "must match one distinct exact parent row")]
fn content_partition_lineage_manifest_rejects_self_reference() {
    let mut program = content_partition_input_validation_fixture();
    let row = &mut program.facts.qualifications.content.partition_compositions[0];
    row.source_plan = row.plan.clone();
    row.source_callable = row.plan.callable;
    row.source_report_fingerprint = row.plan.report_fingerprint;
    row.source_derivation_depth = 1;
    validate_content_partition_lineage(&program);
}

#[test]
#[should_panic(expected = "must exactly increment its parent")]
fn content_partition_lineage_manifest_rejects_two_row_cycle() {
    let mut program = content_partition_input_validation_fixture();
    push_content_partition_lineage_child(&mut program, 1);
    let child_plan = program.facts.qualifications.content.partition_compositions[1]
        .plan
        .clone();
    let base = &mut program.facts.qualifications.content.partition_compositions[0];
    base.source_plan = child_plan;
    base.source_callable = base.source_plan.callable;
    base.source_report_fingerprint = base.source_plan.report_fingerprint;
    base.source_derivation_depth = 2;
    validate_content_partition_lineage(&program);
}

#[test]
#[should_panic(expected = "depth-zero source must match one exact authored plan")]
fn content_partition_lineage_manifest_rejects_partial_fingerprint_match() {
    let mut program = content_partition_input_validation_fixture();
    let row = &mut program.facts.qualifications.content.partition_compositions[0];
    row.source_plan.owner = SymbolHandle::from_arena_index(999);
    validate_content_partition_lineage(&program);
}

fn content_projection_validation_fixture() -> CheckedTrees {
    let domain_symbol = SymbolHandle::from_arena_index(90);
    let carrier_symbol = SymbolHandle::from_arena_index(91);
    let machine_symbol = SymbolHandle::from_arena_index(92);
    let mut program = CheckedTrees::default();
    let carrier = program
        .typed
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: carrier_symbol,
            name: Identifier::generated("Resource"),
        });
    let semantic_domain = program.typed.semantic_domains.intern("Resource::Counted");
    program.typed.push_domain_definition(DomainDefinition {
        symbol: domain_symbol,
        name: Identifier::generated("Resource::Counted"),
        target_type: carrier,
        semantic_id: semantic_domain,
        ..Default::default()
    });
    let mut projection_machine = Machine {
        symbol: machine_symbol,
        name: Identifier::generated("Resource::content"),
        ..Default::default()
    };
    program.typed.push_machine_state(
        &mut projection_machine,
        State {
            symbol: SymbolHandle::from_arena_index(99),
            name: Identifier::generated("entry"),
            ..Default::default()
        },
    );
    program.typed.push_machine(projection_machine);
    let algebra = ContentAlgebraIdentity::CountedQuantity {
        unit: "named(name(Unit))".to_owned(),
    };
    let expression = ContentProjectionExpression::CountedQuantity {
        magnitude: ContentScalarExpression::Natural("1".to_owned()),
    };
    let report_fingerprint = projection_report_fingerprint(&algebra, &expression);
    program
        .facts
        .qualifications
        .content
        .plans
        .push(ContentProjectionPlan {
            domain: domain_symbol,
            semantic_domain,
            carrier_identity: program
                .typed
                .normalized_type_identity(carrier)
                .into_string(),
            machine: machine_symbol,
            algebra,
            expression,
            report_fingerprint,
        });
    program
}

fn content_conservation_plan(
    projection: &ContentProjectionPlan,
    owner_kind: ContentConservationOwnerKind,
    owner: SymbolHandle,
    callable: SymbolHandle,
) -> ContentConservationPlan {
    let left = ContentConservationTerm::Projection {
        domain: projection.domain,
        semantic_domain: projection.semantic_domain,
        projection_machine: projection.machine,
        projection_report_fingerprint: projection.report_fingerprint,
        subject: ContentStructuralPlace {
            version: ContentPlaceVersion::Entry,
            root: ContentPlaceRoot::Parameter {
                position: 0,
                symbol: SymbolHandle::invalid(),
                name: "resource".to_owned(),
                is_self: false,
            },
            segments: Vec::new(),
        },
    };
    let right = ContentConservationTerm::Projection {
        domain: projection.domain,
        semantic_domain: projection.semantic_domain,
        projection_machine: projection.machine,
        projection_report_fingerprint: projection.report_fingerprint,
        subject: ContentStructuralPlace {
            version: ContentPlaceVersion::Current,
            root: ContentPlaceRoot::Result,
            segments: Vec::new(),
        },
    };
    let algebra = projection.algebra.clone();
    let equation = ContentConservationEquation::new(left, right);
    let report_fingerprint = conservation_report_fingerprint(&algebra, &equation);
    ContentConservationPlan {
        owner_kind,
        owner,
        callable,
        algebra,
        equation,
        report_fingerprint,
    }
}

fn content_conservation_validation_fixture() -> (
    CheckedTrees,
    SymbolHandle,
    SymbolHandle,
    SymbolHandle,
    SymbolHandle,
) {
    let machine_symbol = SymbolHandle::from_arena_index(93);
    let state_symbol = SymbolHandle::from_arena_index(94);
    let other_machine_symbol = SymbolHandle::from_arena_index(95);
    let other_state_symbol = SymbolHandle::from_arena_index(96);
    let trait_symbol = SymbolHandle::from_arena_index(97);
    let requirement_symbol = SymbolHandle::from_arena_index(98);
    let mut program = content_projection_validation_fixture();
    for (machine, state, machine_name, state_name) in [
        (
            machine_symbol,
            state_symbol,
            "Resource::transfer",
            "transfer",
        ),
        (
            other_machine_symbol,
            other_state_symbol,
            "OtherResource::transfer",
            "transfer",
        ),
    ] {
        let mut definition = Machine {
            symbol: machine,
            name: Identifier::generated(machine_name),
            ..Default::default()
        };
        program.typed.push_machine_state(
            &mut definition,
            State {
                symbol: state,
                name: Identifier::generated(state_name),
                ..Default::default()
            },
        );
        program.typed.push_machine(definition);
    }
    let mut trait_definition = TraitDefinition {
        symbol: trait_symbol,
        name: Identifier::generated("ResourceContract"),
        ..Default::default()
    };
    program.typed.push_trait_machine_signature(
        &mut trait_definition,
        StateSignature {
            symbol: requirement_symbol,
            name: Identifier::generated("transfer"),
            ..Default::default()
        },
    );
    program.typed.push_trait_definition(trait_definition);
    let projection = program.facts.qualifications.content.plans[0].clone();
    program
        .facts
        .qualifications
        .content
        .conservation_plans
        .extend([
            content_conservation_plan(
                &projection,
                ContentConservationOwnerKind::Machine,
                machine_symbol,
                state_symbol,
            ),
            content_conservation_plan(
                &projection,
                ContentConservationOwnerKind::TraitRequirement,
                trait_symbol,
                requirement_symbol,
            ),
        ]);
    (
        program,
        machine_symbol,
        state_symbol,
        other_machine_symbol,
        other_state_symbol,
    )
}

fn push_content_partition_row(
    program: &mut CheckedTrees,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    source_plan: ContentConservationPlan,
    plan: ContentConservationPlan,
) {
    program
        .facts
        .qualifications
        .content
        .partition_compositions
        .push(ContentPartitionCompositionFact {
            machine_symbol,
            state_symbol,
            source_callable: source_plan.callable,
            source_report_fingerprint: source_plan.report_fingerprint,
            source_derivation_depth: 0,
            source_plan,
            statement_index: 0,
            call_ordinal: 0,
            input_claim_identities: Vec::new(),
            input_claim_bindings: Vec::new(),
            result_rewrites: Vec::new(),
            substitutions: Vec::new(),
            plan,
        });
}

#[test]
fn content_conservation_manifest_accepts_exact_machine_and_trait_custody() {
    let (program, ..) = content_conservation_validation_fixture();
    let projections = validated_content_projection_plans(&program);

    for plan in &program.facts.qualifications.content.conservation_plans {
        validate_content_conservation_plan(&program, &projections, plan);
    }
    let json = claim_outcome_manifest_json(&program);
    assert_eq!(json.matches("\"owner_kind\": \"machine\"").count(), 1);
    assert_eq!(
        json.matches("\"owner_kind\": \"trait_requirement\"")
            .count(),
        1
    );
}

#[test]
#[should_panic(expected = "trait owner must name an exact typed trait definition")]
fn content_conservation_manifest_rejects_wrong_owner_kind() {
    let (mut program, ..) = content_conservation_validation_fixture();
    program.facts.qualifications.content.conservation_plans[0].owner_kind =
        ContentConservationOwnerKind::TraitRequirement;
    claim_outcome_manifest_json(&program);
}

#[test]
#[should_panic(expected = "machine callable must be a state owned by its exact machine")]
fn content_conservation_manifest_rejects_cross_owner_callable() {
    let (mut program, _, _, _, other_state) = content_conservation_validation_fixture();
    program.facts.qualifications.content.conservation_plans[0].callable = other_state;
    claim_outcome_manifest_json(&program);
}

#[test]
#[should_panic(expected = "must retain its exact normalized report_fingerprint")]
fn content_conservation_manifest_rejects_fingerprint_drift() {
    let (mut program, ..) = content_conservation_validation_fixture();
    program.facts.qualifications.content.conservation_plans[0].report_fingerprint ^= 1;
    claim_outcome_manifest_json(&program);
}

#[test]
#[should_panic(expected = "must join one exact retained projection plan")]
fn content_conservation_manifest_rejects_projection_tuple_drift() {
    let (mut program, ..) = content_conservation_validation_fixture();
    let plan = &mut program.facts.qualifications.content.conservation_plans[0];
    let mut left = plan.equation.left().clone();
    let ContentConservationTerm::Projection {
        projection_report_fingerprint,
        ..
    } = &mut left
    else {
        unreachable!("fixture term is a projection")
    };
    *projection_report_fingerprint ^= 1;
    plan.equation = ContentConservationEquation::new(left, plan.equation.right().clone());
    plan.report_fingerprint = conservation_report_fingerprint(&plan.algebra, &plan.equation);
    claim_outcome_manifest_json(&program);
}

#[test]
#[should_panic(expected = "projection term must retain the plan's exact algebra")]
fn content_conservation_manifest_rejects_projection_algebra_drift() {
    let (mut program, ..) = content_conservation_validation_fixture();
    let plan = &mut program.facts.qualifications.content.conservation_plans[0];
    plan.algebra = ContentAlgebraIdentity::CountedQuantity {
        unit: "named(name(UnrelatedUnit))".to_owned(),
    };
    plan.report_fingerprint = conservation_report_fingerprint(&plan.algebra, &plan.equation);
    claim_outcome_manifest_json(&program);
}

#[test]
#[should_panic(expected = "must retain one authored row per exact owner, callable, and algebra")]
fn content_conservation_manifest_rejects_duplicate_authored_key() {
    let (mut program, ..) = content_conservation_validation_fixture();
    let duplicate = program.facts.qualifications.content.conservation_plans[0].clone();
    program
        .facts
        .qualifications
        .content
        .conservation_plans
        .push(duplicate);
    claim_outcome_manifest_json(&program);
}

#[test]
#[should_panic(expected = "identity reshuffle must retain its exact plan owner and callable")]
fn content_conservation_manifest_rejects_reshuffle_outer_coordinate_drift() {
    let (mut program, _, state, other_machine, _) = content_conservation_validation_fixture();
    let plan = program.facts.qualifications.content.conservation_plans[0].clone();
    program
        .facts
        .qualifications
        .content
        .identity_reshuffles
        .push(ContentIdentityReshuffleFact {
            machine_symbol: other_machine,
            state_symbol: state,
            claim_identity: Default::default(),
            input_parameter_symbol: SymbolHandle::invalid(),
            input_segments: Default::default(),
            output_segments: Default::default(),
            plan,
        });
    claim_outcome_manifest_json(&program);
}

#[test]
#[should_panic(expected = "must retain its exact derived-plan owner and callable")]
fn content_conservation_manifest_rejects_partition_outer_coordinate_drift() {
    let (mut program, _, state, other_machine, _) = content_conservation_validation_fixture();
    let plan = program.facts.qualifications.content.conservation_plans[0].clone();
    push_content_partition_row(&mut program, other_machine, state, plan.clone(), plan);
    claim_outcome_manifest_json(&program);
}

#[test]
#[should_panic(expected = "must retain its exact source-plan coordinates")]
fn content_conservation_manifest_rejects_partition_source_coordinate_drift() {
    let (mut program, machine, state, ..) = content_conservation_validation_fixture();
    let plan = program.facts.qualifications.content.conservation_plans[0].clone();
    push_content_partition_row(&mut program, machine, state, plan.clone(), plan);
    program.facts.qualifications.content.partition_compositions[0].source_report_fingerprint ^= 1;
    claim_outcome_manifest_json(&program);
}

#[test]
fn content_projection_manifest_accepts_exact_normalized_plan_custody() {
    let program = content_projection_validation_fixture();
    let plans = validated_content_projection_plans(&program);

    assert_eq!(
        plans,
        program
            .facts
            .qualifications
            .content
            .plans
            .iter()
            .collect::<Vec<_>>()
    );
}

#[test]
#[should_panic(expected = "must name a nonempty exact declared domain")]
fn content_projection_manifest_rejects_missing_domain() {
    let mut program = content_projection_validation_fixture();
    program.facts.qualifications.content.plans[0].domain = SymbolHandle::from_arena_index(99);
    validated_content_projection_plans(&program);
}

#[test]
#[should_panic(expected = "must retain its exact registered semantic domain")]
fn content_projection_manifest_rejects_unregistered_semantic_domain() {
    let mut program = content_projection_validation_fixture();
    program.facts.qualifications.content.plans[0].semantic_domain = SemanticDomainId(u32::MAX);
    validated_content_projection_plans(&program);
}

#[test]
#[should_panic(expected = "must retain its exact normalized carrier identity")]
fn content_projection_manifest_rejects_carrier_identity_drift() {
    let mut program = content_projection_validation_fixture();
    program.facts.qualifications.content.plans[0].carrier_identity =
        "named(name(Unrelated))".to_owned();
    validated_content_projection_plans(&program);
}

#[test]
#[should_panic(expected = "must name an exact typed projection machine")]
fn content_projection_manifest_rejects_missing_projection_machine() {
    let mut program = content_projection_validation_fixture();
    program.facts.qualifications.content.plans[0].machine = SymbolHandle::from_arena_index(99);
    validated_content_projection_plans(&program);
}

#[test]
#[should_panic(expected = "must retain its exact normalized report_fingerprint")]
fn content_projection_manifest_rejects_fingerprint_drift() {
    let mut program = content_projection_validation_fixture();
    program.facts.qualifications.content.plans[0].report_fingerprint ^= 1;
    validated_content_projection_plans(&program);
}

#[test]
#[should_panic(expected = "must retain one row per exact domain")]
fn content_projection_manifest_rejects_duplicate_domain() {
    let mut program = content_projection_validation_fixture();
    let duplicate = program.facts.qualifications.content.plans[0].clone();
    program.facts.qualifications.content.plans.push(duplicate);
    validated_content_projection_plans(&program);
}

#[test]
#[should_panic(expected = "must retain one row per exact semantic domain")]
fn content_projection_manifest_rejects_duplicate_semantic_domain() {
    let mut program = content_projection_validation_fixture();
    let first = program.facts.qualifications.content.plans[0].clone();
    let second_domain_symbol = SymbolHandle::from_arena_index(93);
    let second_carrier = program
        .typed
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: SymbolHandle::from_arena_index(94),
            name: Identifier::generated("OtherResource"),
        });
    program.typed.push_domain_definition(DomainDefinition {
        symbol: second_domain_symbol,
        name: Identifier::generated("OtherResource::Counted"),
        target_type: second_carrier,
        semantic_id: first.semantic_domain,
        ..Default::default()
    });
    let mut duplicate_semantic = first;
    duplicate_semantic.domain = second_domain_symbol;
    duplicate_semantic.carrier_identity = program
        .typed
        .normalized_type_identity(second_carrier)
        .into_string();
    program
        .facts
        .qualifications
        .content
        .plans
        .push(duplicate_semantic);
    validated_content_projection_plans(&program);
}

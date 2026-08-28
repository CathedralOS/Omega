use super::*;

pub(super) fn content_identity_reshuffle_validation_fixture() -> CheckedTrees {
    let (mut program, machine, state, parameter, ..) = claim_outcome_validation_fixture();
    let domain = SymbolHandle::from_arena_index(105);
    let carrier_symbol = SymbolHandle::from_arena_index(106);
    let projection_machine_symbol = SymbolHandle::from_arena_index(107);
    let projection_state_symbol = SymbolHandle::from_arena_index(108);
    let carrier = program
        .typed
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: carrier_symbol,
            name: Identifier::generated("Resource"),
        });
    let semantic_domain = program.typed.semantic_domains.intern("Resource::Counted");
    program.typed.push_domain_definition(DomainDefinition {
        symbol: domain,
        name: Identifier::generated("Resource::Counted"),
        target_type: carrier,
        semantic_id: semantic_domain,
        ..Default::default()
    });
    let mut projection_machine = Machine {
        symbol: projection_machine_symbol,
        name: Identifier::generated("Resource::content"),
        ..Default::default()
    };
    program.typed.push_machine_state(
        &mut projection_machine,
        State {
            symbol: projection_state_symbol,
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
    let projection_identity = projection_fingerprint(&algebra, &expression);
    program
        .facts
        .qualifications
        .content
        .plans
        .push(ContentProjectionPlan {
            domain,
            semantic_domain,
            carrier_identity: program
                .typed
                .normalized_type_identity(carrier)
                .into_string(),
            machine: projection_machine_symbol,
            algebra: algebra.clone(),
            expression,
            fingerprint: projection_identity,
        });
    let input = ContentConservationTerm::Projection {
        domain,
        semantic_domain,
        projection_machine: projection_machine_symbol,
        projection_fingerprint: projection_identity,
        subject: ContentStructuralPlace {
            version: ContentPlaceVersion::Entry,
            root: ContentPlaceRoot::Parameter {
                position: 0,
                symbol: parameter,
                name: "resource".to_owned(),
                is_self: false,
            },
            segments: Vec::new(),
        },
    };
    let output = ContentConservationTerm::Projection {
        domain,
        semantic_domain,
        projection_machine: projection_machine_symbol,
        projection_fingerprint: projection_identity,
        subject: ContentStructuralPlace {
            version: ContentPlaceVersion::Current,
            root: ContentPlaceRoot::Result,
            segments: Vec::new(),
        },
    };
    let equation = ContentConservationEquation::new(input, output);
    let fingerprint = conservation_fingerprint(&algebra, &equation);
    let claim_identity = program
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .find_map(|(_, event)| {
            (event.source == PermissionEventSource::StateEntry).then_some(event.claim_identity)
        })
        .expect("fixture entry identity");
    program
        .facts
        .qualifications
        .content
        .identity_reshuffles
        .push(ContentIdentityReshuffleFact {
            machine_symbol: machine,
            state_symbol: state,
            claim_identity,
            input_parameter_symbol: parameter,
            input_segments: Default::default(),
            output_segments: Default::default(),
            plan: ContentConservationPlan {
                owner_kind: ContentConservationOwnerKind::Machine,
                owner: machine,
                callable: state,
                algebra,
                equation,
                fingerprint,
            },
        });
    program
}

#[test]
fn content_identity_reshuffle_manifest_accepts_exact_witness_custody() {
    let program = content_identity_reshuffle_validation_fixture();
    let json = claim_outcome_manifest_json(&program);
    assert!(json.contains("\"content_identity_reshuffles\": [\n    {"));
    assert!(json.contains("\"input\": {\"parameter\":"));
    assert!(json.contains("\"output_path\": []"));
}

#[test]
#[should_panic(expected = "input must retain an exact valid path span")]
fn content_identity_reshuffle_manifest_rejects_invalid_input_span() {
    let mut program = content_identity_reshuffle_validation_fixture();
    program.facts.qualifications.content.identity_reshuffles[0].input_segments =
        psi_arena::HandleSpan::from_parts(psi_arena::Handle::from_arena_index(999), 1);
    claim_outcome_manifest_json(&program);
}

#[test]
#[should_panic(expected = "output must retain an exact valid path span")]
fn content_identity_reshuffle_manifest_rejects_invalid_output_span() {
    let mut program = content_identity_reshuffle_validation_fixture();
    program.facts.qualifications.content.identity_reshuffles[0].output_segments =
        psi_arena::HandleSpan::from_parts(psi_arena::Handle::from_arena_index(999), 1);
    claim_outcome_manifest_json(&program);
}

#[test]
#[should_panic(expected = "exact parameter owned by its state")]
fn content_identity_reshuffle_manifest_rejects_missing_parameter() {
    let mut program = content_identity_reshuffle_validation_fixture();
    program.facts.qualifications.content.identity_reshuffles[0].input_parameter_symbol =
        SymbolHandle::from_arena_index(999);
    claim_outcome_manifest_json(&program);
}

#[test]
#[should_panic(expected = "must retain a non-unknown claim identity")]
fn content_identity_reshuffle_manifest_rejects_unknown_claim_identity() {
    let mut program = content_identity_reshuffle_validation_fixture();
    program.facts.qualifications.content.identity_reshuffles[0].claim_identity =
        PermissionClaimIdentity::Unknown;
    claim_outcome_manifest_json(&program);
}

#[test]
#[should_panic(expected = "must retain its exact input permission identity")]
fn content_identity_reshuffle_manifest_rejects_wrong_claim_identity() {
    let mut program = content_identity_reshuffle_validation_fixture();
    let row = &mut program.facts.qualifications.content.identity_reshuffles[0];
    row.claim_identity = PermissionClaimIdentity::Established {
        machine_symbol: row.machine_symbol,
        state_symbol: row.state_symbol,
        source: PermissionEventSource::StateEntry,
        ordinal: 99,
    };
    claim_outcome_manifest_json(&program);
}

#[test]
#[should_panic(expected = "one distinct live retained permission identity")]
fn content_identity_reshuffle_manifest_rejects_ambiguous_entry_identity() {
    let mut program = content_identity_reshuffle_validation_fixture();
    let row = program.facts.qualifications.content.identity_reshuffles[0].clone();
    program
        .facts
        .flow
        .ownership
        .permissions
        .insert(FlowPermissionEventFact {
            machine_symbol: row.machine_symbol,
            state_symbol: row.state_symbol,
            source: PermissionEventSource::StateEntry,
            kind: PermissionEventKind::Establish,
            access: PermissionAccess::Owned,
            claim_identity: PermissionClaimIdentity::Established {
                machine_symbol: row.machine_symbol,
                state_symbol: row.state_symbol,
                source: PermissionEventSource::StateEntry,
                ordinal: 99,
            },
            provenance: PermissionProvenance::Established {
                machine_symbol: row.machine_symbol,
                state_symbol: row.state_symbol,
                source: PermissionEventSource::StateEntry,
            },
            root: PlaceRoot::Symbol(row.input_parameter_symbol),
            obligation_live: true,
            ..Default::default()
        });
    validate_content_identity_reshuffle(&program, &row);
}

#[test]
#[should_panic(expected = "one exact input-relative claim outcome")]
fn content_identity_reshuffle_manifest_rejects_absent_input_outcome() {
    let mut program = content_identity_reshuffle_validation_fixture();
    let unmatched_output = program
        .facts
        .flow
        .ownership
        .segments
        .insert_many([psi_facts::PlaceSegment::FixedIndex { index: 9 }]);
    program.facts.qualifications.content.identity_reshuffles[0].output_segments = unmatched_output;
    claim_outcome_manifest_json(&program);
}

#[test]
#[should_panic(expected = "paths must not retain a runtime index")]
fn content_identity_reshuffle_manifest_rejects_runtime_index_path() {
    let mut program = content_identity_reshuffle_validation_fixture();
    let runtime_output =
        program
            .facts
            .flow
            .ownership
            .segments
            .insert_many([psi_facts::PlaceSegment::Index {
                expression: ExpressionHandle::invalid(),
            }]);
    program.facts.qualifications.content.identity_reshuffles[0].output_segments = runtime_output;
    first_claim_outcome_entries_mut(&mut program)[0].output_segments = runtime_output;
    claim_outcome_manifest_json(&program);
}

#[test]
#[should_panic(expected = "field path must name an exact typed field")]
fn content_identity_reshuffle_manifest_rejects_missing_typed_segment() {
    let mut program = content_identity_reshuffle_validation_fixture();
    let missing_output =
        program
            .facts
            .flow
            .ownership
            .segments
            .insert_many([psi_facts::PlaceSegment::Field {
                symbol: SymbolHandle::from_arena_index(999),
            }]);
    program.facts.qualifications.content.identity_reshuffles[0].output_segments = missing_output;
    first_claim_outcome_entries_mut(&mut program)[0].output_segments = missing_output;
    claim_outcome_manifest_json(&program);
}

#[test]
#[should_panic(expected = "equation must retain its exact input and output projection subjects")]
fn content_identity_reshuffle_manifest_rejects_subject_drift() {
    let mut program = content_identity_reshuffle_validation_fixture();
    let row = &mut program.facts.qualifications.content.identity_reshuffles[0];
    let mutate_subject = |term: &ContentConservationTerm| match term {
        ContentConservationTerm::Projection {
            domain,
            semantic_domain,
            projection_machine,
            projection_fingerprint,
            subject,
        } if subject.version == ContentPlaceVersion::Entry => {
            let mut subject = subject.clone();
            let ContentPlaceRoot::Parameter { position, .. } = &mut subject.root else {
                unreachable!("fixture entry subject is a parameter")
            };
            *position = 1;
            ContentConservationTerm::Projection {
                domain: *domain,
                semantic_domain: *semantic_domain,
                projection_machine: *projection_machine,
                projection_fingerprint: *projection_fingerprint,
                subject,
            }
        }
        other => other.clone(),
    };
    let left = mutate_subject(row.plan.equation.left());
    let right = mutate_subject(row.plan.equation.right());
    row.plan.equation = ContentConservationEquation::new(left, right);
    row.plan.fingerprint = conservation_fingerprint(&row.plan.algebra, &row.plan.equation);
    claim_outcome_manifest_json(&program);
}

#[test]
#[should_panic(expected = "must retain one exact witness row per plan")]
fn content_identity_reshuffle_manifest_rejects_duplicate_row() {
    let mut program = content_identity_reshuffle_validation_fixture();
    let duplicate = program.facts.qualifications.content.identity_reshuffles[0].clone();
    program
        .facts
        .qualifications
        .content
        .identity_reshuffles
        .push(duplicate);
    claim_outcome_manifest_json(&program);
}

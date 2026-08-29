use super::*;

pub(super) fn content_partition_input_validation_fixture() -> CheckedTrees {
    let mut program = content_identity_reshuffle_validation_fixture();
    let reshuffle = program.facts.qualifications.content.identity_reshuffles[0].clone();
    let entry_place = [
        reshuffle.plan.equation.left(),
        reshuffle.plan.equation.right(),
    ]
    .into_iter()
    .find_map(|term| match term {
        ContentConservationTerm::Projection { subject, .. }
            if subject.version == ContentPlaceVersion::Entry =>
        {
            Some(subject.clone())
        }
        _ => None,
    })
    .expect("fixture entry projection subject");
    let source_left = reshuffle.plan.equation.left().clone();
    let source_right = reshuffle.plan.equation.right().clone();
    let source_equation = ContentConservationEquation::new(
        ContentConservationTerm::Separate(vec![source_left.clone(), source_right.clone()]),
        ContentConservationTerm::Separate(vec![source_left, source_right]),
    );
    let source_plan = ContentConservationPlan {
        owner_kind: reshuffle.plan.owner_kind,
        owner: reshuffle.plan.owner,
        callable: reshuffle.plan.callable,
        algebra: reshuffle.plan.algebra.clone(),
        report_fingerprint: conservation_report_fingerprint(
            &reshuffle.plan.algebra,
            &source_equation,
        ),
        equation: source_equation,
    };
    let mut substitution_subjects = Vec::new();
    for term in [source_plan.equation.left(), source_plan.equation.right()] {
        let ContentConservationTerm::Separate(children) = term else {
            unreachable!("fixture source equation is separated")
        };
        for child in children {
            let ContentConservationTerm::Projection { subject, .. } = child else {
                unreachable!("fixture separated children are projections")
            };
            if !substitution_subjects.contains(subject) {
                substitution_subjects.push(subject.clone());
            }
        }
    }
    let substitutions = substitution_subjects
        .into_iter()
        .map(|subject| ContentPartitionPlaceSubstitution {
            source: subject.clone(),
            target: subject,
        })
        .collect();
    program
        .facts
        .qualifications
        .content
        .conservation_plans
        .push(source_plan.clone());
    let calls = program.facts.flow.control.calls.insert_many([FlowCallFact {
        statement_index: 4,
        call_ordinal: 2,
        target_symbol: reshuffle.state_symbol,
        ..Default::default()
    }]);
    program.facts.flow.control.states.insert(FlowStateFact {
        machine_symbol: reshuffle.machine_symbol,
        state_symbol: reshuffle.state_symbol,
        calls,
        ..Default::default()
    });
    program
        .facts
        .qualifications
        .content
        .partition_compositions
        .push(ContentPartitionCompositionFact {
            machine_symbol: reshuffle.machine_symbol,
            state_symbol: reshuffle.state_symbol,
            source_callable: source_plan.callable,
            source_report_fingerprint: source_plan.report_fingerprint,
            source_derivation_depth: 0,
            source_plan: source_plan.clone(),
            statement_index: 4,
            call_ordinal: 2,
            input_claim_identities: vec![reshuffle.claim_identity],
            input_claim_bindings: vec![psi_checked_trees::ContentPartitionInputClaimBinding {
                claim_identity: reshuffle.claim_identity,
                entry_place,
            }],
            result_rewrites: Vec::new(),
            substitutions,
            plan: source_plan,
        });
    program
}

#[test]
fn content_partition_input_manifest_accepts_exact_call_and_input_custody() {
    let program = content_partition_input_validation_fixture();
    let json = claim_outcome_manifest_json(&program);

    assert!(json.contains("\"call\": {\"statement_index\": 4, \"call_ordinal\": 2}"));
    assert!(json.contains("\"input_claim_identities\": [{\"kind\": \"established\""));
    assert!(json.contains("\"entry_place\": {\"version\": \"entry\""));
}

#[test]
#[should_panic(expected = "statement index must be within its exact state")]
fn content_partition_input_manifest_rejects_out_of_range_statement() {
    let mut program = content_partition_input_validation_fixture();
    program.facts.qualifications.content.partition_compositions[0].statement_index = 5;
    claim_outcome_manifest_json(&program);
}

#[test]
#[should_panic(expected = "must name one exact checked flow state")]
fn content_partition_input_manifest_rejects_missing_flow_state() {
    let mut program = content_partition_input_validation_fixture();
    program.facts.flow.control.states.clear();
    claim_outcome_manifest_json(&program);
}

#[test]
#[should_panic(expected = "must name exactly one checked flow state")]
fn content_partition_input_manifest_rejects_duplicate_flow_state() {
    let mut program = content_partition_input_validation_fixture();
    let duplicate = program
        .facts
        .flow
        .control
        .states
        .iter()
        .next()
        .expect("fixture flow state")
        .1
        .clone();
    program.facts.flow.control.states.insert(duplicate);
    claim_outcome_manifest_json(&program);
}

#[test]
#[should_panic(expected = "flow state must retain an exact valid call span")]
fn content_partition_input_manifest_rejects_invalid_call_span() {
    let mut program = content_partition_input_validation_fixture();
    program.facts.flow.control.states.for_each_mut(|_, state| {
        state.calls =
            psi_arena::HandleSpan::from_parts(psi_arena::Handle::from_arena_index(999), 1);
    });
    claim_outcome_manifest_json(&program);
}

#[test]
#[should_panic(expected = "must retain one exact checked call coordinate")]
fn content_partition_input_manifest_rejects_wrong_call_ordinal() {
    let mut program = content_partition_input_validation_fixture();
    let calls = program
        .facts
        .flow
        .control
        .states
        .iter()
        .next()
        .expect("fixture flow state")
        .1
        .calls;
    program
        .facts
        .flow
        .control
        .calls
        .span_mut(calls)
        .expect("fixture calls")[0]
        .call_ordinal = 3;
    claim_outcome_manifest_json(&program);
}

#[test]
#[should_panic(expected = "must retain its exact checked source target")]
fn content_partition_input_manifest_rejects_wrong_call_target() {
    let mut program = content_partition_input_validation_fixture();
    let calls = program
        .facts
        .flow
        .control
        .states
        .iter()
        .next()
        .expect("fixture flow state")
        .1
        .calls;
    program
        .facts
        .flow
        .control
        .calls
        .span_mut(calls)
        .expect("fixture calls")[0]
        .target_symbol = SymbolHandle::from_arena_index(999);
    claim_outcome_manifest_json(&program);
}

#[test]
#[should_panic(expected = "must retain exactly one checked call coordinate")]
fn content_partition_input_manifest_rejects_duplicate_call_coordinate() {
    let mut program = content_partition_input_validation_fixture();
    let old_calls = program
        .facts
        .flow
        .control
        .states
        .iter()
        .next()
        .expect("fixture flow state")
        .1
        .calls;
    let call = program
        .facts
        .flow
        .control
        .calls
        .span(old_calls)
        .expect("fixture calls")[0]
        .clone();
    let calls = program
        .facts
        .flow
        .control
        .calls
        .insert_many([call.clone(), call]);
    program
        .facts
        .flow
        .control
        .states
        .for_each_mut(|_, state| state.calls = calls);
    claim_outcome_manifest_json(&program);
}

#[test]
#[should_panic(expected = "must retain at least one input claim identity")]
fn content_partition_input_manifest_rejects_empty_inputs() {
    let mut program = content_partition_input_validation_fixture();
    let row = &mut program.facts.qualifications.content.partition_compositions[0];
    row.input_claim_identities.clear();
    row.input_claim_bindings.clear();
    claim_outcome_manifest_json(&program);
}

#[test]
#[should_panic(expected = "input claim identities must be non-unknown")]
fn content_partition_input_manifest_rejects_unknown_identity() {
    let mut program = content_partition_input_validation_fixture();
    let row = &mut program.facts.qualifications.content.partition_compositions[0];
    row.input_claim_identities[0] = PermissionClaimIdentity::Unknown;
    row.input_claim_bindings[0].claim_identity = PermissionClaimIdentity::Unknown;
    claim_outcome_manifest_json(&program);
}

#[test]
#[should_panic(expected = "input claim identities must be unique")]
fn content_partition_input_manifest_rejects_duplicate_identity() {
    let mut program = content_partition_input_validation_fixture();
    let row = &mut program.facts.qualifications.content.partition_compositions[0];
    row.input_claim_identities
        .push(row.input_claim_identities[0]);
    row.input_claim_bindings
        .push(row.input_claim_bindings[0].clone());
    claim_outcome_manifest_json(&program);
}

#[test]
#[should_panic(expected = "input identities must exactly match ordered bindings")]
fn content_partition_input_manifest_rejects_binding_identity_drift() {
    let mut program = content_partition_input_validation_fixture();
    let row = &mut program.facts.qualifications.content.partition_compositions[0];
    row.input_claim_bindings[0].claim_identity = PermissionClaimIdentity::Established {
        machine_symbol: row.machine_symbol,
        state_symbol: row.state_symbol,
        source: PermissionEventSource::StateEntry,
        ordinal: 99,
    };
    claim_outcome_manifest_json(&program);
}

#[test]
#[should_panic(expected = "must name its exact caller parameter")]
fn content_partition_input_manifest_rejects_wrong_entry_parameter() {
    let mut program = content_partition_input_validation_fixture();
    let ContentPlaceRoot::Parameter { symbol, .. } =
        &mut program.facts.qualifications.content.partition_compositions[0].input_claim_bindings[0]
            .entry_place
            .root
    else {
        unreachable!("fixture binding is an entry parameter")
    };
    *symbol = SymbolHandle::from_arena_index(999);
    claim_outcome_manifest_json(&program);
}

#[test]
#[should_panic(expected = "must match one live retained permission event")]
fn content_partition_input_manifest_rejects_binding_path_drift() {
    let mut program = content_partition_input_validation_fixture();
    program.facts.qualifications.content.partition_compositions[0].input_claim_bindings[0]
        .entry_place
        .segments
        .push(psi_language_semantics::content::ContentPlaceSegment::FixedIndex(9));
    claim_outcome_manifest_json(&program);
}

#[test]
#[should_panic(expected = "paths must not retain a runtime index")]
fn content_partition_input_manifest_rejects_runtime_permission_path() {
    let mut program = content_partition_input_validation_fixture();
    let row = program.facts.qualifications.content.partition_compositions[0].clone();
    let runtime_path =
        program
            .facts
            .flow
            .ownership
            .segments
            .insert_many([psi_facts::PlaceSegment::Index {
                expression: ExpressionHandle::invalid(),
            }]);
    program
        .facts
        .flow
        .ownership
        .permissions
        .for_each_mut(|_, event| {
            if event.source == PermissionEventSource::StateEntry {
                event.segments = runtime_path;
            }
        });
    validate_content_partition_input_custody(&program, &row);
}

#[test]
#[should_panic(expected = "must match one live retained permission event")]
fn content_partition_input_manifest_rejects_missing_permission_event() {
    let mut program = content_partition_input_validation_fixture();
    let row = program.facts.qualifications.content.partition_compositions[0].clone();
    program.facts.flow.ownership.permissions.clear();
    validate_content_partition_input_custody(&program, &row);
}

#[test]
#[should_panic(expected = "must match one live retained permission event")]
fn content_partition_input_manifest_rejects_ambiguous_permission_event() {
    let mut program = content_partition_input_validation_fixture();
    let row = program.facts.qualifications.content.partition_compositions[0].clone();
    let duplicate = program
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .find(|(_, event)| event.source == PermissionEventSource::StateEntry)
        .expect("fixture entry permission")
        .1
        .clone();
    program.facts.flow.ownership.permissions.insert(duplicate);
    validate_content_partition_input_custody(&program, &row);
}

#[test]
#[should_panic(expected = "must retain one exact row per call and plan")]
fn content_partition_input_manifest_rejects_duplicate_partition_row() {
    let mut program = content_partition_input_validation_fixture();
    let duplicate = program.facts.qualifications.content.partition_compositions[0].clone();
    program
        .facts
        .qualifications
        .content
        .partition_compositions
        .push(duplicate);
    claim_outcome_manifest_json(&program);
}

#[test]
fn content_partition_substitution_manifest_accepts_exact_closed_replay() {
    let program = content_partition_input_validation_fixture();
    let json = claim_outcome_manifest_json(&program);

    assert!(json.contains("\"substitutions\": [{\"source\": {\"version\": \"entry\""));
    assert!(json.contains("\"kind\": \"separate\""));
}

#[test]
#[should_panic(expected = "source equation must retain an authored partition")]
fn content_partition_substitution_manifest_rejects_nonpartition_source() {
    let mut program = content_partition_input_validation_fixture();
    let row = &mut program.facts.qualifications.content.partition_compositions[0];
    let ContentConservationTerm::Separate(left) = row.source_plan.equation.left() else {
        unreachable!("fixture left is separated")
    };
    let left = left.clone();
    row.source_plan.equation = ContentConservationEquation::new(left[0].clone(), left[1].clone());
    validate_content_partition_substitution_replay(row);
}

#[test]
#[should_panic(expected = "must retain a nonempty exact substitution map")]
fn content_partition_substitution_manifest_rejects_empty_map() {
    let mut program = content_partition_input_validation_fixture();
    let row = &mut program.facts.qualifications.content.partition_compositions[0];
    row.substitutions.clear();
    validate_content_partition_substitution_replay(row);
}

#[test]
#[should_panic(expected = "substitution sources must be unique")]
fn content_partition_substitution_manifest_rejects_duplicate_source() {
    let mut program = content_partition_input_validation_fixture();
    let row = &mut program.facts.qualifications.content.partition_compositions[0];
    row.substitutions.push(row.substitutions[0].clone());
    validate_content_partition_substitution_replay(row);
}

#[test]
#[should_panic(expected = "substitution targets must be unique")]
fn content_partition_substitution_manifest_rejects_duplicate_target() {
    let mut program = content_partition_input_validation_fixture();
    let row = &mut program.facts.qualifications.content.partition_compositions[0];
    row.substitutions[1].target = row.substitutions[0].target.clone();
    validate_content_partition_substitution_replay(row);
}

#[test]
#[should_panic(expected = "substitution source must occur in the source equation")]
fn content_partition_substitution_manifest_rejects_extra_source() {
    let mut program = content_partition_input_validation_fixture();
    let row = &mut program.facts.qualifications.content.partition_compositions[0];
    row.substitutions.push(ContentPartitionPlaceSubstitution {
        source: ContentStructuralPlace {
            version: ContentPlaceVersion::Current,
            root: ContentPlaceRoot::Result,
            segments: vec![psi_language_semantics::content::ContentPlaceSegment::FixedIndex(98)],
        },
        target: ContentStructuralPlace {
            version: ContentPlaceVersion::Current,
            root: ContentPlaceRoot::Result,
            segments: vec![psi_language_semantics::content::ContentPlaceSegment::FixedIndex(99)],
        },
    });
    validate_content_partition_substitution_replay(row);
}

#[test]
#[should_panic(expected = "must cover every source subject exactly once")]
fn content_partition_substitution_manifest_rejects_missing_subject() {
    let mut program = content_partition_input_validation_fixture();
    let row = &mut program.facts.qualifications.content.partition_compositions[0];
    row.substitutions.pop();
    validate_content_partition_substitution_replay(row);
}

#[test]
#[should_panic(expected = "derived equation must equal exact substitution replay")]
fn content_partition_substitution_manifest_rejects_target_drift() {
    let mut program = content_partition_input_validation_fixture();
    let row = &mut program.facts.qualifications.content.partition_compositions[0];
    row.substitutions[0]
        .target
        .segments
        .push(psi_language_semantics::content::ContentPlaceSegment::FixedIndex(9));
    validate_content_partition_substitution_replay(row);
}

#[test]
#[should_panic(expected = "derived equation must equal exact substitution replay")]
fn content_partition_substitution_manifest_rejects_projection_tuple_drift() {
    fn drift_first_projection(term: &ContentConservationTerm) -> ContentConservationTerm {
        match term {
            ContentConservationTerm::Projection {
                domain,
                semantic_domain,
                projection_machine,
                projection_report_fingerprint,
                subject,
            } => ContentConservationTerm::Projection {
                domain: SymbolHandle::from_arena_index(domain.arena_index() + 1000),
                semantic_domain: *semantic_domain,
                projection_machine: *projection_machine,
                projection_report_fingerprint: *projection_report_fingerprint,
                subject: subject.clone(),
            },
            ContentConservationTerm::Separate(terms) => {
                let mut terms = terms.clone();
                terms[0] = drift_first_projection(&terms[0]);
                ContentConservationTerm::Separate(terms)
            }
        }
    }

    let mut program = content_partition_input_validation_fixture();
    let row = &mut program.facts.qualifications.content.partition_compositions[0];
    row.plan.equation = ContentConservationEquation::new(
        drift_first_projection(row.plan.equation.left()),
        row.plan.equation.right().clone(),
    );
    validate_content_partition_substitution_replay(row);
}

#[test]
#[should_panic(expected = "replay must preserve the exact source algebra")]
fn content_partition_substitution_manifest_rejects_algebra_drift() {
    let mut program = content_partition_input_validation_fixture();
    let row = &mut program.facts.qualifications.content.partition_compositions[0];
    row.plan.algebra = ContentAlgebraIdentity::CountedQuantity {
        unit: "named(name(OtherUnit))".to_owned(),
    };
    validate_content_partition_substitution_replay(row);
}

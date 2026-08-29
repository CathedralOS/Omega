use super::*;

fn content_partition_result_rewrite_validation_fixture() -> CheckedTrees {
    fn rewrite_result_subject(
        term: &ContentConservationTerm,
        source: &ContentStructuralPlace,
        target: &ContentStructuralPlace,
    ) -> ContentConservationTerm {
        match term {
            ContentConservationTerm::Projection {
                domain,
                semantic_domain,
                projection_machine,
                projection_report_fingerprint,
                subject,
            } => ContentConservationTerm::Projection {
                domain: *domain,
                semantic_domain: *semantic_domain,
                projection_machine: *projection_machine,
                projection_report_fingerprint: *projection_report_fingerprint,
                subject: if subject == source {
                    target.clone()
                } else {
                    subject.clone()
                },
            },
            ContentConservationTerm::Separate(terms) => ContentConservationTerm::Separate(
                terms
                    .iter()
                    .map(|term| rewrite_result_subject(term, source, target))
                    .collect(),
            ),
        }
    }

    let mut program = content_partition_input_validation_fixture();
    let local_symbol = SymbolHandle::from_arena_index(109);
    let (machine_symbol, state_symbol, statement_index, call_ordinal, source_callable) = {
        let row = &program.facts.qualifications.content.partition_compositions[0];
        (
            row.machine_symbol,
            row.state_symbol,
            row.statement_index,
            row.call_ordinal,
            row.source_callable,
        )
    };
    let statement_nodes = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol)
        .and_then(|machine| {
            program
                .machine_states(machine)
                .iter()
                .find(|state| state.symbol == state_symbol)
        })
        .expect("fixture caller state")
        .statement_nodes;
    program
        .typed
        .statement_table
        .statements_mut(statement_nodes)[statement_index] =
        StatementNode::LocalData(psi_typed_trees::statement::TableLocalData {
            symbol: local_symbol,
            name: Identifier::generated("staged"),
            ..Default::default()
        });

    let result_identity = PermissionClaimIdentity::Established {
        machine_symbol,
        state_symbol,
        source: PermissionEventSource::Call {
            statement_index,
            call_ordinal,
            target_symbol: source_callable,
        },
        ordinal: 12,
    };
    let result_provenance = PermissionProvenance::Established {
        machine_symbol,
        state_symbol,
        source: PermissionEventSource::Call {
            statement_index,
            call_ordinal,
            target_symbol: source_callable,
        },
    };
    program
        .facts
        .flow
        .ownership
        .permissions
        .insert(FlowPermissionEventFact {
            machine_symbol,
            state_symbol,
            source: PermissionEventSource::Call {
                statement_index,
                call_ordinal,
                target_symbol: source_callable,
            },
            kind: PermissionEventKind::Establish,
            access: PermissionAccess::Owned,
            claim_identity: result_identity,
            provenance: result_provenance,
            root: PlaceRoot::Symbol(local_symbol),
            obligation_live: true,
            ..Default::default()
        });

    let (source, target) = {
        let row = &mut program.facts.qualifications.content.partition_compositions[0];
        let substitution = row
            .substitutions
            .iter_mut()
            .find(|substitution| substitution.source.root == ContentPlaceRoot::Result)
            .expect("fixture result substitution");
        substitution
            .target
            .segments
            .push(psi_language_semantics::content::ContentPlaceSegment::FixedIndex(2));
        let source = substitution.source.clone();
        let target = substitution.target.clone();
        row.plan.equation = ContentConservationEquation::new(
            rewrite_result_subject(row.source_plan.equation.left(), &source, &target),
            rewrite_result_subject(row.source_plan.equation.right(), &source, &target),
        );
        row.plan.report_fingerprint =
            conservation_report_fingerprint(&row.plan.algebra, &row.plan.equation);
        row.result_rewrites = vec![ContentPartitionResultRewrite {
            claim_identity: result_identity,
            source: source.clone(),
            target: target.clone(),
        }];
        (source, target)
    };
    let output_segments = program
        .facts
        .flow
        .ownership
        .segments
        .insert_many([psi_facts::PlaceSegment::FixedIndex { index: 2 }]);
    assert!(source.segments.is_empty());
    assert_eq!(
        target.segments,
        vec![psi_language_semantics::content::ContentPlaceSegment::FixedIndex(2)]
    );
    let old_entries = program
        .facts
        .flow
        .ownership
        .claim_outcome_maps
        .iter()
        .next()
        .expect("fixture outcome map")
        .1
        .entries;
    let mut entries = program
        .facts
        .flow
        .ownership
        .claim_outcome_entries
        .span(old_entries)
        .expect("fixture outcome entries")
        .to_vec();
    entries.push(FlowClaimOutcomeEntryFact {
        output_segments,
        source: FlowClaimOutcomeSource::Established {
            claim_identity: result_identity,
            provenance: result_provenance,
        },
    });
    let entries = program
        .facts
        .flow
        .ownership
        .claim_outcome_entries
        .insert_many(entries);
    program
        .facts
        .flow
        .ownership
        .claim_outcome_maps
        .for_each_mut(|_, map| map.entries = entries);
    program
}

fn append_result_outcome_entry(program: &mut CheckedTrees, entry: FlowClaimOutcomeEntryFact) {
    let old_entries = program
        .facts
        .flow
        .ownership
        .claim_outcome_maps
        .iter()
        .next()
        .expect("fixture outcome map")
        .1
        .entries;
    let mut entries = program
        .facts
        .flow
        .ownership
        .claim_outcome_entries
        .span(old_entries)
        .expect("fixture outcome entries")
        .to_vec();
    entries.push(entry);
    let entries = program
        .facts
        .flow
        .ownership
        .claim_outcome_entries
        .insert_many(entries);
    program
        .facts
        .flow
        .ownership
        .claim_outcome_maps
        .for_each_mut(|_, map| map.entries = entries);
}

#[test]
fn content_partition_result_rewrite_manifest_accepts_exact_staged_custody() {
    let program = content_partition_result_rewrite_validation_fixture();
    let json = claim_outcome_manifest_json(&program);

    assert!(json.contains("\"result_rewrites\": [{\"claim_identity\": {\"kind\": \"established\""));
    assert!(json.contains("\"target\": {\"version\": \"current\""));
    assert!(json.contains("\"fixed_index\": 2"));
}

#[test]
fn content_partition_result_rewrite_manifest_accepts_explicit_empty() {
    let program = content_partition_input_validation_fixture();
    let json = claim_outcome_manifest_json(&program);
    assert!(json.contains("\"result_rewrites\": []"));
}

#[test]
#[should_panic(expected = "must retain a non-unknown claim identity")]
fn content_partition_result_rewrite_manifest_rejects_unknown_identity() {
    let mut program = content_partition_result_rewrite_validation_fixture();
    program.facts.qualifications.content.partition_compositions[0].result_rewrites[0]
        .claim_identity = PermissionClaimIdentity::Unknown;
    let row = program.facts.qualifications.content.partition_compositions[0].clone();
    validate_content_partition_result_rewrites(&program, &row);
}

#[test]
#[should_panic(expected = "claim identities must be unique")]
fn content_partition_result_rewrite_manifest_rejects_duplicate_identity() {
    let mut program = content_partition_result_rewrite_validation_fixture();
    let row = &mut program.facts.qualifications.content.partition_compositions[0];
    row.result_rewrites.push(row.result_rewrites[0].clone());
    let row = row.clone();
    validate_content_partition_result_rewrites(&program, &row);
}

#[test]
#[should_panic(expected = "result rewrite sources must be unique")]
fn content_partition_result_rewrite_manifest_rejects_duplicate_source() {
    let mut program = content_partition_result_rewrite_validation_fixture();
    let row = &mut program.facts.qualifications.content.partition_compositions[0];
    let mut duplicate = row.result_rewrites[0].clone();
    duplicate.claim_identity = PermissionClaimIdentity::Established {
        machine_symbol: row.machine_symbol,
        state_symbol: row.state_symbol,
        source: PermissionEventSource::Statement {
            statement_index: row.statement_index,
        },
        ordinal: 99,
    };
    duplicate
        .target
        .segments
        .push(psi_language_semantics::content::ContentPlaceSegment::FixedIndex(3));
    row.result_rewrites.push(duplicate);
    let row = row.clone();
    validate_content_partition_result_rewrites(&program, &row);
}

#[test]
#[should_panic(expected = "result rewrite targets must be unique")]
fn content_partition_result_rewrite_manifest_rejects_duplicate_target() {
    let mut program = content_partition_result_rewrite_validation_fixture();
    let row = &mut program.facts.qualifications.content.partition_compositions[0];
    let mut duplicate = row.result_rewrites[0].clone();
    duplicate.claim_identity = PermissionClaimIdentity::Established {
        machine_symbol: row.machine_symbol,
        state_symbol: row.state_symbol,
        source: PermissionEventSource::Statement {
            statement_index: row.statement_index,
        },
        ordinal: 99,
    };
    duplicate
        .source
        .segments
        .push(psi_language_semantics::content::ContentPlaceSegment::FixedIndex(3));
    row.result_rewrites.push(duplicate);
    let row = row.clone();
    validate_content_partition_result_rewrites(&program, &row);
}

#[test]
#[should_panic(expected = "source must be an exact current result place")]
fn content_partition_result_rewrite_manifest_rejects_wrong_source_root() {
    let mut program = content_partition_result_rewrite_validation_fixture();
    program.facts.qualifications.content.partition_compositions[0].result_rewrites[0]
        .source
        .version = ContentPlaceVersion::Entry;
    let row = program.facts.qualifications.content.partition_compositions[0].clone();
    validate_content_partition_result_rewrites(&program, &row);
}

#[test]
#[should_panic(expected = "target must be an exact current result place")]
fn content_partition_result_rewrite_manifest_rejects_wrong_target_root() {
    let mut program = content_partition_result_rewrite_validation_fixture();
    let row = &mut program.facts.qualifications.content.partition_compositions[0];
    row.result_rewrites[0].target.root = ContentPlaceRoot::Parameter {
        position: 0,
        symbol: SymbolHandle::from_arena_index(102),
        name: "resource".to_owned(),
        is_self: false,
    };
    let row = row.clone();
    validate_content_partition_result_rewrites(&program, &row);
}

#[test]
#[should_panic(expected = "must retain one exact substitution pair")]
fn content_partition_result_rewrite_manifest_rejects_missing_substitution_pair() {
    let mut program = content_partition_result_rewrite_validation_fixture();
    let row = &mut program.facts.qualifications.content.partition_compositions[0];
    row.result_rewrites[0]
        .target
        .segments
        .push(psi_language_semantics::content::ContentPlaceSegment::FixedIndex(9));
    let row = row.clone();
    validate_content_partition_result_rewrites(&program, &row);
}

#[test]
#[should_panic(expected = "must belong to its exact staged local")]
fn content_partition_result_rewrite_manifest_rejects_nonlocal_statement() {
    let mut program = content_partition_result_rewrite_validation_fixture();
    let row = program.facts.qualifications.content.partition_compositions[0].clone();
    let statement_nodes = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == row.machine_symbol)
        .and_then(|machine| {
            program
                .machine_states(machine)
                .iter()
                .find(|state| state.symbol == row.state_symbol)
        })
        .expect("fixture state")
        .statement_nodes;
    program
        .typed
        .statement_table
        .statements_mut(statement_nodes)[row.statement_index] = StatementNode::default();
    validate_content_partition_result_rewrites(&program, &row);
}

#[test]
#[should_panic(expected = "must match one live staged-local permission event")]
fn content_partition_result_rewrite_manifest_rejects_wrong_local_root() {
    let mut program = content_partition_result_rewrite_validation_fixture();
    let row = program.facts.qualifications.content.partition_compositions[0].clone();
    program
        .facts
        .flow
        .ownership
        .permissions
        .for_each_mut(|_, event| {
            if event.claim_identity == row.result_rewrites[0].claim_identity {
                event.root = PlaceRoot::Symbol(SymbolHandle::from_arena_index(999));
            }
        });
    validate_content_partition_result_rewrites(&program, &row);
}

#[test]
#[should_panic(expected = "must match one live staged-local permission event")]
fn content_partition_result_rewrite_manifest_rejects_missing_event() {
    let mut program = content_partition_result_rewrite_validation_fixture();
    let row = program.facts.qualifications.content.partition_compositions[0].clone();
    program
        .facts
        .flow
        .ownership
        .permissions
        .for_each_mut(|_, event| {
            if event.claim_identity == row.result_rewrites[0].claim_identity {
                event.obligation_live = false;
            }
        });
    validate_content_partition_result_rewrites(&program, &row);
}

#[test]
#[should_panic(expected = "must match one live staged-local permission event")]
fn content_partition_result_rewrite_manifest_rejects_ambiguous_event() {
    let mut program = content_partition_result_rewrite_validation_fixture();
    let row = program.facts.qualifications.content.partition_compositions[0].clone();
    let duplicate = program
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .find(|(_, event)| event.claim_identity == row.result_rewrites[0].claim_identity)
        .expect("fixture result event")
        .1
        .clone();
    program.facts.flow.ownership.permissions.insert(duplicate);
    validate_content_partition_result_rewrites(&program, &row);
}

#[test]
#[should_panic(expected = "result event must retain an exact valid path")]
fn content_partition_result_rewrite_manifest_rejects_invalid_event_path() {
    let mut program = content_partition_result_rewrite_validation_fixture();
    let row = program.facts.qualifications.content.partition_compositions[0].clone();
    program
        .facts
        .flow
        .ownership
        .permissions
        .for_each_mut(|_, event| {
            if event.claim_identity == row.result_rewrites[0].claim_identity {
                event.segments =
                    psi_arena::HandleSpan::from_parts(psi_arena::Handle::from_arena_index(999), 1);
            }
        });
    validate_content_partition_result_rewrites(&program, &row);
}

#[test]
#[should_panic(expected = "paths must not retain a runtime index")]
fn content_partition_result_rewrite_manifest_rejects_runtime_event_path() {
    let mut program = content_partition_result_rewrite_validation_fixture();
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
            if event.claim_identity == row.result_rewrites[0].claim_identity {
                event.segments = runtime_path;
            }
        });
    validate_content_partition_result_rewrites(&program, &row);
}

#[test]
#[should_panic(expected = "must name one exact checked outcome map")]
fn content_partition_result_rewrite_manifest_rejects_missing_outcome_map() {
    let mut program = content_partition_result_rewrite_validation_fixture();
    let row = program.facts.qualifications.content.partition_compositions[0].clone();
    program.facts.flow.ownership.claim_outcome_maps.clear();
    validate_content_partition_result_rewrites(&program, &row);
}

#[test]
#[should_panic(expected = "must name exactly one checked outcome map")]
fn content_partition_result_rewrite_manifest_rejects_ambiguous_outcome_map() {
    let mut program = content_partition_result_rewrite_validation_fixture();
    let row = program.facts.qualifications.content.partition_compositions[0].clone();
    let duplicate = program
        .facts
        .flow
        .ownership
        .claim_outcome_maps
        .iter()
        .next()
        .expect("fixture outcome map")
        .1
        .clone();
    program
        .facts
        .flow
        .ownership
        .claim_outcome_maps
        .insert(duplicate);
    validate_content_partition_result_rewrites(&program, &row);
}

#[test]
#[should_panic(expected = "outcome map must retain an exact valid entry span")]
fn content_partition_result_rewrite_manifest_rejects_invalid_outcome_span() {
    let mut program = content_partition_result_rewrite_validation_fixture();
    let row = program.facts.qualifications.content.partition_compositions[0].clone();
    program
        .facts
        .flow
        .ownership
        .claim_outcome_maps
        .for_each_mut(|_, map| {
            map.entries =
                psi_arena::HandleSpan::from_parts(psi_arena::Handle::from_arena_index(999), 1);
        });
    validate_content_partition_result_rewrites(&program, &row);
}

#[test]
#[should_panic(expected = "must match one exact established outcome")]
fn content_partition_result_rewrite_manifest_rejects_output_path_mismatch() {
    let mut program = content_partition_result_rewrite_validation_fixture();
    let row = program.facts.qualifications.content.partition_compositions[0].clone();
    let mismatched = program
        .facts
        .flow
        .ownership
        .segments
        .insert_many([psi_facts::PlaceSegment::FixedIndex { index: 3 }]);
    let entries = program
        .facts
        .flow
        .ownership
        .claim_outcome_maps
        .iter()
        .next()
        .expect("fixture outcome map")
        .1
        .entries;
    program
        .facts
        .flow
        .ownership
        .claim_outcome_entries
        .span_mut(entries)
        .expect("fixture outcome entries")
        .iter_mut()
        .filter(|entry| {
            matches!(
                entry.source,
                FlowClaimOutcomeSource::Established { claim_identity, .. }
                    if claim_identity == row.result_rewrites[0].claim_identity
            )
        })
        .for_each(|entry| entry.output_segments = mismatched);
    validate_content_partition_result_rewrites(&program, &row);
}

#[test]
#[should_panic(expected = "must match one exact established outcome")]
fn content_partition_result_rewrite_manifest_rejects_ambiguous_outcome_entry() {
    let mut program = content_partition_result_rewrite_validation_fixture();
    let row = program.facts.qualifications.content.partition_compositions[0].clone();
    let entry = {
        let entries = program
            .facts
            .flow
            .ownership
            .claim_outcome_maps
            .iter()
            .next()
            .expect("fixture outcome map")
            .1
            .entries;
        program
            .facts
            .flow
            .ownership
            .claim_outcome_entries
            .span(entries)
            .expect("fixture outcome entries")
            .iter()
            .find(|entry| {
                matches!(
                    entry.source,
                    FlowClaimOutcomeSource::Established { claim_identity, .. }
                        if claim_identity == row.result_rewrites[0].claim_identity
                )
            })
            .expect("fixture result outcome")
            .clone()
    };
    append_result_outcome_entry(&mut program, entry);
    validate_content_partition_result_rewrites(&program, &row);
}

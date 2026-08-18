use super::*;

#[test]
fn claim_outcome_manifest_keeps_paths_and_source_kinds_structured() {
    let machine_symbol = SymbolHandle::from_arena_index(20);
    let state_symbol = SymbolHandle::from_arena_index(21);
    let projection_machine_symbol = SymbolHandle::from_arena_index(22);
    let projection_state_symbol = SymbolHandle::from_arena_index(23);
    let domain_symbol = SymbolHandle::from_arena_index(24);
    let carrier_symbol = SymbolHandle::from_arena_index(25);
    let parameter_symbol = SymbolHandle::from_arena_index(26);
    let local_symbol = SymbolHandle::from_arena_index(27);
    let mut program = CheckedTrees::default();
    let mut machine = Machine {
        symbol: machine_symbol,
        name: Identifier::generated("Region::partition"),
        ..Default::default()
    };
    let mut state = State {
        symbol: state_symbol,
        name: Identifier::generated("entry"),
        ..Default::default()
    };
    program.typed.push_state_parameter(
        &mut state,
        StateParameter {
            symbol: parameter_symbol,
            name: Identifier::generated("region"),
            ..Default::default()
        },
    );
    for _ in 0..4 {
        program
            .typed
            .statement_table
            .push_statement(&mut state.statement_nodes, Default::default());
    }
    program.typed.statement_table.push_statement(
        &mut state.statement_nodes,
        StatementNode::LocalData(psi_typed_trees::statement::TableLocalData {
            symbol: local_symbol,
            name: Identifier::generated("partitioned"),
            ..Default::default()
        }),
    );
    program.typed.push_machine_state(&mut machine, state);
    program.typed.push_machine(machine);
    let mut projection_machine = Machine {
        symbol: projection_machine_symbol,
        name: Identifier::generated("Region::content"),
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
    let carrier = program
        .typed
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: carrier_symbol,
            name: Identifier::generated("Region"),
        });
    let semantic_domain = program
        .typed
        .semantic_domains
        .intern("Region::PartitionedContent");
    program.typed.push_domain_definition(DomainDefinition {
        symbol: domain_symbol,
        name: Identifier::generated("Region::PartitionedContent"),
        target_type: carrier,
        semantic_id: semantic_domain,
        ..Default::default()
    });
    let output_segments = program.facts.flow.ownership.segments.insert_many([
        psi_facts::PlaceSegment::Case {
            variant: SymbolHandle::invalid(),
        },
        psi_facts::PlaceSegment::Field {
            symbol: SymbolHandle::invalid(),
        },
    ]);
    let input_identity = PermissionClaimIdentity::Established {
        machine_symbol,
        state_symbol,
        source: PermissionEventSource::StateEntry,
        ordinal: 6,
    };
    let input_provenance = PermissionProvenance::Established {
        machine_symbol,
        state_symbol,
        source: PermissionEventSource::StateEntry,
    };
    program
        .facts
        .flow
        .ownership
        .permissions
        .insert(FlowPermissionEventFact {
            machine_symbol,
            state_symbol,
            source: PermissionEventSource::StateEntry,
            kind: PermissionEventKind::Establish,
            access: PermissionAccess::Owned,
            claim_identity: input_identity,
            provenance: input_provenance,
            root: PlaceRoot::Symbol(parameter_symbol),
            obligation_live: true,
            ..Default::default()
        });
    let established_identity = PermissionClaimIdentity::Established {
        machine_symbol,
        state_symbol,
        source: PermissionEventSource::Statement { statement_index: 2 },
        ordinal: 7,
    };
    let established_provenance = PermissionProvenance::Established {
        machine_symbol,
        state_symbol,
        source: PermissionEventSource::StateEntry,
    };
    program
        .facts
        .flow
        .ownership
        .permissions
        .insert(FlowPermissionEventFact {
            machine_symbol,
            state_symbol,
            source: PermissionEventSource::Statement { statement_index: 2 },
            kind: PermissionEventKind::Transfer,
            access: PermissionAccess::Owned,
            claim_identity: established_identity,
            provenance: established_provenance,
            root: PlaceRoot::Symbol(state_symbol),
            obligation_live: true,
            ..Default::default()
        });
    let result_identity = PermissionClaimIdentity::Established {
        machine_symbol,
        state_symbol,
        source: PermissionEventSource::Call {
            statement_index: 4,
            call_ordinal: 2,
            target_symbol: state_symbol,
        },
        ordinal: 12,
    };
    let result_provenance = PermissionProvenance::Established {
        machine_symbol,
        state_symbol,
        source: PermissionEventSource::Call {
            statement_index: 4,
            call_ordinal: 2,
            target_symbol: state_symbol,
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
                statement_index: 4,
                call_ordinal: 2,
                target_symbol: state_symbol,
            },
            kind: PermissionEventKind::Establish,
            access: PermissionAccess::Owned,
            claim_identity: result_identity,
            provenance: result_provenance,
            root: PlaceRoot::Symbol(local_symbol),
            obligation_live: true,
            ..Default::default()
        });
    let result_output_segments = program
        .facts
        .flow
        .ownership
        .segments
        .insert_many([psi_facts::PlaceSegment::FixedIndex { index: 2 }]);
    let entries = program
        .facts
        .flow
        .ownership
        .claim_outcome_entries
        .insert_many([
            FlowClaimOutcomeEntryFact {
                output_segments: Default::default(),
                source: FlowClaimOutcomeSource::Input {
                    parameter_symbol,
                    segments: Default::default(),
                },
            },
            FlowClaimOutcomeEntryFact {
                output_segments,
                source: FlowClaimOutcomeSource::Established {
                    claim_identity: established_identity,
                    provenance: established_provenance,
                },
            },
            FlowClaimOutcomeEntryFact {
                output_segments: result_output_segments,
                source: FlowClaimOutcomeSource::Established {
                    claim_identity: result_identity,
                    provenance: result_provenance,
                },
            },
        ]);
    program
        .facts
        .flow
        .ownership
        .claim_outcome_maps
        .insert(FlowClaimOutcomeMapFact {
            machine_symbol,
            state_symbol,
            entries,
        });
    let projection_algebra = ContentAlgebraIdentity::CountedQuantity {
        unit: "named(name(ByteUnit))".to_owned(),
    };
    let projection_expression = ContentProjectionExpression::CountedQuantity {
        magnitude: ContentScalarExpression::Arithmetic {
            operator: ContentArithmeticOperator::Add,
            left: Box::new(ContentScalarExpression::RuntimeScalarEmbedding(vec![
                ContentFieldSegment {
                    symbol: SymbolHandle::invalid(),
                    name: "length".to_owned(),
                },
            ])),
            right: Box::new(ContentScalarExpression::Natural("1".to_owned())),
        },
    };
    let projection_identity = projection_fingerprint(&projection_algebra, &projection_expression);
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
            machine: projection_machine_symbol,
            algebra: projection_algebra,
            expression: projection_expression,
            fingerprint: projection_identity,
        });
    let input = ContentConservationTerm::Projection {
        domain: domain_symbol,
        semantic_domain,
        projection_machine: projection_machine_symbol,
        projection_fingerprint: projection_identity,
        subject: ContentStructuralPlace {
            version: ContentPlaceVersion::Entry,
            root: ContentPlaceRoot::Parameter {
                position: 0,
                symbol: parameter_symbol,
                name: "region".to_owned(),
                is_self: false,
            },
            segments: Vec::new(),
        },
    };
    let output = ContentConservationTerm::Projection {
        domain: domain_symbol,
        semantic_domain,
        projection_machine: projection_machine_symbol,
        projection_fingerprint: projection_identity,
        subject: ContentStructuralPlace {
            version: ContentPlaceVersion::Current,
            root: ContentPlaceRoot::Result,
            segments: Vec::new(),
        },
    };
    let algebra = ContentAlgebraIdentity::CountedQuantity {
        unit: "named(name(ByteUnit))".to_owned(),
    };
    let ContentConservationTerm::Projection {
        subject: input_subject,
        ..
    } = &input
    else {
        unreachable!("fixture input is a projection")
    };
    let ContentConservationTerm::Projection {
        subject: output_subject,
        ..
    } = &output
    else {
        unreachable!("fixture output is a projection")
    };
    let substitutions = vec![
        ContentPartitionPlaceSubstitution {
            source: input_subject.clone(),
            target: input_subject.clone(),
        },
        ContentPartitionPlaceSubstitution {
            source: output_subject.clone(),
            target: ContentStructuralPlace {
                segments: vec![psi_language_semantics::content::ContentPlaceSegment::FixedIndex(2)],
                ..output_subject.clone()
            },
        },
    ];
    let result_rewrite = ContentPartitionResultRewrite {
        claim_identity: result_identity,
        source: substitutions[1].source.clone(),
        target: substitutions[1].target.clone(),
    };
    let partition_entry_place = input_subject.clone();
    let equation = ContentConservationEquation::new(input.clone(), output.clone());
    let fingerprint = conservation_fingerprint(&algebra, &equation);
    let plan = ContentConservationPlan {
        owner_kind: ContentConservationOwnerKind::Machine,
        owner: machine_symbol,
        callable: state_symbol,
        algebra,
        equation,
        fingerprint,
    };
    let source_partition_equation = ContentConservationEquation::new(
        ContentConservationTerm::Separate(vec![input.clone(), output.clone()]),
        ContentConservationTerm::Separate(vec![input.clone(), output.clone()]),
    );
    let source_partition_plan = ContentConservationPlan {
        owner_kind: ContentConservationOwnerKind::Machine,
        owner: machine_symbol,
        callable: state_symbol,
        algebra: plan.algebra.clone(),
        fingerprint: conservation_fingerprint(&plan.algebra, &source_partition_equation),
        equation: source_partition_equation,
    };
    let derived_output = ContentConservationTerm::Projection {
        domain: domain_symbol,
        semantic_domain,
        projection_machine: projection_machine_symbol,
        projection_fingerprint: projection_identity,
        subject: substitutions[1].target.clone(),
    };
    let derived_partition_equation = ContentConservationEquation::new(
        ContentConservationTerm::Separate(vec![input.clone(), derived_output.clone()]),
        ContentConservationTerm::Separate(vec![input, derived_output]),
    );
    let derived_partition_plan = ContentConservationPlan {
        owner_kind: ContentConservationOwnerKind::Machine,
        owner: machine_symbol,
        callable: state_symbol,
        algebra: plan.algebra.clone(),
        fingerprint: conservation_fingerprint(&plan.algebra, &derived_partition_equation),
        equation: derived_partition_equation,
    };
    program
        .facts
        .qualifications
        .content
        .identity_reshuffles
        .push(ContentIdentityReshuffleFact {
            machine_symbol,
            state_symbol,
            claim_identity: input_identity,
            input_parameter_symbol: parameter_symbol,
            input_segments: Default::default(),
            output_segments: Default::default(),
            plan: plan.clone(),
        });
    program
        .facts
        .qualifications
        .content
        .conservation_plans
        .push(source_partition_plan.clone());
    let calls = program.facts.flow.control.calls.insert_many([FlowCallFact {
        statement_index: 4,
        call_ordinal: 2,
        target_symbol: state_symbol,
        ..Default::default()
    }]);
    program.facts.flow.control.states.insert(FlowStateFact {
        machine_symbol,
        state_symbol,
        calls,
        ..Default::default()
    });
    program
        .facts
        .qualifications
        .content
        .partition_compositions
        .push(ContentPartitionCompositionFact {
            machine_symbol,
            state_symbol,
            source_callable: state_symbol,
            source_fingerprint: source_partition_plan.fingerprint,
            source_derivation_depth: 0,
            source_plan: source_partition_plan,
            statement_index: 4,
            call_ordinal: 2,
            input_claim_identities: vec![input_identity],
            input_claim_bindings: vec![psi_checked_trees::ContentPartitionInputClaimBinding {
                claim_identity: input_identity,
                entry_place: partition_entry_place,
            }],
            result_rewrites: vec![result_rewrite],
            substitutions,
            plan: derived_partition_plan,
        });

    let json = claim_outcome_manifest_json(&program);
    let claim_maps = &json[..json
        .find("\"content_projections\"")
        .expect("content projection section")];
    let projections_start = claim_maps.len();
    let projections_end = json
        .find("\"content_identity_reshuffles\"")
        .expect("identity reshuffle section");
    let projections = &json[projections_start..projections_end];
    let reshuffles_start = json
        .find("\"content_identity_reshuffles\"")
        .expect("identity reshuffle section");
    let reshuffles_end = json
        .find("\"content_partition_compositions\"")
        .expect("partition composition section");
    let reshuffles = &json[reshuffles_start..reshuffles_end];
    let compositions_start = reshuffles_end;
    let compositions_end = json
        .find("\"content_conservation\"")
        .expect("content conservation section");
    let compositions = &json[compositions_start..compositions_end];
    let conservation = &json[compositions_end..];

    assert!(json.contains("\"claim_outcome_maps\""));
    assert!(
        claim_maps
            .contains("\"machine_overload_identity\": \"named-callable(path(Region::partition)")
    );
    assert!(json.contains("\"output_path\": [{\"case\": \"invalid\"}, {\"field\": \"invalid\"}]"));
    assert!(json.contains("\"kind\": \"input\""));
    assert!(json.contains("\"kind\": \"established\""));
    assert!(json.contains("\"statement_index\": 2"));
    assert!(json.contains("\"ordinal\": 7"));
    assert!(json.contains("\"kind\": \"state_entry\""));
    assert!(json.contains("\"content_projections\""));
    assert!(projections.contains(
        "\"projection_machine_overload_identity\": \"named-callable(path(Region::content)"
    ));
    assert!(json.contains("\"content_identity_reshuffles\": [\n    {"));
    assert!(
        reshuffles
            .contains("\"machine_overload_identity\": \"named-callable(path(Region::partition)")
    );
    assert!(json.contains("\"content_partition_compositions\": [\n    {"));
    assert!(
        compositions
            .contains("\"machine_overload_identity\": \"named-callable(path(Region::partition)")
    );
    assert!(compositions.contains(
        "\"source_callable_overload_identity\": \"named-callable(path(Region::partition)"
    ));
    assert!(json.contains("\"source_derivation_depth\": 0"));
    assert!(json.contains("\"source_equation\": {\"left\":"));
    assert!(json.contains("\"substitutions\": [{\"source\": {\"version\": \"entry\""));
    assert!(json.contains("\"call\": {\"statement_index\": 4, \"call_ordinal\": 2}"));
    assert!(json.contains("\"input_claim_identities\": [{\"kind\": \"established\""));
    assert!(
        json.contains("\"input_claim_bindings\": [{\"claim_identity\": {\"kind\": \"established\"")
    );
    assert!(json.contains("\"entry_place\": {\"version\": \"entry\""));
    assert!(json.contains("\"result_rewrites\": [{\"claim_identity\": {\"kind\": \"established\""));
    assert!(json.contains("\"source\": {\"version\": \"current\""));
    assert!(json.contains("\"target\": {\"version\": \"current\""));
    assert!(json.contains("\"ordinal\": 12"));
    assert!(json.contains(&format!(
        "\"input\": {{\"parameter\": \"{}\", \"path\": []}}",
        symbol_label(&program, parameter_symbol)
    )));
    assert!(json.contains("\"ordinal\": 6"));
    assert!(json.contains(&format!("\"semantic_domain_id\": {}", semantic_domain.0)));
    assert!(json.contains("\"kind\": \"counted_quantity\""));
    assert!(json.contains("\"unit\": \"named(name(ByteUnit))\""));
    assert!(json.contains("\"kind\": \"runtime_scalar_embedding\""));
    assert!(json.contains("\"path\": [\"length\"]"));
    assert!(json.contains("\"operator\": \"add\""));
    assert!(json.contains(&format!(
        "\"fingerprint\": \"0x{projection_identity:016x}\""
    )));
    assert!(
        conservation
            .contains("\"callable_overload_identity\": \"named-callable(path(Region::partition)")
    );
}

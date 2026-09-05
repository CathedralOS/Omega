use super::*;

fn semantic_domain_commitment_fixture() -> (
    CheckedTrees,
    SymbolHandle,
    SymbolHandle,
    SemanticDomainId,
    SemanticDomainId,
) {
    let first_machine = SymbolHandle::from_arena_index(80);
    let second_machine = SymbolHandle::from_arena_index(81);
    let mut program = CheckedTrees::default();
    for (symbol, state_symbol, name) in [
        (
            first_machine,
            SymbolHandle::from_arena_index(82),
            "DistanceWorker::run",
        ),
        (
            second_machine,
            SymbolHandle::from_arena_index(83),
            "AuditWorker::run",
        ),
    ] {
        let mut machine = Machine {
            symbol,
            name: Identifier::generated(name),
            ..Default::default()
        };
        program.typed.push_machine_state(
            &mut machine,
            State {
                symbol: state_symbol,
                name: Identifier::generated("entry"),
                ..Default::default()
            },
        );
        program.typed.push_machine(machine);
    }
    let distance = program.typed.semantic_domains.intern("i64::Distance<1000>");
    let wrapping = program.typed.semantic_domains.intern("i64::Wrapping");
    (program, first_machine, second_machine, distance, wrapping)
}

#[test]
fn qualification_manifest_publishes_ordered_exact_machine_domain_commitments() {
    let (mut program, first_machine, second_machine, distance, wrapping) =
        semantic_domain_commitment_fixture();
    program
        .facts
        .qualifications
        .machines
        .push(MachineQualifications {
            machine: first_machine,
            body_committed: vec![distance, wrapping],
        });
    program
        .facts
        .qualifications
        .machines
        .push(MachineQualifications {
            machine: second_machine,
            body_committed: vec![distance],
        });

    let json = qualification_evidence_manifest_json(
        &program,
        &effects::SelectedProviderPlanFacts::default(),
    );

    let commitments = json
        .split_once("\"machine_semantic_domain_commitments\": [")
        .expect("qualification artifact publishes implementation commitments")
        .1
        .split_once("\"vacuous_qualification_uses\": [")
        .expect("commitments remain independent from vacuous evidence")
        .0;
    assert!(commitments.contains("\"machine\": \"#80\""));
    assert!(commitments.contains("\"machine\": \"#81\""));
    assert!(commitments.contains("\"machine_overload_identity\":"));
    assert_eq!(
        commitments
            .matches(&format!("\"semantic_domain_id\": {}", distance.0))
            .count(),
        2,
        "the same normalized domain may be committed independently by two machines"
    );
    let first_distance = commitments
        .find(&format!("\"semantic_domain_id\": {}", distance.0))
        .expect("first ordered domain");
    let first_wrapping = commitments
        .find(&format!("\"semantic_domain_id\": {}", wrapping.0))
        .expect("second ordered domain");
    assert!(first_distance < first_wrapping);
    assert!(commitments.contains("\"semantic_domain\": \"i64::Distance<1000>\""));
    assert!(commitments.contains("\"semantic_domain\": \"i64::Wrapping\""));
}

#[test]
#[should_panic(expected = "must name an exact owning machine")]
fn qualification_manifest_rejects_missing_commitment_machine() {
    let (mut program, _, _, distance, _) = semantic_domain_commitment_fixture();
    program
        .facts
        .qualifications
        .machines
        .push(MachineQualifications {
            machine: SymbolHandle::from_arena_index(99),
            body_committed: vec![distance],
        });
    validated_machine_semantic_domain_commitments(&program);
}

#[test]
#[should_panic(expected = "one row per exact machine")]
fn qualification_manifest_rejects_duplicate_commitment_machine() {
    let (mut program, machine, _, distance, wrapping) = semantic_domain_commitment_fixture();
    for domain in [distance, wrapping] {
        program
            .facts
            .qualifications
            .machines
            .push(MachineQualifications {
                machine,
                body_committed: vec![domain],
            });
    }
    validated_machine_semantic_domain_commitments(&program);
}

#[test]
#[should_panic(expected = "must retain at least one domain")]
fn qualification_manifest_rejects_empty_commitment_domains() {
    let (mut program, machine, _, _, _) = semantic_domain_commitment_fixture();
    program
        .facts
        .qualifications
        .machines
        .push(MachineQualifications {
            machine,
            body_committed: Vec::new(),
        });
    validated_machine_semantic_domain_commitments(&program);
}

#[test]
#[should_panic(expected = "must be strictly increasing")]
fn qualification_manifest_rejects_duplicate_commitment_domains() {
    let (mut program, machine, _, distance, _) = semantic_domain_commitment_fixture();
    program
        .facts
        .qualifications
        .machines
        .push(MachineQualifications {
            machine,
            body_committed: vec![distance, distance],
        });
    validated_machine_semantic_domain_commitments(&program);
}

#[test]
#[should_panic(expected = "must be strictly increasing")]
fn qualification_manifest_rejects_out_of_order_commitment_domains() {
    let (mut program, machine, _, distance, wrapping) = semantic_domain_commitment_fixture();
    let (higher, lower) = if distance.0 > wrapping.0 {
        (distance, wrapping)
    } else {
        (wrapping, distance)
    };
    program
        .facts
        .qualifications
        .machines
        .push(MachineQualifications {
            machine,
            body_committed: vec![higher, lower],
        });
    validated_machine_semantic_domain_commitments(&program);
}

#[test]
#[should_panic(expected = "must name a registered domain")]
fn qualification_manifest_rejects_unknown_commitment_domain() {
    let (mut program, machine, _, _, _) = semantic_domain_commitment_fixture();
    program
        .facts
        .qualifications
        .machines
        .push(MachineQualifications {
            machine,
            body_committed: vec![SemanticDomainId(u32::MAX)],
        });
    validated_machine_semantic_domain_commitments(&program);
}

#[test]
fn qualification_evidence_manifest_separates_origin_point_and_receipt() {
    let subject = SymbolHandle::from_arena_index(4);
    let domain = SymbolHandle::from_arena_index(5);
    let machine_symbol = SymbolHandle::from_arena_index(80);
    let state_symbol = SymbolHandle::from_arena_index(81);
    let plan = selected_storage_plan();
    assert_ne!(plan.schema.trait_name, "StorageBase");
    let receipt_identity = plan.report_fingerprint();
    let selected = effects::SelectedProviderPlanFacts::from_selection(
        std::slice::from_ref(&plan),
        std::slice::from_ref(&plan.name),
    )
    .expect("complete selected provider plan");
    let mut program = CheckedTrees::default();
    program.typed.push_domain_definition(DomainDefinition {
        symbol: domain,
        name: Identifier::generated("Storage::Qualified"),
        ..Default::default()
    });
    let mut machine = Machine {
        symbol: machine_symbol,
        name: Identifier::generated("StorageCaller::run"),
        ..Default::default()
    };
    let mut state = State {
        symbol: state_symbol,
        name: Identifier::generated("run"),
        ..Default::default()
    };
    for _ in 0..3 {
        program
            .typed
            .statement_table
            .push_statement(&mut state.statement_nodes, Default::default());
    }
    program.typed.push_machine_state(&mut machine, state);
    program.typed.push_machine(machine);
    let mut calls = Default::default();
    program.facts.flow.control.calls.append_to_span(
        &mut calls,
        FlowCallFact {
            statement_index: 2,
            call_ordinal: 1,
            ..Default::default()
        },
    );
    program.facts.flow.control.states.append(FlowStateFact {
        machine_symbol,
        state_symbol,
        calls,
        ..Default::default()
    });
    let (requirement_owner, requirement) =
        push_qualification_requirement(&mut program, true, 70, 71, "StorageBase");
    let place = program.facts.semantic.append_symbol_place(subject);
    program.facts.semantic.append_fact(Fact {
        place: FactPlace::Place(place),
        point: ProgramPoint::CallEnsures {
            machine_symbol,
            state_symbol,
            statement_index: 2,
            call_ordinal: 1,
        },
        origin: FactOrigin::CallEnsures,
        evidence: QualificationEvidence {
            origin: QualificationEvidenceOrigin::AdmittedReceipt,
            source_symbol: requirement_owner,
            requirement_symbol: requirement,
            receipt_identity,
        },
        payload: FactPayload::DomainMembership {
            value: Default::default(),
            domain: Default::default(),
            domain_symbol: domain,
        },
    });
    let unstamped_subject = SymbolHandle::from_arena_index(7);
    let unstamped_place = program
        .facts
        .semantic
        .append_symbol_place(unstamped_subject);
    program.facts.semantic.append_fact(Fact {
        place: FactPlace::Place(unstamped_place),
        point: ProgramPoint::Global,
        origin: FactOrigin::CallEnsures,
        evidence: QualificationEvidence {
            origin: QualificationEvidenceOrigin::AdmittedReceipt,
            source_symbol: requirement_owner,
            requirement_symbol: requirement,
            receipt_identity: 0,
        },
        payload: FactPayload::DomainMembership {
            value: Default::default(),
            domain: Default::default(),
            domain_symbol: domain,
        },
    });

    let json = qualification_evidence_manifest_json(&program, &selected);

    assert!(json.contains(&format!(
        "\"selected_provider_closure_report_fingerprint\": \"0x{:016x}\"",
        selected.compatibility_report_identity()
    )));
    assert!(json.contains("\"subject\": \"#4\""));
    assert!(json.contains("\"domain\": \"#5\""));
    assert!(json.contains("\"origin\": \"admitted_receipt\""));
    assert!(json.contains("\"program_point\": \"call_ensures\""));
    assert!(json.contains("\"program_point_identity\": \"#81:call-ensures-2-1\""));
    assert!(json.contains("\"program_point_identity\": \"global\""));
    assert!(json.contains("\"source\": \"#70\""));
    assert!(json.contains("\"requirement\": \"#71\""));
    assert!(
        json.contains("\"requirement_identity\": \"named-callable(path(StorageBase::transfer)")
    );
    assert!(json.contains(&format!(
        "\"receipt_identity\": \"0x{receipt_identity:016x}\""
    )));
    assert!(json.contains("\"receipt_identity\": null"));
}

#[test]
#[should_panic(expected = "qualification evidence must name an exact declared domain")]
fn qualification_manifest_rejects_missing_declared_domain() {
    let subject = SymbolHandle::from_arena_index(4);
    let domain = SymbolHandle::from_arena_index(5);
    let mut program = CheckedTrees::default();
    let place = program.facts.semantic.append_symbol_place(subject);
    program.facts.semantic.append_fact(Fact {
        place: FactPlace::Place(place),
        point: ProgramPoint::Global,
        origin: FactOrigin::MachineFieldDomain {
            machine_symbol: subject,
        },
        evidence: QualificationEvidence::from_origin(
            QualificationEvidenceOrigin::CheckedValidation,
            subject,
        ),
        payload: FactPayload::DomainMembership {
            value: Default::default(),
            domain: Default::default(),
            domain_symbol: domain,
        },
    });

    qualification_evidence_manifest_json(&program, &effects::SelectedProviderPlanFacts::default());
}

#[test]
#[should_panic(expected = "must retain a semantic subject position")]
fn qualification_manifest_rejects_unknown_subject() {
    qualification_subject(
        &CheckedTrees::default(),
        &Fact {
            place: FactPlace::Unknown,
            ..Default::default()
        },
    );
}

#[test]
#[should_panic(expected = "must retain a semantic subject position")]
fn qualification_manifest_rejects_unknown_place_root() {
    let mut program = CheckedTrees::default();
    let place = program.facts.semantic.append_place(Place {
        root: PlaceRoot::Unknown,
        segments: Default::default(),
    });
    qualification_subject(
        &program,
        &Fact {
            place: FactPlace::Place(place),
            ..Default::default()
        },
    );
}

#[test]
#[should_panic(expected = "program point must name an exact typed machine")]
fn qualification_manifest_rejects_missing_program_point_machine() {
    validate_qualification_program_point(
        &CheckedTrees::default(),
        ProgramPoint::Machine {
            machine_symbol: SymbolHandle::from_arena_index(80),
        },
    );
}

#[test]
#[should_panic(expected = "program point state must belong to its exact typed machine")]
fn qualification_manifest_rejects_cross_machine_program_point_state() {
    let machine_symbol = SymbolHandle::from_arena_index(80);
    let other_machine_symbol = SymbolHandle::from_arena_index(82);
    let other_state_symbol = SymbolHandle::from_arena_index(83);
    let mut program = CheckedTrees::default();
    for (symbol, state) in [
        (machine_symbol, SymbolHandle::from_arena_index(81)),
        (other_machine_symbol, other_state_symbol),
    ] {
        let mut machine = Machine {
            symbol,
            name: Identifier::generated("Worker::run"),
            ..Default::default()
        };
        program.typed.push_machine_state(
            &mut machine,
            State {
                symbol: state,
                name: Identifier::generated("run"),
                ..Default::default()
            },
        );
        program.typed.push_machine(machine);
    }

    validate_qualification_program_point(
        &program,
        ProgramPoint::State {
            machine_symbol,
            state_symbol: other_state_symbol,
        },
    );
}

#[test]
#[should_panic(expected = "statement index must be within its exact typed state")]
fn qualification_manifest_rejects_out_of_range_program_point_statement() {
    let machine_symbol = SymbolHandle::from_arena_index(80);
    let state_symbol = SymbolHandle::from_arena_index(81);
    let mut program = CheckedTrees::default();
    let mut machine = Machine {
        symbol: machine_symbol,
        name: Identifier::generated("Worker::run"),
        ..Default::default()
    };
    let mut state = State {
        symbol: state_symbol,
        name: Identifier::generated("run"),
        ..Default::default()
    };
    program
        .typed
        .statement_table
        .push_statement(&mut state.statement_nodes, Default::default());
    program.typed.push_machine_state(&mut machine, state);
    program.typed.push_machine(machine);

    validate_qualification_program_point(
        &program,
        ProgramPoint::Statement {
            machine_symbol,
            state_symbol,
            statement_index: 1,
        },
    );
}

#[test]
#[should_panic(expected = "call point must name an exact checked flow state")]
fn qualification_manifest_rejects_call_without_flow_state() {
    let machine_symbol = SymbolHandle::from_arena_index(80);
    let state_symbol = SymbolHandle::from_arena_index(81);
    let mut program = CheckedTrees::default();
    let mut machine = Machine {
        symbol: machine_symbol,
        name: Identifier::generated("Worker::run"),
        ..Default::default()
    };
    let mut state = State {
        symbol: state_symbol,
        name: Identifier::generated("run"),
        ..Default::default()
    };
    program
        .typed
        .statement_table
        .push_statement(&mut state.statement_nodes, Default::default());
    program.typed.push_machine_state(&mut machine, state);
    program.typed.push_machine(machine);

    validate_qualification_program_point(
        &program,
        ProgramPoint::Call {
            machine_symbol,
            state_symbol,
            statement_index: 0,
            call_ordinal: 0,
        },
    );
}

#[test]
#[should_panic(expected = "call point must name an exact owned checked flow call")]
fn qualification_manifest_rejects_wrong_call_ordinal() {
    let machine_symbol = SymbolHandle::from_arena_index(80);
    let state_symbol = SymbolHandle::from_arena_index(81);
    let mut program = CheckedTrees::default();
    let mut machine = Machine {
        symbol: machine_symbol,
        name: Identifier::generated("Worker::run"),
        ..Default::default()
    };
    let mut state = State {
        symbol: state_symbol,
        name: Identifier::generated("run"),
        ..Default::default()
    };
    program
        .typed
        .statement_table
        .push_statement(&mut state.statement_nodes, Default::default());
    program.typed.push_machine_state(&mut machine, state);
    program.typed.push_machine(machine);
    let mut calls = Default::default();
    program.facts.flow.control.calls.append_to_span(
        &mut calls,
        FlowCallFact {
            statement_index: 0,
            call_ordinal: 1,
            ..Default::default()
        },
    );
    program.facts.flow.control.states.append(FlowStateFact {
        machine_symbol,
        state_symbol,
        calls,
        ..Default::default()
    });

    validate_qualification_program_point(
        &program,
        ProgramPoint::CallEnsures {
            machine_symbol,
            state_symbol,
            statement_index: 0,
            call_ordinal: 2,
        },
    );
}

#[test]
fn qualification_manifest_accepts_exact_independent_semantic_sources() {
    let machine_symbol = SymbolHandle::from_arena_index(80);
    let state_symbol = SymbolHandle::from_arena_index(81);
    let root_operator_symbol = SymbolHandle::from_arena_index(82);
    let domain_symbol = SymbolHandle::from_arena_index(83);
    let domain_operator_symbol = SymbolHandle::from_arena_index(84);
    let trait_symbol = SymbolHandle::from_arena_index(85);
    let parameter_symbol = SymbolHandle::from_arena_index(86);
    let parameter_signature_symbol = SymbolHandle::from_arena_index(87);
    let mut program = CheckedTrees::default();

    let mut machine = Machine {
        symbol: machine_symbol,
        name: Identifier::generated("Worker::run"),
        ..Default::default()
    };
    program.typed.push_machine_state(
        &mut machine,
        State {
            symbol: state_symbol,
            name: Identifier::generated("run"),
            ..Default::default()
        },
    );
    program.typed.push_machine_type_parameter(
        &mut machine,
        TypeParameter {
            symbol: parameter_symbol,
            name: Identifier::generated("Dependency"),
            kind: TypeParameterKind::Machine {
                contract: MachineParameterContract::Structural(StateSignature {
                    symbol: parameter_signature_symbol,
                    name: Identifier::generated("invoke"),
                    ..Default::default()
                }),
            },
            ..Default::default()
        },
    );
    program.typed.push_machine(machine);
    program.typed.push_operator(OperatorDefinition {
        symbol: root_operator_symbol,
        ..Default::default()
    });
    let mut domain = DomainDefinition {
        symbol: domain_symbol,
        name: Identifier::generated("Validated"),
        ..Default::default()
    };
    program.typed.push_domain_operator(
        &mut domain,
        OperatorDefinition {
            symbol: domain_operator_symbol,
            ..Default::default()
        },
    );
    program.typed.push_domain_definition(domain);
    program.typed.push_trait_definition(TraitDefinition {
        symbol: trait_symbol,
        name: Identifier::generated("Transform"),
        ..Default::default()
    });

    for (source_symbol, origin) in [
        (
            machine_symbol,
            QualificationEvidenceOrigin::CheckedValidation,
        ),
        (state_symbol, QualificationEvidenceOrigin::Prover),
        (
            root_operator_symbol,
            QualificationEvidenceOrigin::CheckedTransformation,
        ),
        (
            domain_operator_symbol,
            QualificationEvidenceOrigin::Propagated,
        ),
        (
            trait_symbol,
            QualificationEvidenceOrigin::AuthorizedRouteEstablishment,
        ),
        (
            parameter_symbol,
            QualificationEvidenceOrigin::VacuousQualification,
        ),
        (
            parameter_signature_symbol,
            QualificationEvidenceOrigin::Prover,
        ),
    ] {
        validate_qualification_source(
            &program,
            &QualificationEvidence::from_origin(origin, source_symbol),
        );
    }
}

#[test]
fn qualification_manifest_keeps_admitted_source_on_requirement_pair_rule() {
    let mut program = CheckedTrees::default();
    let (requirement_owner, requirement) =
        push_qualification_requirement(&mut program, true, 70, 71, "StorageBase");

    validate_qualification_source(
        &program,
        &QualificationEvidence {
            origin: QualificationEvidenceOrigin::AdmittedReceipt,
            source_symbol: requirement_owner,
            requirement_symbol: requirement,
            ..Default::default()
        },
    );
}

#[test]
#[should_panic(expected = "must retain a nonempty exact source symbol")]
fn qualification_manifest_rejects_empty_non_admitted_source() {
    validate_qualification_source(
        &CheckedTrees::default(),
        &QualificationEvidence {
            origin: QualificationEvidenceOrigin::Prover,
            ..Default::default()
        },
    );
}

#[test]
#[should_panic(expected = "must resolve to exactly one retained typed semantic declaration")]
fn qualification_manifest_rejects_absent_non_admitted_source() {
    validate_qualification_source(
        &CheckedTrees::default(),
        &QualificationEvidence::from_origin(
            QualificationEvidenceOrigin::Prover,
            SymbolHandle::from_arena_index(80),
        ),
    );
}

#[test]
#[should_panic(expected = "must resolve to exactly one retained typed semantic declaration")]
fn qualification_manifest_rejects_ambiguous_non_admitted_source() {
    let source = SymbolHandle::from_arena_index(80);
    let mut program = CheckedTrees::default();
    program.typed.push_machine(Machine {
        symbol: source,
        name: Identifier::generated("Worker::run"),
        ..Default::default()
    });
    program.typed.push_trait_definition(TraitDefinition {
        symbol: source,
        name: Identifier::generated("Worker"),
        ..Default::default()
    });

    validate_qualification_source(
        &program,
        &QualificationEvidence::from_origin(QualificationEvidenceOrigin::Prover, source),
    );
}

#[test]
#[should_panic(expected = "must name an exact retained selected provider plan")]
fn qualification_manifest_rejects_unselected_nonzero_receipt() {
    validate_qualification_receipt(
        &effects::SelectedProviderPlanFacts::default(),
        QualificationEvidenceOrigin::AdmittedReceipt,
        selected_storage_plan().report_fingerprint(),
    );
}

#[test]
#[should_panic(expected = "must use admitted-receipt origin")]
fn qualification_manifest_rejects_selected_receipt_on_non_admitted_origin() {
    let plan = selected_storage_plan();
    let selected = effects::SelectedProviderPlanFacts::from_selection(
        std::slice::from_ref(&plan),
        std::slice::from_ref(&plan.name),
    )
    .expect("complete selected provider plan");
    validate_qualification_receipt(
        &selected,
        QualificationEvidenceOrigin::Prover,
        plan.report_fingerprint(),
    );
}

#[test]
#[should_panic(expected = "must name an exact boundary requirement")]
fn qualification_manifest_rejects_admitted_evidence_without_requirement() {
    qualification_requirement_identity(
        &CheckedTrees::default(),
        &QualificationEvidence {
            origin: QualificationEvidenceOrigin::AdmittedReceipt,
            ..Default::default()
        },
    );
}

#[test]
#[should_panic(expected = "must name an exact boundary requirement")]
fn qualification_manifest_rejects_admitted_ordinary_trait_requirement() {
    let mut program = CheckedTrees::default();
    let (requirement_owner, requirement) =
        push_qualification_requirement(&mut program, false, 70, 71, "StorageBase");
    qualification_requirement_identity(
        &program,
        &QualificationEvidence {
            origin: QualificationEvidenceOrigin::AdmittedReceipt,
            source_symbol: requirement_owner,
            requirement_symbol: requirement,
            ..Default::default()
        },
    );
}

#[test]
#[should_panic(expected = "must name an exact boundary requirement owner/signature pair")]
fn qualification_manifest_rejects_admitted_requirement_without_owner() {
    let mut program = CheckedTrees::default();
    let (_, requirement) =
        push_qualification_requirement(&mut program, true, 70, 71, "StorageBase");
    qualification_requirement_identity(
        &program,
        &QualificationEvidence {
            origin: QualificationEvidenceOrigin::AdmittedReceipt,
            requirement_symbol: requirement,
            ..Default::default()
        },
    );
}

#[test]
#[should_panic(expected = "must name an exact boundary requirement owner/signature pair")]
fn qualification_manifest_rejects_cross_owner_requirement() {
    let mut program = CheckedTrees::default();
    let (_, requirement) =
        push_qualification_requirement(&mut program, true, 70, 71, "StorageBase");
    let (unrelated_owner, _) =
        push_qualification_requirement(&mut program, true, 72, 73, "AuditBase");
    qualification_requirement_identity(
        &program,
        &QualificationEvidence {
            origin: QualificationEvidenceOrigin::AdmittedReceipt,
            source_symbol: unrelated_owner,
            requirement_symbol: requirement,
            ..Default::default()
        },
    );
}

#[test]
#[should_panic(expected = "non-admitted qualification evidence must not name")]
fn qualification_manifest_rejects_requirement_on_non_admitted_evidence() {
    let mut program = CheckedTrees::default();
    let (_, requirement) =
        push_qualification_requirement(&mut program, true, 70, 71, "StorageBase");
    qualification_requirement_identity(
        &program,
        &QualificationEvidence {
            origin: QualificationEvidenceOrigin::Prover,
            requirement_symbol: requirement,
            ..Default::default()
        },
    );
}

#[test]
fn qualification_manifest_retains_provider_origin_outside_plan_identity() {
    let plan = selected_storage_plan();
    let plan_identity = plan.report_fingerprint();
    let mut relocated = plan.clone();
    relocated.origin_package = "omega::providers::relocated".to_owned();
    assert_eq!(
        relocated.report_fingerprint(),
        plan_identity,
        "provider origin is provenance beside, not part of, plan identity"
    );
    let selected = effects::SelectedProviderPlanFacts::from_selection(
        std::slice::from_ref(&plan),
        std::slice::from_ref(&plan.name),
    )
    .expect("complete selected provider plan");
    let selected_closure_identity = selected.report_fingerprint();

    let json = qualification_evidence_manifest_json(&CheckedTrees::default(), &selected);
    assert!(json.contains(&format!(
        "\"selected_provider_closure_report_fingerprint\": \"0x{selected_closure_identity:016x}\""
    )));
    assert_eq!(json.matches("\"provider_origin_package\"").count(), 2);
    assert_eq!(
        json.matches("\"provider_origin_package\": \"omega::providers::storage\"")
            .count(),
        2
    );
    assert!(json.contains("\"flow\": \"accepts\""));
    assert!(json.contains("\"flow\": \"returns\""));
    assert_eq!(json.matches("\"boundary\": \"StorageRoot\"").count(), 2);
    assert_eq!(
        json.matches("\"requirement\": \"StorageBase::transfer\"")
            .count(),
        2
    );
    assert_eq!(
        json.matches("\"requirement_owner\": \"StorageBase\"")
            .count(),
        2
    );
    assert_eq!(
        json.matches("\"requirement_identity\": \"StorageBase::transfer\"")
            .count(),
        2
    );
    assert!(json.contains(&format!("\"receipt_identity\": \"0x{plan_identity:016x}\"")));

    let mut absent = plan.clone();
    absent.origin_package.clear();
    let selected_absent = effects::SelectedProviderPlanFacts::from_selection(
        std::slice::from_ref(&absent),
        std::slice::from_ref(&absent.name),
    )
    .expect("selected provider with explicitly absent origin");
    let absent_json =
        qualification_evidence_manifest_json(&CheckedTrees::default(), &selected_absent);
    assert_eq!(
        absent_json
            .matches("\"provider_origin_package\": null")
            .count(),
        2
    );
}

#[test]
fn qualification_manifest_retains_vacuous_use_owner_overload_identity() {
    let (
        mut program,
        machine_symbol,
        state_symbol,
        _,
        domain_symbol,
        semantic_domain,
        cast_expression,
        _,
    ) = vacuous_qualification_fixture();
    assert_ne!(
        program
            .domain_definitions()
            .iter()
            .find(|domain| domain.symbol == domain_symbol)
            .expect("declared domain")
            .semantic_id,
        semantic_domain,
        "the declared family and selected indexed instance remain independent",
    );
    program
        .facts
        .qualifications
        .vacuous_uses
        .push(VacuousQualificationUse {
            machine: machine_symbol,
            state: state_symbol,
            statement_index: 3,
            expression: cast_expression,
            domain: domain_symbol,
            semantic_domain,
        });

    let json = qualification_evidence_manifest_json(
        &program,
        &effects::SelectedProviderPlanFacts::default(),
    );

    assert!(json.contains("\"machine\": \"#60\""));
    assert!(json.contains("\"machine_overload_identity\": \"named-callable(path(Main::main)"));
    assert!(json.contains("\"statement_index\": 3"));
    assert!(json.contains(&format!("\"semantic_domain_id\": {}", semantic_domain.0)));
    assert!(json.contains("\"semantic_domain\": \"i64::Distance<1000>\""));
}

#[test]
#[should_panic(expected = "statement index must be within its exact state")]
fn qualification_manifest_rejects_out_of_range_vacuous_statement() {
    let (program, machine, state, _, domain, semantic_domain, cast_expression, _) =
        vacuous_qualification_fixture();
    validate_vacuous_qualification_use(
        &program,
        &VacuousQualificationUse {
            machine,
            state,
            statement_index: 4,
            expression: cast_expression,
            domain,
            semantic_domain,
        },
    );
}

#[test]
#[should_panic(expected = "must name a valid retained expression")]
fn qualification_manifest_rejects_invalid_vacuous_expression() {
    let (program, machine, state, _, domain, semantic_domain, _, _) =
        vacuous_qualification_fixture();
    validate_vacuous_qualification_use(
        &program,
        &VacuousQualificationUse {
            machine,
            state,
            statement_index: 3,
            expression: ExpressionHandle::invalid(),
            domain,
            semantic_domain,
        },
    );
}

#[test]
#[should_panic(expected = "must name its exact retained cast")]
fn qualification_manifest_rejects_reachable_non_cast_vacuous_expression() {
    let (program, machine, state, _, domain, semantic_domain, _, statement_expression) =
        vacuous_qualification_fixture();
    validate_vacuous_qualification_use(
        &program,
        &VacuousQualificationUse {
            machine,
            state,
            statement_index: 3,
            expression: statement_expression,
            domain,
            semantic_domain,
        },
    );
}

#[test]
#[should_panic(expected = "cast must belong to its exact statement")]
fn qualification_manifest_rejects_cross_statement_vacuous_cast() {
    let (program, machine, state, _, domain, semantic_domain, cast_expression, _) =
        vacuous_qualification_fixture();
    validate_vacuous_qualification_use(
        &program,
        &VacuousQualificationUse {
            machine,
            state,
            statement_index: 2,
            expression: cast_expression,
            domain,
            semantic_domain,
        },
    );
}

#[test]
#[should_panic(expected = "state must belong to its exact owning machine")]
fn qualification_manifest_rejects_cross_machine_vacuous_state() {
    let (program, machine, _, other_state, domain, semantic_domain, cast_expression, _) =
        vacuous_qualification_fixture();
    validate_vacuous_qualification_use(
        &program,
        &VacuousQualificationUse {
            machine,
            state: other_state,
            statement_index: 0,
            expression: cast_expression,
            domain,
            semantic_domain,
        },
    );
}

#[test]
#[should_panic(expected = "must name an exact declared domain")]
fn qualification_manifest_rejects_missing_vacuous_domain() {
    let (program, machine, state, _, _, semantic_domain, cast_expression, _) =
        vacuous_qualification_fixture();
    validate_vacuous_qualification_use(
        &program,
        &VacuousQualificationUse {
            machine,
            state,
            statement_index: 3,
            expression: cast_expression,
            domain: SymbolHandle::from_arena_index(99),
            semantic_domain,
        },
    );
}

#[test]
#[should_panic(expected = "must name a registered semantic-domain instance")]
fn qualification_manifest_rejects_unknown_vacuous_semantic_domain() {
    let (program, machine, state, _, domain, _, cast_expression, _) =
        vacuous_qualification_fixture();
    validate_vacuous_qualification_use(
        &program,
        &VacuousQualificationUse {
            machine,
            state,
            statement_index: 3,
            expression: cast_expression,
            domain,
            semantic_domain: SemanticDomainId(99),
        },
    );
}

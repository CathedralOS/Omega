use super::{
    TheoremSchemaMutation, TransportSchemaMutation, call_with_arguments, carrier_type,
    named_argument, quotient_type, request_with_representative, selected_theorem_schema_fixture,
    static_argument, symbol, transport_schema_fixture, verify_transport_fixture,
};
use crate::proof_contracts::quotients::relation_plan::correspondence_certificate::{
    QuotientCorrespondenceEvidence, compose_lift_transport_correspondence_certificate,
};
use crate::proof_contracts::quotients::relation_plan::theorem_schema::{
    TheoremApplicationSide, TheoremContractFactLocation, TheoremContractOwner,
};
use crate::proof_contracts::quotients::relation_plan::theorem_schema_verification::verify_selected_theorem_schema;
use crate::proof_contracts::quotients::relation_plan::{
    InputRelation, RelationPlanError, derive_direct_terminal_plan,
};
use typed_trees::TypedTrees;
use typed_trees::domain::ProofFact;
use typed_trees::expression::{
    ExpressionHandle, ExpressionNode, QuotientTheoremRole, QuotientTheoremSelection,
};
use typed_trees::machine::Machine;
use typed_trees::name::Identifier;
use typed_trees::proposition::PropositionApplication;
use typed_trees::signature::{SignatureContract, SignatureContractKind, StateParameter};
use typed_trees::state::State;
use typed_trees::types::TypeReferenceNode;

#[test]
fn selected_theorem_schema_verification_certifies_exact_fact_coordinates() {
    let (program, representative, theorem, expected) =
        selected_theorem_schema_fixture(TheoremSchemaMutation::Exact);
    let certificate =
        verify_selected_theorem_schema(&program, &representative, &theorem, &expected)
            .expect("the exact selected theorem schema must be certified");

    assert_eq!(certificate.theorem_machine_symbol, symbol(861));
    assert_eq!(certificate.theorem_state_symbol, symbol(862));
    assert_eq!(certificate.parameters.len(), 3);
    assert_eq!(certificate.relation_premises.len(), 1);
    assert_eq!(certificate.relation_premises[0].expected_position, 0);
    assert_eq!(certificate.legality_premises.len(), 2);
    assert_ne!(
        certificate.legality_premises[0].actual,
        certificate.legality_premises[1].actual,
    );
    assert_eq!(certificate.conclusion.owner, TheoremContractOwner::State);
}

#[test]
fn selected_theorem_schema_verification_rejects_extra_and_wrong_premises() {
    for (mutation, expected_error) in [
        (
            TheoremSchemaMutation::ExtraPremise,
            RelationPlanError::TheoremSchemaPremiseCountMismatch,
        ),
        (
            TheoremSchemaMutation::WrongRelation,
            RelationPlanError::TheoremSchemaRelationPremiseMismatch(0),
        ),
        (
            TheoremSchemaMutation::WrongLegality,
            RelationPlanError::TheoremSchemaLegalityPremiseMismatch(1),
        ),
    ] {
        let (program, representative, theorem, schema) = selected_theorem_schema_fixture(mutation);
        assert_eq!(
            verify_selected_theorem_schema(&program, &representative, &theorem, &schema),
            Err(expected_error),
        );
    }
}

#[test]
fn selected_theorem_schema_verification_rejects_application_mapping_drift() {
    for mutation in [
        TheoremSchemaMutation::RedirectedOperation,
        TheoremSchemaMutation::DuplicatedLeftApplication,
        TheoremSchemaMutation::OmittedRightApplication,
        TheoremSchemaMutation::ReboundSharedArgument,
    ] {
        let (program, representative, theorem, schema) = selected_theorem_schema_fixture(mutation);
        assert_eq!(
            verify_selected_theorem_schema(&program, &representative, &theorem, &schema),
            Err(RelationPlanError::TheoremSchemaConclusionMismatch),
        );
    }
}

#[test]
fn selected_theorem_schema_verification_rejects_runtime_evidence_and_const_parameters() {
    for (mutation, expected_error) in [
        (
            TheoremSchemaMutation::NamedEvidenceLane,
            RelationPlanError::TheoremSchemaNamedEvidenceLane,
        ),
        (
            TheoremSchemaMutation::ConstParameter,
            RelationPlanError::TheoremSchemaConstParameter(0),
        ),
        (
            TheoremSchemaMutation::AttachedReceiver,
            RelationPlanError::TheoremSchemaAttachedReceiver(0),
        ),
        (
            TheoremSchemaMutation::ParameterTypeMismatch,
            RelationPlanError::TheoremSchemaParameterTypeMismatch(2),
        ),
        (
            TheoremSchemaMutation::UnexpectedContractKind,
            RelationPlanError::TheoremSchemaUnexpectedContractKind,
        ),
        (
            TheoremSchemaMutation::MissingConclusion,
            RelationPlanError::TheoremSchemaConclusionCountMismatch,
        ),
    ] {
        let (program, representative, theorem, schema) = selected_theorem_schema_fixture(mutation);
        assert_eq!(
            verify_selected_theorem_schema(&program, &representative, &theorem, &schema),
            Err(expected_error),
        );
    }
}

#[test]
fn forward_transport_schema_certifies_complete_ordered_both_side_rosters() {
    let fixture = transport_schema_fixture(TransportSchemaMutation::Exact);
    let verified = verify_transport_fixture(&fixture).expect("exact transport schema");
    assert_eq!(verified.parameters.len(), 3);
    assert_eq!(verified.public_premises.len(), 4);
    assert_eq!(verified.representative_conclusions.len(), 4);
    assert_eq!(
        verified
            .public_premises
            .iter()
            .map(|row| row.application)
            .collect::<Vec<_>>(),
        vec![
            TheoremApplicationSide::Left,
            TheoremApplicationSide::Right,
            TheoremApplicationSide::Left,
            TheoremApplicationSide::Right,
        ]
    );
    assert_eq!(verified.public_premises[0].source.fact_position, 0);
    assert_eq!(verified.public_premises[1].source.fact_position, 0);
    assert_eq!(verified.public_premises[2].source.fact_position, 1);
    assert_eq!(verified.public_premises[2].actual.fact_position, 2);
    assert_eq!(
        verified.representative_conclusions[3].actual.fact_position,
        3
    );
}

#[test]
fn forward_transport_schema_rejects_roster_and_substitution_mutations() {
    for (mutation, expected) in [
        (
            TransportSchemaMutation::MissingPremise,
            RelationPlanError::TransportSchemaPremiseCountMismatch,
        ),
        (
            TransportSchemaMutation::ExtraPremise,
            RelationPlanError::TransportSchemaPremiseCountMismatch,
        ),
        (
            TransportSchemaMutation::ReorderedPremise,
            RelationPlanError::TransportSchemaPublicPremiseMismatch(0),
        ),
        (
            TransportSchemaMutation::SideMajorPremiseOrder,
            RelationPlanError::TransportSchemaPublicPremiseMismatch(1),
        ),
        (
            TransportSchemaMutation::MissingConclusion,
            RelationPlanError::TransportSchemaConclusionCountMismatch,
        ),
        (
            TransportSchemaMutation::WrongConclusion,
            RelationPlanError::TransportSchemaRepresentativeConclusionMismatch(2),
        ),
        (
            TransportSchemaMutation::ReboundSharedConclusion,
            RelationPlanError::TransportSchemaRepresentativeConclusionMismatch(3),
        ),
        (
            TransportSchemaMutation::NamedEvidenceLane,
            RelationPlanError::TheoremSchemaNamedEvidenceLane,
        ),
        (
            TransportSchemaMutation::UnexpectedContractKind,
            RelationPlanError::TheoremSchemaUnexpectedContractKind,
        ),
        (
            TransportSchemaMutation::WrongParameterType,
            RelationPlanError::TheoremSchemaParameterTypeMismatch(2),
        ),
    ] {
        let fixture = transport_schema_fixture(mutation);
        assert_eq!(verify_transport_fixture(&fixture), Err(expected));
    }
}

#[test]
fn transport_certificate_requires_exact_role_identity_and_each_eligibility_fence() {
    let fixture = transport_schema_fixture(TransportSchemaMutation::Exact);
    let verified = verify_transport_fixture(&fixture).expect("exact transport schema");
    let congruence = crate::proof_contracts::quotients::relation_plan::theorem_schema_verification::VerifiedTheoremSchema {
        theorem_machine_symbol: symbol(920),
        theorem_state_symbol: symbol(921),
        parameters: Vec::new(),
        relation_premises: Vec::new(),
        legality_premises: Vec::new(),
        conclusion: TheoremContractFactLocation {
            owner: TheoremContractOwner::State,
            contract_position: 0,
            fact_position: 0,
        },
    };
    let exact_evidence = crate::proof_contracts::quotients::relation_plan::PlannedQuotientTheoremEvidence {
        role: QuotientTheoremRole::ForwardPreconditionTransport,
        selected_application: fixture.theorem.clone(),
        termination: Some(crate::proof_contracts::quotients::relation_plan::theorem::SelectedTheoremTermination {
            machine_symbol: fixture.theorem.machine_symbol,
            state_symbol: fixture.theorem.state_symbol,
        }),
        purity: Some(crate::proof_contracts::quotients::relation_plan::theorem::SelectedTheoremPurity {
            machine_symbol: fixture.theorem.machine_symbol,
            state_symbol: fixture.theorem.state_symbol,
        }),
        crash_free: true,
    };
    let certificate = compose_lift_transport_correspondence_certificate(
        &Ok(congruence.clone()),
        &{
            let mut evidence = exact_evidence.clone();
            evidence.role = QuotientTheoremRole::Congruence;
            evidence.selected_application.machine_symbol = congruence.theorem_machine_symbol;
            evidence.selected_application.state_symbol = congruence.theorem_state_symbol;
            evidence.termination = Some(crate::proof_contracts::quotients::relation_plan::theorem::SelectedTheoremTermination {
                machine_symbol: congruence.theorem_machine_symbol,
                state_symbol: congruence.theorem_state_symbol,
            });
            evidence.purity = Some(crate::proof_contracts::quotients::relation_plan::theorem::SelectedTheoremPurity {
                machine_symbol: congruence.theorem_machine_symbol,
                state_symbol: congruence.theorem_state_symbol,
            });
            evidence
        },
        &Ok(verified.clone()),
        &exact_evidence,
        &fixture.runtime,
    )
    .expect("all transport eligibility and exact identity compose");
    let mut congruence_evidence = exact_evidence.clone();
    congruence_evidence.role = QuotientTheoremRole::Congruence;
    congruence_evidence.selected_application.machine_symbol = congruence.theorem_machine_symbol;
    congruence_evidence.selected_application.state_symbol = congruence.theorem_state_symbol;
    congruence_evidence.termination = Some(
        crate::proof_contracts::quotients::relation_plan::theorem::SelectedTheoremTermination {
            machine_symbol: congruence.theorem_machine_symbol,
            state_symbol: congruence.theorem_state_symbol,
        },
    );
    congruence_evidence.purity = Some(
        crate::proof_contracts::quotients::relation_plan::theorem::SelectedTheoremPurity {
            machine_symbol: congruence.theorem_machine_symbol,
            state_symbol: congruence.theorem_state_symbol,
        },
    );
    let plan = crate::proof_contracts::quotients::relation_plan::DirectTerminalRelationPlan {
        input_relations: vec![
            InputRelation::Quotient(fixture.expected_congruence.relation_premises[0].relation),
            InputRelation::ExactEquality(fixture.representative.parameters[1].type_reference),
        ],
        result_relation: fixture.expected_congruence.result_relation,
        representative: fixture.representative.clone(),
        representative_termination: None,
        theorem_evidence: vec![congruence_evidence.clone(), exact_evidence.clone()],
        expected_theorem_schema: fixture.expected_congruence.clone(),
        theorem_schema_verification: Ok(congruence.clone()),
        transport_schema_verification: Some(Ok(verified.clone())),
        direct_lift_correspondence: Some(fixture.runtime.clone()),
        define_correspondence: None,
        public_precondition: Some(fixture.public_partition.clone()),
        representative_precondition: Some(fixture.representative_partition.clone()),
        direct_lift_precondition_implication: None,
        fixed_representative_call_preconditions: None,
        define_precondition_correspondence: None,
        correspondence_certificate: Some(certificate.clone()),
    };
    assert!(plan.direct_lift_precondition_implication.is_none());
    assert!(plan.fixed_representative_call_preconditions.is_none());
    assert!(
        !plan.has_undischarged_fixed_representative_preconditions(),
        "the complete selected transport roster includes fixed P on both sides"
    );
    let QuotientCorrespondenceEvidence::DirectLiftWithTransport { runtime, transport } =
        certificate.evidence
    else {
        panic!("selected transport must have a distinct no-automatic-row certificate variant")
    };
    assert_eq!(runtime, fixture.runtime);
    assert_eq!(transport, verified);

    let mut mutations = Vec::new();
    let mut wrong_role = exact_evidence.clone();
    wrong_role.role = QuotientTheoremRole::Congruence;
    mutations.push(wrong_role);
    let mut wrong_identity = exact_evidence.clone();
    wrong_identity.selected_application.state_symbol = symbol(922);
    mutations.push(wrong_identity);
    let mut static_application_drift = exact_evidence.clone();
    static_application_drift
        .selected_application
        .static_application
        .lifetime_arguments
        .push(Identifier::generated_static("drift"));
    mutations.push(static_application_drift);
    let mut missing_termination = exact_evidence.clone();
    missing_termination.termination = None;
    mutations.push(missing_termination);
    let mut missing_purity = exact_evidence.clone();
    missing_purity.purity = None;
    mutations.push(missing_purity);
    let mut crash_route = exact_evidence.clone();
    crash_route.crash_free = false;
    mutations.push(crash_route);
    for mutation in mutations {
        assert!(
            compose_lift_transport_correspondence_certificate(
                &Ok(congruence.clone()),
                &congruence_evidence,
                &Ok(verified.clone()),
                &mutation,
                &fixture.runtime,
            )
            .is_none()
        );
    }
    let mut ineligible_congruence = congruence_evidence;
    ineligible_congruence.purity = None;
    assert!(
        compose_lift_transport_correspondence_certificate(
            &Ok(congruence),
            &ineligible_congruence,
            &Ok(verified),
            &exact_evidence,
            &fixture.runtime,
        )
        .is_none()
    );
}

#[test]
fn direct_plan_composes_selected_transport_without_automatic_implication_rows() {
    let mut program = TypedTrees::default();
    let quotient = quotient_type(
        &mut program,
        symbol(930),
        "PlanTransportQ",
        symbol(931),
        "PlanTransportR",
    );
    let carrier = carrier_type(&mut program);
    let unit = program.type_reference_table.insert(TypeReferenceNode::Unit);

    let representative_parameter = symbol(932);
    let mut representative_state = State {
        symbol: symbol(933),
        name: Identifier::generated_static("apply"),
        return_type: carrier,
        ..Default::default()
    };
    program.push_state_parameter(
        &mut representative_state,
        StateParameter {
            symbol: representative_parameter,
            name: Identifier::generated_static("value"),
            type_reference: carrier,
            ..Default::default()
        },
    );
    let mut representative_machine = Machine {
        symbol: symbol(934),
        name: Identifier::generated_static("representative"),
        termination_plan: language_semantics::MachineTerminationPlan {
            checked_summary: language_semantics::TerminationGuarantee::Terminates {
                premises: Vec::new(),
            },
            ..Default::default()
        },
        ..Default::default()
    };
    program.push_machine_state(&mut representative_machine, representative_state);
    program.push_machine(representative_machine);

    let relation_symbol = symbol(931);
    let left_symbol = symbol(935);
    let right_symbol = symbol(936);
    let left = named_argument(&mut program, "left", left_symbol);
    let right = named_argument(&mut program, "right", right_symbol);
    let relation_arguments = program
        .expression_table
        .insert_expression_handles([left, right]);
    let relation_premise =
        program
            .proof_facts
            .insert_many([ProofFact::Proposition(PropositionApplication {
                proposition: relation_symbol,
                name: Identifier::generated_static("PlanTransportR"),
                binder_arguments: Box::default(),
                arguments: relation_arguments,
            })]);
    let representative_call = |program: &mut TypedTrees, argument: ExpressionHandle| {
        let arguments = program
            .expression_table
            .insert_expression_handles([argument]);
        let mut call = call_with_arguments(arguments);
        call.target_symbol = symbol(933);
        call.target = Identifier::generated_static("apply");
        program.expression_table.insert(ExpressionNode::Call(call))
    };
    let left_result = representative_call(&mut program, left);
    let right_result = representative_call(&mut program, right);
    let conclusion_arguments = program
        .expression_table
        .insert_expression_handles([left_result, right_result]);
    let conclusion =
        program
            .proof_facts
            .insert_many([ProofFact::Proposition(PropositionApplication {
                proposition: relation_symbol,
                name: Identifier::generated_static("PlanTransportR"),
                binder_arguments: Box::default(),
                arguments: conclusion_arguments,
            })]);
    let congruence_machine_contracts =
        program.signature_contracts.insert_many([SignatureContract {
            kind: SignatureContractKind::Requires,
            facts: relation_premise,
            ..Default::default()
        }]);
    let congruence_state_contracts = program.signature_contracts.insert_many([SignatureContract {
        kind: SignatureContractKind::Ensures,
        facts: conclusion,
        ..Default::default()
    }]);
    let mut congruence_state = State {
        symbol: symbol(937),
        return_type: unit,
        contracts: congruence_state_contracts,
        ..Default::default()
    };
    for (parameter_symbol, name) in [(left_symbol, "left"), (right_symbol, "right")] {
        program.push_state_parameter(
            &mut congruence_state,
            StateParameter {
                symbol: parameter_symbol,
                name: Identifier::generated_static(name),
                type_reference: carrier,
                ..Default::default()
            },
        );
    }
    let mut congruence_machine = Machine {
        symbol: symbol(938),
        name: Identifier::generated_static("congruence"),
        contracts: congruence_machine_contracts,
        termination_plan: language_semantics::MachineTerminationPlan {
            checked_summary: language_semantics::TerminationGuarantee::Terminates {
                premises: Vec::new(),
            },
            ..Default::default()
        },
        ..Default::default()
    };
    program.push_machine_state(&mut congruence_machine, congruence_state);
    program.push_machine(congruence_machine);

    let transport_left_symbol = symbol(939);
    let transport_right_symbol = symbol(940);
    let mut transport_state = State {
        symbol: symbol(941),
        return_type: unit,
        ..Default::default()
    };
    for (parameter_symbol, name) in [
        (transport_left_symbol, "left"),
        (transport_right_symbol, "right"),
    ] {
        program.push_state_parameter(
            &mut transport_state,
            StateParameter {
                symbol: parameter_symbol,
                name: Identifier::generated_static(name),
                type_reference: carrier,
                ..Default::default()
            },
        );
    }
    let mut transport_machine = Machine {
        symbol: symbol(942),
        name: Identifier::generated_static("transport"),
        termination_plan: language_semantics::MachineTerminationPlan {
            checked_summary: language_semantics::TerminationGuarantee::Terminates {
                premises: Vec::new(),
            },
            ..Default::default()
        },
        ..Default::default()
    };
    program.push_machine_state(&mut transport_machine, transport_state);
    program.push_machine(transport_machine);

    let public_symbol = symbol(943);
    let public_value = named_argument(&mut program, "value", public_symbol);
    let arguments = program
        .expression_table
        .insert_expression_handles([public_value]);
    let call = call_with_arguments(arguments);
    let public_machine = Machine::default();
    let mut public_state = State {
        return_type: quotient,
        ..Default::default()
    };
    program.push_state_parameter(
        &mut public_state,
        StateParameter {
            symbol: public_symbol,
            name: Identifier::generated_static("value"),
            type_reference: quotient,
            ..Default::default()
        },
    );
    let mut request = request_with_representative(symbol(933));
    request.theorem_evidence[0].application.symbol = symbol(937);
    let mut selected_transport = static_argument("transport");
    selected_transport.symbol = symbol(941);
    request.theorem_evidence = vec![
        request.theorem_evidence[0].clone(),
        QuotientTheoremSelection {
            role: QuotientTheoremRole::ForwardPreconditionTransport,
            application: selected_transport,
        },
    ]
    .into_boxed_slice();

    let plan = derive_direct_terminal_plan(
        &program,
        &program,
        &public_machine,
        &public_state,
        &call,
        &request,
    )
    .expect("the exact selected transport should complete checked relation planning");
    assert!(
        plan.transport_schema_verification
            .as_ref()
            .is_some_and(Result::is_ok)
    );
    assert!(plan.direct_lift_precondition_implication.is_none());
    assert!(plan.fixed_representative_call_preconditions.is_none());
    assert!(matches!(
        plan.correspondence_certificate
            .as_ref()
            .map(|certificate| &certificate.evidence),
        Some(QuotientCorrespondenceEvidence::DirectLiftWithTransport { .. })
    ));
}

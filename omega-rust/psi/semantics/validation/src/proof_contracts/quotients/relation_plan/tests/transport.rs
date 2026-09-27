use super::{
    TheoremSchemaMutation, TransportSchemaMutation, selected_theorem_schema_fixture, symbol,
    transport_schema_fixture, verify_transport_fixture,
};
use crate::proof_contracts::quotients::relation_plan::correspondence_certificate::{
    QuotientCorrespondenceEvidence, compose_lift_transport_correspondence_certificate,
};
use crate::proof_contracts::quotients::relation_plan::theorem_schema::{
    TheoremContractFactLocation, TheoremContractOwner,
};
use crate::proof_contracts::quotients::relation_plan::theorem_schema_verification::verify_selected_theorem_schema;
use crate::proof_contracts::quotients::relation_plan::{InputRelation, RelationPlanError};
use typed_trees::expression::QuotientTheoremRole;
use typed_trees::name::Identifier;

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
        TheoremSchemaMutation::DispatchedOperation,
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

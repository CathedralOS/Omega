use super::fixtures::{id, plan};
use crate::{
    ObservationEventClass, ObservationKnowledge, OwnershipFrontierFact,
    OwnershipFrontierFactIndexError, OwnershipFrontierOwnedPlace, OwnershipFrontierSite,
    OwnershipFrontierSnapshot, ProofQuestion, ProofQuestionAdmissionKind, ProofQuestionClass,
    ProofQuestionIndexError, ProofQuestionOwner, attach_ownership_frontier_facts,
    attach_proof_questions, reconstruct_psi_observation_model,
    reconstruct_psi_optimization_unit_seed,
};
use omega_abstract_operations::AbstractOperation;
use psi_core::{
    AdmissionSiteId, ContractId, EdgeId, EvidenceIdentity, FuelScheduleIdentity, ObligationId,
    OperationId, PlaceId,
};
use psi_terminal::StructuralMultiplicity;

#[test]
fn proof_question_attachment_preserves_order_and_rejects_forgery_or_duplicates() {
    let seed = reconstruct_psi_optimization_unit_seed(
        &plan(),
        FuelScheduleIdentity::new(1).expect("nonzero schedule"),
    )
    .unwrap();
    let machine = seed.functions[0].machine;
    let first = ProofQuestion::new(
        seed.psi,
        [5; 32],
        ProofQuestionOwner::Operation {
            machine,
            operation: id(5, OperationId::new),
        },
        id(118, ObligationId::new),
        ProofQuestionClass::Derivable,
        vec![1],
        vec![vec![2]],
        vec![vec![3]],
        true,
    );
    let second = ProofQuestion::new(
        seed.psi,
        [5; 32],
        ProofQuestionOwner::ContractEnsures {
            machine,
            contract: id(119, ContractId::new),
            clause_position: 0,
        },
        id(120, ObligationId::new),
        ProofQuestionClass::AdmissionAuthorized {
            site: id(121, AdmissionSiteId::new),
            kind: ProofQuestionAdmissionKind::ProviderFact,
            authority_identity: id(122, EvidenceIdentity::new),
        },
        vec![4],
        vec![vec![5], vec![6]],
        vec![vec![7]],
        false,
    );
    let attached = attach_proof_questions(seed.clone(), vec![second.clone(), first.clone()])
        .expect("verifier order is retained, not sorted");
    assert_eq!(
        attached.proof_questions,
        vec![second.clone(), first.clone()]
    );
    assert_eq!(
        attach_proof_questions(attached, Vec::new()),
        Err(ProofQuestionIndexError::AlreadyAttached)
    );
    assert_eq!(
        attach_proof_questions(seed.clone(), vec![first.clone(), first]),
        Err(ProofQuestionIndexError::DuplicateQuestion)
    );
    let mut forged = second;
    forged.semantic_axioms.push(vec![8]);
    assert_eq!(
        attach_proof_questions(seed, vec![forged]),
        Err(ProofQuestionIndexError::InvalidQuestionIdentity)
    );
}

#[test]
fn ownership_frontier_attachment_is_canonical_and_single_use() {
    let seed = reconstruct_psi_optimization_unit_seed(
        &plan(),
        FuelScheduleIdentity::new(1).expect("nonzero schedule"),
    )
    .unwrap();
    let machine = seed.functions[0].machine;
    let block = seed.functions[0].entry;
    let empty = OwnershipFrontierSnapshot {
        claims: Vec::new(),
        owned_places: Vec::new(),
        partial_custody: Vec::new(),
    };
    let block_fact = OwnershipFrontierFact::new(
        seed.psi,
        machine,
        OwnershipFrontierSite::BlockEntry(block),
        empty.clone(),
    );
    let edge_fact = OwnershipFrontierFact::new(
        seed.psi,
        machine,
        OwnershipFrontierSite::EdgeEntry(id(6, EdgeId::new)),
        empty,
    );
    assert_eq!(
        attach_ownership_frontier_facts(seed.clone(), vec![edge_fact.clone(), block_fact.clone()]),
        Err(OwnershipFrontierFactIndexError::NonCanonicalOrder)
    );
    let place = id(20, PlaceId::new);
    let duplicate_place_snapshot = OwnershipFrontierSnapshot {
        claims: Vec::new(),
        owned_places: vec![
            OwnershipFrontierOwnedPlace {
                place,
                multiplicity: StructuralMultiplicity::Affine,
            },
            OwnershipFrontierOwnedPlace {
                place,
                multiplicity: StructuralMultiplicity::Affine,
            },
        ],
        partial_custody: Vec::new(),
    };
    assert_eq!(
        attach_ownership_frontier_facts(
            seed.clone(),
            vec![OwnershipFrontierFact::new(
                seed.psi,
                machine,
                OwnershipFrontierSite::BlockEntry(block),
                duplicate_place_snapshot,
            )],
        ),
        Err(OwnershipFrontierFactIndexError::NonCanonicalSnapshot)
    );

    let attached =
        attach_ownership_frontier_facts(seed.clone(), vec![block_fact.clone(), edge_fact.clone()])
            .unwrap();
    let replay = attach_ownership_frontier_facts(seed, vec![block_fact, edge_fact]).unwrap();
    assert_eq!(attached, replay);
    assert_eq!(
        attach_ownership_frontier_facts(attached, Vec::new()),
        Err(OwnershipFrontierFactIndexError::AlreadyAttached)
    );
}

#[test]
fn observation_projection_keeps_external_events_and_semantic_accounting() {
    let unit = reconstruct_psi_optimization_unit_seed(
        &plan(),
        FuelScheduleIdentity::new(1).expect("nonzero schedule"),
    )
    .unwrap();
    let observations = reconstruct_psi_observation_model(&unit);

    assert_eq!(observations.revision, unit.identity);
    assert_eq!(observations.nodes.len(), 2);
    assert!(observations.nodes[0].events.is_empty());
    assert_eq!(observations.nodes[0].crash, ObservationKnowledge::No);
    assert_eq!(observations.nodes[0].provenance.len(), 1);
    assert_eq!(observations.nodes[0].fuel.len(), 1);
    assert_eq!(observations.nodes[1].events.len(), 1);
    assert_eq!(
        observations.nodes[1].events[0].class,
        ObservationEventClass::NormalExit
    );
    assert!(matches!(
        observations.nodes[1].events[0].operation,
        AbstractOperation::Return { .. }
    ));
}

//! Substitutable-field inventory for the sealed canonical proof section.
//!
//! The family's declared legs are driven through the shared substitution
//! matrix by `proof_section_custody.rs`; keeping the inventory in its own
//! file names the boundary between what is declared substitutable and the
//! driver that exercises every leg. A leg is one authored substitution of a
//! `ProofBundle` field that still canonicalizes under the sealed-section
//! encoder, so roster-order violations, empty or non-dense identities, and
//! non-canonical proposition shapes are not representable and stay authored
//! in `proof_section_custody.rs` as encode- and decode-level rejections
//! alongside the raw wire axes.
//!
//! Every declared leg is rejected by the retained-manifest replay with
//! `ArtifactManifestError::ManifestMismatch`; whether the semantic verifier
//! also refuses the mutated bundle is a per-leg verdict carried by
//! [`proof_section_semantic_verdict`] and asserted through the matrix's
//! joined-replay hook, since provenance fields such as certificate identities
//! are admitted by the verifier while still diverging the bound identity.

use super::super::{
    block_id, edge_id, evidence_id, evidence_term_id, i32_type, machine_id, obligation_id,
};
use mutation_matrix::{MutationOutcome, custody_field_inventory};
use proof_admission::{
    AdmissionEvidence, AdmissionKind, CertificateEnvelope, EvidenceRoute, PrimitiveJudgment,
    ProofNode, ProofRule, ProofSystemMarker, RecursiveComponentCertificate,
    RecursiveEdgeCertificate,
};
use semantic_vocabulary::{
    AdmissionSiteId, CycleComponentId, IntegerValue, ProfileDecisionId, Proposition,
    RankingRelationId, RecursiveComponentId, ScalarTerm,
};
use terminal_codec::ArtifactManifestError;
use terminal_verifier::{
    ControlCycleEvidence, CrashCertificate, CrashObligationEvidence, CrashObligationOwner,
    EvidenceProducerProvenance, EvidenceProducerRowSource, ObligationEvidence, ProofBundle,
    RecursiveComponentEvidence,
};

custody_field_inventory! {
    /// One substitutable field of the sealed proof bundle; each leg's verdict
    /// is checked by the retained-manifest replay, and the semantic replay
    /// verdict per leg is declared by `proof_section_semantic_verdict`.
    pub enum ProofSectionCustodyFieldForTest {
        // --- obligation evidence roster ---
        TrailingObligationIdentity,
        EvidenceRouteToEqualStrengthKernel,
        EvidenceRouteToUnprovenKernel,
        EvidenceRouteToReflexiveKernel,
        EvidenceRouteToAdmitted,
        AdmittedSiteSubstituted,
        AdmittedKindSubstituted,
        AdmittedAuthoritySubstituted,
        AdmittedEvidenceIdentitySubstituted,
        AdmittedProfileDecisionSubstituted,
        CertificateEvidenceIdentitySubstituted,
        CertificateConclusionSubstituted,
        CertificateConclusionOperandSubstituted,
        CertificateRuleKindSubstituted,
        CertificateChildRuleSubstituted,
        CertificateChildConclusionSubstituted,
        ConjunctionChildDropped,
        ConjunctionLeafRuleSubstituted,
        EvidenceRowDropped,
        EvidenceRowAppended,
        // --- recursive-component certificates ---
        ComponentIdentitySubstituted,
        ComponentCertificateIdentitySubstituted,
        ComponentRankingRelationSubstituted,
        ComponentWellFoundednessRouteSubstituted,
        WellFoundednessEnvelopeIdentitySubstituted,
        WellFoundednessAxiomIndexSubstituted,
        WellFoundednessConclusionSubstituted,
        DecreaseEdgeObligationSubstituted,
        DecreaseEdgeAxiomIndexSubstituted,
        DecreaseEdgeEnvelopeIdentitySubstituted,
        DecreaseEdgeRouteKindSubstituted,
        ComponentRowDropped,
        ComponentRowAppended,
        ComponentEdgeDropped,
        // --- control-cycle certificates (surplus authority: the module
        // reconstructs no cycle obligation) ---
        ControlCycleRowAppended,
        ControlCycleComponentSubstituted,
        ControlCycleCertificateIdentitySubstituted,
        ControlCycleRankingRelationSubstituted,
        ControlCycleWellFoundednessRouteSubstituted,
        ControlCycleEdgeObligationSubstituted,
        ControlCycleEdgeRouteSubstituted,
        // --- crash-obligation certificates (surplus authority: the module
        // reconstructs no crash question) ---
        CrashObligationRowAppended,
        CrashObligationOwnerSubstituted,
        CrashCertificateDenotationSubstituted,
        CrashCertificateRuleSubstituted,
        // --- evidence-producer provenance ---
        ProducerEvidenceTermSubstituted,
        ProducerConformanceIdentitySubstituted,
        ProducerEvidenceTraitSubstituted,
        RealizationDeclaringTraitSubstituted,
        RealizationTraitArgumentsSubstituted,
        RealizationRequirementSubstituted,
        RealizationMachineSubstituted,
        RealizationStateSubstituted,
        RealizationSourceKindSubstituted,
        ProducerRowDropped,
        ProducerRowAppended,
    }
}

/// A surplus control-cycle certificate in canonical roster order, owned by
/// the machine identity named by `component`. Shared with the retained
/// encode-level roster axes in `proof_section_custody.rs`.
pub(crate) fn surplus_cycle(component: u64) -> ControlCycleEvidence {
    ControlCycleEvidence {
        component: CycleComponentId::new(component).expect("nonzero cycle identity"),
        certificate: RecursiveComponentCertificate {
            identity: evidence_id(41),
            ranking_relation: RankingRelationId::new(51).expect("nonzero ranking relation"),
            well_foundedness: EvidenceRoute::KernelDerived(PrimitiveJudgment::Truth),
            edges: vec![
                RecursiveEdgeCertificate {
                    obligation: obligation_id(61),
                    evidence: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                        identity: evidence_id(71),
                        proof_system_marker: ProofSystemMarker::CURRENT,
                        proof: ProofNode {
                            conclusion: Proposition::Truth,
                            rule: ProofRule::Primitive(PrimitiveJudgment::Truth),
                        },
                    }),
                },
                RecursiveEdgeCertificate {
                    obligation: obligation_id(62),
                    evidence: EvidenceRoute::KernelDerived(PrimitiveJudgment::Truth),
                },
            ],
        },
    }
}

/// A surplus crash-obligation row in canonical roster order, owned by the
/// machine identity named by `machine`. Shared with the retained encode-level
/// roster axes in `proof_section_custody.rs`.
pub(crate) fn surplus_crash(machine: u64) -> CrashObligationEvidence {
    CrashObligationEvidence {
        owner: CrashObligationOwner::Site {
            machine: machine_id(machine),
            block: block_id(81),
            edge: edge_id(82),
        },
        coverage: vec![vec![CrashCertificate {
            with_value_equalities: false,
            proof: ProofNode {
                conclusion: Proposition::Truth,
                rule: ProofRule::Primitive(PrimitiveJudgment::Truth),
            },
        }]],
        refutation: Vec::new(),
    }
}

/// The `Admitted` route payload the admitted-route legs mutate one field at a
/// time before installing.
fn admitted_fixture() -> AdmissionEvidence {
    AdmissionEvidence {
        site: AdmissionSiteId::new(21).expect("site identity"),
        kind: AdmissionKind::ProviderFact,
        authority_identity: evidence_id(22),
        evidence_identity: evidence_id(23),
        profile_decision: ProfileDecisionId::new(24).expect("decision identity"),
    }
}

/// Apply one declared substitution to the proof bundle.
///
/// Every leg uses a fixed alternate rather than donor values: the foreign
/// donor bundle belongs to a different module's obligations, so its fields
/// would not canonicalize inside this roster.
pub(crate) fn substitute_proof_section_for_test(
    record: &mut ProofBundle,
    field: ProofSectionCustodyFieldForTest,
    donor: &ProofBundle,
) {
    let _ = donor;
    match field {
        // --- obligation evidence roster ---
        ProofSectionCustodyFieldForTest::TrailingObligationIdentity => {
            record.evidence[2].obligation = obligation_id(4);
        }
        ProofSectionCustodyFieldForTest::EvidenceRouteToEqualStrengthKernel => {
            record.evidence[0].route =
                EvidenceRoute::KernelDerived(PrimitiveJudgment::ClosedIntegerRelation);
        }
        ProofSectionCustodyFieldForTest::EvidenceRouteToUnprovenKernel => {
            record.evidence[0].route = EvidenceRoute::KernelDerived(PrimitiveJudgment::Truth);
        }
        ProofSectionCustodyFieldForTest::EvidenceRouteToReflexiveKernel => {
            record.evidence[2].route =
                EvidenceRoute::KernelDerived(PrimitiveJudgment::ReflexiveEquality);
        }
        ProofSectionCustodyFieldForTest::EvidenceRouteToAdmitted => {
            record.evidence[2].route = EvidenceRoute::Admitted(admitted_fixture());
        }
        ProofSectionCustodyFieldForTest::AdmittedSiteSubstituted => {
            let mut admitted = admitted_fixture();
            admitted.site = AdmissionSiteId::new(25).expect("site identity");
            record.evidence[2].route = EvidenceRoute::Admitted(admitted);
        }
        ProofSectionCustodyFieldForTest::AdmittedKindSubstituted => {
            let mut admitted = admitted_fixture();
            admitted.kind = AdmissionKind::CheckedAssemblyClaim;
            record.evidence[2].route = EvidenceRoute::Admitted(admitted);
        }
        ProofSectionCustodyFieldForTest::AdmittedAuthoritySubstituted => {
            let mut admitted = admitted_fixture();
            admitted.authority_identity = evidence_id(26);
            record.evidence[2].route = EvidenceRoute::Admitted(admitted);
        }
        ProofSectionCustodyFieldForTest::AdmittedEvidenceIdentitySubstituted => {
            let mut admitted = admitted_fixture();
            admitted.evidence_identity = evidence_id(27);
            record.evidence[2].route = EvidenceRoute::Admitted(admitted);
        }
        ProofSectionCustodyFieldForTest::AdmittedProfileDecisionSubstituted => {
            let mut admitted = admitted_fixture();
            admitted.profile_decision = ProfileDecisionId::new(28).expect("decision identity");
            record.evidence[2].route = EvidenceRoute::Admitted(admitted);
        }
        ProofSectionCustodyFieldForTest::CertificateEvidenceIdentitySubstituted => {
            let EvidenceRoute::CertificateDerived(envelope) = &mut record.evidence[0].route else {
                unreachable!()
            };
            envelope.identity = evidence_id(91);
        }
        ProofSectionCustodyFieldForTest::CertificateConclusionSubstituted => {
            let EvidenceRoute::CertificateDerived(envelope) = &mut record.evidence[0].route else {
                unreachable!()
            };
            envelope.proof.conclusion = Proposition::Truth;
        }
        ProofSectionCustodyFieldForTest::CertificateConclusionOperandSubstituted => {
            let EvidenceRoute::CertificateDerived(envelope) = &mut record.evidence[0].route else {
                unreachable!()
            };
            let Proposition::Equal(_, right) = &mut envelope.proof.conclusion else {
                unreachable!()
            };
            *right = ScalarTerm::integer(i32_type(), IntegerValue::Signed(8)).expect("i32 literal");
        }
        ProofSectionCustodyFieldForTest::CertificateRuleKindSubstituted => {
            let EvidenceRoute::CertificateDerived(envelope) = &mut record.evidence[0].route else {
                unreachable!()
            };
            envelope.proof.rule = ProofRule::Primitive(PrimitiveJudgment::Truth);
        }
        ProofSectionCustodyFieldForTest::CertificateChildRuleSubstituted => {
            let EvidenceRoute::CertificateDerived(envelope) = &mut record.evidence[0].route else {
                unreachable!()
            };
            let ProofRule::EqualityTransitivity {
                left_equals_middle, ..
            } = &mut envelope.proof.rule
            else {
                unreachable!()
            };
            left_equals_middle.rule = ProofRule::Assumption { index: 9 };
        }
        ProofSectionCustodyFieldForTest::CertificateChildConclusionSubstituted => {
            let EvidenceRoute::CertificateDerived(envelope) = &mut record.evidence[0].route else {
                unreachable!()
            };
            let ProofRule::EqualityTransitivity {
                left_equals_middle, ..
            } = &mut envelope.proof.rule
            else {
                unreachable!()
            };
            left_equals_middle.conclusion = Proposition::Falsehood;
        }
        ProofSectionCustodyFieldForTest::ConjunctionChildDropped => {
            let EvidenceRoute::CertificateDerived(envelope) = &mut record.evidence[1].route else {
                unreachable!()
            };
            let ProofRule::ConjunctionIntroduction(children) = &mut envelope.proof.rule else {
                unreachable!()
            };
            children.pop();
        }
        ProofSectionCustodyFieldForTest::ConjunctionLeafRuleSubstituted => {
            let EvidenceRoute::CertificateDerived(envelope) = &mut record.evidence[1].route else {
                unreachable!()
            };
            let ProofRule::ConjunctionIntroduction(children) = &mut envelope.proof.rule else {
                unreachable!()
            };
            children[0].rule = ProofRule::Primitive(PrimitiveJudgment::ReflexiveEquality);
        }
        ProofSectionCustodyFieldForTest::EvidenceRowDropped => {
            record.evidence.pop();
        }
        ProofSectionCustodyFieldForTest::EvidenceRowAppended => {
            record.evidence.push(ObligationEvidence {
                obligation: obligation_id(9),
                route: EvidenceRoute::KernelDerived(PrimitiveJudgment::Truth),
            });
        }
        // --- recursive-component certificates ---
        ProofSectionCustodyFieldForTest::ComponentIdentitySubstituted => {
            record.recursive_components[0].component =
                RecursiveComponentId::new(record.recursive_components[0].component.get() + 1)
                    .expect("nonzero component identity");
        }
        ProofSectionCustodyFieldForTest::ComponentCertificateIdentitySubstituted => {
            record.recursive_components[0].certificate.identity = evidence_id(9021);
        }
        ProofSectionCustodyFieldForTest::ComponentRankingRelationSubstituted => {
            record.recursive_components[0].certificate.ranking_relation =
                RankingRelationId::new(9022).expect("nonzero ranking relation");
        }
        ProofSectionCustodyFieldForTest::ComponentWellFoundednessRouteSubstituted => {
            record.recursive_components[0].certificate.well_foundedness =
                EvidenceRoute::KernelDerived(PrimitiveJudgment::Truth);
        }
        ProofSectionCustodyFieldForTest::WellFoundednessEnvelopeIdentitySubstituted => {
            let EvidenceRoute::CertificateDerived(envelope) =
                &mut record.recursive_components[0].certificate.well_foundedness
            else {
                unreachable!()
            };
            envelope.identity = evidence_id(9023);
        }
        ProofSectionCustodyFieldForTest::WellFoundednessAxiomIndexSubstituted => {
            let EvidenceRoute::CertificateDerived(envelope) =
                &mut record.recursive_components[0].certificate.well_foundedness
            else {
                unreachable!()
            };
            envelope.proof.rule = ProofRule::SemanticAxiom { index: 9 };
        }
        ProofSectionCustodyFieldForTest::WellFoundednessConclusionSubstituted => {
            let EvidenceRoute::CertificateDerived(envelope) =
                &mut record.recursive_components[0].certificate.well_foundedness
            else {
                unreachable!()
            };
            envelope.proof.conclusion = Proposition::Truth;
        }
        ProofSectionCustodyFieldForTest::DecreaseEdgeObligationSubstituted => {
            record.recursive_components[0].certificate.edges[0].obligation =
                obligation_id(9_000_001);
        }
        ProofSectionCustodyFieldForTest::DecreaseEdgeAxiomIndexSubstituted => {
            let EvidenceRoute::CertificateDerived(envelope) =
                &mut record.recursive_components[0].certificate.edges[0].evidence
            else {
                unreachable!()
            };
            envelope.proof.rule = ProofRule::SemanticAxiom { index: 7 };
        }
        ProofSectionCustodyFieldForTest::DecreaseEdgeEnvelopeIdentitySubstituted => {
            let EvidenceRoute::CertificateDerived(envelope) =
                &mut record.recursive_components[0].certificate.edges[0].evidence
            else {
                unreachable!()
            };
            envelope.identity = evidence_id(9024);
        }
        ProofSectionCustodyFieldForTest::DecreaseEdgeRouteKindSubstituted => {
            record.recursive_components[0].certificate.edges[0].evidence =
                EvidenceRoute::KernelDerived(PrimitiveJudgment::Truth);
        }
        ProofSectionCustodyFieldForTest::ComponentRowDropped => {
            record.recursive_components.pop();
        }
        ProofSectionCustodyFieldForTest::ComponentRowAppended => {
            let surplus = RecursiveComponentEvidence {
                component: RecursiveComponentId::new(
                    record.recursive_components[0].component.get() + 1,
                )
                .expect("nonzero component identity"),
                certificate: record.recursive_components[0].certificate.clone(),
            };
            record.recursive_components.push(surplus);
        }
        ProofSectionCustodyFieldForTest::ComponentEdgeDropped => {
            record.recursive_components[0].certificate.edges.pop();
        }
        // --- control-cycle certificates ---
        ProofSectionCustodyFieldForTest::ControlCycleRowAppended => {
            record.control_cycles.push(surplus_cycle(31));
        }
        ProofSectionCustodyFieldForTest::ControlCycleComponentSubstituted => {
            let mut row = surplus_cycle(31);
            row.component = CycleComponentId::new(32).expect("nonzero cycle identity");
            record.control_cycles.push(row);
        }
        ProofSectionCustodyFieldForTest::ControlCycleCertificateIdentitySubstituted => {
            let mut row = surplus_cycle(31);
            row.certificate.identity = evidence_id(42);
            record.control_cycles.push(row);
        }
        ProofSectionCustodyFieldForTest::ControlCycleRankingRelationSubstituted => {
            let mut row = surplus_cycle(31);
            row.certificate.ranking_relation =
                RankingRelationId::new(52).expect("nonzero ranking relation");
            record.control_cycles.push(row);
        }
        ProofSectionCustodyFieldForTest::ControlCycleWellFoundednessRouteSubstituted => {
            let mut row = surplus_cycle(31);
            row.certificate.well_foundedness =
                EvidenceRoute::CertificateDerived(CertificateEnvelope {
                    identity: evidence_id(72),
                    proof_system_marker: ProofSystemMarker::CURRENT,
                    proof: ProofNode {
                        conclusion: Proposition::Truth,
                        rule: ProofRule::Primitive(PrimitiveJudgment::Truth),
                    },
                });
            record.control_cycles.push(row);
        }
        ProofSectionCustodyFieldForTest::ControlCycleEdgeObligationSubstituted => {
            let mut row = surplus_cycle(31);
            row.certificate.edges[0].obligation = obligation_id(60);
            record.control_cycles.push(row);
        }
        ProofSectionCustodyFieldForTest::ControlCycleEdgeRouteSubstituted => {
            let mut row = surplus_cycle(31);
            row.certificate.edges[1].evidence = EvidenceRoute::Admitted(AdmissionEvidence {
                site: AdmissionSiteId::new(63).expect("site identity"),
                kind: AdmissionKind::ForeignBoundaryGuarantee,
                authority_identity: evidence_id(64),
                evidence_identity: evidence_id(65),
                profile_decision: ProfileDecisionId::new(66).expect("decision identity"),
            });
            record.control_cycles.push(row);
        }
        // --- crash-obligation certificates ---
        ProofSectionCustodyFieldForTest::CrashObligationRowAppended => {
            record.crash_obligations.push(surplus_crash(83));
        }
        ProofSectionCustodyFieldForTest::CrashObligationOwnerSubstituted => {
            let mut row = surplus_crash(83);
            row.owner = CrashObligationOwner::Site {
                machine: machine_id(84),
                block: block_id(81),
                edge: edge_id(82),
            };
            record.crash_obligations.push(row);
        }
        ProofSectionCustodyFieldForTest::CrashCertificateDenotationSubstituted => {
            let mut row = surplus_crash(83);
            row.coverage[0][0].with_value_equalities = true;
            record.crash_obligations.push(row);
        }
        ProofSectionCustodyFieldForTest::CrashCertificateRuleSubstituted => {
            let mut row = surplus_crash(83);
            row.coverage[0][0].proof.rule =
                ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation);
            record.crash_obligations.push(row);
        }
        // --- evidence-producer provenance ---
        ProofSectionCustodyFieldForTest::ProducerEvidenceTermSubstituted => {
            record.evidence_producers[0].term = evidence_term_id(2);
        }
        ProofSectionCustodyFieldForTest::ProducerConformanceIdentitySubstituted => {
            record.evidence_producers[0].conformance_identity =
                "package::other-conformance".to_owned();
        }
        ProofSectionCustodyFieldForTest::ProducerEvidenceTraitSubstituted => {
            record.evidence_producers[0].evidence_trait_identity = "package::Other".to_owned();
        }
        ProofSectionCustodyFieldForTest::RealizationDeclaringTraitSubstituted => {
            record.evidence_producers[0].rows[0].declaring_trait_identity =
                "package::Other".to_owned();
        }
        ProofSectionCustodyFieldForTest::RealizationTraitArgumentsSubstituted => {
            record.evidence_producers[0].rows[0].declaring_trait_arguments =
                vec!["package::Argument".to_owned()];
        }
        ProofSectionCustodyFieldForTest::RealizationRequirementSubstituted => {
            record.evidence_producers[0].rows[0].requirement_identity =
                "package::Evidence::other".to_owned();
        }
        ProofSectionCustodyFieldForTest::RealizationMachineSubstituted => {
            record.evidence_producers[0].rows[0].realization_machine_identity =
                "package::other-machine".to_owned();
        }
        ProofSectionCustodyFieldForTest::RealizationStateSubstituted => {
            record.evidence_producers[0].rows[0].realization_state_identity =
                "package::producer::other".to_owned();
        }
        ProofSectionCustodyFieldForTest::RealizationSourceKindSubstituted => {
            record.evidence_producers[0].rows[0].source = EvidenceProducerRowSource::Reference;
        }
        ProofSectionCustodyFieldForTest::ProducerRowDropped => {
            record.evidence_producers.pop();
        }
        ProofSectionCustodyFieldForTest::ProducerRowAppended => {
            record.evidence_producers.push(EvidenceProducerProvenance {
                id: evidence_id(2),
                term: evidence_term_id(2),
                conformance_identity: "package::conformance".to_owned(),
                evidence_trait_identity: "package::Evidence".to_owned(),
                rows: Vec::new(),
            });
        }
    }
}

/// Whether `verify_module` still accepts the bundle after a leg's
/// substitution. Provenance fields — certificate identities, producer
/// realization names — are admitted by the verifier while the bound identity
/// diverges; semantic fields — routes that cannot prove, obligations the
/// module never asks, surplus rosters — are refused.
pub(crate) fn proof_section_semantic_verdict(field: ProofSectionCustodyFieldForTest) -> bool {
    matches!(
        field,
        ProofSectionCustodyFieldForTest::EvidenceRouteToEqualStrengthKernel
            | ProofSectionCustodyFieldForTest::CertificateEvidenceIdentitySubstituted
            | ProofSectionCustodyFieldForTest::ComponentCertificateIdentitySubstituted
            | ProofSectionCustodyFieldForTest::WellFoundednessEnvelopeIdentitySubstituted
            | ProofSectionCustodyFieldForTest::DecreaseEdgeEnvelopeIdentitySubstituted
            | ProofSectionCustodyFieldForTest::ProducerConformanceIdentitySubstituted
            | ProofSectionCustodyFieldForTest::RealizationMachineSubstituted
            | ProofSectionCustodyFieldForTest::RealizationStateSubstituted
            | ProofSectionCustodyFieldForTest::RealizationSourceKindSubstituted
    )
}

/// Name the verdict a declared leg must produce at the retained-manifest
/// replay: every representable substitution diverges the bundle fingerprint
/// the manifest binds, so the join always rejects with a manifest mismatch.
pub(crate) fn proof_section_custody_outcome(
    _field: ProofSectionCustodyFieldForTest,
) -> MutationOutcome<ArtifactManifestError> {
    MutationOutcome::ExactError(ArtifactManifestError::ManifestMismatch)
}

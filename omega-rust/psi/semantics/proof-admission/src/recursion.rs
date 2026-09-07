use std::collections::{BTreeMap, BTreeSet};
pub use terminal_psi::{RecursiveComponentCertificate, RecursiveEdgeCertificate};

use semantic_vocabulary::{
    ContractId, EvidenceIdentity, ObligationId, Proposition, PropositionContext, RankingRelationId,
};

use crate::{
    AcceptedFact, AdmissionProfile, EvidenceError, EvidenceRoute, Obligation,
    verify_obligation_with_machine_parameters,
};

/// One kernel-owned proposition with its fixed premise and reconstructed-axiom
/// context. Evidence may discharge this obligation but cannot choose the facts
/// under which it is checked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CertificateObligation {
    pub obligation: Obligation,
    pub assumptions: Vec<Proposition>,
    pub semantic_axioms: Vec<Proposition>,
}

/// One exact component edge with its reconstructed rank-comparison obligation.
/// Contract identities name proof-call endpoints; block identities can name
/// runtime control-flow endpoints without manufacturing contracts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecursiveEdgeObligation<Member = ContractId> {
    pub caller: Member,
    pub callee: Member,
    pub decrease: CertificateObligation,
}

/// Artifact/source reconstruction owns this shape. A proof bundle may
/// discharge it but cannot add members, remove edges, or select another
/// ranking relation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecursiveComponentObligation<Member = ContractId> {
    pub members: Vec<Member>,
    pub ranking_relation: Option<RankingRelationId>,
    pub well_foundedness: CertificateObligation,
    pub edges: Vec<RecursiveEdgeObligation<Member>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecursiveComponentAcceptance<Member = ContractId> {
    pub certificate: EvidenceIdentity,
    pub ranking_relation: RankingRelationId,
    pub members: Vec<Member>,
    pub well_foundedness: AcceptedFact,
    pub decreases: Vec<AcceptedFact>,
}

/// Check exact component shape and every supplied proof route. Reconstruction
/// owns the comparison propositions and any strict-edge cycle-coverage rule.
pub fn verify_recursive_component<Member: Copy + Ord>(
    context: &PropositionContext,
    obligation: &RecursiveComponentObligation<Member>,
    certificate: RecursiveComponentCertificate,
    profile: &AdmissionProfile,
) -> Result<RecursiveComponentAcceptance<Member>, RecursiveComponentError<Member>> {
    verify_recursive_component_with_machine_parameters(
        context,
        obligation,
        &BTreeSet::new(),
        certificate,
        profile,
    )
}

/// Runtime reconstruction supplies only the owning invocation's scalar
/// parameters; loop-local values never acquire parameter-only proof rules.
pub fn verify_recursive_component_with_machine_parameters<Member: Copy + Ord>(
    context: &PropositionContext,
    obligation: &RecursiveComponentObligation<Member>,
    machine_parameter_values: &BTreeSet<semantic_vocabulary::ValueId>,
    certificate: RecursiveComponentCertificate,
    profile: &AdmissionProfile,
) -> Result<RecursiveComponentAcceptance<Member>, RecursiveComponentError<Member>> {
    validate_component_shape(obligation)?;
    let ranking_relation = obligation
        .ranking_relation
        .ok_or(RecursiveComponentError::MissingRankingRelation)?;
    if certificate.ranking_relation != ranking_relation {
        return Err(RecursiveComponentError::RankingRelationMismatch);
    }
    let well_foundedness = verify_recursive_obligation(
        context,
        &obligation.well_foundedness,
        machine_parameter_values,
        certificate.well_foundedness,
        profile,
    )
    .map_err(RecursiveComponentError::WellFoundedness)?;

    let mut evidence = BTreeMap::new();
    let mut previous = None;
    for edge in certificate.edges {
        if previous.is_some_and(|previous| previous >= edge.obligation) {
            return Err(RecursiveComponentError::NonCanonicalCertificateEdges);
        }
        previous = Some(edge.obligation);
        evidence.insert(edge.obligation, edge.evidence);
    }
    let mut decreases = Vec::with_capacity(obligation.edges.len());
    for edge in &obligation.edges {
        let id = edge.decrease.obligation.id;
        let route = evidence
            .remove(&id)
            .ok_or(RecursiveComponentError::MissingDecreaseEvidence(id))?;
        let accepted = verify_recursive_obligation(
            context,
            &edge.decrease,
            machine_parameter_values,
            route,
            profile,
        )
        .map_err(|error| RecursiveComponentError::Decrease {
            obligation: id,
            error,
        })?;
        decreases.push(accepted);
    }
    if let Some(obligation) = evidence.keys().next().copied() {
        return Err(RecursiveComponentError::UnknownDecreaseEvidence(obligation));
    }

    Ok(RecursiveComponentAcceptance {
        certificate: certificate.identity,
        ranking_relation,
        members: obligation.members.clone(),
        well_foundedness,
        decreases,
    })
}

fn verify_recursive_obligation(
    context: &PropositionContext,
    obligation: &CertificateObligation,
    machine_parameter_values: &BTreeSet<semantic_vocabulary::ValueId>,
    route: EvidenceRoute,
    profile: &AdmissionProfile,
) -> Result<AcceptedFact, EvidenceError> {
    verify_obligation_with_machine_parameters(
        context,
        &obligation.obligation,
        &obligation.assumptions,
        &obligation.semantic_axioms,
        machine_parameter_values,
        route,
        profile,
    )
}

fn validate_component_shape<Member: Copy + Ord>(
    component: &RecursiveComponentObligation<Member>,
) -> Result<(), RecursiveComponentError<Member>> {
    if component.members.is_empty() {
        return Err(RecursiveComponentError::EmptyComponent);
    }
    if component
        .members
        .windows(2)
        .any(|members| members[0] >= members[1])
    {
        return Err(RecursiveComponentError::NonCanonicalMembers);
    }
    if component.edges.is_empty() {
        return Err(RecursiveComponentError::AcyclicComponent);
    }
    if component
        .edges
        .windows(2)
        .any(|edges| edges[0].decrease.obligation.id >= edges[1].decrease.obligation.id)
    {
        return Err(RecursiveComponentError::NonCanonicalObligationEdges);
    }
    let members = component.members.iter().copied().collect::<BTreeSet<_>>();
    let mut outgoing = members
        .iter()
        .copied()
        .map(|member| (member, Vec::new()))
        .collect::<BTreeMap<_, _>>();
    for edge in &component.edges {
        if !members.contains(&edge.caller) || !members.contains(&edge.callee) {
            return Err(RecursiveComponentError::EdgeOutsideComponent {
                caller: edge.caller,
                callee: edge.callee,
            });
        }
        outgoing
            .get_mut(&edge.caller)
            .expect("component member has an adjacency row")
            .push(edge.callee);
    }
    for start in &component.members {
        let mut reached = BTreeSet::new();
        let mut pending = vec![*start];
        while let Some(member) = pending.pop() {
            for next in &outgoing[&member] {
                if reached.insert(*next) {
                    pending.push(*next);
                }
            }
        }
        if !component
            .members
            .iter()
            .all(|member| reached.contains(member))
        {
            return Err(RecursiveComponentError::NotStronglyConnected);
        }
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecursiveComponentError<Member = ContractId> {
    EmptyComponent,
    NonCanonicalMembers,
    AcyclicComponent,
    NonCanonicalObligationEdges,
    EdgeOutsideComponent {
        caller: Member,
        callee: Member,
    },
    NotStronglyConnected,
    MissingRankingRelation,
    RankingRelationMismatch,
    WellFoundedness(EvidenceError),
    NonCanonicalCertificateEdges,
    MissingDecreaseEvidence(ObligationId),
    UnknownDecreaseEvidence(ObligationId),
    Decrease {
        obligation: ObligationId,
        error: EvidenceError,
    },
}

impl<Member: std::fmt::Debug> std::fmt::Display for RecursiveComponentError<Member> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl<Member: std::fmt::Debug> std::error::Error for RecursiveComponentError<Member> {}

#[cfg(test)]
mod tests {
    use semantic_vocabulary::{
        AdmissionSiteId, BlockId, ProfileDecisionId, PropositionId, PsiSemanticId,
    };

    use super::*;
    use crate::{
        AcceptedFactRoute, AdmissionAcceptance, AdmissionEvidence, AdmissionKind,
        AuthorizedAdmission, CertificateEnvelope, ObligationClass, ProofNode, ProofRule,
        ProofSystemMarker,
    };

    fn id<T: PsiSemanticId>(raw: u64) -> T {
        T::new(raw).expect("test identity")
    }

    fn proposition(raw: u64) -> Proposition {
        Proposition::Atom(id::<PropositionId>(raw))
    }

    fn proof_obligation(raw: u64, proposition: Proposition) -> CertificateObligation {
        CertificateObligation {
            obligation: Obligation {
                id: id(raw),
                proposition: proposition.clone(),
                class: ObligationClass::Derivable,
            },
            assumptions: vec![proposition],
            semantic_axioms: Vec::new(),
        }
    }

    fn certificate_route(proposition: Proposition) -> EvidenceRoute {
        EvidenceRoute::CertificateDerived(CertificateEnvelope {
            identity: id(90),
            proof_system_marker: ProofSystemMarker::CURRENT,
            proof: ProofNode {
                conclusion: proposition,
                rule: ProofRule::Assumption { index: 0 },
            },
        })
    }

    fn component() -> RecursiveComponentObligation {
        component_with_members::<ContractId>()
    }

    fn component_with_members<Member: PsiSemanticId>() -> RecursiveComponentObligation<Member> {
        RecursiveComponentObligation {
            members: vec![id(1), id(2)],
            ranking_relation: Some(id(10)),
            well_foundedness: proof_obligation(11, proposition(11)),
            edges: vec![
                RecursiveEdgeObligation {
                    caller: id(1),
                    callee: id(2),
                    decrease: proof_obligation(12, proposition(12)),
                },
                RecursiveEdgeObligation {
                    caller: id(2),
                    callee: id(1),
                    decrease: proof_obligation(13, proposition(13)),
                },
            ],
        }
    }

    fn certificate<Member>(
        component: &RecursiveComponentObligation<Member>,
    ) -> RecursiveComponentCertificate {
        RecursiveComponentCertificate {
            identity: id(20),
            ranking_relation: component.ranking_relation.expect("measured component"),
            well_foundedness: certificate_route(
                component.well_foundedness.obligation.proposition.clone(),
            ),
            edges: component
                .edges
                .iter()
                .map(|edge| RecursiveEdgeCertificate {
                    obligation: edge.decrease.obligation.id,
                    evidence: certificate_route(edge.decrease.obligation.proposition.clone()),
                })
                .collect(),
        }
    }

    #[test]
    fn one_certificate_checks_a_measured_mutual_component() {
        let component = component();
        let accepted = verify_recursive_component(
            &PropositionContext::default(),
            &component,
            certificate(&component),
            &AdmissionProfile::default(),
        )
        .expect("measured mutual recursion");
        assert_eq!(accepted.members, component.members);
        assert_eq!(accepted.decreases.len(), 2);
        assert!(matches!(
            accepted.well_foundedness.route,
            AcceptedFactRoute::CertificateDerived { .. }
        ));
    }

    #[test]
    fn block_component_retains_exact_members_and_accepted_facts() {
        let component = component_with_members::<BlockId>();
        let certificate = certificate(&component);
        let context = PropositionContext::default();
        let profile = AdmissionProfile::default();
        let accepted: RecursiveComponentAcceptance<BlockId> =
            verify_recursive_component(&context, &component, certificate.clone(), &profile)
                .expect("block identities retain the same admission checks");
        assert_eq!(accepted.members, component.members);
        assert_eq!(accepted.certificate, certificate.identity);
        assert_eq!(Some(accepted.ranking_relation), component.ranking_relation);
        assert_eq!(
            accepted.well_foundedness,
            verify_recursive_obligation(
                &context,
                &component.well_foundedness,
                &BTreeSet::new(),
                certificate.well_foundedness,
                &profile,
            )
            .unwrap()
        );
        let expected = component
            .edges
            .iter()
            .zip(certificate.edges)
            .map(|(edge, certificate)| {
                verify_recursive_obligation(
                    &context,
                    &edge.decrease,
                    &BTreeSet::new(),
                    certificate.evidence,
                    &profile,
                )
                .unwrap()
            })
            .collect::<Vec<_>>();
        assert_eq!(accepted.decreases, expected);
    }

    #[test]
    fn block_component_rejects_noncanonical_or_invalid_topology() {
        let original = component_with_members::<BlockId>();
        for mutation in 0..7 {
            let mut component = original.clone();
            let expected = match mutation {
                0 => {
                    component.members.clear();
                    RecursiveComponentError::EmptyComponent
                }
                1 => {
                    component.members.swap(0, 1);
                    RecursiveComponentError::NonCanonicalMembers
                }
                2 => {
                    component.members[1] = component.members[0];
                    RecursiveComponentError::NonCanonicalMembers
                }
                3 => {
                    component.edges.clear();
                    RecursiveComponentError::AcyclicComponent
                }
                4 => {
                    component.edges.swap(0, 1);
                    RecursiveComponentError::NonCanonicalObligationEdges
                }
                5 => {
                    component.edges[0].callee = id(99);
                    RecursiveComponentError::EdgeOutsideComponent {
                        caller: id::<BlockId>(1),
                        callee: id::<BlockId>(99),
                    }
                }
                6 => {
                    component.edges.pop();
                    RecursiveComponentError::NotStronglyConnected
                }
                _ => unreachable!(),
            };
            let error: RecursiveComponentError<BlockId> = verify_recursive_component(
                &PropositionContext::default(),
                &component,
                certificate(&original),
                &AdmissionProfile::default(),
            )
            .unwrap_err();
            assert_eq!(error, expected, "topology mutation {mutation}");
        }
    }

    #[test]
    fn block_component_rejects_substituted_or_noncanonical_evidence() {
        let component = component_with_members::<BlockId>();
        for mutation in 0..7 {
            let mut certificate = certificate(&component);
            match mutation {
                0 => certificate.ranking_relation = id(99),
                1 => {
                    certificate.edges.pop();
                }
                2 => certificate.edges.swap(0, 1),
                3 => certificate.edges.push(certificate.edges[1].clone()),
                4 => certificate.edges.push(RecursiveEdgeCertificate {
                    obligation: id(99),
                    evidence: certificate_route(proposition(99)),
                }),
                5 => certificate.well_foundedness = certificate_route(proposition(99)),
                6 => certificate.edges[0].evidence = certificate_route(proposition(99)),
                _ => unreachable!(),
            }
            let error = verify_recursive_component(
                &PropositionContext::default(),
                &component,
                certificate,
                &AdmissionProfile::default(),
            )
            .unwrap_err();
            match mutation {
                0 => assert_eq!(error, RecursiveComponentError::RankingRelationMismatch),
                1 => assert_eq!(
                    error,
                    RecursiveComponentError::MissingDecreaseEvidence(id(13))
                ),
                2 | 3 => assert_eq!(error, RecursiveComponentError::NonCanonicalCertificateEdges),
                4 => assert_eq!(
                    error,
                    RecursiveComponentError::UnknownDecreaseEvidence(id(99))
                ),
                5 => assert!(matches!(
                    error,
                    RecursiveComponentError::WellFoundedness(EvidenceError::Certificate(_))
                )),
                6 => assert!(matches!(
                    error,
                    RecursiveComponentError::Decrease {
                        obligation,
                        error: EvidenceError::Certificate(_),
                    } if obligation == id(12)
                )),
                _ => unreachable!(),
            }
        }
    }

    #[test]
    fn unmeasured_or_non_scc_cycles_reject() {
        let mut unmeasured = component();
        unmeasured.ranking_relation = None;
        assert_eq!(
            verify_recursive_component(
                &PropositionContext::default(),
                &unmeasured,
                certificate(&component()),
                &AdmissionProfile::default(),
            ),
            Err(RecursiveComponentError::MissingRankingRelation)
        );

        let mut not_scc = component();
        not_scc.edges.pop();
        assert_eq!(
            verify_recursive_component(
                &PropositionContext::default(),
                &not_scc,
                certificate(&not_scc),
                &AdmissionProfile::default(),
            ),
            Err(RecursiveComponentError::NotStronglyConnected)
        );
    }

    #[test]
    fn relation_substitution_and_missing_or_perturbed_edges_reject() {
        let component = component();
        let mut wrong_relation = certificate(&component);
        wrong_relation.ranking_relation = id(99);
        assert_eq!(
            verify_recursive_component(
                &PropositionContext::default(),
                &component,
                wrong_relation,
                &AdmissionProfile::default(),
            ),
            Err(RecursiveComponentError::RankingRelationMismatch)
        );

        let mut wrong_well_foundedness = certificate(&component);
        wrong_well_foundedness.well_foundedness = certificate_route(proposition(99));
        assert!(matches!(
            verify_recursive_component(
                &PropositionContext::default(),
                &component,
                wrong_well_foundedness,
                &AdmissionProfile::default(),
            ),
            Err(RecursiveComponentError::WellFoundedness(
                EvidenceError::Certificate(_)
            ))
        ));

        let mut missing = certificate(&component);
        missing.edges.pop();
        assert_eq!(
            verify_recursive_component(
                &PropositionContext::default(),
                &component,
                missing,
                &AdmissionProfile::default(),
            ),
            Err(RecursiveComponentError::MissingDecreaseEvidence(id(13)))
        );

        let mut perturbed = certificate(&component);
        perturbed.edges[0].evidence = certificate_route(proposition(99));
        assert!(matches!(
            verify_recursive_component(
                &PropositionContext::default(),
                &component,
                perturbed,
                &AdmissionProfile::default(),
            ),
            Err(RecursiveComponentError::Decrease {
                obligation,
                error: EvidenceError::Certificate(_),
            }) if obligation == id(12)
        ));
    }

    #[test]
    fn admitted_well_foundedness_remains_in_component_provenance() {
        assert_admitted_well_foundedness_provenance::<ContractId>();
    }

    #[test]
    fn block_component_retains_admitted_well_foundedness_provenance() {
        assert_admitted_well_foundedness_provenance::<BlockId>();
    }

    fn assert_admitted_well_foundedness_provenance<Member: PsiSemanticId + std::fmt::Debug>() {
        let mut component = component_with_members::<Member>();
        let site = id::<AdmissionSiteId>(30);
        let authority = id(31);
        component.well_foundedness.obligation.class =
            ObligationClass::AdmissionAuthorized(AuthorizedAdmission {
                site,
                kind: AdmissionKind::ProviderFact,
                authority_identity: authority,
            });
        component.well_foundedness.assumptions.clear();
        let admission = AdmissionEvidence {
            site,
            kind: AdmissionKind::ProviderFact,
            authority_identity: authority,
            evidence_identity: id(32),
            profile_decision: id::<ProfileDecisionId>(33),
        };
        let profile = AdmissionProfile::from_acceptances([AdmissionAcceptance {
            site,
            evidence_identity: admission.evidence_identity,
            profile_decision: admission.profile_decision,
        }]);
        let mut certificate = certificate(&component);
        certificate.well_foundedness = EvidenceRoute::Admitted(admission);
        let accepted = verify_recursive_component(
            &PropositionContext::default(),
            &component,
            certificate,
            &profile,
        )
        .expect("authorized well-foundedness admission");
        assert_eq!(
            accepted.well_foundedness.route,
            AcceptedFactRoute::Admitted(admission)
        );
    }
}

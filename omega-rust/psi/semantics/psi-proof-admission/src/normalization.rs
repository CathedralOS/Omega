use std::collections::BTreeMap;

use psi_core::{EvidenceIdentity, PropositionContext};

use crate::{
    AcceptedFact, AcceptedFactRoute, AdmissionProfile, CertificateObligation, EvidenceError,
    EvidenceRoute, verify_obligation,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizationLawObligation {
    pub identity: EvidenceIdentity,
    pub proof: CertificateObligation,
}

/// Reconstruction fixes the selected conformance, the exact laws consumed by
/// normalization, and the resulting proposition. The certificate cannot
/// introduce a more convenient law set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizationObligation {
    pub conformance: EvidenceIdentity,
    pub laws: Vec<NormalizationLawObligation>,
    pub conclusion: CertificateObligation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizationLawCertificate {
    pub identity: EvidenceIdentity,
    pub evidence: EvidenceRoute,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizationCertificate {
    pub identity: EvidenceIdentity,
    pub conformance: EvidenceIdentity,
    pub laws: Vec<NormalizationLawCertificate>,
    pub conclusion: EvidenceRoute,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizationLawAcceptance {
    pub identity: EvidenceIdentity,
    pub fact: AcceptedFact,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizationAcceptance {
    pub certificate: EvidenceIdentity,
    pub conformance: EvidenceIdentity,
    pub laws: Vec<NormalizationLawAcceptance>,
    pub conclusion: AcceptedFact,
}

pub fn verify_normalization(
    context: &PropositionContext,
    obligation: &NormalizationObligation,
    certificate: NormalizationCertificate,
    profile: &AdmissionProfile,
) -> Result<NormalizationAcceptance, NormalizationError> {
    validate_law_order(obligation)?;
    if certificate.conformance != obligation.conformance {
        return Err(NormalizationError::ConformanceMismatch);
    }
    let mut evidence = BTreeMap::new();
    let mut previous = None;
    for law in certificate.laws {
        if previous.is_some_and(|previous| previous >= law.identity) {
            return Err(NormalizationError::NonCanonicalCertificateLaws);
        }
        previous = Some(law.identity);
        evidence.insert(law.identity, law.evidence);
    }

    let mut laws = Vec::with_capacity(obligation.laws.len());
    for law in &obligation.laws {
        let route = evidence
            .remove(&law.identity)
            .ok_or(NormalizationError::MissingLawEvidence(law.identity))?;
        let fact = verify_obligation(
            context,
            &law.proof.obligation,
            &law.proof.assumptions,
            &law.proof.semantic_axioms,
            route,
            profile,
        )
        .map_err(|error| NormalizationError::Law {
            identity: law.identity,
            error,
        })?;
        laws.push(NormalizationLawAcceptance {
            identity: law.identity,
            fact,
        });
    }
    if let Some(identity) = evidence.keys().next().copied() {
        return Err(NormalizationError::UnknownLawEvidence(identity));
    }

    let base_assumption_count = obligation.conclusion.assumptions.len();
    let mut assumptions = obligation.conclusion.assumptions.clone();
    assumptions.extend(laws.iter().map(|law| law.fact.proposition.clone()));
    let conclusion = verify_obligation(
        context,
        &obligation.conclusion.obligation,
        &assumptions,
        &obligation.conclusion.semantic_axioms,
        certificate.conclusion,
        profile,
    )
    .map_err(NormalizationError::Conclusion)?;

    if !laws.is_empty() {
        let AcceptedFactRoute::CertificateDerived { acceptance, .. } = &conclusion.route else {
            return Err(NormalizationError::ConclusionDoesNotCiteLaws);
        };
        for (offset, law) in laws.iter().enumerate() {
            let index = base_assumption_count + offset;
            if !acceptance.assumptions.iter().any(|premise| {
                premise.index == index && premise.proposition == law.fact.proposition
            }) {
                return Err(NormalizationError::UncitedLaw(law.identity));
            }
        }
    }

    Ok(NormalizationAcceptance {
        certificate: certificate.identity,
        conformance: certificate.conformance,
        laws,
        conclusion,
    })
}

fn validate_law_order(obligation: &NormalizationObligation) -> Result<(), NormalizationError> {
    if obligation
        .laws
        .windows(2)
        .any(|laws| laws[0].identity >= laws[1].identity)
    {
        return Err(NormalizationError::NonCanonicalObligationLaws);
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NormalizationError {
    NonCanonicalObligationLaws,
    ConformanceMismatch,
    NonCanonicalCertificateLaws,
    MissingLawEvidence(EvidenceIdentity),
    UnknownLawEvidence(EvidenceIdentity),
    Law {
        identity: EvidenceIdentity,
        error: EvidenceError,
    },
    Conclusion(EvidenceError),
    ConclusionDoesNotCiteLaws,
    UncitedLaw(EvidenceIdentity),
}

impl std::fmt::Display for NormalizationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for NormalizationError {}

#[cfg(test)]
mod tests {
    use psi_core::{
        AdmissionSiteId, ObligationId, ProfileDecisionId, Proposition, PropositionId, PsiSemanticId,
    };

    use super::*;
    use crate::{
        AdmissionAcceptance, AdmissionEvidence, AdmissionKind, AuthorizedAdmission,
        CertificateEnvelope, Obligation, ObligationClass, ProofNode, ProofRule, ProofSystemMarker,
    };

    fn id<T: PsiSemanticId>(raw: u64) -> T {
        T::new(raw).expect("test identity")
    }

    fn proposition(raw: u64) -> Proposition {
        Proposition::Atom(id::<PropositionId>(raw))
    }

    fn obligation(raw: u64, proposition: Proposition) -> CertificateObligation {
        CertificateObligation {
            obligation: Obligation {
                id: id::<ObligationId>(raw),
                proposition,
                class: ObligationClass::Derivable,
            },
            assumptions: Vec::new(),
            semantic_axioms: Vec::new(),
        }
    }

    fn assumption_route(proposition: Proposition, index: usize) -> EvidenceRoute {
        EvidenceRoute::CertificateDerived(CertificateEnvelope {
            identity: id(80),
            proof_system_marker: ProofSystemMarker::CURRENT,
            proof: ProofNode {
                conclusion: proposition,
                rule: ProofRule::Assumption { index },
            },
        })
    }

    fn normalization() -> (NormalizationObligation, NormalizationCertificate) {
        let law = proposition(1);
        let obligation = NormalizationObligation {
            conformance: id(10),
            laws: vec![NormalizationLawObligation {
                identity: id(11),
                proof: CertificateObligation {
                    assumptions: vec![law.clone()],
                    ..obligation(11, law.clone())
                },
            }],
            conclusion: obligation(12, law.clone()),
        };
        let certificate = NormalizationCertificate {
            identity: id(20),
            conformance: id(10),
            laws: vec![NormalizationLawCertificate {
                identity: id(11),
                evidence: assumption_route(law.clone(), 0),
            }],
            conclusion: assumption_route(law, 0),
        };
        (obligation, certificate)
    }

    #[test]
    fn exact_conformance_and_law_are_recorded() {
        let (obligation, certificate) = normalization();
        let accepted = verify_normalization(
            &PropositionContext::default(),
            &obligation,
            certificate,
            &AdmissionProfile::default(),
        )
        .expect("licensed normalization");
        assert_eq!(accepted.conformance, id(10));
        assert_eq!(accepted.laws[0].identity, id(11));
        let AcceptedFactRoute::CertificateDerived { acceptance, .. } = accepted.conclusion.route
        else {
            panic!("normalization conclusion is certificate-derived");
        };
        assert_eq!(acceptance.assumptions[0].proposition, proposition(1));
    }

    #[test]
    fn conformance_law_substitution_and_uncited_laws_reject() {
        let (obligation, mut wrong_conformance) = normalization();
        wrong_conformance.conformance = id(99);
        assert_eq!(
            verify_normalization(
                &PropositionContext::default(),
                &obligation,
                wrong_conformance,
                &AdmissionProfile::default(),
            ),
            Err(NormalizationError::ConformanceMismatch)
        );

        let (_, mut wrong_law) = normalization();
        wrong_law.laws[0].identity = id(99);
        assert_eq!(
            verify_normalization(
                &PropositionContext::default(),
                &obligation,
                wrong_law,
                &AdmissionProfile::default(),
            ),
            Err(NormalizationError::MissingLawEvidence(id(11)))
        );

        let (mut uncited_obligation, mut uncited) = normalization();
        uncited_obligation.conclusion.obligation.proposition = Proposition::Truth;
        uncited.conclusion = EvidenceRoute::KernelDerived(crate::PrimitiveJudgment::Truth);
        assert_eq!(
            verify_normalization(
                &PropositionContext::default(),
                &uncited_obligation,
                uncited,
                &AdmissionProfile::default(),
            ),
            Err(NormalizationError::ConclusionDoesNotCiteLaws)
        );
    }

    #[test]
    fn admitted_law_is_retained_as_a_normalization_dependency() {
        let (mut obligation, mut certificate) = normalization();
        let site = id::<AdmissionSiteId>(30);
        let authority = id(31);
        obligation.laws[0].proof.obligation.class =
            ObligationClass::AdmissionAuthorized(AuthorizedAdmission {
                site,
                kind: AdmissionKind::ProviderFact,
                authority_identity: authority,
            });
        obligation.laws[0].proof.assumptions.clear();
        let admission = AdmissionEvidence {
            site,
            kind: AdmissionKind::ProviderFact,
            authority_identity: authority,
            evidence_identity: id(32),
            profile_decision: id::<ProfileDecisionId>(33),
        };
        certificate.laws[0].evidence = EvidenceRoute::Admitted(admission);
        let profile = AdmissionProfile::from_acceptances([AdmissionAcceptance {
            site,
            evidence_identity: admission.evidence_identity,
            profile_decision: admission.profile_decision,
        }]);
        let accepted = verify_normalization(
            &PropositionContext::default(),
            &obligation,
            certificate,
            &profile,
        )
        .expect("admission-dependent normalization");
        assert_eq!(
            accepted.laws[0].fact.route,
            AcceptedFactRoute::Admitted(admission)
        );
        let AcceptedFactRoute::CertificateDerived { acceptance, .. } = accepted.conclusion.route
        else {
            panic!("normalization conclusion is certificate-derived");
        };
        assert!(
            acceptance
                .assumptions
                .iter()
                .any(|premise| premise.proposition == proposition(1))
        );
    }
}

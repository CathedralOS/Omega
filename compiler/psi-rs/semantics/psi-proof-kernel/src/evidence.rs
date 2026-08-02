use std::collections::BTreeSet;
use std::num::NonZeroU16;

use psi_core::{
    AdmissionSiteId, EvidenceIdentity, ObligationId, ProfileDecisionId, Proposition,
    PropositionContext,
};

use crate::{PrimitiveJudgment, ProofNode, check_certificate, decide_primitive};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AdmissionKind {
    ForeignBoundaryGuarantee,
    ProviderFact,
    CheckedAssemblyClaim,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuthorizedAdmission {
    pub site: AdmissionSiteId,
    pub kind: AdmissionKind,
    pub authority_identity: EvidenceIdentity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObligationClass {
    Derivable,
    AdmissionAuthorized(AuthorizedAdmission),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Obligation {
    pub id: ObligationId,
    pub proposition: Proposition,
    pub class: ObligationClass,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProofSystemVersion(NonZeroU16);

impl ProofSystemVersion {
    pub const CURRENT: Self = Self(NonZeroU16::MIN);

    pub fn new(raw: u16) -> Option<Self> {
        NonZeroU16::new(raw).map(Self)
    }

    pub const fn get(self) -> u16 {
        self.0.get()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CertificateEnvelope {
    pub identity: EvidenceIdentity,
    pub proof_system_version: ProofSystemVersion,
    pub proof: ProofNode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdmissionEvidence {
    pub site: AdmissionSiteId,
    pub kind: AdmissionKind,
    pub authority_identity: EvidenceIdentity,
    pub evidence_identity: EvidenceIdentity,
    pub profile_decision: ProfileDecisionId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct AdmissionAcceptance {
    pub site: AdmissionSiteId,
    pub evidence_identity: EvidenceIdentity,
    pub profile_decision: ProfileDecisionId,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AdmissionProfile {
    accepted: BTreeSet<AdmissionAcceptance>,
}

impl AdmissionProfile {
    pub fn from_acceptances(acceptances: impl IntoIterator<Item = AdmissionAcceptance>) -> Self {
        Self {
            accepted: acceptances.into_iter().collect(),
        }
    }

    fn accepts(&self, evidence: AdmissionEvidence) -> bool {
        self.accepted.contains(&AdmissionAcceptance {
            site: evidence.site,
            evidence_identity: evidence.evidence_identity,
            profile_decision: evidence.profile_decision,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvidenceRoute {
    KernelDerived(PrimitiveJudgment),
    CertificateDerived(CertificateEnvelope),
    Admitted(AdmissionEvidence),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AcceptedFactRoute {
    KernelDerived(PrimitiveJudgment),
    CertificateDerived {
        identity: EvidenceIdentity,
        proof_system_version: ProofSystemVersion,
    },
    Admitted(AdmissionEvidence),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceptedFact {
    pub obligation: ObligationId,
    pub proposition: Proposition,
    pub route: AcceptedFactRoute,
}

pub fn verify_obligation(
    context: &PropositionContext,
    obligation: &Obligation,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    route: EvidenceRoute,
    profile: &AdmissionProfile,
) -> Result<AcceptedFact, EvidenceError> {
    context
        .validate(&obligation.proposition)
        .map_err(EvidenceError::MalformedProposition)?;
    let route = match route {
        EvidenceRoute::KernelDerived(judgment) => {
            decide_primitive(context, &obligation.proposition, judgment)
                .map_err(EvidenceError::Kernel)?;
            AcceptedFactRoute::KernelDerived(judgment)
        }
        EvidenceRoute::CertificateDerived(certificate) => {
            if certificate.proof_system_version != ProofSystemVersion::CURRENT {
                return Err(EvidenceError::UnsupportedProofSystemVersion(
                    certificate.proof_system_version,
                ));
            }
            check_certificate(
                context,
                &obligation.proposition,
                assumptions,
                semantic_axioms,
                &certificate.proof,
            )
            .map_err(EvidenceError::Certificate)?;
            AcceptedFactRoute::CertificateDerived {
                identity: certificate.identity,
                proof_system_version: certificate.proof_system_version,
            }
        }
        EvidenceRoute::Admitted(evidence) => {
            if primitive_derivation(context, &obligation.proposition).is_some() {
                return Err(EvidenceError::AdmissionCannotReplaceDerivation);
            }
            let ObligationClass::AdmissionAuthorized(authorized) = obligation.class else {
                return Err(EvidenceError::AdmissionCannotReplaceDerivation);
            };
            if evidence.site != authorized.site
                || evidence.kind != authorized.kind
                || evidence.authority_identity != authorized.authority_identity
            {
                return Err(EvidenceError::AdmissionSiteMismatch);
            }
            if !profile.accepts(evidence) {
                return Err(EvidenceError::AdmissionNotAcceptedByProfile);
            }
            AcceptedFactRoute::Admitted(evidence)
        }
    };
    Ok(AcceptedFact {
        obligation: obligation.id,
        proposition: obligation.proposition.clone(),
        route,
    })
}

fn primitive_derivation(
    context: &PropositionContext,
    proposition: &Proposition,
) -> Option<PrimitiveJudgment> {
    [
        PrimitiveJudgment::Truth,
        PrimitiveJudgment::ReflexiveEquality,
        PrimitiveJudgment::ClosedIntegerRelation,
    ]
    .into_iter()
    .find(|judgment| decide_primitive(context, proposition, *judgment).is_ok())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvidenceError {
    MalformedProposition(psi_core::PropositionError),
    Kernel(crate::KernelError),
    Certificate(crate::ProofError),
    UnsupportedProofSystemVersion(ProofSystemVersion),
    AdmissionCannotReplaceDerivation,
    AdmissionSiteMismatch,
    AdmissionNotAcceptedByProfile,
}

impl std::fmt::Display for EvidenceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for EvidenceError {}

#[cfg(test)]
mod tests {
    use super::*;
    fn obligation(proposition: Proposition, class: ObligationClass) -> Obligation {
        Obligation {
            id: ObligationId::new(1).expect("obligation identity"),
            proposition,
            class,
        }
    }

    fn admission() -> AdmissionEvidence {
        AdmissionEvidence {
            site: AdmissionSiteId::new(2).expect("site identity"),
            kind: AdmissionKind::ProviderFact,
            authority_identity: EvidenceIdentity::new(3).expect("authority identity"),
            evidence_identity: EvidenceIdentity::new(4).expect("evidence identity"),
            profile_decision: ProfileDecisionId::new(5).expect("profile decision"),
        }
    }

    #[test]
    fn admission_cannot_replace_a_derivable_obligation() {
        let evidence = admission();
        let authorized = AuthorizedAdmission {
            site: evidence.site,
            kind: evidence.kind,
            authority_identity: evidence.authority_identity,
        };
        let profile = AdmissionProfile::from_acceptances([AdmissionAcceptance {
            site: evidence.site,
            evidence_identity: evidence.evidence_identity,
            profile_decision: evidence.profile_decision,
        }]);
        assert_eq!(
            verify_obligation(
                &PropositionContext::default(),
                &obligation(
                    Proposition::Truth,
                    ObligationClass::AdmissionAuthorized(authorized),
                ),
                &[],
                &[],
                EvidenceRoute::Admitted(evidence),
                &profile,
            )
            .expect_err("admission must not replace derivation"),
            EvidenceError::AdmissionCannotReplaceDerivation
        );
    }

    #[test]
    fn admitted_fact_requires_exact_site_authority_and_profile_acceptance() {
        let evidence = admission();
        let authorized = AuthorizedAdmission {
            site: evidence.site,
            kind: evidence.kind,
            authority_identity: evidence.authority_identity,
        };
        let proposition =
            Proposition::Atom(psi_core::PropositionId::new(6).expect("proposition identity"));
        let profile = AdmissionProfile::from_acceptances([AdmissionAcceptance {
            site: evidence.site,
            evidence_identity: evidence.evidence_identity,
            profile_decision: evidence.profile_decision,
        }]);
        let accepted = verify_obligation(
            &PropositionContext::default(),
            &obligation(
                proposition,
                ObligationClass::AdmissionAuthorized(authorized),
            ),
            &[],
            &[],
            EvidenceRoute::Admitted(evidence),
            &profile,
        )
        .expect("exact admission");
        assert_eq!(accepted.route, AcceptedFactRoute::Admitted(evidence));
    }

    #[test]
    fn admission_rejects_authority_substitution_and_missing_profile_acceptance() {
        let evidence = admission();
        let proposition =
            Proposition::Atom(psi_core::PropositionId::new(6).expect("proposition identity"));
        let authorized = AuthorizedAdmission {
            site: evidence.site,
            kind: evidence.kind,
            authority_identity: EvidenceIdentity::new(99).expect("other authority"),
        };
        assert_eq!(
            verify_obligation(
                &PropositionContext::default(),
                &obligation(
                    proposition.clone(),
                    ObligationClass::AdmissionAuthorized(authorized),
                ),
                &[],
                &[],
                EvidenceRoute::Admitted(evidence),
                &AdmissionProfile::default(),
            )
            .expect_err("substituted authority must reject"),
            EvidenceError::AdmissionSiteMismatch
        );

        let authorized = AuthorizedAdmission {
            site: evidence.site,
            kind: evidence.kind,
            authority_identity: evidence.authority_identity,
        };
        assert_eq!(
            verify_obligation(
                &PropositionContext::default(),
                &obligation(
                    proposition,
                    ObligationClass::AdmissionAuthorized(authorized),
                ),
                &[],
                &[],
                EvidenceRoute::Admitted(evidence),
                &AdmissionProfile::default(),
            )
            .expect_err("unaccepted evidence must reject"),
            EvidenceError::AdmissionNotAcceptedByProfile
        );
    }

    #[test]
    fn certificate_version_is_checked_separately_from_semantics() {
        let certificate = CertificateEnvelope {
            identity: EvidenceIdentity::new(7).expect("certificate identity"),
            proof_system_version: ProofSystemVersion::new(2).expect("version 2"),
            proof: ProofNode {
                conclusion: Proposition::Truth,
                rule: crate::ProofRule::Primitive(PrimitiveJudgment::Truth),
            },
        };
        assert_eq!(
            verify_obligation(
                &PropositionContext::default(),
                &obligation(Proposition::Truth, ObligationClass::Derivable),
                &[],
                &[],
                EvidenceRoute::CertificateDerived(certificate),
                &AdmissionProfile::default(),
            )
            .expect_err("unsupported proof-system version must reject"),
            EvidenceError::UnsupportedProofSystemVersion(
                ProofSystemVersion::new(2).expect("version 2")
            )
        );
    }
}

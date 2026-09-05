use std::collections::BTreeSet;
pub use terminal_psi::ProofSystemMarker;
pub use terminal_psi::{AdmissionEvidence, AdmissionKind, CertificateEnvelope, EvidenceRoute};

use semantic_vocabulary::{
    AdmissionSiteId, EvidenceIdentity, ObligationId, ProfileDecisionId, Proposition,
    PropositionContext,
};

use crate::{
    CertificateAcceptance, PrimitiveJudgment, accept_certificate_with_machine_parameters,
    decide_primitive,
};

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
pub enum AcceptedFactRoute {
    KernelDerived(PrimitiveJudgment),
    CertificateDerived {
        identity: EvidenceIdentity,
        proof_system_marker: ProofSystemMarker,
        acceptance: CertificateAcceptance,
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
    verify_obligation_with_machine_parameters(
        context,
        obligation,
        assumptions,
        semantic_axioms,
        &BTreeSet::new(),
        route,
        profile,
    )
}

pub fn verify_obligation_with_machine_parameters(
    context: &PropositionContext,
    obligation: &Obligation,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    machine_parameter_values: &BTreeSet<semantic_vocabulary::ValueId>,
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
            let acceptance = accept_certificate_with_machine_parameters(
                context,
                &obligation.proposition,
                assumptions,
                semantic_axioms,
                machine_parameter_values,
                &certificate.proof,
            )
            .map_err(EvidenceError::Certificate)?;
            AcceptedFactRoute::CertificateDerived {
                identity: certificate.identity,
                proof_system_marker: certificate.proof_system_marker,
                acceptance,
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
    MalformedProposition(semantic_vocabulary::PropositionError),
    Kernel(crate::KernelError),
    Certificate(crate::ProofError),
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
        let proposition = Proposition::Atom(
            semantic_vocabulary::PropositionId::new(6).expect("proposition identity"),
        );
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
        let proposition = Proposition::Atom(
            semantic_vocabulary::PropositionId::new(6).expect("proposition identity"),
        );
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
    fn proof_system_marker_has_no_compatibility_ladder() {
        assert_eq!(ProofSystemMarker::new(1), Some(ProofSystemMarker::CURRENT));
        assert_eq!(ProofSystemMarker::new(0), None);
        assert_eq!(ProofSystemMarker::new(2), None);
    }
}

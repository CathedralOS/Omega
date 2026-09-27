//! Shared producer-side crash-certificate supply for verification tests.
//!
//! Verification consumes a certificate roster from the proof bundle; these
//! helpers play the producer's role in tests — reconstruct the exact
//! questions the verifier will ask and answer them through the shared
//! denotation-lane search, mirroring
//! `checked_trees_to_lowered_psi::produce_crash_obligation_evidence`. A goal
//! the search cannot prove keeps an empty roster slot so verification still
//! rejects rather than silently passing.

use proof_admission::{
    AdmissionProfile, CertificateEnvelope, EvidenceRoute, PrimitiveJudgment, ProofNode, ProofRule,
    ProofSystemMarker,
};
use semantic_vocabulary::{EvidenceIdentity, Proposition};
use terminal_psi::{
    CrashCertificate, CrashObligationEvidence, CrashObligationOwner, ObligationEvidence,
    ProofBundle, TerminalModule,
};
use terminal_verifier::{
    CrashObligationQuestion, VerificationError, reconstruct_crash_obligations,
    reconstruct_terminal_obligations, verify_module,
};

/// The produced crash-obligation roster alone, in canonical owner order, for
/// splicing into a bundle that already carries other evidence.
pub(crate) fn crash_roster(module: &TerminalModule) -> Vec<CrashObligationEvidence> {
    reconstruct_crash_obligations(module)
        .expect("fixture modules validate before their questions reconstruct")
        .iter()
        .map(|question| {
            let produce =
                |goal: &Proposition, semantic_axioms: &[Proposition]| -> Vec<CrashCertificate> {
                    question.context.as_ref().map_or_else(Vec::new, |context| {
                        proof_admission::produce_denotation_certificates(
                            context,
                            goal,
                            &question.requirements,
                            semantic_axioms,
                        )
                    })
                };
            let (coverage, refutation) = match &question.question {
                CrashObligationQuestion::Site { guards, paths } => (
                    guards
                        .iter()
                        .map(|guard| {
                            paths
                                .iter()
                                .flat_map(|axioms| produce(guard, axioms))
                                .collect()
                        })
                        .collect(),
                    paths
                        .iter()
                        .map(|axioms| produce(&Proposition::Falsehood, axioms))
                        .collect(),
                ),
                CrashObligationQuestion::Continuation {
                    coverage_goals,
                    refutation_goal,
                } => (
                    coverage_goals
                        .iter()
                        .map(|goal| produce(goal, &[]))
                        .collect(),
                    refutation_goal
                        .iter()
                        .map(|goal| produce(goal, &[]))
                        .collect(),
                ),
            };
            CrashObligationEvidence {
                owner: question.owner,
                coverage,
                refutation,
            }
        })
        .collect()
}

/// `base` with the produced crash roster attached.
pub(crate) fn crash_bundle(module: &TerminalModule, mut base: ProofBundle) -> ProofBundle {
    base.crash_obligations = crash_roster(module);
    base
}

/// A proof bundle whose crash-obligation roster is produced against the
/// exact module under test. Every other section stays empty.
pub(crate) fn produced_crash_bundle(module: &TerminalModule) -> ProofBundle {
    crash_bundle(module, ProofBundle::default())
}

/// Verify a module whose only evidence is the produced crash roster.
pub(crate) fn verify_with_crash_supply(module: &TerminalModule) {
    verify_module(
        module,
        &produced_crash_bundle(module),
        &AdmissionProfile::default(),
    )
    .expect("the produced crash roster discharges the reconstructed question");
}

/// Assert both roster rejections for one undischarged module under an
/// arbitrary verification entry: the absent row is a missing-evidence
/// rejection and the producer's own supply — which cannot prove the
/// reconstructed question — is a rejected-evidence one. `base` carries the
/// bundle's other evidence sections the fixture requires. Returns the
/// question's owner for caller-side shape assertions.
pub(crate) fn rejected_crash_owner_under(
    module: &TerminalModule,
    base: &ProofBundle,
    verify: impl Fn(&TerminalModule, &ProofBundle) -> Result<(), VerificationError>,
) -> CrashObligationOwner {
    let missing = verify(module, base).expect_err("a missing crash-obligation row rejects");
    let VerificationError::MissingCrashObligationEvidence(owner) = missing else {
        panic!("expected missing crash-obligation evidence, got {missing:?}");
    };
    let bundle = crash_bundle(module, base.clone());
    let rejected =
        verify(module, &bundle).expect_err("an undischarged crash-obligation row rejects");
    let VerificationError::RejectedCrashObligationEvidence {
        owner: rejected_owner,
    } = rejected
    else {
        panic!("expected rejected crash-obligation evidence, got {rejected:?}");
    };
    assert_eq!(owner, rejected_owner);
    owner
}

/// `rejected_crash_owner_under` under ordinary execution verification with an
/// otherwise empty bundle.
pub(crate) fn rejected_crash_owner(module: &TerminalModule) -> CrashObligationOwner {
    rejected_crash_owner_under(module, &ProofBundle::default(), |module, bundle| {
        verify_module(module, bundle, &AdmissionProfile::default()).map(|_| ())
    })
}

/// Evidence rows for reconstructed terminal obligations that cite their goal
/// verbatim — `Truth`, a reflexive equality, or a proposition already present
/// in the site's own requirements or reconstructed axiom roster. Obligations
/// needing real proof search stay pending so their rejection still surfaces.
/// This is the same trivial discharge the producer's `operation_proofs`
/// fallback performs.
pub(crate) fn simple_evidence_bundle(module: &TerminalModule) -> ProofBundle {
    let mut bundle = ProofBundle::default();
    let Ok(obligations) = reconstruct_terminal_obligations(module) else {
        return bundle;
    };
    for site in obligations.obligations() {
        let conclusion = site.obligation.proposition.clone();
        let rule = if conclusion == Proposition::Truth {
            ProofRule::Primitive(PrimitiveJudgment::Truth)
        } else if matches!(&conclusion, Proposition::Equal(left, right) if left == right) {
            ProofRule::Primitive(PrimitiveJudgment::ReflexiveEquality)
        } else if let Some(index) = site
            .semantic_axioms
            .iter()
            .position(|axiom| *axiom == conclusion)
        {
            ProofRule::SemanticAxiom { index }
        } else if let Some(index) = site
            .requirements
            .iter()
            .position(|requirement| *requirement == conclusion)
        {
            ProofRule::Assumption { index }
        } else {
            continue;
        };
        bundle.evidence.push(ObligationEvidence {
            obligation: site.obligation.id,
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: EvidenceIdentity::new(site.obligation.id.get())
                    .expect("reconstructed obligations have nonzero identities"),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: ProofNode { conclusion, rule },
            }),
        });
    }
    bundle
}

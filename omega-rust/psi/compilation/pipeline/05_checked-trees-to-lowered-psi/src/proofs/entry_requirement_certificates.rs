//! Producer-side crash entry-requirement certificates.
//!
//! The verifier checks crash-site guards and call-continuation coverage from
//! certificates the producer supplies; it re-decides supplied nodes against
//! goals it reconstructs itself and never searches inside a check. This
//! module is the producer-facing spelling of that split: the bounded
//! denotation-lane search itself lives in
//! `proof_admission::certificate_search`, shared by every producer, and this
//! wrapper keeps the lowering pipeline's certificate type and function
//! names. A certificate produced here is accepted exactly when the
//! verifier's recorded-lane replay accepts the identical
//! `(with_value_equalities, proof)` pair.

use proof_admission::ProofNode;
use semantic_vocabulary::{Proposition, PropositionContext};
use terminal_psi::{CrashCertificate, CrashObligationEvidence, TerminalModule};

/// A producer-supplied certificate for one crash goal: which denotation lane
/// the producing search ran under and the proof node it emitted. Consumers
/// re-run only the recorded conversion and re-decide the node; they never
/// search for a route themselves.
#[derive(Clone)]
pub struct EntryRequirementCertificate {
    certificate: CrashCertificate,
}

impl EntryRequirementCertificate {
    /// Whether the producing search ran under value-equality transport. The
    /// accepting side replays the identical conversion.
    pub fn with_value_equalities(&self) -> bool {
        self.certificate.with_value_equalities
    }

    /// The emitted proof node.
    pub fn proof(&self) -> &ProofNode {
        &self.certificate.proof
    }

    /// The wire form stored in the proof bundle's crash-obligation roster.
    pub fn into_certificate(self) -> CrashCertificate {
        self.certificate
    }
}

impl From<CrashCertificate> for EntryRequirementCertificate {
    fn from(certificate: CrashCertificate) -> Self {
        Self { certificate }
    }
}

/// The producer stage of the crash ledger: bounded proof search over the
/// denotation lanes, recording which lane each emitted node was built under.
/// An empty supply is not a rejection verdict — the consumer's check of a
/// supplied node is what grants coverage, and no supply means none passes.
pub fn produce_entry_requirement_certificates(
    context: &PropositionContext,
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Vec<EntryRequirementCertificate> {
    proof_admission::produce_denotation_certificates(context, goal, requirements, semantic_axioms)
        .into_iter()
        .map(EntryRequirementCertificate::from)
        .collect()
}

/// Check a supplied crash certificate without searching. The recorded
/// denotation lane is part of the certificate: a node produced under equality
/// transport is replayed against that conversion exactly as produced. This is
/// the producer-side mirror of the verifier's certificate replay; the
/// accepting path runs the same replay over the supply it receives.
pub fn check_entry_requirement_certificate(
    context: &PropositionContext,
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
    certificate: &EntryRequirementCertificate,
) -> bool {
    proof_admission::check_denotation_certificate(
        context,
        goal,
        requirements,
        semantic_axioms,
        &certificate.certificate,
    )
}

/// Why the producer cannot attach a crash-obligation roster to a module.
#[derive(Debug)]
pub enum CrashRosterError {
    /// The module did not validate, so no question could be reconstructed.
    Module(terminal_verifier::ModuleError),
    /// The producer's own replay found a reconstructed question no produced
    /// certificate discharges — the roster would be rejected by every
    /// receiver, so none is emitted. Owners arrive in canonical order.
    Undischarged(Vec<terminal_psi::CrashObligationOwner>),
}

impl From<terminal_verifier::ModuleError> for CrashRosterError {
    fn from(error: terminal_verifier::ModuleError) -> Self {
        Self::Module(error)
    }
}

/// Produce the complete crash-obligation roster for one lowered module.
///
/// The producer reconstructs the exact questions verification asks — over
/// the validated artifact bytes' decoded module, not producer-side state —
/// searches each goal under the shared denotation lanes, and returns rows in
/// canonical owner order for `ProofBundle::crash_obligations`. Before the
/// roster is emitted each row is replayed through the verifier's own
/// discharge predicate: a question the produced supply cannot answer is a
/// lowering failure, not an artifact a receiver must reject.
pub fn produce_crash_obligation_evidence(
    module: &TerminalModule,
) -> Result<Vec<CrashObligationEvidence>, CrashRosterError> {
    use terminal_verifier::CrashObligationQuestion;
    let questions = terminal_verifier::reconstruct_crash_obligations(module)?;
    let roster: Vec<CrashObligationEvidence> = questions
        .iter()
        .map(|question| {
            let produce_with_requirements = |goal: &Proposition,
                                             semantic_axioms: &[Proposition],
                                             requirements: &[Proposition]|
             -> Vec<CrashCertificate> {
                question.context.as_ref().map_or_else(Vec::new, |context| {
                    produce_entry_requirement_certificates(
                        context,
                        goal,
                        requirements,
                        semantic_axioms,
                    )
                    .into_iter()
                    .map(EntryRequirementCertificate::into_certificate)
                    .collect()
                })
            };
            let produce = |goal: &Proposition, axioms: &[Proposition]| {
                produce_with_requirements(goal, axioms, &question.requirements)
            };
            let (coverage, refutation) = match &question.question {
                // One coverage roster per asserted guard, holding the
                // certificates produced under every reconstructed path's
                // axioms; one refutation roster per path for the path's own
                // infeasibility (`Falsehood`) discharge.
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
                    refutation_requirements,
                    paths,
                } => (
                    coverage_goals
                        .iter()
                        .map(|goal| {
                            paths
                                .iter()
                                .flat_map(|axioms| produce(goal, axioms))
                                .collect()
                        })
                        .collect(),
                    refutation_goal
                        .iter()
                        .map(|goal| {
                            paths
                                .iter()
                                .flat_map(|axioms| {
                                    produce_with_requirements(goal, axioms, refutation_requirements)
                                })
                                .collect()
                        })
                        .collect(),
                ),
            };
            CrashObligationEvidence {
                owner: question.owner,
                coverage,
                refutation,
            }
        })
        .collect();
    let undischarged: Vec<_> = questions
        .iter()
        .zip(&roster)
        .filter(|(question, row)| !terminal_verifier::crash_obligation_discharged(question, row))
        .map(|(question, _)| question.owner)
        .collect();
    if undischarged.is_empty() {
        Ok(roster)
    } else {
        Err(CrashRosterError::Undischarged(undischarged))
    }
}

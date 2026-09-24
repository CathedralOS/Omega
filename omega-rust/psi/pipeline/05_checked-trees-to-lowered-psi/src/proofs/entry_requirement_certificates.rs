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
            let produce =
                |goal: &Proposition, semantic_axioms: &[Proposition]| -> Vec<CrashCertificate> {
                    question.context.as_ref().map_or_else(Vec::new, |context| {
                        produce_entry_requirement_certificates(
                            context,
                            goal,
                            &question.requirements,
                            semantic_axioms,
                        )
                        .into_iter()
                        .map(EntryRequirementCertificate::into_certificate)
                        .collect()
                    })
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

#[cfg(test)]
mod tests {
    use super::{check_entry_requirement_certificate, produce_entry_requirement_certificates};
    use semantic_vocabulary::{
        IntegerSign, IntegerType, Proposition, PropositionContext, ScalarTerm, ScalarType, ValueId,
    };

    fn boolean_value(identity: u64) -> ScalarTerm {
        ScalarTerm::value(ValueId::new(identity).unwrap(), ScalarType::Boolean)
    }

    fn boolean_context(identities: &[u64]) -> PropositionContext {
        PropositionContext::from_value_types(
            identities
                .iter()
                .map(|identity| (ValueId::new(*identity).unwrap(), ScalarType::Boolean)),
        )
        .unwrap()
    }

    fn integer_type() -> IntegerType {
        IntegerType::new(IntegerSign::Signed, 32).unwrap()
    }

    fn integer_value(identity: u64) -> ScalarTerm {
        ScalarTerm::value(
            ValueId::new(identity).unwrap(),
            ScalarType::Integer(integer_type()),
        )
    }

    fn integer_context(identities: &[u64]) -> PropositionContext {
        PropositionContext::from_value_types(identities.iter().map(|identity| {
            (
                ValueId::new(*identity).unwrap(),
                ScalarType::Integer(integer_type()),
            )
        }))
        .unwrap()
    }

    fn integer_order(left: u64, right: u64, strict: bool) -> Proposition {
        if strict {
            Proposition::LessThan(integer_value(left), integer_value(right))
        } else {
            Proposition::LessOrEqual(integer_value(left), integer_value(right))
        }
    }

    #[test]
    fn produced_certificates_are_redecided_not_trusted() {
        let context = boolean_context(&[1]);
        let goal = Proposition::Equal(boolean_value(1), ScalarTerm::boolean(true));
        let requirements = [Proposition::Conjunction(vec![
            Proposition::Truth,
            goal.clone(),
        ])];
        let certificates =
            produce_entry_requirement_certificates(&context, &goal, &requirements, &[]);
        assert!(!certificates.is_empty());
        // Every supplied node is replayed through the recorded denotation
        // conversion and the kernel check; supply alone grants nothing.
        assert!(certificates.iter().all(|certificate| {
            check_entry_requirement_certificate(&context, &goal, &requirements, &[], certificate)
        }));
        // A node produced for this goal is not evidence for another question.
        let other = Proposition::Equal(boolean_value(1), ScalarTerm::boolean(false));
        assert!(!certificates.iter().any(|certificate| {
            check_entry_requirement_certificate(&context, &other, &requirements, &[], certificate)
        }));
    }

    #[test]
    fn an_empty_supply_establishes_nothing() {
        let context = boolean_context(&[1]);
        let goal = Proposition::Equal(boolean_value(1), ScalarTerm::boolean(true));
        assert!(produce_entry_requirement_certificates(&context, &goal, &[], &[]).is_empty());
    }

    #[test]
    fn integer_order_certificates_reach_through_inclusive_premises() {
        let context = integer_context(&[1, 2, 3]);
        // A strict conclusion composes through inclusive links, but the
        // kernel's strict transitivity requires at least one strict edge
        // (`classicality.md`). Every chain that has one reaches `1 < 3`.
        let strict = integer_order(1, 3, true);
        for (first, second) in [(true, false), (false, true), (true, true)] {
            let requirements = [integer_order(1, 2, first), integer_order(2, 3, second)];
            let certificates =
                produce_entry_requirement_certificates(&context, &strict, &requirements, &[]);
            assert!(certificates.iter().any(|certificate| {
                check_entry_requirement_certificate(
                    &context,
                    &strict,
                    &requirements,
                    &[],
                    certificate,
                )
            }));
            // The certificate's recorded lane stays part of the check: the
            // same node replayed under a mismatched lane conversion is not
            // accepted.
            assert!(
                certificates
                    .iter()
                    .all(|certificate| certificate.proof().conclusion == strict)
            );
        }
        // Two inclusive links compose to the inclusive conclusion and to
        // nothing stronger: `a <= b <= c` leaves `a = c` open. The producer
        // supplies the `<=` certificate and supplies nothing for `<`.
        let requirements = [integer_order(1, 2, false), integer_order(2, 3, false)];
        let inclusive = integer_order(1, 3, false);
        let certificates =
            produce_entry_requirement_certificates(&context, &inclusive, &requirements, &[]);
        assert!(certificates.iter().any(|certificate| {
            check_entry_requirement_certificate(
                &context,
                &inclusive,
                &requirements,
                &[],
                certificate,
            )
        }));
        assert!(
            produce_entry_requirement_certificates(&context, &strict, &requirements, &[])
                .is_empty()
        );
    }

    #[test]
    fn case_analysis_requirements_produce_a_checkable_supply() {
        let context = boolean_context(&[1]);
        let goal = Proposition::Equal(boolean_value(1), ScalarTerm::boolean(true));
        // A disjunctive requirement whose branches each carry the goal still
        // discharges it through disjunction elimination.
        let requirements = [Proposition::Disjunction(vec![
            Proposition::Conjunction(vec![Proposition::Truth, goal.clone()]),
            goal.clone(),
        ])];
        let certificates =
            produce_entry_requirement_certificates(&context, &goal, &requirements, &[]);
        assert!(certificates.iter().any(|certificate| {
            check_entry_requirement_certificate(&context, &goal, &requirements, &[], certificate)
        }));
    }
}

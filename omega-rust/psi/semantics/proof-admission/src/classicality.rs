//! Logical-foundation classification of the certificate proof rules.
//!
//! The classicality audit (`wiki/spec/proofs/classicality.md`)
//! classifies every rule an accepted certificate can cite against the
//! classical/constructive boundary the matching-logic exploration requires:
//! classical reasoning must never be silently imported into the accepted
//! foundation. The classification is enforced by exhaustive `match` — a new
//! `AcceptedProofRule` variant that is not classified here does not compile.

use crate::proof::AcceptedProofRule;

/// The logical foundation an accepted proof rule stands on.
///
/// This is a review classification, not a second checker: it names the
/// reasoning principle each rule is allowed to use, so a reader can audit
/// the boundary without re-deriving it from the checkers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ProofRuleFoundation {
    /// Intuitionistically valid inference: the conclusion is witnessed from
    /// checked premises with no appeal to excluded middle, double-negation
    /// elimination, or proof by contradiction.
    Constructive,
    /// Constructive only because the checked relation is decidable over its
    /// carrier — closed integer relations, carrier bounds, discreteness of
    /// fixed integers, bounded denotation and checked witness conversion.
    /// A rule in this class must not be generalized to an undecidable
    /// domain without reclassification; decision over an undecidable
    /// relation is a disguised excluded middle.
    ConstructiveDecidable,
    /// Not an inference rule: the node cites a verifier-reconstructed
    /// semantic axiom whose proposition is itself a trusted premise of the
    /// compilation, recorded on the acceptance for audit.
    TrustedAdmission,
    /// A rule resting on a classically-only principle — excluded middle,
    /// double-negation elimination, proof by contradiction, choice, or a
    /// decision procedure over an undecidable relation. Nothing maps here:
    /// the proposition language has no negation connective, so these forms
    /// are currently unexpressible, and any rule that could express one
    /// must be reclassified here first.
    Classical,
}

impl AcceptedProofRule {
    /// The foundation this rule's justification stands on.
    pub const fn foundation(self) -> ProofRuleFoundation {
        match self {
            Self::Primitive => ProofRuleFoundation::ConstructiveDecidable,
            Self::SemanticAxiom => ProofRuleFoundation::TrustedAdmission,
            Self::Assumption
            | Self::ConjunctionIntroduction
            | Self::ConjunctionElimination
            | Self::DisjunctionIntroduction
            | Self::DisjunctionElimination
            | Self::ImplicationIntroduction
            | Self::ImplicationElimination
            | Self::EqualityTransitivity
            | Self::EqualitySymmetry
            | Self::ValueEqualityTransport
            | Self::IntegerOrderWeakening
            | Self::IntegerLessOrEqualTransitivity
            | Self::IntegerStrictOrderTransitivity
            | Self::IntegerOrderSubstitution => ProofRuleFoundation::Constructive,
            Self::PredicateDenotation
            | Self::IntegerOrderDiscreteness
            | Self::IntegerSubtractOrder
            | Self::IntegerAffineBound
            | Self::IntegerExactAddDefinitionBound
            | Self::IntegerCastBound
            | Self::IntegerCorrelatedForbiddenRoots => ProofRuleFoundation::ConstructiveDecidable,
        }
    }

    /// Whether checking this rule records cited semantic axioms on the
    /// acceptance — the bound rules prove nothing by themselves; each
    /// accepted use re-commits the witness's cited axiom roster.
    pub const fn cites_semantic_axioms(self) -> bool {
        matches!(
            self,
            Self::SemanticAxiom
                | Self::IntegerAffineBound
                | Self::IntegerExactAddDefinitionBound
                | Self::IntegerCastBound
                | Self::IntegerCorrelatedForbiddenRoots
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every rule an accepted certificate records. Duplicating the variant
    /// list here is the audit's completeness check: the exhaustive `match`
    /// in `foundation` forces new rules into the classifier, and this list
    /// forces them into the test assertions.
    const ALL: [AcceptedProofRule; 23] = [
        AcceptedProofRule::Primitive,
        AcceptedProofRule::SemanticAxiom,
        AcceptedProofRule::Assumption,
        AcceptedProofRule::ConjunctionIntroduction,
        AcceptedProofRule::ConjunctionElimination,
        AcceptedProofRule::DisjunctionIntroduction,
        AcceptedProofRule::DisjunctionElimination,
        AcceptedProofRule::ImplicationIntroduction,
        AcceptedProofRule::ImplicationElimination,
        AcceptedProofRule::EqualityTransitivity,
        AcceptedProofRule::EqualitySymmetry,
        AcceptedProofRule::PredicateDenotation,
        AcceptedProofRule::ValueEqualityTransport,
        AcceptedProofRule::IntegerOrderWeakening,
        AcceptedProofRule::IntegerOrderDiscreteness,
        AcceptedProofRule::IntegerSubtractOrder,
        AcceptedProofRule::IntegerLessOrEqualTransitivity,
        AcceptedProofRule::IntegerStrictOrderTransitivity,
        AcceptedProofRule::IntegerOrderSubstitution,
        AcceptedProofRule::IntegerAffineBound,
        AcceptedProofRule::IntegerExactAddDefinitionBound,
        AcceptedProofRule::IntegerCastBound,
        AcceptedProofRule::IntegerCorrelatedForbiddenRoots,
    ];

    #[test]
    fn no_accepted_rule_stands_on_a_classical_foundation() {
        for rule in ALL {
            assert_ne!(
                rule.foundation(),
                ProofRuleFoundation::Classical,
                "{rule:?} must not silently rest on classical reasoning"
            );
        }
    }

    #[test]
    fn only_semantic_axiom_is_a_trusted_admission() {
        for rule in ALL {
            assert_eq!(
                rule.foundation() == ProofRuleFoundation::TrustedAdmission,
                matches!(rule, AcceptedProofRule::SemanticAxiom),
                "{rule:?} foundation drifted across the admission boundary"
            );
        }
    }

    #[test]
    fn exactly_the_bound_rules_and_axiom_citation_carry_axiom_rosters() {
        for rule in ALL {
            assert_eq!(
                rule.cites_semantic_axioms(),
                matches!(
                    rule,
                    AcceptedProofRule::SemanticAxiom
                        | AcceptedProofRule::IntegerAffineBound
                        | AcceptedProofRule::IntegerExactAddDefinitionBound
                        | AcceptedProofRule::IntegerCastBound
                        | AcceptedProofRule::IntegerCorrelatedForbiddenRoots
                ),
                "{rule:?} axiom-citation record changed"
            );
        }
    }
}

/*
    Constructive and classical foundation classification for admitted proof
    rules.

    We assign each `AcceptedProofRule` its `ProofRuleFoundation` so consumers
    can audit whether a certificate remains within the constructive core.
*/

use crate::proof::AcceptedProofRule;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ProofRuleFoundation {
    Constructive,
    ConstructiveDecidable,
    TrustedAdmission,
    Classical,
}

impl AcceptedProofRule {
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
            | Self::IntegerMathOrderTranslation
            | Self::IntegerOrderAntisymmetry
            | Self::IntegerLessOrEqualTransitivity
            | Self::IntegerStrictOrderTransitivity
            | Self::IntegerOrderSubstitution => ProofRuleFoundation::Constructive,
            Self::PredicateDenotation
            | Self::IntegerOrderDiscreteness
            | Self::IntegerSubtractOrder
            | Self::IntegerAddOrder
            | Self::IntegerSubtractAntitone
            | Self::IntegerAffineBound
            | Self::IntegerRemainderWithinDividend
            | Self::IntegerExactAddDefinitionBound
            | Self::IntegerExactSubtractDefinitionBound
            | Self::IntegerCastBound
            | Self::IntegerCorrelatedForbiddenRoots => ProofRuleFoundation::ConstructiveDecidable,
        }
    }

    /// Whether the kernel trusts this rule's arithmetic statement without
    /// the mathematical core deriving it (`wiki/spec/proofs/foundation.md`,
    /// "Admitting an arithmetic rule"). A provisional row was tested against
    /// computed arithmetic and is to be replaced by a checked proof; none may
    /// remain when the kernel is claimed verified. Only rows admitted under
    /// that decision are marked: `false` on an older row does not say the
    /// core derives it.
    pub const fn is_provisional(self) -> bool {
        matches!(self, Self::IntegerRemainderWithinDividend)
    }

    pub const fn cites_semantic_axioms(self) -> bool {
        matches!(
            self,
            Self::SemanticAxiom
                | Self::IntegerAffineBound
                | Self::IntegerRemainderWithinDividend
                | Self::IntegerExactAddDefinitionBound
                | Self::IntegerExactSubtractDefinitionBound
                | Self::IntegerCastBound
                | Self::IntegerCorrelatedForbiddenRoots
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{AcceptedProofRule, ProofRuleFoundation};

    const ALL: [AcceptedProofRule; 29] = [
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
        AcceptedProofRule::IntegerMathOrderTranslation,
        AcceptedProofRule::IntegerOrderAntisymmetry,
        AcceptedProofRule::IntegerSubtractOrder,
        AcceptedProofRule::IntegerAddOrder,
        AcceptedProofRule::IntegerSubtractAntitone,
        AcceptedProofRule::IntegerLessOrEqualTransitivity,
        AcceptedProofRule::IntegerStrictOrderTransitivity,
        AcceptedProofRule::IntegerOrderSubstitution,
        AcceptedProofRule::IntegerAffineBound,
        AcceptedProofRule::IntegerRemainderWithinDividend,
        AcceptedProofRule::IntegerExactAddDefinitionBound,
        AcceptedProofRule::IntegerExactSubtractDefinitionBound,
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
                        | AcceptedProofRule::IntegerRemainderWithinDividend
                        | AcceptedProofRule::IntegerExactAddDefinitionBound
                        | AcceptedProofRule::IntegerExactSubtractDefinitionBound
                        | AcceptedProofRule::IntegerCastBound
                        | AcceptedProofRule::IntegerCorrelatedForbiddenRoots
                ),
                "{rule:?} axiom-citation record changed"
            );
        }
    }

    #[test]
    fn only_rules_admitted_as_provisional_are_marked_so() {
        for rule in ALL {
            assert_eq!(
                rule.is_provisional(),
                matches!(rule, AcceptedProofRule::IntegerRemainderWithinDividend),
                "{rule:?} provisional marking changed"
            );
        }
    }
}

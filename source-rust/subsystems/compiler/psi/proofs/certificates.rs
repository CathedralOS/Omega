pub(crate) mod admission {
    use super::{EvidenceIdentity, PrimitiveJudgment, ProofNode};
    use semantic_vocabulary::{AdmissionSiteId, ProfileDecisionId};

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct ProofSystemMarker;

    impl ProofSystemMarker {
        pub const CURRENT: Self = Self;

        pub const fn new(raw: u16) -> Option<Self> {
            if raw == Self::CURRENT.get() {
                Some(Self::CURRENT)
            } else {
                None
            }
        }

        pub const fn get(self) -> u16 {
            10
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum AdmissionKind {
        ForeignBoundaryGuarantee,
        ProviderFact,
        CheckedAssemblyClaim,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct CertificateEnvelope<Proof = ProofNode> {
        pub identity: EvidenceIdentity,
        pub proof_system_marker: ProofSystemMarker,
        pub proof: Proof,
    }

    impl<Proof> CertificateEnvelope<Proof> {
        /// Verification reads the proof; the stored envelope keeps owning it.
        pub fn as_ref(&self) -> CertificateEnvelope<&Proof> {
            CertificateEnvelope {
                identity: self.identity,
                proof_system_marker: self.proof_system_marker,
                proof: &self.proof,
            }
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct AdmissionEvidence {
        pub site: AdmissionSiteId,
        pub kind: AdmissionKind,
        pub authority_identity: EvidenceIdentity,
        pub evidence_identity: EvidenceIdentity,
        pub profile_decision: ProfileDecisionId,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum EvidenceRoute<Proof = ProofNode> {
        KernelDerived(PrimitiveJudgment),
        CertificateDerived(CertificateEnvelope<Proof>),
        Admitted(AdmissionEvidence),
        SourceChecked,
    }

    impl<Proof> EvidenceRoute<Proof> {
        pub fn as_ref(&self) -> EvidenceRoute<&Proof> {
            match self {
                Self::KernelDerived(judgment) => EvidenceRoute::KernelDerived(*judgment),
                Self::CertificateDerived(certificate) => {
                    EvidenceRoute::CertificateDerived(certificate.as_ref())
                }
                Self::Admitted(evidence) => EvidenceRoute::Admitted(*evidence),
                Self::SourceChecked => EvidenceRoute::SourceChecked,
            }
        }
    }
}
pub(crate) mod nodes {
    use super::{
        IntegerAffineWitness, IntegerCastChainWitness, IntegerCorrelatedForbiddenRootWitness,
        Proposition,
    };
    use semantic_vocabulary::IntegerMathTerm;
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum PrimitiveJudgment {
        Truth,
        ReflexiveEquality,
        ClosedIntegerRelation,
        IntegerCarrierBound,
        ClosedIeeeFloatRelation,
        ClosedFloatIntegerRelation,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ProofNode {
        pub conclusion: Proposition,
        pub rule: ProofRule,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum ProofRule {
        /// Exact integer representability, including complete-carrier
        /// embedding. Provisional until derived in the mathematical core.
        IntegerFloatExact,
        /// `c in S::A || c in S::B || ...`: a sum input of a theorem is in
        /// one of its cases, each named once. The kernel derives it by
        /// induction over the sum's declaration, so it is checked only where
        /// that declaration is bound.
        StructuralCaseExhaustion,
        /// A checked exactness premise entails the converter's entry
        /// conditions. Alternatively, finite nonzero IEEE equality
        /// transports a separately checked computed goal.
        FloatIntegerConversion {
            premise: Box<ProofNode>,
            computed: Option<Box<ProofNode>>,
        },
        ValueEqualityTransport {
            premise: Box<ProofNode>,
            equalities: Vec<ProofNode>,
        },
        PredicateDenotation {
            premise: Box<ProofNode>,
        },
        Primitive(PrimitiveJudgment),
        SemanticAxiom {
            index: usize,
        },
        Assumption {
            index: usize,
        },
        ConjunctionIntroduction(Vec<ProofNode>),
        ConjunctionElimination {
            /// Each projection is a separate logical use of this proof.
            conjunction: std::sync::Arc<ProofNode>,
            conjunct: usize,
        },
        DisjunctionIntroduction {
            disjunct: Box<ProofNode>,
            index: usize,
        },
        DisjunctionElimination {
            disjunction: Box<ProofNode>,
            branches: Vec<ProofNode>,
        },
        ImplicationIntroduction {
            body: Box<ProofNode>,
        },
        ImplicationElimination {
            implication: Box<ProofNode>,
            premise: Box<ProofNode>,
        },
        EqualityTransitivity {
            left_equals_middle: Box<ProofNode>,
            middle_equals_right: Box<ProofNode>,
        },
        EqualitySymmetry {
            equality: Box<ProofNode>,
        },
        IntegerOrderWeakening {
            relation: Box<ProofNode>,
        },
        IntegerOrderDiscreteness {
            relation: Box<ProofNode>,
        },
        IntegerMathOrderTranslation {
            relation: Box<ProofNode>,
            addend: IntegerMathTerm,
        },
        IntegerOrderAntisymmetry {
            forward: Box<ProofNode>,
            backward: Box<ProofNode>,
        },
        IntegerSubtractOrder {
            difference: Box<ProofNode>,
            positive: Box<ProofNode>,
        },
        IntegerAddOrder {
            sum: Box<ProofNode>,
            positive: Box<ProofNode>,
        },
        IntegerSubtractAntitone {
            smaller: Box<ProofNode>,
            larger: Box<ProofNode>,
            order: Box<ProofNode>,
        },
        IntegerLessOrEqualTransitivity {
            left_less_or_equal_middle: Box<ProofNode>,
            middle_less_or_equal_right: Box<ProofNode>,
        },
        IntegerStrictOrderTransitivity {
            left_to_middle: Box<ProofNode>,
            middle_to_right: Box<ProofNode>,
        },
        IntegerOrderSubstitution {
            relation: Box<ProofNode>,
            equality: Box<ProofNode>,
            endpoint: usize,
        },
        IntegerAffineBound {
            root_bound: Box<ProofNode>,
            witness: IntegerAffineWitness,
        },
        IntegerExactAddDefinitionBound {
            left_bound: Box<ProofNode>,
            right_bound: Box<ProofNode>,
            definition_axiom: usize,
        },
        IntegerExactSubtractDefinitionBound {
            left_bound: Box<ProofNode>,
            right_bound: Box<ProofNode>,
            definition_axiom: usize,
        },
        IntegerCastBound {
            root_bound: Box<ProofNode>,
            witness: IntegerCastChainWitness,
        },
        IntegerCorrelatedForbiddenRoots {
            witness: IntegerCorrelatedForbiddenRootWitness,
        },
        /// `c <= k |- 2^c <= 2^k` for a nonnegative literal `c`: a
        /// parameter width's range endpoints are powers of two of the
        /// width, and its declared lower bound orders them.
        IntegerPowerOfTwoOrder {
            order: Box<ProofNode>,
        },
        /// Compose nonempty positive IEEE order premises. Ordered truth
        /// excludes NaN; unordered complements are not order premises.
        IeeeFloatOrderChain(Vec<ProofNode>),
        /// Normalize meaning equations from independently checked cited
        /// equations, closed catalog evaluation and exact widening reversal.
        FloatMeaningNormalization(Vec<ProofNode>),
    }
}
pub(crate) mod witnesses {
    use semantic_vocabulary::{Proposition, ScalarTerm};

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct IntegerAffineWitness {
        pub root: ScalarTerm,
        pub target: ScalarTerm,
        pub definition_axioms: Vec<usize>,
        pub literal_axioms: Vec<Option<usize>>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct IntegerCastChainWitness {
        pub root: ScalarTerm,
        pub target: ScalarTerm,
        pub definition_axioms: Vec<usize>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct CorrelatedAffineStepWitness {
        pub definition_axiom: usize,
        pub literal_axiom: Option<usize>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct CorrelatedAffineBranchWitness {
        pub root: ScalarTerm,
        pub target: ScalarTerm,
        pub steps: Vec<CorrelatedAffineStepWitness>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct IntegerCorrelatedForbiddenRootWitness {
        pub dividend: CorrelatedAffineBranchWitness,
        pub divisor: CorrelatedAffineBranchWitness,
        pub definition_axiom_count: usize,
        pub lower_bound_axiom: usize,
        pub upper_bound_axiom: usize,
        pub conclusion: Proposition,
    }
}
pub use admission::{
    AdmissionEvidence, AdmissionKind, CertificateEnvelope, EvidenceRoute, ProofSystemMarker,
};
pub use nodes::{PrimitiveJudgment, ProofNode, ProofRule};
use semantic_vocabulary::{EvidenceIdentity, Proposition};
pub use witnesses::{
    CorrelatedAffineBranchWitness, CorrelatedAffineStepWitness, IntegerAffineWitness,
    IntegerCastChainWitness, IntegerCorrelatedForbiddenRootWitness,
};

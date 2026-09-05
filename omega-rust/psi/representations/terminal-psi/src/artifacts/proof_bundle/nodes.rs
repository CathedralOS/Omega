use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrimitiveJudgment {
    Truth,
    ReflexiveEquality,
    ClosedIntegerRelation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProofNode {
    pub conclusion: Proposition,
    pub rule: ProofRule,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProofRule {
    Primitive(PrimitiveJudgment),
    /// Cite one verifier-reconstructed semantic axiom.
    SemanticAxiom {
        index: usize,
    },
    Assumption {
        index: usize,
    },
    ConjunctionIntroduction(Vec<ProofNode>),
    ConjunctionElimination {
        conjunction: Box<ProofNode>,
        conjunct: usize,
    },
    DisjunctionIntroduction {
        disjunct: Box<ProofNode>,
        index: usize,
    },
    /// Prove one common conclusion under each ordered disjunct, discharging
    /// the single assumption appended for that branch.
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
    IntegerLessOrEqualTransitivity {
        left_less_or_equal_middle: Box<ProofNode>,
        middle_less_or_equal_right: Box<ProofNode>,
    },
    IntegerLessOrEqualSubstitution {
        relation: Box<ProofNode>,
        equality: Box<ProofNode>,
        endpoint: usize,
    },
    /// Map one independently proved root bound through an exact, ordered
    /// endpoint-transform witness whose affine or landed-count shift
    /// semantic-axiom custody is rechecked.
    IntegerAffineBound {
        root_bound: Box<ProofNode>,
        witness: IntegerAffineWitness,
    },
    /// Map two independently proved scalar endpoints through one cited prior
    /// exact-add definition. This is distinct from an affine literal sibling.
    IntegerExactAddDefinitionBound {
        left_bound: Box<ProofNode>,
        right_bound: Box<ProofNode>,
        definition_axiom: usize,
    },
    /// Map one independently proved root bound through one checked ordered
    /// word of partial fixed-integer exact casts and strict widening identities.
    IntegerCastBound {
        root_bound: Box<ProofNode>,
        witness: IntegerCastChainWitness,
    },
    /// Prove canonical signed exact-division definedness by replaying two
    /// correlated affine branches over one machine signature parameter.
    IntegerCorrelatedForbiddenRoots {
        witness: IntegerCorrelatedForbiddenRootWitness,
    },
}

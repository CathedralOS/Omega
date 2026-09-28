use semantic_vocabulary::{Proposition, ScalarTerm};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntegerAffineWitness {
    pub root: ScalarTerm,
    pub target: ScalarTerm,
    pub definition_axioms: Vec<usize>,
    /// One optional, earlier equality landing the non-chain operand at each
    /// affine definition. The vector is position-aligned with
    /// `definition_axioms`; `None` means that definition embeds its literal.
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
    /// Exact prior equality that lands a non-closed right sibling.
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
    /// Separates prior operation definitions from retained signature facts.
    pub definition_axiom_count: usize,
    pub lower_bound_axiom: usize,
    pub upper_bound_axiom: usize,
    /// Exact reducer-facing sufficient proposition reconstructed by the check.
    pub conclusion: Proposition,
}

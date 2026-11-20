use crate::{ContractClause, DeclarationId, PredicateOperator, ScalarType, Site, TypeExpression};
use num_bigint::BigInt;

/// A checked term remains available for unfolding in proofs. It introduces no executable machine.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MathematicalDeclaration {
    pub declaration: DeclarationId,
    pub public: bool,
    pub site: Site,
    pub generic_bounds: Vec<TypeExpression>,
    pub parameter_count: usize,
    pub signature: TypeExpression,
    pub body: MathematicalBody,
    /// Exact boundary assumptions on which this declaration transitively depends.
    pub assumptions: Vec<DeclarationId>,
}

/// A resultless theorem whose machine-shaped hypotheses are logical evidence.
/// It has no executable identity: no machine ID, body, call or callback entry.
/// Its authored body is empty, so every `ensures` remains an obligation over
/// `requires` and the hypotheses alone; this row proves nothing by itself.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LogicalTheorem {
    pub declaration: DeclarationId,
    pub public: bool,
    pub site: Site,
    /// The ordered generic telescope.
    pub binders: Vec<TheoremBinder>,
    pub parameters: Vec<TypeExpression>,
    pub hypotheses: Vec<LogicalHypothesis>,
    pub requires: Vec<ContractClause>,
    pub ensures: Vec<ContractClause>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TheoremBinder {
    /// A `machine` binder supplied only by evidence for this hypothesis.
    Hypothesis(usize),
    /// A mathematical binder with its checked bound.
    Mathematical(TypeExpression),
}

/// A universal premise: for all parameters, premises imply conclusions.
/// Clauses name its parameters with `PredicateStep::HypothesisParameter`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LogicalHypothesis {
    pub site: Site,
    pub parameters: Vec<TypeExpression>,
    pub premises: Vec<ContractClause>,
    pub conclusions: Vec<ContractClause>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MathematicalBody {
    Definition(Vec<MathematicalStep>),
    BoundaryAssumption,
}

/// In an application the function precedes the supplied terms, preserving partial application.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MathematicalStep {
    Parameter(usize),
    /// A universally bound input in the named theorem hypothesis.
    HypothesisParameter {
        hypothesis: usize,
        index: usize,
    },
    GenericParameter {
        owner: DeclarationId,
        index: usize,
    },
    /// A stored field of the data value being established.
    SelfField(DeclarationId),
    Declaration(DeclarationId),
    Integer {
        value: BigInt,
        carrier: Option<ScalarType>,
    },
    Boolean(bool),
    Predicate(PredicateOperator),
    /// The preceding term forms a proposition at this sort. Its truth is still an obligation.
    FormProposition(TypeExpression),
    Apply {
        generic_arguments: Vec<TypeExpression>,
        argument_count: usize,
        assumptions: Vec<DeclarationId>,
    },
}

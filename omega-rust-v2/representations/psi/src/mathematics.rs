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

/// A citation of a `LogicalTheorem` from a machine body. It executes nothing,
/// reads no storage and yields no value; native lowering erases it.
///
/// The checker substitutes `subjects` for the theorem's formals and
/// `arguments` for its binders. Under that substitution it must establish each
/// `requirements` clause of the theorem (by position in
/// `LogicalTheorem::requires`) and show that each cited argument's complete
/// contract implies its hypothesis. Only then does the theorem's `ensures`
/// hold at the same substitution after this operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TheoremApplication {
    /// The cited `LogicalTheorem` of this package.
    pub theorem: DeclarationId,
    /// One argument per `LogicalTheorem::binders`, in order.
    pub arguments: Vec<TheoremArgument>,
    /// One value per `LogicalTheorem::parameters`, in order. Each is a proof
    /// subject: the citation creates no runtime demand for it.
    pub subjects: Vec<crate::ValueId>,
    pub requirements: Vec<crate::CallRequirement>,
}

/// Evidence supplied for one theorem binder at a citation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TheoremArgument {
    /// A checked machine cited for `TheoremBinder::Hypothesis`. It is
    /// evidence, not a call target: the checker owes the refinement of the
    /// hypothesis by this machine's whole contract at the citation's subjects.
    Citation(DeclarationId),
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
    /// Optional mathematical result of this premise, never a machine return slot.
    pub result: Option<TypeExpression>,
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
    /// The result subject in the indexed hypothesis's conclusion, not the theorem's result.
    HypothesisResult(usize),
    GenericParameter {
        owner: DeclarationId,
        index: usize,
    },
    /// A stored field of the data value being established.
    SelfField(DeclarationId),
    Declaration(DeclarationId),
    /// A proof dependency, not a call or an admitted assumption. Under the
    /// expected binders the checker must establish the selected contract's
    /// premises and derive the expected conclusion from its guarantees.
    RefineCitation {
        machine: DeclarationId,
        expected: TypeExpression,
    },
    Integer {
        value: BigInt,
        carrier: Option<ScalarType>,
    },
    Boolean(bool),
    Predicate(PredicateOperator),
    /// Witness-hidden existence over this type. Forms a term, not evidence of inhabitation.
    Squash(TypeExpression),
    /// The preceding term forms a proposition at this sort. Its truth is still an obligation.
    FormProposition(TypeExpression),
    Apply {
        generic_arguments: Vec<TypeExpression>,
        argument_count: usize,
        assumptions: Vec<DeclarationId>,
    },
}

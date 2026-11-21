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
/// Every `ensures` remains an obligation over `requires`, the hypotheses and
/// the authored `proof` steps; this row proves nothing by itself.
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
    /// The body's proof steps in authored order; empty for an empty body.
    /// The checker reads them in order, under the theorem's `requires` and
    /// hypotheses. An empty or incomplete list is not a success assertion:
    /// checked automation must supply any missing term or the theorem fails.
    pub proof: Vec<ProofStep>,
}

/// One authored step of a theorem body. A step executes nothing, reads no
/// storage and demands no value: its subjects are erased.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProofStep {
    /// `let name = subject;` names an erased subject for later steps, which
    /// refer to it as `ProofInput::Local` with this step's position in
    /// `LogicalTheorem::proof`. The binding proves nothing.
    Bind { subject: ProofSubject<ProofInput>, site: Site },
    /// A statement that cites a checked local machine as proof. Under the
    /// substitution of `subjects` for its formals, the checker must establish
    /// each indexed `requires` clause of `machine` and its total denotation by
    /// `progress`. Only then do that machine's `ensures` clauses hold at the
    /// same subjects for the following steps and the goal. The cited machine
    /// is not called and its body is not entered.
    CiteMachine {
        machine: DeclarationId,
        subjects: Vec<ProofSubject<ProofInput>>,
        requirements: std::ops::Range<usize>,
        progress: crate::PredicateProgress,
        site: Site,
    },
}

/// A variable that a theorem proof step may name.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProofInput {
    /// The theorem formal at this position.
    Formal(usize),
    /// The subject bound by the `ProofStep::Bind` at this earlier position in
    /// `LogicalTheorem::proof`.
    Local(usize),
}

/// An erased proof subject. `Input` names a variable of the enclosing scope:
/// a caller's value at an executable citation, a `ProofInput` in a theorem
/// body. `Literal` is a closed scalar the source fixed at compile time (a
/// literal, a folded exact expression or a frozen constant) and already fits
/// the formal's carrier. No operation produces it and substitution never
/// changes it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProofSubject<Input> {
    Input(Input),
    Literal(crate::ScalarLiteral),
}

impl<Input: Copy> ProofSubject<Input> {
    pub fn input(&self) -> Option<Input> {
        match self {
            Self::Input(input) => Some(*input),
            Self::Literal(_) => None,
        }
    }
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
    /// One subject per `LogicalTheorem::parameters`, in order: a caller value
    /// or a closed literal. Each is erased: the citation creates no runtime
    /// demand for a value, and no operation computes a literal.
    pub subjects: Vec<ProofSubject<crate::ValueId>>,
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

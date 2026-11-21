use crate::{DeclarationId, EdgeId, OperationId, Site};
use num_bigint::BigInt;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallSite {
    Operation(OperationId),
    Edge(EdgeId),
}

/// The producing call and the zero-based position of a requirement in its
/// selected callee. Both coordinates are needed when two predicates are equal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CallRequirement {
    pub site: CallSite,
    pub clause: usize,
}

/// One published crash alternative of a callable interface, in authored order.
/// The guard is a predicate over the invocation's entry formals; `None` is the
/// unconditional route (`Truth`). Each call of the interface owes coverage of
/// every route, identified by its position here, under that call's own ordered
/// arguments. A route permits its crash; it does not execute one.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CrashRoute {
    pub cause: omega_program_contracts::CrashCause,
    pub guard: Option<ContractClause>,
    /// The authored `crashes` clause.
    pub site: Site,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContractClause {
    /// Operands precede their operation in left-to-right order. The final
    /// item produces the Boolean proposition.
    pub predicate: Vec<PredicateStep>,
    pub site: Site,
}

/// Progress input for one predicate application, not a termination judgment.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PredicateProgress {
    /// Establish the selected contract's termination promise with its premises.
    CheckPromise,
    /// Establish progress from the selected private machine's body. It may be cyclic.
    CheckPrivateBody,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PredicateStep {
    Formal(usize),
    /// Apply the selected local machine in this predicate, not in the executable
    /// body. The preceding operand terms supply the formals in declaration order.
    /// Before using the result, establish each indexed requirement of `callee`
    /// at this occurrence under that substitution (never assume the enclosing goal).
    MachineApplication {
        callee: DeclarationId,
        operands: Vec<crate::ScalarType>,
        result: crate::ScalarType,
        requirements: std::ops::Range<usize>,
        /// The non-specialized `callee` identifies one body and contract in this package.
        /// The checker owes total denotation (including no crash) under the actuals;
        /// neither an empty ceiling nor availability of the body establishes progress.
        progress: PredicateProgress,
        site: Site,
    },
    /// Apply this binder's selected guarantee to the preceding arguments and result.
    CallbackGuarantee {
        binder: usize,
        operand_count: usize,
    },
    TypesEqual {
        left: crate::TypeExpression,
        right: crate::TypeExpression,
    },
    /// Index in the owning requirement's value-index parameter list. At a member
    /// call the same position of `MemberCallSelection::static_arguments` supplies it.
    ValueIndex(usize),
    Result,
    /// The value whose named or default domain predicate is being checked.
    SelfValue,
    /// Selects one declared record field of the preceding structural subject.
    Field(DeclarationId),
    /// Selects one payload field of the named case of the preceding sum subject.
    /// The case is only a path segment: it neither tests nor assumes that the
    /// subject currently has this case. Any guard remains a separate `IsCase`.
    CasePayload {
        case: DeclarationId,
        field: DeclarationId,
    },
    /// Selects the element at this exact in-bounds position of the preceding
    /// owned fixed-array subject. Runtime selectors never form this segment.
    FixedIndex(u64),
    /// Tests the case discriminator of the preceding structural subject.
    IsCase(DeclarationId),
    FixedArrayLength,
    ByteSequenceLength,
    /// The live element count of the preceding shared element-view subject,
    /// never a byte count, fixed extent or a sibling view's count.
    ElementViewLength,
    Boolean(bool),
    Integer(BigInt),
    /// An erased mathematical term in this clause, including any application admission obligations.
    /// In a `LogicalTheorem` clause a sole term ending in `FormProposition` is a strict fact,
    /// not a Boolean; its truth remains an obligation or assumption.
    MathematicalTerm(Vec<crate::MathematicalStep>),
    /// A parameter of the enclosing logical hypothesis, by position in
    /// `LogicalTheorem::hypotheses` and in that hypothesis's parameter list.
    HypothesisParameter {
        hypothesis: usize,
        index: usize,
    },
    /// The conclusions of this hypothesis at the preceding operands: parameters
    /// in order, followed by the result subject if present. Proof-only: it
    /// never denotes a call, callback entry or runtime result.
    HypothesisGuarantee {
        hypothesis: usize,
        operand_count: usize,
    },
    Apply(PredicateOperator),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PredicateOperator {
    Not,
    And,
    Or,
    Equal,
    NotEqual,
    LessThan,
    LessOrEqual,
    GreaterThan,
    GreaterOrEqual,
}

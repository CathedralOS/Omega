use crate::{DeclarationId, EdgeId, OperationId, Site};
use num_bigint::BigInt;
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CallSite {
    Operation(OperationId),
    Edge(EdgeId),
    /// A selected nominal cleanup runs as an invocation of its own.
    Cleanup(CleanupSite),
}

/// One `CleanupAction::Invoke`, named as Terminal Psi names it: the list that
/// carries it and its zero-based position in that list. The invocation's only
/// actual is the action's cleaned place, moved into the target's one formal.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct CleanupSite {
    pub list: CleanupList,
    pub action: usize,
}

/// Where a cleanup-action list lives: on a return, successor or terminal-call
/// edge, or on a `StructuralStore` whose disposition disposes the old value.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CleanupList {
    Edge(EdgeId),
    Store(OperationId),
}

/// A call in a closed instance that invokes the static callable binder at
/// `binder` in `LoweredMachine::arguments`. The callee is that selection's
/// machine. The generic body owes the binder's preconditions under the call's
/// arguments; the selection's refinement, judged by the checker, is what makes
/// the selected machine's own premises available.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CallbackInvocation {
    pub call: CallSite,
    pub binder: usize,
}

/// The producing call and the zero-based position of a requirement in its
/// selected callee. Both coordinates are needed when two predicates are equal.
/// A call listed in `MachineBody::callback_invocations` invokes a closed static
/// binder: its positions refer to that binder's `StaticCallableSelection::requires`,
/// not to the selected machine's own contract.
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
    /// item states the clause: a Boolean predicate or, in a logical theorem,
    /// a strict proposition. Neither is a verdict.
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
    /// Selects the element at this exact frozen position of the preceding
    /// owned fixed-array subject. Lowering keeps a position past the declared
    /// extent; the checker compares it with the subject's type and rejects it.
    /// Runtime selectors never form this segment.
    FixedIndex(u64),
    /// A by-copy core read, not a static path segment. Consume the preceding
    /// owned fixed-array term and u64 selector term, in that order. The checker
    /// instantiates the selected primitive's entire contract at this occurrence
    /// with those actuals (including the receiver's declared extent); the
    /// enclosing predicate cannot serve as evidence for its own formation.
    FixedArrayRead {
        callee: DeclarationId,
        site: Site,
    },
    /// Tests the case discriminator of the preceding structural subject.
    IsCase(DeclarationId),
    /// Logical membership of the preceding subject in this domain application,
    /// including the requirements of its ancestors; not evidence that it holds.
    /// A nominal domain such as `Audio::Audible` has no arguments. Like
    /// `requires x in D`, it states a fact and leaves the subject's declared
    /// type unchanged. It supplies no routed provenance; a routed domain still
    /// needs it.
    InDomain(crate::DomainAtom),
    FixedArrayLength,
    ByteSequenceLength,
    /// The live element count of the preceding element-view subject,
    /// never a byte count, fixed extent or a sibling view's count.
    ElementViewLength,
    Boolean(bool),
    Integer(BigInt),
    /// The exact bits of a float literal or anonymous number, landed once in
    /// its format by preparation. It is never rounded again.
    IeeeFloat(crate::IeeeFloatBits),
    /// The IEEE comparison of the two preceding terms, both of `format`, as a
    /// Boolean. Ordered kinds and `Equal` are false when either operand is NaN,
    /// `NotEqual` is true then, and signed zeros compare equal. It asserts no
    /// mathematical or `FloatMeaning` equality and no integer order law.
    IeeeComparison {
        kind: IeeeComparisonKind,
        format: crate::IeeeFloatFormat,
    },
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

/// The canonical IEEE comparison kinds a proposition asserts. There is no
/// greater-than kind: source `a > b` is `LessThan` of `b` then `a`, and
/// `a >= b` is `LessOrEqual` of `b` then `a`, which agree on every input.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IeeeComparisonKind {
    Equal,
    NotEqual,
    LessThan,
    LessOrEqual,
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

use psi_arena::{Handle, HandleSpan};
use psi_symbols::SymbolHandle;
use psi_typed_trees::domain::ProofFact;
use psi_typed_trees::expression::ExpressionHandle;
use psi_typed_trees::name::Identifier;
use psi_typed_trees::types::{TypeConstraintNode, TypeReferenceHandle};

pub type FactHandle = Handle<Fact>;
pub type FactRefHandle = Handle<FactRef>;
pub type FactContextHandle = Handle<FactContext>;
pub type PlaceHandle = Handle<Place>;
pub type PlaceSegmentHandle = Handle<PlaceSegment>;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PlaceRoot {
    #[default]
    Unknown,
    Symbol(SymbolHandle),
    Expression(ExpressionHandle),
    TypeReference(TypeReferenceHandle),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaceSegment {
    Field {
        symbol: SymbolHandle,
    },
    /// Compiler-normalized identity for one statically selected sum case.
    /// Payload fields follow this segment, so otherwise identical field
    /// spellings in distinct variants cannot alias.
    Case {
        variant: SymbolHandle,
    },
    /// Compiler-normalized identity for one statically known fixed-array
    /// element. Unlike `Index`, this is independent of expression handles and
    /// can therefore appear in a type-derived ownership frontier.
    FixedIndex {
        index: usize,
    },
    /// One compiler-normalized half-open window selected from a collection.
    /// The bounds are element ordinals, not byte offsets; `start == end`
    /// denotes the empty window. Keeping the window structural lets mutation,
    /// loan-overlap, and caller-frame reasoning preserve untouched siblings
    /// without depending on expression-handle identity.
    FixedRange {
        start: usize,
        end: usize,
    },
    /// A runtime or otherwise non-normalized index expression. Ownership
    /// decomposition treats this conservatively as potentially selecting any
    /// element.
    Index {
        expression: ExpressionHandle,
    },
}

impl Default for PlaceSegment {
    fn default() -> Self {
        Self::Field {
            symbol: SymbolHandle::invalid(),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Place {
    pub root: PlaceRoot,
    pub segments: HandleSpan<PlaceSegment>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FactPlace {
    #[default]
    Unknown,
    Place(PlaceHandle),
    Symbol(SymbolHandle),
    Expression(ExpressionHandle),
    TypeReference(TypeReferenceHandle),
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ProgramPoint {
    #[default]
    Global,
    Definition {
        symbol: SymbolHandle,
    },
    Machine {
        machine_symbol: SymbolHandle,
    },
    State {
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
    },
    Statement {
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        statement_index: usize,
    },
    Call {
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        statement_index: usize,
        call_ordinal: usize,
    },
    CallRequires {
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        statement_index: usize,
        call_ordinal: usize,
    },
    CallEnsures {
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        statement_index: usize,
        call_ordinal: usize,
    },
    Exit {
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        statement_index: usize,
    },
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FactOrigin {
    #[default]
    Unknown,
    DomainDefinition {
        domain_symbol: SymbolHandle,
    },
    InvariantDefinition {
        invariant_symbol: SymbolHandle,
    },
    TypeReference,
    ProofObligation,
    MachineContract {
        machine_symbol: SymbolHandle,
    },
    StateContract {
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
    },
    /// A declared encoding-domain refinement on a machine-attached-data field
    /// (`out: &[u8] in Utf8`), surfaced as an always-holding entry fact for the
    /// machine (#66 read-narrowing). NOT a contract: it imposes no caller
    /// obligation -- write-enforcement guarantees the invariant.
    MachineFieldDomain {
        machine_symbol: SymbolHandle,
    },
    /// A declared domain qualification on a state PARAMETER, surfaced as an
    /// entry assumption for the machine (#66/P1a). Sound: the param's implicit
    /// `requires param in Domain` is a CALLER obligation, so predicate proof or
    /// bodyless establishment evidence already exists at entry.
    StateParameterDomain {
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
    },
    /// The declared encoding domain on a sum-CASE PAYLOAD field, surfaced for a
    /// local constructed as that case (`let cmd = Command::Say { text: "ok" }`):
    /// construction enforcement (#60-1c) proved the payload in-domain, so any
    /// later read of `cmd.<payload>` (e.g. a destructured `Command::Say { text }`
    /// forwarded as a call argument) carries the domain. Invalidation-aware via
    /// the flow (a reassignment of `cmd` drops it).
    LocalCasePayloadDomain {
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
    },
    StateSignatureContract {
        owner_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
    },
    CallRequires,
    CallEnsures,
    ExitEnsures,
    OperatorRequires {
        operator_symbol: SymbolHandle,
    },
    OperatorEnsures {
        operator_symbol: SymbolHandle,
    },
    OperatorBoundary {
        operator_symbol: SymbolHandle,
    },
    StatementTransfer,
}

/// Establishment evidence carried beside a qualification fact.
///
/// `source_symbol` names the checked machine, boundary requirement, operator,
/// or declaration that supplied the evidence when one exists.
/// `requirement_symbol` names the exact boundary state signature for admitted
/// qualification evidence; it is invalid for non-admitted evidence.
/// `receipt_identity == 0` means no admitted receipt was retained for this
/// compilation; admitted provider selection fills the normalized receipt
/// identity after the checked program retains its selected provider plans.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct QualificationEvidence {
    pub origin: psi_language_semantics::QualificationEvidenceOrigin,
    pub source_symbol: SymbolHandle,
    pub requirement_symbol: SymbolHandle,
    pub receipt_identity: u64,
}

impl QualificationEvidence {
    pub const fn from_origin(
        origin: psi_language_semantics::QualificationEvidenceOrigin,
        source_symbol: SymbolHandle,
    ) -> Self {
        Self {
            origin,
            source_symbol,
            requirement_symbol: SymbolHandle::invalid(),
            receipt_identity: 0,
        }
    }

    pub const fn from_admitted_requirement(
        source_symbol: SymbolHandle,
        requirement_symbol: SymbolHandle,
    ) -> Self {
        Self {
            origin: psi_language_semantics::QualificationEvidenceOrigin::AdmittedReceipt,
            source_symbol,
            requirement_symbol,
            receipt_identity: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ContractFactKind {
    #[default]
    Requires,
    Ensures,
    Boundary,
}

/// A contract expression after positional call/operator substitution. Typed
/// expression handles remain owned by the immutable checked program, so flow
/// elaboration records the canonical caller-term label in this fact-owned
/// arena instead of pretending the callee expression handle changed meaning.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InstantiatedExpression {
    pub label: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ProofObligationKind {
    #[default]
    BoundedAssignment,
    BoundedCallArgument,
    BoundedInitializer,
    BoundedStateReturn,
    BoundedValue,
    BoundedTransitionArgument,
    GuardedTransition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FactPayload {
    BooleanExpression(ExpressionHandle),
    DomainMembership {
        value: ExpressionHandle,
        domain: HandleSpan<Identifier>,
        domain_symbol: SymbolHandle,
    },
    PropositionApplication {
        fact: Handle<ProofFact>,
        proposition: SymbolHandle,
    },
    CarryPermission {
        value: ExpressionHandle,
        permission: psi_language_semantics::CarryPermission,
    },
    /// An undischarged resource provenance with a born-strict carry policy.
    /// This is independent of the current qualification fact set so
    /// qualification weakening cannot silently recover structural mobility.
    CarryOrigin {
        value: ExpressionHandle,
    },
    TypeConstraint {
        constraint: Handle<TypeConstraintNode>,
    },
    ProofObligation {
        kind: ProofObligationKind,
    },
    Contract {
        kind: ContractFactKind,
        fact: Handle<ProofFact>,
    },
    ContractBooleanExpression {
        kind: ContractFactKind,
        fact: Handle<ProofFact>,
        expression: ExpressionHandle,
        /// Invalid for declaration-shaped facts. Valid when a flow pass has
        /// substituted formal parameters onto concrete caller operands.
        instantiated: Handle<InstantiatedExpression>,
    },
    ContractDomainMembership {
        kind: ContractFactKind,
        fact: Handle<ProofFact>,
        value: ExpressionHandle,
        domain: HandleSpan<Identifier>,
        domain_symbol: SymbolHandle,
    },
    ContractPropositionApplication {
        kind: ContractFactKind,
        fact: Handle<ProofFact>,
        proposition: SymbolHandle,
        /// Canonical caller-term identity after call/operator substitution.
        /// Invalid on declaration-shaped facts.
        instantiated: Handle<InstantiatedExpression>,
    },
    ContractCarryPermission {
        kind: ContractFactKind,
        fact: Handle<ProofFact>,
        value: ExpressionHandle,
        permission: psi_language_semantics::CarryPermission,
    },
    InvariantDefinition {
        constraint_count: usize,
    },
}

impl Default for FactPayload {
    fn default() -> Self {
        Self::BooleanExpression(ExpressionHandle::invalid())
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Fact {
    pub place: FactPlace,
    pub point: ProgramPoint,
    pub origin: FactOrigin,
    pub evidence: QualificationEvidence,
    pub payload: FactPayload,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FactRef {
    pub fact: FactHandle,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FactContext {
    pub point: ProgramPoint,
    pub facts: HandleSpan<FactRef>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SymbolFactSet {
    pub symbol: SymbolHandle,
    pub facts: HandleSpan<FactRef>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BooleanFact {
    pub expression: ExpressionHandle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DomainMembershipFact {
    pub value: ExpressionHandle,
    pub domain: HandleSpan<Identifier>,
    pub domain_symbol: SymbolHandle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TypeConstraintFact {
    pub constraint: Handle<TypeConstraintNode>,
}

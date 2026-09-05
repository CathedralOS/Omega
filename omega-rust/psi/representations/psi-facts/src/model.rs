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
pub type QualificationCorrespondenceHandle = Handle<QualificationCorrespondence>;

/// Exact checked ownership retained for one fact authored by a domain
/// definition. The ordinary semantic fact remains the flow-facing row; this
/// record binds that row back to its typed fact and retains every structural
/// expression place used to interpret the public predicate.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DomainDefinitionFactRecord {
    pub domain_symbol: SymbolHandle,
    pub fact: Handle<ProofFact>,
    pub semantic_fact: FactHandle,
    pub dependencies: Vec<DomainDefinitionFactDependency>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DomainDefinitionFactDependency {
    pub expression: ExpressionHandle,
    pub place: PlaceHandle,
}

/// Exact checked ownership retained for one invariant authored by a data
/// definition. The semantic fact remains the flow-facing row; this record
/// binds it to the exact data declaration and typed proof fact while retaining
/// every structural field place needed to interpret the invariant.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DataDefinitionFactRecord {
    pub data_symbol: SymbolHandle,
    pub fact: Handle<ProofFact>,
    pub semantic_fact: FactHandle,
    pub dependencies: Vec<DataDefinitionFactDependency>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DataDefinitionFactDependency {
    pub expression: ExpressionHandle,
    pub place: PlaceHandle,
}

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
    TransitionArm {
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        statement_index: usize,
        /// Invalid identifies the guard-false fallthrough to the next arm.
        transition_target: psi_typed_trees::statement::TransitionTargetHandle,
    },
    Exit {
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        statement_index: usize,
        transition_target: psi_typed_trees::statement::TransitionTargetHandle,
    },
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FactOrigin {
    #[default]
    Unknown,
    DomainDefinition {
        domain_symbol: SymbolHandle,
    },
    DataDefinition {
        data_symbol: SymbolHandle,
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
    TransitionGuard,
    ExitEnsures,
    OperatorRequires {
        operator_symbol: SymbolHandle,
    },
    OperatorEnsures {
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
    /// Exact immutable value installed at `Fact.place` by a checked statement.
    /// The statement coordinate lives on the enclosing fact. Only literals
    /// are retained; this is not a deferred read of the source expression.
    AssignedValue {
        value: ExpressionHandle,
    },
    /// A per-byte character class proved to hold for EVERY byte currently
    /// stored at `Fact.place`. This is whole-carrier evidence with no declared
    /// domain behind it: an indexed write retires the exact `AssignedValue`
    /// snapshot, but a per-byte class survives replacing one byte by another
    /// byte of the same class, which is what keeps a text carrier provable
    /// across `buffer[i] = byte`. Mutation invalidation owns its lifetime
    /// exactly as it owns `AssignedValue`; it is matched by place overlap, so
    /// any write reaching the carrier retires it.
    BytePredicate {
        predicate: psi_language_semantics::byte_predicates::ByteSequencePredicate,
    },
    BooleanExpression(ExpressionHandle),
    /// Storage lifetime metadata for another fact in the same context. This
    /// row asserts no proposition about its place; a write there retires the
    /// dependent context through the ordinary mutation filter.
    StorageDependency {
        dependent: FactHandle,
    },
    /// A branch-local truth value, invalidated when any expression input changes.
    BooleanValue {
        expression: ExpressionHandle,
        value: bool,
    },
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
}

impl Default for FactPayload {
    fn default() -> Self {
        Self::BooleanExpression(ExpressionHandle::invalid())
    }
}

/// Closed identity of a qualification payload conserved by a checked
/// statement transfer. Expression operands and contract wrappers identify the
/// source occurrence, not the carried qualification, so they do not enter
/// correspondence identity.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum QualificationPayloadIdentity {
    DomainMembership {
        domain: HandleSpan<Identifier>,
        domain_symbol: SymbolHandle,
    },
    CarryPermission {
        permission: psi_language_semantics::CarryPermission,
    },
    #[default]
    CarryOrigin,
}

impl QualificationPayloadIdentity {
    pub const fn from_fact_payload(payload: FactPayload) -> Option<Self> {
        match payload {
            FactPayload::DomainMembership {
                domain,
                domain_symbol,
                ..
            }
            | FactPayload::ContractDomainMembership {
                domain,
                domain_symbol,
                ..
            } => Some(Self::DomainMembership {
                domain,
                domain_symbol,
            }),
            FactPayload::CarryPermission { permission, .. }
            | FactPayload::ContractCarryPermission { permission, .. } => {
                Some(Self::CarryPermission { permission })
            }
            FactPayload::CarryOrigin { .. } => Some(Self::CarryOrigin),
            FactPayload::AssignedValue { .. }
            | FactPayload::StorageDependency { .. }
            | FactPayload::BytePredicate { .. }
            | FactPayload::BooleanValue { .. }
            | FactPayload::BooleanExpression(_)
            | FactPayload::PropositionApplication { .. }
            | FactPayload::TypeConstraint { .. }
            | FactPayload::ProofObligation { .. }
            | FactPayload::Contract { .. }
            | FactPayload::ContractBooleanExpression { .. }
            | FactPayload::ContractPropositionApplication { .. } => None,
        }
    }
}

/// Checked-only proof ledger row for one qualification-preserving statement
/// transfer. This is separate from ordinary flow facts and grants no
/// qualification by itself.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct QualificationCorrespondence {
    pub source_fact: FactHandle,
    pub destination_fact: FactHandle,
    /// The exact contextual source occurrence selected by the statement. It
    /// is retained separately from `source_place`, which is the source fact's
    /// own place, so replay can prove their structural correspondence.
    pub source_occurrence_place: PlaceHandle,
    pub source_place: PlaceHandle,
    pub destination_place: PlaceHandle,
    pub formation: ProgramPoint,
    pub payload: QualificationPayloadIdentity,
    pub evidence: QualificationEvidence,
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

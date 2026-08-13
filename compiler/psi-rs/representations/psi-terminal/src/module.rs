use psi_core::{
    BlockId, BoundaryMachineId, ClaimId, ContentAlgebra, ContentConservation,
    ContentProjectionIdentity, ContentStructuralPlace, ContentTerm, ContractId, EdgeId,
    IntegerValue, MachineId, ObligationId, OperationId, PlaceId, Proposition, ScalarType,
    ServiceId, StructuralDomainId, StructuralFieldId, StructuralPlaceKind, StructuralTypeId,
    ValueId,
};
use psi_language_core::BindingRelevance;

/// Marker for the single unstable terminal-Psi semantic vocabulary.
///
/// Omega and Psi are pre-release. The compiler accepts only the vocabulary it
/// was built with; historical terminal artifacts are not compatibility inputs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VocabularyMarker;

impl VocabularyMarker {
    pub const CURRENT: Self = Self;

    pub const fn new(raw: u16) -> Option<Self> {
        if raw == Self::CURRENT.get() {
            Some(Self::CURRENT)
        } else {
            None
        }
    }

    pub const fn get(self) -> u16 {
        9
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ValueDeclaration {
    pub id: ValueId,
    pub scalar_type: ScalarType,
}

/// The normal result shape of one terminal machine.
///
/// Unit is the absence of a runtime value. It therefore has no `ValueId`, no
/// scalar type, and no result pseudo-value that contracts can name.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TerminalMachineResult {
    Unit,
    Scalar(ValueDeclaration),
    Structural(StructuralResultDeclaration),
}

impl TerminalMachineResult {
    pub const fn scalar(&self) -> Option<ValueDeclaration> {
        match self {
            Self::Scalar(result) => Some(*result),
            Self::Unit | Self::Structural(_) => None,
        }
    }

    pub const fn scalar_ref(&self) -> Option<&ValueDeclaration> {
        match self {
            Self::Scalar(result) => Some(result),
            Self::Unit | Self::Structural(_) => None,
        }
    }

    pub fn scalar_mut(&mut self) -> Option<&mut ValueDeclaration> {
        match self {
            Self::Scalar(result) => Some(result),
            Self::Unit | Self::Structural(_) => None,
        }
    }

    pub const fn structural(&self) -> Option<&StructuralResultDeclaration> {
        match self {
            Self::Structural(result) => Some(result),
            Self::Unit | Self::Scalar(_) => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalModule {
    pub vocabulary_marker: VocabularyMarker,
    pub entry: MachineId,
    /// Concrete target-neutral instantiated type shapes, ordered by `id`.
    /// Native layout is deliberately absent and is selected by Omega.
    pub structural_types: Vec<StructuralTypeDeclaration>,
    /// Structural qualification domains, ordered by `id`.
    pub structural_domains: Vec<StructuralDomainDeclaration>,
    /// Boundary-service declarations and their normalized parent closure.
    pub services: Vec<ServiceDeclaration>,
    /// Bodyless target-neutral Unit machines callable from terminal Psi.
    pub boundary_machines: Vec<BoundaryMachineDeclaration>,
    /// Nominal proof-formula vocabulary, strictly ordered by `id`.
    /// Transparent aliases never receive a declaration row.
    pub proposition_declarations: Vec<PropositionDeclaration>,
    /// Normalized applications retained without frontend arena handles.
    pub proposition_applications: Vec<PropositionApplicationIdentity>,
    pub machines: Vec<TerminalMachine>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StructuralTypeDeclaration {
    pub id: StructuralTypeId,
    pub identity: String,
    pub shape: StructuralTypeShape,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StructuralTypeShape {
    Record {
        /// Declaration order is semantic. Field IDs must nevertheless be
        /// strictly increasing so the same record has one canonical spelling.
        fields: Vec<StructuralFieldDeclaration>,
    },
    FixedArray {
        element: StructuralTypeId,
        length: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StructuralFieldDeclaration {
    pub id: StructuralFieldId,
    pub identity: String,
    /// Authored semantic relevance. Erased rows remain in terminal identity and
    /// proof structure even though Omega omits them from native layout.
    pub relevance: BindingRelevance,
    pub field_type: StructuralFieldType,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StructuralFieldType {
    Scalar(ScalarType),
    Structural(StructuralTypeId),
    /// Exact semantic type identity for an erased field whose carrier need not
    /// belong to the executable structural/layout vocabulary.
    Erased {
        type_identity: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StructuralDomainDeclaration {
    pub id: StructuralDomainId,
    pub identity: String,
    /// Exact carrier accepted by this domain. Qualification never changes the
    /// runtime carrier and never authorizes its own establishment.
    pub carrier: StructuralTypeId,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ServiceDeclaration {
    pub id: ServiceId,
    pub identity: String,
    /// Strictly ordered canonical parent closure.
    pub parents: Vec<ServiceId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StructuralMultiplicity {
    Unrestricted,
    Affine,
    Linear,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StructuralParameterDeclaration {
    pub place: PlaceId,
    pub position: u32,
    pub is_self: bool,
    pub structural_type: StructuralTypeId,
    pub multiplicity: StructuralMultiplicity,
    /// Strictly ordered exact signature preconditions. A parameter does not
    /// establish these facts by declaration: its caller or root installation
    /// must discharge them at invocation.
    pub qualifications: Vec<StructuralDomainId>,
}

/// Exact normal structural result signature. The result place is proof-visible
/// and receives ownership only through a `ReturnStructural` edge.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StructuralResultDeclaration {
    pub place: PlaceId,
    pub structural_type: StructuralTypeId,
    pub multiplicity: StructuralMultiplicity,
    /// Strictly ordered qualifications transferred with the value.
    pub qualifications: Vec<StructuralDomainId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StructuralDomainRequirement {
    pub argument_index: u32,
    pub domain: StructuralDomainId,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BoundaryMachineDeclaration {
    pub id: BoundaryMachineId,
    pub identity: String,
    pub attachment: Option<StructuralTypeId>,
    /// This first boundary slice is structurally parameterized and returns
    /// Unit. It therefore declares no scalar parameters or result value.
    pub structural_parameters: Vec<StructuralParameterDeclaration>,
    /// Strictly ordered by `(argument_index, domain)`.
    pub requires: Vec<StructuralDomainRequirement>,
    /// Strictly ordered normalized published ceiling.
    pub published_service_ceiling: Vec<ServiceId>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PropositionDeclaration {
    pub id: psi_core::PropositionId,
    pub name: String,
    pub binders: Vec<PropositionBinderDeclaration>,
    pub parameter_types: Vec<String>,
    pub evidence: PropositionEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PropositionBinderDeclaration {
    pub name: String,
    pub kind: PropositionBinderKind,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PropositionBinderKind {
    Type,
    Const { type_identity: String },
    Machine,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PropositionEvidence {
    FactOnly,
    Witness { evidence_type: String },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PropositionApplicationIdentity {
    pub id: psi_core::PropositionId,
    pub declaration: psi_core::PropositionId,
    pub binder_arguments: Vec<PropositionBinderArgumentIdentity>,
    pub arguments: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PropositionBinderArgumentIdentity {
    pub kind: PropositionBinderArgumentKind,
    pub identity: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PropositionBinderArgumentKind {
    Type,
    Const,
    Machine,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalMachine {
    pub id: MachineId,
    /// Nominal type to which this machine is attached. An attached static
    /// machine need not have a runtime `self` parameter.
    pub attachment: Option<StructuralTypeId>,
    pub parameters: Vec<ValueDeclaration>,
    /// Ordered runtime structural parameters, separate from scalar values.
    pub structural_parameters: Vec<StructuralParameterDeclaration>,
    /// Unit carries no value; scalar results have a stable pseudo-value bound
    /// by every scalar return edge and available to `ensures`.
    pub result: TerminalMachineResult,
    /// Proof-visible roots for structural-place propositions. Runtime scalar
    /// parameters remain independently declared above.
    pub structural_places: Vec<StructuralPlaceDeclaration>,
    /// Dense one-based machine-local claims present at entry, independent of
    /// content projections. Content claims below refine these identities when
    /// present.
    pub entry_claims: Vec<EntryClaim>,
    /// Strictly ordered normalized published boundary-service ceiling.
    pub published_service_ceiling: Vec<ServiceId>,
    /// Canonical machine-local identities for claims present at entry. These
    /// rows name content independently of any later output equality.
    pub content_entry_claims: Vec<ContentEntryClaim>,
    /// Canonical one-to-one claim mappings. These are semantic ownership facts,
    /// not authored algebra theorems: each exact projection below yields one
    /// verifier-reconstructed equality between `input` and `output`.
    pub content_identity_reshuffles: Vec<ContentIdentityReshuffle>,
    /// Exact substitutions of already-authored partition theorems. These rows
    /// retain the source theorem and do not permit a producer to introduce a
    /// new `Separate` node in the derived equation.
    pub content_partition_compositions: Vec<ContentPartitionComposition>,
    pub entry: BlockId,
    pub blocks: Vec<Block>,
    pub contract: MachineContract,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EntryClaim {
    pub claim: ClaimId,
    /// Structural parameter root that owns this claim.
    pub input: PlaceId,
    /// Statically typed structural projection below `input`. Empty names the
    /// complete parameter.
    pub path: Vec<StructuralPathSegment>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StructuralPathSegment {
    Field(String),
    FixedIndex(u64),
}

impl From<String> for StructuralPathSegment {
    fn from(identity: String) -> Self {
        Self::Field(identity)
    }
}

impl From<&str> for StructuralPathSegment {
    fn from(identity: &str) -> Self {
        Self::Field(identity.to_owned())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StructuralPlaceDeclaration {
    pub id: PlaceId,
    pub kind: StructuralPlaceKind,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ClaimContentProjection {
    pub projection: ContentProjectionIdentity,
    pub algebra: ContentAlgebra,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContentEntryClaim {
    pub claim: ClaimId,
    pub input: ContentStructuralPlace,
    /// Strictly ordered by `(projection, algebra)` in canonical modules.
    pub projections: Vec<ClaimContentProjection>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentIdentityReshuffle {
    pub claim: ClaimId,
    pub input: ContentStructuralPlace,
    pub output: ContentStructuralPlace,
    /// Strictly ordered by `(projection, algebra)` in canonical modules.
    pub projections: Vec<ClaimContentProjection>,
}

impl ContentIdentityReshuffle {
    pub fn inferred_propositions(&self) -> impl Iterator<Item = Proposition> + '_ {
        self.projections.iter().map(|content| {
            Proposition::ContentConservation(ContentConservation::new(
                content.algebra.clone(),
                ContentTerm::Projection {
                    projection: content.projection,
                    subject: self.input.clone(),
                },
                ContentTerm::Projection {
                    projection: content.projection,
                    subject: self.output.clone(),
                },
            ))
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContentPlaceSubstitution {
    pub source: ContentStructuralPlace,
    pub target: ContentStructuralPlace,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContentPartitionComposition {
    pub source_fingerprint: u64,
    /// Structural-place declarations for the source callable's theorem. They
    /// live in a namespace local to this witness rather than the wrapper.
    pub source_structural_places: Vec<StructuralPlaceDeclaration>,
    pub source: ContentConservation,
    /// Dense machine-local claims whose exact entry projections participate.
    pub input_claims: Vec<ClaimId>,
    /// Strictly ordered by `source`; every source projection has exactly one
    /// substitution and all rows are used by replay.
    pub substitutions: Vec<ContentPlaceSubstitution>,
    pub derived: ContentConservation,
}

impl ContentPartitionComposition {
    pub fn inferred_proposition(&self) -> Proposition {
        Proposition::ContentConservation(self.derived.clone())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineContract {
    pub id: ContractId,
    /// Strictly ordered canonical may-routes. Omitting a cause forbids it.
    pub crash_routes: Vec<CrashRouteBucket>,
    pub requires: Vec<Proposition>,
    pub ensures: Vec<ContractClause>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CrashRouteBucket {
    pub cause: CrashCause,
    /// Canonical nonempty disjunction. `Truth`, when present, is the sole row.
    pub alternatives: Vec<CrashRouteGuard>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CrashRouteGuard {
    Truth,
    Predicate(CrashPredicateTerm),
}

/// Canonical source-independent term for one normalized crash predicate.
///
/// Terminal Psi retains the proposition itself. The verifier can therefore
/// type-check it, substitute callee values at a call, and reconstruct the exact
/// surviving continuation without trusting producer-authored identity bytes.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CrashPredicateTerm(Proposition);

impl CrashPredicateTerm {
    pub const fn new(proposition: Proposition) -> Self {
        Self(proposition)
    }

    pub const fn proposition(&self) -> &Proposition {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContractClause {
    pub obligation: ObligationId,
    pub proposition: Proposition,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    pub id: BlockId,
    pub parameters: Vec<ValueDeclaration>,
    pub operations: Vec<Operation>,
    pub terminator: Terminator,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Operation {
    pub id: OperationId,
    pub result: OperationResult,
    pub kind: OperationKind,
}

/// Runtime result of one operation. Unit creates no `ValueId`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum OperationResult {
    Unit,
    Scalar(ValueDeclaration),
}

impl OperationResult {
    pub const fn scalar(self) -> Option<ValueDeclaration> {
        match self {
            Self::Unit => None,
            Self::Scalar(value) => Some(value),
        }
    }

    pub const fn scalar_ref(&self) -> Option<&ValueDeclaration> {
        match self {
            Self::Unit => None,
            Self::Scalar(value) => Some(value),
        }
    }

    pub fn scalar_mut(&mut self) -> Option<&mut ValueDeclaration> {
        match self {
            Self::Unit => None,
            Self::Scalar(value) => Some(value),
        }
    }

    /// Scalar-only consumer helper. Callers must reject Unit-capable operations
    /// before using this accessor.
    pub const fn expect_scalar(self) -> ValueDeclaration {
        match self {
            Self::Scalar(value) => value,
            Self::Unit => panic!("Unit operation has no scalar result"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StructuralArgument {
    pub place: PlaceId,
    pub path: Vec<StructuralPathSegment>,
}

/// One exact claim-free affine structural place disposed on an ordinary edge.
/// Unlike the root-only trivial discard vocabulary, this action retains the
/// canonical path and independently checkable leaf type reached by that path.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StructuralAffineDiscard {
    pub place: PlaceId,
    pub path: Vec<StructuralPathSegment>,
    pub structural_type: StructuralTypeId,
}

/// One whole claim-free affine structural parameter disposed by its exact
/// nominal cleanup machine. Unlike a trivial affine discard, this action is
/// executable edge work and therefore retains the selected machine identity.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NominalAffineCleanup {
    pub place: PlaceId,
    pub structural_type: StructuralTypeId,
    pub cleanup_machine: MachineId,
    /// Target-contract-local proof root for the borrowed cleanup receiver.
    /// This is not an executable structural parameter or ABI argument.
    pub cleanup_receiver: Option<PlaceId>,
    /// Obligation identities aligned positionally with the selected cleanup
    /// machine's contextual `requires` clauses.
    pub requirement_obligations: Vec<ObligationId>,
}

/// One exact affine cleanup action committed by a terminal ownership edge.
/// The surrounding vector is the semantic execution order; consumers must not
/// regroup actions by kind or reconstruct their order from declarations.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TerminalAffineCleanupAction {
    DiscardRoot(PlaceId),
    DiscardResidual(StructuralAffineDiscard),
    InvokeNominal(NominalAffineCleanup),
}

/// Transfer one caller-local live claim through the structural argument at
/// `argument_index`. The callee reconstructs its own entry claim from that
/// parameter; callers cannot author callee-local claim identities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ClaimTransfer {
    pub claim: ClaimId,
    pub argument_index: u32,
}

/// Correlate successful completion of one exact bodyless boundary invocation
/// with one caller-local live claim and structural argument position.
///
/// The receipt becomes effective only after the boundary effect succeeds. A
/// rejected effect consumes no claim, so it cannot acknowledge completion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CompletionReceipt {
    pub claim: ClaimId,
    pub argument_index: u32,
}

/// Closed operation vocabulary for the current pre-release compiler.
///
/// `IntegerConstant` writes the declared integer value to its result and
/// establishes the semantic axiom `result == literal`. It cannot trap and
/// generates no additional obligation because construction verifies that the
/// literal belongs to the declared terminal integer type.
///
/// `BooleanConstant` writes the declared Boolean value to its result and
/// establishes `result == literal`.
///
/// `WrappingIntegerAdd` reads two values of
/// the result's exact integer type and reduces their sum modulo the declared
/// width. Signed values interpret the reduced bits as two's complement. It is
/// total and therefore generates no overflow obligation; the verifier
/// reconstructs its exact result-term axiom.
///
/// `SaturatingIntegerMultiply` reads two
/// values of the result's exact integer type and clamps their product at that
/// type's representable bounds. It is total and generates no overflow
/// obligation; the verifier reconstructs its exact result-term axiom.
///
/// `WrappingIntegerMultiply` reads two
/// values of the result's exact integer type and reduces their product modulo
/// the declared width. Signed values interpret the reduced bits as two's
/// complement. It is total and generates no overflow obligation; the verifier
/// reconstructs its exact result-term axiom.
///
/// `SaturatingIntegerSubtract` reads two
/// values of the result's exact integer type and clamps `left - right` at that
/// type's representable bounds. It is total and generates no overflow
/// obligation; the verifier reconstructs its exact result-term axiom.
///
/// `SaturatingIntegerAdd` reads two values
/// of the result's exact integer type and clamps their sum at that type's
/// representable bounds. It is total and therefore generates no overflow
/// obligation; the verifier reconstructs its exact result-term axiom.
///
/// `WrappingIntegerSubtract` reads two
/// values of the result's exact integer type and reduces `left - right` modulo
/// the declared width. Signed values interpret the reduced bits as two's
/// complement. It is total and generates no overflow obligation; the verifier
/// reconstructs its exact result-term axiom.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperationKind {
    /// Establish one whole, claim-free affine empty-record local. This is a
    /// semantic ownership event, not an ABI input or a target storage choice.
    EstablishTrivialAffineLocal {
        destination: PlaceId,
    },
    /// Invoke one in-module machine with positional scalar arguments. Each
    /// callee `requires` clause has the obligation identity at the same index;
    /// successful return binds the operation result. `crash_continuations`
    /// records the invocation-specific no-successor routes that survive call
    /// composition. The verifier reconstructs guarded in-module routes by
    /// substituting callee parameter values with these exact argument values.
    Call {
        callee: MachineId,
        arguments: Vec<ValueId>,
        requirement_obligations: Vec<ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    /// Invoke one in-module Unit machine with positional structural arguments.
    CallUnit {
        callee: MachineId,
        structural_arguments: Vec<StructuralArgument>,
        claim_transfers: Vec<ClaimTransfer>,
        requirement_obligations: Vec<ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
    },
    /// Invoke one exact bodyless boundary Unit machine. Completion receipts
    /// name every live caller claim consumed by the successful invocation at
    /// its exact structural argument position.
    BoundaryCallUnit {
        boundary: BoundaryMachineId,
        structural_arguments: Vec<StructuralArgument>,
        completion_receipts: Vec<CompletionReceipt>,
        requirement_obligations: Vec<ObligationId>,
    },
    /// Immediate x86 port-space byte output. This first closed variant retains
    /// exactly a `u16` port and `u8` value; runtime operands are a later slice.
    /// The exact service identity is carried by the operation rather than
    /// rediscovered from a declaration name by downstream consumers.
    PortWrite {
        service: ServiceId,
        port: u16,
        value: u8,
    },
    IntegerConstant {
        value: IntegerValue,
    },
    BooleanConstant {
        value: bool,
    },
    BooleanNot {
        operand: ValueId,
    },
    BooleanEqual {
        left: ValueId,
        right: ValueId,
    },
    IntegerEqual {
        left: ValueId,
        right: ValueId,
    },
    IntegerLessThan {
        left: ValueId,
        right: ValueId,
    },
    IntegerLessOrEqual {
        left: ValueId,
        right: ValueId,
    },
    IntegerBitwiseNot {
        operand: ValueId,
    },
    IntegerWiden {
        operand: ValueId,
    },
    IntegerExactCast {
        operand: ValueId,
        obligation: ObligationId,
    },
    IntegerBitwiseAnd {
        left: ValueId,
        right: ValueId,
    },
    IntegerBitwiseOr {
        left: ValueId,
        right: ValueId,
    },
    IntegerBitwiseXor {
        left: ValueId,
        right: ValueId,
    },
    WrappingIntegerShiftLeft {
        value: ValueId,
        count: ValueId,
    },
    WrappingIntegerShiftRight {
        value: ValueId,
        count: ValueId,
    },
    ExactIntegerShiftLeft {
        value: ValueId,
        count: ValueId,
        obligation: ObligationId,
    },
    ExactIntegerShiftRight {
        value: ValueId,
        count: ValueId,
        obligation: ObligationId,
    },
    ExactIntegerAdd {
        left: ValueId,
        right: ValueId,
        obligation: ObligationId,
    },
    ExactIntegerSubtract {
        left: ValueId,
        right: ValueId,
        obligation: ObligationId,
    },
    ExactIntegerMultiply {
        left: ValueId,
        right: ValueId,
        obligation: ObligationId,
    },
    ExactIntegerDivide {
        left: ValueId,
        right: ValueId,
        obligation: ObligationId,
    },
    ExactIntegerRemainder {
        left: ValueId,
        right: ValueId,
        obligation: ObligationId,
    },
    WrappingIntegerDivide {
        left: ValueId,
        right: ValueId,
        obligation: ObligationId,
    },
    WrappingIntegerRemainder {
        left: ValueId,
        right: ValueId,
        obligation: ObligationId,
    },
    SaturatingIntegerDivide {
        left: ValueId,
        right: ValueId,
        obligation: ObligationId,
    },
    SaturatingIntegerRemainder {
        left: ValueId,
        right: ValueId,
        obligation: ObligationId,
    },
    WrappingIntegerAdd {
        left: ValueId,
        right: ValueId,
    },
    SaturatingIntegerAdd {
        left: ValueId,
        right: ValueId,
    },
    WrappingIntegerSubtract {
        left: ValueId,
        right: ValueId,
    },
    SaturatingIntegerSubtract {
        left: ValueId,
        right: ValueId,
    },
    WrappingIntegerMultiply {
        left: ValueId,
        right: ValueId,
    },
    SaturatingIntegerMultiply {
        left: ValueId,
        right: ValueId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Terminator {
    /// Simultaneously bind target block parameters from the listed values.
    Jump {
        edge: EdgeId,
        target: BlockId,
        arguments: Vec<ValueId>,
        /// Exact no-code affine discards performed after edge fuel and outgoing
        /// scalar materialization, in reverse parameter declaration order.
        trivial_affine_discards: Vec<PlaceId>,
    },
    /// Select exactly one ordered successor from an already-defined Boolean
    /// value. Exhaustiveness and mutual exclusion are structural.
    Conditional {
        condition: ValueId,
        when_true: SuccessorEdge,
        when_false: SuccessorEdge,
    },
    /// Bind a scalar result, then perform the exact ordered affine cleanup
    /// actions before returning to the caller.
    Return {
        edge: EdgeId,
        value: ValueId,
        /// Semantic execution order. Consumers must preserve this list rather
        /// than regrouping actions by cleanup kind.
        cleanup_actions: Vec<TerminalAffineCleanupAction>,
    },
    /// Finish normally without producing or binding a runtime value.
    ReturnUnit {
        edge: EdgeId,
        /// Exact no-code affine discards performed after outgoing-value
        /// materialization and before control returns to the caller.
        /// Entries are structural places in reverse declaration order.
        trivial_affine_discards: Vec<PlaceId>,
    },
    /// Finish normally after committing an exact projected transfer and then
    /// disposing only the remaining live affine structural places. This is a
    /// distinct pre-release variant so root-only consumers cannot silently
    /// erase path-sensitive cleanup.
    ReturnUnitPartialAffine {
        edge: EdgeId,
        trivial_affine_discards: Vec<PlaceId>,
        residual_affine_discards: Vec<StructuralAffineDiscard>,
    },
    /// Finish normally after executing the exact ordered nominal cleanups for
    /// whole affine structural parameters. Entries are in reverse parameter
    /// declaration order.
    ReturnUnitNominalAffine {
        edge: EdgeId,
        cleanups: Vec<NominalAffineCleanup>,
    },
    /// Transfer one structural value and its complete live claim set to the
    /// machine result. Fuel is charged before any transfer or cleanup commits.
    ReturnStructural {
        edge: EdgeId,
        source: PlaceId,
        /// Strictly ordered exact live claims transferred with `source`.
        returned_claims: Vec<ClaimId>,
        /// Exact no-code affine discards committed after result materialization.
        trivial_affine_discards: Vec<PlaceId>,
    },
    /// Leave checked execution without cleanup or a successor.
    ///
    /// `site_guard` is the canonical conjunction known on every path into this
    /// site. `frontier_lower_bound` is deliberately not described as the
    /// complete process-wide abandonment set: it is the machine-local claim
    /// frontier the verifier can reconstruct at this site.
    Crash {
        edge: EdgeId,
        cause: CrashCause,
        site_guard: Vec<CrashPredicateTerm>,
        frontier_lower_bound: Vec<ClaimId>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CrashCause {
    Trap,
    Abort,
}

impl Terminator {
    /// The sole edge of an unconditional terminator.
    ///
    /// Conditional consumers must use [`Self::edges`] or inspect the selected
    /// successor instead of silently treating one arm as the terminator edge.
    pub const fn edge(&self) -> EdgeId {
        match self {
            Self::Jump { edge, .. }
            | Self::Return { edge, .. }
            | Self::ReturnUnit { edge, .. }
            | Self::ReturnUnitPartialAffine { edge, .. }
            | Self::ReturnUnitNominalAffine { edge, .. }
            | Self::ReturnStructural { edge, .. }
            | Self::Crash { edge, .. } => *edge,
            Self::Conditional { .. } => {
                panic!("a conditional terminator has two successor edges")
            }
        }
    }

    pub fn edges(&self) -> impl Iterator<Item = EdgeId> + '_ {
        let (first, second) = match self {
            Self::Jump { edge, .. }
            | Self::Return { edge, .. }
            | Self::ReturnUnit { edge, .. }
            | Self::ReturnUnitPartialAffine { edge, .. }
            | Self::ReturnUnitNominalAffine { edge, .. }
            | Self::ReturnStructural { edge, .. }
            | Self::Crash { edge, .. } => (*edge, None),
            Self::Conditional {
                when_true,
                when_false,
                ..
            } => (when_true.edge, Some(when_false.edge)),
        };
        std::iter::once(first).chain(second)
    }
}

/// One ordered conditional successor and its simultaneous block-parameter
/// bindings. The bindings are the current scalar edge-action vocabulary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuccessorEdge {
    pub edge: EdgeId,
    pub target: BlockId,
    pub arguments: Vec<ValueId>,
    /// Exact no-code affine discards committed only when this successor is
    /// selected, in reverse parameter declaration order.
    pub trivial_affine_discards: Vec<PlaceId>,
}

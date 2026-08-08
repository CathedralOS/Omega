use std::num::NonZeroU16;

use psi_core::{
    BlockId, ClaimId, ContentAlgebra, ContentConservation, ContentProjectionIdentity,
    ContentStructuralPlace, ContentTerm, ContractId, EdgeId, IntegerValue, MachineId, ObligationId,
    OperationId, PlaceId, Proposition, ScalarType, StructuralPlaceKind, ValueId,
};

/// Version of the in-memory terminal-Psi semantic vocabulary.
///
/// Version 1 has canonical bytes and a semantic fingerprint defined by
/// `psi-terminal-codec`. Version 2 adds `BooleanConstant`; version 3 adds
/// width-relative `WrappingIntegerAdd`; version 4 adds
/// `SaturatingIntegerAdd`; version 5 adds `WrappingIntegerSubtract`; version 6
/// adds `SaturatingIntegerSubtract`; version 7 adds
/// `WrappingIntegerMultiply`; version 8 adds `SaturatingIntegerMultiply`.
/// Version 9 adds structural-place declarations and content-conservation
/// propositions without adding an executable operation.
/// Version 10 adds canonical identity-preserving claim reshuffles from which
/// the verifier reconstructs one-to-one content equalities.
/// Version 11 adds stable sum-case segments to structural content paths.
/// Version 12 adds exact authored-partition substitution rows. The verifier
/// replays each substitution and reconstructs only the resulting theorem.
/// Version 13 adds a structural conditional terminator over an already-defined
/// Boolean value. Its ordered true and false successors have independent edge
/// identities and bindings.
/// Version 14 adds canonical machine-local entry-claim bindings independently
/// of one-to-one output equalities.
/// Version 15 adds total Boolean logical negation.
/// Version 16 adds self-contained nominal proposition declarations and
/// normalized application identities. Transparent aliases remain absent.
/// Version 17 adds total Boolean equality over two already-defined Boolean
/// values.
/// Version 18 adds total integer equality over two already-defined values of
/// one exact integer type.
/// Version 19 adds signedness-aware integer less-than and less-or-equal over
/// two already-defined values of one exact integer type.
/// Version 20 adds total bitwise AND, OR, and XOR over two already-defined
/// values of one exact integer type.
/// Version 21 adds total wrapping left and right shifts. The shifted value and
/// result share one exact integer type; the count retains its own integer type
/// and is reduced modulo the shifted value's width.
/// Version 22 adds an explicit no-successor crash terminator. It records the
/// crash cause, nominal damage-scope demand, and the machine-local claim
/// frontier known to be abandoned; a crash is never encoded as an ordinary
/// terminal transition or as an absent cleanup list.
/// Version 23 separates the body-derived damage minimum from the selected
/// published containment demand. Version 22 decodes conservatively with both
/// fields equal to its single encoded scope.
/// Version 24 adds each machine's effective sparse per-cause crash-context
/// maxima. Absence forbids that cause; older modules migrate with the legacy
/// portable-root maximum for every cause used by one of their crash exits.
/// Version 25 adds fixed-width integer bitwise complement.
/// Version 26 adds universally total integer widening whose target contains the
/// complete source range.
/// Older bytes retain their original meaning and identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SemanticVersion(NonZeroU16);

impl SemanticVersion {
    pub const V1: Self = Self(NonZeroU16::MIN);
    pub const V2: Self = Self(NonZeroU16::new(2).expect("two is nonzero"));
    pub const V3: Self = Self(NonZeroU16::new(3).expect("three is nonzero"));
    pub const V4: Self = Self(NonZeroU16::new(4).expect("four is nonzero"));
    pub const V5: Self = Self(NonZeroU16::new(5).expect("five is nonzero"));
    pub const V6: Self = Self(NonZeroU16::new(6).expect("six is nonzero"));
    pub const V7: Self = Self(NonZeroU16::new(7).expect("seven is nonzero"));
    pub const V8: Self = Self(NonZeroU16::new(8).expect("eight is nonzero"));
    pub const V9: Self = Self(NonZeroU16::new(9).expect("nine is nonzero"));
    pub const V10: Self = Self(NonZeroU16::new(10).expect("ten is nonzero"));
    pub const V11: Self = Self(NonZeroU16::new(11).expect("eleven is nonzero"));
    pub const V12: Self = Self(NonZeroU16::new(12).expect("twelve is nonzero"));
    pub const V13: Self = Self(NonZeroU16::new(13).expect("thirteen is nonzero"));
    pub const V14: Self = Self(NonZeroU16::new(14).expect("fourteen is nonzero"));
    pub const V15: Self = Self(NonZeroU16::new(15).expect("fifteen is nonzero"));
    pub const V16: Self = Self(NonZeroU16::new(16).expect("sixteen is nonzero"));
    pub const V17: Self = Self(NonZeroU16::new(17).expect("seventeen is nonzero"));
    pub const V18: Self = Self(NonZeroU16::new(18).expect("eighteen is nonzero"));
    pub const V19: Self = Self(NonZeroU16::new(19).expect("nineteen is nonzero"));
    pub const V20: Self = Self(NonZeroU16::new(20).expect("twenty is nonzero"));
    pub const V21: Self = Self(NonZeroU16::new(21).expect("twenty-one is nonzero"));
    pub const V22: Self = Self(NonZeroU16::new(22).expect("twenty-two is nonzero"));
    pub const V23: Self = Self(NonZeroU16::new(23).expect("twenty-three is nonzero"));
    pub const V24: Self = Self(NonZeroU16::new(24).expect("twenty-four is nonzero"));
    pub const V25: Self = Self(NonZeroU16::new(25).expect("twenty-five is nonzero"));
    pub const V26: Self = Self(NonZeroU16::new(26).expect("twenty-six is nonzero"));
    pub const CURRENT: Self = Self::V26;

    pub fn new(raw: u16) -> Option<Self> {
        NonZeroU16::new(raw).map(Self)
    }

    pub const fn get(self) -> u16 {
        self.0.get()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ValueDeclaration {
    pub id: ValueId,
    pub scalar_type: ScalarType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalModule {
    pub semantic_version: SemanticVersion,
    pub entry: MachineId,
    /// Nominal proof-formula vocabulary, strictly ordered by `id`.
    /// Transparent aliases never receive a declaration row.
    pub proposition_declarations: Vec<PropositionDeclaration>,
    /// Normalized applications retained without frontend arena handles.
    pub proposition_applications: Vec<PropositionApplicationIdentity>,
    pub machines: Vec<TerminalMachine>,
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
    pub parameters: Vec<ValueDeclaration>,
    /// Stable pseudo-value bound by every return edge and used by `ensures`.
    pub result: ValueDeclaration,
    /// Proof-visible roots for structural-place propositions. Runtime scalar
    /// parameters remain independently declared above.
    pub structural_places: Vec<StructuralPlaceDeclaration>,
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
    /// Strictly ordered by cause. Missing causes are forbidden in this
    /// execution context.
    pub crash_context: Vec<CrashContextMaximum>,
    pub requires: Vec<Proposition>,
    pub ensures: Vec<ContractClause>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CrashContextMaximum {
    pub cause: CrashCause,
    pub maximum_scope: String,
}

impl CrashContextMaximum {
    pub fn portable_root() -> Vec<Self> {
        vec![
            Self {
                cause: CrashCause::Trap,
                maximum_scope: "ExecutionDomain".to_owned(),
            },
            Self {
                cause: CrashCause::Abort,
                maximum_scope: "ExecutionDomain".to_owned(),
            },
        ]
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
    pub result: ValueDeclaration,
    pub kind: OperationKind,
}

/// Closed operation vocabulary through semantic version 8.
///
/// `IntegerConstant` writes the declared integer value to its result and
/// establishes the semantic axiom `result == literal`. It cannot trap and
/// generates no additional obligation because construction verifies that the
/// literal belongs to the declared terminal integer type.
///
/// `BooleanConstant` was added in semantic version 2. It writes the declared
/// Boolean value to its result and establishes `result == literal`.
///
/// `WrappingIntegerAdd` was added in semantic version 3. It reads two values of
/// the result's exact integer type and reduces their sum modulo the declared
/// width. Signed values interpret the reduced bits as two's complement. It is
/// total and therefore generates no overflow obligation; the verifier
/// reconstructs its exact result-term axiom.
///
/// `SaturatingIntegerMultiply` was added in semantic version 8. It reads two
/// values of the result's exact integer type and clamps their product at that
/// type's representable bounds. It is total and generates no overflow
/// obligation; the verifier reconstructs its exact result-term axiom.
///
/// `WrappingIntegerMultiply` was added in semantic version 7. It reads two
/// values of the result's exact integer type and reduces their product modulo
/// the declared width. Signed values interpret the reduced bits as two's
/// complement. It is total and generates no overflow obligation; the verifier
/// reconstructs its exact result-term axiom.
///
/// `SaturatingIntegerSubtract` was added in semantic version 6. It reads two
/// values of the result's exact integer type and clamps `left - right` at that
/// type's representable bounds. It is total and generates no overflow
/// obligation; the verifier reconstructs its exact result-term axiom.
///
/// `SaturatingIntegerAdd` was added in semantic version 4. It reads two values
/// of the result's exact integer type and clamps their sum at that type's
/// representable bounds. It is total and therefore generates no overflow
/// obligation; the verifier reconstructs its exact result-term axiom.
///
/// `WrappingIntegerSubtract` was added in semantic version 5. It reads two
/// values of the result's exact integer type and reduces `left - right` modulo
/// the declared width. Signed values interpret the reduced bits as two's
/// complement. It is total and generates no overflow obligation; the verifier
/// reconstructs its exact result-term axiom.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationKind {
    IntegerConstant { value: IntegerValue },
    BooleanConstant { value: bool },
    BooleanNot { operand: ValueId },
    BooleanEqual { left: ValueId, right: ValueId },
    IntegerEqual { left: ValueId, right: ValueId },
    IntegerLessThan { left: ValueId, right: ValueId },
    IntegerLessOrEqual { left: ValueId, right: ValueId },
    IntegerBitwiseNot { operand: ValueId },
    IntegerWiden { operand: ValueId },
    IntegerBitwiseAnd { left: ValueId, right: ValueId },
    IntegerBitwiseOr { left: ValueId, right: ValueId },
    IntegerBitwiseXor { left: ValueId, right: ValueId },
    WrappingIntegerShiftLeft { value: ValueId, count: ValueId },
    WrappingIntegerShiftRight { value: ValueId, count: ValueId },
    WrappingIntegerAdd { left: ValueId, right: ValueId },
    SaturatingIntegerAdd { left: ValueId, right: ValueId },
    WrappingIntegerSubtract { left: ValueId, right: ValueId },
    SaturatingIntegerSubtract { left: ValueId, right: ValueId },
    WrappingIntegerMultiply { left: ValueId, right: ValueId },
    SaturatingIntegerMultiply { left: ValueId, right: ValueId },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Terminator {
    /// Simultaneously bind target block parameters from the listed values.
    Jump {
        edge: EdgeId,
        target: BlockId,
        arguments: Vec<ValueId>,
    },
    /// Select exactly one ordered successor from an already-defined Boolean
    /// value. Exhaustiveness and mutual exclusion are structural.
    Conditional {
        condition: ValueId,
        when_true: SuccessorEdge,
        when_false: SuccessorEdge,
    },
    /// Bind the machine's stable result pseudo-value and finish execution.
    Return { edge: EdgeId, value: ValueId },
    /// Leave checked execution without cleanup or a successor.
    ///
    /// Both scopes are portable nominal identities; installation gives the
    /// selected demand physical meaning. `frontier_lower_bound` is deliberately
    /// not described as the complete process-wide abandonment set: it is the
    /// machine-local claim frontier the verifier can reconstruct at this site.
    Crash {
        edge: EdgeId,
        cause: CrashCause,
        damage_minimum: String,
        containment_demand: String,
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
            Self::Jump { edge, .. } | Self::Return { edge, .. } | Self::Crash { edge, .. } => *edge,
            Self::Conditional { .. } => {
                panic!("a conditional terminator has two successor edges")
            }
        }
    }

    pub fn edges(&self) -> impl Iterator<Item = EdgeId> + '_ {
        let (first, second) = match self {
            Self::Jump { edge, .. } | Self::Return { edge, .. } | Self::Crash { edge, .. } => {
                (*edge, None)
            }
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
}

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
    pub const CURRENT: Self = Self::V10;

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
    pub machines: Vec<TerminalMachine>,
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
    /// Canonical one-to-one claim mappings. These are semantic ownership facts,
    /// not authored algebra theorems: each exact projection below yields one
    /// verifier-reconstructed equality between `input` and `output`.
    pub content_identity_reshuffles: Vec<ContentIdentityReshuffle>,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineContract {
    pub id: ContractId,
    pub requires: Vec<Proposition>,
    pub ensures: Vec<ContractClause>,
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
    /// Bind the machine's stable result pseudo-value and finish execution.
    Return { edge: EdgeId, value: ValueId },
}

impl Terminator {
    pub const fn edge(&self) -> EdgeId {
        match self {
            Self::Jump { edge, .. } | Self::Return { edge, .. } => *edge,
        }
    }
}

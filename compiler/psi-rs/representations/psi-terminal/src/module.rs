use std::num::NonZeroU16;

use psi_core::{
    BlockId, ContractId, EdgeId, IntegerValue, MachineId, ObligationId, OperationId, Proposition,
    ScalarType, ValueId,
};

/// Version of the in-memory terminal-Psi semantic vocabulary.
///
/// Version 1 has canonical bytes and a semantic fingerprint defined by
/// `psi-terminal-codec`. Version 2 adds `BooleanConstant`; version 3 adds
/// width-relative `WrappingIntegerAdd`. Older bytes retain their original
/// meaning and identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SemanticVersion(NonZeroU16);

impl SemanticVersion {
    pub const V1: Self = Self(NonZeroU16::MIN);
    pub const V2: Self = Self(NonZeroU16::new(2).expect("two is nonzero"));
    pub const V3: Self = Self(NonZeroU16::new(3).expect("three is nonzero"));
    pub const CURRENT: Self = Self::V3;

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
    pub entry: BlockId,
    pub blocks: Vec<Block>,
    pub contract: MachineContract,
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

/// Closed operation vocabulary through semantic version 3.
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationKind {
    IntegerConstant { value: IntegerValue },
    BooleanConstant { value: bool },
    WrappingIntegerAdd { left: ValueId, right: ValueId },
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

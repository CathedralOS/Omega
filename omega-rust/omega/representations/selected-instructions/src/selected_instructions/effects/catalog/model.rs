use register_model::{RegisterConstraintCatalogIdentity, RegisterConstraintKey, RegisterViewId};
use target::NativeTarget;

use crate::SelectedConstraintKeys;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MachineEffectCatalogIdentity([u8; 32]);

impl MachineEffectCatalogIdentity {
    pub(crate) fn from_canonical_bytes(bytes: &[u8]) -> Self {
        use sha2::{Digest, Sha256};
        Self(Sha256::digest(bytes).into())
    }

    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MachineSemanticKind {
    Load8Indexed,
    CompareI64Zero,
    MaterializeI64,
    CopyI64,
    ExactAddI64,
    ExactAddI64Immediate,
    ExactSubtractI64,
    ExactSubtractI64Immediate,
    ConditionalBranchNonZero,
    ReturnI64,
    ReturnUnit,
    CompareI64,
    ConditionalBranchU64LessThan,
    ConditionalBranchI64LessThan,
    CallI64,
    Jump,
    ZeroExtendU8,
    ZeroExtendU32,
    Load64,
    Store64,
    FrameAddress,
    CallUnit,
    ByteViewAddress,
    HostedWriteByteI32,
    Store,
    AddressOffset,
}

impl MachineSemanticKind {
    pub const ALL: [Self; 26] = [
        Self::Load8Indexed,
        Self::CompareI64Zero,
        Self::MaterializeI64,
        Self::CopyI64,
        Self::ExactAddI64,
        Self::ExactAddI64Immediate,
        Self::ExactSubtractI64,
        Self::ExactSubtractI64Immediate,
        Self::ConditionalBranchNonZero,
        Self::ReturnI64,
        Self::ReturnUnit,
        Self::CompareI64,
        Self::ConditionalBranchU64LessThan,
        Self::ConditionalBranchI64LessThan,
        Self::CallI64,
        Self::Jump,
        Self::ZeroExtendU8,
        Self::ZeroExtendU32,
        Self::Load64,
        Self::Store64,
        Self::FrameAddress,
        Self::CallUnit,
        Self::ByteViewAddress,
        Self::HostedWriteByteI32,
        Self::Store,
        Self::AddressOffset,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MachineAlternativeFamily {
    Load8Indexed,
    CompareI64Zero,
    MaterializeI64,
    CopyI64,
    ExactAddI64,
    ExactAddI64Immediate,
    ExactSubtractI64,
    ExactSubtractI64Immediate,
    ConditionalBranchNonZero,
    ReturnI64,
    ReturnUnit,
    CompareI64,
    ConditionalBranchU64LessThan,
    ConditionalBranchI64LessThan,
    CallI64,
    Jump,
    ZeroExtendU8,
    ZeroExtendU32,
    Load64,
    Store64,
    FrameAddress,
    CallUnit,
    ByteViewAddress,
    HostedWriteByteI32,
    Store,
    AddressOffset,
}

impl From<MachineSemanticKind> for MachineAlternativeFamily {
    fn from(value: MachineSemanticKind) -> Self {
        match value {
            MachineSemanticKind::HostedWriteByteI32 => Self::HostedWriteByteI32,
            MachineSemanticKind::Store => Self::Store,
            MachineSemanticKind::AddressOffset => Self::AddressOffset,
            MachineSemanticKind::ByteViewAddress => Self::ByteViewAddress,
            MachineSemanticKind::Load8Indexed => Self::Load8Indexed,
            MachineSemanticKind::CompareI64Zero => Self::CompareI64Zero,
            MachineSemanticKind::MaterializeI64 => Self::MaterializeI64,
            MachineSemanticKind::CopyI64 => Self::CopyI64,
            MachineSemanticKind::ExactAddI64 => Self::ExactAddI64,
            MachineSemanticKind::ExactAddI64Immediate => Self::ExactAddI64Immediate,
            MachineSemanticKind::ExactSubtractI64 => Self::ExactSubtractI64,
            MachineSemanticKind::ExactSubtractI64Immediate => Self::ExactSubtractI64Immediate,
            MachineSemanticKind::ConditionalBranchNonZero => Self::ConditionalBranchNonZero,
            MachineSemanticKind::ReturnI64 => Self::ReturnI64,
            MachineSemanticKind::ReturnUnit => Self::ReturnUnit,
            MachineSemanticKind::CompareI64 => Self::CompareI64,
            MachineSemanticKind::ConditionalBranchU64LessThan => Self::ConditionalBranchU64LessThan,
            MachineSemanticKind::ConditionalBranchI64LessThan => Self::ConditionalBranchI64LessThan,
            MachineSemanticKind::CallI64 => Self::CallI64,
            MachineSemanticKind::Jump => Self::Jump,
            MachineSemanticKind::ZeroExtendU8 => Self::ZeroExtendU8,
            MachineSemanticKind::ZeroExtendU32 => Self::ZeroExtendU32,
            MachineSemanticKind::Load64 => Self::Load64,
            MachineSemanticKind::Store64 => Self::Store64,
            MachineSemanticKind::FrameAddress => Self::FrameAddress,
            MachineSemanticKind::CallUnit => Self::CallUnit,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MachineAlternativeKey {
    pub family: MachineAlternativeFamily,
    pub variant: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MachineAlternativeApplicability {
    Always,
    ResultAliasesOperand {
        result: u16,
        operand: u16,
    },
    ResultAliasesOperandAndDistinctFromOperand {
        result: u16,
        aliased_operand: u16,
        distinct_operand: u16,
    },
    ResultAliasesOperands {
        result: u16,
        left: u16,
        right: u16,
    },
    ResultDistinctFromOperands {
        result: u16,
        left: u16,
        right: u16,
    },
    /// A commutative target form for which either input may fill the restricted
    /// encoding role, but one named physical view cannot fill that role.
    AtLeastOneOperandDoesNotAliasView {
        left: u16,
        right: u16,
        excluded_view: RegisterViewId,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MachineMemoryEffect {
    /// Private-byte initialization and kernel read, with an observable stdout write.
    HostedWriteByteV1,
    WritePointerV1,
    NoneV1,
    ReadPointerV1,
    WriteFrameStorageV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MachineTrapBehavior {
    HostedWriteFailureV1,
    NeverV1,
    MayArchitecturalFaultV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MachineBarrier {
    ExternalEffect,
    None,
    ControlFlow,
    Call,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MachineCallEffect {
    NoneV1,
    DirectInternalNormalReturnV1 { pre_call_stack_alignment: u16 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MachineCleanupEffect {
    NoneV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MachineSizeKnowledge {
    ExactBytes(u16),
    EncoderResolved {
        minimum_bytes: u16,
        maximum_bytes: Option<u16>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MachineLatencyKnowledge {
    StableBaselineUnavailable,
}

/// External dependencies and architectural effects of one encoded
/// alternative. These refine, but never replace, the selected instruction's
/// semantic/ABI operand custody and complete conservative constraint row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineEncodedEffects {
    /// Numbered selected operands whose incoming values affect the encoded
    /// result. Internal reads of values defined earlier in a multi-instruction
    /// realization are deliberately excluded.
    pub external_operand_reads: Vec<u16>,
    /// Numbered selected operands whose physical homes are written.
    pub external_operand_writes: Vec<u16>,
    pub implicit_unit_uses: Vec<register_model::RegisterUnitId>,
    pub implicit_unit_defs: Vec<register_model::RegisterUnitId>,
    pub implicit_unit_clobbers: Vec<register_model::RegisterUnitId>,
    pub memory: MachineEncodedMemoryEffect,
    pub stack: MachineEncodedStackEffect,
    pub trap: MachineEncodedTrapBehavior,
    pub control: MachineEncodedControlEffect,
}

impl MachineEncodedEffects {
    pub fn fallthrough_v1(
        external_operand_reads: Vec<u16>,
        external_operand_writes: Vec<u16>,
    ) -> Self {
        Self {
            external_operand_reads,
            external_operand_writes,
            implicit_unit_uses: Vec::new(),
            implicit_unit_defs: Vec::new(),
            implicit_unit_clobbers: Vec::new(),
            memory: MachineEncodedMemoryEffect::NoneV1,
            stack: MachineEncodedStackEffect::UnchangedV1,
            trap: MachineEncodedTrapBehavior::NeverV1,
            control: MachineEncodedControlEffect::FallThroughV1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MachineEncodedMemoryEffect {
    /// Write one frame byte, then let the selected host kernel read it for stdout.
    HostedWriteByteV1 {
        stack_pointer: RegisterViewId,
    },
    /// Exact footprint is the receiving-validated Store instruction byte size.
    WritePointerV1 {
        pointer_operand: u16,
    },
    ReadIndexedPointerV1 {
        pointer_operand: u16,
        index_operand: u16,
        byte_count: u16,
    },
    NoneV1,
    ReadPointerV1 {
        pointer_operand: u16,
        byte_count: u16,
    },
    WriteFrameStorageV1 {
        stack_pointer: RegisterViewId,
        byte_count: u16,
    },
    ReadActivationStackV1 {
        stack_pointer: RegisterViewId,
        byte_count: u16,
    },
    WriteReturnAddressBelowStackPointerV1 {
        stack_pointer: RegisterViewId,
        byte_count: u16,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MachineEncodedStackEffect {
    UnchangedV1,
    PopBytesV1 {
        stack_pointer: RegisterViewId,
        byte_count: u16,
    },
    CallReturnAddressLifecycleV1 {
        stack_pointer: RegisterViewId,
        return_address_byte_count: u16,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MachineEncodedTrapBehavior {
    /// Architectural faults remain possible; a nonpositive syscall result traps.
    HostedWriteFailureV1,
    NeverV1,
    MayArchitecturalFaultV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MachineEncodedControlEffect {
    HostedWriteReturnOrTrapV1,
    FallThroughV1,
    ConditionalRelativeBranchV1,
    ReturnFromActivationStackV1,
    ReturnIndirectRegisterV1 { target: RegisterViewId },
    DirectRelativeCallV1,
    UnconditionalRelativeBranchV1,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineAlternative {
    pub key: MachineAlternativeKey,
    pub applicability: MachineAlternativeApplicability,
    pub size: MachineSizeKnowledge,
    pub latency: MachineLatencyKnowledge,
    pub encoded: MachineEncodedEffects,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineEffectDeclaration {
    pub semantic: MachineSemanticKind,
    pub constraint: RegisterConstraintKey,
    pub memory: MachineMemoryEffect,
    pub trap: MachineTrapBehavior,
    pub barrier: MachineBarrier,
    pub call: MachineCallEffect,
    pub cleanup: MachineCleanupEffect,
    pub alternatives: Vec<MachineAlternative>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineEffectCatalog {
    pub target: NativeTarget,
    pub register_constraints: RegisterConstraintCatalogIdentity,
    pub selected_keys: SelectedConstraintKeys,
    pub declarations: Vec<MachineEffectDeclaration>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedMachineEffectCatalog {
    pub(super) catalog: MachineEffectCatalog,
    pub(super) identity: MachineEffectCatalogIdentity,
}

impl ValidatedMachineEffectCatalog {
    pub const fn catalog(&self) -> &MachineEffectCatalog {
        &self.catalog
    }

    pub const fn identity(&self) -> MachineEffectCatalogIdentity {
        self.identity
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MachineEffectCatalogValidationError {
    TargetArchitectureMismatch,
    RegisterConstraintRootMismatch,
    DuplicateSelectedConstraintKey,
    NonCanonicalDeclarations,
    DeclarationRosterMismatch,
    UnknownConstraint(MachineSemanticKind),
    NonCanonicalAlternatives(MachineSemanticKind),
    EmptyAlternatives(MachineSemanticKind),
    AlternativeFamilyMismatch(MachineSemanticKind),
    InvalidAlternativeApplicability(MachineSemanticKind),
    InvalidEncodedEffects(MachineSemanticKind),
    InvalidSizeKnowledge(MachineSemanticKind),
    BarrierMismatch(MachineSemanticKind),
}

impl std::fmt::Display for MachineEffectCatalogValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid machine-effect catalog: {self:?}")
    }
}

impl std::error::Error for MachineEffectCatalogValidationError {}

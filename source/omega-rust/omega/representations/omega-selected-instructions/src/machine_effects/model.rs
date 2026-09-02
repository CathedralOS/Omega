use omega_register_model::{
    RegisterConstraintCatalogIdentity, RegisterConstraintKey, RegisterViewId,
};
use omega_target::NativeTarget;

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
}

impl MachineSemanticKind {
    pub const ALL: [Self; 12] = [
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
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MachineAlternativeFamily {
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
}

impl From<MachineSemanticKind> for MachineAlternativeFamily {
    fn from(value: MachineSemanticKind) -> Self {
        match value {
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
    NoneV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MachineTrapBehavior {
    NeverV1,
    MayArchitecturalFaultV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MachineBarrier {
    None,
    ControlFlow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MachineCallEffect {
    NoneV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MachineCleanupEffect {
    NoneV1,
}

/// Structural memory footprint of the bounded Microsoft-x64 Unit call pseudo.
/// This is semantic/pre-allocation custody, not an emitted load/store recipe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StructuralUnitCallMemoryEffect {
    ReadOwnedIndirectPairWriteCallerCopiesV1 {
        root_byte_count: u16,
        copy_stack_byte_offsets: [u32; 2],
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StructuralUnitCallFrameEffect {
    BalancedCallerFrameV1 {
        frame_byte_count: u32,
        shadow_byte_count: u32,
        pre_call_stack_alignment: u16,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StructuralUnitCallBarrier {
    CallV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StructuralUnitCallEffect {
    DirectInternalUnitV1,
}

/// Target-applicable semantic machine effects for the atomic structural call.
/// Keeping this outside the ordinary alternative roster prevents this stage
/// from claiming an encoding, displacement, symbol, or object relocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StructuralUnitCallEffectDeclaration {
    pub constraint: RegisterConstraintKey,
    pub memory: StructuralUnitCallMemoryEffect,
    pub frame: StructuralUnitCallFrameEffect,
    pub trap: MachineTrapBehavior,
    pub barrier: StructuralUnitCallBarrier,
    pub call: StructuralUnitCallEffect,
    pub cleanup: MachineCleanupEffect,
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
    pub implicit_unit_uses: Vec<omega_register_model::RegisterUnitId>,
    pub implicit_unit_defs: Vec<omega_register_model::RegisterUnitId>,
    pub implicit_unit_clobbers: Vec<omega_register_model::RegisterUnitId>,
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
    NoneV1,
    ReadActivationStackV1 {
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MachineEncodedTrapBehavior {
    NeverV1,
    MayArchitecturalFaultV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MachineEncodedControlEffect {
    FallThroughV1,
    ConditionalRelativeBranchV1,
    ReturnFromActivationStackV1,
    ReturnIndirectRegisterV1 { target: RegisterViewId },
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
    pub structural_unit_call: Option<StructuralUnitCallEffectDeclaration>,
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
    StructuralCallDeclarationMismatch,
}

impl std::fmt::Display for MachineEffectCatalogValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid machine-effect catalog: {self:?}")
    }
}

impl std::error::Error for MachineEffectCatalogValidationError {}

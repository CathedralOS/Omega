use std::collections::BTreeSet;

use omega_register_model::{
    RegisterConstraintCatalogIdentity, RegisterConstraintKey, RegisterInstructionConstraint,
    RegisterOperandAccess, RegisterViewId, ValidatedRegisterConstraintCatalog,
};
use omega_target::NativeTarget;

use crate::{SelectedConstraintKeys, machine_effect_catalog_identity};

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
}

impl MachineSemanticKind {
    pub const ALL: [Self; 10] = [
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
    catalog: MachineEffectCatalog,
    identity: MachineEffectCatalogIdentity,
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

pub fn validate_machine_effect_catalog(
    constraints: &ValidatedRegisterConstraintCatalog,
    catalog: MachineEffectCatalog,
) -> Result<ValidatedMachineEffectCatalog, MachineEffectCatalogValidationError> {
    if catalog.target.architecture != constraints.architecture() {
        return Err(MachineEffectCatalogValidationError::TargetArchitectureMismatch);
    }
    if catalog.register_constraints != constraints.identity() {
        return Err(MachineEffectCatalogValidationError::RegisterConstraintRootMismatch);
    }
    let selected = catalog.selected_keys.in_identity_order();
    if selected.iter().copied().collect::<BTreeSet<_>>().len() != selected.len() {
        return Err(MachineEffectCatalogValidationError::DuplicateSelectedConstraintKey);
    }
    if catalog
        .declarations
        .windows(2)
        .any(|pair| pair[0].semantic >= pair[1].semantic)
    {
        return Err(MachineEffectCatalogValidationError::NonCanonicalDeclarations);
    }
    let expected = MachineSemanticKind::ALL
        .map(|semantic| (semantic, catalog.selected_keys.for_semantic(semantic)));
    if catalog.declarations.len() != expected.len()
        || catalog
            .declarations
            .iter()
            .zip(expected)
            .any(|(actual, (semantic, constraint))| {
                actual.semantic != semantic || actual.constraint != constraint
            })
    {
        return Err(MachineEffectCatalogValidationError::DeclarationRosterMismatch);
    }
    validate_structural_unit_call(constraints, &catalog)?;
    for declaration in &catalog.declarations {
        let row = constraints
            .catalog()
            .constraints
            .iter()
            .find(|row| row.key == declaration.constraint)
            .ok_or(MachineEffectCatalogValidationError::UnknownConstraint(
                declaration.semantic,
            ))?;
        validate_declaration(row, declaration)?;
    }
    let identity = machine_effect_catalog_identity(&catalog);
    Ok(ValidatedMachineEffectCatalog { catalog, identity })
}

fn validate_structural_unit_call(
    constraints: &ValidatedRegisterConstraintCatalog,
    catalog: &MachineEffectCatalog,
) -> Result<(), MachineEffectCatalogValidationError> {
    let (Some(key), Some(declaration)) = (
        catalog.selected_keys.structural_unit_call,
        catalog.structural_unit_call,
    ) else {
        return if catalog.selected_keys.structural_unit_call.is_none()
            && catalog.structural_unit_call.is_none()
        {
            Ok(())
        } else {
            Err(MachineEffectCatalogValidationError::StructuralCallDeclarationMismatch)
        };
    };
    let row = constraints
        .catalog()
        .constraints
        .iter()
        .find(|row| row.key == key)
        .ok_or(MachineEffectCatalogValidationError::StructuralCallDeclarationMismatch)?;
    if declaration.constraint != key
        || !row.operands.is_empty()
        || row.implicit_uses.is_empty()
        || row.implicit_defs.is_empty()
        || row.clobbers.is_empty()
        || declaration.memory
            != (StructuralUnitCallMemoryEffect::ReadOwnedIndirectPairWriteCallerCopiesV1 {
                root_byte_count: 16,
                copy_stack_byte_offsets: [32, 48],
            })
        || declaration.frame
            != (StructuralUnitCallFrameEffect::BalancedCallerFrameV1 {
                frame_byte_count: 72,
                shadow_byte_count: 32,
                pre_call_stack_alignment: 16,
            })
        || declaration.trap != MachineTrapBehavior::MayArchitecturalFaultV1
        || declaration.barrier != StructuralUnitCallBarrier::CallV1
        || declaration.call != StructuralUnitCallEffect::DirectInternalUnitV1
        || declaration.cleanup != MachineCleanupEffect::NoneV1
    {
        return Err(MachineEffectCatalogValidationError::StructuralCallDeclarationMismatch);
    }
    Ok(())
}

fn validate_declaration(
    constraint: &RegisterInstructionConstraint,
    declaration: &MachineEffectDeclaration,
) -> Result<(), MachineEffectCatalogValidationError> {
    let semantic = declaration.semantic;
    let expected_barrier = if matches!(
        semantic,
        MachineSemanticKind::ConditionalBranchNonZero
            | MachineSemanticKind::ReturnI64
            | MachineSemanticKind::ReturnUnit
    ) {
        MachineBarrier::ControlFlow
    } else {
        MachineBarrier::None
    };
    if declaration.barrier != expected_barrier {
        return Err(MachineEffectCatalogValidationError::BarrierMismatch(
            semantic,
        ));
    }
    if declaration.alternatives.is_empty() {
        return Err(MachineEffectCatalogValidationError::EmptyAlternatives(
            semantic,
        ));
    }
    if declaration
        .alternatives
        .windows(2)
        .any(|pair| pair[0].key >= pair[1].key)
    {
        return Err(MachineEffectCatalogValidationError::NonCanonicalAlternatives(semantic));
    }
    let expected_family = semantic.into();
    for alternative in &declaration.alternatives {
        if alternative.key.family != expected_family {
            return Err(MachineEffectCatalogValidationError::AlternativeFamilyMismatch(semantic));
        }
        validate_applicability(constraint, alternative.applicability).map_err(|()| {
            MachineEffectCatalogValidationError::InvalidAlternativeApplicability(semantic)
        })?;
        validate_encoded_effects(constraint, declaration, &alternative.encoded)
            .map_err(|()| MachineEffectCatalogValidationError::InvalidEncodedEffects(semantic))?;
        match alternative.size {
            MachineSizeKnowledge::ExactBytes(0)
            | MachineSizeKnowledge::EncoderResolved {
                minimum_bytes: 0, ..
            } => {
                return Err(MachineEffectCatalogValidationError::InvalidSizeKnowledge(
                    semantic,
                ));
            }
            MachineSizeKnowledge::EncoderResolved {
                minimum_bytes,
                maximum_bytes: Some(maximum),
            } if maximum < minimum_bytes => {
                return Err(MachineEffectCatalogValidationError::InvalidSizeKnowledge(
                    semantic,
                ));
            }
            _ => {}
        }
    }
    Ok(())
}

fn validate_encoded_effects(
    constraint: &RegisterInstructionConstraint,
    declaration: &MachineEffectDeclaration,
    encoded: &MachineEncodedEffects,
) -> Result<(), ()> {
    let canonical = |values: &[u16]| values.windows(2).all(|pair| pair[0] < pair[1]);
    if !canonical(&encoded.external_operand_reads)
        || !canonical(&encoded.external_operand_writes)
        || encoded
            .implicit_unit_uses
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        || encoded
            .implicit_unit_defs
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        || encoded
            .implicit_unit_clobbers
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
    {
        return Err(());
    }
    for operand in &encoded.external_operand_reads {
        let row = constraint
            .operands
            .iter()
            .find(|row| row.operand == *operand)
            .ok_or(())?;
        if !matches!(
            row.access,
            RegisterOperandAccess::Use | RegisterOperandAccess::UseDef
        ) {
            return Err(());
        }
    }
    for operand in &encoded.external_operand_writes {
        let row = constraint
            .operands
            .iter()
            .find(|row| row.operand == *operand)
            .ok_or(())?;
        if !matches!(
            row.access,
            RegisterOperandAccess::Def | RegisterOperandAccess::UseDef
        ) {
            return Err(());
        }
    }
    if !encoded
        .implicit_unit_uses
        .iter()
        .all(|unit| constraint.implicit_uses.contains(unit))
        || !encoded
            .implicit_unit_defs
            .iter()
            .all(|unit| constraint.implicit_defs.contains(unit))
        || !encoded
            .implicit_unit_clobbers
            .iter()
            .all(|unit| constraint.clobbers.contains(unit))
    {
        return Err(());
    }
    let control = !matches!(encoded.control, MachineEncodedControlEffect::FallThroughV1);
    if control != matches!(declaration.barrier, MachineBarrier::ControlFlow) {
        return Err(());
    }
    match (encoded.memory, encoded.stack, encoded.trap) {
        (
            MachineEncodedMemoryEffect::ReadActivationStackV1 {
                stack_pointer: memory_pointer,
                byte_count: memory_bytes,
            },
            MachineEncodedStackEffect::PopBytesV1 {
                stack_pointer,
                byte_count: stack_bytes,
            },
            MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
        ) if memory_pointer == stack_pointer
            && memory_bytes == stack_bytes
            && memory_bytes != 0 => {}
        (MachineEncodedMemoryEffect::NoneV1, MachineEncodedStackEffect::UnchangedV1, _) => {}
        _ => return Err(()),
    }
    Ok(())
}

fn validate_applicability(
    constraint: &RegisterInstructionConstraint,
    applicability: MachineAlternativeApplicability,
) -> Result<(), ()> {
    let operand = |number| {
        constraint
            .operands
            .iter()
            .find(|operand| operand.operand == number)
    };
    let reads = |access| {
        matches!(
            access,
            RegisterOperandAccess::Use | RegisterOperandAccess::UseDef
        )
    };
    let writes = |access| {
        matches!(
            access,
            RegisterOperandAccess::Def | RegisterOperandAccess::UseDef
        )
    };
    match applicability {
        MachineAlternativeApplicability::Always => Ok(()),
        MachineAlternativeApplicability::ResultAliasesOperand {
            result,
            operand: input,
        } => {
            let (Some(result), Some(input)) = (operand(result), operand(input)) else {
                return Err(());
            };
            (result.operand != input.operand
                && writes(result.access)
                && reads(input.access)
                && result.class == input.class)
                .then_some(())
                .ok_or(())
        }
        MachineAlternativeApplicability::ResultAliasesOperandAndDistinctFromOperand {
            result,
            aliased_operand,
            distinct_operand,
        } => {
            let (Some(result), Some(aliased), Some(distinct)) = (
                operand(result),
                operand(aliased_operand),
                operand(distinct_operand),
            ) else {
                return Err(());
            };
            (result.operand != aliased.operand
                && result.operand != distinct.operand
                && aliased.operand != distinct.operand
                && writes(result.access)
                && reads(aliased.access)
                && reads(distinct.access)
                && result.class == aliased.class
                && result.class == distinct.class)
                .then_some(())
                .ok_or(())
        }
        MachineAlternativeApplicability::ResultAliasesOperands {
            result,
            left,
            right,
        } => {
            let (Some(result), Some(left), Some(right)) =
                (operand(result), operand(left), operand(right))
            else {
                return Err(());
            };
            (result.operand != left.operand
                && result.operand != right.operand
                && left.operand != right.operand
                && writes(result.access)
                && reads(left.access)
                && reads(right.access)
                && result.class == left.class
                && result.class == right.class)
                .then_some(())
                .ok_or(())
        }
        MachineAlternativeApplicability::ResultDistinctFromOperands {
            result,
            left,
            right,
        } => {
            let (Some(result), Some(left), Some(right)) =
                (operand(result), operand(left), operand(right))
            else {
                return Err(());
            };
            (result.operand != left.operand
                && result.operand != right.operand
                && left.operand != right.operand
                && writes(result.access)
                && reads(left.access)
                && reads(right.access)
                && result.class == left.class
                && result.class == right.class)
                .then_some(())
                .ok_or(())
        }
        MachineAlternativeApplicability::AtLeastOneOperandDoesNotAliasView {
            left, right, ..
        } => {
            let (Some(left), Some(right)) = (operand(left), operand(right)) else {
                return Err(());
            };
            (left.operand != right.operand
                && reads(left.access)
                && reads(right.access)
                && left.class == right.class)
                .then_some(())
                .ok_or(())
        }
    }
}

impl SelectedConstraintKeys {
    pub fn in_identity_order(self) -> Vec<RegisterConstraintKey> {
        self.structural_unit_call
            .into_iter()
            .chain([
                self.materialize_i64,
                self.copy_i64,
                self.add_i64,
                self.add_i64_immediate,
                self.subtract_i64,
                self.subtract_i64_immediate,
                self.compare_i64_zero,
                self.conditional_branch,
                self.return_i64,
                self.return_unit,
            ])
            .collect()
    }

    pub const fn for_semantic(self, semantic: MachineSemanticKind) -> RegisterConstraintKey {
        match semantic {
            MachineSemanticKind::CompareI64Zero => self.compare_i64_zero,
            MachineSemanticKind::MaterializeI64 => self.materialize_i64,
            MachineSemanticKind::CopyI64 => self.copy_i64,
            MachineSemanticKind::ExactAddI64 => self.add_i64,
            MachineSemanticKind::ExactAddI64Immediate => self.add_i64_immediate,
            MachineSemanticKind::ExactSubtractI64 => self.subtract_i64,
            MachineSemanticKind::ExactSubtractI64Immediate => self.subtract_i64_immediate,
            MachineSemanticKind::ConditionalBranchNonZero => self.conditional_branch,
            MachineSemanticKind::ReturnI64 => self.return_i64,
            MachineSemanticKind::ReturnUnit => self.return_unit,
        }
    }
}

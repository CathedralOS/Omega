//! Optimizer module role: executable entrance. Machine-effect catalog admission and identity sealing.
//!
//! Admission binds target and register roots, checks the exact semantic roster,
//! delegates row replay, and seals the independently encoded catalog identity.

mod constraint_keys;
mod identity;
mod validation;

use std::collections::BTreeSet;

use register_model::ValidatedRegisterConstraintCatalog;

use validation::validate_declaration;

use crate::{
    AtomicForm, FloatResultPolicyForm, IeeeFloatArithmeticForm, PlacedScalarForm,
    SaturatingCarrier, SelectedConstraintKeys, TrappingForm,
};
pub use identity::{
    MachineIdentityBytes, alternative_family_tag, encode_machine_alternative_identity,
    encode_machine_alternative_key_identity, encode_machine_encoded_effects_identity,
    machine_effect_catalog_identity, saturating_family_tag, trapping_family_tag,
};
use register_model::{RegisterConstraintCatalogIdentity, RegisterConstraintKey, RegisterViewId};
use target::NativeTarget;

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
        .any(|pair| pair[0].semantic > pair[1].semantic)
    {
        return Err(MachineEffectCatalogValidationError::NonCanonicalDeclarations);
    }
    let expected = catalog.selected_keys.declaration_keys();
    if catalog.declarations.len() != expected.len()
        || catalog
            .declarations
            .iter()
            .zip(&expected)
            .any(|(actual, (semantic, constraint))| {
                actual.semantic != *semantic || actual.constraint != *constraint
            })
    {
        return Err(MachineEffectCatalogValidationError::DeclarationRosterMismatch);
    }
    for declaration in &catalog.declarations {
        let row = register_model::find_register_constraint(
            &constraints.catalog().constraints,
            declaration.constraint,
        )
        .ok_or(MachineEffectCatalogValidationError::UnknownConstraint(
            declaration.semantic,
        ))?;
        validate_declaration(catalog.target.architecture, row, declaration)?;
    }
    let identity = machine_effect_catalog_identity(&catalog);
    Ok(ValidatedMachineEffectCatalog { catalog, identity })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MachineEffectCatalogIdentity([u8; 32]);

impl MachineEffectCatalogIdentity {
    pub(crate) fn from_canonical_bytes(bytes: &[u8]) -> Self {
        Self(crate::digest_encoder::content_identity(&bytes).into())
    }

    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// The `MachineSemanticKind` catalog: a single roster drives the enum and the
/// fixed-entry table so a variant cannot be declared in only one place.
/// `fixed` items are unit variants carried into `FIXED`; `family` items are
/// carrier-scaled variants expanded over that carrier's `ALL`; `dynamic`
/// items are variants whose forms `ALL` appends after the fixed roster.
macro_rules! machine_semantic_kinds {
    (@count fixed $variant:ident) => { 1usize };
    (@count family $variant:ident($carrier:ident)) => { $carrier::ALL.len() };
    (@count dynamic $variant:ident($form:ident)) => { 0usize };
    (@entry ($fixed:ident, $index:ident) fixed $variant:ident) => {{
        $fixed[$index] = Self::$variant;
        $index += 1;
    }};
    (@entry ($fixed:ident, $index:ident) family $variant:ident($carrier:ident)) => {{
        let mut carrier = 0usize;
        while carrier < $carrier::ALL.len() {
            $fixed[$index] = Self::$variant($carrier::ALL[carrier]);
            $index += 1;
            carrier += 1;
        }
    }};
    (@entry ($fixed:ident, $index:ident) dynamic $variant:ident($form:ident)) => {};
    ($( { $(#[$doc:meta])* $kind:ident $variant:ident $(($payload:ident))? } ),* $(,)?) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub enum MachineSemanticKind {
            $( $(#[$doc])* $variant $(($payload))?, )*
        }

        impl MachineSemanticKind {
            #[allow(unused_assignments)]
            const FIXED: [Self; 0 $(+ machine_semantic_kinds!(@count $kind $variant $(($payload))?))*] = {
                let mut fixed = [Self::Crash; 0 $(+ machine_semantic_kinds!(@count $kind $variant $(($payload))?))*];
                let mut index = 0usize;
                $( machine_semantic_kinds!(@entry (fixed, index) $kind $variant $(($payload))?); )*
                fixed
            };
        }
    };
}

machine_semantic_kinds! {
    { fixed Crash },
    { fixed CopyBytes },
    { fixed BitwiseAndI64 },
    { fixed BitwiseXorI64 },
    { fixed CallAggregate },
    { fixed ReturnAggregate },
    { fixed HostedExitProcessI32 },
    { fixed LoadPacked3 },
    { fixed LoadPacked5 },
    { fixed LoadPacked6 },
    { fixed LoadPacked7 },
    { fixed StorePacked },
    { fixed Load8 },
    { fixed Load16 },
    { fixed Load32 },
    { fixed Float32ToBits },
    { fixed Float64ToBits },
    { fixed BitsToFloat32 },
    { fixed BitsToFloat64 },
    { fixed Load8Indexed },
    { fixed CompareI64Zero },
    { fixed MaterializeI64 },
    { fixed CopyI64 },
    { fixed ExactAddI64 },
    { fixed ExactAddI64Immediate },
    { fixed ExactSubtractI64 },
    { fixed ExactDivideU64 },
    { fixed ExactSubtractI64Immediate },
    { fixed ConditionalBranchNonZero },
    { fixed ReturnScalar },
    { fixed ReturnUnit },
    { fixed CompareI64 },
    { fixed CompareI64Immediate },
    { fixed ConditionalBranchU64LessThan },
    { fixed ConditionalBranchI64LessThan },
    { fixed CallScalar },
    { fixed Jump },
    { fixed ZeroExtendU8 },
    { fixed ZeroExtendU32 },
    { fixed ZeroExtendU16 },
    { fixed SignExtendI8 },
    { fixed SignExtendI16 },
    { fixed SignExtendI32 },
    { fixed Load64 },
    { fixed Store64 },
    { fixed FrameAddress },
    { fixed CallUnit },
    { fixed ByteViewAddress },
    { fixed HostedReadByte },
    { fixed HostedWriteByteI32 },
    { fixed Store },
    { fixed AddressOffset },
    { fixed MaterializeBooleanEqual },
    { fixed MaterializeBooleanU64LessThan },
    { fixed MaterializeBooleanI64LessThan },
    { fixed MaterializeBooleanU64LessOrEqual },
    { fixed MaterializeBooleanI64LessOrEqual },
    { fixed WrappingRemainderI64 },
    { fixed WrappingAddI64 },
    { family SaturatingAdd(SaturatingCarrier) },
    { family SaturatingSubtract(SaturatingCarrier) },
    { family SaturatingDivide(SaturatingCarrier) },
    { family SaturatingRemainder(SaturatingCarrier) },
    { fixed NormalizedForeignCall },
    { fixed ExactMultiplyI64 },
    { fixed ExactRemainderU64 },
    { fixed WrappingSubtractI64 },
    { fixed WrappingMultiplyI64 },
    { fixed WrappingDivideI64 },
    { fixed BitwiseOrI64 },
    { fixed BitwiseNotI64 },
    { fixed SaveFloatingControl },
    { fixed RestoreFloatingControl },
    { fixed WrappingShiftLeftI64 },
    { fixed WrappingShiftRightI64 },
    { fixed WrappingShiftRightU64 },
    { fixed ExactShiftLeftI64 },
    { fixed ExactShiftRightI64 },
    { fixed ExactShiftRightU64 },
    { fixed ExactDivideI64 },
    { fixed ExactRemainderI64 },
    { family SaturatingMultiply(SaturatingCarrier) },
    {
        /// PC-relative address of compiler-owned immutable image storage.
        fixed ImmutableImageAddress
    },
    {
        /// Evaluate one byte-sequence predicate over `count` bytes at the base
        /// pointer; the defined operand is the 0/1 verdict.
        fixed ScanByteSequence
    },
    { fixed LoadEncodedScalar },
    { fixed StoreEncodedScalar },
    {
        /// Compare two measured byte spans; the defined operand is the 0/1
        /// verdict, and mismatched counts read no bytes.
        fixed CompareByteSequences
    },
    {
        /// Write zero to `count` bytes at the destination pointer; the
        /// cursor scratch is the only defined operand.
        fixed ZeroBytes
    },
    { fixed PortWrite },
    { fixed SystemRegisterRead },
    { fixed SystemRegisterWrite },
    { fixed PortRegisterRead },
    { fixed PortRegisterWrite },
    { fixed ModelSpecificRead },
    { fixed ModelSpecificWrite },
    { fixed ProcessorHalt },

    { fixed AtomicFence },
    {
        /// One `Trapping` primitive at one carrier: the exact result, or an
        /// in-function architectural trap when the policy predicate holds.
        dynamic TrappingInteger(TrappingForm)
    },
    { dynamic AtomicAccess(AtomicForm) },
    { dynamic NearestIeeeFloatArithmetic(IeeeFloatArithmeticForm) },
    { dynamic FloatResultPolicy(FloatResultPolicyForm) },
    { dynamic PlacedScalarAccess(PlacedScalarForm) },
}

impl MachineSemanticKind {
    /// Every semantic in canonical declaration order: the fixed roster, then
    /// every Trapping form in its dense ordinal (and derived) order.
    pub const ALL: [Self;
        Self::FIXED.len()
            + TrappingForm::COUNT
            + AtomicForm::COUNT
            + IeeeFloatArithmeticForm::COUNT
            + FloatResultPolicyForm::COUNT
            + PlacedScalarForm::COUNT] = {
        let mut all = [Self::Crash;
            Self::FIXED.len()
                + TrappingForm::COUNT
                + AtomicForm::COUNT
                + IeeeFloatArithmeticForm::COUNT
                + FloatResultPolicyForm::COUNT
                + PlacedScalarForm::COUNT];
        let mut index = 0;
        while index < Self::FIXED.len() {
            all[index] = Self::FIXED[index];
            index += 1;
        }
        let mut form = 0;
        while form < TrappingForm::COUNT {
            all[Self::FIXED.len() + form] = Self::TrappingInteger(TrappingForm::ALL[form]);
            form += 1;
        }
        let mut atomic = 0;
        while atomic < AtomicForm::COUNT {
            all[Self::FIXED.len() + TrappingForm::COUNT + atomic] =
                Self::AtomicAccess(AtomicForm::ALL[atomic]);
            atomic += 1;
        }
        let mut float = 0;
        while float < IeeeFloatArithmeticForm::COUNT {
            all[Self::FIXED.len() + TrappingForm::COUNT + AtomicForm::COUNT + float] =
                Self::NearestIeeeFloatArithmetic(IeeeFloatArithmeticForm::ALL[float]);
            float += 1;
        }
        let mut policy = 0;
        while policy < FloatResultPolicyForm::COUNT {
            all[Self::FIXED.len()
                + TrappingForm::COUNT
                + AtomicForm::COUNT
                + IeeeFloatArithmeticForm::COUNT
                + policy] = Self::FloatResultPolicy(FloatResultPolicyForm::ALL[policy]);
            policy += 1;
        }
        let mut placed = 0;
        while placed < PlacedScalarForm::COUNT {
            all[Self::FIXED.len()
                + TrappingForm::COUNT
                + AtomicForm::COUNT
                + IeeeFloatArithmeticForm::COUNT
                + FloatResultPolicyForm::COUNT
                + placed] = Self::PlacedScalarAccess(PlacedScalarForm::ALL[placed]);
            placed += 1;
        }
        all
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MachineAlternativeFamily {
    PortWrite,
    SystemRegisterRead,
    SystemRegisterWrite,
    PortRegisterRead,
    PortRegisterWrite,
    ModelSpecificRead,
    ModelSpecificWrite,
    ProcessorHalt,

    AtomicFence,
    AtomicAccess(AtomicForm),
    PlacedScalarAccess(PlacedScalarForm),
    Crash,
    CopyBytes,
    BitwiseAndI64,
    BitwiseXorI64,
    CallAggregate,
    ReturnAggregate,
    HostedExitProcessI32,
    LoadPacked3,
    LoadPacked5,
    LoadPacked6,
    LoadPacked7,
    StorePacked,
    Load8,
    Load16,
    Load32,
    Float32ToBits,
    Float64ToBits,
    BitsToFloat32,
    BitsToFloat64,
    Load8Indexed,
    CompareI64Zero,
    MaterializeI64,
    CopyI64,
    ExactAddI64,
    ExactAddI64Immediate,
    ExactSubtractI64,
    ExactDivideU64,
    ExactSubtractI64Immediate,
    ConditionalBranchNonZero,
    ReturnScalar,
    ReturnUnit,
    CompareI64,
    CompareI64Immediate,
    ConditionalBranchU64LessThan,
    ConditionalBranchI64LessThan,
    CallScalar,
    Jump,
    ZeroExtendU8,
    ZeroExtendU32,
    ZeroExtendU16,
    SignExtendI8,
    SignExtendI16,
    SignExtendI32,
    Load64,
    Store64,
    FrameAddress,
    CallUnit,
    ByteViewAddress,
    HostedReadByte,
    HostedWriteByteI32,
    Store,
    AddressOffset,
    MaterializeBooleanEqual,
    MaterializeBooleanU64LessThan,
    MaterializeBooleanI64LessThan,
    MaterializeBooleanU64LessOrEqual,
    MaterializeBooleanI64LessOrEqual,
    WrappingRemainderI64,
    WrappingAddI64,
    SaturatingAdd(SaturatingCarrier),
    SaturatingSubtract(SaturatingCarrier),
    SaturatingDivide(SaturatingCarrier),
    SaturatingRemainder(SaturatingCarrier),
    NormalizedForeignCall,
    ExactMultiplyI64,
    ExactRemainderU64,
    WrappingSubtractI64,
    WrappingMultiplyI64,
    WrappingDivideI64,
    BitwiseOrI64,
    BitwiseNotI64,
    SaveFloatingControl,
    RestoreFloatingControl,
    WrappingShiftLeftI64,
    WrappingShiftRightI64,
    WrappingShiftRightU64,
    ExactShiftLeftI64,
    ExactShiftRightI64,
    ExactShiftRightU64,
    ExactDivideI64,
    ExactRemainderI64,
    SaturatingMultiply(SaturatingCarrier),
    ImmutableImageAddress,
    ScanByteSequence,
    LoadEncodedScalar,
    StoreEncodedScalar,
    CompareByteSequences,
    ZeroBytes,
    TrappingInteger(TrappingForm),
    NearestIeeeFloatArithmetic(IeeeFloatArithmeticForm),
    FloatResultPolicy(FloatResultPolicyForm),
}

impl From<MachineSemanticKind> for MachineAlternativeFamily {
    fn from(value: MachineSemanticKind) -> Self {
        match value {
            MachineSemanticKind::SystemRegisterRead => Self::SystemRegisterRead,
            MachineSemanticKind::SystemRegisterWrite => Self::SystemRegisterWrite,
            MachineSemanticKind::PortRegisterRead => Self::PortRegisterRead,
            MachineSemanticKind::PortRegisterWrite => Self::PortRegisterWrite,
            MachineSemanticKind::ModelSpecificRead => Self::ModelSpecificRead,
            MachineSemanticKind::ModelSpecificWrite => Self::ModelSpecificWrite,
            MachineSemanticKind::ProcessorHalt => Self::ProcessorHalt,

            MachineSemanticKind::CopyBytes => Self::CopyBytes,
            MachineSemanticKind::BitwiseAndI64 => Self::BitwiseAndI64,
            MachineSemanticKind::BitwiseXorI64 => Self::BitwiseXorI64,
            MachineSemanticKind::CallAggregate => Self::CallAggregate,
            MachineSemanticKind::ReturnAggregate => Self::ReturnAggregate,
            MachineSemanticKind::Crash => Self::Crash,
            MachineSemanticKind::HostedExitProcessI32 => Self::HostedExitProcessI32,
            MachineSemanticKind::LoadPacked3 => Self::LoadPacked3,
            MachineSemanticKind::LoadPacked5 => Self::LoadPacked5,
            MachineSemanticKind::LoadPacked6 => Self::LoadPacked6,
            MachineSemanticKind::LoadPacked7 => Self::LoadPacked7,
            MachineSemanticKind::StorePacked => Self::StorePacked,
            MachineSemanticKind::Load8 => Self::Load8,
            MachineSemanticKind::Load16 => Self::Load16,
            MachineSemanticKind::Load32 => Self::Load32,
            MachineSemanticKind::Float32ToBits => Self::Float32ToBits,
            MachineSemanticKind::Float64ToBits => Self::Float64ToBits,
            MachineSemanticKind::BitsToFloat32 => Self::BitsToFloat32,
            MachineSemanticKind::BitsToFloat64 => Self::BitsToFloat64,
            MachineSemanticKind::HostedReadByte => Self::HostedReadByte,
            MachineSemanticKind::HostedWriteByteI32 => Self::HostedWriteByteI32,
            MachineSemanticKind::SaveFloatingControl => Self::SaveFloatingControl,
            MachineSemanticKind::RestoreFloatingControl => Self::RestoreFloatingControl,
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
            MachineSemanticKind::ExactDivideU64 => Self::ExactDivideU64,
            MachineSemanticKind::WrappingRemainderI64 => Self::WrappingRemainderI64,
            MachineSemanticKind::WrappingAddI64 => Self::WrappingAddI64,
            MachineSemanticKind::SaturatingAdd(carrier) => Self::SaturatingAdd(carrier),
            MachineSemanticKind::SaturatingSubtract(carrier) => Self::SaturatingSubtract(carrier),
            MachineSemanticKind::SaturatingDivide(carrier) => Self::SaturatingDivide(carrier),
            MachineSemanticKind::SaturatingRemainder(carrier) => Self::SaturatingRemainder(carrier),
            MachineSemanticKind::ExactSubtractI64Immediate => Self::ExactSubtractI64Immediate,
            MachineSemanticKind::ConditionalBranchNonZero => Self::ConditionalBranchNonZero,
            MachineSemanticKind::ReturnScalar => Self::ReturnScalar,
            MachineSemanticKind::ReturnUnit => Self::ReturnUnit,
            MachineSemanticKind::CompareI64 => Self::CompareI64,
            MachineSemanticKind::CompareI64Immediate => Self::CompareI64Immediate,
            MachineSemanticKind::ConditionalBranchU64LessThan => Self::ConditionalBranchU64LessThan,
            MachineSemanticKind::ConditionalBranchI64LessThan => Self::ConditionalBranchI64LessThan,
            MachineSemanticKind::CallScalar => Self::CallScalar,
            MachineSemanticKind::Jump => Self::Jump,
            MachineSemanticKind::ZeroExtendU8 => Self::ZeroExtendU8,
            MachineSemanticKind::ZeroExtendU32 => Self::ZeroExtendU32,
            MachineSemanticKind::ZeroExtendU16 => Self::ZeroExtendU16,
            MachineSemanticKind::SignExtendI8 => Self::SignExtendI8,
            MachineSemanticKind::SignExtendI16 => Self::SignExtendI16,
            MachineSemanticKind::SignExtendI32 => Self::SignExtendI32,
            MachineSemanticKind::MaterializeBooleanEqual => Self::MaterializeBooleanEqual,
            MachineSemanticKind::MaterializeBooleanU64LessThan => {
                Self::MaterializeBooleanU64LessThan
            }
            MachineSemanticKind::MaterializeBooleanI64LessThan => {
                Self::MaterializeBooleanI64LessThan
            }
            MachineSemanticKind::MaterializeBooleanU64LessOrEqual => {
                Self::MaterializeBooleanU64LessOrEqual
            }
            MachineSemanticKind::MaterializeBooleanI64LessOrEqual => {
                Self::MaterializeBooleanI64LessOrEqual
            }

            MachineSemanticKind::Load64 => Self::Load64,
            MachineSemanticKind::Store64 => Self::Store64,
            MachineSemanticKind::FrameAddress => Self::FrameAddress,
            MachineSemanticKind::ImmutableImageAddress => Self::ImmutableImageAddress,
            MachineSemanticKind::ScanByteSequence => Self::ScanByteSequence,
            MachineSemanticKind::LoadEncodedScalar => Self::LoadEncodedScalar,
            MachineSemanticKind::StoreEncodedScalar => Self::StoreEncodedScalar,
            MachineSemanticKind::CompareByteSequences => Self::CompareByteSequences,
            MachineSemanticKind::ZeroBytes => Self::ZeroBytes,
            MachineSemanticKind::CallUnit => Self::CallUnit,
            MachineSemanticKind::NormalizedForeignCall => Self::NormalizedForeignCall,
            MachineSemanticKind::ExactMultiplyI64 => Self::ExactMultiplyI64,
            MachineSemanticKind::ExactRemainderU64 => Self::ExactRemainderU64,
            MachineSemanticKind::WrappingSubtractI64 => Self::WrappingSubtractI64,
            MachineSemanticKind::WrappingMultiplyI64 => Self::WrappingMultiplyI64,
            MachineSemanticKind::WrappingDivideI64 => Self::WrappingDivideI64,
            MachineSemanticKind::BitwiseOrI64 => Self::BitwiseOrI64,
            MachineSemanticKind::BitwiseNotI64 => Self::BitwiseNotI64,
            MachineSemanticKind::WrappingShiftLeftI64 => Self::WrappingShiftLeftI64,
            MachineSemanticKind::WrappingShiftRightI64 => Self::WrappingShiftRightI64,
            MachineSemanticKind::WrappingShiftRightU64 => Self::WrappingShiftRightU64,
            MachineSemanticKind::ExactShiftLeftI64 => Self::ExactShiftLeftI64,
            MachineSemanticKind::ExactShiftRightI64 => Self::ExactShiftRightI64,
            MachineSemanticKind::ExactShiftRightU64 => Self::ExactShiftRightU64,
            MachineSemanticKind::ExactDivideI64 => Self::ExactDivideI64,
            MachineSemanticKind::ExactRemainderI64 => Self::ExactRemainderI64,
            MachineSemanticKind::SaturatingMultiply(carrier) => Self::SaturatingMultiply(carrier),
            MachineSemanticKind::TrappingInteger(form) => Self::TrappingInteger(form),
            MachineSemanticKind::PortWrite => Self::PortWrite,
            MachineSemanticKind::AtomicFence => Self::AtomicFence,
            MachineSemanticKind::AtomicAccess(form) => Self::AtomicAccess(form),
            MachineSemanticKind::PlacedScalarAccess(form) => Self::PlacedScalarAccess(form),
            MachineSemanticKind::FloatResultPolicy(form) => Self::FloatResultPolicy(form),
            MachineSemanticKind::NearestIeeeFloatArithmetic(form) => {
                Self::NearestIeeeFloatArithmetic(form)
            }
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
    /// An alternative the post-allocation legality rule never chooses: only
    /// the named post-allocation optimization, when selected by the build and
    /// executed by the post-allocation stage, rewrites an instruction onto it
    /// after proving the rule's own guard. The catalog declares the form once,
    /// beside the baseline alternatives, so the rule consumes the declared
    /// entry rather than a second name-to-form table.
    OptimizationSelected {
        optimization: optimization_core::Optimization,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MachineMemoryEffect {
    /// Orders coherent memory without accessing a location.
    AtomicFenceV1,
    /// Private aligned storage used and released within the form.
    TemporaryStackStorageV1,
    /// The semantic form owns one indivisible sequentially consistent access.
    AtomicPointerV1,
    /// Reads source and writes destination for the explicit runtime count.
    CopyBytesV1,
    /// Reads `count` bytes at the base pointer while evaluating the predicate.
    ScanByteSequenceV1,
    /// Reads precisely the footprint selected by an admitted scalar storage codec.
    ReadEncodedScalarV1,
    /// Writes precisely that footprint in the original backing.
    WriteEncodedScalarV1,
    /// Reads each span's bytes while comparing them; mismatched counts read
    /// no bytes.
    CompareByteSequencesV1,
    /// Writes zero to `count` bytes at the destination pointer.
    ZeroBytesV1,
    /// Initialize an eight-byte structural home and read stdin into its i32 payload.
    HostedReadByteV1,
    /// Private-byte initialization and kernel read, with an observable stdout write.
    HostedWriteByteV1,
    WritePointerV1,
    NoneV1,
    ReadPointerV1,
    WriteFrameStorageV1,
    ReadFrameStorageV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MachineTrapBehavior {
    ExplicitCrashV1,
    /// The encoded work checks a `Trapping` policy predicate and executes the
    /// `Crash` leaf's architectural trap in place when it holds; otherwise it
    /// falls through with the exact result. It never faults otherwise.
    TrappingIntegerV1,
    HostedExitReturnedV1,
    HostedReadFailureV1,
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
    /// Returning supervisor entry, without a user-space call frame or callee.
    SupervisorNormalReturnV1,
    /// Returning local invocation; the encoded control retains direct versus
    /// indirect selection without changing its data calling convention.
    InternalNormalReturnV1 {
        pre_call_stack_alignment: u16,
    },
    /// Call to an admitted foreign boundary that returns normally. Its encoded
    /// control distinguishes a linker call from named-table dispatch; neither
    /// changes the callee's external status or its ordinary ABI contract.
    ExternalNormalReturnV1 {
        pre_call_stack_alignment: u16,
    },
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
    /// realization are deliberately excluded. Admission requires this list to
    /// restate the constraint row's read contract exactly; the only surfaces
    /// that consume fewer operands are a return — whose operand homes are the
    /// caller's contract, not the encoding's inputs — and an all-aliased
    /// subtract, whose `x - x` result depends on neither input home.
    pub external_operand_reads: Vec<u16>,
    /// Numbered selected operands whose physical homes are written. Admission
    /// requires this list to equal the constraint row's contracted
    /// definitions exactly.
    pub external_operand_writes: Vec<u16>,
    /// Physical register units the encoding reads beyond its operand list.
    /// Admission requires this list to equal the constraint row's implicit
    /// uses exactly, with one named exception: an indirect-register return
    /// honestly reads only its target register and may narrow to a non-empty
    /// subset of the row's uses.
    pub implicit_unit_uses: Vec<register_model::RegisterUnitId>,
    /// Physical register units the encoding defines beyond its operand list.
    /// Admission requires this list to equal the constraint row's implicit
    /// definitions exactly.
    pub implicit_unit_defs: Vec<register_model::RegisterUnitId>,
    /// Physical register units the encoding clobbers. Admission requires
    /// this list to equal the constraint row's declared clobbers exactly.
    pub implicit_unit_clobbers: Vec<register_model::RegisterUnitId>,
    pub memory: MachineEncodedMemoryEffect,
    pub stack: MachineEncodedStackEffect,
    pub trap: MachineEncodedTrapBehavior,
    pub control: MachineEncodedControlEffect,
}

impl MachineEncodedEffects {
    pub fn atomic_fence_v1() -> Self {
        Self {
            memory: MachineEncodedMemoryEffect::AtomicFenceV1,
            ..Self::fallthrough_v1(Vec::new(), Vec::new())
        }
    }

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

    /// A crash alternative's effect row: like a branch it records its site
    /// and transfers control; no caller-owned value or storage is returned
    /// or cleaned up.
    pub fn crash_v1(program_counter: &[register_model::RegisterUnitId]) -> Self {
        Self {
            implicit_unit_uses: program_counter.to_vec(),
            implicit_unit_defs: program_counter.to_vec(),
            trap: MachineEncodedTrapBehavior::ExplicitCrashV1,
            control: MachineEncodedControlEffect::CrashV1,
            ..Self::fallthrough_v1(Vec::new(), Vec::new())
        }
    }

    /// A float result-policy form's effect row: the ISA's scratch clobbers
    /// plus a trap/control pair when the form traps.
    pub fn float_result_policy_v1(
        inputs: u16,
        clobbers: Vec<register_model::RegisterUnitId>,
        trapping: bool,
    ) -> Self {
        let mut effects = Self {
            implicit_unit_clobbers: clobbers,
            ..Self::fallthrough_v1((0..inputs).collect(), vec![inputs])
        };
        if trapping {
            effects.trap = MachineEncodedTrapBehavior::TrappingIntegerV1;
            effects.control = MachineEncodedControlEffect::FallThroughOrTrapV1;
        }
        effects
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MachineEncodedMemoryEffect {
    /// Load one pointer from a named, layout-derived table field. The call's
    /// return-address write remains described by its stack lifecycle.
    ReadTableFieldV1 {
        pointer_operand: u16,
        byte_offset: u32,
    },
    /// Full coherent-memory ordering, independently decoded from barrier bytes.
    AtomicFenceV1,
    /// All accesses are confined to this newly reserved private region.
    TemporaryStackStorageV1 {
        stack_pointer: RegisterViewId,
        byte_count: u16,
    },
    /// Exact atomic width, operation and observed-value contract. Ordinary
    /// pointer read/write effects cannot substitute for this event.
    AtomicPointerV1 {
        pointer_operand: u16,
        form: AtomicForm,
    },
    CopyBytesV1 {
        source_pointer_operand: u16,
        destination_pointer_operand: u16,
        count_operand: u16,
    },
    /// Reads `count` bytes at the base pointer while evaluating the predicate;
    /// the operand homes double as the loop's live inputs.
    ScanByteSequenceV1 {
        base_pointer_operand: u16,
        count_operand: u16,
    },
    /// Read `codec & 255` bytes at the original pointer. The codec operand
    /// retains an admitted immutable runtime-ABI word, not an arbitrary count.
    /// Unknown codec values cannot be treated as a fixed carrier-sized access.
    ReadEncodedScalarV1 {
        pointer_operand: u16,
        codec_operand: u16,
    },
    /// Write the same admitted codec-selected footprint; neither descriptor
    /// storage nor bytes outside the original backing are written.
    WriteEncodedScalarV1 {
        pointer_operand: u16,
        codec_operand: u16,
    },
    /// Reads each span's bytes while comparing them; the four operand homes
    /// double as the compare's live inputs and mismatched counts read none.
    CompareByteSequencesV1 {
        left_base_operand: u16,
        left_count_operand: u16,
        right_base_operand: u16,
        right_count_operand: u16,
    },
    /// Writes zero to `count` bytes at the destination pointer; the operand
    /// homes double as the loop's live inputs and a zero count writes none.
    ZeroBytesV1 {
        destination_pointer_operand: u16,
        count_operand: u16,
    },
    /// Zero eight frame bytes, read stdin into payload at offset four, and set the tag at zero.
    HostedReadByteV1 {
        stack_pointer: RegisterViewId,
    },
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
    ReadFrameStorageV1 {
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
    /// Reserve then release exactly this many bytes. Net displacement is zero;
    /// the region still contributes to the function's peak stack demand.
    TemporaryBytesV1 {
        stack_pointer: RegisterViewId,
        byte_count: u16,
    },
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
    ExplicitCrashV1,
    /// A policy check branches over an inline architectural trap: the trap
    /// executes exactly when the Trapping predicate holds.
    TrappingIntegerV1,
    HostedExitReturnedV1,
    /// Architectural faults remain possible; syscall results other than zero or one trap.
    HostedReadFailureV1,
    /// Architectural faults remain possible; a nonpositive syscall result traps.
    HostedWriteFailureV1,
    NeverV1,
    MayArchitecturalFaultV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MachineEncodedControlEffect {
    CrashV1,
    /// Falls through to the next instruction, or stops at an inline
    /// architectural trap; no other control transfer leaves the form.
    FallThroughOrTrapV1,
    HostedExitOrTrapV1,
    HostedReadReturnOrTrapV1,
    HostedWriteReturnOrTrapV1,
    FallThroughV1,
    /// A software compare-exchange loop retries until an update commits,
    /// then falls through. Each failed attempt is target-relative work with
    /// no static bound: this is not a fixed-work or termination claim. The
    /// successful attempt supplies the observed prior, not its initial load.
    AtomicRetryUntilCommitV1,
    ConditionalRelativeBranchV1,
    ReturnFromActivationStackV1,
    ReturnIndirectRegisterV1 {
        target: RegisterViewId,
    },
    DirectRelativeCallV1,
    /// Invoke an externally admitted address loaded from a named table field.
    IndirectExternalCallV1,
    /// Load an exact immutable table slot; zero traps, otherwise call its address.
    IndirectTableCallOrTrapV1,
    /// Enter the kernel and resume at the following instruction on return.
    SupervisorCallV1,
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
    /// The declared memory footprint the semantic owns. Admission binds this
    /// exactly: a footprint class belongs to the semantics whose encoding
    /// performs it, and every other declaration — including returns and
    /// calls, whose activation-stack lifecycle lives in the encoded stack
    /// and call surfaces — declares `NoneV1`.
    pub memory: MachineMemoryEffect,
    /// The declared trap surface the semantic owns. Admission binds this
    /// exactly: hosted trap results name their owning hosted operation, a
    /// Trapping form declares its policy trap `TrappingIntegerV1`, only a
    /// semantic whose encoded work dereferences memory declares
    /// `MayArchitecturalFaultV1`, and every other rule declares `NeverV1`
    /// even when its ISA-specific encoded trap still admits a fault.
    pub trap: MachineTrapBehavior,
    pub barrier: MachineBarrier,
    pub call: MachineCallEffect,
    /// The cleanup surface the declaration carries. Admission binds it
    /// fail-closed: the vocabulary has one value today, and a second
    /// variant must name its owning semantics before a row may carry it.
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

impl MachineEffectCatalog {
    /// The effect declaration for `constraint`, when the catalog carries it.
    pub fn declaration(
        &self,
        constraint: RegisterConstraintKey,
    ) -> Option<&MachineEffectDeclaration> {
        self.declarations
            .iter()
            .find(|declaration| declaration.constraint == constraint)
    }

    /// The effect declaration for `semantic`, when the catalog carries it.
    pub fn declaration_by_semantic(
        &self,
        semantic: MachineSemanticKind,
    ) -> Option<&MachineEffectDeclaration> {
        self.declarations
            .iter()
            .find(|declaration| declaration.semantic == semantic)
    }

    /// The mutable effect declaration for `semantic`, when the catalog carries it.
    pub fn declaration_by_semantic_mut(
        &mut self,
        semantic: MachineSemanticKind,
    ) -> Option<&mut MachineEffectDeclaration> {
        self.declarations
            .iter_mut()
            .find(|declaration| declaration.semantic == semantic)
    }
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
}

impl std::fmt::Display for MachineEffectCatalogValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid machine-effect catalog: {self:?}")
    }
}

impl std::error::Error for MachineEffectCatalogValidationError {}

/// One ISA-owned catalog build memoized per process: an ISA effect catalog is
/// a pure function of its target and register-constraint identities, so each
/// canonical artifact is built once and shared by every validator.
pub fn canonical_machine_effect_catalog<E>(
    target: NativeTarget,
    constraints: &ValidatedRegisterConstraintCatalog,
    build: impl FnOnce(
        NativeTarget,
        &ValidatedRegisterConstraintCatalog,
    ) -> Result<MachineEffectCatalog, E>,
) -> Result<std::sync::Arc<MachineEffectCatalog>, E> {
    type Canonical = (
        NativeTarget,
        RegisterConstraintCatalogIdentity,
        std::sync::Arc<MachineEffectCatalog>,
    );
    static CANONICAL: std::sync::OnceLock<std::sync::Mutex<Vec<Canonical>>> =
        std::sync::OnceLock::new();
    let cache = CANONICAL.get_or_init(Default::default);
    let identity = constraints.identity();
    if let Some(catalog) = cache.lock().ok().and_then(|cached| {
        cached
            .iter()
            .find(|(seen_target, seen_identity, _)| {
                *seen_target == target && *seen_identity == identity
            })
            .map(|(_, _, catalog)| catalog.clone())
    }) {
        return Ok(catalog);
    }
    let catalog = std::sync::Arc::new(build(target, constraints)?);
    if let Ok(mut cached) = cache.lock() {
        cached.push((target, identity, catalog.clone()));
    }
    Ok(catalog)
}

/// The validation envelope every ISA effect catalog shares: gate the target
/// architecture, structurally validate, then compare to the ISA's canonical
/// build. Each ISA error enum keeps its own variants and maps this outcome.
#[derive(Debug)]
pub enum IsaCatalogValidation<E> {
    ArchitectureMismatch,
    Structural(MachineEffectCatalogValidationError),
    SemanticMismatch,
    Builder(E),
}

/// Validate one ISA-declared effect catalog against its canonical build.
/// `architecture` is the ISA's own `Architecture` variant and `build` its
/// catalog declaration; both stay ISA-owned while the envelope is shared.
pub fn validate_isa_machine_effect_catalog<E>(
    architecture: target::Architecture,
    target: NativeTarget,
    constraints: &ValidatedRegisterConstraintCatalog,
    catalog: MachineEffectCatalog,
    build: impl Fn(NativeTarget, &ValidatedRegisterConstraintCatalog) -> Result<MachineEffectCatalog, E>,
) -> Result<ValidatedMachineEffectCatalog, IsaCatalogValidation<E>> {
    if target.architecture != architecture || constraints.architecture() != architecture {
        return Err(IsaCatalogValidation::ArchitectureMismatch);
    }
    let canonical = canonical_machine_effect_catalog(target, constraints, build)
        .map_err(IsaCatalogValidation::Builder)?;
    let validated = validate_machine_effect_catalog(constraints, catalog)
        .map_err(IsaCatalogValidation::Structural)?;
    if validated.catalog() != &*canonical {
        return Err(IsaCatalogValidation::SemanticMismatch);
    }
    Ok(validated)
}

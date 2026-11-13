/*
  Executable selected instruction forms (`SelectedInstruction`,
  `SelectedInstructionKind`) and closed domain form families (`AccessorScalarForm`,
  `FloatResultPolicyForm`, `IeeeFloatArithmeticForm`, `PackedByteWidth`).

  Each `SelectedInstruction` binds a dense `SelectedInstructionId`, a
  `SelectedInstructionKind`, a `RegisterConstraintKey`, explicit virtual-register
  `SelectedOperand` rows, implicit physical unit uses/defs/clobbers, and
  `SelectedInstructionProvenance`.
*/

use super::SaturatingCarrier;
use super::{MachineAlternativeFamily, MachineAlternativeKey};
use super::{SelectedInstructionId, SelectedInstructionProvenance, SelectedOperand};
use optimization_core::AcceptedObligationFactIdentity;
use register_model::{RegisterConstraintKey, RegisterUnitId};
use semantic_vocabulary::{IntegerValue, MachineId, ObligationId, OperationId, PlaceId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedInstruction {
    pub id: SelectedInstructionId,
    pub kind: SelectedInstructionKind,
    pub constraint: RegisterConstraintKey,
    pub operands: Vec<SelectedOperand>,
    pub implicit_uses: Vec<RegisterUnitId>,
    pub implicit_defs: Vec<RegisterUnitId>,
    pub clobbers: Vec<RegisterUnitId>,
    pub provenance: SelectedInstructionProvenance,
}

pub fn instruction_position(
    instructions: &[SelectedInstruction],
    id: SelectedInstructionId,
) -> Option<usize> {
    instructions
        .iter()
        .position(|instruction| instruction.id == id)
}

impl SelectedInstruction {
    pub fn operand(&self, operand: u16) -> Option<&SelectedOperand> {
        self.operands
            .iter()
            .find(|candidate| candidate.operand == operand)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectedCallTarget {
    Machine(MachineId),
    Dynamic { interface: u32, slot: u32 },
    Callable { contract: semantic_vocabulary::CallableContractId },
}

impl SelectedCallTarget {
    pub const fn from_native(target: target_operations::NativeCallTarget) -> Self {
        match target {
            target_operations::NativeCallTarget::Machine(machine) => Self::Machine(machine),
            target_operations::NativeCallTarget::Dynamic {
                interface, slot, ..
            } => Self::Dynamic { interface, slot },
            target_operations::NativeCallTarget::Callable { contract, .. } => Self::Callable { contract },
        }
    }

    pub const fn machine(self) -> Option<MachineId> {
        match self {
            Self::Machine(machine) => Some(machine),
            Self::Dynamic { .. } | Self::Callable { .. } => None,
        }
    }

    /// Both indirect carriers point at code-address cells. A machine descriptor
    /// has one target-owned cell; a legacy interface selects a cell by slot.
    pub const fn indirect_code_offset(self) -> Option<u64> {
        match self {
            Self::Machine(_) => None,
            Self::Dynamic { slot, .. } => Some(slot as u64 * 8),
            Self::Callable { .. } => Some(0),
        }
    }

    pub fn encode_identity(self, bytes: &mut Vec<u8>) {
        match self {
            Self::Machine(machine) => {
                bytes.push(0);
                crate::codec_bytes::push_encoded(bytes, machine.get().to_le_bytes());
            }
            Self::Callable { contract } => {
                bytes.push(2);
                crate::codec_bytes::push_encoded(bytes, contract.get().to_le_bytes());
            }
            Self::Dynamic { interface, slot } => {
                bytes.push(1);
                crate::codec_bytes::push_encoded(bytes, interface.to_le_bytes());
                crate::codec_bytes::push_encoded(bytes, slot.to_le_bytes());
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectedImageAddressReference {
    PrivateFunction {
        identity: function_identity::MachineFunctionIdentity,
    },
    NativeCodeLayout {
        machine: MachineId,
    },
    RetainedEntryStorageStart,
    ByteLiteral {
        operation: OperationId,
        place: PlaceId,
    },
    ImplementationTable {
        table: u32,
    },
    MachineDescriptor {
        descriptor: semantic_vocabulary::MachineDescriptorId,
    },
}

impl SelectedImageAddressReference {
    pub fn encode_identity(self, bytes: &mut Vec<u8>) {
        match self {
            Self::MachineDescriptor { descriptor } => {
                bytes.push(5);
                crate::codec_bytes::push_encoded(bytes, descriptor.get().to_le_bytes());
            }
            Self::PrivateFunction { identity } => {
                bytes.push(4);
                crate::codec_bytes::push_encoded(bytes, identity.to_bytes());
            }
            Self::RetainedEntryStorageStart => bytes.push(3),
            Self::ByteLiteral { operation, place } => {
                bytes.push(0);
                crate::codec_bytes::push_encoded(bytes, operation.get().to_le_bytes());
                crate::codec_bytes::push_encoded(bytes, place.get().to_le_bytes());
            }
            Self::NativeCodeLayout { machine } => {
                bytes.push(2);
                crate::codec_bytes::push_encoded(bytes, machine.get().to_le_bytes());
            }
            Self::ImplementationTable { table } => {
                bytes.push(1);
                crate::codec_bytes::push_encoded(bytes, table.to_le_bytes());
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectedInstructionKind {
    InvokeNoNormal {
        target: SelectedCallTarget,
    },
    RegisterAccess {
        operation: terminal_psi::RegisterOperation,
    },
    InstalledInterruptCpuCheck {
        gdt_bytes: u32,
    },
    InstalledInterruptTaskCommit {
        gdt_bytes: u32,
    },
    InstalledInterruptTablePublication {
        table_bytes: u32,
    },
    PortWrite {
        port: u16,
        value: u8,
    },
    AccessorScalarAccess {
        form: super::AccessorScalarForm,
        contract: [u8; 32],
        byte_offset: u32,
    },
    AtomicFence,
    AtomicAccess {
        form: super::AtomicForm,
    },
    NearestIeeeFloatArithmetic {
        form: super::IeeeFloatArithmeticForm,
    },
    FloatResultPolicy {
        form: super::FloatResultPolicyForm,
    },
    Crash,
    PrivilegedCrash,
    CopyBytes,
    ZeroBytes,
    LoadEncodedScalar,
    StoreEncodedScalar,
    ScanByteSequence {
        predicate: terminal_psi::ByteSequencePredicate,
    },
    CompareByteSequences,
    LoadPacked {
        byte_offset: u32,
        width: PackedByteWidth,
    },
    StorePacked {
        byte_offset: u32,
        width: PackedByteWidth,
    },
    HostedExitProcessI32,
    HostedReadByte {
        slot: crate::LocalStorageSlotId,
    },
    HostedWriteByteI32 {
        slot: super::LocalStorageSlotId,
    },
    Store {
        byte_offset: u32,
        byte_size: u8,
    },
    AddressOffset {
        byte_offset: u32,
    },
    ByteViewAddress,
    Load8Indexed,
    Load64 {
        byte_offset: u32,
    },
    Load8 {
        byte_offset: u32,
    },
    Load16 {
        byte_offset: u32,
    },
    Load32 {
        byte_offset: u32,
    },
    Store64 {
        slot: super::FrameStorageSlotId,
        byte_offset: u32,
    },
    FrameAddress {
        slot: super::FrameStorageSlotId,
        byte_offset: u32,
    },
    ImageAddress {
        reference: SelectedImageAddressReference,
    },
    CallUnit {
        target: SelectedCallTarget,
    },
    CompareI64Zero,
    CompareI64,
    CompareI64Immediate {
        immediate: IntegerValue,
    },
    MaterializeI64 {
        value: IntegerValue,
    },
    ExactDivideU64 {
        obligation: ObligationId,
        accepted_fact: AcceptedObligationFactIdentity,
    },
    ExactRemainderU64 {
        obligation: ObligationId,
        accepted_fact: AcceptedObligationFactIdentity,
    },
    WrappingRemainderI64 {
        obligation: ObligationId,
        accepted_fact: AcceptedObligationFactIdentity,
    },
    WrappingDivideI64 {
        obligation: ObligationId,
        accepted_fact: AcceptedObligationFactIdentity,
    },
    ExactDivideI64 {
        obligation: ObligationId,
        accepted_fact: AcceptedObligationFactIdentity,
    },
    ExactRemainderI64 {
        obligation: ObligationId,
        accepted_fact: AcceptedObligationFactIdentity,
    },
    SaturatingAdd {
        carrier: SaturatingCarrier,
    },
    SaturatingSubtract {
        carrier: SaturatingCarrier,
    },
    SaturatingDivide {
        carrier: SaturatingCarrier,
        obligation: ObligationId,
        accepted_fact: AcceptedObligationFactIdentity,
    },
    SaturatingRemainder {
        carrier: SaturatingCarrier,
        obligation: ObligationId,
        accepted_fact: AcceptedObligationFactIdentity,
    },
    SaturatingMultiply {
        carrier: SaturatingCarrier,
    },
    TrappingInteger {
        form: super::TrappingForm,
    },
    WrappingAddI64,
    WrappingSubtractI64,
    WrappingMultiplyI64,
    CopyI64,
    BitwiseAndI64,
    BitwiseOrI64,
    BitwiseXorI64,
    BitwiseNotI64,
    WrappingShiftLeftI64,
    WrappingShiftRightI64,
    WrappingShiftRightU64,
    ExactShiftLeftI64 {
        obligation: ObligationId,
        accepted_fact: AcceptedObligationFactIdentity,
    },
    ExactShiftRightI64 {
        obligation: ObligationId,
        accepted_fact: AcceptedObligationFactIdentity,
    },
    ExactShiftRightU64 {
        obligation: ObligationId,
        accepted_fact: AcceptedObligationFactIdentity,
    },
    Float32ToBits,
    Float64ToBits,
    BitsToFloat32,
    BitsToFloat64,
    ZeroExtendU8,
    ZeroExtendU32,
    ZeroExtendU16,
    SignExtendI8,
    SignExtendI16,
    SignExtendI32,
    ExactAddI64 {
        obligation: ObligationId,
        accepted_fact: AcceptedObligationFactIdentity,
    },
    ExactSubtractI64 {
        obligation: ObligationId,
        accepted_fact: AcceptedObligationFactIdentity,
    },
    ExactMultiplyI64 {
        obligation: ObligationId,
        accepted_fact: AcceptedObligationFactIdentity,
    },
    ExactAddI64Immediate {
        immediate: IntegerValue,
        obligation: ObligationId,
        accepted_fact: AcceptedObligationFactIdentity,
    },
    ExactSubtractI64Immediate {
        immediate: IntegerValue,
        obligation: ObligationId,
        accepted_fact: AcceptedObligationFactIdentity,
    },
    ConditionalBranchNonZero,
    ConditionalBranchU64LessThan,
    ConditionalBranchI64LessThan,
    Jump,
    ReturnScalar,
    ReturnAggregate {
        fragment_count: u8,
    },
    ReturnUnit,
    CallScalar {
        target: SelectedCallTarget,
    },
    NormalizedForeignCall {
        boundary: semantic_vocabulary::BoundaryMachineId,
        ordinal: u32,
    },
    CallAggregate {
        target: SelectedCallTarget,
    },
    MaterializeBooleanEqual,
    MaterializeBooleanU64LessThan,
    MaterializeBooleanI64LessThan,
    MaterializeBooleanU64LessOrEqual,
    MaterializeBooleanI64LessOrEqual,
    SaveFloatingControl {
        slot: crate::LocalStorageSlotId,
    },
    RestoreFloatingControl {
        slot: crate::LocalStorageSlotId,
    },
}

impl SelectedInstructionKind {
    pub const fn call_target(self) -> Option<SelectedCallTarget> {
        match self {
            Self::CallScalar { target }
            | Self::CallAggregate { target }
            | Self::CallUnit { target }
            | Self::InvokeNoNormal { target } => Some(target),
            _ => None,
        }
    }

    pub const fn machine_callee(self) -> Option<MachineId> {
        match self {
            Self::InvokeNoNormal {
                target: SelectedCallTarget::Machine(callee),
            }
            | Self::CallScalar {
                target: SelectedCallTarget::Machine(callee),
            }
            | Self::CallAggregate {
                target: SelectedCallTarget::Machine(callee),
            }
            | Self::CallUnit {
                target: SelectedCallTarget::Machine(callee),
            } => Some(callee),
            _ => None,
        }
    }

    pub const fn call_alternative_key(self) -> MachineAlternativeKey {
        MachineAlternativeKey {
            family: match self {
                Self::InvokeNoNormal { .. } => MachineAlternativeFamily::InvokeNoNormal,
                Self::CallUnit { .. } => MachineAlternativeFamily::CallUnit,
                Self::CallAggregate { .. } => MachineAlternativeFamily::CallAggregate,
                Self::NormalizedForeignCall { .. } => {
                    MachineAlternativeFamily::NormalizedForeignCall
                }
                _ => MachineAlternativeFamily::CallScalar,
            },
            variant: 0,
        }
    }
}

#[macro_export]
macro_rules! layout_dependent_selected_transfers {
    () => {
        $crate::SelectedInstructionKind::ConditionalBranchNonZero
            | $crate::SelectedInstructionKind::ConditionalBranchU64LessThan
            | $crate::SelectedInstructionKind::ConditionalBranchI64LessThan
            | $crate::SelectedInstructionKind::Jump
    };
}

#[macro_export]
macro_rules! dedicated_selected_forms {
    () => {
        $crate::SelectedInstructionKind::AccessorScalarAccess { .. }
            | $crate::SelectedInstructionKind::Float32ToBits
            | $crate::SelectedInstructionKind::Float64ToBits
            | $crate::SelectedInstructionKind::BitsToFloat32
            | $crate::SelectedInstructionKind::BitsToFloat64
            | $crate::SelectedInstructionKind::CopyBytes
            | $crate::SelectedInstructionKind::ZeroBytes
            | $crate::SelectedInstructionKind::ScanByteSequence { .. }
            | $crate::SelectedInstructionKind::LoadEncodedScalar
            | $crate::SelectedInstructionKind::StoreEncodedScalar
            | $crate::SelectedInstructionKind::CompareByteSequences
            | $crate::SelectedInstructionKind::Load8Indexed
            | $crate::SelectedInstructionKind::Load8 { .. }
            | $crate::SelectedInstructionKind::Load16 { .. }
            | $crate::SelectedInstructionKind::Load32 { .. }
            | $crate::SelectedInstructionKind::Load64 { .. }
            | $crate::SelectedInstructionKind::LoadPacked { .. }
            | $crate::SelectedInstructionKind::StorePacked { .. }
            | $crate::SelectedInstructionKind::Store { .. }
            | $crate::SelectedInstructionKind::AddressOffset { .. }
            | $crate::SelectedInstructionKind::Store64 { .. }
            | $crate::SelectedInstructionKind::HostedExitProcessI32
            | $crate::SelectedInstructionKind::HostedReadByte { .. }
            | $crate::SelectedInstructionKind::SaveFloatingControl { .. }
            | $crate::SelectedInstructionKind::RestoreFloatingControl { .. }
            | $crate::SelectedInstructionKind::HostedWriteByteI32 { .. }
            | $crate::SelectedInstructionKind::FrameAddress { .. }
            | $crate::SelectedInstructionKind::InvokeNoNormal { .. }
            | $crate::SelectedInstructionKind::CallUnit { .. }
            | $crate::SelectedInstructionKind::CallAggregate { .. }
            | $crate::SelectedInstructionKind::NormalizedForeignCall { .. }
            | $crate::SelectedInstructionKind::CallScalar { .. }
    };
}

#[macro_export]
macro_rules! exact_shift_i64_pair_pattern {
    () => {
        $crate::SelectedInstructionKind::ExactShiftLeftI64 { .. }
            | $crate::SelectedInstructionKind::ExactShiftRightI64 { .. }
    };
}

#[macro_export]
macro_rules! saturating_add_or_subtract_pattern {
    () => {
        $crate::SelectedInstructionKind::SaturatingAdd { .. }
            | $crate::SelectedInstructionKind::SaturatingSubtract { .. }
    };
}

#[macro_export]
macro_rules! load_width_triple_pattern {
    ($($fields:tt)*) => {
        $crate::SelectedInstructionKind::Load8 { $($fields)* }
            | $crate::SelectedInstructionKind::Load16 { $($fields)* }
            | $crate::SelectedInstructionKind::Load32 { $($fields)* }
    };
}

#[macro_export]
macro_rules! selected_call_pattern {
    ($($fields:tt)*) => {
        $crate::SelectedInstructionKind::InvokeNoNormal { $($fields)* }
            | $crate::SelectedInstructionKind::CallAggregate { $($fields)* }
            | $crate::SelectedInstructionKind::CallScalar { $($fields)* }
            | $crate::SelectedInstructionKind::CallUnit { $($fields)* }
    };
}

#[macro_export]
macro_rules! hosted_slot_quad_pattern {
    ($($fields:tt)*) => {
        $crate::SelectedInstructionKind::HostedReadByte { $($fields)* }
            | $crate::SelectedInstructionKind::HostedWriteByteI32 { $($fields)* }
            | $crate::SelectedInstructionKind::RestoreFloatingControl { $($fields)* }
            | $crate::SelectedInstructionKind::SaveFloatingControl { $($fields)* }
    };
}

#[macro_export]
macro_rules! exact_add_or_sub_pair_pattern {
    ($($fields:tt)*) => {
        $crate::SelectedInstructionKind::ExactAddI64Immediate { $($fields)* }
            | $crate::SelectedInstructionKind::ExactSubtractI64Immediate { $($fields)* }
    };
}

#[macro_export]
macro_rules! exact_wrapping_divrem_set_pattern {
    ($($fields:tt)*) => {
        $crate::SelectedInstructionKind::ExactDivideI64 { $($fields)* }
            | $crate::SelectedInstructionKind::ExactRemainderI64 { $($fields)* }
            | $crate::SelectedInstructionKind::ExactRemainderU64 { $($fields)* }
            | $crate::SelectedInstructionKind::WrappingDivideI64 { $($fields)* }
            | $crate::SelectedInstructionKind::WrappingRemainderI64 { $($fields)* }
    };
}

mod accessor_scalar {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct AccessorScalarForm {
        pub write: bool,
        pub bytes: u8,
    }
    impl AccessorScalarForm {
        pub const COUNT: usize = 8;
        pub const ALL: [Self; Self::COUNT] = [
            Self {
                write: false,
                bytes: 1,
            },
            Self {
                write: false,
                bytes: 2,
            },
            Self {
                write: false,
                bytes: 4,
            },
            Self {
                write: false,
                bytes: 8,
            },
            Self {
                write: true,
                bytes: 1,
            },
            Self {
                write: true,
                bytes: 2,
            },
            Self {
                write: true,
                bytes: 4,
            },
            Self {
                write: true,
                bytes: 8,
            },
        ];
        pub const FAMILY_TAG: u8 = 246;
        pub const fn ordinal(self) -> u8 {
            let width = match self.bytes {
                1 => 0,
                2 => 1,
                4 => 2,
                8 => 3,
                _ => return 255,
            };
            (if self.write { 4 } else { 0 }) + width
        }
        pub const fn from_machine_payload(payload: u8) -> Option<Self> {
            if (payload as usize) < Self::COUNT {
                Some(Self::ALL[payload as usize])
            } else {
                None
            }
        }
        pub const fn machine_tag(self) -> u8 {
            Self::FAMILY_TAG
        }
        pub const fn machine_payload(self) -> u8 {
            self.ordinal()
        }
        pub const fn ordinary_semantic(self) -> Option<crate::MachineSemanticKind> {
            use crate::MachineSemanticKind as Kind;
            if self.write {
                if matches!(self.bytes, 1 | 2 | 4 | 8) {
                    Some(Kind::Store)
                } else {
                    None
                }
            } else {
                match self.bytes {
                    1 => Some(Kind::Load8),
                    2 => Some(Kind::Load16),
                    4 => Some(Kind::Load32),
                    8 => Some(Kind::Load64),
                    _ => None,
                }
            }
        }
        pub const fn ordinary_kind(
            self,
            byte_offset: u32,
        ) -> Option<crate::SelectedInstructionKind> {
            use crate::SelectedInstructionKind as Kind;
            if self.write {
                Some(Kind::Store {
                    byte_offset,
                    byte_size: self.bytes,
                })
            } else {
                match self.bytes {
                    1 => Some(Kind::Load8 { byte_offset }),
                    2 => Some(Kind::Load16 { byte_offset }),
                    4 => Some(Kind::Load32 { byte_offset }),
                    8 => Some(Kind::Load64 { byte_offset }),
                    _ => None,
                }
            }
        }
    }
}
pub use accessor_scalar::AccessorScalarForm;

mod float_result_policy {

    use semantic_vocabulary::IeeeFloatFormat;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum FloatResultPolicyKind {
        Trapping,
        Saturating,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct FloatResultPolicyForm {
        pub policy: FloatResultPolicyKind,
        pub format: IeeeFloatFormat,
    }

    impl FloatResultPolicyForm {
        pub const COUNT: usize = 4;
        pub const ALL: [Self; Self::COUNT] = {
            let policies = [
                FloatResultPolicyKind::Trapping,
                FloatResultPolicyKind::Saturating,
            ];
            let formats = [IeeeFloatFormat::Binary32, IeeeFloatFormat::Binary64];
            let mut forms = [Self {
                policy: FloatResultPolicyKind::Trapping,
                format: IeeeFloatFormat::Binary32,
            }; Self::COUNT];
            let mut policy_index = 0;
            while policy_index < policies.len() {
                let mut format_index = 0;
                while format_index < formats.len() {
                    forms[policy_index * formats.len() + format_index] = Self {
                        policy: policies[policy_index],
                        format: formats[format_index],
                    };
                    format_index += 1;
                }
                policy_index += 1;
            }
            forms
        };

        pub const FAMILY_TAG: u8 = 231;

        pub const fn machine_tag(self) -> u8 {
            Self::FAMILY_TAG
        }

        pub const fn ordinal(self) -> u8 {
            let policy = match self.policy {
                FloatResultPolicyKind::Trapping => 0,
                FloatResultPolicyKind::Saturating => 3,
            };
            policy * 2
                + match self.format {
                    IeeeFloatFormat::Binary32 => 0,
                    IeeeFloatFormat::Binary64 => 1,
                }
        }

        pub const fn machine_payload(self) -> u8 {
            self.ordinal()
        }

        pub const fn from_machine_payload(payload: u8) -> Option<Self> {
            match payload {
                0 | 1 => Some(Self::ALL[payload as usize]),
                6 | 7 => Some(Self::ALL[payload as usize - 4]),
                _ => None,
            }
        }

        pub const fn is_binary32(self) -> bool {
            matches!(self.format, IeeeFloatFormat::Binary32)
        }

        pub const fn can_trap(self) -> bool {
            matches!(self.policy, FloatResultPolicyKind::Trapping)
        }

        pub const fn operand_count(self) -> usize {
            match self.policy {
                FloatResultPolicyKind::Trapping => 1,
                FloatResultPolicyKind::Saturating => 2,
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use crate::*;

        #[test]
        fn float_result_policy_forms_have_closed_stable_payloads() {
            assert!(FloatResultPolicyForm::ALL.is_sorted());
            for (payload, form) in [0, 1, 6, 7].into_iter().zip(FloatResultPolicyForm::ALL) {
                assert_eq!(form.machine_payload(), payload);
                assert_eq!(form.machine_tag(), 231);
                assert_eq!(
                    FloatResultPolicyForm::from_machine_payload(payload),
                    Some(form)
                );
                assert_eq!(form.is_binary32(), payload % 2 == 0);
                assert_eq!(form.can_trap(), payload < 2);
                assert_eq!(form.operand_count(), if payload < 2 { 1 } else { 2 });
            }
            for payload in (2..=5).chain(8..=u8::MAX) {
                assert_eq!(FloatResultPolicyForm::from_machine_payload(payload), None);
            }
        }
    }
}
pub use float_result_policy::{FloatResultPolicyForm, FloatResultPolicyKind};

mod ieee_float_arithmetic {

    use crate::MachineAlternativeFamily;
    use crate::SelectedInstructionKind;
    pub use semantic_vocabulary::IeeeFloatFormat;
    pub use target_operations::{
        IeeeFloatArithmeticOperation, IeeeFloatBinaryOperation, IeeeFloatIntegerCarrier,
        IeeeFloatRoundingDirection, IeeeFloatToIntegerDomain,
    };

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct IeeeFloatArithmeticForm {
        pub operation: IeeeFloatArithmeticOperation,
        pub format: IeeeFloatFormat,
    }

    impl IeeeFloatArithmeticForm {
        pub const COUNT: usize = IeeeFloatArithmeticOperation::ALL.len() * 2;
        pub const ALL: [Self; Self::COUNT] = {
            let formats = [IeeeFloatFormat::Binary32, IeeeFloatFormat::Binary64];
            let mut forms = [Self {
                operation: IeeeFloatArithmeticOperation::Add,
                format: IeeeFloatFormat::Binary32,
            }; Self::COUNT];
            let mut operation = 0;
            while operation < IeeeFloatArithmeticOperation::ALL.len() {
                let mut format = 0;
                while format < formats.len() {
                    forms[operation * formats.len() + format] = Self {
                        operation: IeeeFloatArithmeticOperation::ALL[operation],
                        format: formats[format],
                    };
                    format += 1;
                }
                operation += 1;
            }
            forms
        };

        pub const fn ordinal(self) -> u8 {
            let operation = self.operation.ordinal();
            let format = match self.format {
                IeeeFloatFormat::Binary32 => 0,
                IeeeFloatFormat::Binary64 => 1,
            };
            operation * 2 + format
        }

        pub const FAMILY_TAG: u8 = 230;

        pub const fn machine_tag(self) -> u8 {
            Self::FAMILY_TAG
        }

        pub const fn machine_payload(self) -> u8 {
            self.ordinal()
        }

        pub const fn from_machine_payload(payload: u8) -> Option<Self> {
            if (payload as usize) < Self::COUNT {
                Some(Self::ALL[payload as usize])
            } else {
                None
            }
        }

        pub const fn is_binary32(self) -> bool {
            matches!(self.format, IeeeFloatFormat::Binary32)
        }

        pub const fn operand_count(self) -> usize {
            self.operation.operand_count()
        }
    }

    pub fn is_float_bit_transfer(kind: SelectedInstructionKind) -> bool {
        matches!(
            kind,
            SelectedInstructionKind::Float32ToBits
                | SelectedInstructionKind::Float64ToBits
                | SelectedInstructionKind::BitsToFloat32
                | SelectedInstructionKind::BitsToFloat64
        )
    }

    pub fn float_bit_transfer_properties(
        kind: SelectedInstructionKind,
    ) -> Option<(MachineAlternativeFamily, bool, bool)> {
        Some(match kind {
            SelectedInstructionKind::Float32ToBits => {
                (MachineAlternativeFamily::Float32ToBits, true, false)
            }
            SelectedInstructionKind::Float64ToBits => {
                (MachineAlternativeFamily::Float64ToBits, true, true)
            }
            SelectedInstructionKind::BitsToFloat32 => {
                (MachineAlternativeFamily::BitsToFloat32, false, false)
            }
            SelectedInstructionKind::BitsToFloat64 => {
                (MachineAlternativeFamily::BitsToFloat64, false, true)
            }
            _ => return None,
        })
    }

    pub fn ieee_float_arithmetic_is_trapping(form: IeeeFloatArithmeticForm) -> bool {
        matches!(
            form.operation,
            IeeeFloatArithmeticOperation::TruncateToInteger {
                domain: IeeeFloatToIntegerDomain::Trapping,
                ..
            }
        )
    }

    #[cfg(test)]
    mod tests {
        use crate::IeeeFloatArithmeticForm;

        #[test]
        fn forms_are_dense_distinct_and_round_trip_their_tags() {
            for (ordinal, form) in IeeeFloatArithmeticForm::ALL.into_iter().enumerate() {
                assert_eq!(usize::from(form.ordinal()), ordinal);
                assert_eq!(form.machine_tag(), IeeeFloatArithmeticForm::FAMILY_TAG);
                assert_eq!(
                    IeeeFloatArithmeticForm::from_machine_payload(form.machine_payload()),
                    Some(form)
                );
            }
            assert_eq!(
                IeeeFloatArithmeticForm::from_machine_payload(IeeeFloatArithmeticForm::COUNT as u8),
                None
            );
            assert_eq!(IeeeFloatArithmeticForm::ALL.len(), 140);
            for payload in 0_u8..88 {
                let form = IeeeFloatArithmeticForm::from_machine_payload(payload)
                    .expect("every pre-extension machine payload remains decodable");
                assert_eq!(form.machine_payload(), payload);
                assert_eq!(form.operation.ordinal(), payload / 2);
                assert_eq!(form.is_binary32(), payload % 2 == 0);
            }
            assert!(IeeeFloatArithmeticForm::ALL.is_sorted());
        }
    }
}
pub use ieee_float_arithmetic::{
    IeeeFloatArithmeticForm, IeeeFloatArithmeticOperation, IeeeFloatBinaryOperation,
    IeeeFloatFormat, IeeeFloatIntegerCarrier, IeeeFloatRoundingDirection, IeeeFloatToIntegerDomain,
    float_bit_transfer_properties, ieee_float_arithmetic_is_trapping, is_float_bit_transfer,
};

mod packed_byte_width {

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum PackedByteWidth {
        Three,
        Five,
        Six,
        Seven,
    }

    impl PackedByteWidth {
        pub const fn byte_size(self) -> u8 {
            match self {
                Self::Three => 3,
                Self::Five => 5,
                Self::Six => 6,
                Self::Seven => 7,
            }
        }

        pub const fn from_byte_size(bytes: u8) -> Option<Self> {
            match bytes {
                3 => Some(Self::Three),
                5 => Some(Self::Five),
                6 => Some(Self::Six),
                7 => Some(Self::Seven),
                _ => None,
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use crate::PackedByteWidth;
        #[test]
        fn packed_widths_admit_only_exact_odd_fragments() {
            for raw in 0..=u8::MAX {
                let decoded = PackedByteWidth::from_byte_size(raw);
                assert_eq!(decoded.is_some(), matches!(raw, 3 | 5 | 6 | 7));
                if let Some(width) = decoded {
                    assert_eq!(width.byte_size(), raw);
                }
            }
        }
    }
}
pub use packed_byte_width::PackedByteWidth;

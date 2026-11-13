/*
We define our calling-plan and machine-state vocabulary: machine registers and
canonical register sets, value classes (`ValueClass`, `SystemVEightbyteClass`),
value shapes (`ValueShape`) and placement locations (`ValueLocation`,
`ValuePlacement`), call signatures and policies (`CallSignature`,
`CallingPolicy`, `CallPlan`), and machine-state entry contracts (`MachineState`,
`MachineStateSet`, `MachineRegime`, `EntryStack`, `Preemption`, `StatePlan`).
We also provide the canonical byte-stream identity encoding of `StatePlan`
(`encode_state_plan_identity`) using one-based variant tags and little-endian
fields, which stays distinct from `Fnv1a::state_plan`'s zero-based compact
call-plan identity.
*/

use crate::callback_materializations::CallbackMaterialization;
use semantic_vocabulary::{IntegerType, ScalarType};
use target::Architecture;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MachineRegister {
    X86Rax,
    X86Rcx,
    X86Rdx,
    X86Rbx,
    X86Rsp,
    X86Rbp,
    X86Rsi,
    X86Rdi,
    X86R8,
    X86R9,
    X86R10,
    X86R11,
    X86R12,
    X86R13,
    X86R14,
    X86R15,
    X86Xmm(u8),
    Aarch64X(u8),
    Aarch64V(u8),
}

impl MachineRegister {
    pub const fn codec_index(self) -> u8 {
        const BANK_BASE: [u8; 5] = [0, 0, 32, 64, 128];
        const BANK_LANES: [u8; 5] = [1, 16, 16, 32, 32];
        let (class, index) = self.wire_class_index();
        let bank = class as usize;
        BANK_BASE[bank] + index % BANK_LANES[bank]
    }

    pub const fn wire_class_index(self) -> (u8, u8) {
        match self {
            Self::X86Rax => (1, 0),
            Self::X86Rcx => (1, 1),
            Self::X86Rdx => (1, 2),
            Self::X86Rbx => (1, 3),
            Self::X86Rsp => (1, 4),
            Self::X86Rbp => (1, 5),
            Self::X86Rsi => (1, 6),
            Self::X86Rdi => (1, 7),
            Self::X86R8 => (1, 8),
            Self::X86R9 => (1, 9),
            Self::X86R10 => (1, 10),
            Self::X86R11 => (1, 11),
            Self::X86R12 => (1, 12),
            Self::X86R13 => (1, 13),
            Self::X86R14 => (1, 14),
            Self::X86R15 => (1, 15),
            Self::X86Xmm(index) => (2, index),
            Self::Aarch64X(index) => (3, index),
            Self::Aarch64V(index) => (4, index),
        }
    }

    pub fn architecture(self) -> Architecture {
        match self {
            Self::X86Rax
            | Self::X86Rcx
            | Self::X86Rdx
            | Self::X86Rbx
            | Self::X86Rsp
            | Self::X86Rbp
            | Self::X86Rsi
            | Self::X86Rdi
            | Self::X86R8
            | Self::X86R9
            | Self::X86R10
            | Self::X86R11
            | Self::X86R12
            | Self::X86R13
            | Self::X86R14
            | Self::X86R15
            | Self::X86Xmm(_) => Architecture::X86_64,
            Self::Aarch64X(_) | Self::Aarch64V(_) => Architecture::Aarch64,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RegisterSet(Vec<MachineRegister>);

impl RegisterSet {
    pub fn new(registers: impl IntoIterator<Item = MachineRegister>) -> Self {
        let mut registers = registers.into_iter().collect::<Vec<_>>();
        registers.sort_unstable_by_key(|register| register_code(*register));
        registers.dedup();
        Self(registers)
    }

    pub fn as_slice(&self) -> &[MachineRegister] {
        &self.0
    }

    pub fn contains(&self, register: MachineRegister) -> bool {
        self.0
            .binary_search_by_key(&register_code(register), |candidate| {
                register_code(*candidate)
            })
            .is_ok()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemVEightbyteClass {
    Integer,
    Sse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueClass {
    Integer,
    Float,
    BorrowedReference,
    BorrowedScalarReference,
    HomogeneousFloatAggregate {
        members: u8,
    },
    SystemVAggregate {
        first: SystemVEightbyteClass,
        second: SystemVEightbyteClass,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ValueShape {
    pub class: ValueClass,
    pub byte_size: u32,
    pub alignment: u16,
}

impl ValueShape {
    pub fn for_integer(integer: IntegerType) -> Option<Self> {
        if integer.is_address() && integer.bits() == 64 {
            return Some(Self::integer(8, 8));
        }
        if integer.carrier() != semantic_vocabulary::IntegerCarrier::Fixed
            || !matches!(integer.bits(), 8 | 16 | 32 | 64)
        {
            return None;
        }
        let bytes = integer.bits() / 8;
        Some(Self::integer(u32::from(bytes), bytes))
    }

    pub fn for_scalar(scalar_type: ScalarType) -> Option<Self> {
        match scalar_type {
            ScalarType::Callable(_) => Some(Self::integer(8, 8)),
            ScalarType::Boolean => Some(Self::integer(1, 1)),
            ScalarType::IeeeFloat(format) => Some(Self::float(format.byte_size())),
            ScalarType::Integer(integer) => Self::for_integer(integer),
        }
    }

    pub const fn borrowed_scalar_reference() -> Self {
        Self {
            class: ValueClass::BorrowedScalarReference,
            byte_size: 16,
            alignment: 8,
        }
    }

    pub const fn integer(byte_size: u32, alignment: u16) -> Self {
        Self {
            class: ValueClass::Integer,
            byte_size,
            alignment,
        }
    }

    pub const fn float(byte_size: u16) -> Self {
        Self {
            class: ValueClass::Float,
            byte_size: byte_size as u32,
            alignment: byte_size,
        }
    }

    pub const fn borrowed_reference(byte_size: u32, alignment: u16) -> Self {
        Self {
            class: ValueClass::BorrowedReference,
            byte_size,
            alignment,
        }
    }

    pub const fn homogeneous_float_aggregate(member_size: u16, members: u8) -> Self {
        Self {
            class: ValueClass::HomogeneousFloatAggregate { members },
            byte_size: member_size as u32 * members as u32,
            alignment: member_size,
        }
    }

    pub const fn system_v_aggregate(
        byte_size: u32,
        alignment: u16,
        first: SystemVEightbyteClass,
        second: SystemVEightbyteClass,
    ) -> Self {
        Self {
            class: ValueClass::SystemVAggregate { first, second },
            byte_size,
            alignment,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CallSignature {
    pub parameters: Vec<ValueShape>,
    pub result: Option<ValueShape>,
}

impl CallSignature {
    pub fn of_placements(parameters: &[ValuePlacement], result: Option<&ValuePlacement>) -> Self {
        Self {
            parameters: parameters.iter().map(|placement| placement.shape).collect(),
            result: result.map(|placement| placement.shape),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueLocation {
    Register {
        register: MachineRegister,
        value_byte_offset: u32,
        byte_size: u32,
    },
    Stack {
        stack_byte_offset: u32,
        value_byte_offset: u32,
        byte_size: u32,
        alignment: u16,
    },
    Indirect {
        pointer: IndirectPointerLocation,
        copy_stack_byte_offset: Option<u32>,
        byte_size: u32,
        alignment: u16,
    },
}

impl ValueLocation {
    pub const fn register(
        register: MachineRegister,
        value_byte_offset: u32,
        byte_size: u32,
    ) -> Self {
        Self::Register {
            register,
            value_byte_offset,
            byte_size,
        }
    }

    pub const fn stack(
        stack_byte_offset: u32,
        value_byte_offset: u32,
        byte_size: u32,
        alignment: u16,
    ) -> Self {
        Self::Stack {
            stack_byte_offset,
            value_byte_offset,
            byte_size,
            alignment,
        }
    }

    pub const fn indirect(
        pointer: IndirectPointerLocation,
        copy_stack_byte_offset: Option<u32>,
        byte_size: u32,
        alignment: u16,
    ) -> Self {
        Self::Indirect {
            pointer,
            copy_stack_byte_offset,
            byte_size,
            alignment,
        }
    }

    pub fn direct_register(&self) -> Option<MachineRegister> {
        match self {
            Self::Register { register, .. }
            | Self::Indirect {
                pointer: IndirectPointerLocation::Register(register),
                ..
            } => Some(*register),
            Self::Stack { .. }
            | Self::Indirect {
                pointer: IndirectPointerLocation::Stack { .. },
                ..
            } => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndirectPointerLocation {
    Register(MachineRegister),
    Stack {
        stack_byte_offset: u32,
        alignment: u16,
    },
}

pub fn byte_size_at_offset_zero(locations: &[ValueLocation]) -> Option<u32> {
    match locations {
        [
            ValueLocation::Register {
                value_byte_offset: 0,
                byte_size,
                ..
            },
        ]
        | [
            ValueLocation::Stack {
                value_byte_offset: 0,
                byte_size,
                ..
            },
        ] => Some(*byte_size),
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValuePlacement {
    pub shape: ValueShape,
    pub locations: Vec<ValueLocation>,
}

impl ValuePlacement {
    pub fn is_complete_scalar_result(&self) -> bool {
        match self.locations.as_slice() {
            [
                ValueLocation::Register {
                    value_byte_offset: 0,
                    byte_size,
                    ..
                },
            ] => *byte_size == self.shape.byte_size,
            [
                ValueLocation::Indirect {
                    pointer: IndirectPointerLocation::Register(_),
                    copy_stack_byte_offset: None,
                    byte_size,
                    alignment,
                },
            ] => *byte_size == self.shape.byte_size && *alignment == self.shape.alignment,
            _ => false,
        }
    }

    pub fn has_same_parameter_transport(&self, boundary: &Self) -> bool {
        if self == boundary {
            return true;
        }
        if self.shape.class != ValueClass::BorrowedReference
            || boundary.shape != ValueShape::integer(8, 8)
        {
            return false;
        }
        match (self.locations.as_slice(), boundary.locations.as_slice()) {
            (
                [
                    ValueLocation::Indirect {
                        pointer: IndirectPointerLocation::Register(internal),
                        copy_stack_byte_offset: None,
                        ..
                    },
                ],
                [
                    ValueLocation::Register {
                        register: external,
                        value_byte_offset: 0,
                        byte_size: 8,
                    },
                ],
            ) => internal == external,
            (
                [
                    ValueLocation::Indirect {
                        pointer:
                            IndirectPointerLocation::Stack {
                                stack_byte_offset: internal,
                                alignment: internal_alignment,
                            },
                        copy_stack_byte_offset: None,
                        ..
                    },
                ],
                [
                    ValueLocation::Stack {
                        stack_byte_offset: external,
                        value_byte_offset: 0,
                        byte_size: 8,
                        alignment: external_alignment,
                    },
                ],
            ) => internal == external && internal_alignment == external_alignment,
            _ => false,
        }
    }

    pub fn single_word_byte_size(&self) -> Option<u32> {
        byte_size_at_offset_zero(&self.locations)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CallingPolicy {
    MicrosoftX64,
    SystemVAMD64,
    Aapcs64,
    LinuxSyscallX86_64,
    LinuxSyscallAarch64,
}

impl CallingPolicy {
    pub const fn callback_field_scratch(self) -> MachineRegister {
        match self.architecture() {
            Architecture::X86_64 => MachineRegister::X86R11,
            Architecture::Aarch64 => MachineRegister::Aarch64X(16),
        }
    }

    pub const fn callback_fragment_scratch(self) -> &'static [MachineRegister] {
        match self.architecture() {
            Architecture::X86_64 => &[
                MachineRegister::X86R11,
                MachineRegister::X86R10,
                MachineRegister::X86Rax,
            ],
            Architecture::Aarch64 => &[
                MachineRegister::Aarch64X(16),
                MachineRegister::Aarch64X(15),
                MachineRegister::Aarch64X(17),
            ],
        }
    }

    pub const fn architecture(self) -> Architecture {
        match self {
            Self::MicrosoftX64 | Self::SystemVAMD64 | Self::LinuxSyscallX86_64 => {
                Architecture::X86_64
            }
            Self::Aapcs64 | Self::LinuxSyscallAarch64 => Architecture::Aarch64,
        }
    }

    pub const fn for_program_entry_convention(
        convention: target::ProgramEntryCallingConvention,
    ) -> Self {
        match convention {
            target::ProgramEntryCallingConvention::MicrosoftX64 => Self::MicrosoftX64,
            target::ProgramEntryCallingConvention::Aapcs64 => Self::Aapcs64,
            target::ProgramEntryCallingConvention::SystemVAMD64 => Self::SystemVAMD64,
        }
    }

    pub const fn native_for_target(target: target::NativeTarget) -> Self {
        match (target.architecture, target.object_format) {
            (Architecture::X86_64, target::ObjectFormat::Coff) => Self::MicrosoftX64,
            (Architecture::X86_64, target::ObjectFormat::Elf)
            | (Architecture::X86_64, target::ObjectFormat::MachO) => Self::SystemVAMD64,
            (Architecture::Aarch64, target::ObjectFormat::Elf) => Self::Aapcs64,
            (Architecture::Aarch64, target::ObjectFormat::MachO) => Self::Aapcs64,
            (Architecture::Aarch64, target::ObjectFormat::Coff) => {
                panic!(
                    "no native calling policy is declared for this (architecture, object-format) pair"
                )
            }
        }
    }

    pub const fn native_syscall_for_target(target: target::NativeTarget) -> Option<Self> {
        match (target.architecture, target.object_format) {
            (Architecture::X86_64, target::ObjectFormat::Elf) => Some(Self::LinuxSyscallX86_64),
            (Architecture::Aarch64, target::ObjectFormat::Elf) => Some(Self::LinuxSyscallAarch64),
            (Architecture::X86_64, target::ObjectFormat::Coff)
            | (Architecture::X86_64, target::ObjectFormat::MachO)
            | (Architecture::Aarch64, target::ObjectFormat::MachO) => None,
            (Architecture::Aarch64, target::ObjectFormat::Coff) => {
                panic!(
                    "no native syscall policy is declared for this (architecture, object-format) pair"
                )
            }
        }
    }

    pub fn ordinary_clobbers(self) -> RegisterSet {
        match self {
            Self::MicrosoftX64 => RegisterSet::new(
                [
                    MachineRegister::X86Rax,
                    MachineRegister::X86Rcx,
                    MachineRegister::X86Rdx,
                    MachineRegister::X86R8,
                    MachineRegister::X86R9,
                    MachineRegister::X86R10,
                    MachineRegister::X86R11,
                ]
                .into_iter()
                .chain((0..=5).map(MachineRegister::X86Xmm)),
            ),
            Self::SystemVAMD64 => RegisterSet::new(
                [
                    MachineRegister::X86Rax,
                    MachineRegister::X86Rcx,
                    MachineRegister::X86Rdx,
                    MachineRegister::X86Rsi,
                    MachineRegister::X86Rdi,
                    MachineRegister::X86R8,
                    MachineRegister::X86R9,
                    MachineRegister::X86R10,
                    MachineRegister::X86R11,
                ]
                .into_iter()
                .chain((0..=15).map(MachineRegister::X86Xmm)),
            ),
            Self::Aapcs64 => RegisterSet::new(
                (0..=17)
                    .map(MachineRegister::Aarch64X)
                    .chain((0..=7).map(MachineRegister::Aarch64V))
                    .chain((16..=31).map(MachineRegister::Aarch64V)),
            ),
            Self::LinuxSyscallX86_64 => RegisterSet::new([
                MachineRegister::X86Rax,
                MachineRegister::X86Rcx,
                MachineRegister::X86R11,
            ]),
            Self::LinuxSyscallAarch64 => RegisterSet::new(
                (0..=5)
                    .map(MachineRegister::Aarch64X)
                    .chain([MachineRegister::Aarch64X(8)]),
            ),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryControl {
    CallReturn,
    SupervisorCall {
        number_register: MachineRegister,
        immediate: u16,
    },
    InterruptReturn,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallPlan {
    pub policy: CallingPolicy,
    pub parameters: Vec<ValuePlacement>,
    pub result: Option<ValuePlacement>,
    pub callback_materializations: Vec<CallbackMaterialization>,
    pub ordinary_clobbers: RegisterSet,
    pub stack_alignment: u16,
    pub shadow_bytes: u16,
    pub entry_control: EntryControl,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MachineState {
    GeneralRegisters = 0,
    VectorRegisters = 1,
    Flags = 2,
    InstructionPointer = 3,
    StackPointer = 4,
    SegmentState = 5,
    ControlState = 6,
    DebugState = 7,
    ExtendedState = 8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MachineStateSet(pub(crate) u16);

impl MachineStateSet {
    pub const fn empty() -> Self {
        Self(0)
    }

    pub fn new(states: impl IntoIterator<Item = MachineState>) -> Self {
        let mut bits = 0;
        for state in states {
            bits |= 1 << state as u8;
        }
        Self(bits)
    }

    pub const fn bits(self) -> u16 {
        self.0
    }

    pub const fn contains_all(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    pub const fn intersection(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }

    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MachineRegime {
    X86Long64,
    Aarch64A64 { exception_level: u8 },
}

impl MachineRegime {
    pub const fn architecture(self) -> Architecture {
        match self {
            Self::X86Long64 => Architecture::X86_64,
            Self::Aarch64A64 { .. } => Architecture::Aarch64,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum EntryStack {
    Interrupted,
    Dedicated { class: u16 },
    ProviderSelected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Preemption {
    NotApplicable,
    Masked,
    Nestable { maximum_depth: u16 },
    ProviderDefined,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatePlan {
    pub initial_regime: MachineRegime,
    pub interrupted_state: MachineStateSet,
    pub saved_state: MachineStateSet,
    pub restored_state: MachineStateSet,
    pub permitted_transitive_use: MachineStateSet,
    pub stack: EntryStack,
    pub preemption: Preemption,
}

pub fn encode_state_plan_identity(bytes: &mut Vec<u8>, state: &StatePlan) {
    match state.initial_regime {
        MachineRegime::X86Long64 => bytes.push(1),
        MachineRegime::Aarch64A64 { exception_level } => {
            bytes.push(2);
            bytes.push(exception_level);
        }
    }
    for set in [
        state.interrupted_state,
        state.saved_state,
        state.restored_state,
        state.permitted_transitive_use,
    ] {
        crate::codec_bytes::push_encoded(bytes, set.bits().to_le_bytes());
    }
    match state.stack {
        EntryStack::Interrupted => bytes.push(1),
        EntryStack::Dedicated { class } => {
            bytes.push(2);
            crate::codec_bytes::push_encoded(bytes, class.to_le_bytes());
        }
        EntryStack::ProviderSelected => bytes.push(3),
    }
    match state.preemption {
        Preemption::NotApplicable => bytes.push(1),
        Preemption::Masked => bytes.push(2),
        Preemption::Nestable { maximum_depth } => {
            bytes.push(3);
            crate::codec_bytes::push_encoded(bytes, maximum_depth.to_le_bytes());
        }
        Preemption::ProviderDefined => bytes.push(4),
    }
}

pub(crate) const fn system_v_eightbyte_class_code(class: SystemVEightbyteClass) -> u8 {
    match class {
        SystemVEightbyteClass::Integer => 0,
        SystemVEightbyteClass::Sse => 1,
    }
}

pub(crate) const fn register_code(register: MachineRegister) -> u16 {
    let (class, index) = register.wire_class_index();
    ((class as u16 - 1) << 8) | index as u16
}

pub fn gpr_code(register: MachineRegister) -> Option<u8> {
    match register_code(register) {
        0..=15 => Some(register_code(register) as u8),
        _ => None,
    }
}

#[macro_export]
macro_rules! register_or_stack_pair_pattern {
    ($($fields:tt)*) => {
        $crate::ValueLocation::Register { $($fields)* }
            | $crate::ValueLocation::Stack { $($fields)* }
    };
}

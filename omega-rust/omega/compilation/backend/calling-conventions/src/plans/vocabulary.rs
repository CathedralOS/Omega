//! The calling-plan vocabulary: registers and register sets, value classes,
//! shapes and locations, call signatures, policies, entry control, the call
//! plan, machine states and regimes, entry stacks and preemption.

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
    /// Stable ordinal for identity hashing and serialized relocation
    /// records: x86-64 general registers occupy 0–15, XMM lanes 32–47,
    /// AArch64 general registers 64–95, and vector lanes 128–159.
    pub const fn codec_index(self) -> u8 {
        match self {
            Self::X86Rax => 0,
            Self::X86Rcx => 1,
            Self::X86Rdx => 2,
            Self::X86Rbx => 3,
            Self::X86Rsp => 4,
            Self::X86Rbp => 5,
            Self::X86Rsi => 6,
            Self::X86Rdi => 7,
            Self::X86R8 => 8,
            Self::X86R9 => 9,
            Self::X86R10 => 10,
            Self::X86R11 => 11,
            Self::X86R12 => 12,
            Self::X86R13 => 13,
            Self::X86R14 => 14,
            Self::X86R15 => 15,
            Self::X86Xmm(index) => 32 + (index & 15),
            Self::Aarch64X(index) => 64 + (index & 31),
            Self::Aarch64V(index) => 128 + (index & 31),
        }
    }

    /// The wire class/index pair serialized register fields and digests use:
    /// x86-64 general registers are class 1, XMM lanes class 2, AArch64
    /// general registers class 3, and vector lanes class 4.
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
    /// A pointer to caller-owned aggregate storage. `ValueShape::byte_size`
    /// and `alignment` describe the referent, while the call placement carries
    /// exactly one pointer and never allocates a caller-side value copy.
    BorrowedReference,
    /// Internal scalar loan: a pointer to an address-plus-codec descriptor.
    /// Geometry measures the descriptor, never the scalar's semantic carrier.
    /// This is not the thin foreign-pointer ABI.
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
    /// Complete value or borrowed referent extent, not a register fragment.
    /// Keep the same range as native storage coordinates so a small pointer
    /// can borrow a record whose arrays exceed 64 KiB.
    pub byte_size: u32,
    pub alignment: u16,
}

impl ValueShape {
    /// The carried shape of an integer scalar for foreign-call transport: the
    /// retained hosted address model uses a 64-bit integer-class slot, while
    /// its semantic carrier remains an address throughout replay.
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

    /// The carried shape of a scalar for foreign-call transport: fixed-width
    /// integers take the integer-class slot of their byte extent, Booleans one
    /// byte, and IEEE floats their interchange size.
    pub fn for_scalar(scalar_type: ScalarType) -> Option<Self> {
        match scalar_type {
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
    /// The signature of a call boundary whose transports are placements:
    /// each parameter's carried shape in order, plus the result's when the
    /// call returns a value.
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
    /// Fragment resident in the ABI's incoming stack-argument area. This
    /// offset deliberately excludes return addresses and callee prologue
    /// storage; the inbound target encoder adds those target-specific biases.
    Stack {
        stack_byte_offset: u32,
        value_byte_offset: u32,
        byte_size: u32,
        alignment: u16,
    },
    /// A value passed indirectly through a pointer. Parameters larger than the
    /// ABI's direct-value ceiling carry a pointer to a caller-owned stack copy;
    /// large results carry a pointer to their final caller-owned destination
    /// and therefore have no copy slot. The copy offset starts a reserved
    /// interval; over-aligned values round the absolute address upward inside
    /// it. Reservation includes `alignment - stack_alignment` padding when
    /// positive, not just payload bytes.
    Indirect {
        pointer: IndirectPointerLocation,
        copy_stack_byte_offset: Option<u32>,
        byte_size: u32,
        alignment: u16,
    },
}

impl ValueLocation {
    /// The register this location resolves to directly: a `Register`
    /// placement, or an `Indirect` whose pointer lives in a register. Stack
    /// placements and stack-resident indirect pointers resolve to none.
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

/// The byte size of a single placed value whose content starts at offset
/// zero — the `Register`-or-`Stack` shape placement checks everywhere restate
/// as a match arm when they only need that one fact.
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
    /// One complete scalar result, either in a register or in caller-owned
    /// storage addressed by the admitted result pointer. This checks transport
    /// extent; signature admission still establishes the semantic scalar shape.
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

    /// Compare an internal parameter's physical transport with a boundary's.
    /// Borrowed aggregate shapes measure the referent and use an indirect
    /// location, while a foreign signature describes the thin pointer word.
    /// The two carry the same address only at the exact same register or stack
    /// location, with no value copy. Owned indirect aggregates and internal
    /// scalar-loan descriptors are not thin borrowed pointers. This comparison
    /// establishes transport only; the admitted signature owns referent custody.
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

    /// The byte width carried by the placement when it is exactly one
    /// register or stack word starting at value offset zero.
    pub fn single_word_byte_size(&self) -> Option<u32> {
        match self.locations.as_slice() {
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
    /// The architecture-fixed scratch a field-destination materialization
    /// encodes through (`lea`/`adrp`+`add`, then the private-slot store). It
    /// is never an integer-bank parameter register and is already inside the
    /// call's clobber footprint under either supported ABI, so a callback row
    /// declares its units as implicit uses exactly when at least one
    /// destination is a record field.
    pub const fn callback_field_scratch(self) -> MachineRegister {
        match self.architecture() {
            Architecture::X86_64 => MachineRegister::X86R11,
            Architecture::Aarch64 => MachineRegister::Aarch64X(16),
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

    /// The policy a `ProgramEntryCallingConvention` selects: program-entry
    /// spellings name ABI families, not syscall conventions, so the three
    /// variants map one-to-one onto the callable policies.
    pub const fn for_program_entry_convention(
        convention: target::ProgramEntryCallingConvention,
    ) -> Self {
        match convention {
            target::ProgramEntryCallingConvention::MicrosoftX64 => Self::MicrosoftX64,
            target::ProgramEntryCallingConvention::Aapcs64 => Self::Aapcs64,
            target::ProgramEntryCallingConvention::SystemVAMD64 => Self::SystemVAMD64,
        }
    }

    /// The native callable matrix declares each supported (architecture,
    /// object-format) pair instead of defaulting per architecture: a pair
    /// with no declared policy fails closed rather than silently inheriting
    /// an ABI. Adding a supported target extends this matrix deliberately.
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

    /// The syscall callable matrix declares which supported targets carry a
    /// second boundary mechanism beside the C call answered by
    /// [`native_for_target`]: reaching the kernel directly through the Linux
    /// syscall ABI. `Some` selects that policy for direct-syscall boundary
    /// declarations; `None` means the declared pair's boundary is exclusively
    /// C-called (Windows imports, Darwin dyld stubs, UEFI services). Pairs
    /// with no declared syscall row fail closed like `native_for_target`, so
    /// a new supported target extends this matrix deliberately rather than
    /// silently inheriting or silently dropping a syscall mechanism.
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

/// The canonical byte-stream identity encoding of [`StatePlan`].
///
/// The one-based variant tags written here and the little-endian field order
/// that follows them *are* the identity: legalized normalized-foreign-call
/// identities and the register-home fixed-view-copy wire rows that replay them
/// hash exactly these bytes, so a changed tag or a reordered field changes
/// every artifact that carries a boundary-entry state plan. Consumers must
/// call this function rather than repeat the table; a second copy is how two
/// crates that must agree drift apart silently, since both keep compiling and
/// only a mismatched identity ever reveals it.
///
/// This is deliberately *not* the same table as
/// [`super::plan_identity::Fnv1a::state_plan`], which writes zero-based tags
/// into the compact call-plan identity. Both encodings are already ratified;
/// merging them would change one of the two.
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
    match register {
        MachineRegister::X86Rax => 0,
        MachineRegister::X86Rcx => 1,
        MachineRegister::X86Rdx => 2,
        MachineRegister::X86Rbx => 3,
        MachineRegister::X86Rsp => 4,
        MachineRegister::X86Rbp => 5,
        MachineRegister::X86Rsi => 6,
        MachineRegister::X86Rdi => 7,
        MachineRegister::X86R8 => 8,
        MachineRegister::X86R9 => 9,
        MachineRegister::X86R10 => 10,
        MachineRegister::X86R11 => 11,
        MachineRegister::X86R12 => 12,
        MachineRegister::X86R13 => 13,
        MachineRegister::X86R14 => 14,
        MachineRegister::X86R15 => 15,
        MachineRegister::X86Xmm(index) => 0x100 + index as u16,
        MachineRegister::Aarch64X(index) => 0x200 + index as u16,
        MachineRegister::Aarch64V(index) => 0x300 + index as u16,
    }
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

//! Normalized boundary calling and machine-state plans.
//!
//! The existing encoders still realize these policies directly. This module is
//! the semantic seam they are migrating toward: policy + signature produces a
//! deterministic `CallPlan`; inbound roots pair it with a `StatePlan`.
//! Backend footprint evidence is deliberately a different artifact.

use omega_target::Architecture;

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
pub enum ValueClass {
    Integer,
    Float,
    HomogeneousFloatAggregate { members: u8 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ValueShape {
    pub class: ValueClass,
    pub byte_size: u16,
    pub alignment: u16,
}

impl ValueShape {
    pub const fn integer(byte_size: u16, alignment: u16) -> Self {
        Self {
            class: ValueClass::Integer,
            byte_size,
            alignment,
        }
    }

    pub const fn float(byte_size: u16) -> Self {
        Self {
            class: ValueClass::Float,
            byte_size,
            alignment: byte_size,
        }
    }

    pub const fn homogeneous_float_aggregate(member_size: u16, members: u8) -> Self {
        Self {
            class: ValueClass::HomogeneousFloatAggregate { members },
            byte_size: member_size * members as u16,
            alignment: member_size,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CallSignature {
    pub parameters: Vec<ValueShape>,
    pub result: Option<ValueShape>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueLocation {
    Register {
        register: MachineRegister,
        value_byte_offset: u16,
        byte_size: u16,
    },
    /// Fragment resident in the ABI's incoming stack-argument area. This
    /// offset deliberately excludes return addresses and callee prologue
    /// storage; the inbound target encoder adds those target-specific biases.
    Stack {
        stack_byte_offset: u32,
        value_byte_offset: u16,
        byte_size: u16,
        alignment: u16,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValuePlacement {
    pub shape: ValueShape,
    pub locations: Vec<ValueLocation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallingPolicy {
    MicrosoftX64,
    SystemVAMD64,
    Aapcs64,
    LinuxSyscallX86_64,
    LinuxSyscallAarch64,
}

impl CallingPolicy {
    pub const fn architecture(self) -> Architecture {
        match self {
            Self::MicrosoftX64 | Self::SystemVAMD64 | Self::LinuxSyscallX86_64 => {
                Architecture::X86_64
            }
            Self::Aapcs64 | Self::LinuxSyscallAarch64 => Architecture::Aarch64,
        }
    }

    pub const fn native_for_target(target: omega_target::NativeTarget) -> Self {
        match (target.architecture, target.object_format) {
            (Architecture::X86_64, omega_target::ObjectFormat::Coff) => Self::MicrosoftX64,
            (Architecture::X86_64, _) => Self::SystemVAMD64,
            (Architecture::Aarch64, _) => Self::Aapcs64,
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
pub struct MachineStateSet(u16);

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryStack {
    Interrupted,
    Dedicated { class: u16 },
    ProviderSelected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundaryEntryPlan {
    pub call: CallPlan,
    pub state: StatePlan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedBoundaryEntryPlan(BoundaryEntryPlan);

impl ValidatedBoundaryEntryPlan {
    pub fn plan(&self) -> &BoundaryEntryPlan {
        &self.0
    }

    /// Deterministic public contract identity. Validation is represented by
    /// the receiver type and implementation evidence is absent by type.
    pub fn contract_fingerprint(&self) -> u64 {
        let mut hash = Fnv1a::new();
        hash.call_plan(&self.0.call);
        hash.state_plan(&self.0.state);
        hash.finish()
    }
}

/// Implementation evidence. This is intentionally not a field of
/// `BoundaryEntryPlan`: changing allocation or emitted code revalidates the
/// provider artifact without changing the published requirement identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateFootprintEvidence {
    registers: RegisterSet,
    machine_state: MachineStateSet,
}

impl StateFootprintEvidence {
    pub fn new(registers: RegisterSet, additional_machine_state: MachineStateSet) -> Self {
        let register_state = machine_state_for_registers(&registers);
        Self {
            registers,
            machine_state: additional_machine_state.union(register_state),
        }
    }

    pub fn registers(&self) -> &RegisterSet {
        &self.registers
    }

    pub const fn machine_state(&self) -> MachineStateSet {
        self.machine_state
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanDiagnostic(pub String);

impl std::fmt::Display for PlanDiagnostic {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for PlanDiagnostic {}

pub fn evaluate_call_plan(
    policy: CallingPolicy,
    signature: &CallSignature,
) -> Result<CallPlan, PlanDiagnostic> {
    validate_signature_shapes(policy, signature)?;
    let mut plan = match policy {
        CallingPolicy::MicrosoftX64 => evaluate_microsoft_x64(signature)?,
        CallingPolicy::SystemVAMD64 => evaluate_system_v_amd64(signature)?,
        CallingPolicy::Aapcs64 => evaluate_aapcs64(signature)?,
        CallingPolicy::LinuxSyscallX86_64 => evaluate_linux_syscall_x86_64(signature)?,
        CallingPolicy::LinuxSyscallAarch64 => evaluate_linux_syscall_aarch64(signature)?,
    };
    plan.policy = policy;
    validate_call_plan(&plan, signature)?;
    Ok(plan)
}

/// Concrete state policy for ordinary call/return entries, including hosted
/// process roots and firmware handoffs. No interrupted activation exists, so
/// the entry stub owes no save/restore; its transitive state ceiling is exactly
/// the machine-state classes touched by the ABI's ordinary volatile registers.
pub fn evaluate_ordinary_boundary_entry_plan(
    policy: CallingPolicy,
    signature: &CallSignature,
) -> Result<ValidatedBoundaryEntryPlan, PlanDiagnostic> {
    let call = evaluate_call_plan(policy, signature)?;
    let permitted_transitive_use = machine_state_for_registers(&call.ordinary_clobbers);
    let initial_regime = match policy.architecture() {
        Architecture::X86_64 => MachineRegime::X86Long64,
        Architecture::Aarch64 => MachineRegime::Aarch64A64 { exception_level: 0 },
    };
    validate_boundary_entry_plan(
        BoundaryEntryPlan {
            call,
            state: StatePlan {
                initial_regime,
                interrupted_state: MachineStateSet::default(),
                saved_state: MachineStateSet::default(),
                restored_state: MachineStateSet::default(),
                permitted_transitive_use,
                stack: EntryStack::ProviderSelected,
                preemption: Preemption::NotApplicable,
            },
        },
        signature,
    )
}

pub fn validate_boundary_entry_plan(
    plan: BoundaryEntryPlan,
    signature: &CallSignature,
) -> Result<ValidatedBoundaryEntryPlan, PlanDiagnostic> {
    validate_call_plan(&plan.call, signature)?;
    if plan.call.policy.architecture() != plan.state.initial_regime.architecture() {
        return Err(PlanDiagnostic(
            "calling policy and initial machine regime name different architectures".into(),
        ));
    }
    if !plan
        .state
        .interrupted_state
        .contains_all(plan.state.saved_state)
    {
        return Err(PlanDiagnostic(
            "saved machine state is not part of the interrupted state".into(),
        ));
    }
    if !plan
        .state
        .saved_state
        .contains_all(plan.state.restored_state)
        || !plan
            .state
            .restored_state
            .contains_all(plan.state.saved_state)
    {
        return Err(PlanDiagnostic(
            "entry plan must restore exactly the machine state it saves".into(),
        ));
    }
    let endangered = plan
        .state
        .permitted_transitive_use
        .intersection(plan.state.interrupted_state);
    if !plan.state.saved_state.contains_all(endangered) {
        return Err(PlanDiagnostic(
            "permitted transitive machine-state use includes interrupted state the entry stub does not save"
                .into(),
        ));
    }
    let ordinary_clobber_state = machine_state_for_registers(&plan.call.ordinary_clobbers);
    if !plan
        .state
        .permitted_transitive_use
        .contains_all(ordinary_clobber_state)
    {
        return Err(PlanDiagnostic(
            "ordinary call clobbers exceed the entry plan's permitted machine-state ceiling".into(),
        ));
    }
    Ok(ValidatedBoundaryEntryPlan(plan))
}

pub fn validate_state_footprint(
    validated: &ValidatedBoundaryEntryPlan,
    evidence: &StateFootprintEvidence,
) -> Result<(), PlanDiagnostic> {
    let plan = validated.plan();
    for register in evidence.registers().as_slice() {
        if register.architecture() != plan.call.policy.architecture() {
            return Err(PlanDiagnostic(format!(
                "footprint register {register:?} belongs to the wrong architecture"
            )));
        }
    }
    if !plan
        .state
        .permitted_transitive_use
        .contains_all(evidence.machine_state())
    {
        return Err(PlanDiagnostic(
            "emitted machine-state footprint exceeds the entry plan ceiling".into(),
        ));
    }
    let unsaved =
        MachineStateSet(plan.state.interrupted_state.bits() & !plan.state.saved_state.bits());
    if !evidence.machine_state().intersection(unsaved).is_empty() {
        return Err(PlanDiagnostic(
            "emitted footprint clobbers interrupted machine state that is not saved".into(),
        ));
    }
    Ok(())
}

impl StateFootprintEvidence {
    pub fn evidence_fingerprint(&self) -> u64 {
        let mut hash = Fnv1a::new();
        hash.u8(0xe1);
        hash.register_set(self.registers());
        hash.u16(self.machine_state().bits());
        hash.finish()
    }
}

fn validate_signature_shapes(
    policy: CallingPolicy,
    signature: &CallSignature,
) -> Result<(), PlanDiagnostic> {
    for shape in signature.parameters.iter().chain(signature.result.iter()) {
        if shape.byte_size == 0 || shape.alignment == 0 || !shape.alignment.is_power_of_two() {
            return Err(PlanDiagnostic(
                "call-signature values need nonzero size and power-of-two alignment".into(),
            ));
        }
        match shape.class {
            ValueClass::Integer if shape.byte_size > 8 => {
                return Err(PlanDiagnostic(
                    "aggregate integer classification is not normalized yet".into(),
                ));
            }
            ValueClass::Float if !matches!(shape.byte_size, 4 | 8) => {
                return Err(PlanDiagnostic(
                    "scalar floating-point call values must be f32 or f64 sized".into(),
                ));
            }
            ValueClass::HomogeneousFloatAggregate { members }
                if policy != CallingPolicy::Aapcs64
                    || !(1..=4).contains(&members)
                    || shape.byte_size % u16::from(members.max(1)) != 0 =>
            {
                return Err(PlanDiagnostic(
                    "homogeneous float aggregates are currently normalized only for AAPCS64 with one to four equal members"
                        .into(),
                ));
            }
            _ => {}
        }
    }
    Ok(())
}

fn machine_state_for_registers(registers: &RegisterSet) -> MachineStateSet {
    MachineStateSet::new(registers.as_slice().iter().map(|register| match register {
        MachineRegister::X86Xmm(_) | MachineRegister::Aarch64V(_) => MachineState::VectorRegisters,
        _ => MachineState::GeneralRegisters,
    }))
}

pub fn validate_call_plan(
    plan: &CallPlan,
    signature: &CallSignature,
) -> Result<(), PlanDiagnostic> {
    if plan.parameters.len() != signature.parameters.len() {
        return Err(PlanDiagnostic(format!(
            "call plan places {} parameters but the signature declares {}",
            plan.parameters.len(),
            signature.parameters.len()
        )));
    }
    if plan.result.as_ref().map(|value| value.shape) != signature.result {
        return Err(PlanDiagnostic(
            "call-plan result placement does not match the signature".into(),
        ));
    }
    if plan.stack_alignment == 0 || !plan.stack_alignment.is_power_of_two() {
        return Err(PlanDiagnostic(
            "call-plan stack alignment must be a nonzero power of two".into(),
        ));
    }
    let architecture = plan.policy.architecture();
    let mut occupied_registers = RegisterSet::default();
    let mut occupied_stack_ranges = Vec::new();
    for (index, (placement, shape)) in plan
        .parameters
        .iter()
        .zip(signature.parameters.iter())
        .enumerate()
    {
        if placement.shape != *shape {
            return Err(PlanDiagnostic(format!(
                "parameter {index} placement shape does not match the signature"
            )));
        }
        validate_value_placement(placement, architecture, index)?;
        for location in &placement.locations {
            match *location {
                ValueLocation::Register { register, .. } => {
                    if occupied_registers.contains(register) {
                        return Err(PlanDiagnostic(format!(
                            "parameter {index} reuses a register occupied by another parameter"
                        )));
                    }
                    occupied_registers = RegisterSet::new(
                        occupied_registers
                            .as_slice()
                            .iter()
                            .copied()
                            .chain([register]),
                    );
                }
                ValueLocation::Stack {
                    stack_byte_offset,
                    byte_size,
                    ..
                } => {
                    let end = stack_byte_offset + u32::from(byte_size);
                    if occupied_stack_ranges
                        .iter()
                        .any(|(start, prior_end)| stack_byte_offset < *prior_end && *start < end)
                    {
                        return Err(PlanDiagnostic(format!(
                            "parameter {index} overlaps another parameter's stack placement"
                        )));
                    }
                    occupied_stack_ranges.push((stack_byte_offset, end));
                }
            }
        }
    }
    if let Some(result) = &plan.result {
        validate_value_placement(result, architecture, signature.parameters.len())?;
    }
    for register in plan.ordinary_clobbers.as_slice() {
        if register.architecture() != architecture {
            return Err(PlanDiagnostic(
                "ordinary clobber set contains a register from the wrong architecture".into(),
            ));
        }
    }
    if let EntryControl::SupervisorCall {
        number_register, ..
    } = plan.entry_control
        && number_register.architecture() != architecture
    {
        return Err(PlanDiagnostic(
            "entry-control register belongs to the wrong architecture".into(),
        ));
    }
    Ok(())
}

fn validate_value_placement(
    placement: &ValuePlacement,
    architecture: Architecture,
    value_index: usize,
) -> Result<(), PlanDiagnostic> {
    let mut covered = vec![false; usize::from(placement.shape.byte_size)];
    for location in &placement.locations {
        let (value_byte_offset, byte_size) = match *location {
            ValueLocation::Register {
                register,
                value_byte_offset,
                byte_size,
            } => {
                if register.architecture() != architecture {
                    return Err(PlanDiagnostic(format!(
                        "value {value_index} uses a register from the wrong architecture"
                    )));
                }
                (value_byte_offset, byte_size)
            }
            ValueLocation::Stack {
                value_byte_offset,
                byte_size,
                alignment,
                stack_byte_offset,
            } => {
                if alignment == 0
                    || !alignment.is_power_of_two()
                    || stack_byte_offset % u32::from(alignment) != 0
                {
                    return Err(PlanDiagnostic(format!(
                        "value {value_index} has a misaligned stack placement"
                    )));
                }
                (value_byte_offset, byte_size)
            }
        };
        let end = usize::from(value_byte_offset) + usize::from(byte_size);
        if byte_size == 0 || end > covered.len() {
            return Err(PlanDiagnostic(format!(
                "value {value_index} placement exceeds its declared shape"
            )));
        }
        for byte in &mut covered[usize::from(value_byte_offset)..end] {
            if *byte {
                return Err(PlanDiagnostic(format!(
                    "value {value_index} placement writes one source byte more than once"
                )));
            }
            *byte = true;
        }
    }
    if covered.iter().any(|covered| !covered) {
        return Err(PlanDiagnostic(format!(
            "value {value_index} placement does not cover every source byte"
        )));
    }
    Ok(())
}

fn evaluate_microsoft_x64(signature: &CallSignature) -> Result<CallPlan, PlanDiagnostic> {
    let integer = [
        MachineRegister::X86Rcx,
        MachineRegister::X86Rdx,
        MachineRegister::X86R8,
        MachineRegister::X86R9,
    ];
    let mut parameters = Vec::with_capacity(signature.parameters.len());
    for (index, shape) in signature.parameters.iter().copied().enumerate() {
        if matches!(shape.class, ValueClass::HomogeneousFloatAggregate { .. }) {
            return Err(PlanDiagnostic(
                "Microsoft x64 aggregate classification is not normalized yet".into(),
            ));
        }
        let location = if index < 4 {
            let register = if matches!(shape.class, ValueClass::Float) {
                MachineRegister::X86Xmm(index as u8)
            } else {
                integer[index]
            };
            register_location(register, shape)
        } else {
            stack_location(32 + ((index - 4) * 8) as u32, shape)
        };
        parameters.push(ValuePlacement {
            shape,
            locations: vec![location],
        });
    }
    Ok(CallPlan {
        policy: CallingPolicy::MicrosoftX64,
        parameters,
        result: result_placement(signature.result, MachineRegister::X86Rax, |index| {
            MachineRegister::X86Xmm(index)
        })?,
        ordinary_clobbers: RegisterSet::new(
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
        stack_alignment: 16,
        shadow_bytes: 32,
        entry_control: EntryControl::CallReturn,
    })
}

fn evaluate_system_v_amd64(signature: &CallSignature) -> Result<CallPlan, PlanDiagnostic> {
    let integer = [
        MachineRegister::X86Rdi,
        MachineRegister::X86Rsi,
        MachineRegister::X86Rdx,
        MachineRegister::X86Rcx,
        MachineRegister::X86R8,
        MachineRegister::X86R9,
    ];
    evaluate_split_bank_call(
        CallingPolicy::SystemVAMD64,
        signature,
        &integer,
        8,
        MachineRegister::X86Xmm,
        MachineRegister::X86Rax,
        16,
        RegisterSet::new(
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
    )
}

fn evaluate_aapcs64(signature: &CallSignature) -> Result<CallPlan, PlanDiagnostic> {
    let integer = (0..8).map(MachineRegister::Aarch64X).collect::<Vec<_>>();
    evaluate_split_bank_call(
        CallingPolicy::Aapcs64,
        signature,
        &integer,
        8,
        MachineRegister::Aarch64V,
        MachineRegister::Aarch64X(0),
        16,
        RegisterSet::new(
            (0..=17)
                .map(MachineRegister::Aarch64X)
                .chain((0..=7).map(MachineRegister::Aarch64V))
                .chain((16..=31).map(MachineRegister::Aarch64V)),
        ),
    )
}

fn evaluate_split_bank_call(
    policy: CallingPolicy,
    signature: &CallSignature,
    integer_registers: &[MachineRegister],
    float_register_count: u8,
    float_register: impl Fn(u8) -> MachineRegister + Copy,
    integer_result: MachineRegister,
    stack_alignment: u16,
    ordinary_clobbers: RegisterSet,
) -> Result<CallPlan, PlanDiagnostic> {
    let mut integer_index = 0usize;
    let mut float_index = 0u8;
    let mut stack_offset = 0u32;
    let mut parameters = Vec::with_capacity(signature.parameters.len());
    for shape in signature.parameters.iter().copied() {
        let mut locations = Vec::new();
        let float_members = match shape.class {
            ValueClass::Float => Some(1),
            ValueClass::HomogeneousFloatAggregate { members } => Some(members),
            ValueClass::Integer => None,
        };
        if let Some(members) = float_members
            && float_index.saturating_add(members) <= float_register_count
        {
            let member_size = shape.byte_size / u16::from(members);
            for member in 0..members {
                locations.push(ValueLocation::Register {
                    register: float_register(float_index + member),
                    value_byte_offset: u16::from(member) * member_size,
                    byte_size: member_size,
                });
            }
            float_index += members;
        } else if float_members.is_none() && integer_index < integer_registers.len() {
            locations.push(register_location(integer_registers[integer_index], shape));
            integer_index += 1;
        } else {
            stack_offset = align_up(stack_offset, u32::from(shape.alignment));
            locations.push(stack_location(stack_offset, shape));
            stack_offset += u32::from(shape.byte_size.max(8));
        }
        parameters.push(ValuePlacement { shape, locations });
    }
    Ok(CallPlan {
        policy,
        parameters,
        result: result_placement(signature.result, integer_result, float_register)?,
        ordinary_clobbers,
        stack_alignment,
        shadow_bytes: 0,
        entry_control: EntryControl::CallReturn,
    })
}

fn evaluate_linux_syscall_x86_64(signature: &CallSignature) -> Result<CallPlan, PlanDiagnostic> {
    let registers = [
        MachineRegister::X86Rdi,
        MachineRegister::X86Rsi,
        MachineRegister::X86Rdx,
        MachineRegister::X86R10,
        MachineRegister::X86R8,
        MachineRegister::X86R9,
    ];
    evaluate_syscall(
        CallingPolicy::LinuxSyscallX86_64,
        signature,
        &registers,
        MachineRegister::X86Rax,
        MachineRegister::X86Rax,
        RegisterSet::new([
            MachineRegister::X86Rax,
            MachineRegister::X86Rcx,
            MachineRegister::X86R11,
        ]),
    )
}

fn evaluate_linux_syscall_aarch64(signature: &CallSignature) -> Result<CallPlan, PlanDiagnostic> {
    let registers = (0..6).map(MachineRegister::Aarch64X).collect::<Vec<_>>();
    evaluate_syscall(
        CallingPolicy::LinuxSyscallAarch64,
        signature,
        &registers,
        MachineRegister::Aarch64X(8),
        MachineRegister::Aarch64X(0),
        RegisterSet::new(
            (0..=5)
                .map(MachineRegister::Aarch64X)
                .chain([MachineRegister::Aarch64X(8)]),
        ),
    )
}

fn evaluate_syscall(
    policy: CallingPolicy,
    signature: &CallSignature,
    registers: &[MachineRegister],
    number_register: MachineRegister,
    result_register: MachineRegister,
    ordinary_clobbers: RegisterSet,
) -> Result<CallPlan, PlanDiagnostic> {
    if signature.parameters.len() > registers.len() {
        return Err(PlanDiagnostic(format!(
            "{policy:?} admits at most {} parameters",
            registers.len()
        )));
    }
    if signature
        .parameters
        .iter()
        .any(|shape| !matches!(shape.class, ValueClass::Integer) || shape.byte_size > 8)
    {
        return Err(PlanDiagnostic(
            "the normalized Linux syscall plans currently admit only integer/pointer values up to 8 bytes"
                .into(),
        ));
    }
    let parameters = signature
        .parameters
        .iter()
        .copied()
        .zip(registers.iter().copied())
        .map(|(shape, register)| ValuePlacement {
            shape,
            locations: vec![register_location(register, shape)],
        })
        .collect();
    let result = match signature.result {
        Some(shape) if matches!(shape.class, ValueClass::Integer) && shape.byte_size <= 8 => {
            Some(ValuePlacement {
                shape,
                locations: vec![register_location(result_register, shape)],
            })
        }
        Some(_) => {
            return Err(PlanDiagnostic(
                "the normalized Linux syscall result must be an integer/pointer value up to 8 bytes"
                    .into(),
            ));
        }
        None => None,
    };
    Ok(CallPlan {
        policy,
        parameters,
        result,
        ordinary_clobbers,
        stack_alignment: 16,
        shadow_bytes: 0,
        entry_control: EntryControl::SupervisorCall {
            number_register,
            immediate: 0,
        },
    })
}

fn result_placement(
    result: Option<ValueShape>,
    integer_register: MachineRegister,
    float_register: impl Fn(u8) -> MachineRegister,
) -> Result<Option<ValuePlacement>, PlanDiagnostic> {
    let Some(shape) = result else {
        return Ok(None);
    };
    let locations = match shape.class {
        ValueClass::Integer => vec![register_location(integer_register, shape)],
        ValueClass::Float => vec![register_location(float_register(0), shape)],
        ValueClass::HomogeneousFloatAggregate { members } => {
            let member_size = shape.byte_size / u16::from(members);
            (0..members)
                .map(|member| ValueLocation::Register {
                    register: float_register(member),
                    value_byte_offset: u16::from(member) * member_size,
                    byte_size: member_size,
                })
                .collect()
        }
    };
    Ok(Some(ValuePlacement { shape, locations }))
}

fn register_location(register: MachineRegister, shape: ValueShape) -> ValueLocation {
    ValueLocation::Register {
        register,
        value_byte_offset: 0,
        byte_size: shape.byte_size,
    }
}

fn stack_location(stack_byte_offset: u32, shape: ValueShape) -> ValueLocation {
    ValueLocation::Stack {
        stack_byte_offset,
        value_byte_offset: 0,
        byte_size: shape.byte_size,
        alignment: shape.alignment.min(16),
    }
}

fn align_up(value: u32, alignment: u32) -> u32 {
    let mask = alignment - 1;
    (value + mask) & !mask
}

struct Fnv1a(u64);

impl Fnv1a {
    const fn new() -> Self {
        Self(0xcbf29ce484222325)
    }

    const fn finish(self) -> u64 {
        self.0
    }

    fn bytes(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 ^= u64::from(*byte);
            self.0 = self.0.wrapping_mul(0x100000001b3);
        }
    }

    fn u8(&mut self, value: u8) {
        self.bytes(&[value]);
    }

    fn u16(&mut self, value: u16) {
        self.bytes(&value.to_le_bytes());
    }

    fn u32(&mut self, value: u32) {
        self.bytes(&value.to_le_bytes());
    }

    fn call_plan(&mut self, plan: &CallPlan) {
        self.u8(match plan.policy {
            CallingPolicy::MicrosoftX64 => 0,
            CallingPolicy::SystemVAMD64 => 1,
            CallingPolicy::Aapcs64 => 2,
            CallingPolicy::LinuxSyscallX86_64 => 3,
            CallingPolicy::LinuxSyscallAarch64 => 4,
        });
        self.u16(plan.stack_alignment);
        self.u16(plan.shadow_bytes);
        self.entry_control(plan.entry_control);
        self.u32(plan.parameters.len() as u32);
        for parameter in &plan.parameters {
            self.value_placement(parameter);
        }
        match &plan.result {
            Some(result) => {
                self.u8(1);
                self.value_placement(result);
            }
            None => self.u8(0),
        }
        self.register_set(&plan.ordinary_clobbers);
    }

    fn state_plan(&mut self, plan: &StatePlan) {
        match plan.initial_regime {
            MachineRegime::X86Long64 => self.u8(0),
            MachineRegime::Aarch64A64 { exception_level } => {
                self.u8(1);
                self.u8(exception_level);
            }
        }
        self.u16(plan.interrupted_state.bits());
        self.u16(plan.saved_state.bits());
        self.u16(plan.restored_state.bits());
        self.u16(plan.permitted_transitive_use.bits());
        match plan.stack {
            EntryStack::Interrupted => self.u8(0),
            EntryStack::Dedicated { class } => {
                self.u8(1);
                self.u16(class);
            }
            EntryStack::ProviderSelected => self.u8(2),
        }
        match plan.preemption {
            Preemption::NotApplicable => self.u8(0),
            Preemption::Masked => self.u8(1),
            Preemption::Nestable { maximum_depth } => {
                self.u8(2);
                self.u16(maximum_depth);
            }
            Preemption::ProviderDefined => self.u8(3),
        }
    }

    fn entry_control(&mut self, control: EntryControl) {
        match control {
            EntryControl::CallReturn => self.u8(0),
            EntryControl::SupervisorCall {
                number_register,
                immediate,
            } => {
                self.u8(1);
                self.register(number_register);
                self.u16(immediate);
            }
            EntryControl::InterruptReturn => self.u8(2),
        }
    }

    fn value_placement(&mut self, placement: &ValuePlacement) {
        self.value_shape(placement.shape);
        self.u32(placement.locations.len() as u32);
        for location in &placement.locations {
            match *location {
                ValueLocation::Register {
                    register,
                    value_byte_offset,
                    byte_size,
                } => {
                    self.u8(0);
                    self.register(register);
                    self.u16(value_byte_offset);
                    self.u16(byte_size);
                }
                ValueLocation::Stack {
                    stack_byte_offset,
                    value_byte_offset,
                    byte_size,
                    alignment,
                } => {
                    self.u8(1);
                    self.u32(stack_byte_offset);
                    self.u16(value_byte_offset);
                    self.u16(byte_size);
                    self.u16(alignment);
                }
            }
        }
    }

    fn value_shape(&mut self, shape: ValueShape) {
        match shape.class {
            ValueClass::Integer => self.u8(0),
            ValueClass::Float => self.u8(1),
            ValueClass::HomogeneousFloatAggregate { members } => {
                self.u8(2);
                self.u8(members);
            }
        }
        self.u16(shape.byte_size);
        self.u16(shape.alignment);
    }

    fn register_set(&mut self, registers: &RegisterSet) {
        self.u32(registers.as_slice().len() as u32);
        for register in registers.as_slice() {
            self.register(*register);
        }
    }

    fn register(&mut self, register: MachineRegister) {
        let code = register_code(register);
        self.u8((code >> 8) as u8);
        self.u8(code as u8);
    }
}

const fn register_code(register: MachineRegister) -> u16 {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn integer_signature(parameter_count: usize) -> CallSignature {
        CallSignature {
            parameters: vec![ValueShape::integer(8, 8); parameter_count],
            result: Some(ValueShape::integer(8, 8)),
        }
    }

    #[test]
    fn microsoft_x64_places_four_registers_then_shadow_relative_stack() {
        let plan = evaluate_call_plan(CallingPolicy::MicrosoftX64, &integer_signature(6))
            .expect("MS x64 plan");
        assert!(matches!(
            plan.parameters[0].locations[0],
            ValueLocation::Register {
                register: MachineRegister::X86Rcx,
                ..
            }
        ));
        assert!(matches!(
            plan.parameters[4].locations[0],
            ValueLocation::Stack {
                stack_byte_offset: 32,
                ..
            }
        ));
        assert_eq!(plan.shadow_bytes, 32);
    }

    #[test]
    fn system_v_and_aapcs_use_independent_float_register_banks() {
        let signature = CallSignature {
            parameters: vec![
                ValueShape::integer(8, 8),
                ValueShape::float(8),
                ValueShape::integer(8, 8),
                ValueShape::float(8),
            ],
            result: None,
        };
        let sysv = evaluate_call_plan(CallingPolicy::SystemVAMD64, &signature).expect("SysV");
        let aapcs = evaluate_call_plan(CallingPolicy::Aapcs64, &signature).expect("AAPCS");
        assert!(matches!(
            sysv.parameters[1].locations[0],
            ValueLocation::Register {
                register: MachineRegister::X86Xmm(0),
                ..
            }
        ));
        assert!(matches!(
            aapcs.parameters[3].locations[0],
            ValueLocation::Register {
                register: MachineRegister::Aarch64V(1),
                ..
            }
        ));
    }

    #[test]
    fn aapcs_hfa_is_one_value_split_across_vector_registers() {
        let signature = CallSignature {
            parameters: vec![ValueShape::homogeneous_float_aggregate(8, 4)],
            result: None,
        };
        let plan = evaluate_call_plan(CallingPolicy::Aapcs64, &signature).expect("AAPCS HFA");
        assert_eq!(plan.parameters[0].locations.len(), 4);
        assert!(matches!(
            plan.parameters[0].locations[3],
            ValueLocation::Register {
                register: MachineRegister::Aarch64V(3),
                value_byte_offset: 24,
                ..
            }
        ));
    }

    #[test]
    fn linux_syscall_pins_non_c_call_registers() {
        let plan = evaluate_call_plan(CallingPolicy::LinuxSyscallX86_64, &integer_signature(6))
            .expect("Linux syscall");
        assert!(matches!(
            plan.parameters[3].locations[0],
            ValueLocation::Register {
                register: MachineRegister::X86R10,
                ..
            }
        ));
        assert!(matches!(
            plan.entry_control,
            EntryControl::SupervisorCall {
                number_register: MachineRegister::X86Rax,
                immediate: 0,
            }
        ));

        let aarch64 = evaluate_call_plan(CallingPolicy::LinuxSyscallAarch64, &integer_signature(6))
            .expect("AArch64 Linux syscall");
        assert!(
            aarch64
                .ordinary_clobbers
                .contains(MachineRegister::Aarch64X(8)),
            "the number register is part of the realized syscall clobber set"
        );
    }

    fn strict_x86_entry() -> BoundaryEntryPlan {
        let mut call = evaluate_call_plan(CallingPolicy::MicrosoftX64, &integer_signature(1))
            .expect("call plan");
        call.ordinary_clobbers = RegisterSet::new([
            MachineRegister::X86Rax,
            MachineRegister::X86Rcx,
            MachineRegister::X86Rdx,
            MachineRegister::X86R8,
            MachineRegister::X86R9,
            MachineRegister::X86R10,
            MachineRegister::X86R11,
        ]);
        call.entry_control = EntryControl::InterruptReturn;
        let interrupted = MachineStateSet::new([
            MachineState::GeneralRegisters,
            MachineState::Flags,
            MachineState::InstructionPointer,
            MachineState::StackPointer,
            MachineState::VectorRegisters,
        ]);
        let saved = MachineStateSet::new([
            MachineState::GeneralRegisters,
            MachineState::Flags,
            MachineState::InstructionPointer,
            MachineState::StackPointer,
        ]);
        BoundaryEntryPlan {
            call,
            state: StatePlan {
                initial_regime: MachineRegime::X86Long64,
                interrupted_state: interrupted,
                saved_state: saved,
                restored_state: saved,
                permitted_transitive_use: MachineStateSet::new([
                    MachineState::GeneralRegisters,
                    MachineState::Flags,
                ]),
                stack: EntryStack::Dedicated { class: 1 },
                preemption: Preemption::Masked,
            },
        }
    }

    #[test]
    fn ordinary_firmware_entry_has_no_interrupted_state_obligation() {
        let signature = integer_signature(2);
        let validated =
            evaluate_ordinary_boundary_entry_plan(CallingPolicy::MicrosoftX64, &signature)
                .expect("ordinary Microsoft x64 boundary entry");
        let plan = validated.plan();

        assert_eq!(plan.state.initial_regime, MachineRegime::X86Long64);
        assert!(plan.state.interrupted_state.is_empty());
        assert!(plan.state.saved_state.is_empty());
        assert!(plan.state.restored_state.is_empty());
        assert_eq!(plan.state.stack, EntryStack::ProviderSelected);
        assert_eq!(plan.state.preemption, Preemption::NotApplicable);
        assert!(
            plan.state
                .permitted_transitive_use
                .contains_all(MachineStateSet::new([MachineState::GeneralRegisters]))
        );
        assert!(
            plan.state
                .permitted_transitive_use
                .contains_all(MachineStateSet::new([MachineState::VectorRegisters]))
        );
    }

    #[test]
    fn boundary_entry_validation_rejects_unsaved_permitted_state() {
        let mut plan = strict_x86_entry();
        validate_boundary_entry_plan(plan.clone(), &integer_signature(1))
            .expect("strict plan is coherent");
        plan.state.permitted_transitive_use = plan
            .state
            .permitted_transitive_use
            .union(MachineStateSet::new([MachineState::VectorRegisters]));
        let error = validate_boundary_entry_plan(plan, &integer_signature(1))
            .expect_err("SIMD is not saved");
        assert!(error.0.contains("does not save"));
    }

    #[test]
    fn evidence_is_validated_but_firewalled_from_contract_identity() {
        let plan = strict_x86_entry();
        let validated =
            validate_boundary_entry_plan(plan, &integer_signature(1)).expect("entry plan");
        let identity = validated.contract_fingerprint();
        let evidence_a = StateFootprintEvidence::new(
            RegisterSet::new([MachineRegister::X86Rax]),
            MachineStateSet::new([MachineState::GeneralRegisters]),
        );
        let evidence_b = StateFootprintEvidence::new(
            RegisterSet::new([MachineRegister::X86Rax, MachineRegister::X86Rcx]),
            MachineStateSet::new([MachineState::GeneralRegisters, MachineState::Flags]),
        );
        validate_state_footprint(&validated, &evidence_a).expect("first footprint");
        validate_state_footprint(&validated, &evidence_b).expect("second footprint");
        assert_ne!(
            evidence_a.evidence_fingerprint(),
            evidence_b.evidence_fingerprint()
        );
        assert_eq!(identity, validated.contract_fingerprint());
    }

    #[test]
    fn evaluated_state_plan_changes_contract_identity() {
        let first = strict_x86_entry();
        let mut second = first.clone();
        second.state.stack = EntryStack::Dedicated { class: 2 };
        let first =
            validate_boundary_entry_plan(first, &integer_signature(1)).expect("first entry plan");
        let second =
            validate_boundary_entry_plan(second, &integer_signature(1)).expect("second entry plan");
        assert_ne!(first.contract_fingerprint(), second.contract_fingerprint());
    }

    #[test]
    fn register_sets_normalize_order_and_duplicates() {
        let first = RegisterSet::new([
            MachineRegister::X86R11,
            MachineRegister::X86Rax,
            MachineRegister::X86R11,
        ]);
        let second = RegisterSet::new([MachineRegister::X86Rax, MachineRegister::X86R11]);
        assert_eq!(first, second);
    }

    #[test]
    fn register_use_derives_machine_state_and_cannot_be_hidden() {
        let plan = strict_x86_entry();
        let validated =
            validate_boundary_entry_plan(plan, &integer_signature(1)).expect("entry plan");
        let evidence = StateFootprintEvidence::new(
            RegisterSet::new([MachineRegister::X86Xmm(0)]),
            MachineStateSet::empty(),
        );
        let error = validate_state_footprint(&validated, &evidence)
            .expect_err("XMM use must derive vector-state use");
        assert!(error.0.contains("ceiling"));
    }

    #[test]
    fn call_clobbers_must_fit_the_entry_state_ceiling() {
        let mut plan = strict_x86_entry();
        plan.call.ordinary_clobbers = RegisterSet::new(
            plan.call
                .ordinary_clobbers
                .as_slice()
                .iter()
                .copied()
                .chain([MachineRegister::X86Xmm(0)]),
        );
        let error = validate_boundary_entry_plan(plan, &integer_signature(1))
            .expect_err("unsaved SIMD clobber must reject");
        assert!(error.0.contains("clobbers exceed"));
    }

    #[test]
    fn unsupported_aggregate_shapes_fail_closed() {
        let signature = CallSignature {
            parameters: vec![ValueShape::integer(16, 8)],
            result: None,
        };
        let error = evaluate_call_plan(CallingPolicy::SystemVAMD64, &signature)
            .expect_err("unclassified aggregate must reject");
        assert!(error.0.contains("not normalized"));
    }
}

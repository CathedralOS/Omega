//! Normalized boundary calling and machine-state plans.
//!
//! The existing encoders still realize these policies directly. This module is
//! the semantic seam they are migrating toward: policy + signature produces a
//! deterministic `CallPlan`; inbound roots pair it with a `StatePlan`.
//! Backend footprint evidence is deliberately a different artifact.

use crate::callback_materializations::{
    CallbackMaterialization, CallbackMaterializationContext, NativePlace,
    validate_callback_materializations,
};
use omega_target::Architecture;
use sha2::{Digest, Sha256};

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

    pub const fn borrowed_reference(byte_size: u16, alignment: u16) -> Self {
        Self {
            class: ValueClass::BorrowedReference,
            byte_size,
            alignment,
        }
    }

    pub const fn homogeneous_float_aggregate(member_size: u16, members: u8) -> Self {
        Self {
            class: ValueClass::HomogeneousFloatAggregate { members },
            byte_size: member_size * members as u16,
            alignment: member_size,
        }
    }

    pub const fn system_v_aggregate(
        byte_size: u16,
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

/// One concrete call to a C variadic function after default argument
/// promotion. The fixed/anonymous boundary is ABI-significant even though all
/// parameters have already acquired exact machine shapes.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ConcreteVariadicCallSignature {
    pub fixed_parameters: Vec<ValueShape>,
    pub variadic_parameters: Vec<ValueShape>,
    pub result: Option<ValueShape>,
}

impl ConcreteVariadicCallSignature {
    pub fn flattened(&self) -> CallSignature {
        CallSignature {
            parameters: self
                .fixed_parameters
                .iter()
                .chain(&self.variadic_parameters)
                .copied()
                .collect(),
            result: self.result,
        }
    }
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
    /// A value passed indirectly through a pointer. Parameters larger than the
    /// ABI's direct-value ceiling carry a pointer to a caller-owned stack copy;
    /// large results carry a pointer to their final caller-owned destination
    /// and therefore have no copy slot.
    Indirect {
        pointer: IndirectPointerLocation,
        copy_stack_byte_offset: Option<u32>,
        byte_size: u16,
        alignment: u16,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndirectPointerLocation {
    Register(MachineRegister),
    Stack {
        stack_byte_offset: u32,
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

/// The compile-time result published by an implementation of the source
/// `CallingPolicy::plan` relationship. A rejected policy is deliberately not
/// representable as a validated plan and therefore cannot acquire contract
/// identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoundaryPlanResult {
    Accepted(BoundaryEntryPlan),
    Rejected(CallingPolicyRejection),
}

/// Structured policy-authored context for a boundary signature the policy
/// cannot represent. The compiler retains this distinct from validator
/// failures in an allegedly accepted plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallingPolicyRejection {
    reason: String,
}

impl CallingPolicyRejection {
    pub fn new(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
        }
    }

    pub fn reason(&self) -> &str {
        &self.reason
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedBoundaryEntryPlan(BoundaryEntryPlan);

impl ValidatedBoundaryEntryPlan {
    pub const fn plan(&self) -> &BoundaryEntryPlan {
        &self.0
    }

    /// Deterministic compatibility/report coordinate for the canonical public
    /// contract. Strong replay uses [`Self::contract_commitment_digest`].
    pub fn contract_report_fingerprint(&self) -> u64 {
        let mut hash = Fnv1a::new();
        hash.call_plan(&self.0.call);
        hash.state_plan(&self.0.state);
        hash.finish()
    }

    /// Domain-separated commitment to the complete canonical boundary plan.
    pub fn contract_commitment_digest(&self) -> [u8; 32] {
        let mut hash = Fnv1a::with_strong_domain(b"omega.boundary-calling-plan.v1");
        hash.call_plan(&self.0.call);
        hash.state_plan(&self.0.state);
        hash.finish_strong()
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

/// Provider evidence for the control-state transition that leaves one
/// externally entered boundary. This is implementation evidence, not part of
/// the public `CallPlan + StatePlan` identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderExitRealization {
    pub control: EntryControl,
    pub restored_state: MachineStateSet,
}

/// Verify that a provider's realized exit is exactly the exit admitted by the
/// boundary contract. Footprint validation covers state touched by the body;
/// this separate check prevents an otherwise-valid footprint from returning
/// through the wrong control mechanism or restore set. The plan's
/// `initial_regime` is an entry fact, so this evidence deliberately does not
/// invent a same-regime exit promise.
pub fn validate_provider_exit_realization(
    plan: &BoundaryEntryPlan,
    realization: &ProviderExitRealization,
) -> Result<(), PlanDiagnostic> {
    if realization.control != plan.call.entry_control {
        return Err(PlanDiagnostic(
            "provider exit control does not match the admitted CallPlan".into(),
        ));
    }
    if realization.restored_state != plan.state.restored_state {
        return Err(PlanDiagnostic(
            "provider exit restored-state set does not match the admitted StatePlan".into(),
        ));
    }
    Ok(())
}

/// Compose implementation evidence from independently derived code fragments.
/// Register sets and machine-state classes are mathematical unions, so the
/// result is deterministic across fragment ordering and repeated evidence.
/// This remains implementation evidence: it does not enter boundary contract
/// identity and does not claim to be a final placed-artifact certificate.
pub fn compose_state_footprints<'a>(
    fragments: impl IntoIterator<Item = &'a StateFootprintEvidence>,
) -> StateFootprintEvidence {
    let mut registers = Vec::new();
    let mut machine_state = MachineStateSet::empty();
    for fragment in fragments {
        registers.extend_from_slice(fragment.registers().as_slice());
        machine_state = machine_state.union(fragment.machine_state());
    }
    StateFootprintEvidence::new(RegisterSet::new(registers), machine_state)
}

/// Compose fragment evidence and validate the whole transitive footprint
/// against one already-validated boundary plan. Returning the normalized
/// aggregate lets later object/final-image consumers retain exactly the
/// evidence that was checked without publishing it as requirement identity.
pub fn validate_composed_state_footprint<'a>(
    validated: &ValidatedBoundaryEntryPlan,
    fragments: impl IntoIterator<Item = &'a StateFootprintEvidence>,
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    let composed = compose_state_footprints(fragments);
    validate_state_footprint(validated, &composed)?;
    Ok(composed)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanDiagnostic(pub String);

impl std::fmt::Display for PlanDiagnostic {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for PlanDiagnostic {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoundaryPlanDiagnostic {
    Rejected(CallingPolicyRejection),
    InvalidAcceptedPlan(PlanDiagnostic),
}

impl std::fmt::Display for BoundaryPlanDiagnostic {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Rejected(rejection) => {
                write!(
                    formatter,
                    "calling policy rejected the boundary: {}",
                    rejection.reason()
                )
            }
            Self::InvalidAcceptedPlan(diagnostic) => {
                write!(
                    formatter,
                    "calling policy accepted an invalid plan: {diagnostic}"
                )
            }
        }
    }
}

impl std::error::Error for BoundaryPlanDiagnostic {}

pub fn evaluate_call_plan(
    policy: CallingPolicy,
    signature: &CallSignature,
) -> Result<CallPlan, PlanDiagnostic> {
    validate_signature_shapes(policy, signature)?;
    let runtime_signature = CallSignature {
        parameters: signature
            .parameters
            .iter()
            .copied()
            .filter(|shape| shape.byte_size != 0)
            .collect(),
        result: signature.result.filter(|shape| shape.byte_size != 0),
    };
    let mut plan = match policy {
        CallingPolicy::MicrosoftX64 => evaluate_microsoft_x64(&runtime_signature)?,
        CallingPolicy::SystemVAMD64 => evaluate_system_v_amd64(&runtime_signature)?,
        CallingPolicy::Aapcs64 => evaluate_aapcs64(&runtime_signature)?,
        CallingPolicy::LinuxSyscallX86_64 => evaluate_linux_syscall_x86_64(&runtime_signature)?,
        CallingPolicy::LinuxSyscallAarch64 => evaluate_linux_syscall_aarch64(&runtime_signature)?,
    };
    let mut runtime_parameters = plan.parameters.into_iter();
    plan.parameters = signature
        .parameters
        .iter()
        .copied()
        .map(|shape| {
            if shape.byte_size == 0 {
                ValuePlacement {
                    shape,
                    locations: Vec::new(),
                }
            } else {
                runtime_parameters
                    .next()
                    .expect("runtime call plan covers every nonempty parameter")
            }
        })
        .collect();
    debug_assert!(runtime_parameters.next().is_none());
    if signature.result.is_some_and(|shape| shape.byte_size == 0) {
        plan.result = Some(ValuePlacement {
            shape: signature.result.expect("zero-sized result exists"),
            locations: Vec::new(),
        });
    }
    plan.policy = policy;
    validate_call_plan(&plan, signature)?;
    Ok(plan)
}

/// Concrete state policy for ordinary call/return entries, including hosted
/// process roots and firmware handoffs. No interrupted activation exists, so
/// the entry stub owes no save/restore; its transitive state ceiling is exactly
/// the machine-state classes touched by the ABI's ordinary volatile registers
/// plus its caller-volatile condition flags.
pub fn evaluate_ordinary_boundary_entry_plan(
    policy: CallingPolicy,
    signature: &CallSignature,
) -> Result<ValidatedBoundaryEntryPlan, PlanDiagnostic> {
    let call = evaluate_call_plan(policy, signature)?;
    evaluate_ordinary_boundary_entry_plan_from_call(call, signature)
}

/// Concrete state policy for the compiler-selected implicit entry of a
/// freestanding program. Unlike a hosted ordinary call, this root is the
/// admitted owns-the-machine domain: checked instruction contracts may use
/// instruction, stack, and control state in addition to the ordinary ABI
/// volatile banks. An explicit source-selected boundary plan remains
/// authoritative and must not be widened through this compatibility path.
pub fn evaluate_freestanding_program_entry_plan(
    policy: CallingPolicy,
    signature: &CallSignature,
) -> Result<ValidatedBoundaryEntryPlan, PlanDiagnostic> {
    let ordinary = evaluate_ordinary_boundary_entry_plan(policy, signature)?;
    let mut plan = ordinary.plan().clone();
    plan.state.permitted_transitive_use =
        plan.state
            .permitted_transitive_use
            .union(MachineStateSet::new([
                MachineState::InstructionPointer,
                MachineState::StackPointer,
                MachineState::ControlState,
            ]));
    validate_boundary_entry_plan(plan, signature)
}

pub fn evaluate_darwin_aapcs64_variadic_boundary_entry_plan(
    signature: &ConcreteVariadicCallSignature,
) -> Result<ValidatedBoundaryEntryPlan, PlanDiagnostic> {
    let call = evaluate_darwin_aapcs64_variadic_call_plan(signature)?;
    evaluate_ordinary_boundary_entry_plan_from_call(call, &signature.flattened())
}

fn evaluate_ordinary_boundary_entry_plan_from_call(
    call: CallPlan,
    signature: &CallSignature,
) -> Result<ValidatedBoundaryEntryPlan, PlanDiagnostic> {
    let permitted_transitive_use = machine_state_for_registers(&call.ordinary_clobbers)
        .union(MachineStateSet::new([MachineState::Flags]));
    let initial_regime = match call.policy.architecture() {
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
    mut plan: BoundaryEntryPlan,
    signature: &CallSignature,
) -> Result<ValidatedBoundaryEntryPlan, PlanDiagnostic> {
    canonicalize_boundary_entry_plan(&mut plan);
    validate_call_plan(&plan.call, signature)?;
    validate_boundary_state_plan(plan, signature)
}

/// Validate a registrar boundary whose outbound plan carries private callback
/// materialization rows. The context is deliberately required: a bare plan
/// cannot establish that nominal binder, native-parameter, or layout-slot
/// identities exist or are compatible.
pub fn validate_boundary_entry_plan_with_callback_materializations(
    mut plan: BoundaryEntryPlan,
    signature: &CallSignature,
    context: &CallbackMaterializationContext,
) -> Result<ValidatedBoundaryEntryPlan, PlanDiagnostic> {
    canonicalize_boundary_entry_plan(&mut plan);
    validate_call_plan_structure(&plan.call, signature)?;
    validate_callback_materializations(&plan.call.callback_materializations, context)?;
    validate_boundary_state_plan(plan, signature)
}

fn validate_boundary_state_plan(
    plan: BoundaryEntryPlan,
    _signature: &CallSignature,
) -> Result<ValidatedBoundaryEntryPlan, PlanDiagnostic> {
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

/// Turns a source policy result into the only artifact allowed to contribute
/// requirement identity. Policy rejection and compiler validation failure stay
/// distinguishable so callers can attach the appropriate declaration-site
/// diagnostic.
pub fn validate_boundary_plan_result(
    result: BoundaryPlanResult,
    signature: &CallSignature,
) -> Result<ValidatedBoundaryEntryPlan, BoundaryPlanDiagnostic> {
    match result {
        BoundaryPlanResult::Accepted(plan) => validate_boundary_entry_plan(plan, signature)
            .map_err(BoundaryPlanDiagnostic::InvalidAcceptedPlan),
        BoundaryPlanResult::Rejected(rejection) => Err(BoundaryPlanDiagnostic::Rejected(rejection)),
    }
}

fn canonicalize_boundary_entry_plan(plan: &mut BoundaryEntryPlan) {
    for placement in plan
        .call
        .parameters
        .iter_mut()
        .chain(plan.call.result.iter_mut())
    {
        placement.locations.sort_by_key(value_location_byte_offset);
    }
    plan.call
        .callback_materializations
        .sort_by_key(|row| row.binder);
}

fn value_location_byte_offset(location: &ValueLocation) -> u16 {
    match location {
        ValueLocation::Register {
            value_byte_offset, ..
        }
        | ValueLocation::Stack {
            value_byte_offset, ..
        } => *value_byte_offset,
        ValueLocation::Indirect { .. } => 0,
    }
}

pub fn validate_state_footprint(
    validated: &ValidatedBoundaryEntryPlan,
    evidence: &StateFootprintEvidence,
) -> Result<(), PlanDiagnostic> {
    validate_state_footprint_under_ceiling(
        validated,
        evidence,
        validated.plan().state.permitted_transitive_use,
    )
}

/// Validate compiler-owned ordinary call-entry/return mechanics. Their
/// stack-pointer, control-transfer, and canonical floating-control-state
/// effects are prescribed by `CallReturn`; they are not handler-body
/// transitive use and therefore sit outside that ceiling. All other machine
/// state remains constrained by the ordinary transitive ceiling, and
/// interrupted state still has to be saved.
pub fn validate_call_return_mechanics_footprint(
    validated: &ValidatedBoundaryEntryPlan,
    evidence: &StateFootprintEvidence,
) -> Result<(), PlanDiagnostic> {
    if validated.plan().call.entry_control != EntryControl::CallReturn {
        return Err(PlanDiagnostic(
            "ordinary call-return footprint evidence requires CallReturn entry control".into(),
        ));
    }
    let prescribed_control = MachineStateSet::new([
        MachineState::InstructionPointer,
        MachineState::StackPointer,
        MachineState::ControlState,
    ]);
    validate_state_footprint_under_ceiling(
        validated,
        evidence,
        validated
            .plan()
            .state
            .permitted_transitive_use
            .union(prescribed_control),
    )
}

/// Validate one outbound call leaf inside an ordinary call-return activation.
/// The callee or supervisor may consume the plan's complete volatile ceiling;
/// control transfer is prescribed by the selected outbound entry mechanism,
/// rather than being ordinary handler-body transitive use.
pub fn validate_outbound_call_footprint(
    validated: &ValidatedBoundaryEntryPlan,
    evidence: &StateFootprintEvidence,
) -> Result<(), PlanDiagnostic> {
    if validated.plan().call.entry_control != EntryControl::CallReturn {
        return Err(PlanDiagnostic(
            "outbound call footprint evidence requires an enclosing CallReturn activation".into(),
        ));
    }
    validate_state_footprint_under_ceiling(
        validated,
        evidence,
        validated
            .plan()
            .state
            .permitted_transitive_use
            .union(MachineStateSet::new([
                MachineState::InstructionPointer,
                MachineState::StackPointer,
                MachineState::ControlState,
            ])),
    )
}

/// Validate a recursive runtime-value evaluator used by guards or ordinary
/// binary writes. Its x86 lowering may use balanced push/pop pairs while
/// evaluating `Binary` operands; that stack effect is prescribed only for an
/// ordinary call-return activation. Every other state class remains under the
/// boundary's transitive ceiling.
pub fn validate_runtime_value_guard_footprint(
    validated: &ValidatedBoundaryEntryPlan,
    evidence: &StateFootprintEvidence,
) -> Result<(), PlanDiagnostic> {
    let stack_use = MachineStateSet::new([MachineState::StackPointer]);
    let control_use = MachineStateSet::new([MachineState::ControlState]);
    let uses_stack = evidence.machine_state().contains_all(stack_use);
    let uses_control = evidence.machine_state().contains_all(control_use);
    if uses_stack
        && (validated.plan().call.policy.architecture() != Architecture::X86_64
            || validated.plan().call.entry_control != EntryControl::CallReturn)
    {
        return Err(PlanDiagnostic(
            "runtime-value guard stack scratch requires an x86 call-return activation".into(),
        ));
    }
    if uses_control && validated.plan().call.entry_control != EntryControl::CallReturn {
        return Err(PlanDiagnostic(
            "runtime-value guard directed rounding requires a call-return activation".into(),
        ));
    }
    validate_state_footprint_under_ceiling(
        validated,
        evidence,
        validated
            .plan()
            .state
            .permitted_transitive_use
            .union(if uses_stack {
                stack_use
            } else {
                MachineStateSet::empty()
            })
            .union(if uses_control {
                control_use
            } else {
                MachineStateSet::empty()
            }),
    )
}

fn validate_state_footprint_under_ceiling(
    validated: &ValidatedBoundaryEntryPlan,
    evidence: &StateFootprintEvidence,
    permitted_state: MachineStateSet,
) -> Result<(), PlanDiagnostic> {
    let plan = validated.plan();
    for register in evidence.registers().as_slice() {
        if register.architecture() != plan.call.policy.architecture() {
            return Err(PlanDiagnostic(format!(
                "footprint register {register:?} belongs to the wrong architecture"
            )));
        }
    }
    if !permitted_state.contains_all(evidence.machine_state()) {
        return Err(PlanDiagnostic(format!(
            "emitted machine-state footprint {:?} exceeds the entry plan ceiling {:?}",
            evidence.machine_state(),
            permitted_state
        )));
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
    /// Non-authoritative report coordinate over the retained exact register
    /// and machine-state evidence.
    pub fn evidence_report_fingerprint(&self) -> u64 {
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
    if signature
        .result
        .is_some_and(|shape| matches!(shape.class, ValueClass::BorrowedReference))
    {
        return Err(PlanDiagnostic(
            "borrowed references are parameter-only call values".into(),
        ));
    }
    for shape in signature.parameters.iter().chain(signature.result.iter()) {
        if shape.alignment == 0 || !shape.alignment.is_power_of_two() {
            return Err(PlanDiagnostic(
                "call-signature values need power-of-two alignment".into(),
            ));
        }
        if shape.byte_size == 0 && (shape.class != ValueClass::Integer || shape.alignment != 1) {
            return Err(PlanDiagnostic(
                "zero-sized call values must use the canonical integer-class shape".into(),
            ));
        }
        match shape.class {
            ValueClass::BorrowedReference if shape.byte_size == 0 => {
                return Err(PlanDiagnostic(
                    "borrowed-reference call values need a nonempty referent".into(),
                ));
            }
            ValueClass::Integer
                if shape.byte_size > 8
                    && policy != CallingPolicy::Aapcs64
                    && policy != CallingPolicy::SystemVAMD64
                    && policy != CallingPolicy::MicrosoftX64 =>
            {
                return Err(PlanDiagnostic(
                    "aggregate integer classification is not normalized for this calling policy"
                        .into(),
                ));
            }
            ValueClass::Float if !matches!(shape.byte_size, 4 | 8) => {
                return Err(PlanDiagnostic(
                    "scalar floating-point call values must be f32 or f64 sized".into(),
                ));
            }
            ValueClass::HomogeneousFloatAggregate { members }
                if !matches!(policy, CallingPolicy::Aapcs64 | CallingPolicy::SystemVAMD64)
                    || !(1..=4).contains(&members)
                    || shape.byte_size % u16::from(members.max(1)) != 0 =>
            {
                return Err(PlanDiagnostic(
                    "homogeneous float aggregates require a supported native policy and equal members"
                        .into(),
                ));
            }
            ValueClass::HomogeneousFloatAggregate { members }
                if policy == CallingPolicy::SystemVAMD64
                    && !(matches!(members, 2..=4)
                        && shape.byte_size <= 16
                        && matches!(shape.alignment, 4 | 8)
                        && shape.byte_size == u16::from(members) * u16::from(shape.alignment)) =>
            {
                return Err(PlanDiagnostic(
                    "SysV AMD64 homogeneous-float normalization requires two to four f32/f64 members totaling at most two eightbytes"
                        .into(),
                ));
            }
            ValueClass::SystemVAggregate { first, second }
                if policy != CallingPolicy::SystemVAMD64
                    || !(9..=16).contains(&shape.byte_size)
                    || shape.alignment > 8
                    || matches!(
                        (first, second),
                        (
                            SystemVEightbyteClass::Integer,
                            SystemVEightbyteClass::Integer
                        )
                    ) =>
            {
                return Err(PlanDiagnostic(
                    "classified SysV aggregates require at least one SSE eightbyte, a 9-16 byte shape, and at most eight-byte alignment"
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
    validate_call_plan_structure(plan, signature)?;
    if !plan.callback_materializations.is_empty() {
        return Err(PlanDiagnostic(
            "callback materializations require their nominal binder and native-place context"
                .into(),
        ));
    }
    Ok(())
}

fn validate_call_plan_structure(
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
                ValueLocation::Indirect {
                    pointer,
                    copy_stack_byte_offset,
                    byte_size,
                    ..
                } => {
                    match pointer {
                        IndirectPointerLocation::Register(register) => {
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
                        IndirectPointerLocation::Stack {
                            stack_byte_offset, ..
                        } => {
                            let end = stack_byte_offset + 8;
                            if occupied_stack_ranges.iter().any(|(start, prior_end)| {
                                stack_byte_offset < *prior_end && *start < end
                            }) {
                                return Err(PlanDiagnostic(format!(
                                    "parameter {index} indirect pointer overlaps another stack placement"
                                )));
                            }
                            occupied_stack_ranges.push((stack_byte_offset, end));
                        }
                    }
                    if let Some(copy_stack_byte_offset) = copy_stack_byte_offset {
                        let end = copy_stack_byte_offset + u32::from(byte_size);
                        if occupied_stack_ranges.iter().any(|(start, prior_end)| {
                            copy_stack_byte_offset < *prior_end && *start < end
                        }) {
                            return Err(PlanDiagnostic(format!(
                                "parameter {index} indirect copy overlaps another stack placement"
                            )));
                        }
                        occupied_stack_ranges.push((copy_stack_byte_offset, end));
                    }
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
    if matches!(placement.shape.class, ValueClass::BorrowedReference)
        && !matches!(
            placement.locations.as_slice(),
            [ValueLocation::Indirect {
                copy_stack_byte_offset: None,
                ..
            }]
        )
    {
        return Err(PlanDiagnostic(format!(
            "borrowed-reference value {value_index} must retain one direct pointer to caller storage"
        )));
    }
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
            ValueLocation::Indirect {
                pointer,
                copy_stack_byte_offset,
                byte_size,
                alignment,
            } => {
                if byte_size != placement.shape.byte_size
                    || alignment != placement.shape.alignment
                    || alignment == 0
                    || !alignment.is_power_of_two()
                {
                    return Err(PlanDiagnostic(format!(
                        "value {value_index} has an invalid indirect shape"
                    )));
                }
                match pointer {
                    IndirectPointerLocation::Register(register) => {
                        if register.architecture() != architecture {
                            return Err(PlanDiagnostic(format!(
                                "value {value_index} uses an indirect pointer register from the wrong architecture"
                            )));
                        }
                    }
                    IndirectPointerLocation::Stack {
                        stack_byte_offset,
                        alignment,
                    } => {
                        if alignment == 0
                            || !alignment.is_power_of_two()
                            || stack_byte_offset % u32::from(alignment) != 0
                        {
                            return Err(PlanDiagnostic(format!(
                                "value {value_index} has a misaligned indirect pointer"
                            )));
                        }
                    }
                }
                if let Some(copy_stack_byte_offset) = copy_stack_byte_offset
                    && copy_stack_byte_offset % u32::from(alignment.clamp(8, 16)) != 0
                {
                    return Err(PlanDiagnostic(format!(
                        "value {value_index} has a misaligned indirect copy"
                    )));
                }
                (0, byte_size)
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
    let indirect_result = signature.result.is_some_and(|shape| {
        matches!(shape.class, ValueClass::Integer) && !matches!(shape.byte_size, 1 | 2 | 4 | 8)
    });
    let parameter_slot_base = usize::from(indirect_result);
    let mut parameters = Vec::with_capacity(signature.parameters.len());
    for (index, shape) in signature.parameters.iter().copied().enumerate() {
        if matches!(
            shape.class,
            ValueClass::HomogeneousFloatAggregate { .. } | ValueClass::SystemVAggregate { .. }
        ) {
            return Err(PlanDiagnostic(
                "Microsoft x64 aggregate classification is not normalized yet".into(),
            ));
        }
        let slot = parameter_slot_base + index;
        let location = if matches!(shape.class, ValueClass::BorrowedReference) {
            let pointer = if slot < 4 {
                IndirectPointerLocation::Register(integer[slot])
            } else {
                IndirectPointerLocation::Stack {
                    stack_byte_offset: 32 + ((slot - 4) * 8) as u32,
                    alignment: 8,
                }
            };
            ValueLocation::Indirect {
                pointer,
                copy_stack_byte_offset: None,
                byte_size: shape.byte_size,
                alignment: shape.alignment,
            }
        } else if matches!(shape.class, ValueClass::Integer)
            && !matches!(shape.byte_size, 1 | 2 | 4 | 8)
        {
            let pointer = if slot < 4 {
                IndirectPointerLocation::Register(integer[slot])
            } else {
                IndirectPointerLocation::Stack {
                    stack_byte_offset: 32 + ((slot - 4) * 8) as u32,
                    alignment: 8,
                }
            };
            ValueLocation::Indirect {
                pointer,
                copy_stack_byte_offset: None,
                byte_size: shape.byte_size,
                alignment: shape.alignment,
            }
        } else if slot < 4 {
            let register = if matches!(shape.class, ValueClass::Float) {
                MachineRegister::X86Xmm(slot as u8)
            } else {
                integer[slot]
            };
            register_location(register, shape)
        } else {
            stack_location(32 + ((slot - 4) * 8) as u32, shape)
        };
        parameters.push(ValuePlacement {
            shape,
            locations: vec![location],
        });
    }
    let stack_parameter_slots = (parameter_slot_base + parameters.len()).saturating_sub(4);
    let mut copy_stack_offset = 32 + (stack_parameter_slots * 8) as u32;
    for placement in &mut parameters {
        if let [
            ValueLocation::Indirect {
                copy_stack_byte_offset,
                byte_size,
                alignment: _,
                ..
            },
        ] = placement.locations.as_mut_slice()
        {
            // Microsoft requires caller-owned temporaries for indirectly
            // passed aggregates to be 16-byte aligned, even when the source
            // type itself has a smaller natural alignment.
            copy_stack_offset = align_up(copy_stack_offset, 16);
            if !matches!(placement.shape.class, ValueClass::BorrowedReference) {
                *copy_stack_byte_offset = Some(copy_stack_offset);
                copy_stack_offset += u32::from(*byte_size).next_multiple_of(8);
            }
        }
    }
    Ok(CallPlan {
        policy: CallingPolicy::MicrosoftX64,
        parameters,
        result: if indirect_result {
            let shape = signature.result.expect("indirect result shape was present");
            Some(ValuePlacement {
                shape,
                locations: vec![ValueLocation::Indirect {
                    pointer: IndirectPointerLocation::Register(MachineRegister::X86Rcx),
                    copy_stack_byte_offset: None,
                    byte_size: shape.byte_size,
                    alignment: shape.alignment,
                }],
            })
        } else {
            result_placement(signature.result, &[MachineRegister::X86Rax], |index| {
                MachineRegister::X86Xmm(index)
            })?
        },
        callback_materializations: Vec::new(),
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
    let mut plan = evaluate_split_bank_call(
        CallingPolicy::SystemVAMD64,
        signature,
        &integer,
        8,
        MachineRegister::X86Xmm,
        &[MachineRegister::X86Rax, MachineRegister::X86Rdx],
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
    )?;
    if let Some(result) = plan.result.as_mut()
        && matches!(result.shape.class, ValueClass::Integer)
        && result.shape.byte_size > 8
    {
        result.locations = if result.shape.byte_size > 16 {
            vec![ValueLocation::Indirect {
                pointer: IndirectPointerLocation::Register(MachineRegister::X86Rdi),
                copy_stack_byte_offset: None,
                byte_size: result.shape.byte_size,
                alignment: result.shape.alignment,
            }]
        } else {
            integer_register_fragment_locations(
                result.shape,
                &[MachineRegister::X86Rax, MachineRegister::X86Rdx],
                0,
            )
        };
    }
    if let Some(result) = plan.result.as_mut()
        && matches!(
            result.shape.class,
            ValueClass::HomogeneousFloatAggregate { .. }
        )
    {
        result.locations = sysv_sse_fragment_locations(result.shape, 0);
    }
    Ok(plan)
}

fn evaluate_aapcs64(signature: &CallSignature) -> Result<CallPlan, PlanDiagnostic> {
    let integer = (0..8).map(MachineRegister::Aarch64X).collect::<Vec<_>>();
    let mut plan = evaluate_split_bank_call(
        CallingPolicy::Aapcs64,
        signature,
        &integer,
        8,
        MachineRegister::Aarch64V,
        &integer[..2],
        16,
        RegisterSet::new(
            (0..=17)
                .map(MachineRegister::Aarch64X)
                .chain((0..=7).map(MachineRegister::Aarch64V))
                .chain((16..=31).map(MachineRegister::Aarch64V)),
        ),
    )?;
    if let Some(result) = plan.result.as_mut()
        && matches!(result.shape.class, ValueClass::Integer)
    {
        if result.shape.byte_size > 16 {
            result.locations = vec![ValueLocation::Indirect {
                pointer: IndirectPointerLocation::Register(MachineRegister::Aarch64X(8)),
                copy_stack_byte_offset: None,
                byte_size: result.shape.byte_size,
                alignment: result.shape.alignment,
            }];
        } else if result.shape.byte_size > 8 {
            result.locations = integer_register_fragment_locations(result.shape, &integer, 0);
        }
    }
    Ok(plan)
}

/// Evaluate Apple's arm64 C variadic rule for one fully shaped call. Fixed
/// parameters use ordinary AAPCS64 placement; promoted anonymous scalar
/// parameters use the outgoing stack area even while argument registers remain.
///
/// The first consumer is Darwin `open(path, flags, mode)`. Aggregate variadic
/// values remain fail-closed until an actual native binding needs their Apple
/// ABI classification.
pub fn evaluate_darwin_aapcs64_variadic_call_plan(
    signature: &ConcreteVariadicCallSignature,
) -> Result<CallPlan, PlanDiagnostic> {
    if signature.variadic_parameters.is_empty() {
        return Err(PlanDiagnostic(
            "a concrete variadic call must supply at least one anonymous parameter".into(),
        ));
    }
    let fixed_signature = CallSignature {
        parameters: signature.fixed_parameters.clone(),
        result: signature.result,
    };
    validate_signature_shapes(CallingPolicy::Aapcs64, &signature.flattened())?;
    let mut plan = evaluate_aapcs64(&fixed_signature)?;
    let mut stack_offset = call_plan_stack_extent(&plan);
    for (index, shape) in signature.variadic_parameters.iter().copied().enumerate() {
        if !matches!(shape.class, ValueClass::Integer | ValueClass::Float) || shape.byte_size > 8 {
            return Err(PlanDiagnostic(format!(
                "Darwin AAPCS64 anonymous parameter {index} is not a promoted scalar"
            )));
        }
        let alignment = shape.alignment.clamp(8, 16);
        stack_offset = align_up(stack_offset, u32::from(alignment));
        plan.parameters.push(ValuePlacement {
            shape,
            locations: vec![ValueLocation::Stack {
                stack_byte_offset: stack_offset,
                value_byte_offset: 0,
                byte_size: shape.byte_size,
                alignment,
            }],
        });
        stack_offset += u32::from(shape.byte_size.max(8));
    }
    validate_call_plan(&plan, &signature.flattened())?;
    Ok(plan)
}

fn call_plan_stack_extent(plan: &CallPlan) -> u32 {
    plan.parameters
        .iter()
        .flat_map(|placement| &placement.locations)
        .fold(0, |extent, location| {
            let end = match *location {
                ValueLocation::Register { .. } => 0,
                ValueLocation::Stack {
                    stack_byte_offset,
                    byte_size,
                    ..
                } => stack_byte_offset + u32::from(byte_size.max(8)),
                ValueLocation::Indirect {
                    pointer,
                    copy_stack_byte_offset,
                    byte_size,
                    ..
                } => {
                    let pointer_end = match pointer {
                        IndirectPointerLocation::Register(_) => 0,
                        IndirectPointerLocation::Stack {
                            stack_byte_offset, ..
                        } => stack_byte_offset + 8,
                    };
                    pointer_end.max(copy_stack_byte_offset.map_or(0, |offset| {
                        offset + u32::from(byte_size).next_multiple_of(8)
                    }))
                }
            };
            extent.max(end)
        })
}

fn evaluate_split_bank_call(
    policy: CallingPolicy,
    signature: &CallSignature,
    integer_registers: &[MachineRegister],
    float_register_count: u8,
    float_register: impl Fn(u8) -> MachineRegister + Copy,
    integer_results: &[MachineRegister],
    stack_alignment: u16,
    ordinary_clobbers: RegisterSet,
) -> Result<CallPlan, PlanDiagnostic> {
    // SysV MEMORY-class results use the hidden first integer argument (`rdi`)
    // as their caller-owned destination, shifting declared integer arguments.
    let mut integer_index = usize::from(
        policy == CallingPolicy::SystemVAMD64
            && signature.result.is_some_and(|shape| {
                matches!(shape.class, ValueClass::Integer) && shape.byte_size > 16
            }),
    );
    let mut float_index = 0u8;
    let mut stack_offset = 0u32;
    let mut parameters = Vec::with_capacity(signature.parameters.len());
    for shape in signature.parameters.iter().copied() {
        let mut locations = Vec::new();
        if matches!(shape.class, ValueClass::BorrowedReference) {
            let pointer = if integer_index < integer_registers.len() {
                let register = integer_registers[integer_index];
                integer_index += 1;
                IndirectPointerLocation::Register(register)
            } else {
                stack_offset = align_up(stack_offset, 8);
                let pointer = IndirectPointerLocation::Stack {
                    stack_byte_offset: stack_offset,
                    alignment: 8,
                };
                stack_offset += 8;
                pointer
            };
            locations.push(ValueLocation::Indirect {
                pointer,
                copy_stack_byte_offset: None,
                byte_size: shape.byte_size,
                alignment: shape.alignment,
            });
            parameters.push(ValuePlacement { shape, locations });
            continue;
        }
        if let ValueClass::SystemVAggregate { first, second } = shape.class {
            debug_assert_eq!(policy, CallingPolicy::SystemVAMD64);
            let classes = [first, second];
            let integer_registers_needed = classes
                .iter()
                .filter(|class| matches!(class, SystemVEightbyteClass::Integer))
                .count();
            let float_registers_needed = classes
                .iter()
                .filter(|class| matches!(class, SystemVEightbyteClass::Sse))
                .count() as u8;
            if integer_index + integer_registers_needed <= integer_registers.len()
                && float_index.saturating_add(float_registers_needed) <= float_register_count
            {
                let mut aggregate_integer_index = integer_index;
                let mut aggregate_float_index = float_index;
                for (fragment, class) in classes.into_iter().enumerate() {
                    let register = match class {
                        SystemVEightbyteClass::Integer => {
                            let register = integer_registers[aggregate_integer_index];
                            aggregate_integer_index += 1;
                            register
                        }
                        SystemVEightbyteClass::Sse => {
                            let register = float_register(aggregate_float_index);
                            aggregate_float_index += 1;
                            register
                        }
                    };
                    let value_byte_offset = fragment as u16 * 8;
                    locations.push(ValueLocation::Register {
                        register,
                        value_byte_offset,
                        byte_size: (shape.byte_size - value_byte_offset).min(8),
                    });
                }
                integer_index = aggregate_integer_index;
                float_index = aggregate_float_index;
            } else {
                stack_offset = align_up(stack_offset, u32::from(shape.alignment.clamp(8, 16)));
                locations.extend(integer_stack_fragment_locations(shape, stack_offset));
                stack_offset += u32::from(shape.byte_size).next_multiple_of(8);
            }
            parameters.push(ValuePlacement { shape, locations });
            continue;
        }
        let float_members = match shape.class {
            ValueClass::Float => Some(1),
            ValueClass::HomogeneousFloatAggregate { members } => Some(members),
            ValueClass::Integer => None,
            ValueClass::BorrowedReference => unreachable!("handled above"),
            ValueClass::SystemVAggregate { .. } => unreachable!("handled above"),
        };
        let float_registers_needed = float_members.map(|members| {
            if policy == CallingPolicy::SystemVAMD64
                && matches!(shape.class, ValueClass::HomogeneousFloatAggregate { .. })
            {
                shape.byte_size.div_ceil(8) as u8
            } else {
                members
            }
        });
        if let Some(registers_needed) = float_registers_needed
            && float_index.saturating_add(registers_needed) <= float_register_count
        {
            if policy == CallingPolicy::SystemVAMD64
                && matches!(shape.class, ValueClass::HomogeneousFloatAggregate { .. })
            {
                locations.extend(sysv_sse_fragment_locations(shape, float_index));
            } else {
                let members = float_members.expect("float register count came from members");
                let member_size = shape.byte_size / u16::from(members);
                for member in 0..members {
                    locations.push(ValueLocation::Register {
                        register: float_register(float_index + member),
                        value_byte_offset: u16::from(member) * member_size,
                        byte_size: member_size,
                    });
                }
            }
            float_index += registers_needed;
        } else if float_members.is_none() && shape.byte_size > 16 {
            if policy == CallingPolicy::Aapcs64 {
                let pointer = if integer_index < integer_registers.len() {
                    let register = integer_registers[integer_index];
                    integer_index += 1;
                    IndirectPointerLocation::Register(register)
                } else {
                    stack_offset = align_up(stack_offset, 8);
                    let pointer = IndirectPointerLocation::Stack {
                        stack_byte_offset: stack_offset,
                        alignment: 8,
                    };
                    stack_offset += 8;
                    pointer
                };
                locations.push(ValueLocation::Indirect {
                    pointer,
                    copy_stack_byte_offset: None,
                    byte_size: shape.byte_size,
                    alignment: shape.alignment,
                });
            } else {
                debug_assert_eq!(policy, CallingPolicy::SystemVAMD64);
                stack_offset = align_up(stack_offset, u32::from(shape.alignment.clamp(8, 16)));
                locations.extend(integer_stack_fragment_locations(shape, stack_offset));
                stack_offset += u32::from(shape.byte_size).next_multiple_of(8);
            }
        } else if float_members.is_none() && shape.byte_size > 8 {
            // Integer-class aggregates up to two eightbytes stay whole: use
            // consecutive integer registers only when the complete value
            // fits, otherwise place every fragment on the stack.
            let register_count = usize::from(shape.byte_size.div_ceil(8));
            if policy == CallingPolicy::Aapcs64 && shape.alignment >= 16 {
                integer_index = integer_index.next_multiple_of(2);
            }
            if integer_index + register_count <= integer_registers.len() {
                locations.extend(integer_register_fragment_locations(
                    shape,
                    integer_registers,
                    integer_index,
                ));
                integer_index += register_count;
            } else {
                // AAPCS64 advances NGRN to eight after a register-exhausted
                // aggregate. SysV rolls back the tentative assignment, so a
                // later scalar may still consume the remaining register.
                if policy == CallingPolicy::Aapcs64 {
                    integer_index = integer_registers.len();
                }
                stack_offset = align_up(stack_offset, u32::from(shape.alignment.clamp(8, 16)));
                locations.extend(integer_stack_fragment_locations(shape, stack_offset));
                stack_offset += u32::from(shape.byte_size).next_multiple_of(8);
            }
        } else if float_members.is_some() && policy == CallingPolicy::SystemVAMD64 {
            stack_offset = align_up(stack_offset, u32::from(shape.alignment.clamp(8, 16)));
            locations.extend(integer_stack_fragment_locations(shape, stack_offset));
            stack_offset += u32::from(shape.byte_size).next_multiple_of(8);
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
    for placement in &mut parameters {
        if let [
            ValueLocation::Indirect {
                copy_stack_byte_offset,
                alignment,
                byte_size,
                ..
            },
        ] = placement.locations.as_mut_slice()
        {
            stack_offset = align_up(stack_offset, u32::from((*alignment).clamp(8, 16)));
            if !matches!(placement.shape.class, ValueClass::BorrowedReference) {
                *copy_stack_byte_offset = Some(stack_offset);
                stack_offset += u32::from(*byte_size).next_multiple_of(8);
            }
        }
    }
    Ok(CallPlan {
        policy,
        parameters,
        result: result_placement(signature.result, integer_results, float_register)?,
        callback_materializations: Vec::new(),
        ordinary_clobbers,
        stack_alignment,
        shadow_bytes: 0,
        entry_control: EntryControl::CallReturn,
    })
}

fn integer_register_fragment_locations(
    shape: ValueShape,
    registers: &[MachineRegister],
    first_register: usize,
) -> Vec<ValueLocation> {
    (0..usize::from(shape.byte_size.div_ceil(8)))
        .map(|fragment| {
            let value_byte_offset = fragment * 8;
            ValueLocation::Register {
                register: registers[first_register + fragment],
                value_byte_offset: value_byte_offset as u16,
                byte_size: (usize::from(shape.byte_size) - value_byte_offset).min(8) as u16,
            }
        })
        .collect()
}

fn integer_stack_fragment_locations(
    shape: ValueShape,
    first_stack_byte_offset: u32,
) -> Vec<ValueLocation> {
    (0..usize::from(shape.byte_size.div_ceil(8)))
        .map(|fragment| {
            let value_byte_offset = fragment * 8;
            ValueLocation::Stack {
                stack_byte_offset: first_stack_byte_offset + value_byte_offset as u32,
                value_byte_offset: value_byte_offset as u16,
                byte_size: (usize::from(shape.byte_size) - value_byte_offset).min(8) as u16,
                alignment: 8,
            }
        })
        .collect()
}

fn sysv_sse_fragment_locations(shape: ValueShape, first_register: u8) -> Vec<ValueLocation> {
    (0..shape.byte_size.div_ceil(8))
        .map(|fragment| {
            let value_byte_offset = fragment * 8;
            ValueLocation::Register {
                register: MachineRegister::X86Xmm(first_register + fragment as u8),
                value_byte_offset,
                byte_size: (shape.byte_size - value_byte_offset).min(8),
            }
        })
        .collect()
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
        callback_materializations: Vec::new(),
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
    integer_registers: &[MachineRegister],
    float_register: impl Fn(u8) -> MachineRegister,
) -> Result<Option<ValuePlacement>, PlanDiagnostic> {
    let Some(shape) = result else {
        return Ok(None);
    };
    let locations = match shape.class {
        ValueClass::Integer => vec![register_location(
            *integer_registers.first().ok_or_else(|| {
                PlanDiagnostic("calling policy has no integer result register".into())
            })?,
            shape,
        )],
        ValueClass::Float => vec![register_location(float_register(0), shape)],
        ValueClass::BorrowedReference => {
            return Err(PlanDiagnostic(
                "borrowed references cannot be call results".into(),
            ));
        }
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
        ValueClass::SystemVAggregate { first, second } => {
            let classes = [first, second];
            let mut integer_index = 0usize;
            let mut float_index = 0u8;
            classes
                .into_iter()
                .enumerate()
                .map(|(fragment, class)| {
                    let register = match class {
                        SystemVEightbyteClass::Integer => {
                            let register =
                                *integer_registers.get(integer_index).ok_or_else(|| {
                                    PlanDiagnostic(
                                    "calling policy has too few integer aggregate result registers"
                                        .into(),
                                )
                                })?;
                            integer_index += 1;
                            register
                        }
                        SystemVEightbyteClass::Sse => {
                            let register = float_register(float_index);
                            float_index += 1;
                            register
                        }
                    };
                    let value_byte_offset = fragment as u16 * 8;
                    Ok(ValueLocation::Register {
                        register,
                        value_byte_offset,
                        byte_size: (shape.byte_size - value_byte_offset).min(8),
                    })
                })
                .collect::<Result<Vec<_>, PlanDiagnostic>>()?
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

const fn system_v_eightbyte_class_code(class: SystemVEightbyteClass) -> u8 {
    match class {
        SystemVEightbyteClass::Integer => 0,
        SystemVEightbyteClass::Sse => 1,
    }
}

struct Fnv1a {
    compact: u64,
    strong: Option<Sha256>,
}

impl Fnv1a {
    const fn new() -> Self {
        Self {
            compact: 0xcbf29ce484222325,
            strong: None,
        }
    }

    fn with_strong_domain(domain: &[u8]) -> Self {
        let mut strong = Sha256::new();
        strong.update((domain.len() as u64).to_le_bytes());
        strong.update(domain);
        Self {
            compact: 0xcbf29ce484222325,
            strong: Some(strong),
        }
    }

    const fn finish(self) -> u64 {
        self.compact
    }

    fn finish_strong(self) -> [u8; 32] {
        self.strong.expect("strong plan hasher").finalize().into()
    }

    fn bytes(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.compact ^= u64::from(*byte);
            self.compact = self.compact.wrapping_mul(0x100000001b3);
        }
        if let Some(strong) = &mut self.strong {
            strong.update(bytes);
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

    fn u64(&mut self, value: u64) {
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
        // Empty catalogs preserve the identity of ordinary plans created
        // before callback materialization existed. A nonempty catalog is a
        // new observable ABI commitment and receives its own domain tag.
        if !plan.callback_materializations.is_empty() {
            self.bytes(b"omega.callback-materializations.v1");
            self.u32(plan.callback_materializations.len() as u32);
            for row in &plan.callback_materializations {
                self.u64(row.binder.get());
                self.native_place(&row.destination);
            }
        }
    }

    fn native_place(&mut self, place: &NativePlace) {
        match place {
            NativePlace::Parameter(parameter) => {
                self.u8(0);
                self.u64(parameter.get());
            }
            NativePlace::Field {
                parameter,
                layout,
                field_path,
            } => {
                self.u8(1);
                self.u64(parameter.get());
                self.u64(layout.get());
                self.u32(field_path.len() as u32);
                for slot in field_path {
                    self.u64(slot.get());
                }
            }
        }
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
                ValueLocation::Indirect {
                    pointer,
                    copy_stack_byte_offset,
                    byte_size,
                    alignment,
                } => {
                    self.u8(2);
                    match pointer {
                        IndirectPointerLocation::Register(register) => {
                            self.u8(0);
                            self.register(register);
                        }
                        IndirectPointerLocation::Stack {
                            stack_byte_offset,
                            alignment,
                        } => {
                            self.u8(1);
                            self.u32(stack_byte_offset);
                            self.u16(alignment);
                        }
                    }
                    match copy_stack_byte_offset {
                        Some(offset) => {
                            self.u8(1);
                            self.u32(offset);
                        }
                        None => self.u8(0),
                    }
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
            ValueClass::BorrowedReference => self.u8(4),
            ValueClass::HomogeneousFloatAggregate { members } => {
                self.u8(2);
                self.u8(members);
            }
            ValueClass::SystemVAggregate { first, second } => {
                self.u8(3);
                self.u8(system_v_eightbyte_class_code(first));
                self.u8(system_v_eightbyte_class_code(second));
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
    fn zero_sized_parameters_retain_identity_without_consuming_abi_locations() {
        let empty = ValueShape::integer(0, 1);
        let scalar = ValueShape::integer(8, 8);
        for policy in [
            CallingPolicy::MicrosoftX64,
            CallingPolicy::SystemVAMD64,
            CallingPolicy::Aapcs64,
        ] {
            let plan = evaluate_call_plan(
                policy,
                &CallSignature {
                    parameters: vec![empty, scalar, empty],
                    result: None,
                },
            )
            .expect("canonical empty values are erased from ABI placement");
            assert_eq!(plan.parameters.len(), 3);
            assert_eq!(plan.parameters[0].shape, empty);
            assert!(plan.parameters[0].locations.is_empty());
            assert_eq!(plan.parameters[1].shape, scalar);
            assert!(!plan.parameters[1].locations.is_empty());
            assert_eq!(plan.parameters[2].shape, empty);
            assert!(plan.parameters[2].locations.is_empty());
            validate_call_plan(
                &plan,
                &CallSignature {
                    parameters: vec![empty, scalar, empty],
                    result: None,
                },
            )
            .unwrap();
        }
    }

    #[test]
    fn microsoft_x64_indirect_result_uses_rcx_and_shifts_parameters() {
        let signature = CallSignature {
            parameters: vec![ValueShape::integer(8, 8); 4],
            result: Some(ValueShape::integer(16, 8)),
        };
        let plan = evaluate_call_plan(CallingPolicy::MicrosoftX64, &signature)
            .expect("MS x64 indirect-result plan");

        assert!(matches!(
            plan.result.expect("indirect result").locations.as_slice(),
            [ValueLocation::Indirect {
                pointer: IndirectPointerLocation::Register(MachineRegister::X86Rcx),
                copy_stack_byte_offset: None,
                byte_size: 16,
                alignment: 8,
            }]
        ));
        assert!(matches!(
            plan.parameters[0].locations.as_slice(),
            [ValueLocation::Register {
                register: MachineRegister::X86Rdx,
                ..
            }]
        ));
        assert!(matches!(
            plan.parameters[3].locations.as_slice(),
            [ValueLocation::Stack {
                stack_byte_offset: 32,
                ..
            }]
        ));
    }

    #[test]
    fn microsoft_x64_indirect_parameters_use_positional_pointer_slots() {
        let signature = CallSignature {
            parameters: vec![
                ValueShape::integer(16, 8),
                ValueShape::integer(8, 8),
                ValueShape::integer(8, 8),
                ValueShape::integer(8, 8),
                ValueShape::integer(16, 8),
            ],
            result: None,
        };
        let plan = evaluate_call_plan(CallingPolicy::MicrosoftX64, &signature)
            .expect("MS x64 indirect-parameter plan");

        assert!(matches!(
            plan.parameters[0].locations.as_slice(),
            [ValueLocation::Indirect {
                pointer: IndirectPointerLocation::Register(MachineRegister::X86Rcx),
                copy_stack_byte_offset: Some(48),
                ..
            }]
        ));
        assert!(matches!(
            plan.parameters[4].locations.as_slice(),
            [ValueLocation::Indirect {
                pointer: IndirectPointerLocation::Stack {
                    stack_byte_offset: 32,
                    alignment: 8,
                },
                copy_stack_byte_offset: Some(64),
                ..
            }]
        ));
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
    fn darwin_aapcs64_variadic_scalars_start_on_the_stack() {
        let word = ValueShape::integer(8, 8);
        let mode = ValueShape::integer(4, 4);
        let signature = ConcreteVariadicCallSignature {
            fixed_parameters: vec![word, ValueShape::integer(4, 4)],
            variadic_parameters: vec![mode],
            result: Some(ValueShape::integer(4, 4)),
        };
        let plan = evaluate_darwin_aapcs64_variadic_call_plan(&signature)
            .expect("Darwin arm64 variadic plan");

        assert!(matches!(
            plan.parameters[0].locations.as_slice(),
            [ValueLocation::Register {
                register: MachineRegister::Aarch64X(0),
                ..
            }]
        ));
        assert!(matches!(
            plan.parameters[1].locations.as_slice(),
            [ValueLocation::Register {
                register: MachineRegister::Aarch64X(1),
                ..
            }]
        ));
        assert_eq!(
            plan.parameters[2].locations,
            vec![ValueLocation::Stack {
                stack_byte_offset: 0,
                value_byte_offset: 0,
                byte_size: 4,
                alignment: 8,
            }]
        );
        assert!(matches!(
            plan.result.expect("result placement").locations.as_slice(),
            [ValueLocation::Register {
                register: MachineRegister::Aarch64X(0),
                ..
            }]
        ));
    }

    #[test]
    fn system_v_small_integer_aggregates_use_consecutive_registers_and_results() {
        let signature = CallSignature {
            parameters: vec![ValueShape::integer(8, 8), ValueShape::integer(16, 8)],
            result: Some(ValueShape::integer(12, 8)),
        };
        let plan = evaluate_call_plan(CallingPolicy::SystemVAMD64, &signature)
            .expect("SysV small aggregate plan");

        assert_eq!(
            plan.parameters[1].locations,
            vec![
                ValueLocation::Register {
                    register: MachineRegister::X86Rsi,
                    value_byte_offset: 0,
                    byte_size: 8,
                },
                ValueLocation::Register {
                    register: MachineRegister::X86Rdx,
                    value_byte_offset: 8,
                    byte_size: 8,
                },
            ]
        );
        assert_eq!(
            plan.result.expect("aggregate result").locations,
            vec![
                ValueLocation::Register {
                    register: MachineRegister::X86Rax,
                    value_byte_offset: 0,
                    byte_size: 8,
                },
                ValueLocation::Register {
                    register: MachineRegister::X86Rdx,
                    value_byte_offset: 8,
                    byte_size: 4,
                },
            ]
        );
    }

    #[test]
    fn system_v_register_exhausted_aggregate_moves_wholly_to_stack_and_rolls_back() {
        let signature = CallSignature {
            parameters: vec![ValueShape::integer(8, 8); 5]
                .into_iter()
                .chain([ValueShape::integer(16, 8), ValueShape::integer(8, 8)])
                .collect(),
            result: None,
        };
        let plan = evaluate_call_plan(CallingPolicy::SystemVAMD64, &signature)
            .expect("SysV exhausted aggregate plan");

        assert_eq!(
            plan.parameters[5].locations,
            vec![
                ValueLocation::Stack {
                    stack_byte_offset: 0,
                    value_byte_offset: 0,
                    byte_size: 8,
                    alignment: 8,
                },
                ValueLocation::Stack {
                    stack_byte_offset: 8,
                    value_byte_offset: 8,
                    byte_size: 8,
                    alignment: 8,
                },
            ]
        );
        assert_eq!(
            plan.parameters[6].locations,
            vec![ValueLocation::Register {
                register: MachineRegister::X86R9,
                value_byte_offset: 0,
                byte_size: 8,
            }]
        );
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
    fn aapcs_small_integer_aggregates_use_consecutive_x_registers() {
        let signature = CallSignature {
            parameters: vec![ValueShape::integer(8, 8), ValueShape::integer(16, 16)],
            result: Some(ValueShape::integer(12, 8)),
        };
        let plan = evaluate_call_plan(CallingPolicy::Aapcs64, &signature)
            .expect("AAPCS small aggregate plan");

        assert_eq!(
            plan.parameters[1].locations,
            vec![
                ValueLocation::Register {
                    register: MachineRegister::Aarch64X(2),
                    value_byte_offset: 0,
                    byte_size: 8,
                },
                ValueLocation::Register {
                    register: MachineRegister::Aarch64X(3),
                    value_byte_offset: 8,
                    byte_size: 8,
                },
            ]
        );
        assert_eq!(
            plan.result.expect("aggregate result").locations,
            vec![
                ValueLocation::Register {
                    register: MachineRegister::Aarch64X(0),
                    value_byte_offset: 0,
                    byte_size: 8,
                },
                ValueLocation::Register {
                    register: MachineRegister::Aarch64X(1),
                    value_byte_offset: 8,
                    byte_size: 4,
                },
            ]
        );
    }

    #[test]
    fn aapcs_small_aggregate_moves_wholly_to_stack_when_x_registers_run_out() {
        let signature = CallSignature {
            parameters: vec![ValueShape::integer(8, 8); 7]
                .into_iter()
                .chain([ValueShape::integer(16, 8), ValueShape::integer(8, 8)])
                .collect(),
            result: None,
        };
        let plan = evaluate_call_plan(CallingPolicy::Aapcs64, &signature)
            .expect("AAPCS exhausted aggregate plan");

        assert_eq!(
            plan.parameters[7].locations,
            vec![
                ValueLocation::Stack {
                    stack_byte_offset: 0,
                    value_byte_offset: 0,
                    byte_size: 8,
                    alignment: 8,
                },
                ValueLocation::Stack {
                    stack_byte_offset: 8,
                    value_byte_offset: 8,
                    byte_size: 8,
                    alignment: 8,
                },
            ]
        );
        assert!(matches!(
            plan.parameters[8].locations.as_slice(),
            [ValueLocation::Stack {
                stack_byte_offset: 16,
                ..
            }]
        ));
    }

    #[test]
    fn aapcs_large_aggregates_use_caller_copies_and_indirect_results() {
        let signature = CallSignature {
            parameters: vec![ValueShape::integer(24, 8), ValueShape::integer(8, 8)],
            result: Some(ValueShape::integer(32, 16)),
        };
        let plan = evaluate_call_plan(CallingPolicy::Aapcs64, &signature)
            .expect("AAPCS large aggregate plan");

        assert_eq!(
            plan.parameters[0].locations,
            vec![ValueLocation::Indirect {
                pointer: IndirectPointerLocation::Register(MachineRegister::Aarch64X(0)),
                copy_stack_byte_offset: Some(0),
                byte_size: 24,
                alignment: 8,
            }]
        );
        assert!(matches!(
            plan.parameters[1].locations.as_slice(),
            [ValueLocation::Register {
                register: MachineRegister::Aarch64X(1),
                ..
            }]
        ));
        assert_eq!(
            plan.result.expect("indirect aggregate result").locations,
            vec![ValueLocation::Indirect {
                pointer: IndirectPointerLocation::Register(MachineRegister::Aarch64X(8)),
                copy_stack_byte_offset: None,
                byte_size: 32,
                alignment: 16,
            }]
        );
    }

    #[test]
    fn aapcs_large_aggregate_pointer_uses_stack_before_its_copy() {
        let signature = CallSignature {
            parameters: vec![ValueShape::integer(8, 8); 8]
                .into_iter()
                .chain([ValueShape::integer(24, 16)])
                .collect(),
            result: None,
        };
        let plan = evaluate_call_plan(CallingPolicy::Aapcs64, &signature)
            .expect("AAPCS stack-indirect aggregate plan");

        assert_eq!(
            plan.parameters[8].locations,
            vec![ValueLocation::Indirect {
                pointer: IndirectPointerLocation::Stack {
                    stack_byte_offset: 0,
                    alignment: 8,
                },
                copy_stack_byte_offset: Some(16),
                byte_size: 24,
                alignment: 16,
            }]
        );
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
        assert!(
            plan.state
                .permitted_transitive_use
                .contains_all(MachineStateSet::new([MachineState::Flags]))
        );
        validate_state_footprint(
            &validated,
            &StateFootprintEvidence::new(
                RegisterSet::default(),
                MachineStateSet::new([MachineState::Flags]),
            ),
        )
        .expect("ordinary caller-volatile condition flags fit the state ceiling");
    }

    #[test]
    fn implicit_freestanding_entry_admits_boot_root_machine_state() {
        let signature = integer_signature(1);
        let hosted = evaluate_ordinary_boundary_entry_plan(CallingPolicy::SystemVAMD64, &signature)
            .expect("ordinary entry");
        let freestanding =
            evaluate_freestanding_program_entry_plan(CallingPolicy::SystemVAMD64, &signature)
                .expect("implicit freestanding entry");
        let boot_root_state = MachineStateSet::new([
            MachineState::InstructionPointer,
            MachineState::StackPointer,
            MachineState::ControlState,
        ]);

        assert!(
            !hosted
                .plan()
                .state
                .permitted_transitive_use
                .contains_all(boot_root_state)
        );
        assert!(
            freestanding
                .plan()
                .state
                .permitted_transitive_use
                .contains_all(boot_root_state)
        );
        assert_eq!(
            hosted.plan().state.interrupted_state,
            freestanding.plan().state.interrupted_state
        );
        assert_eq!(
            hosted.plan().state.saved_state,
            freestanding.plan().state.saved_state
        );
        assert_eq!(
            hosted.plan().state.restored_state,
            freestanding.plan().state.restored_state
        );
    }

    #[test]
    fn provider_exit_realization_must_match_the_complete_boundary_exit() {
        let validated = validate_boundary_entry_plan(strict_x86_entry(), &integer_signature(1))
            .expect("strict interrupt boundary");
        let expected = ProviderExitRealization {
            control: validated.plan().call.entry_control,
            restored_state: validated.plan().state.restored_state,
        };
        validate_provider_exit_realization(validated.plan(), &expected)
            .expect("exact provider exit realization");

        for (realization, expected_diagnostic) in [
            (
                ProviderExitRealization {
                    control: EntryControl::CallReturn,
                    ..expected
                },
                "exit control",
            ),
            (
                ProviderExitRealization {
                    restored_state: MachineStateSet::empty(),
                    ..expected
                },
                "restored-state set",
            ),
        ] {
            let error = validate_provider_exit_realization(validated.plan(), &realization)
                .expect_err("drifted provider exit must reject");
            assert!(
                error.0.contains(expected_diagnostic),
                "expected `{expected_diagnostic}`, got `{error}`"
            );
        }
    }

    #[test]
    fn runtime_value_guard_stack_scratch_is_not_admitted_for_interrupt_return() {
        let validated = validate_boundary_entry_plan(strict_x86_entry(), &integer_signature(1))
            .expect("strict interrupt boundary");
        let evidence = StateFootprintEvidence::new(
            RegisterSet::new([MachineRegister::X86R10]),
            MachineStateSet::new([MachineState::Flags, MachineState::StackPointer]),
        );

        let error = validate_runtime_value_guard_footprint(&validated, &evidence)
            .expect_err("interrupt-return body must not borrow ordinary stack scratch");

        assert!(error.0.contains("x86 call-return activation"));
    }

    #[test]
    fn runtime_value_guard_control_state_is_not_admitted_for_interrupt_return() {
        let validated = validate_boundary_entry_plan(strict_x86_entry(), &integer_signature(1))
            .expect("strict interrupt boundary");
        let evidence = StateFootprintEvidence::new(
            RegisterSet::new([MachineRegister::X86R10]),
            MachineStateSet::new([MachineState::Flags, MachineState::ControlState]),
        );

        let error = validate_runtime_value_guard_footprint(&validated, &evidence)
            .expect_err("interrupt-return body must not change floating control state");

        assert!(
            error
                .0
                .contains("directed rounding requires a call-return activation")
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
        let identity = validated.contract_report_fingerprint();
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
            evidence_a.evidence_report_fingerprint(),
            evidence_b.evidence_report_fingerprint()
        );
        assert_eq!(identity, validated.contract_report_fingerprint());
    }

    #[test]
    fn fragment_footprints_compose_deterministically_before_validation() {
        let validated = validate_boundary_entry_plan(strict_x86_entry(), &integer_signature(1))
            .expect("strict entry plan");
        let entry = StateFootprintEvidence::new(
            RegisterSet::new([MachineRegister::X86R11, MachineRegister::X86Rax]),
            MachineStateSet::empty(),
        );
        let body = StateFootprintEvidence::new(
            RegisterSet::new([MachineRegister::X86Rcx, MachineRegister::X86R11]),
            MachineStateSet::new([MachineState::Flags]),
        );

        let first = validate_composed_state_footprint(&validated, [&entry, &body, &entry])
            .expect("whole-entry footprint");
        let second = validate_composed_state_footprint(&validated, [&body, &entry])
            .expect("reordered whole-entry footprint");

        assert_eq!(first, second);
        assert_eq!(
            first.registers().as_slice(),
            &[
                MachineRegister::X86Rax,
                MachineRegister::X86Rcx,
                MachineRegister::X86R11,
            ]
        );
        assert_eq!(
            first.machine_state(),
            MachineStateSet::new([MachineState::GeneralRegisters, MachineState::Flags])
        );
        assert_eq!(
            first.evidence_report_fingerprint(),
            second.evidence_report_fingerprint()
        );
    }

    #[test]
    fn composed_footprint_rejects_one_fragment_above_the_state_ceiling() {
        let validated = validate_boundary_entry_plan(strict_x86_entry(), &integer_signature(1))
            .expect("strict entry plan");
        let entry = StateFootprintEvidence::new(
            RegisterSet::new([MachineRegister::X86Rax]),
            MachineStateSet::empty(),
        );
        let veneer = StateFootprintEvidence::new(
            RegisterSet::new([MachineRegister::X86Xmm(0)]),
            MachineStateSet::empty(),
        );

        let error = validate_composed_state_footprint(&validated, [&entry, &veneer])
            .expect_err("one vector-using fragment must reject the aggregate");

        assert!(error.0.contains("ceiling"));
    }

    #[test]
    fn composed_footprint_rejects_a_foreign_architecture_fragment() {
        let validated = validate_boundary_entry_plan(strict_x86_entry(), &integer_signature(1))
            .expect("strict entry plan");
        let entry = StateFootprintEvidence::new(
            RegisterSet::new([MachineRegister::X86Rax]),
            MachineStateSet::empty(),
        );
        let foreign_thunk = StateFootprintEvidence::new(
            RegisterSet::new([MachineRegister::Aarch64X(16)]),
            MachineStateSet::empty(),
        );

        let error = validate_composed_state_footprint(&validated, [&entry, &foreign_thunk])
            .expect_err("foreign-architecture evidence must reject the aggregate");

        assert!(error.0.contains("wrong architecture"));
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
        assert_ne!(
            first.contract_report_fingerprint(),
            second.contract_report_fingerprint()
        );
    }

    #[test]
    fn accepted_policy_results_canonicalize_fragment_order_before_identity() {
        let shape = ValueShape::system_v_aggregate(
            16,
            8,
            SystemVEightbyteClass::Integer,
            SystemVEightbyteClass::Sse,
        );
        let signature = CallSignature {
            parameters: vec![shape],
            result: None,
        };
        let baseline =
            evaluate_ordinary_boundary_entry_plan(CallingPolicy::SystemVAMD64, &signature)
                .expect("ordinary mixed-aggregate entry");
        let mut authored = baseline.plan().clone();
        authored.call.parameters[0].locations.reverse();

        let accepted =
            validate_boundary_plan_result(BoundaryPlanResult::Accepted(authored), &signature)
                .expect("semantically equivalent authored plan");

        assert_eq!(
            accepted.plan().call.parameters[0].locations,
            baseline.plan().call.parameters[0].locations
        );
        assert_eq!(
            accepted.contract_report_fingerprint(),
            baseline.contract_report_fingerprint()
        );
    }

    #[test]
    fn rejected_policy_results_cannot_acquire_contract_identity() {
        let result = validate_boundary_plan_result(
            BoundaryPlanResult::Rejected(CallingPolicyRejection::new(
                "interrupt policies do not admit return values",
            )),
            &integer_signature(1),
        );

        let BoundaryPlanDiagnostic::Rejected(rejection) =
            result.expect_err("policy rejection must not validate")
        else {
            panic!("policy rejection was reported as a malformed accepted plan");
        };
        assert_eq!(
            rejection.reason(),
            "interrupt policies do not admit return values"
        );
    }

    #[test]
    fn invalid_accepted_policy_plan_is_distinct_from_policy_rejection() {
        let mut plan = strict_x86_entry();
        plan.call.stack_alignment = 3;
        let result = validate_boundary_plan_result(
            BoundaryPlanResult::Accepted(plan),
            &integer_signature(1),
        );

        let BoundaryPlanDiagnostic::InvalidAcceptedPlan(diagnostic) =
            result.expect_err("invalid accepted plan must fail validation")
        else {
            panic!("invalid accepted plan was reported as policy rejection");
        };
        assert!(diagnostic.0.contains("stack alignment"));
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
    fn system_v_memory_class_uses_stack_values_and_a_hidden_result_pointer() {
        let signature = CallSignature {
            parameters: vec![ValueShape::integer(24, 8), ValueShape::integer(8, 8)],
            result: Some(ValueShape::integer(24, 8)),
        };
        let plan = evaluate_call_plan(CallingPolicy::SystemVAMD64, &signature)
            .expect("SysV MEMORY-class plan");

        assert_eq!(
            plan.parameters[0].locations,
            vec![
                ValueLocation::Stack {
                    stack_byte_offset: 0,
                    value_byte_offset: 0,
                    byte_size: 8,
                    alignment: 8,
                },
                ValueLocation::Stack {
                    stack_byte_offset: 8,
                    value_byte_offset: 8,
                    byte_size: 8,
                    alignment: 8,
                },
                ValueLocation::Stack {
                    stack_byte_offset: 16,
                    value_byte_offset: 16,
                    byte_size: 8,
                    alignment: 8,
                },
            ]
        );
        assert!(matches!(
            plan.parameters[1].locations.as_slice(),
            [ValueLocation::Register {
                register: MachineRegister::X86Rsi,
                ..
            }]
        ));
        assert!(matches!(
            plan.result.expect("indirect result").locations.as_slice(),
            [ValueLocation::Indirect {
                pointer: IndirectPointerLocation::Register(MachineRegister::X86Rdi),
                copy_stack_byte_offset: None,
                byte_size: 24,
                alignment: 8,
            }]
        ));
    }

    #[test]
    fn system_v_two_f64_record_uses_sse_fragments_or_whole_stack_rollback() {
        let pair = ValueShape::homogeneous_float_aggregate(8, 2);
        let signature = CallSignature {
            parameters: vec![ValueShape::float(8); 8]
                .into_iter()
                .chain([pair])
                .collect(),
            result: Some(pair),
        };
        let plan = evaluate_call_plan(CallingPolicy::SystemVAMD64, &signature)
            .expect("SysV two-f64 record plan");

        assert_eq!(
            plan.parameters[8].locations,
            vec![
                ValueLocation::Stack {
                    stack_byte_offset: 0,
                    value_byte_offset: 0,
                    byte_size: 8,
                    alignment: 8,
                },
                ValueLocation::Stack {
                    stack_byte_offset: 8,
                    value_byte_offset: 8,
                    byte_size: 8,
                    alignment: 8,
                },
            ]
        );
        assert!(matches!(
            plan.result.expect("SSE result").locations.as_slice(),
            [
                ValueLocation::Register {
                    register: MachineRegister::X86Xmm(0),
                    ..
                },
                ValueLocation::Register {
                    register: MachineRegister::X86Xmm(1),
                    ..
                }
            ]
        ));
    }

    #[test]
    fn system_v_three_f32_record_packs_into_two_sse_eightbytes() {
        let triple = ValueShape::homogeneous_float_aggregate(4, 3);
        let plan = evaluate_call_plan(
            CallingPolicy::SystemVAMD64,
            &CallSignature {
                parameters: vec![triple],
                result: Some(triple),
            },
        )
        .expect("SysV three-f32 record plan");

        let expected = vec![
            ValueLocation::Register {
                register: MachineRegister::X86Xmm(0),
                value_byte_offset: 0,
                byte_size: 8,
            },
            ValueLocation::Register {
                register: MachineRegister::X86Xmm(1),
                value_byte_offset: 8,
                byte_size: 4,
            },
        ];
        assert_eq!(plan.parameters[0].locations, expected);
        assert_eq!(plan.result.expect("packed SSE result").locations, expected);
    }

    #[test]
    fn system_v_mixed_record_uses_independent_register_banks() {
        let integer_sse = ValueShape::system_v_aggregate(
            16,
            8,
            SystemVEightbyteClass::Integer,
            SystemVEightbyteClass::Sse,
        );
        let sse_integer = ValueShape::system_v_aggregate(
            16,
            8,
            SystemVEightbyteClass::Sse,
            SystemVEightbyteClass::Integer,
        );
        let plan = evaluate_call_plan(
            CallingPolicy::SystemVAMD64,
            &CallSignature {
                parameters: vec![integer_sse],
                result: Some(sse_integer),
            },
        )
        .expect("mixed SysV aggregate plan");

        assert_eq!(
            plan.parameters[0].locations,
            vec![
                ValueLocation::Register {
                    register: MachineRegister::X86Rdi,
                    value_byte_offset: 0,
                    byte_size: 8,
                },
                ValueLocation::Register {
                    register: MachineRegister::X86Xmm(0),
                    value_byte_offset: 8,
                    byte_size: 8,
                },
            ]
        );
        assert_eq!(
            plan.result.expect("mixed result").locations,
            vec![
                ValueLocation::Register {
                    register: MachineRegister::X86Xmm(0),
                    value_byte_offset: 0,
                    byte_size: 8,
                },
                ValueLocation::Register {
                    register: MachineRegister::X86Rax,
                    value_byte_offset: 8,
                    byte_size: 8,
                },
            ]
        );
    }

    #[test]
    fn system_v_mixed_record_rolls_back_both_banks() {
        let mixed = ValueShape::system_v_aggregate(
            16,
            8,
            SystemVEightbyteClass::Integer,
            SystemVEightbyteClass::Sse,
        );
        let mut parameters = vec![ValueShape::float(8); 8];
        parameters.extend([mixed, ValueShape::integer(8, 8)]);
        let plan = evaluate_call_plan(
            CallingPolicy::SystemVAMD64,
            &CallSignature {
                parameters,
                result: None,
            },
        )
        .expect("mixed SysV rollback plan");

        assert!(matches!(
            plan.parameters[8].locations.as_slice(),
            [
                ValueLocation::Stack {
                    stack_byte_offset: 0,
                    value_byte_offset: 0,
                    byte_size: 8,
                    ..
                },
                ValueLocation::Stack {
                    stack_byte_offset: 8,
                    value_byte_offset: 8,
                    byte_size: 8,
                    ..
                }
            ]
        ));
        assert!(matches!(
            plan.parameters[9].locations.as_slice(),
            [ValueLocation::Register {
                register: MachineRegister::X86Rdi,
                ..
            }]
        ));

        let mut parameters = vec![ValueShape::integer(8, 8); 6];
        parameters.extend([mixed, ValueShape::float(8)]);
        let plan = evaluate_call_plan(
            CallingPolicy::SystemVAMD64,
            &CallSignature {
                parameters,
                result: None,
            },
        )
        .expect("inverse mixed SysV rollback plan");
        assert!(
            plan.parameters[6]
                .locations
                .iter()
                .all(|location| matches!(location, ValueLocation::Stack { .. }))
        );
        assert!(matches!(
            plan.parameters[7].locations.as_slice(),
            [ValueLocation::Register {
                register: MachineRegister::X86Xmm(0),
                ..
            }]
        ));
    }

    #[test]
    fn system_v_classified_record_rejects_all_integer_eightbytes() {
        let malformed = ValueShape::system_v_aggregate(
            16,
            8,
            SystemVEightbyteClass::Integer,
            SystemVEightbyteClass::Integer,
        );
        let error = evaluate_call_plan(
            CallingPolicy::SystemVAMD64,
            &CallSignature {
                parameters: vec![malformed],
                result: None,
            },
        )
        .expect_err("equal classes must use an existing normalized aggregate class");
        assert!(error.0.contains("at least one SSE eightbyte"));
    }

    #[test]
    fn borrowed_references_retain_original_storage_pointers_without_copies() {
        let borrowed = ValueShape::borrowed_reference(16, 8);
        for (policy, register) in [
            (CallingPolicy::MicrosoftX64, MachineRegister::X86Rcx),
            (CallingPolicy::SystemVAMD64, MachineRegister::X86Rdi),
            (CallingPolicy::Aapcs64, MachineRegister::Aarch64X(0)),
        ] {
            let plan = evaluate_call_plan(
                policy,
                &CallSignature {
                    parameters: vec![borrowed],
                    result: Some(ValueShape::integer(4, 4)),
                },
            )
            .expect("borrowed-reference plan");
            assert_eq!(
                plan.parameters[0].locations,
                vec![ValueLocation::Indirect {
                    pointer: IndirectPointerLocation::Register(register),
                    copy_stack_byte_offset: None,
                    byte_size: 16,
                    alignment: 8,
                }]
            );
            validate_call_plan(
                &plan,
                &CallSignature {
                    parameters: vec![borrowed],
                    result: Some(ValueShape::integer(4, 4)),
                },
            )
            .expect("borrowed-reference plan validates");
        }
    }

    #[test]
    fn borrowed_reference_stack_pointer_is_not_a_referent_copy() {
        let mut parameters = vec![ValueShape::integer(8, 8); 6];
        parameters.push(ValueShape::borrowed_reference(16, 8));
        let plan = evaluate_call_plan(
            CallingPolicy::SystemVAMD64,
            &CallSignature {
                parameters,
                result: None,
            },
        )
        .expect("stacked borrowed-reference plan");
        assert_eq!(
            plan.parameters[6].locations,
            vec![ValueLocation::Indirect {
                pointer: IndirectPointerLocation::Stack {
                    stack_byte_offset: 0,
                    alignment: 8,
                },
                copy_stack_byte_offset: None,
                byte_size: 16,
                alignment: 8,
            }]
        );
    }

    #[test]
    fn borrowed_references_are_not_results() {
        let error = evaluate_call_plan(
            CallingPolicy::SystemVAMD64,
            &CallSignature {
                parameters: Vec::new(),
                result: Some(ValueShape::borrowed_reference(8, 8)),
            },
        )
        .expect_err("borrowed-reference result must be rejected");
        assert!(error.0.contains("parameter-only"));
    }
}

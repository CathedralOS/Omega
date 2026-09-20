//! Deriver-owned entry/exit stub contract for one installed x86-64 external
//! root.
//!
//! The sealed target rule (`derive_x86_64_hardware_arrival`) fixes the
//! hardware frame a member's entry runs on. This module derives the rest of
//! the deriver-owned answer: per-context error-code normalization, the exact
//! register/state footprint the stub saves, and the interrupt-return exit the
//! emitted bytes must realize. The installed facts carry the admitted boundary
//! plan's commitment, so deriving against a `ValidatedBoundaryEntryPlan` whose
//! commitment differs rejects rather than attaching a lookalike policy. Byte
//! emission joins the contract at `identity` — the installed artifact, code,
//! entry, and entry offset the sealed gate already names.

use super::{
    ArrivalContextId, Fnv1a, InstalledEntryFactIdentity,
    ValidatedX86_64InstalledHardwareEntryFacts, X86_64ArrivalMechanism, X86_64GateKind,
    x86_64_arrival_frame_bytes, x86_64_arrival_pushes_error_code,
};
use crate::{
    EntryControl, EntryStack, MachineRegime, MachineRegister, MachineState, MachineStateSet,
    PlanDiagnostic, Preemption, ProviderExitRealization, RegisterSet, StateFootprintEvidence,
    ValidatedBoundaryEntryPlan, ValueLocation,
};

/// Where the normalized frame's error-code word comes from for one arrival
/// context.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum X86_64ErrorCodeDisposition {
    /// The CPU pushed the architectural error-code word; the stub leaves the
    /// frame as delivered.
    HardwarePushed,
    /// The vector carries no architectural error code, so the stub pushes a
    /// synthetic zero word. Every member therefore sees one frame shape.
    StubSynthesized,
}

/// The frame the stub reserves below the save area to stage the member call.
///
/// `outgoing_stack_bytes` is the 8-aligned extent of the boundary plan's
/// outgoing stack-argument locations; `reserved_bytes` adds the 8-byte slot
/// that holds the pre-normalization stack pointer and rounds up to the
/// 16-byte alignment the stub's `and rsp, -16` normalization establishes.
/// Both are derived from the admitted boundary plan here so the emitted
/// bytes and the composed stack demand read one contract-owned number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct X86_64DeriverStubCallFrame {
    pub outgoing_stack_bytes: u64,
    pub reserved_bytes: u64,
}

/// The deriver-owned stub shape for one admissible arrival context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct X86_64DeriverStubContext {
    pub context: ArrivalContextId,
    pub mechanism: X86_64ArrivalMechanism,
    pub error_code: X86_64ErrorCodeDisposition,
    /// Bytes hardware pushed at arrival: the iret frame plus the old SS/RSP
    /// pair on a stack switch and the architectural error-code word where
    /// present.
    pub hardware_frame_bytes: u64,
    /// Bytes the stub adds to normalize the frame: 8 for a synthesized error
    /// code, 0 when hardware pushed it.
    pub normalizing_bytes: u64,
    /// Bytes of the stub's register save area below the normalized frame.
    pub saved_area_bytes: u64,
    pub nesting: Preemption,
}

/// The complete deriver-owned entry/exit contract for one installed root.
/// `identity` is the installed occurrence the emitted stub occupies;
/// `saved_footprint` is the state the entry/exit code realizes under the
/// admitted plan's save/restore law; `exit` is the evidence the exit bytes
/// satisfy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct X86_64DeriverStub {
    pub identity: InstalledEntryFactIdentity,
    pub vector: u8,
    pub gate: X86_64GateKind,
    /// The machine state the stub's entry/exit code preserves: exactly the
    /// plan's saved-state law (register classes materialize as the push list;
    /// the rest ride the hardware frame the `iretq` exit consumes).
    pub saved_footprint: StateFootprintEvidence,
    /// The machine-state envelope the member's emitted body may transitively
    /// realize — exactly the plan's permitted transitive use, preserved by the
    /// stub's save area around it.
    pub member_body_envelope: StateFootprintEvidence,
    /// The member-call frame the stub reserves below the save area. Derived
    /// from the admitted boundary plan's outgoing stack-argument area plus
    /// the saved pre-normalization stack word; emission encodes exactly this
    /// reservation.
    pub member_call_frame: X86_64DeriverStubCallFrame,
    pub exit: ProviderExitRealization,
    pub contexts: Vec<X86_64DeriverStubContext>,
}

impl X86_64DeriverStub {
    /// Worst-case stack bytes the emitted stub adds below the delivered
    /// hardware frame before the member body's own stack demand begins:
    /// per-context error-code normalization and register save area, at most
    /// 15 bytes the `and rsp, -16` normalization can drop below the pushed
    /// save area, the anchored member-call frame reservation, and the member
    /// call's 8-byte pushed return address. This is the deriver-owned term a
    /// dedicated critical stack must carry in addition to the architectural
    /// arrival frame and the member's own terminal demand.
    pub fn peak_entry_overhead_bytes(&self) -> u64 {
        let context_peak = self
            .contexts
            .iter()
            .map(|context| context.normalizing_bytes + context.saved_area_bytes)
            .max()
            .unwrap_or(0);
        // 15 bytes of worst-case alignment normalization plus the call's
        // pushed return address are recipe constants of the emitted shape.
        context_peak + 15 + self.member_call_frame.reserved_bytes + 8
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedX86_64DeriverStub {
    stub: X86_64DeriverStub,
    non_authoritative_report_fingerprint: u64,
}

impl ValidatedX86_64DeriverStub {
    pub const fn stub(&self) -> &X86_64DeriverStub {
        &self.stub
    }

    /// Compact report/cache coordinate over the retained exact contract.
    pub const fn report_fingerprint(&self) -> u64 {
        self.non_authoritative_report_fingerprint
    }
}

/// Derive the compiler-owned entry/exit stub contract for one installed
/// x86-64 gate's member.
///
/// The stub saves exactly the machine state the admitted boundary plan's
/// save/restore law requires — general and vector register classes become the
/// push list, and the remaining classes ride the hardware frame the `iretq`
/// exit consumes. Every arrival context must carry the plan's declared
/// preemption and stack disposition; a context that disagrees means the
/// installed facts were built against a different boundary than the member
/// was admitted under.
pub fn derive_x86_64_entry_exit_stub(
    installed: &ValidatedX86_64InstalledHardwareEntryFacts,
    boundary: &ValidatedBoundaryEntryPlan,
) -> Result<ValidatedX86_64DeriverStub, PlanDiagnostic> {
    let facts = installed.facts();
    let identity = facts.identity;
    let plan = boundary.plan();

    if boundary.contract_report_fingerprint() != identity.boundary_plan_report_fingerprint
        || boundary.contract_commitment_digest() != identity.boundary_plan_commitment
    {
        return Err(PlanDiagnostic(
            "x86-64 deriver stub requires the installed facts' exact admitted boundary plan".into(),
        ));
    }
    if plan.call.entry_control != EntryControl::InterruptReturn {
        return Err(PlanDiagnostic(
            "x86-64 deriver entry/exit stub requires InterruptReturn entry control".into(),
        ));
    }
    if plan.state.initial_regime != MachineRegime::X86Long64 {
        return Err(PlanDiagnostic(
            "x86-64 deriver entry/exit stub requires the X86Long64 machine regime".into(),
        ));
    }
    if plan.state.stack != facts.boundary_stack {
        return Err(PlanDiagnostic(
            "x86-64 deriver stub's boundary stack disposition disagrees with the installed facts"
                .into(),
        ));
    }
    if facts.boundary_stack == EntryStack::ProviderSelected {
        return Err(PlanDiagnostic(
            "x86-64 deriver stub retains an unresolved provider-selected boundary stack".into(),
        ));
    }

    let saved_registers = stub_saved_registers(plan.state.saved_state);
    let saved_area_bytes = saved_registers
        .iter()
        .map(saved_register_area_bytes)
        .sum::<u64>();
    let saved_footprint =
        StateFootprintEvidence::new(RegisterSet::new(saved_registers), plan.state.saved_state);
    if saved_footprint.machine_state() != plan.state.saved_state {
        return Err(PlanDiagnostic(
            "x86-64 deriver stub cannot realize the boundary plan's saved-state law".into(),
        ));
    }

    let member_body_envelope = StateFootprintEvidence::new(
        RegisterSet::new(stub_saved_registers(plan.state.permitted_transitive_use)),
        plan.state.permitted_transitive_use,
    );
    if member_body_envelope.machine_state() != plan.state.permitted_transitive_use {
        return Err(PlanDiagnostic(
            "x86-64 deriver stub cannot realize the member body's machine-state envelope".into(),
        ));
    }

    let exit = ProviderExitRealization {
        control: EntryControl::InterruptReturn,
        restored_state: plan.state.restored_state,
    };

    let mut outgoing_stack_end = 0_u64;
    for parameter in &plan.call.parameters {
        for location in &parameter.locations {
            if let ValueLocation::Stack {
                stack_byte_offset,
                byte_size,
                ..
            } = location
            {
                outgoing_stack_end =
                    outgoing_stack_end.max(u64::from(*stack_byte_offset) + u64::from(*byte_size));
            }
        }
    }
    let outgoing_stack_bytes = outgoing_stack_end.next_multiple_of(8);
    let member_call_frame = X86_64DeriverStubCallFrame {
        outgoing_stack_bytes,
        reserved_bytes: (outgoing_stack_bytes + 8).next_multiple_of(16),
    };

    let mut contexts = Vec::with_capacity(facts.contexts.len());
    for context in &facts.contexts {
        if context.nesting != plan.state.preemption {
            return Err(PlanDiagnostic(format!(
                "x86-64 deriver stub arrival context 0x{:016x} carries preemption outside the admitted boundary plan",
                context.context.get()
            )));
        }
        let error_code = if x86_64_arrival_pushes_error_code(facts.vector, context) {
            X86_64ErrorCodeDisposition::HardwarePushed
        } else {
            X86_64ErrorCodeDisposition::StubSynthesized
        };
        contexts.push(X86_64DeriverStubContext {
            context: context.context,
            mechanism: context.mechanism,
            error_code,
            hardware_frame_bytes: x86_64_arrival_frame_bytes(facts.vector, context),
            normalizing_bytes: match error_code {
                X86_64ErrorCodeDisposition::HardwarePushed => 0,
                X86_64ErrorCodeDisposition::StubSynthesized => 8,
            },
            saved_area_bytes,
            nesting: context.nesting,
        });
    }

    let stub = X86_64DeriverStub {
        identity,
        vector: facts.vector,
        gate: facts.gate,
        saved_footprint,
        member_body_envelope,
        member_call_frame,
        exit,
        contexts,
    };
    let non_authoritative_report_fingerprint = stub_report_fingerprint(&stub);
    Ok(ValidatedX86_64DeriverStub {
        stub,
        non_authoritative_report_fingerprint,
    })
}

/// Save-area bytes one register occupies below the normalized frame: a
/// GPR-sized slot for word registers, a 16-byte slot for vector registers —
/// vector state cannot ride a qword push, so its reserve is the store slot
/// the emission must occupy rather than a push of its own.
fn saved_register_area_bytes(register: &MachineRegister) -> u64 {
    match register {
        MachineRegister::X86Xmm(_) | MachineRegister::Aarch64V(_) => 16,
        _ => 8,
    }
}

/// The stub's push list under the plan's saved-state law: every
/// general-purpose register class becomes all fifteen non-stack x86-64 GPRs
/// (the hardware frame, not the push list, carries RSP), and the vector class
/// becomes all sixteen XMM registers. Other state classes live in the
/// hardware frame or machine state and produce no registers.
fn stub_saved_registers(saved_state: MachineStateSet) -> Vec<MachineRegister> {
    let mut registers = Vec::new();
    if saved_state.contains_all(MachineStateSet::new([MachineState::GeneralRegisters])) {
        registers.extend([
            MachineRegister::X86Rax,
            MachineRegister::X86Rbx,
            MachineRegister::X86Rcx,
            MachineRegister::X86Rdx,
            MachineRegister::X86Rsi,
            MachineRegister::X86Rdi,
            MachineRegister::X86Rbp,
            MachineRegister::X86R8,
            MachineRegister::X86R9,
            MachineRegister::X86R10,
            MachineRegister::X86R11,
            MachineRegister::X86R12,
            MachineRegister::X86R13,
            MachineRegister::X86R14,
            MachineRegister::X86R15,
        ]);
    }
    if saved_state.contains_all(MachineStateSet::new([MachineState::VectorRegisters])) {
        registers.extend((0..16).map(MachineRegister::X86Xmm));
    }
    registers
}

fn stub_report_fingerprint(stub: &X86_64DeriverStub) -> u64 {
    let mut hash = Fnv1a::new();
    hash.u64(0x7838_365f_7374_7562); // "x86_stub"
    hash.u64(stub.identity.target_profile.get());
    hash.u64(stub.identity.artifact);
    hash.u64(stub.identity.installed_code);
    hash.u64(stub.identity.entry);
    hash.u64(stub.identity.entry_offset);
    hash.u64(u64::from(stub.vector));
    hash.u64(match stub.gate {
        X86_64GateKind::Interrupt => 0,
        X86_64GateKind::Trap => 1,
    });
    hash.u64(stub.saved_footprint.registers().as_slice().len() as u64);
    for register in stub.saved_footprint.registers().as_slice() {
        hash.bytes(format!("{register:?}").as_bytes());
    }
    hash.u64(u64::from(stub.saved_footprint.machine_state().bits()));
    hash.u64(u64::from(stub.member_body_envelope.machine_state().bits()));
    hash.u64(stub.member_body_envelope.registers().as_slice().len() as u64);
    hash.u64(match stub.exit.control {
        EntryControl::InterruptReturn => 0,
        _ => unreachable!("validated above"),
    });
    hash.u64(u64::from(stub.exit.restored_state.bits()));
    hash.u64(stub.member_call_frame.outgoing_stack_bytes);
    hash.u64(stub.member_call_frame.reserved_bytes);
    hash.u64(stub.contexts.len() as u64);
    for context in &stub.contexts {
        hash.u64(context.context.get());
        hash.u64(match context.mechanism {
            X86_64ArrivalMechanism::Exception => 0,
            X86_64ArrivalMechanism::ExternalInterrupt => 1,
            X86_64ArrivalMechanism::NonMaskableInterrupt => 2,
            X86_64ArrivalMechanism::SoftwareInterrupt => 3,
        });
        hash.u64(match context.error_code {
            X86_64ErrorCodeDisposition::HardwarePushed => 0,
            X86_64ErrorCodeDisposition::StubSynthesized => 1,
        });
        hash.u64(context.hardware_frame_bytes);
        hash.u64(context.normalizing_bytes);
        hash.u64(context.saved_area_bytes);
    }
    hash.finish()
}

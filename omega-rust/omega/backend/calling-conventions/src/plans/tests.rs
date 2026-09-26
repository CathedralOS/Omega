//! Fixtures shared by the plan tests.

mod entries_footprints_and_policies;
mod native_policies_and_syscalls;
mod parameter_and_aggregate_placement;
mod system_v_records_and_references;

use crate::plans::{
    BoundaryEntryPlan, CallSignature, CallingPolicy, EntryControl, EntryStack, MachineRegime,
    MachineRegister, MachineState, MachineStateSet, Preemption, RegisterSet, StatePlan, ValueShape,
    evaluate_call_plan,
};

fn integer_signature(parameter_count: usize) -> CallSignature {
    CallSignature {
        parameters: vec![ValueShape::integer(8, 8); parameter_count],
        result: Some(ValueShape::integer(8, 8)),
    }
}

fn strict_x86_entry() -> BoundaryEntryPlan {
    let mut call =
        evaluate_call_plan(CallingPolicy::MicrosoftX64, &integer_signature(1)).expect("call plan");
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

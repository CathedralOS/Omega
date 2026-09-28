//! Bounded physical-component bytes, never a complete boundary-policy record.

use crate::encoding::encode::declarations::{
    calling_policy_tag, encode_machine_register, encode_value_placement,
};
use crate::encoding::encode::encoder::Encoder;
use crate::encoding::{
    PACKAGE_PHYSICAL_CALLING_POLICY_VERSION, PHYSICAL_CALLING_POLICY_MAGIC,
    PackageReviewEncodingError,
};
use crate::record::{
    PackagePolicyEntryControl, PackagePolicyEntryStack, PackagePolicyMachineRegime,
    PackagePolicyMachineState, PackagePolicyMachineStateSet, PackagePolicyPhysicalCallingContract,
    PackagePolicyPreemption, PackagePolicyStatePlan, PackageReviewBoundaryCallingPolicy,
};

impl PackagePolicyPhysicalCallingContract {
    /// Versioned bytes for this physical facet only. No callback identities,
    /// evidence receipts, or acceptance authority are encoded.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, PackageReviewEncodingError> {
        let mut encoder = Encoder::policy_bounded(4 * 1024 * 1024);
        encoder.fixed_bytes(PHYSICAL_CALLING_POLICY_MAGIC);
        encoder.u16(PACKAGE_PHYSICAL_CALLING_POLICY_VERSION);
        encode_physical(&mut encoder, self)?;
        encoder.finish()
    }
}

pub(crate) fn encode_physical(
    encoder: &mut Encoder,
    physical: &PackagePolicyPhysicalCallingContract,
) -> Result<(), PackageReviewEncodingError> {
    if physical
        .ordinary_clobbers()
        .windows(2)
        .any(|pair| pair[0] >= pair[1])
    {
        return Err(PackageReviewEncodingError::new(
            "physical calling policy requires canonical register sets",
        ));
    }
    encoder.field("policy", |encoder| {
        let name = match physical.policy() {
            PackageReviewBoundaryCallingPolicy::MicrosoftX64 => "microsoft_x64",
            PackageReviewBoundaryCallingPolicy::SystemVAMD64 => "system_v_amd64",
            PackageReviewBoundaryCallingPolicy::Aapcs64 => "aapcs64",
            PackageReviewBoundaryCallingPolicy::LinuxSyscallX86_64 => "linux_syscall_x86_64",
            PackageReviewBoundaryCallingPolicy::LinuxSyscallAarch64 => "linux_syscall_aarch64",
        };
        encoder.tag(name, calling_policy_tag(physical.policy()));
        Ok(())
    })?;
    encoder.field("parameters", |encoder| {
        encoder.sequence(physical.parameters(), encode_value_placement)
    })?;
    encoder.field("result", |encoder| {
        encoder.option(physical.result(), encode_value_placement)
    })?;
    encoder.field("ordinary_clobbers", |encoder| {
        encoder.sequence(physical.ordinary_clobbers(), |encoder, register| {
            encoder.field("register", |encoder| {
                encode_machine_register(encoder, *register);
                Ok(())
            })?;
            Ok(())
        })
    })?;
    encoder.field("stack_alignment", |encoder| {
        encoder.u16(physical.stack_alignment());
        Ok(())
    })?;
    encoder.field("shadow_bytes", |encoder| {
        encoder.u16(physical.shadow_bytes());
        Ok(())
    })?;
    encoder.field("entry_control", |encoder| {
        match physical.entry_control() {
            PackagePolicyEntryControl::CallReturn => encoder.tag("call_return", 0),
            PackagePolicyEntryControl::SupervisorCall {
                number_register,
                immediate,
            } => {
                encoder.tag("supervisor_call", 1);
                encoder.field("number_register", |encoder| {
                    encode_machine_register(encoder, number_register);
                    Ok(())
                })?;
                encoder.field("immediate", |encoder| {
                    encoder.u16(immediate);
                    Ok(())
                })?;
            }
            PackagePolicyEntryControl::InterruptReturn => encoder.tag("interrupt_return", 2),
        };
        Ok(())
    })?;
    encoder.field("state", |encoder| encode_state(encoder, physical.state()))
}

fn encode_state(
    encoder: &mut Encoder,
    state: &PackagePolicyStatePlan,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field("initial_regime", |encoder| {
        match state.initial_regime() {
            PackagePolicyMachineRegime::X86Long64 => encoder.tag("x86_long64", 0),
            PackagePolicyMachineRegime::Aarch64A64 { exception_level } => {
                encoder.tag("aarch64_a64", 1);
                encoder.field("exception_level", |encoder| {
                    encoder.byte(exception_level);
                    Ok(())
                })?;
            }
        };
        Ok(())
    })?;
    encoder.field("interrupted_state", |encoder| {
        encode_state_set(encoder, state.interrupted_state())
    })?;
    encoder.field("saved_state", |encoder| {
        encode_state_set(encoder, state.saved_state())
    })?;
    encoder.field("restored_state", |encoder| {
        encode_state_set(encoder, state.restored_state())
    })?;
    encoder.field("permitted_transitive_use", |encoder| {
        encode_state_set(encoder, state.permitted_transitive_use())
    })?;
    encoder.field("stack", |encoder| {
        match state.stack() {
            PackagePolicyEntryStack::Interrupted => encoder.tag("interrupted", 0),
            PackagePolicyEntryStack::Dedicated { class } => {
                encoder.tag("dedicated", 1);
                encoder.field("class", |encoder| {
                    encoder.u16(class);
                    Ok(())
                })?;
            }
            PackagePolicyEntryStack::ProviderSelected => encoder.tag("provider_selected", 2),
        };
        Ok(())
    })?;
    encoder.field("preemption", |encoder| {
        match state.preemption() {
            PackagePolicyPreemption::NotApplicable => encoder.tag("not_applicable", 0),
            PackagePolicyPreemption::Masked => encoder.tag("masked", 1),
            PackagePolicyPreemption::Nestable { maximum_depth } => {
                encoder.tag("nestable", 2);
                encoder.field("maximum_depth", |encoder| {
                    encoder.u16(maximum_depth);
                    Ok(())
                })?;
            }
            PackagePolicyPreemption::ProviderDefined => encoder.tag("provider_defined", 3),
        };
        Ok(())
    })?;
    Ok(())
}

fn encode_state_set(
    encoder: &mut Encoder,
    states: &PackagePolicyMachineStateSet,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field("states", |encoder| {
        encoder.sequence(states.as_slice(), |encoder, state| {
            encoder.field("state", |encoder| {
                match state {
                    PackagePolicyMachineState::GeneralRegisters => {
                        encoder.tag("general_registers", 0)
                    }
                    PackagePolicyMachineState::VectorRegisters => {
                        encoder.tag("vector_registers", 1)
                    }
                    PackagePolicyMachineState::Flags => encoder.tag("flags", 2),
                    PackagePolicyMachineState::InstructionPointer => {
                        encoder.tag("instruction_pointer", 3)
                    }
                    PackagePolicyMachineState::StackPointer => encoder.tag("stack_pointer", 4),
                    PackagePolicyMachineState::SegmentState => encoder.tag("segment_state", 5),
                    PackagePolicyMachineState::ControlState => encoder.tag("control_state", 6),
                    PackagePolicyMachineState::DebugState => encoder.tag("debug_state", 7),
                    PackagePolicyMachineState::ExtendedState => encoder.tag("extended_state", 8),
                };
                Ok(())
            })?;
            Ok(())
        })
    })
}

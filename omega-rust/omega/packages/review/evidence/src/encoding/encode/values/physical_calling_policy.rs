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
    PackagePolicyPreemption, PackagePolicyStatePlan,
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
    encoder.byte(calling_policy_tag(physical.policy()));
    encoder.sequence(physical.parameters(), encode_value_placement)?;
    encoder.option(physical.result(), encode_value_placement)?;
    encoder.sequence(physical.ordinary_clobbers(), |encoder, register| {
        encode_machine_register(encoder, *register);
        Ok(())
    })?;
    encoder.u16(physical.stack_alignment());
    encoder.u16(physical.shadow_bytes());
    match physical.entry_control() {
        PackagePolicyEntryControl::CallReturn => encoder.byte(0),
        PackagePolicyEntryControl::SupervisorCall {
            number_register,
            immediate,
        } => {
            encoder.byte(1);
            encode_machine_register(encoder, number_register);
            encoder.u16(immediate);
        }
        PackagePolicyEntryControl::InterruptReturn => encoder.byte(2),
    }
    encode_state(encoder, physical.state())
}

fn encode_state(
    encoder: &mut Encoder,
    state: &PackagePolicyStatePlan,
) -> Result<(), PackageReviewEncodingError> {
    match state.initial_regime() {
        PackagePolicyMachineRegime::X86Long64 => encoder.byte(0),
        PackagePolicyMachineRegime::Aarch64A64 { exception_level } => {
            encoder.byte(1);
            encoder.byte(exception_level);
        }
    }
    encode_state_set(encoder, state.interrupted_state())?;
    encode_state_set(encoder, state.saved_state())?;
    encode_state_set(encoder, state.restored_state())?;
    encode_state_set(encoder, state.permitted_transitive_use())?;
    match state.stack() {
        PackagePolicyEntryStack::Interrupted => encoder.byte(0),
        PackagePolicyEntryStack::Dedicated { class } => {
            encoder.byte(1);
            encoder.u16(class);
        }
        PackagePolicyEntryStack::ProviderSelected => encoder.byte(2),
    }
    match state.preemption() {
        PackagePolicyPreemption::NotApplicable => encoder.byte(0),
        PackagePolicyPreemption::Masked => encoder.byte(1),
        PackagePolicyPreemption::Nestable { maximum_depth } => {
            encoder.byte(2);
            encoder.u16(maximum_depth);
        }
        PackagePolicyPreemption::ProviderDefined => encoder.byte(3),
    }
    Ok(())
}

fn encode_state_set(
    encoder: &mut Encoder,
    states: &PackagePolicyMachineStateSet,
) -> Result<(), PackageReviewEncodingError> {
    encoder.sequence(states.as_slice(), |encoder, state| {
        encoder.byte(match state {
            PackagePolicyMachineState::GeneralRegisters => 0,
            PackagePolicyMachineState::VectorRegisters => 1,
            PackagePolicyMachineState::Flags => 2,
            PackagePolicyMachineState::InstructionPointer => 3,
            PackagePolicyMachineState::StackPointer => 4,
            PackagePolicyMachineState::SegmentState => 5,
            PackagePolicyMachineState::ControlState => 6,
            PackagePolicyMachineState::DebugState => 7,
            PackagePolicyMachineState::ExtendedState => 8,
        });
        Ok(())
    })
}

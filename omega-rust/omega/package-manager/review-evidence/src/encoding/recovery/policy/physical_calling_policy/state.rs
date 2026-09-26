use super::{Error, Reader, placement::register};
use crate::record::{
    PackagePolicyEntryControl, PackagePolicyEntryStack, PackagePolicyMachineRegime,
    PackagePolicyMachineState, PackagePolicyMachineStateSet, PackagePolicyPreemption,
    PackagePolicyStatePlan,
};

pub(super) fn entry_control(reader: &mut Reader<'_>) -> Result<PackagePolicyEntryControl, Error> {
    Ok(match reader.byte()? {
        0 => PackagePolicyEntryControl::CallReturn,
        1 => PackagePolicyEntryControl::SupervisorCall {
            number_register: register(reader)?,
            immediate: reader.u16()?,
        },
        2 => PackagePolicyEntryControl::InterruptReturn,
        _ => return Err(Error::InvalidTag),
    })
}

pub(super) fn state_plan(reader: &mut Reader<'_>) -> Result<PackagePolicyStatePlan, Error> {
    Ok(PackagePolicyStatePlan {
        initial_regime: match reader.byte()? {
            0 => PackagePolicyMachineRegime::X86Long64,
            1 => PackagePolicyMachineRegime::Aarch64A64 {
                exception_level: reader.byte()?,
            },
            _ => return Err(Error::InvalidTag),
        },
        interrupted_state: state_set(reader)?,
        saved_state: state_set(reader)?,
        restored_state: state_set(reader)?,
        permitted_transitive_use: state_set(reader)?,
        stack: match reader.byte()? {
            0 => PackagePolicyEntryStack::Interrupted,
            1 => PackagePolicyEntryStack::Dedicated {
                class: reader.u16()?,
            },
            2 => PackagePolicyEntryStack::ProviderSelected,
            _ => return Err(Error::InvalidTag),
        },
        preemption: match reader.byte()? {
            0 => PackagePolicyPreemption::NotApplicable,
            1 => PackagePolicyPreemption::Masked,
            2 => PackagePolicyPreemption::Nestable {
                maximum_depth: reader.u16()?,
            },
            3 => PackagePolicyPreemption::ProviderDefined,
            _ => return Err(Error::InvalidTag),
        },
    })
}

fn state_set(reader: &mut Reader<'_>) -> Result<PackagePolicyMachineStateSet, Error> {
    let states = reader.sequence(1, |reader| {
        Ok(match reader.byte()? {
            0 => PackagePolicyMachineState::GeneralRegisters,
            1 => PackagePolicyMachineState::VectorRegisters,
            2 => PackagePolicyMachineState::Flags,
            3 => PackagePolicyMachineState::InstructionPointer,
            4 => PackagePolicyMachineState::StackPointer,
            5 => PackagePolicyMachineState::SegmentState,
            6 => PackagePolicyMachineState::ControlState,
            7 => PackagePolicyMachineState::DebugState,
            8 => PackagePolicyMachineState::ExtendedState,
            _ => return Err(Error::InvalidTag),
        })
    })?;
    PackagePolicyMachineStateSet::from_canonical(states).ok_or(Error::NonCanonicalEncoding)
}

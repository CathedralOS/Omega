#[cfg(test)]
mod tests;

use crate::capture::representation::physical_contract::{
    project_calling_policy, project_machine_register, project_value_placement,
};
use crate::record::{
    PackagePolicyEntryControl, PackagePolicyEntryStack, PackagePolicyMachineRegime,
    PackagePolicyMachineState, PackagePolicyMachineStateSet, PackagePolicyPhysicalCallingContract,
    PackagePolicyPreemption, PackagePolicyStatePlan,
};
use calling_conventions::{
    BoundaryEntryPlan, EntryControl, EntryStack, MachineRegime, MachineState, MachineStateSet,
    Preemption, ValidatedBoundaryEntryPlan,
};

impl PackagePolicyPhysicalCallingContract {
    /// Capture only the complete physical facet of a validated plan. The
    /// containing policy must separately retain exact requirement/target,
    /// native-parameter origins, and semantic callback materializations.
    /// No validator or native implementation receipt enters this value.
    pub fn from_validated_plan(plan: &ValidatedBoundaryEntryPlan) -> Self {
        project(plan.plan())
    }
}

fn project(plan: &BoundaryEntryPlan) -> PackagePolicyPhysicalCallingContract {
    let mut ordinary_clobbers: Vec<_> = plan
        .call
        .ordinary_clobbers
        .as_slice()
        .iter()
        .map(|register| project_machine_register(*register))
        .collect();
    ordinary_clobbers.sort_unstable();
    ordinary_clobbers.dedup();
    PackagePolicyPhysicalCallingContract {
        policy: project_calling_policy(plan.call.policy),
        parameters: plan
            .call
            .parameters
            .iter()
            .map(project_value_placement)
            .collect(),
        result: plan.call.result.as_ref().map(project_value_placement),
        ordinary_clobbers,
        stack_alignment: plan.call.stack_alignment,
        shadow_bytes: plan.call.shadow_bytes,
        entry_control: match plan.call.entry_control {
            EntryControl::CallReturn => PackagePolicyEntryControl::CallReturn,
            EntryControl::SupervisorCall {
                number_register,
                immediate,
            } => PackagePolicyEntryControl::SupervisorCall {
                number_register: project_machine_register(number_register),
                immediate,
            },
            EntryControl::InterruptReturn => PackagePolicyEntryControl::InterruptReturn,
        },
        state: PackagePolicyStatePlan {
            initial_regime: match plan.state.initial_regime {
                MachineRegime::X86Long64 => PackagePolicyMachineRegime::X86Long64,
                MachineRegime::Aarch64A64 { exception_level } => {
                    PackagePolicyMachineRegime::Aarch64A64 { exception_level }
                }
            },
            interrupted_state: state_set(plan.state.interrupted_state),
            saved_state: state_set(plan.state.saved_state),
            restored_state: state_set(plan.state.restored_state),
            permitted_transitive_use: state_set(plan.state.permitted_transitive_use),
            stack: match plan.state.stack {
                EntryStack::Interrupted => PackagePolicyEntryStack::Interrupted,
                EntryStack::Dedicated { class } => PackagePolicyEntryStack::Dedicated { class },
                EntryStack::ProviderSelected => PackagePolicyEntryStack::ProviderSelected,
            },
            preemption: match plan.state.preemption {
                Preemption::NotApplicable => PackagePolicyPreemption::NotApplicable,
                Preemption::Masked => PackagePolicyPreemption::Masked,
                Preemption::Nestable { maximum_depth } => {
                    PackagePolicyPreemption::Nestable { maximum_depth }
                }
                Preemption::ProviderDefined => PackagePolicyPreemption::ProviderDefined,
            },
        },
    }
}

fn state_set(states: MachineStateSet) -> PackagePolicyMachineStateSet {
    PackagePolicyMachineStateSet::new(
        [
            (
                MachineState::GeneralRegisters,
                PackagePolicyMachineState::GeneralRegisters,
            ),
            (
                MachineState::VectorRegisters,
                PackagePolicyMachineState::VectorRegisters,
            ),
            (MachineState::Flags, PackagePolicyMachineState::Flags),
            (
                MachineState::InstructionPointer,
                PackagePolicyMachineState::InstructionPointer,
            ),
            (
                MachineState::StackPointer,
                PackagePolicyMachineState::StackPointer,
            ),
            (
                MachineState::SegmentState,
                PackagePolicyMachineState::SegmentState,
            ),
            (
                MachineState::ControlState,
                PackagePolicyMachineState::ControlState,
            ),
            (
                MachineState::DebugState,
                PackagePolicyMachineState::DebugState,
            ),
            (
                MachineState::ExtendedState,
                PackagePolicyMachineState::ExtendedState,
            ),
        ]
        .into_iter()
        .filter_map(|(native, policy)| {
            states
                .contains_all(MachineStateSet::new([native]))
                .then_some(policy)
        }),
    )
}

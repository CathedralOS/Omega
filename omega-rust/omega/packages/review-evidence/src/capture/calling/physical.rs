// Physical calling-contract projection from a validated boundary entry plan. The
// containing policy separately retains exact requirement/target, native-parameter
// origins, and semantic callback materializations so no validator or native
// implementation receipt enters this value.

use crate::record::{
    PackagePolicyEntryControl, PackagePolicyMachineRegime, PackagePolicyMachineState,
    PackagePolicyMachineStateSet, PackagePolicyPhysicalCallingContract, PackagePolicyStatePlan,
    PackageReviewBoundaryValueClass, PackageReviewBoundaryValueLocation,
    PackageReviewBoundaryValuePlacement, PackageReviewBoundaryValueShape,
    PackageReviewIndirectPointerLocation, PackageReviewSystemVEightbyteClass,
};
use calling_conventions::{
    BoundaryEntryPlan, EntryControl, MachineRegime, MachineState, MachineStateSet,
    SystemVEightbyteClass, ValidatedBoundaryEntryPlan, ValueClass,
};

impl PackagePolicyPhysicalCallingContract {
    pub(crate) fn from_validated_plan(plan: &ValidatedBoundaryEntryPlan) -> Self {
        project(plan.plan())
    }
}

fn project(plan: &BoundaryEntryPlan) -> PackagePolicyPhysicalCallingContract {
    let mut ordinary_clobbers: Vec<_> = plan.call.ordinary_clobbers.as_slice().to_vec();
    ordinary_clobbers.sort_unstable();
    ordinary_clobbers.dedup();
    PackagePolicyPhysicalCallingContract {
        policy: plan.call.policy,
        parameters: plan.call.parameters.iter().map(value_placement).collect(),
        result: plan.call.result.as_ref().map(value_placement),
        ordinary_clobbers,
        stack_alignment: plan.call.stack_alignment,
        shadow_bytes: plan.call.shadow_bytes,
        entry_control: match plan.call.entry_control {
            EntryControl::CallReturn => PackagePolicyEntryControl::CallReturn,
            EntryControl::SupervisorCall {
                number_register,
                immediate,
            } => PackagePolicyEntryControl::SupervisorCall {
                number_register,
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
            stack: plan.state.stack,
            preemption: plan.state.preemption,
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

fn value_placement(
    placement: &calling_conventions::ValuePlacement,
) -> PackageReviewBoundaryValuePlacement {
    PackageReviewBoundaryValuePlacement {
        shape: PackageReviewBoundaryValueShape {
            class: match placement.shape.class {
                ValueClass::Integer => PackageReviewBoundaryValueClass::Integer,
                ValueClass::Float => PackageReviewBoundaryValueClass::Float,
                ValueClass::BorrowedReference => PackageReviewBoundaryValueClass::BorrowedReference,
                ValueClass::BorrowedScalarReference => {
                    PackageReviewBoundaryValueClass::BorrowedScalarReference
                }
                ValueClass::HomogeneousFloatAggregate { members } => {
                    PackageReviewBoundaryValueClass::HomogeneousFloatAggregate { members }
                }
                ValueClass::SystemVAggregate { first, second } => {
                    PackageReviewBoundaryValueClass::SystemVAggregate {
                        first: project_system_v_class(first),
                        second: project_system_v_class(second),
                    }
                }
            },
            byte_size: placement.shape.byte_size,
            alignment: placement.shape.alignment,
        },
        locations: placement
            .locations
            .iter()
            .map(|location| match *location {
                calling_conventions::ValueLocation::Register {
                    register,
                    value_byte_offset,
                    byte_size,
                } => PackageReviewBoundaryValueLocation::Register {
                    register,
                    value_byte_offset,
                    byte_size,
                },
                calling_conventions::ValueLocation::Stack {
                    stack_byte_offset,
                    value_byte_offset,
                    byte_size,
                    alignment,
                } => PackageReviewBoundaryValueLocation::Stack {
                    stack_byte_offset,
                    value_byte_offset,
                    byte_size,
                    alignment,
                },
                calling_conventions::ValueLocation::Indirect {
                    pointer,
                    copy_stack_byte_offset,
                    byte_size,
                    alignment,
                } => PackageReviewBoundaryValueLocation::Indirect {
                    pointer: match pointer {
                        calling_conventions::IndirectPointerLocation::Register(register) => {
                            PackageReviewIndirectPointerLocation::Register(register)
                        }
                        calling_conventions::IndirectPointerLocation::Stack {
                            stack_byte_offset,
                            alignment,
                        } => PackageReviewIndirectPointerLocation::Stack {
                            stack_byte_offset,
                            alignment,
                        },
                    },
                    copy_stack_byte_offset,
                    byte_size,
                    alignment,
                },
            })
            .collect(),
    }
}

fn project_system_v_class(class: SystemVEightbyteClass) -> PackageReviewSystemVEightbyteClass {
    match class {
        SystemVEightbyteClass::Integer => PackageReviewSystemVEightbyteClass::Integer,
        SystemVEightbyteClass::Sse => PackageReviewSystemVEightbyteClass::Sse,
    }
}

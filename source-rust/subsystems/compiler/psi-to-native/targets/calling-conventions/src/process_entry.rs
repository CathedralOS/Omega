//! Target-authored process-entry calling conventions, arrival shapes and stack guarantees.

use crate::{
    BoundaryEntryPlan, CallPlan, CallSignature, CallingPolicy, EntryControl, EntryStack,
    MachineRegime, MachineRegister, MachineState, MachineStateSet, Preemption, StatePlan,
    ValidatedBoundaryEntryPlan, ValueLocation, ValuePlacement, ValueShape,
    evaluate_ordinary_boundary_entry_plan, validate_boundary_entry_plan,
};
use target::{
    ProgramEntryCallingConvention as Convention, ProgramEntryPhysicalContractPackage as Package,
    TargetEntryStackGuarantee, TargetProfile,
};

const INTEGER32: ValueShape = ValueShape::integer(4, 4);
const INTEGER64: ValueShape = ValueShape::integer(8, 8);
const ADDRESS: ValueShape = ValueShape::integer(8, 8);

fn process_entry_state_plan(initial_regime: MachineRegime) -> StatePlan {
    StatePlan {
        initial_regime,
        interrupted_state: MachineStateSet::default(),
        saved_state: MachineStateSet::default(),
        restored_state: MachineStateSet::default(),
        permitted_transitive_use: MachineStateSet::new([
            MachineState::GeneralRegisters,
            MachineState::VectorRegisters,
            MachineState::Flags,
            MachineState::InstructionPointer,
            MachineState::StackPointer,
            // A returning import's floating-control envelope restores the
            // callee-preserved MXCSR/FPCR controls.
            MachineState::ControlState,
        ]),
        stack: EntryStack::ProviderSelected,
        preemption: Preemption::NotApplicable,
    }
}

pub fn exact_hosted_physical_boundary_entry_plan(
    package: target::ProgramEntryPhysicalContractPackage,
) -> Option<ValidatedBoundaryEntryPlan> {
    physical_arrival(package).replay()
}

/// Physical arrival requirements of one installed target entry package.
pub struct PhysicalArrival {
    package: Package,
    calling_convention: Convention,
    policy: CallingPolicy,
    parameter_shapes: &'static [ValueShape],
    result_shape: ValueShape,
    replay: Option<fn(CallingPolicy, &CallSignature) -> ValidatedBoundaryEntryPlan>,
    entry_stack_closure: Option<fn(TargetProfile) -> Result<TargetEntryStackGuarantee, String>>,
}

impl PhysicalArrival {
    pub const fn calling_convention(&self) -> Convention {
        self.calling_convention
    }

    pub const fn policy(&self) -> CallingPolicy {
        self.policy
    }

    pub fn parameter_count(&self) -> usize {
        self.parameter_shapes.len()
    }

    fn replay(&self) -> Option<ValidatedBoundaryEntryPlan> {
        let signature = CallSignature {
            parameters: self.parameter_shapes.to_vec(),
            result: Some(self.result_shape),
        };
        Some((self.replay?)(self.policy, &signature))
    }

    pub fn guaranteed_entry_stack(
        &self,
        owner: TargetProfile,
    ) -> Result<Option<TargetEntryStackGuarantee>, String> {
        match self.entry_stack_closure {
            Some(close) => close(owner).map(Some),
            None => Ok(None),
        }
    }
}

const fn row(
    package: Package,
    calling_convention: Convention,
    policy: CallingPolicy,
    parameter_shapes: &'static [ValueShape],
    result_shape: ValueShape,
    replay: Option<fn(CallingPolicy, &CallSignature) -> ValidatedBoundaryEntryPlan>,
    entry_stack_closure: Option<fn(TargetProfile) -> Result<TargetEntryStackGuarantee, String>>,
) -> PhysicalArrival {
    PhysicalArrival {
        package,
        calling_convention,
        policy,
        parameter_shapes,
        result_shape,
        replay,
        entry_stack_closure,
    }
}

const ARRIVALS: [PhysicalArrival; 6] = [
    row(
        Package::WindowsX64,
        Convention::MicrosoftX64,
        CallingPolicy::MicrosoftX64,
        &[],
        INTEGER32,
        Some(ordinary_arrival),
        None,
    ),
    row(
        Package::MacosX64,
        Convention::SystemVAMD64,
        CallingPolicy::SystemVAMD64,
        &[INTEGER32, ADDRESS, ADDRESS, ADDRESS],
        INTEGER32,
        Some(ordinary_arrival),
        None,
    ),
    row(
        Package::MacosArm64,
        Convention::Aapcs64,
        CallingPolicy::Aapcs64,
        &[INTEGER32, ADDRESS, ADDRESS, ADDRESS],
        INTEGER32,
        Some(ordinary_arrival),
        None,
    ),
    row(
        Package::LinuxX86_64,
        Convention::SystemVAMD64,
        CallingPolicy::SystemVAMD64,
        &[ADDRESS],
        INTEGER32,
        Some(linux_x86_64_arrival),
        None,
    ),
    row(
        Package::LinuxArm64,
        Convention::Aapcs64,
        CallingPolicy::Aapcs64,
        &[ADDRESS],
        INTEGER32,
        Some(linux_arm64_arrival),
        None,
    ),
    row(
        Package::UefiX64,
        Convention::MicrosoftX64,
        CallingPolicy::MicrosoftX64,
        &[ADDRESS, ADDRESS],
        INTEGER64,
        None,
        Some(close_uefi_entry_stack),
    ),
];

pub fn physical_arrival(package: Package) -> &'static PhysicalArrival {
    ARRIVALS
        .iter()
        .find(|arrival| arrival.package == package)
        .expect("every closed physical contract package has one arrival row")
}

fn ordinary_arrival(
    policy: CallingPolicy,
    signature: &CallSignature,
) -> ValidatedBoundaryEntryPlan {
    evaluate_ordinary_boundary_entry_plan(policy, signature)
        .expect("the closed target-authored physical entry plan must remain valid")
}

fn linux_x86_64_arrival(
    policy: CallingPolicy,
    signature: &CallSignature,
) -> ValidatedBoundaryEntryPlan {
    let plan = BoundaryEntryPlan {
        call: CallPlan {
            policy,
            parameters: vec![ValuePlacement {
                shape: ADDRESS,
                locations: vec![ValueLocation::Register {
                    register: MachineRegister::X86Rsp,
                    value_byte_offset: 0,
                    byte_size: 8,
                }],
            }],
            result: Some(ValuePlacement {
                shape: INTEGER32,
                locations: vec![ValueLocation::Register {
                    register: MachineRegister::X86Rdi,
                    value_byte_offset: 0,
                    byte_size: 4,
                }],
            }),
            callback_materializations: Vec::new(),
            ordinary_clobbers: policy.ordinary_clobbers(),
            stack_alignment: 16,
            shadow_bytes: 0,
            entry_control: EntryControl::SupervisorCall {
                number_register: MachineRegister::X86Rax,
                immediate: 231,
            },
        },
        state: process_entry_state_plan(MachineRegime::X86Long64),
    };
    validate_boundary_entry_plan(plan, signature)
        .expect("the closed target-authored Linux x86-64 physical entry plan must remain valid")
}

fn linux_arm64_arrival(
    policy: CallingPolicy,
    signature: &CallSignature,
) -> ValidatedBoundaryEntryPlan {
    let plan = BoundaryEntryPlan {
        call: CallPlan {
            policy,
            parameters: vec![ValuePlacement {
                shape: ADDRESS,
                locations: vec![ValueLocation::Stack {
                    stack_byte_offset: 0,
                    value_byte_offset: 0,
                    byte_size: 8,
                    alignment: 8,
                }],
            }],
            result: Some(ValuePlacement {
                shape: INTEGER32,
                locations: vec![ValueLocation::Register {
                    register: MachineRegister::Aarch64X(0),
                    value_byte_offset: 0,
                    byte_size: 4,
                }],
            }),
            callback_materializations: Vec::new(),
            ordinary_clobbers: policy.ordinary_clobbers(),
            stack_alignment: 16,
            shadow_bytes: 0,
            entry_control: EntryControl::SupervisorCall {
                number_register: MachineRegister::Aarch64X(8),
                immediate: 94,
            },
        },
        state: process_entry_state_plan(MachineRegime::Aarch64A64 { exception_level: 0 }),
    };
    validate_boundary_entry_plan(plan, signature)
        .expect("the closed target-authored Linux ARM64 physical entry plan must remain valid")
}

fn close_uefi_entry_stack(owner: TargetProfile) -> Result<TargetEntryStackGuarantee, String> {
    let application = target::TargetSemantics::guaranteed_entry_stack::<target::UefiX86_64>(owner)
        .map_err(|error| error.to_string())?;
    let guarantee =
        target::TargetSemantics::close_guaranteed_entry_stack::<target::UefiX86_64>(owner)
            .map_err(|error| error.to_string())?;
    if guarantee.application() != &application {
        return Err(
            "physical UEFI entry contract target-stack application and numeric closure drifted"
                .into(),
        );
    }
    Ok(guarantee)
}

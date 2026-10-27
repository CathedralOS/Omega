//! The closed physical arrival of each admitted contract package.
//!
//! One row per package names the authored calling convention, the exact
//! parameter and result shapes, the replay that independently reconstructs its
//! plan, and — where the authored contract publishes one — the numeric
//! entry-stack closure. Contract construction, the hosted replay, and the
//! image-side contract row all read this table instead of re-deriving the same
//! per-target association.

use calling_conventions::{
    BoundaryEntryPlan, CallPlan, CallSignature, CallingPolicy, EntryControl, MachineRegime,
    MachineRegister, ValidatedBoundaryEntryPlan, ValueLocation, ValuePlacement, ValueShape,
    evaluate_ordinary_boundary_entry_plan, validate_boundary_entry_plan,
};
use target::{
    ProgramEntryCallingConvention as Convention, ProgramEntryPhysicalContractPackage as Package,
    TargetEntryStackGuarantee, TargetProfile,
};

const INTEGER32: ValueShape = ValueShape::integer(4, 4);
const INTEGER64: ValueShape = ValueShape::integer(8, 8);
const ADDRESS: ValueShape = ValueShape::integer(8, 8);

pub(super) struct PhysicalArrival {
    pub(super) package: Package,
    /// Calling convention the package's own slot declaration must name.
    pub(super) calling_convention: Convention,
    pub(super) policy: CallingPolicy,
    parameter_shapes: &'static [ValueShape],
    result_shape: ValueShape,
    /// Independent replay of this arrival. `None` where no hosted replay exists:
    /// the UEFI contract's plan is authored and admitted by provider planning,
    /// not reconstructed here.
    replay: Option<fn(CallingPolicy, &CallSignature) -> ValidatedBoundaryEntryPlan>,
    /// Numeric entry-stack closure where the authored contract publishes one.
    entry_stack_closure: Option<fn(TargetProfile) -> Result<TargetEntryStackGuarantee, String>>,
}

impl PhysicalArrival {
    pub(super) fn parameter_count(&self) -> usize {
        self.parameter_shapes.len()
    }

    /// Reconstruct and validate this arrival's boundary entry plan. The hosted
    /// rows are the authored policy's own evaluator; the two Linux rows are a
    /// dedicated supervisor-call surface, so they own their placement literally.
    pub(super) fn replay(&self) -> Option<ValidatedBoundaryEntryPlan> {
        let signature = CallSignature {
            parameters: self.parameter_shapes.to_vec(),
            result: Some(self.result_shape),
        };
        Some((self.replay?)(self.policy, &signature))
    }

    pub(super) fn guaranteed_entry_stack(
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

/// Every closed arrival, in package identity order.
const ARRIVALS: [PhysicalArrival; 6] = [
    // PE AddressOfEntryPoint under Microsoft x64: no contractual register input,
    // and the returned u32 is the process exit code.
    row(
        Package::WindowsX64,
        Convention::MicrosoftX64,
        CallingPolicy::MicrosoftX64,
        &[],
        INTEGER32,
        Some(ordinary_arrival),
        None,
    ),
    // dyld `appMain`: argc, argv, envp and apple in registers, status in eax.
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
    // The kernel enters with rsp at the initial process-stack image and
    // completes through `exit_group`; no ordinary call-return surface exists.
    row(
        Package::LinuxX86_64,
        Convention::SystemVAMD64,
        CallingPolicy::SystemVAMD64,
        &[ADDRESS],
        INTEGER32,
        Some(linux_x86_64_arrival),
        None,
    ),
    // The kernel enters with the machine stack pointer at the initial
    // process-stack image; the argument count is its head word.
    row(
        Package::LinuxArm64,
        Convention::Aapcs64,
        CallingPolicy::Aapcs64,
        &[ADDRESS],
        INTEGER32,
        Some(linux_arm64_arrival),
        None,
    ),
    // Firmware hands the image handle and system table to the authored UEFI
    // entry under Microsoft x64 and receives an `EfiStatus` u64; the closed
    // stack guarantee comes from the target's own symbolic application.
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

/// The row for one closed contract package. Every variant has exactly one.
pub(super) fn physical_arrival(package: Package) -> &'static PhysicalArrival {
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
        state: super::process_entry_state_plan(MachineRegime::X86Long64),
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
        state: super::process_entry_state_plan(MachineRegime::Aarch64A64 { exception_level: 0 }),
    };
    validate_boundary_entry_plan(plan, signature)
        .expect("the closed target-authored Linux ARM64 physical entry plan must remain valid")
}

/// Close the UEFI entry-stack guarantee from the target's own symbolic
/// application, refusing an application and a numeric closure that disagree.
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

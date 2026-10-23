//! Exact Linux ARM64 replay owned beside the physical entry-plan carrier.

use calling_conventions::{
    BoundaryEntryPlan, CallPlan, CallSignature, CallingPolicy, EntryControl, MachineRegime,
    MachineRegister, ValidatedBoundaryEntryPlan, ValueLocation, ValuePlacement, ValueShape,
    validate_boundary_entry_plan,
};

const LINUX_ARM64_TARGET_PACKAGE_SOURCE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../../../source/library/std/targets/linux_arm64/entry.omg"
));

/// Canonical checked-tree overload identity for `LinuxArm64PhysicalEntry::enter`.
pub const LINUX_ARM64_PHYSICAL_REQUIREMENT_IDENTITY: &str = concat!(
    "named-callable(path(LinuxArm64PhysicalEntry::enter),parameters(",
    "parameter\\(self\\(no\\)\\,mutable\\(no\\)\\,const\\(no\\)\\,",
    "named\\(name\\(u64\\)\\)\\)",
    "),result-dispatch())",
);
/// Canonical normalized type identity of the `argc` arrival count delivered as
/// the head word of the initial process-stack image.
pub const LINUX_ARM64_U64_TYPE_IDENTITY: &str = "named(name(u64))";
/// Canonical normalized type identity of the process-completion status.
pub const LINUX_ARM64_I32_TYPE_IDENTITY: &str = "named(name(i32))";

/// Strong commitment to the exact closed Linux ARM64 target-package source
/// compiled into this plan owner.
pub fn exact_linux_arm64_physical_contract_package_source_digest()
-> super::ProgramEntryPhysicalContractPackageSourceDigest {
    super::ProgramEntryPhysicalContractPackageSourceDigest::from_package_source(
        target::ProgramEntryPhysicalContractPackage::LinuxArm64,
        LINUX_ARM64_TARGET_PACKAGE_SOURCE,
    )
}

/// Independently reconstruct and validate the exact plan authored by the
/// closed Linux ARM64 target package. The kernel enters a static ELF
/// executable with the machine stack pointer at the initial process-stack
/// image and supplies no argument registers and no return continuation, so
/// the authored `LinuxArm64` policy is a dedicated physical surface rather
/// than the ordinary AAPCS64 evaluator: the lone `u64` input is the argument
/// count delivered as the image's head stack word, and the `i32` result
/// completes through `exit_group` (x8 = 94) with status in w0. Only that
/// arrival is repeated here: the volatile bank belongs to the policy
/// (`CallingPolicy::Aapcs64::ordinary_clobbers`), and the bridge may also
/// observe the incoming stack pointer before switching to its private stack.
pub fn exact_linux_arm64_physical_boundary_entry_plan() -> ValidatedBoundaryEntryPlan {
    let signature = CallSignature {
        parameters: vec![ValueShape::integer(8, 8)],
        result: Some(ValueShape::integer(4, 4)),
    };
    let plan = BoundaryEntryPlan {
        call: CallPlan {
            policy: calling_conventions::CallingPolicy::Aapcs64,
            parameters: vec![ValuePlacement {
                shape: ValueShape::integer(8, 8),
                locations: vec![ValueLocation::Stack {
                    stack_byte_offset: 0,
                    value_byte_offset: 0,
                    byte_size: 8,
                    alignment: 8,
                }],
            }],
            result: Some(ValuePlacement {
                shape: ValueShape::integer(4, 4),
                locations: vec![ValueLocation::Register {
                    register: MachineRegister::Aarch64X(0),
                    value_byte_offset: 0,
                    byte_size: 4,
                }],
            }),
            callback_materializations: Vec::new(),
            ordinary_clobbers: CallingPolicy::Aapcs64.ordinary_clobbers(),
            stack_alignment: 16,
            shadow_bytes: 0,
            entry_control: EntryControl::SupervisorCall {
                number_register: MachineRegister::Aarch64X(8),
                immediate: 94,
            },
        },
        state: super::process_entry_state_plan(MachineRegime::Aarch64A64 { exception_level: 0 }),
    };
    validate_boundary_entry_plan(plan, &signature)
        .expect("the closed target-authored Linux ARM64 physical entry plan must remain valid")
}

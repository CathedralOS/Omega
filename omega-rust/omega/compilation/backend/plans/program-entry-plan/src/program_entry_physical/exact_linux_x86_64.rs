//! Exact Linux x86-64 replay owned beside the physical entry-plan carrier.

use calling_conventions::{
    BoundaryEntryPlan, CallPlan, CallSignature, CallingPolicy, EntryControl, MachineRegime,
    MachineRegister, ValidatedBoundaryEntryPlan, ValueLocation, ValuePlacement, ValueShape,
    validate_boundary_entry_plan,
};

const LINUX_X86_64_TARGET_PACKAGE_SOURCE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../../../source/library/std/targets/linux_x86_64/entry.omg"
));

/// Canonical checked-tree overload identity for `LinuxX86_64PhysicalEntry::enter`.
pub const LINUX_X86_64_PHYSICAL_REQUIREMENT_IDENTITY: &str = concat!(
    "named-callable(path(LinuxX86_64PhysicalEntry::enter),parameters(",
    "parameter\\(self\\(no\\)\\,mutable\\(no\\)\\,const\\(no\\)\\,",
    "named\\(name\\(addr\\)\\)\\)",
    "),result-dispatch())",
);
/// Canonical normalized type identity of the `initial_stack` arrival address.
pub const LINUX_X86_64_ADDRESS_TYPE_IDENTITY: &str = "named(name(addr))";
/// Canonical normalized type identity of the process-completion status.
pub const LINUX_X86_64_I32_TYPE_IDENTITY: &str = "named(name(i32))";

/// Strong commitment to the exact closed Linux x86-64 target-package source
/// compiled into this plan owner.
pub fn exact_linux_x86_64_physical_contract_package_source_digest()
-> super::ProgramEntryPhysicalContractPackageSourceDigest {
    super::ProgramEntryPhysicalContractPackageSourceDigest::from_package_source(
        target::ProgramEntryPhysicalContractPackage::LinuxX86_64,
        LINUX_X86_64_TARGET_PACKAGE_SOURCE,
    )
}

/// Independently reconstruct and validate the exact plan authored by the
/// closed Linux x86-64 target package. The kernel enters a static ELF
/// executable with rsp holding the initial process-stack image and supplies
/// no return continuation, so the authored `LinuxX86_64` policy is a
/// dedicated physical surface rather than the ordinary System V evaluator:
/// the lone `addr` input is the arriving stack pointer itself, and the `i32`
/// result completes through `exit_group` (rax = 231) with status in edi. Only
/// that arrival is repeated here: the volatile bank belongs to the policy
/// (`CallingPolicy::SystemVAMD64::ordinary_clobbers`), and the bridge may
/// also observe the incoming stack pointer before it switches to its private
/// stack.
pub fn exact_linux_x86_64_physical_boundary_entry_plan() -> ValidatedBoundaryEntryPlan {
    let signature = CallSignature {
        parameters: vec![ValueShape::integer(8, 8)],
        result: Some(ValueShape::integer(4, 4)),
    };
    let plan = BoundaryEntryPlan {
        call: CallPlan {
            policy: calling_conventions::CallingPolicy::SystemVAMD64,
            parameters: vec![ValuePlacement {
                shape: ValueShape::integer(8, 8),
                locations: vec![ValueLocation::Register {
                    register: MachineRegister::X86Rsp,
                    value_byte_offset: 0,
                    byte_size: 8,
                }],
            }],
            result: Some(ValuePlacement {
                shape: ValueShape::integer(4, 4),
                locations: vec![ValueLocation::Register {
                    register: MachineRegister::X86Rdi,
                    value_byte_offset: 0,
                    byte_size: 4,
                }],
            }),
            callback_materializations: Vec::new(),
            ordinary_clobbers: CallingPolicy::SystemVAMD64.ordinary_clobbers(),
            stack_alignment: 16,
            shadow_bytes: 0,
            entry_control: EntryControl::SupervisorCall {
                number_register: MachineRegister::X86Rax,
                immediate: 231,
            },
        },
        state: super::process_entry_state_plan(MachineRegime::X86Long64),
    };
    validate_boundary_entry_plan(plan, &signature)
        .expect("the closed target-authored Linux x86-64 physical entry plan must remain valid")
}

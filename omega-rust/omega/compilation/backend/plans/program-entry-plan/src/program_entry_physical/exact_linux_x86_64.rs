//! Exact Linux x86-64 replay owned beside the physical entry-plan carrier.

use calling_conventions::{
    BoundaryEntryPlan, CallPlan, CallSignature, EntryControl, EntryStack, MachineRegime,
    MachineRegister, MachineState, MachineStateSet, Preemption, RegisterSet, StatePlan,
    ValidatedBoundaryEntryPlan, ValueLocation, ValuePlacement, ValueShape,
    validate_boundary_entry_plan,
};

use super::ProgramEntryPhysicalContractPlan;

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

/// System V AMD64 caller-saved bank: the nine integer argument/return
/// registers plus all sixteen vector registers. The bridge may also observe
/// the incoming stack pointer before switching to its private stack.
const LINUX_X86_64_ORDINARY_CLOBBERS: [MachineRegister; 25] = [
    MachineRegister::X86Rax,
    MachineRegister::X86Rcx,
    MachineRegister::X86Rdx,
    MachineRegister::X86Rsi,
    MachineRegister::X86Rdi,
    MachineRegister::X86R8,
    MachineRegister::X86R9,
    MachineRegister::X86R10,
    MachineRegister::X86R11,
    MachineRegister::X86Xmm(0),
    MachineRegister::X86Xmm(1),
    MachineRegister::X86Xmm(2),
    MachineRegister::X86Xmm(3),
    MachineRegister::X86Xmm(4),
    MachineRegister::X86Xmm(5),
    MachineRegister::X86Xmm(6),
    MachineRegister::X86Xmm(7),
    MachineRegister::X86Xmm(8),
    MachineRegister::X86Xmm(9),
    MachineRegister::X86Xmm(10),
    MachineRegister::X86Xmm(11),
    MachineRegister::X86Xmm(12),
    MachineRegister::X86Xmm(13),
    MachineRegister::X86Xmm(14),
    MachineRegister::X86Xmm(15),
];

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
/// result completes through `exit_group` (rax = 231) with status in edi.
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
            ordinary_clobbers: RegisterSet::new(LINUX_X86_64_ORDINARY_CLOBBERS),
            stack_alignment: 16,
            shadow_bytes: 0,
            entry_control: EntryControl::SupervisorCall {
                number_register: MachineRegister::X86Rax,
                immediate: 231,
            },
        },
        state: StatePlan {
            initial_regime: MachineRegime::X86Long64,
            interrupted_state: MachineStateSet::default(),
            saved_state: MachineStateSet::default(),
            restored_state: MachineStateSet::default(),
            permitted_transitive_use: MachineStateSet::new([
                MachineState::GeneralRegisters,
                MachineState::VectorRegisters,
                MachineState::Flags,
            ]),
            stack: EntryStack::ProviderSelected,
            preemption: Preemption::NotApplicable,
        },
    };
    validate_boundary_entry_plan(plan, &signature)
        .expect("the closed target-authored Linux x86-64 physical entry plan must remain valid")
}

impl ProgramEntryPhysicalContractPlan {
    /// Replay the complete target-owned Linux x86-64 physical-entry contract.
    /// Constructor compatibility remains deliberately broader for synthetic
    /// compiler fixtures; runtime custody must use this exact verdict. The
    /// authored contract retains no numeric entry-stack guarantee, so this
    /// verdict rejects any injected closure rather than comparing one.
    pub fn matches_exact_linux_x86_64_physical_contract(&self) -> bool {
        let expected = exact_linux_x86_64_physical_boundary_entry_plan();
        self.target_slot == target::TargetProfile::LinuxX64.program_entry_slot()
            && self.target_package == target::ProgramEntryPhysicalContractPackage::LinuxX86_64
            && self.target_package_source_digest
                == exact_linux_x86_64_physical_contract_package_source_digest()
            && self.requirement_identity == LINUX_X86_64_PHYSICAL_REQUIREMENT_IDENTITY
            && self.parameter_type_identities.len() == 1
            && self.parameter_type_identities[0] == LINUX_X86_64_ADDRESS_TYPE_IDENTITY
            && self.result_type_identity == LINUX_X86_64_I32_TYPE_IDENTITY
            && self.calling_plan_report_fingerprint == expected.contract_report_fingerprint()
            && &self.boundary_entry_plan == expected.plan()
            && self.guaranteed_entry_stack.is_none()
    }
}

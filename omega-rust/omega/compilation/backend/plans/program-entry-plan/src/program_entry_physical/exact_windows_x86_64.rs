//! Exact Windows x86-64 replay owned beside the physical entry-plan carrier.

use calling_conventions::{
    BoundaryEntryPlan, CallPlan, CallSignature, EntryControl, EntryStack, MachineRegime,
    MachineRegister, MachineState, MachineStateSet, Preemption, RegisterSet, StatePlan,
    ValidatedBoundaryEntryPlan, ValueLocation, ValuePlacement, ValueShape,
    validate_boundary_entry_plan,
};

use super::ProgramEntryPhysicalContractPlan;

const WINDOWS_X86_64_TARGET_PACKAGE_SOURCE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../../../source/library/std/targets/windows_x86_64/entry.omg"
));

/// Canonical checked-tree overload identity for `WindowsProcessEntry::enter`.
pub const WINDOWS_X86_64_PHYSICAL_REQUIREMENT_IDENTITY: &str =
    "named-callable(path(WindowsProcessEntry::enter),parameters(),result-dispatch())";
/// Canonical normalized type identity of the process-completion status.
pub const WINDOWS_X86_64_U32_TYPE_IDENTITY: &str = "named(name(u32))";

/// Microsoft x64 caller-saved bank: the seven volatile integer registers plus
/// the six volatile vector registers.
const WINDOWS_X86_64_ORDINARY_CLOBBERS: [MachineRegister; 13] = [
    MachineRegister::X86Rax,
    MachineRegister::X86Rcx,
    MachineRegister::X86Rdx,
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
];

/// Strong commitment to the exact closed Windows x86-64 target-package source
/// compiled into this plan owner.
pub fn exact_windows_x86_64_physical_contract_package_source_digest()
-> super::ProgramEntryPhysicalContractPackageSourceDigest {
    super::ProgramEntryPhysicalContractPackageSourceDigest::from_package_source(
        target::ProgramEntryPhysicalContractPackage::WindowsX64,
        WINDOWS_X86_64_TARGET_PACKAGE_SOURCE,
    )
}

/// Independently reconstruct and validate the exact plan authored by the
/// closed Windows x86-64 target package. The loader enters a PE32+ executable
/// at its AddressOfEntryPoint under the Microsoft x64 convention: no register
/// carries contractual input at process arrival, the stack is 16-byte aligned
/// with thirty-two bytes of shadow space, and the `u32` result returns in eax
/// as the process exit code.
pub fn exact_windows_x86_64_physical_boundary_entry_plan() -> ValidatedBoundaryEntryPlan {
    let signature = CallSignature {
        parameters: Vec::new(),
        result: Some(ValueShape::integer(4, 4)),
    };
    let plan = BoundaryEntryPlan {
        call: CallPlan {
            policy: calling_conventions::CallingPolicy::MicrosoftX64,
            parameters: Vec::new(),
            result: Some(ValuePlacement {
                shape: ValueShape::integer(4, 4),
                locations: vec![ValueLocation::Register {
                    register: MachineRegister::X86Rax,
                    value_byte_offset: 0,
                    byte_size: 4,
                }],
            }),
            callback_materializations: Vec::new(),
            ordinary_clobbers: RegisterSet::new(WINDOWS_X86_64_ORDINARY_CLOBBERS),
            stack_alignment: 16,
            shadow_bytes: 32,
            entry_control: EntryControl::CallReturn,
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
        .expect("the closed target-authored Windows x86-64 physical entry plan must remain valid")
}

impl ProgramEntryPhysicalContractPlan {
    /// Replay the complete target-owned Windows x86-64 physical-entry
    /// contract. Constructor compatibility remains deliberately broader for
    /// synthetic compiler fixtures; runtime custody must use this exact
    /// verdict. The authored contract retains no numeric entry-stack
    /// guarantee, so this verdict rejects any injected closure rather than
    /// comparing one.
    pub fn matches_exact_windows_x86_64_physical_contract(&self) -> bool {
        let expected = exact_windows_x86_64_physical_boundary_entry_plan();
        self.target_slot == target::TargetProfile::WindowsX64.program_entry_slot()
            && self.target_package == target::ProgramEntryPhysicalContractPackage::WindowsX64
            && self.target_package_source_digest
                == exact_windows_x86_64_physical_contract_package_source_digest()
            && self.requirement_identity == WINDOWS_X86_64_PHYSICAL_REQUIREMENT_IDENTITY
            && self.parameter_type_identities.is_empty()
            && self.result_type_identity == WINDOWS_X86_64_U32_TYPE_IDENTITY
            && self.calling_plan_report_fingerprint == expected.contract_report_fingerprint()
            && &self.boundary_entry_plan == expected.plan()
            && self.guaranteed_entry_stack.is_none()
    }
}

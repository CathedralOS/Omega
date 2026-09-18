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
    "/../../../../../source/library/std/targets/windows_x86_64/entry.omg"
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

#[cfg(test)]
mod tests {
    use super::{
        ProgramEntryPhysicalContractPlan, WINDOWS_X86_64_PHYSICAL_REQUIREMENT_IDENTITY,
        WINDOWS_X86_64_U32_TYPE_IDENTITY, exact_windows_x86_64_physical_boundary_entry_plan,
        exact_windows_x86_64_physical_contract_package_source_digest,
    };
    use crate::ProgramEntryPhysicalContractPackageSourceDigest;

    fn exact_contract() -> ProgramEntryPhysicalContractPlan {
        let package = target::ProgramEntryPhysicalContractPackage::WindowsX64;
        let calling_plan = exact_windows_x86_64_physical_boundary_entry_plan();
        ProgramEntryPhysicalContractPlan::new(
            target::TargetProfile::WindowsX64.program_entry_slot(),
            WINDOWS_X86_64_PHYSICAL_REQUIREMENT_IDENTITY.into(),
            package,
            exact_windows_x86_64_physical_contract_package_source_digest(),
            1,
            Vec::new(),
            WINDOWS_X86_64_U32_TYPE_IDENTITY.into(),
            calling_plan.contract_report_fingerprint(),
            calling_plan.plan().clone(),
        )
        .expect("exact physical contract")
    }

    #[test]
    fn exact_runtime_contract_replays_all_owned_fields() {
        let exact = exact_contract();
        assert!(exact.matches_exact_windows_x86_64_physical_contract());
        assert!(exact.guaranteed_entry_stack().is_none());
        assert!(exact.guaranteed_entry_stack_application().is_none());

        let mut requirement_drift = exact.clone();
        requirement_drift.requirement_identity =
            "named-callable(path(ProgramStorageEntry::enter),parameters(),result-dispatch())"
                .into();
        assert!(!requirement_drift.matches_exact_windows_x86_64_physical_contract());

        let mut parameter_drift = exact.clone();
        parameter_drift
            .parameter_type_identities
            .push("named(name(u32))".into());
        assert!(!parameter_drift.matches_exact_windows_x86_64_physical_contract());

        let mut result_drift = exact.clone();
        result_drift.result_type_identity = "named(name(Unit))".into();
        assert!(!result_drift.matches_exact_windows_x86_64_physical_contract());

        let mut source_drift = exact.clone();
        source_drift.target_package_source_digest =
            ProgramEntryPhysicalContractPackageSourceDigest::from_package_source(
                target::ProgramEntryPhysicalContractPackage::WindowsX64,
                b"substituted target package",
            );
        assert!(!source_drift.matches_exact_windows_x86_64_physical_contract());

        let mut placement_drift = exact;
        placement_drift
            .boundary_entry_plan
            .call
            .result
            .as_mut()
            .expect("loader arrival completes u32")
            .locations[0] = calling_conventions::ValueLocation::Register {
            register: calling_conventions::MachineRegister::X86Rcx,
            value_byte_offset: 0,
            byte_size: 4,
        };
        assert!(!placement_drift.matches_exact_windows_x86_64_physical_contract());
    }

    #[test]
    fn physical_plan_is_exact_loader_arrival_shape() {
        let plan = exact_windows_x86_64_physical_boundary_entry_plan();
        let call = &plan.plan().call;
        assert_eq!(
            call.policy,
            calling_conventions::CallingPolicy::MicrosoftX64
        );
        assert!(
            call.parameters.is_empty(),
            "loader arrival carries no contractual inputs"
        );
        let result = call.result.as_ref().expect("loader arrival completes u32");
        assert_eq!(
            result.locations.as_slice(),
            &[calling_conventions::ValueLocation::Register {
                register: calling_conventions::MachineRegister::X86Rax,
                value_byte_offset: 0,
                byte_size: 4,
            }]
        );
        assert_eq!(
            call.entry_control,
            calling_conventions::EntryControl::CallReturn
        );
        assert_eq!(call.stack_alignment, 16);
        assert_eq!(call.shadow_bytes, 32);
        assert_eq!(
            plan.plan().state.stack,
            calling_conventions::EntryStack::ProviderSelected
        );
        assert_eq!(
            plan.plan().state.initial_regime,
            calling_conventions::MachineRegime::X86Long64
        );
    }
}

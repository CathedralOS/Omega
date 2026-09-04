//! Exact UEFI x64 replay owned beside the physical entry-plan carrier.

use omega_calling_conventions::{
    BoundaryEntryPlan, CallPlan, CallSignature, CallingPolicy, EntryControl, EntryStack,
    MachineRegime, MachineRegister, MachineState, MachineStateSet, Preemption, RegisterSet,
    StatePlan, ValidatedBoundaryEntryPlan, ValueLocation, ValuePlacement, ValueShape,
    validate_boundary_entry_plan,
};

use super::ProgramEntryPhysicalContractPlan;

const UEFI_X64_TARGET_PACKAGE_SOURCE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../../source/library/std/targets/uefi_x86_64/entry.omg"
));

/// Canonical checked-tree overload identity for `UefiPhysicalEntry::enter`.
pub const UEFI_X64_PHYSICAL_REQUIREMENT_IDENTITY: &str = concat!(
    "named-callable(path(UefiPhysicalEntry::enter),parameters(",
    "parameter\\(self\\(no\\)\\,mutable\\(no\\)\\,const\\(no\\)\\,",
    "named\\(name\\(EfiImageHandle\\)\\)\\)\\,",
    "parameter\\(self\\(no\\)\\,mutable\\(no\\)\\,const\\(no\\)\\,",
    "ref\\(named\\(name\\(EfiSystemTable\\)\\)\\)\\)",
    "),result-dispatch())",
);
/// Canonical normalized type identity of the opaque image-handle input.
pub const UEFI_X64_IMAGE_HANDLE_TYPE_IDENTITY: &str = "named(name(EfiImageHandle))";
/// Canonical normalized type identity of the shared system-table input.
pub const UEFI_X64_SYSTEM_TABLE_REFERENCE_TYPE_IDENTITY: &str = "ref(named(name(EfiSystemTable)))";
/// Canonical normalized type identity of the physical status result.
pub const UEFI_X64_STATUS_TYPE_IDENTITY: &str = "named(name(EfiStatus))";

/// Strong commitment to the exact closed UEFI target-package source compiled
/// into this plan owner.
pub fn exact_uefi_x64_physical_contract_package_source_digest()
-> super::ProgramEntryPhysicalContractPackageSourceDigest {
    super::ProgramEntryPhysicalContractPackageSourceDigest::from_package_source(
        omega_target::ProgramEntryPhysicalContractPackage::UefiX64,
        UEFI_X64_TARGET_PACKAGE_SOURCE,
    )
}

/// Independently reconstruct and validate the exact plan authored by the
/// closed UEFI target package. This deliberately does not substitute the
/// generic Microsoft-x64 ordinary-call plan: the target contract admits only
/// the seven integer volatile registers and excludes the XMM volatile bank.
pub fn exact_uefi_x64_physical_boundary_entry_plan() -> ValidatedBoundaryEntryPlan {
    let word = ValueShape::integer(8, 8);
    let register_word = |register| ValuePlacement {
        shape: word,
        locations: vec![ValueLocation::Register {
            register,
            value_byte_offset: 0,
            byte_size: 8,
        }],
    };
    let signature = CallSignature {
        parameters: vec![word, word],
        result: Some(word),
    };
    validate_boundary_entry_plan(
        BoundaryEntryPlan {
            call: CallPlan {
                policy: CallingPolicy::MicrosoftX64,
                parameters: vec![
                    register_word(MachineRegister::X86Rcx),
                    register_word(MachineRegister::X86Rdx),
                ],
                result: Some(register_word(MachineRegister::X86Rax)),
                callback_materializations: Vec::new(),
                ordinary_clobbers: RegisterSet::new([
                    MachineRegister::X86Rax,
                    MachineRegister::X86Rcx,
                    MachineRegister::X86Rdx,
                    MachineRegister::X86R8,
                    MachineRegister::X86R9,
                    MachineRegister::X86R10,
                    MachineRegister::X86R11,
                ]),
                stack_alignment: 16,
                shadow_bytes: 32,
                entry_control: EntryControl::CallReturn,
            },
            state: StatePlan {
                initial_regime: MachineRegime::X86Long64,
                interrupted_state: MachineStateSet::empty(),
                saved_state: MachineStateSet::empty(),
                restored_state: MachineStateSet::empty(),
                permitted_transitive_use: MachineStateSet::new([
                    MachineState::GeneralRegisters,
                    MachineState::Flags,
                ]),
                stack: EntryStack::ProviderSelected,
                preemption: Preemption::NotApplicable,
            },
        },
        &signature,
    )
    .expect("the closed target-authored UEFI x64 physical entry plan must remain valid")
}

impl ProgramEntryPhysicalContractPlan {
    /// Replay the complete target-owned UEFI x64 physical-entry contract.
    /// Constructor compatibility remains deliberately broader for synthetic
    /// compiler fixtures; runtime custody must use this exact verdict.
    pub fn matches_exact_uefi_x64_physical_contract(&self) -> bool {
        let expected = exact_uefi_x64_physical_boundary_entry_plan();
        self.target_slot == omega_target::TargetProfile::UefiX64.program_entry_slot()
            && self.target_package == omega_target::ProgramEntryPhysicalContractPackage::UefiX64
            && self.target_package_source_digest
                == exact_uefi_x64_physical_contract_package_source_digest()
            && self.requirement_identity == UEFI_X64_PHYSICAL_REQUIREMENT_IDENTITY
            && self.parameter_type_identities.len() == 2
            && self.parameter_type_identities[0] == UEFI_X64_IMAGE_HANDLE_TYPE_IDENTITY
            && self.parameter_type_identities[1] == UEFI_X64_SYSTEM_TABLE_REFERENCE_TYPE_IDENTITY
            && self.result_type_identity == UEFI_X64_STATUS_TYPE_IDENTITY
            && self.calling_plan_report_fingerprint == expected.contract_report_fingerprint()
            && &self.boundary_entry_plan == expected.plan()
            && self
                .guaranteed_entry_stack_application
                .matches_exact_uefi_x64_entry_stack_application()
            && self
                .guaranteed_entry_stack
                .matches_exact_uefi_x64_entry_stack_guarantee()
            && self.guaranteed_entry_stack.application() == &self.guaranteed_entry_stack_application
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ProgramEntryPhysicalContractPackageSourceDigest;

    fn exact_contract() -> ProgramEntryPhysicalContractPlan {
        let package = omega_target::ProgramEntryPhysicalContractPackage::UefiX64;
        let calling_plan = exact_uefi_x64_physical_boundary_entry_plan();
        ProgramEntryPhysicalContractPlan::new(
            omega_target::TargetProfile::UefiX64.program_entry_slot(),
            UEFI_X64_PHYSICAL_REQUIREMENT_IDENTITY.into(),
            package,
            exact_uefi_x64_physical_contract_package_source_digest(),
            1,
            vec![
                UEFI_X64_IMAGE_HANDLE_TYPE_IDENTITY.into(),
                UEFI_X64_SYSTEM_TABLE_REFERENCE_TYPE_IDENTITY.into(),
            ],
            UEFI_X64_STATUS_TYPE_IDENTITY.into(),
            calling_plan.contract_report_fingerprint(),
            calling_plan.plan().clone(),
        )
        .expect("exact physical contract")
    }

    #[test]
    fn exact_runtime_contract_replays_all_owned_fields() {
        let exact = exact_contract();
        assert!(exact.matches_exact_uefi_x64_physical_contract());
        assert_eq!(
            exact
                .guaranteed_entry_stack_application()
                .selected_profile(),
            omega_target::TargetProfile::UefiX64,
        );
        assert_eq!(
            exact.guaranteed_entry_stack_application().subject(),
            "UefiX86_64",
        );
        assert_ne!(
            exact
                .guaranteed_entry_stack_application()
                .compatibility_commitment(),
            &[0; 32],
        );
        assert_eq!(
            exact.guaranteed_entry_stack().guaranteed_available_bytes(),
            128 * 1024,
        );
        assert_eq!(exact.guaranteed_entry_stack().required_alignment(), 16);
        assert!(
            exact
                .guaranteed_entry_stack()
                .matches_exact_uefi_x64_entry_stack_guarantee()
        );

        let mut requirement_drift = exact.clone();
        requirement_drift.requirement_identity =
            "named-callable(path(ProgramStorageEntry::enter),parameters(),result-dispatch())"
                .into();
        assert!(!requirement_drift.matches_exact_uefi_x64_physical_contract());

        let mut parameter_drift = exact.clone();
        parameter_drift.parameter_type_identities.swap(0, 1);
        assert!(!parameter_drift.matches_exact_uefi_x64_physical_contract());

        let mut result_drift = exact.clone();
        result_drift.result_type_identity = "named(name(Unit))".into();
        assert!(!result_drift.matches_exact_uefi_x64_physical_contract());

        let mut source_drift = exact.clone();
        source_drift.target_package_source_digest =
            ProgramEntryPhysicalContractPackageSourceDigest::from_package_source(
                omega_target::ProgramEntryPhysicalContractPackage::UefiX64,
                b"substituted target package",
            );
        assert!(!source_drift.matches_exact_uefi_x64_physical_contract());

        let mut placement_drift = exact;
        placement_drift
            .boundary_entry_plan
            .call
            .parameters
            .swap(0, 1);
        assert!(!placement_drift.matches_exact_uefi_x64_physical_contract());
    }
}

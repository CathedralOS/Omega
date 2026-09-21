//! Exact macOS x86-64 replay owned beside the physical entry-plan carrier.

use calling_conventions::{
    CallSignature, ValidatedBoundaryEntryPlan, ValueShape, evaluate_ordinary_boundary_entry_plan,
};

use super::ProgramEntryPhysicalContractPlan;

const MACOS_X86_64_TARGET_PACKAGE_SOURCE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../../source/library/std/targets/macos_x86_64/entry.omg"
));

/// Canonical checked-tree overload identity for `MacosPhysicalEntry::enter`.
pub const MACOS_X86_64_PHYSICAL_REQUIREMENT_IDENTITY: &str = concat!(
    "named-callable(path(MacosPhysicalEntry::enter),parameters(",
    "parameter\\(self\\(no\\)\\,mutable\\(no\\)\\,const\\(no\\)\\,",
    "named\\(name\\(i32\\)\\)\\)\\,",
    "parameter\\(self\\(no\\)\\,mutable\\(no\\)\\,const\\(no\\)\\,",
    "named\\(name\\(addr\\)\\)\\)\\,",
    "parameter\\(self\\(no\\)\\,mutable\\(no\\)\\,const\\(no\\)\\,",
    "named\\(name\\(addr\\)\\)\\)\\,",
    "parameter\\(self\\(no\\)\\,mutable\\(no\\)\\,const\\(no\\)\\,",
    "named\\(name\\(addr\\)\\)\\)",
    "),result-dispatch())",
);
/// Canonical normalized type identity of the `argc` and result integers.
pub const MACOS_X86_64_I32_TYPE_IDENTITY: &str = "named(name(i32))";
/// Canonical normalized type identity of the `argv`, `envp`, and `apple`
/// raw process-arrival addresses.
pub const MACOS_X86_64_ADDRESS_TYPE_IDENTITY: &str = "named(name(addr))";

/// Strong commitment to the exact closed macOS x86-64 target-package source
/// compiled into this plan owner.
pub fn exact_macos_x86_64_physical_contract_package_source_digest()
-> super::ProgramEntryPhysicalContractPackageSourceDigest {
    super::ProgramEntryPhysicalContractPackageSourceDigest::from_package_source(
        target::ProgramEntryPhysicalContractPackage::MacosX64,
        MACOS_X86_64_TARGET_PACKAGE_SOURCE,
    )
}

/// Independently reconstruct and validate the exact plan authored by the
/// closed macOS x86-64 target package. The authored `MacosX64` policy is the
/// platform's ordinary System V AMD64 volatile bank, so the independent
/// replay is that evaluator applied to the exact dyld `appMain` arrival
/// signature: four inputs (`i32` then three addresses) and one `i32` result.
pub fn exact_macos_x86_64_physical_boundary_entry_plan() -> ValidatedBoundaryEntryPlan {
    let signature = CallSignature {
        parameters: vec![
            ValueShape::integer(4, 4),
            ValueShape::integer(8, 8),
            ValueShape::integer(8, 8),
            ValueShape::integer(8, 8),
        ],
        result: Some(ValueShape::integer(4, 4)),
    };
    evaluate_ordinary_boundary_entry_plan(
        calling_conventions::CallingPolicy::SystemVAMD64,
        &signature,
    )
    .expect("the closed target-authored macOS x86-64 physical entry plan must remain valid")
}

impl ProgramEntryPhysicalContractPlan {
    /// Replay the complete target-owned macOS x86-64 physical-entry contract.
    /// Constructor compatibility remains deliberately broader for synthetic
    /// compiler fixtures; runtime custody must use this exact verdict. The
    /// authored contract retains no numeric entry-stack guarantee, so this
    /// verdict rejects any injected closure rather than comparing one.
    pub fn matches_exact_macos_x86_64_physical_contract(&self) -> bool {
        let expected = exact_macos_x86_64_physical_boundary_entry_plan();
        self.target_slot == target::TargetProfile::MacosX64.program_entry_slot()
            && self.target_package == target::ProgramEntryPhysicalContractPackage::MacosX64
            && self.target_package_source_digest
                == exact_macos_x86_64_physical_contract_package_source_digest()
            && self.requirement_identity == MACOS_X86_64_PHYSICAL_REQUIREMENT_IDENTITY
            && self.parameter_type_identities.len() == 4
            && self.parameter_type_identities[0] == MACOS_X86_64_I32_TYPE_IDENTITY
            && self.parameter_type_identities[1..]
                .iter()
                .all(|identity| identity == MACOS_X86_64_ADDRESS_TYPE_IDENTITY)
            && self.result_type_identity == MACOS_X86_64_I32_TYPE_IDENTITY
            && self.calling_plan_report_fingerprint == expected.contract_report_fingerprint()
            && &self.boundary_entry_plan == expected.plan()
            && self.guaranteed_entry_stack.is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        MACOS_X86_64_ADDRESS_TYPE_IDENTITY, MACOS_X86_64_I32_TYPE_IDENTITY,
        MACOS_X86_64_PHYSICAL_REQUIREMENT_IDENTITY, ProgramEntryPhysicalContractPlan,
        exact_macos_x86_64_physical_boundary_entry_plan,
        exact_macos_x86_64_physical_contract_package_source_digest,
    };
    use crate::ProgramEntryPhysicalContractPackageSourceDigest;

    fn exact_contract() -> ProgramEntryPhysicalContractPlan {
        let package = target::ProgramEntryPhysicalContractPackage::MacosX64;
        let calling_plan = exact_macos_x86_64_physical_boundary_entry_plan();
        ProgramEntryPhysicalContractPlan::new(
            target::TargetProfile::MacosX64.program_entry_slot(),
            MACOS_X86_64_PHYSICAL_REQUIREMENT_IDENTITY.into(),
            package,
            exact_macos_x86_64_physical_contract_package_source_digest(),
            1,
            vec![
                MACOS_X86_64_I32_TYPE_IDENTITY.into(),
                MACOS_X86_64_ADDRESS_TYPE_IDENTITY.into(),
                MACOS_X86_64_ADDRESS_TYPE_IDENTITY.into(),
                MACOS_X86_64_ADDRESS_TYPE_IDENTITY.into(),
            ],
            MACOS_X86_64_I32_TYPE_IDENTITY.into(),
            calling_plan.contract_report_fingerprint(),
            calling_plan.plan().clone(),
        )
        .expect("exact physical contract")
    }

    #[test]
    fn exact_runtime_contract_replays_all_owned_fields() {
        let exact = exact_contract();
        assert!(exact.matches_exact_macos_x86_64_physical_contract());
        assert!(exact.guaranteed_entry_stack().is_none());
        assert!(exact.guaranteed_entry_stack_application().is_none());

        let mut requirement_drift = exact.clone();
        requirement_drift.requirement_identity =
            "named-callable(path(ProgramStorageEntry::enter),parameters(),result-dispatch())"
                .into();
        assert!(!requirement_drift.matches_exact_macos_x86_64_physical_contract());

        let mut parameter_drift = exact.clone();
        parameter_drift.parameter_type_identities.swap(0, 1);
        assert!(!parameter_drift.matches_exact_macos_x86_64_physical_contract());

        let mut result_drift = exact.clone();
        result_drift.result_type_identity = "named(name(Unit))".into();
        assert!(!result_drift.matches_exact_macos_x86_64_physical_contract());

        let mut source_drift = exact.clone();
        source_drift.target_package_source_digest =
            ProgramEntryPhysicalContractPackageSourceDigest::from_package_source(
                target::ProgramEntryPhysicalContractPackage::MacosX64,
                b"substituted target package",
            );
        assert!(!source_drift.matches_exact_macos_x86_64_physical_contract());

        let mut placement_drift = exact;
        placement_drift
            .boundary_entry_plan
            .call
            .parameters
            .swap(0, 1);
        assert!(!placement_drift.matches_exact_macos_x86_64_physical_contract());
    }

    #[test]
    fn physical_plan_is_exact_dyld_arrival_shape() {
        let plan = exact_macos_x86_64_physical_boundary_entry_plan();
        let call = &plan.plan().call;
        assert_eq!(
            call.policy,
            calling_conventions::CallingPolicy::SystemVAMD64
        );
        assert_eq!(call.parameters.len(), 4);
        let expected_registers = [
            calling_conventions::MachineRegister::X86Rdi,
            calling_conventions::MachineRegister::X86Rsi,
            calling_conventions::MachineRegister::X86Rdx,
            calling_conventions::MachineRegister::X86Rcx,
        ];
        for (index, parameter) in call.parameters.iter().enumerate() {
            let [location] = parameter.locations.as_slice() else {
                panic!("dyld input {index} is not one register fragment");
            };
            let calling_conventions::ValueLocation::Register {
                register,
                value_byte_offset,
                byte_size,
            } = location
            else {
                panic!("dyld input {index} is not register-placed");
            };
            assert_eq!(*value_byte_offset, 0);
            assert_eq!(*register, expected_registers[index]);
            assert_eq!(*byte_size, if index == 0 { 4 } else { 8 });
        }
        let result = call.result.as_ref().expect("dyld arrival returns i32");
        let [location] = result.locations.as_slice() else {
            panic!("dyld result is not one register fragment");
        };
        assert_eq!(
            *location,
            calling_conventions::ValueLocation::Register {
                register: calling_conventions::MachineRegister::X86Rax,
                value_byte_offset: 0,
                byte_size: 4,
            }
        );
        assert_eq!(call.stack_alignment, 16);
        assert_eq!(
            plan.plan().state.stack,
            calling_conventions::EntryStack::ProviderSelected
        );
    }
}

//! Exact macOS x86-64 replay owned beside the physical entry-plan carrier.

use calling_conventions::{
    CallSignature, ValidatedBoundaryEntryPlan, ValueShape, evaluate_ordinary_boundary_entry_plan,
};

use super::ProgramEntryPhysicalContractPlan;

const MACOS_X86_64_TARGET_PACKAGE_SOURCE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../../../source/library/std/targets/macos_x86_64/entry.omg"
));

/// Canonical checked-tree overload identity for `MacosX64PhysicalEntry::enter`.
pub const MACOS_X86_64_PHYSICAL_REQUIREMENT_IDENTITY: &str = concat!(
    "named-callable(path(MacosX64PhysicalEntry::enter),parameters(",
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

//! Exact macOS ARM64 replay owned beside the physical entry-plan carrier.

use calling_conventions::{
    CallSignature, ValidatedBoundaryEntryPlan, ValueShape, evaluate_ordinary_boundary_entry_plan,
};

const MACOS_ARM64_TARGET_PACKAGE_SOURCE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../../../source/library/std/targets/macos_arm64/entry.omg"
));

/// Canonical checked-tree overload identity for `MacosArm64PhysicalEntry::enter`.
pub const MACOS_ARM64_PHYSICAL_REQUIREMENT_IDENTITY: &str = concat!(
    "named-callable(path(MacosArm64PhysicalEntry::enter),parameters(",
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
pub const MACOS_ARM64_I32_TYPE_IDENTITY: &str = "named(name(i32))";
/// Canonical normalized type identity of the `argv`, `envp`, and `apple`
/// raw process-arrival addresses.
pub const MACOS_ARM64_ADDRESS_TYPE_IDENTITY: &str = "named(name(addr))";

/// Strong commitment to the exact closed macOS ARM64 target-package source
/// compiled into this plan owner.
pub fn exact_macos_arm64_physical_contract_package_source_digest()
-> super::ProgramEntryPhysicalContractPackageSourceDigest {
    super::ProgramEntryPhysicalContractPackageSourceDigest::from_package_source(
        target::ProgramEntryPhysicalContractPackage::MacosArm64,
        MACOS_ARM64_TARGET_PACKAGE_SOURCE,
    )
}

/// Independently reconstruct and validate the exact plan authored by the
/// closed macOS ARM64 target package. The authored `MacosArm64` policy is the
/// platform's ordinary AAPCS64 volatile bank, so the independent replay is
/// that evaluator applied to the exact dyld arrival signature: four inputs
/// (`i32` then three addresses) and one `i32` result.
pub fn exact_macos_arm64_physical_boundary_entry_plan() -> ValidatedBoundaryEntryPlan {
    let signature = CallSignature {
        parameters: vec![
            ValueShape::integer(4, 4),
            ValueShape::integer(8, 8),
            ValueShape::integer(8, 8),
            ValueShape::integer(8, 8),
        ],
        result: Some(ValueShape::integer(4, 4)),
    };
    evaluate_ordinary_boundary_entry_plan(calling_conventions::CallingPolicy::Aapcs64, &signature)
        .expect("the closed target-authored macOS ARM64 physical entry plan must remain valid")
}

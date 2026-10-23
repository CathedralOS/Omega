//! Exact Windows x86-64 replay owned beside the physical entry-plan carrier.

use calling_conventions::{
    CallSignature, CallingPolicy, ValidatedBoundaryEntryPlan, ValueShape,
    evaluate_ordinary_boundary_entry_plan,
};

const WINDOWS_X86_64_TARGET_PACKAGE_SOURCE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../../../source/library/std/targets/windows_x86_64/entry.omg"
));

/// Canonical checked-tree overload identity for `WindowsProcessEntry::enter`.
pub const WINDOWS_X86_64_PHYSICAL_REQUIREMENT_IDENTITY: &str =
    "named-callable(path(WindowsProcessEntry::enter),parameters(),result-dispatch())";
/// Canonical normalized type identity of the process-completion status.
pub const WINDOWS_X86_64_U32_TYPE_IDENTITY: &str = "named(name(u32))";

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
/// as the process exit code. That arrival is the platform's ordinary
/// Microsoft-x64 call-return surface — the Windows loader's contract and the
/// authored `WindowsX86_64` policy agree on every ABI property — so the
/// independent replay is the policy's own evaluator applied to the exact
/// loader signature rather than a second hand-spelled copy of it.
pub fn exact_windows_x86_64_physical_boundary_entry_plan() -> ValidatedBoundaryEntryPlan {
    let signature = CallSignature {
        parameters: Vec::new(),
        result: Some(ValueShape::integer(4, 4)),
    };
    evaluate_ordinary_boundary_entry_plan(CallingPolicy::MicrosoftX64, &signature)
        .expect("the closed target-authored Windows x86-64 physical entry plan must remain valid")
}

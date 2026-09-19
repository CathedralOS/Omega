//! Exact UEFI x64 replay owned beside the physical entry-plan carrier.
//!
//! The bundled closed target package's authored `UefiX86_64::plan` machine is
//! the single source definition of this calling policy; admission evaluation
//! mints the contract's retained boundary plan. Custody checks here replay
//! that retained evidence against the source-minted calling-plan commitment
//! instead of consulting a second Rust copy of the policy. The only Rust
//! constants below are the fixed package identity and the recorded commitment
//! — evidence of the evaluation's output, never the policy itself.

use calling_conventions::{
    BoundaryEntryPlan, CallSignature, CallingPolicy, MachineRegister, MachineState,
    MachineStateSet, RegisterSet, ValidatedBoundaryEntryPlan, ValueShape,
    evaluate_ordinary_boundary_entry_plan, validate_boundary_entry_plan,
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
        target::ProgramEntryPhysicalContractPackage::UefiX64,
        UEFI_X64_TARGET_PACKAGE_SOURCE,
    )
}

/// Domain-separated commitment to the canonical boundary-entry plan the
/// bundled closed package's authored `UefiX86_64::plan` machine produces for
/// `UefiPhysicalEntry::enter`'s exact signature. This is the
/// `contract_commitment_digest` of the plan the source evaluation mints; the
/// compiler replay
/// (`entry_and_abi::program_entries_and_image_validation`) keeps it honest
/// against live evaluation, so it changes only when that authored policy
/// changes.
pub const UEFI_X64_PHYSICAL_CALLING_PLAN_COMMITMENT: [u8; 32] = [
    10, 172, 212, 105, 61, 70, 186, 33, 243, 129, 223, 176, 113, 147, 97, 164, 49, 60, 230, 159,
    253, 121, 213, 251, 126, 106, 127, 144, 31, 84, 13, 130,
];

/// Replay independent validation of `plan` under the signature its own
/// placements imply, then bind the result to the source-minted calling-plan
/// commitment. Returns the canonical validated plan only when `plan` is
/// exactly what the authored policy produced: a substituted, drifted, or
/// foreign plan fails closed here even when every other pinned identity
/// matches. A source digest alone never establishes plan correctness — the
/// retained plan itself must reproduce the recorded commitment.
pub fn replayed_uefi_x64_physical_calling_plan(
    plan: &BoundaryEntryPlan,
) -> Option<ValidatedBoundaryEntryPlan> {
    let signature = CallSignature {
        parameters: plan
            .call
            .parameters
            .iter()
            .map(|placement| placement.shape)
            .collect(),
        result: plan.call.result.as_ref().map(|placement| placement.shape),
    };
    let validated = validate_boundary_entry_plan(plan.clone(), &signature).ok()?;
    (validated.plan() == plan
        && validated.contract_commitment_digest() == UEFI_X64_PHYSICAL_CALLING_PLAN_COMMITMENT)
        .then_some(validated)
}

/// Materialize the canonical plan the authored `UefiX86_64::plan` machine
/// produces for the exact physical signature. The authored machine narrows
/// the ordinary Microsoft-x64 boundary plan to the seven integer volatile
/// registers and the machine-state classes they cover; everything else is the
/// generic policy evaluation, and the result is admitted only because the
/// replay above binds it to the source-minted commitment.
///
/// Production verdicts never consult this materialization. It exists to
/// construct contract fixtures below the build layer, where source
/// evaluation is unavailable.
pub fn exact_uefi_x64_physical_boundary_entry_plan() -> ValidatedBoundaryEntryPlan {
    let word = ValueShape::integer(8, 8);
    let signature = CallSignature {
        parameters: vec![word, word],
        result: Some(word),
    };
    let mut plan = evaluate_ordinary_boundary_entry_plan(CallingPolicy::MicrosoftX64, &signature)
        .expect("the ordinary Microsoft-x64 boundary plan remains the authored policy's base")
        .plan()
        .clone();
    plan.call.ordinary_clobbers = RegisterSet::new([
        MachineRegister::X86Rax,
        MachineRegister::X86Rcx,
        MachineRegister::X86Rdx,
        MachineRegister::X86R8,
        MachineRegister::X86R9,
        MachineRegister::X86R10,
        MachineRegister::X86R11,
    ]);
    plan.state.permitted_transitive_use =
        MachineStateSet::new([MachineState::GeneralRegisters, MachineState::Flags]);
    replayed_uefi_x64_physical_calling_plan(&plan).expect(
        "the authored UEFI x64 narrowing must replay the source-minted calling-plan commitment",
    )
}

impl ProgramEntryPhysicalContractPlan {
    /// Replay the complete target-owned UEFI x64 physical-entry contract.
    /// Constructor compatibility remains deliberately broader for synthetic
    /// compiler fixtures; runtime custody must use this exact verdict.
    pub fn matches_exact_uefi_x64_physical_contract(&self) -> bool {
        let Some(guarantee) = &self.guaranteed_entry_stack else {
            return false;
        };
        let Some(replayed) = replayed_uefi_x64_physical_calling_plan(&self.boundary_entry_plan)
        else {
            return false;
        };
        self.target_slot == target::TargetProfile::UefiX64.program_entry_slot()
            && self.target_package == target::ProgramEntryPhysicalContractPackage::UefiX64
            && self.target_package_source_digest
                == exact_uefi_x64_physical_contract_package_source_digest()
            && self.requirement_identity == UEFI_X64_PHYSICAL_REQUIREMENT_IDENTITY
            && self.parameter_type_identities.len() == 2
            && self.parameter_type_identities[0] == UEFI_X64_IMAGE_HANDLE_TYPE_IDENTITY
            && self.parameter_type_identities[1] == UEFI_X64_SYSTEM_TABLE_REFERENCE_TYPE_IDENTITY
            && self.result_type_identity == UEFI_X64_STATUS_TYPE_IDENTITY
            && self.calling_plan_report_fingerprint == replayed.contract_report_fingerprint()
            && guarantee
                .application()
                .matches_exact_uefi_x64_entry_stack_application()
            && guarantee.matches_exact_uefi_x64_entry_stack_guarantee()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ProgramEntryPhysicalContractPlan, UEFI_X64_IMAGE_HANDLE_TYPE_IDENTITY,
        UEFI_X64_PHYSICAL_REQUIREMENT_IDENTITY, UEFI_X64_STATUS_TYPE_IDENTITY,
        UEFI_X64_SYSTEM_TABLE_REFERENCE_TYPE_IDENTITY, exact_uefi_x64_physical_boundary_entry_plan,
        exact_uefi_x64_physical_contract_package_source_digest,
        replayed_uefi_x64_physical_calling_plan,
    };
    use crate::ProgramEntryPhysicalContractPackageSourceDigest;
    use calling_conventions::{CallingPolicy, evaluate_ordinary_boundary_entry_plan};

    fn exact_contract() -> ProgramEntryPhysicalContractPlan {
        let package = target::ProgramEntryPhysicalContractPackage::UefiX64;
        let calling_plan = exact_uefi_x64_physical_boundary_entry_plan();
        ProgramEntryPhysicalContractPlan::new(
            target::TargetProfile::UefiX64.program_entry_slot(),
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
        let application = exact
            .guaranteed_entry_stack_application()
            .expect("exact UEFI contract retains stack evidence");
        let guarantee = exact
            .guaranteed_entry_stack()
            .expect("exact UEFI contract retains its numeric stack closure");
        assert_eq!(
            application.selected_profile(),
            target::TargetProfile::UefiX64,
        );
        assert_eq!(application.subject(), "UefiX86_64",);
        assert_ne!(application.compatibility_commitment(), &[0; 32],);
        assert_eq!(guarantee.guaranteed_available_bytes(), 128 * 1024,);
        assert_eq!(guarantee.required_alignment(), 16);
        assert!(guarantee.matches_exact_uefi_x64_entry_stack_guarantee());

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
                target::ProgramEntryPhysicalContractPackage::UefiX64,
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

    #[test]
    fn replay_rejects_foreign_and_unvalidated_plans() {
        let word = calling_conventions::ValueShape::integer(8, 8);
        let signature = calling_conventions::CallSignature {
            parameters: vec![word, word],
            result: Some(word),
        };
        // The ordinary Microsoft-x64 plan is structurally valid but admits the
        // XMM volatile bank the authored UEFI policy deliberately excludes.
        let ordinary =
            evaluate_ordinary_boundary_entry_plan(CallingPolicy::MicrosoftX64, &signature)
                .expect("ordinary boundary plan");
        assert!(replayed_uefi_x64_physical_calling_plan(ordinary.plan()).is_none());

        let mut noncanonical = exact_uefi_x64_physical_boundary_entry_plan().plan().clone();
        noncanonical.call.parameters.swap(0, 1);
        assert!(replayed_uefi_x64_physical_calling_plan(&noncanonical).is_none());
    }
}

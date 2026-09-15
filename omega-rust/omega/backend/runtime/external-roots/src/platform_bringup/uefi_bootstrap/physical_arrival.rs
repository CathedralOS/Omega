//! The UEFI application physical arrival and its join.

use crate::platform_bringup::uefi_bootstrap::{
    LifecycleScopedUefiSystemTable, UefiApplicationFirmwareLedger, UefiImageHandleProvenance,
};
use crate::{
    ExternalRootDiagnostic, UefiApplicationBootstrapLedgerId, UefiFirmwareSessionId,
    UefiImageHandleOccurrenceId, UefiPhysicalInvocationId, UefiSystemTableOccurrenceId,
};
use program_entry_plan::ProgramEntryPhysicalContractPlan;

/// Non-authorizing custody of both physical inputs under one exact UEFI entry
/// contract. This carrier establishes neither firmware-provider access nor
/// program-storage roots, a shell invocation, or native execution.
#[must_use = "UEFI physical arrival retains both linear physical inputs"]
pub struct UefiApplicationPhysicalArrival<'occurrence> {
    pub(super) image_handle: UefiImageHandleProvenance,
    pub(super) system_table: LifecycleScopedUefiSystemTable<'occurrence>,
    pub(super) physical_contract: ProgramEntryPhysicalContractPlan,
}

impl std::fmt::Debug for UefiApplicationPhysicalArrival<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("UefiApplicationPhysicalArrival")
            .field("ledger", &self.ledger_id())
            .field("session", &self.firmware_session())
            .field("invocation", &self.physical_invocation())
            .field("image_handle_occurrence", &self.image_handle_occurrence())
            .field("system_table_occurrence", &self.system_table_occurrence())
            .field(
                "physical_requirement_identity",
                &self.physical_contract.requirement_identity(),
            )
            .field(
                "calling_plan_report_fingerprint",
                &self.physical_contract.calling_plan_report_fingerprint(),
            )
            .finish_non_exhaustive()
    }
}

impl UefiApplicationPhysicalArrival<'_> {
    pub const fn ledger_id(&self) -> UefiApplicationBootstrapLedgerId {
        self.image_handle.ledger
    }

    pub const fn firmware_session(&self) -> UefiFirmwareSessionId {
        self.image_handle.session
    }

    pub const fn physical_invocation(&self) -> UefiPhysicalInvocationId {
        self.image_handle.invocation
    }

    pub const fn image_handle_occurrence(&self) -> UefiImageHandleOccurrenceId {
        self.image_handle.occurrence
    }

    pub const fn system_table_occurrence(&self) -> UefiSystemTableOccurrenceId {
        self.system_table.occurrence_id()
    }

    pub const fn physical_contract(&self) -> &ProgramEntryPhysicalContractPlan {
        &self.physical_contract
    }
}

/// Readiness custody for composing the target-runtime bootstrap adapter.
///
/// Construction consumes the complete physical-arrival join and independently
/// retains the collision-resistant commitment to the exact target-owned entry
/// plan. It does not assert that the launch environment, generated shell, or
/// native adapter invocation has been admitted or executed.
#[must_use = "UEFI adapter-composition readiness retains physical-arrival custody"]
pub struct UefiApplicationBootstrapAdapterInvocationReadiness<'occurrence> {
    pub(super) arrival: UefiApplicationPhysicalArrival<'occurrence>,
    pub(super) physical_calling_plan_commitment: [u8; 32],
}

impl std::fmt::Debug for UefiApplicationBootstrapAdapterInvocationReadiness<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("UefiApplicationBootstrapAdapterInvocationReadiness")
            .field("ledger", &self.ledger_id())
            .field("physical_invocation", &self.physical_invocation())
            .field(
                "physical_requirement_identity",
                &self.physical_requirement_identity(),
            )
            .field(
                "physical_calling_plan_commitment",
                &self.physical_calling_plan_commitment,
            )
            .finish_non_exhaustive()
    }
}

impl UefiApplicationBootstrapAdapterInvocationReadiness<'_> {
    pub const fn ledger_id(&self) -> UefiApplicationBootstrapLedgerId {
        self.arrival.ledger_id()
    }

    pub const fn physical_invocation(&self) -> UefiPhysicalInvocationId {
        self.arrival.physical_invocation()
    }

    pub const fn firmware_session(&self) -> UefiFirmwareSessionId {
        self.arrival.firmware_session()
    }

    pub const fn image_handle_occurrence(&self) -> UefiImageHandleOccurrenceId {
        self.arrival.image_handle_occurrence()
    }

    pub const fn system_table_occurrence(&self) -> UefiSystemTableOccurrenceId {
        self.arrival.system_table_occurrence()
    }

    pub fn physical_requirement_identity(&self) -> &str {
        self.arrival.physical_contract.requirement_identity()
    }

    pub const fn physical_calling_plan_commitment(&self) -> &[u8; 32] {
        &self.physical_calling_plan_commitment
    }
}

/// Recoverable physical-arrival rejection retaining both linear inputs and the
/// immutable contract plan for a corrected join attempt.
#[derive(Debug)]
#[must_use = "UEFI physical-arrival rejection retains all join inputs"]
pub struct UefiApplicationPhysicalArrivalJoinError<'occurrence> {
    image_handle: UefiImageHandleProvenance,
    system_table: LifecycleScopedUefiSystemTable<'occurrence>,
    physical_contract: ProgramEntryPhysicalContractPlan,
    diagnostic: ExternalRootDiagnostic,
}

impl<'occurrence> UefiApplicationPhysicalArrivalJoinError<'occurrence> {
    pub const fn diagnostic(&self) -> &ExternalRootDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(
        self,
    ) -> (
        UefiImageHandleProvenance,
        LifecycleScopedUefiSystemTable<'occurrence>,
        ProgramEntryPhysicalContractPlan,
        ExternalRootDiagnostic,
    ) {
        (
            self.image_handle,
            self.system_table,
            self.physical_contract,
            self.diagnostic,
        )
    }
}

impl std::fmt::Display for UefiApplicationPhysicalArrivalJoinError<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}

impl std::error::Error for UefiApplicationPhysicalArrivalJoinError<'_> {}

/// Join the two physical inputs only after replaying the exact target-owned
/// UEFI requirement and complete Microsoft-x64 entry plan. Success remains
/// pre-provider and pre-installation; no firmware or storage authority is
/// introduced here.
pub fn join_uefi_application_physical_arrival<'occurrence>(
    ledger: &UefiApplicationFirmwareLedger<'occurrence>,
    image_handle: UefiImageHandleProvenance,
    system_table: LifecycleScopedUefiSystemTable<'occurrence>,
    physical_contract: ProgramEntryPhysicalContractPlan,
) -> Result<
    UefiApplicationPhysicalArrival<'occurrence>,
    Box<UefiApplicationPhysicalArrivalJoinError<'occurrence>>,
> {
    if !ledger.matches_image_handle(&image_handle) {
        return reject_physical_arrival_join(
            image_handle,
            system_table,
            physical_contract,
            "UEFI image-handle provenance belongs to a different physical invocation",
        );
    }
    if !ledger.matches_provenance(&system_table.provenance)
        || !ledger.matches_lease(&system_table.phase_lease)
    {
        return reject_physical_arrival_join(
            image_handle,
            system_table,
            physical_contract,
            "UEFI system-table scope belongs to a different or inactive physical invocation",
        );
    }
    if image_handle.ledger != system_table.ledger_id()
        || image_handle.session != system_table.firmware_session()
        || image_handle.invocation != system_table.physical_invocation()
    {
        return reject_physical_arrival_join(
            image_handle,
            system_table,
            physical_contract,
            "UEFI image handle and system table do not belong to the same physical invocation",
        );
    }
    if !physical_contract.matches_exact_uefi_x64_physical_contract() {
        return reject_physical_arrival_join(
            image_handle,
            system_table,
            physical_contract,
            "UEFI physical arrival does not retain the exact target requirement, types, result, and Microsoft-x64 entry plan",
        );
    }
    Ok(UefiApplicationPhysicalArrival {
        image_handle,
        system_table,
        physical_contract,
    })
}

fn reject_physical_arrival_join<'occurrence>(
    image_handle: UefiImageHandleProvenance,
    system_table: LifecycleScopedUefiSystemTable<'occurrence>,
    physical_contract: ProgramEntryPhysicalContractPlan,
    message: impl Into<String>,
) -> Result<
    UefiApplicationPhysicalArrival<'occurrence>,
    Box<UefiApplicationPhysicalArrivalJoinError<'occurrence>>,
> {
    Err(Box::new(UefiApplicationPhysicalArrivalJoinError {
        image_handle,
        system_table,
        physical_contract,
        diagnostic: ExternalRootDiagnostic(message.into()),
    }))
}

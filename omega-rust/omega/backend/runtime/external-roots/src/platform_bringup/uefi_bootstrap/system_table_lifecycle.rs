//! Lifecycle-scoped UEFI system tables, their release and their join.

use crate::platform_bringup::uefi_bootstrap::UefiApplicationFirmwareLedger;
use crate::{
    ExternalRootDiagnostic, UefiApplicationBootstrapLedgerId, UefiBootServicesPhaseLeaseId,
    UefiFirmwareSessionId, UefiImageHandleOccurrenceId, UefiPhysicalInvocationId,
    UefiSystemTableOccurrenceId,
};
use std::num::NonZeroU64;
use target::{
    TargetProfile, ValidatedUefiSystemTableHeaderIntegrity, ValidatedUefiSystemTableNativeLayout,
    plan_uefi_system_table_native_layout,
};

/// Opaque provenance for the image handle supplied to one exact physical
/// invocation. A concrete physical-input admission retains the non-null handle
/// value privately for exact provider invocation; no public handle/address
/// projection exists and the carrier cannot become storage authority.
#[must_use = "UEFI image-handle provenance is a linear physical-arrival input"]
pub struct UefiImageHandleProvenance {
    pub(super) authority: u64,
    pub(super) ledger: UefiApplicationBootstrapLedgerId,
    pub(super) session: UefiFirmwareSessionId,
    pub(super) invocation: UefiPhysicalInvocationId,
    pub(super) occurrence: UefiImageHandleOccurrenceId,
    pub(super) opaque_handle: Option<NonZeroU64>,
}

impl std::fmt::Debug for UefiImageHandleProvenance {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("UefiImageHandleProvenance")
            .field("ledger", &self.ledger)
            .field("session", &self.session)
            .field("invocation", &self.invocation)
            .field("occurrence", &self.occurrence)
            .finish_non_exhaustive()
    }
}

/// Exact physical-arrival provenance for one borrowed system-table range.
/// The range is retained privately and has no raw-address projection.
pub struct UefiSystemTableOccurrenceProvenance<'occurrence> {
    pub(super) authority: u64,
    pub(super) ledger: UefiApplicationBootstrapLedgerId,
    pub(super) session: UefiFirmwareSessionId,
    pub(super) invocation: UefiPhysicalInvocationId,
    pub(super) occurrence: UefiSystemTableOccurrenceId,
    pub(super) table_bytes: &'occurrence [u8],
}

impl std::fmt::Debug for UefiSystemTableOccurrenceProvenance<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("UefiSystemTableOccurrenceProvenance")
            .field("ledger", &self.ledger)
            .field("session", &self.session)
            .field("invocation", &self.invocation)
            .field("occurrence", &self.occurrence)
            .field("table_byte_len", &self.table_bytes.len())
            .finish_non_exhaustive()
    }
}

/// Linear evidence that Boot Services remain live for one exact invocation.
#[derive(Debug)]
pub struct UefiBootServicesPhaseLease {
    pub(super) authority: u64,
    pub(super) ledger: UefiApplicationBootstrapLedgerId,
    pub(super) session: UefiFirmwareSessionId,
    pub(super) invocation: UefiPhysicalInvocationId,
    pub(super) lease: UefiBootServicesPhaseLeaseId,
    pub(super) generation: u64,
}

/// Metadata-only table carrier scoped to the retained firmware phase lease.
/// It intentionally exposes neither bytes nor integrity/provenance inputs.
#[must_use = "scoped UEFI system table retains live firmware-phase custody"]
pub struct LifecycleScopedUefiSystemTable<'occurrence> {
    pub(super) integrity: ValidatedUefiSystemTableHeaderIntegrity<'occurrence>,
    pub(super) provenance: UefiSystemTableOccurrenceProvenance<'occurrence>,
    pub(super) phase_lease: UefiBootServicesPhaseLease,
}

impl std::fmt::Debug for LifecycleScopedUefiSystemTable<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("LifecycleScopedUefiSystemTable")
            .field("ledger", &self.ledger_id())
            .field("session", &self.firmware_session())
            .field("invocation", &self.physical_invocation())
            .field("occurrence", &self.occurrence_id())
            .field("phase_lease", &self.phase_lease_id())
            .field(
                "non_authoritative_layout_report_fingerprint",
                &self.layout().non_authoritative_layout_report_fingerprint(),
            )
            .field("revision", &self.revision())
            .field("header_size", &self.header_size())
            .field("stored_crc32", &self.stored_crc32())
            .finish_non_exhaustive()
    }
}

impl LifecycleScopedUefiSystemTable<'_> {
    pub const fn layout(&self) -> &ValidatedUefiSystemTableNativeLayout {
        self.integrity.layout()
    }

    pub const fn ledger_id(&self) -> UefiApplicationBootstrapLedgerId {
        self.provenance.ledger
    }

    pub const fn firmware_session(&self) -> UefiFirmwareSessionId {
        self.provenance.session
    }

    pub const fn physical_invocation(&self) -> UefiPhysicalInvocationId {
        self.provenance.invocation
    }

    pub const fn occurrence_id(&self) -> UefiSystemTableOccurrenceId {
        self.provenance.occurrence
    }

    pub const fn phase_lease_id(&self) -> UefiBootServicesPhaseLeaseId {
        self.phase_lease.lease
    }

    pub const fn revision(&self) -> u32 {
        self.integrity.revision()
    }

    pub const fn header_size(&self) -> u32 {
        self.integrity.header_size()
    }

    pub const fn stored_crc32(&self) -> u32 {
        self.integrity.stored_crc32()
    }
}

/// Report-only observation that one scoped table released its phase lease.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReleasedUefiSystemTableScope {
    pub ledger: UefiApplicationBootstrapLedgerId,
    pub session: UefiFirmwareSessionId,
    pub invocation: UefiPhysicalInvocationId,
    pub occurrence: UefiSystemTableOccurrenceId,
    pub lease: UefiBootServicesPhaseLeaseId,
    pub non_authoritative_layout_report_fingerprint: u64,
}

/// Recoverable release failure retaining the complete scoped carrier.
#[derive(Debug)]
#[must_use = "UEFI scope release rejection retains lifecycle custody"]
pub struct UefiSystemTableScopeReleaseError<'occurrence> {
    pub(super) scoped: LifecycleScopedUefiSystemTable<'occurrence>,
    pub(super) diagnostic: ExternalRootDiagnostic,
}

impl<'occurrence> UefiSystemTableScopeReleaseError<'occurrence> {
    pub const fn diagnostic(&self) -> &ExternalRootDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(
        self,
    ) -> (
        LifecycleScopedUefiSystemTable<'occurrence>,
        ExternalRootDiagnostic,
    ) {
        (self.scoped, self.diagnostic)
    }
}

impl std::fmt::Display for UefiSystemTableScopeReleaseError<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}

impl std::error::Error for UefiSystemTableScopeReleaseError<'_> {}

/// Recoverable composition failure retaining every linear input for a
/// corrected join attempt.
#[derive(Debug)]
#[must_use = "UEFI lifecycle join rejection retains all composition inputs"]
pub struct UefiSystemTableLifecycleJoinError<'occurrence> {
    integrity: ValidatedUefiSystemTableHeaderIntegrity<'occurrence>,
    provenance: UefiSystemTableOccurrenceProvenance<'occurrence>,
    phase_lease: UefiBootServicesPhaseLease,
    diagnostic: ExternalRootDiagnostic,
}

impl<'occurrence> UefiSystemTableLifecycleJoinError<'occurrence> {
    pub const fn diagnostic(&self) -> &ExternalRootDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(
        self,
    ) -> (
        ValidatedUefiSystemTableHeaderIntegrity<'occurrence>,
        UefiSystemTableOccurrenceProvenance<'occurrence>,
        UefiBootServicesPhaseLease,
        ExternalRootDiagnostic,
    ) {
        (
            self.integrity,
            self.provenance,
            self.phase_lease,
            self.diagnostic,
        )
    }
}

impl std::fmt::Display for UefiSystemTableLifecycleJoinError<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}

impl std::error::Error for UefiSystemTableLifecycleJoinError<'_> {}

/// Join target-owned header integrity to the exact physical occurrence and
/// current firmware phase. Every check precedes success construction, so a
/// rejection returns all three inputs unchanged.
pub fn join_lifecycle_scoped_uefi_system_table<'occurrence>(
    ledger: &UefiApplicationFirmwareLedger<'occurrence>,
    integrity: ValidatedUefiSystemTableHeaderIntegrity<'occurrence>,
    provenance: UefiSystemTableOccurrenceProvenance<'occurrence>,
    phase_lease: UefiBootServicesPhaseLease,
) -> Result<
    LifecycleScopedUefiSystemTable<'occurrence>,
    Box<UefiSystemTableLifecycleJoinError<'occurrence>>,
> {
    let expected_layout = plan_uefi_system_table_native_layout(TargetProfile::UefiX64)
        .expect("the closed UEFI x64 target must retain its system-table layout");
    if !integrity.layout().matches_exact_plan(&expected_layout) {
        return reject_join(
            integrity,
            provenance,
            phase_lease,
            "UEFI system-table integrity does not retain the exact UEFI x64 target entry layout",
        );
    }
    if !ledger.matches_provenance(&provenance) {
        return reject_join(
            integrity,
            provenance,
            phase_lease,
            "UEFI system-table occurrence provenance belongs to a different physical invocation",
        );
    }
    if !std::ptr::eq(integrity.table_bytes(), provenance.table_bytes) {
        return reject_join(
            integrity,
            provenance,
            phase_lease,
            "UEFI header integrity and physical provenance do not retain the exact same byte range",
        );
    }
    if !ledger.matches_lease(&phase_lease) {
        return reject_join(
            integrity,
            provenance,
            phase_lease,
            "UEFI Boot Services phase lease is foreign, stale, spent, or no longer live",
        );
    }
    Ok(LifecycleScopedUefiSystemTable {
        integrity,
        provenance,
        phase_lease,
    })
}

fn reject_join<'occurrence>(
    integrity: ValidatedUefiSystemTableHeaderIntegrity<'occurrence>,
    provenance: UefiSystemTableOccurrenceProvenance<'occurrence>,
    phase_lease: UefiBootServicesPhaseLease,
    message: impl Into<String>,
) -> Result<
    LifecycleScopedUefiSystemTable<'occurrence>,
    Box<UefiSystemTableLifecycleJoinError<'occurrence>>,
> {
    Err(Box::new(UefiSystemTableLifecycleJoinError {
        integrity,
        provenance,
        phase_lease,
        diagnostic: ExternalRootDiagnostic(message.into()),
    }))
}

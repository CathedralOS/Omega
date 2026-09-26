//! The UEFI application firmware ledger and its authority claim.

use crate::LifecycleScopedUefiBootServicesProjection;
pub(crate) use crate::UefiBootServicesProjectionReleaseError;
use crate::platform_bringup::uefi_bootstrap::{
    LifecycleScopedUefiSystemTable, ReleasedUefiSystemTableScope, UefiBootServicesPhaseLease,
    UefiImageHandleProvenance, UefiSystemTableOccurrenceProvenance,
    UefiSystemTableScopeReleaseError,
};
use crate::{
    ExternalRootDiagnostic, UefiApplicationBootstrapLedgerId, UefiBootServicesPhaseLeaseId,
    UefiFirmwareSessionId, UefiImageHandleOccurrenceId, UefiPhysicalInvocationId,
    UefiSystemTableOccurrenceId,
};
use std::num::NonZeroU64;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_LEDGER_AUTHORITY: AtomicU64 = AtomicU64::new(1);

pub(crate) fn claim_ledger_authority() -> Result<u64, ExternalRootDiagnostic> {
    NEXT_LEDGER_AUTHORITY
        .try_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
            current.checked_add(1)
        })
        .map_err(|_| {
            ExternalRootDiagnostic(
                "UEFI application bootstrap ledger authority identity exhausted".into(),
            )
        })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReturningApplicationPhase {
    BootServicesLive,
    Returning,
}

/// Invocation-owned authority for the returning `UefiApplication` firmware
/// phase. Report identities name the occurrence but cannot mint its private
/// authority binding or either linear input carrier.
pub struct UefiApplicationFirmwareLedger<'occurrence> {
    authority: u64,
    ledger: UefiApplicationBootstrapLedgerId,
    session: UefiFirmwareSessionId,
    invocation: UefiPhysicalInvocationId,
    phase: ReturningApplicationPhase,
    image_handle: Option<UefiImageHandleOccurrenceId>,
    image_handle_value: Option<NonZeroU64>,
    image_handle_provenance_issued: bool,
    occurrence: Option<UefiSystemTableOccurrenceId>,
    table_bytes: Option<&'occurrence [u8]>,
    provenance_issued: bool,
    active_lease: Option<UefiBootServicesPhaseLeaseId>,
    phase_generation: u64,
}

impl std::fmt::Debug for UefiApplicationFirmwareLedger<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("UefiApplicationFirmwareLedger")
            .field("ledger", &self.ledger)
            .field("session", &self.session)
            .field("invocation", &self.invocation)
            .field("phase", &self.phase)
            .field("image_handle", &self.image_handle)
            .field("has_image_handle_value", &self.image_handle_value.is_some())
            .field(
                "image_handle_provenance_issued",
                &self.image_handle_provenance_issued,
            )
            .field("occurrence", &self.occurrence)
            .field("table_byte_len", &self.table_bytes.map(<[u8]>::len))
            .field("provenance_issued", &self.provenance_issued)
            .field("active_lease", &self.active_lease)
            .field("phase_generation", &self.phase_generation)
            .finish_non_exhaustive()
    }
}

impl<'occurrence> UefiApplicationFirmwareLedger<'occurrence> {
    /// Start the firmware ledger for one exact physical invocation. The
    /// private authority identity distinguishes separate ledgers even if a
    /// caller accidentally reuses the same normalized report keys.
    pub fn new(
        ledger: UefiApplicationBootstrapLedgerId,
        session: UefiFirmwareSessionId,
        invocation: UefiPhysicalInvocationId,
    ) -> Result<Self, ExternalRootDiagnostic> {
        Ok(Self {
            authority: claim_ledger_authority()?,
            ledger,
            session,
            invocation,
            phase: ReturningApplicationPhase::BootServicesLive,
            image_handle: None,
            image_handle_value: None,
            image_handle_provenance_issued: false,
            occurrence: None,
            table_bytes: None,
            provenance_issued: false,
            active_lease: None,
            phase_generation: 1,
        })
    }

    pub const fn ledger_id(&self) -> UefiApplicationBootstrapLedgerId {
        self.ledger
    }

    pub const fn firmware_session(&self) -> UefiFirmwareSessionId {
        self.session
    }

    pub const fn physical_invocation(&self) -> UefiPhysicalInvocationId {
        self.invocation
    }

    /// Admit the opaque image-handle occurrence supplied by physical arrival.
    /// Admission is single-shot and retains no raw handle value or storage
    /// projection.
    pub fn admit_image_handle_occurrence(
        &mut self,
        occurrence: UefiImageHandleOccurrenceId,
    ) -> Result<UefiImageHandleProvenance, ExternalRootDiagnostic> {
        self.admit_image_handle(occurrence, None)
    }

    /// Admit the opaque non-null image-handle value supplied by the exact
    /// physical UEFI entry occurrence. The value stays private beneath the
    /// provenance carrier: it can become a firmware-call operand, but never a
    /// storage address or `Extent` projection.
    pub fn admit_image_handle_physical_input(
        &mut self,
        occurrence: UefiImageHandleOccurrenceId,
        handle: NonZeroU64,
    ) -> Result<UefiImageHandleProvenance, ExternalRootDiagnostic> {
        self.admit_image_handle(occurrence, Some(handle))
    }

    fn admit_image_handle(
        &mut self,
        occurrence: UefiImageHandleOccurrenceId,
        handle: Option<NonZeroU64>,
    ) -> Result<UefiImageHandleProvenance, ExternalRootDiagnostic> {
        if self.phase != ReturningApplicationPhase::BootServicesLive {
            return Err(ExternalRootDiagnostic(
                "UEFI image handle arrived after the returning firmware phase began".into(),
            ));
        }
        if self.image_handle_provenance_issued || self.image_handle.is_some() {
            return Err(ExternalRootDiagnostic(
                "UEFI physical invocation already admitted an image-handle occurrence".into(),
            ));
        }
        self.image_handle = Some(occurrence);
        self.image_handle_value = handle;
        self.image_handle_provenance_issued = true;
        Ok(UefiImageHandleProvenance {
            authority: self.authority,
            ledger: self.ledger,
            session: self.session,
            invocation: self.invocation,
            occurrence,
            opaque_handle: handle,
        })
    }

    /// Admit the exact byte range supplied by physical arrival. Admission is
    /// single-shot; equal contents in a different allocation are a different
    /// occurrence and cannot satisfy the resulting provenance.
    pub fn admit_system_table_occurrence(
        &mut self,
        occurrence: UefiSystemTableOccurrenceId,
        table_bytes: &'occurrence [u8],
    ) -> Result<UefiSystemTableOccurrenceProvenance<'occurrence>, ExternalRootDiagnostic> {
        if self.phase != ReturningApplicationPhase::BootServicesLive {
            return Err(ExternalRootDiagnostic(
                "UEFI system-table occurrence arrived after the returning firmware phase began"
                    .into(),
            ));
        }
        if self.provenance_issued || self.occurrence.is_some() {
            return Err(ExternalRootDiagnostic(
                "UEFI physical invocation already admitted a system-table occurrence".into(),
            ));
        }
        if table_bytes.is_empty() {
            return Err(ExternalRootDiagnostic(
                "UEFI system-table occurrence provenance cannot name an empty byte range".into(),
            ));
        }
        self.occurrence = Some(occurrence);
        self.table_bytes = Some(table_bytes);
        self.provenance_issued = true;
        Ok(UefiSystemTableOccurrenceProvenance {
            authority: self.authority,
            ledger: self.ledger,
            session: self.session,
            invocation: self.invocation,
            occurrence,
            table_bytes,
        })
    }

    /// Acquire the sole current Boot-Services-live phase lease. The lease is
    /// non-clone and the ledger will not issue another while it remains live.
    pub fn acquire_boot_services_phase_lease(
        &mut self,
        lease: UefiBootServicesPhaseLeaseId,
    ) -> Result<UefiBootServicesPhaseLease, ExternalRootDiagnostic> {
        if self.phase != ReturningApplicationPhase::BootServicesLive {
            return Err(ExternalRootDiagnostic(
                "UEFI Boot Services phase is no longer live for this returning invocation".into(),
            ));
        }
        if self.active_lease.is_some() {
            return Err(ExternalRootDiagnostic(
                "UEFI Boot Services phase already has a live lease".into(),
            ));
        }
        self.active_lease = Some(lease);
        Ok(UefiBootServicesPhaseLease {
            authority: self.authority,
            ledger: self.ledger,
            session: self.session,
            invocation: self.invocation,
            lease,
            generation: self.phase_generation,
        })
    }

    /// Retire one scoped table before the returning adapter gives control back
    /// to firmware. All retained inputs are consumed here; report identities
    /// survive only as observations.
    pub fn release_lifecycle_scoped_system_table(
        &mut self,
        scoped: LifecycleScopedUefiSystemTable<'occurrence>,
    ) -> Result<ReleasedUefiSystemTableScope, Box<UefiSystemTableScopeReleaseError<'occurrence>>>
    {
        if !self.matches_lease(&scoped.phase_lease)
            || scoped.provenance.authority != self.authority
            || Some(scoped.provenance.occurrence) != self.occurrence
        {
            return Err(Box::new(UefiSystemTableScopeReleaseError {
                scoped,
                diagnostic: ExternalRootDiagnostic(
                    "lifecycle-scoped UEFI system table belongs to a different firmware ledger"
                        .into(),
                ),
            }));
        }
        self.active_lease = None;
        let report = ReleasedUefiSystemTableScope {
            ledger: self.ledger,
            session: self.session,
            invocation: self.invocation,
            occurrence: scoped.provenance.occurrence,
            lease: scoped.phase_lease.lease,
            non_authoritative_layout_report_fingerprint: scoped
                .integrity
                .layout()
                .non_authoritative_layout_report_fingerprint(),
        };
        drop(scoped);
        Ok(report)
    }

    /// Retire the exact Boot-Services field correspondence before returning
    /// to firmware. A failed release preserves the complete physical-arrival
    /// and field-projection custody for a corrected ledger join.
    pub fn release_lifecycle_scoped_boot_services_projection(
        &mut self,
        projection: LifecycleScopedUefiBootServicesProjection<'occurrence>,
    ) -> Result<
        ReleasedUefiSystemTableScope,
        Box<UefiBootServicesProjectionReleaseError<'occurrence>>,
    > {
        if !self.matches_image_handle(&projection.readiness.arrival.image_handle)
            || !self.matches_provenance(&projection.readiness.arrival.system_table.provenance)
            || !self.matches_lease(&projection.readiness.arrival.system_table.phase_lease)
        {
            return Err(Box::new(UefiBootServicesProjectionReleaseError {
                projection,
                diagnostic: ExternalRootDiagnostic(
                    "lifecycle-scoped UEFI Boot Services projection belongs to a different firmware ledger"
                        .into(),
                ),
            }));
        }
        self.active_lease = None;
        let report = ReleasedUefiSystemTableScope {
            ledger: self.ledger,
            session: self.session,
            invocation: self.invocation,
            occurrence: projection
                .readiness
                .arrival
                .system_table
                .provenance
                .occurrence,
            lease: projection.readiness.arrival.system_table.phase_lease.lease,
            non_authoritative_layout_report_fingerprint: projection
                .readiness
                .arrival
                .system_table
                .integrity
                .layout()
                .non_authoritative_layout_report_fingerprint(),
        };
        drop(projection);
        Ok(report)
    }

    /// Complete the live-service portion of the returning profile. A scoped
    /// provider must be released first.
    pub fn begin_firmware_return(&mut self) -> Result<(), ExternalRootDiagnostic> {
        if self.active_lease.is_some() {
            return Err(ExternalRootDiagnostic(
                "cannot begin UEFI firmware return while a Boot Services phase lease is live"
                    .into(),
            ));
        }
        if self.phase != ReturningApplicationPhase::BootServicesLive {
            return Err(ExternalRootDiagnostic(
                "UEFI returning invocation already left the Boot Services live phase".into(),
            ));
        }
        let next_generation = self.phase_generation.checked_add(1).ok_or_else(|| {
            ExternalRootDiagnostic("UEFI firmware phase generation overflowed".into())
        })?;
        self.phase = ReturningApplicationPhase::Returning;
        self.phase_generation = next_generation;
        Ok(())
    }

    pub(crate) fn matches_provenance(
        &self,
        provenance: &UefiSystemTableOccurrenceProvenance<'_>,
    ) -> bool {
        provenance.authority == self.authority
            && provenance.ledger == self.ledger
            && provenance.session == self.session
            && provenance.invocation == self.invocation
            && Some(provenance.occurrence) == self.occurrence
            && self
                .table_bytes
                .is_some_and(|bytes| std::ptr::eq(bytes, provenance.table_bytes))
    }

    pub(crate) fn matches_image_handle(&self, provenance: &UefiImageHandleProvenance) -> bool {
        provenance.authority == self.authority
            && provenance.ledger == self.ledger
            && provenance.session == self.session
            && provenance.invocation == self.invocation
            && Some(provenance.occurrence) == self.image_handle
            && provenance.opaque_handle == self.image_handle_value
    }

    pub(crate) fn matches_lease(&self, lease: &UefiBootServicesPhaseLease) -> bool {
        self.phase == ReturningApplicationPhase::BootServicesLive
            && lease.authority == self.authority
            && lease.ledger == self.ledger
            && lease.session == self.session
            && lease.invocation == self.invocation
            && Some(lease.lease) == self.active_lease
            && lease.generation == self.phase_generation
    }
}

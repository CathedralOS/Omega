//! Returning-application UEFI system-table lifecycle composition.
//!
//! Header integrity is deliberately weaker than permission to use firmware
//! services. This module joins that target-owned integrity evidence to the
//! exact physical-arrival occurrence and a current Boot-Services-live phase
//! lease. The result remains a metadata-only lifecycle carrier: service-field
//! projection belongs to a later provider-specific edge.

use std::sync::atomic::{AtomicU64, Ordering};

use omega_program_entry_plan::{
    ProgramEntryPhysicalContractPlan, exact_uefi_x64_physical_boundary_entry_plan,
};
use omega_target::{
    TargetProfile, ValidatedUefiSystemTableHeaderIntegrity, ValidatedUefiSystemTableNativeLayout,
    plan_uefi_system_table_native_layout,
};

use crate::{
    ExternalRootDiagnostic, UefiApplicationBootstrapLedgerId, UefiBootServicesPhaseLeaseId,
    UefiFirmwareSessionId, UefiImageHandleOccurrenceId, UefiPhysicalInvocationId,
    UefiSystemTableOccurrenceId,
};

mod provider_projection;
pub use provider_projection::*;
mod handle_protocol_provider;
pub use handle_protocol_provider::*;

static NEXT_LEDGER_AUTHORITY: AtomicU64 = AtomicU64::new(1);

fn claim_ledger_authority() -> Result<u64, ExternalRootDiagnostic> {
    NEXT_LEDGER_AUTHORITY
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
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
        self.image_handle_provenance_issued = true;
        Ok(UefiImageHandleProvenance {
            authority: self.authority,
            ledger: self.ledger,
            session: self.session,
            invocation: self.invocation,
            occurrence,
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

    fn matches_provenance(&self, provenance: &UefiSystemTableOccurrenceProvenance<'_>) -> bool {
        provenance.authority == self.authority
            && provenance.ledger == self.ledger
            && provenance.session == self.session
            && provenance.invocation == self.invocation
            && Some(provenance.occurrence) == self.occurrence
            && self
                .table_bytes
                .is_some_and(|bytes| std::ptr::eq(bytes, provenance.table_bytes))
    }

    fn matches_image_handle(&self, provenance: &UefiImageHandleProvenance) -> bool {
        provenance.authority == self.authority
            && provenance.ledger == self.ledger
            && provenance.session == self.session
            && provenance.invocation == self.invocation
            && Some(provenance.occurrence) == self.image_handle
    }

    fn matches_lease(&self, lease: &UefiBootServicesPhaseLease) -> bool {
        self.phase == ReturningApplicationPhase::BootServicesLive
            && lease.authority == self.authority
            && lease.ledger == self.ledger
            && lease.session == self.session
            && lease.invocation == self.invocation
            && Some(lease.lease) == self.active_lease
            && lease.generation == self.phase_generation
    }
}

/// Opaque provenance for the image handle supplied to one exact physical
/// invocation. The carrier deliberately retains no raw handle address and
/// cannot be projected into storage authority.
#[must_use = "UEFI image-handle provenance is a linear physical-arrival input"]
pub struct UefiImageHandleProvenance {
    authority: u64,
    ledger: UefiApplicationBootstrapLedgerId,
    session: UefiFirmwareSessionId,
    invocation: UefiPhysicalInvocationId,
    occurrence: UefiImageHandleOccurrenceId,
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
    authority: u64,
    ledger: UefiApplicationBootstrapLedgerId,
    session: UefiFirmwareSessionId,
    invocation: UefiPhysicalInvocationId,
    occurrence: UefiSystemTableOccurrenceId,
    table_bytes: &'occurrence [u8],
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
    authority: u64,
    ledger: UefiApplicationBootstrapLedgerId,
    session: UefiFirmwareSessionId,
    invocation: UefiPhysicalInvocationId,
    lease: UefiBootServicesPhaseLeaseId,
    generation: u64,
}

/// Metadata-only table carrier scoped to the retained firmware phase lease.
/// It intentionally exposes neither bytes nor integrity/provenance inputs.
#[must_use = "scoped UEFI system table retains live firmware-phase custody"]
pub struct LifecycleScopedUefiSystemTable<'occurrence> {
    integrity: ValidatedUefiSystemTableHeaderIntegrity<'occurrence>,
    provenance: UefiSystemTableOccurrenceProvenance<'occurrence>,
    phase_lease: UefiBootServicesPhaseLease,
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
    scoped: LifecycleScopedUefiSystemTable<'occurrence>,
    diagnostic: ExternalRootDiagnostic,
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

/// Non-authorizing custody of both physical inputs under one exact UEFI entry
/// contract. This carrier establishes neither firmware-provider access nor
/// program-storage roots, a shell invocation, or native execution.
#[must_use = "UEFI physical arrival retains both linear physical inputs"]
pub struct UefiApplicationPhysicalArrival<'occurrence> {
    image_handle: UefiImageHandleProvenance,
    system_table: LifecycleScopedUefiSystemTable<'occurrence>,
    physical_contract: ProgramEntryPhysicalContractPlan,
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
    physical_calling_plan_commitment: [u8; 32],
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

/// Recoverable readiness rejection retaining the complete physical arrival.
#[derive(Debug)]
#[must_use = "UEFI adapter-readiness rejection retains physical-arrival custody"]
pub struct UefiApplicationBootstrapAdapterReadinessError<'occurrence> {
    arrival: UefiApplicationPhysicalArrival<'occurrence>,
    diagnostic: ExternalRootDiagnostic,
}

impl<'occurrence> UefiApplicationBootstrapAdapterReadinessError<'occurrence> {
    pub const fn diagnostic(&self) -> &ExternalRootDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(
        self,
    ) -> (
        UefiApplicationPhysicalArrival<'occurrence>,
        ExternalRootDiagnostic,
    ) {
        (self.arrival, self.diagnostic)
    }
}

impl std::fmt::Display for UefiApplicationBootstrapAdapterReadinessError<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}

impl std::error::Error for UefiApplicationBootstrapAdapterReadinessError<'_> {}

/// Consume the exact physical-arrival join before any provider projection can
/// enter target-runtime adapter composition. This is readiness evidence only:
/// target-owned environment/stack evidence and a generated native invocation
/// remain required before `UefiPhysicalEntry::enter` is realized.
pub fn prepare_uefi_application_bootstrap_adapter_invocation<'occurrence>(
    ledger: &UefiApplicationFirmwareLedger<'occurrence>,
    arrival: UefiApplicationPhysicalArrival<'occurrence>,
) -> Result<
    UefiApplicationBootstrapAdapterInvocationReadiness<'occurrence>,
    Box<UefiApplicationBootstrapAdapterReadinessError<'occurrence>>,
> {
    if !ledger.matches_image_handle(&arrival.image_handle)
        || !ledger.matches_provenance(&arrival.system_table.provenance)
        || !ledger.matches_lease(&arrival.system_table.phase_lease)
    {
        return Err(Box::new(UefiApplicationBootstrapAdapterReadinessError {
            arrival,
            diagnostic: ExternalRootDiagnostic(
                "UEFI adapter readiness belongs to a different or inactive physical invocation"
                    .into(),
            ),
        }));
    }
    if !arrival
        .physical_contract
        .matches_exact_uefi_x64_physical_contract()
    {
        return Err(Box::new(
            UefiApplicationBootstrapAdapterReadinessError {
                arrival,
                diagnostic: ExternalRootDiagnostic(
                    "UEFI adapter readiness does not retain the exact target-owned physical entry contract"
                        .into(),
                ),
            },
        ));
    }
    let expected = exact_uefi_x64_physical_boundary_entry_plan();
    let commitment = expected.contract_commitment_digest();
    if commitment == [0; 32]
        || expected.contract_report_fingerprint()
            != arrival.physical_contract.calling_plan_report_fingerprint()
        || expected.plan() != arrival.physical_contract.boundary_entry_plan()
    {
        return Err(Box::new(UefiApplicationBootstrapAdapterReadinessError {
            arrival,
            diagnostic: ExternalRootDiagnostic(
                "UEFI adapter readiness physical calling-plan replay drifted".into(),
            ),
        }));
    }
    Ok(UefiApplicationBootstrapAdapterInvocationReadiness {
        arrival,
        physical_calling_plan_commitment: commitment,
    })
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

#[cfg(test)]
mod tests {
    use super::*;
    use omega_calling_conventions::{MachineRegister, ValueLocation};
    use omega_program_entry_plan::{
        UEFI_X64_IMAGE_HANDLE_TYPE_IDENTITY, UEFI_X64_PHYSICAL_REQUIREMENT_IDENTITY,
        UEFI_X64_STATUS_TYPE_IDENTITY, UEFI_X64_SYSTEM_TABLE_REFERENCE_TYPE_IDENTITY,
        exact_uefi_x64_physical_boundary_entry_plan,
        exact_uefi_x64_physical_contract_package_source_digest,
    };
    use omega_target::{
        ProgramEntryPhysicalContractPackage, UEFI_SYSTEM_TABLE_SIGNATURE,
        validate_uefi_system_table_occurrence,
    };

    const REVISION: u32 = (2 << 16) | 100;

    fn id<T>(value: u64, constructor: impl FnOnce(u64) -> Result<T, ExternalRootDiagnostic>) -> T {
        constructor(value).unwrap()
    }

    fn ledger<'a>(base: u64) -> UefiApplicationFirmwareLedger<'a> {
        UefiApplicationFirmwareLedger::new(
            id(
                base,
                UefiApplicationBootstrapLedgerId::from_normalized_identity,
            ),
            id(base + 1, UefiFirmwareSessionId::from_normalized_identity),
            id(base + 2, UefiPhysicalInvocationId::from_normalized_identity),
        )
        .unwrap()
    }

    fn inputs<'a>(
        ledger: &mut UefiApplicationFirmwareLedger<'a>,
        bytes: &'a [u8],
        occurrence_id: u64,
        lease_id: u64,
    ) -> (
        ValidatedUefiSystemTableHeaderIntegrity<'a>,
        UefiSystemTableOccurrenceProvenance<'a>,
        UefiBootServicesPhaseLease,
    ) {
        let integrity = validate_uefi_system_table_occurrence(
            plan_uefi_system_table_native_layout(TargetProfile::UefiX64).unwrap(),
            bytes,
        )
        .unwrap();
        let provenance = ledger
            .admit_system_table_occurrence(
                id(
                    occurrence_id,
                    UefiSystemTableOccurrenceId::from_normalized_identity,
                ),
                integrity.table_bytes(),
            )
            .unwrap();
        let lease = ledger
            .acquire_boot_services_phase_lease(id(
                lease_id,
                UefiBootServicesPhaseLeaseId::from_normalized_identity,
            ))
            .unwrap();
        (integrity, provenance, lease)
    }

    fn valid_occurrence(header_size: usize) -> Vec<u8> {
        assert!(header_size >= 120);
        let mut bytes = vec![0; header_size];
        bytes[0..8].copy_from_slice(&UEFI_SYSTEM_TABLE_SIGNATURE.to_le_bytes());
        bytes[8..12].copy_from_slice(&REVISION.to_le_bytes());
        bytes[12..16].copy_from_slice(&(header_size as u32).to_le_bytes());
        for (offset, byte) in bytes[24..].iter_mut().enumerate() {
            *byte = (offset as u8).wrapping_mul(17).wrapping_add(3);
        }
        let crc = system_table_crc32(&bytes);
        bytes[16..20].copy_from_slice(&crc.to_le_bytes());
        bytes
    }

    fn system_table_crc32(bytes: &[u8]) -> u32 {
        let mut crc = u32::MAX;
        for (index, byte) in bytes.iter().copied().enumerate() {
            let byte = if (16..20).contains(&index) { 0 } else { byte };
            crc ^= u32::from(byte);
            for _ in 0..8 {
                let low_bit_mask = 0_u32.wrapping_sub(crc & 1);
                crc = (crc >> 1) ^ (0xedb8_8320 & low_bit_mask);
            }
        }
        !crc
    }

    fn physical_contract(
        requirement_identity: &str,
        mutate_plan: impl FnOnce(&mut omega_calling_conventions::BoundaryEntryPlan),
    ) -> ProgramEntryPhysicalContractPlan {
        let expected = exact_uefi_x64_physical_boundary_entry_plan();
        let mut plan = expected.plan().clone();
        mutate_plan(&mut plan);
        ProgramEntryPhysicalContractPlan::new(
            TargetProfile::UefiX64.program_entry_slot(),
            requirement_identity.into(),
            ProgramEntryPhysicalContractPackage::UefiX64,
            exact_uefi_x64_physical_contract_package_source_digest(),
            0xfeed,
            vec![
                UEFI_X64_IMAGE_HANDLE_TYPE_IDENTITY.into(),
                UEFI_X64_SYSTEM_TABLE_REFERENCE_TYPE_IDENTITY.into(),
            ],
            UEFI_X64_STATUS_TYPE_IDENTITY.into(),
            expected.contract_report_fingerprint(),
            plan,
        )
        .unwrap()
    }

    fn exact_physical_contract() -> ProgramEntryPhysicalContractPlan {
        physical_contract(UEFI_X64_PHYSICAL_REQUIREMENT_IDENTITY, |_| {})
    }

    fn item_block<'a>(source: &'a str, declaration: &str) -> &'a str {
        let start = source.find(declaration).expect("source declaration");
        let body = &source[start..];
        let mut depth = 0_u32;
        let mut opened = false;
        for (index, character) in body.char_indices() {
            match character {
                '{' => {
                    opened = true;
                    depth += 1;
                }
                '}' if opened => {
                    depth -= 1;
                    if depth == 0 {
                        return &body[..=index];
                    }
                }
                _ => {}
            }
        }
        panic!("unterminated source declaration {declaration}")
    }

    fn public_method_names(block: &str) -> Vec<&str> {
        block
            .lines()
            .filter_map(|line| {
                let line = line.trim();
                if !line.starts_with("pub ") {
                    return None;
                }
                let function = line.find("fn ")?;
                line[function + 3..].split('(').next()
            })
            .collect()
    }

    #[test]
    fn joins_exact_occurrence_and_live_phase_without_pointer_projection() {
        let bytes = valid_occurrence(120);
        let mut ledger = ledger(10);
        let (integrity, provenance, lease) = inputs(&mut ledger, &bytes, 13, 14);
        let scoped =
            join_lifecycle_scoped_uefi_system_table(&ledger, integrity, provenance, lease).unwrap();

        assert_eq!(scoped.layout().profile(), TargetProfile::UefiX64);
        assert_eq!(
            scoped.layout().entry_slot(),
            TargetProfile::UefiX64.program_entry_slot()
        );
        assert_eq!(scoped.revision(), REVISION);
        assert_eq!(scoped.header_size(), 120);
        assert_eq!(scoped.physical_invocation(), ledger.physical_invocation());
        assert_eq!(scoped.firmware_session(), ledger.firmware_session());

        let released = ledger
            .release_lifecycle_scoped_system_table(scoped)
            .unwrap();
        assert_eq!(released.ledger, ledger.ledger_id());
        ledger.begin_firmware_return().unwrap();
        assert!(
            ledger
                .acquire_boot_services_phase_lease(id(
                    15,
                    UefiBootServicesPhaseLeaseId::from_normalized_identity
                ))
                .is_err()
        );
    }

    #[test]
    fn joins_both_physical_inputs_under_the_exact_non_authorizing_contract() {
        let bytes = valid_occurrence(120);
        let mut ledger = ledger(70);
        let image_handle = ledger
            .admit_image_handle_occurrence(id(
                73,
                UefiImageHandleOccurrenceId::from_normalized_identity,
            ))
            .unwrap();
        let (integrity, provenance, lease) = inputs(&mut ledger, &bytes, 74, 75);
        let system_table =
            join_lifecycle_scoped_uefi_system_table(&ledger, integrity, provenance, lease).unwrap();
        let arrival = join_uefi_application_physical_arrival(
            &ledger,
            image_handle,
            system_table,
            exact_physical_contract(),
        )
        .unwrap();

        assert_eq!(arrival.ledger_id(), ledger.ledger_id());
        assert_eq!(arrival.firmware_session(), ledger.firmware_session());
        assert_eq!(arrival.physical_invocation(), ledger.physical_invocation());
        assert_eq!(
            arrival.physical_contract().requirement_identity(),
            UEFI_X64_PHYSICAL_REQUIREMENT_IDENTITY
        );

        let UefiApplicationPhysicalArrival {
            image_handle: _,
            system_table,
            physical_contract: _,
        } = arrival;
        ledger
            .release_lifecycle_scoped_system_table(system_table)
            .unwrap();
    }

    #[test]
    fn physical_arrival_public_surface_has_no_handle_or_storage_projection() {
        let source = include_str!("uefi_bootstrap.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("production UEFI source");
        let handle = item_block(source, "pub struct UefiImageHandleProvenance");
        let compact_handle = handle
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect::<String>();
        assert_eq!(
            compact_handle,
            "pubstructUefiImageHandleProvenance{authority:u64,ledger:UefiApplicationBootstrapLedgerId,session:UefiFirmwareSessionId,invocation:UefiPhysicalInvocationId,occurrence:UefiImageHandleOccurrenceId,}"
        );
        let compact_source = source
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect::<String>();
        assert!(!compact_source.contains("implUefiImageHandleProvenance{"));
        assert_eq!(
            compact_source
                .matches("forUefiImageHandleProvenance{")
                .count(),
            1,
            "image-handle provenance must implement only report-only Debug",
        );
        assert!(compact_source.contains("implstd::fmt::DebugforUefiImageHandleProvenance{"));

        let arrival = item_block(
            source,
            "pub struct UefiApplicationPhysicalArrival<'occurrence>",
        );
        let compact_arrival = arrival
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect::<String>();
        assert_eq!(
            compact_arrival,
            "pubstructUefiApplicationPhysicalArrival<'occurrence>{image_handle:UefiImageHandleProvenance,system_table:LifecycleScopedUefiSystemTable<'occurrence>,physical_contract:ProgramEntryPhysicalContractPlan,}"
        );
        let arrival_impl = item_block(source, "impl UefiApplicationPhysicalArrival<'_>");
        assert_eq!(
            public_method_names(arrival_impl),
            [
                "ledger_id",
                "firmware_session",
                "physical_invocation",
                "image_handle_occurrence",
                "system_table_occurrence",
                "physical_contract",
            ]
        );
        assert_eq!(
            compact_source
                .matches("forUefiApplicationPhysicalArrival<'_>{")
                .count(),
            1,
            "physical-arrival custody must implement only report-only Debug",
        );

        let readiness = item_block(
            source,
            "pub struct UefiApplicationBootstrapAdapterInvocationReadiness<'occurrence>",
        );
        let compact_readiness = readiness
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect::<String>();
        assert_eq!(
            compact_readiness,
            "pubstructUefiApplicationBootstrapAdapterInvocationReadiness<'occurrence>{pub(super)arrival:UefiApplicationPhysicalArrival<'occurrence>,physical_calling_plan_commitment:[u8;32],}"
        );
        let readiness_impl = item_block(
            source,
            "impl UefiApplicationBootstrapAdapterInvocationReadiness<'_>",
        );
        assert_eq!(
            public_method_names(readiness_impl),
            [
                "ledger_id",
                "physical_invocation",
                "firmware_session",
                "image_handle_occurrence",
                "system_table_occurrence",
                "physical_requirement_identity",
                "physical_calling_plan_commitment",
            ]
        );
        assert_eq!(
            compact_source
                .matches("forUefiApplicationBootstrapAdapterInvocationReadiness<'_>{")
                .count(),
            1,
            "adapter readiness must implement only report-only Debug",
        );

        for forbidden in [
            "psi_extents::Extent",
            "pub fn raw_",
            "pub const fn raw_",
            "pub fn address",
            "pub const fn address",
            "impl From<UefiImageHandleProvenance",
            "impl Into<UefiImageHandleProvenance",
            "impl From<UefiApplicationPhysicalArrival",
            "impl Into<UefiApplicationPhysicalArrival",
        ] {
            assert!(
                !source.contains(forbidden),
                "forbidden UEFI physical-arrival API appeared: {forbidden}"
            );
        }
    }

    #[test]
    fn image_handle_provenance_is_issued_only_once() {
        let mut ledger = ledger(80);
        let _handle = ledger
            .admit_image_handle_occurrence(id(
                83,
                UefiImageHandleOccurrenceId::from_normalized_identity,
            ))
            .unwrap();
        let error = ledger
            .admit_image_handle_occurrence(id(
                84,
                UefiImageHandleOccurrenceId::from_normalized_identity,
            ))
            .unwrap_err();
        assert!(error.0.contains("already admitted"));
    }

    #[test]
    fn foreign_image_handle_rejects_even_when_report_ids_match() {
        let bytes = valid_occurrence(120);
        let mut exact = ledger(90);
        let mut foreign = ledger(90);
        let image_handle = foreign
            .admit_image_handle_occurrence(id(
                93,
                UefiImageHandleOccurrenceId::from_normalized_identity,
            ))
            .unwrap();
        let (integrity, provenance, lease) = inputs(&mut exact, &bytes, 94, 95);
        let system_table =
            join_lifecycle_scoped_uefi_system_table(&exact, integrity, provenance, lease).unwrap();

        let error = join_uefi_application_physical_arrival(
            &exact,
            image_handle,
            system_table,
            exact_physical_contract(),
        )
        .unwrap_err();
        assert!(
            error
                .diagnostic()
                .0
                .contains("different physical invocation")
        );
        let (_image_handle, system_table, _contract, _) = error.into_parts();
        exact
            .release_lifecycle_scoped_system_table(system_table)
            .unwrap();
    }

    #[test]
    fn drifted_physical_plan_rejects_and_returns_inputs_for_retry() {
        let bytes = valid_occurrence(120);
        let mut ledger = ledger(100);
        let image_handle = ledger
            .admit_image_handle_occurrence(id(
                103,
                UefiImageHandleOccurrenceId::from_normalized_identity,
            ))
            .unwrap();
        let (integrity, provenance, lease) = inputs(&mut ledger, &bytes, 104, 105);
        let system_table =
            join_lifecycle_scoped_uefi_system_table(&ledger, integrity, provenance, lease).unwrap();
        let drifted = physical_contract(UEFI_X64_PHYSICAL_REQUIREMENT_IDENTITY, |plan| {
            plan.call.parameters[0].locations[0] = ValueLocation::Register {
                register: MachineRegister::X86R8,
                value_byte_offset: 0,
                byte_size: 8,
            };
        });

        let error =
            join_uefi_application_physical_arrival(&ledger, image_handle, system_table, drifted)
                .unwrap_err();
        assert!(error.diagnostic().0.contains("exact target requirement"));
        let (image_handle, system_table, _drifted, _) = error.into_parts();
        let arrival = join_uefi_application_physical_arrival(
            &ledger,
            image_handle,
            system_table,
            exact_physical_contract(),
        )
        .unwrap();
        let UefiApplicationPhysicalArrival { system_table, .. } = arrival;
        ledger
            .release_lifecycle_scoped_system_table(system_table)
            .unwrap();
    }

    #[test]
    fn semantic_requirement_conflation_rejects() {
        let bytes = valid_occurrence(120);
        let mut ledger = ledger(110);
        let image_handle = ledger
            .admit_image_handle_occurrence(id(
                113,
                UefiImageHandleOccurrenceId::from_normalized_identity,
            ))
            .unwrap();
        let (integrity, provenance, lease) = inputs(&mut ledger, &bytes, 114, 115);
        let system_table =
            join_lifecycle_scoped_uefi_system_table(&ledger, integrity, provenance, lease).unwrap();
        let conflated = physical_contract(
            "named-callable(path(ProgramStorageEntry::enter),parameters(),result-dispatch())",
            |_| {},
        );

        let error =
            join_uefi_application_physical_arrival(&ledger, image_handle, system_table, conflated)
                .unwrap_err();
        assert!(error.diagnostic().0.contains("exact target requirement"));
        let (_image_handle, system_table, _contract, _) = error.into_parts();
        ledger
            .release_lifecycle_scoped_system_table(system_table)
            .unwrap();
    }

    #[test]
    fn accepts_crc_covered_forward_compatible_suffix() {
        let bytes = valid_occurrence(136);
        let mut ledger = ledger(20);
        let (integrity, provenance, lease) = inputs(&mut ledger, &bytes, 23, 24);
        let scoped =
            join_lifecycle_scoped_uefi_system_table(&ledger, integrity, provenance, lease).unwrap();
        assert_eq!(scoped.header_size(), 136);
    }

    #[test]
    fn equal_contents_in_a_different_allocation_reject_and_return_all_inputs() {
        let bytes = valid_occurrence(120);
        let copy = bytes.clone();
        let mut ledger = ledger(30);
        let integrity = validate_uefi_system_table_occurrence(
            plan_uefi_system_table_native_layout(TargetProfile::UefiX64).unwrap(),
            &copy,
        )
        .unwrap();
        let provenance = ledger
            .admit_system_table_occurrence(
                id(33, UefiSystemTableOccurrenceId::from_normalized_identity),
                &bytes,
            )
            .unwrap();
        let lease = ledger
            .acquire_boot_services_phase_lease(id(
                34,
                UefiBootServicesPhaseLeaseId::from_normalized_identity,
            ))
            .unwrap();

        let error = join_lifecycle_scoped_uefi_system_table(&ledger, integrity, provenance, lease)
            .unwrap_err();
        assert!(error.diagnostic().0.contains("exact same byte range"));
        let (_wrong_integrity, provenance, lease, _) = error.into_parts();
        let integrity = validate_uefi_system_table_occurrence(
            plan_uefi_system_table_native_layout(TargetProfile::UefiX64).unwrap(),
            &bytes,
        )
        .unwrap();
        let scoped =
            join_lifecycle_scoped_uefi_system_table(&ledger, integrity, provenance, lease).unwrap();
        ledger
            .release_lifecycle_scoped_system_table(scoped)
            .unwrap();
    }

    #[test]
    fn same_allocation_with_a_different_range_rejects() {
        let mut bytes = valid_occurrence(120);
        bytes.extend_from_slice(&[0; 16]);
        let mut ledger = ledger(35);
        let integrity = validate_uefi_system_table_occurrence(
            plan_uefi_system_table_native_layout(TargetProfile::UefiX64).unwrap(),
            &bytes,
        )
        .unwrap();
        assert_eq!(integrity.table_bytes().len(), 120);
        let provenance = ledger
            .admit_system_table_occurrence(
                id(38, UefiSystemTableOccurrenceId::from_normalized_identity),
                &bytes,
            )
            .unwrap();
        let lease = ledger
            .acquire_boot_services_phase_lease(id(
                39,
                UefiBootServicesPhaseLeaseId::from_normalized_identity,
            ))
            .unwrap();

        let error = join_lifecycle_scoped_uefi_system_table(&ledger, integrity, provenance, lease)
            .unwrap_err();
        assert!(error.diagnostic().0.contains("exact same byte range"));
    }

    #[test]
    fn copied_report_ids_from_a_foreign_ledger_reject_and_exact_ledger_accepts_retry() {
        let bytes = valid_occurrence(120);
        let mut exact = ledger(40);
        let foreign = ledger(40);
        let (integrity, provenance, lease) = inputs(&mut exact, &bytes, 43, 44);
        let error = join_lifecycle_scoped_uefi_system_table(&foreign, integrity, provenance, lease)
            .unwrap_err();
        assert!(
            error
                .diagnostic()
                .0
                .contains("different physical invocation")
        );
        let (integrity, provenance, lease, _) = error.into_parts();
        let scoped =
            join_lifecycle_scoped_uefi_system_table(&exact, integrity, provenance, lease).unwrap();
        exact.release_lifecycle_scoped_system_table(scoped).unwrap();
    }

    #[test]
    fn failed_release_returns_the_complete_scoped_carrier() {
        let bytes = valid_occurrence(120);
        let mut exact = ledger(50);
        let mut foreign = ledger(50);
        let (integrity, provenance, lease) = inputs(&mut exact, &bytes, 53, 54);
        let scoped =
            join_lifecycle_scoped_uefi_system_table(&exact, integrity, provenance, lease).unwrap();

        let error = foreign
            .release_lifecycle_scoped_system_table(scoped)
            .unwrap_err();
        assert!(error.diagnostic().0.contains("different firmware ledger"));
        let (scoped, _) = error.into_parts();
        exact.release_lifecycle_scoped_system_table(scoped).unwrap();
    }

    #[test]
    fn stale_or_spent_phase_lease_fails_closed() {
        let bytes = valid_occurrence(120);
        let mut ledger = ledger(60);
        let (integrity, provenance, mut lease) = inputs(&mut ledger, &bytes, 63, 64);
        lease.generation += 1;
        let error = join_lifecycle_scoped_uefi_system_table(&ledger, integrity, provenance, lease)
            .unwrap_err();
        assert!(error.diagnostic().0.contains("foreign, stale, spent"));
    }
}

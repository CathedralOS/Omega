//! Returning-application UEFI system-table lifecycle composition.
//!
//! Header integrity is deliberately weaker than permission to use firmware
//! services. This module joins that target-owned integrity evidence to the
//! exact physical-arrival occurrence and a current Boot-Services-live phase
//! lease. The result remains a metadata-only lifecycle carrier: service-field
//! projection belongs to a later provider-specific edge.

use std::num::NonZeroU64;
use std::sync::atomic::{AtomicU64, Ordering};

use omega_calling_conventions::{CallSignature, CallingPolicy, validate_boundary_entry_plan};
use omega_program_entry_plan::{
    OptimizedProgramStoragePhysicalEntryDisposition, OptimizedProgramStorageSemanticEntryContract,
    ProgramEntryPhysicalContractPlan, ProgramEntrySourceReceiverSignature,
    ProgramEntrySourceResultSignature, ProgramEntrySourceSignatureIdentity,
    ProgramStorageEntryRootRole, exact_uefi_x64_physical_boundary_entry_plan,
};
use omega_target::{
    TargetProfile, ValidatedUefiSystemTableHeaderIntegrity, ValidatedUefiSystemTableNativeLayout,
    plan_uefi_system_table_native_layout,
};

use crate::{
    ExternalRootDiagnostic, GeneratedProgramStorageAdapterLiveFrameDemand,
    UefiApplicationBootstrapLedgerId, UefiBootServicesPhaseLeaseId, UefiFirmwareSessionId,
    UefiImageHandleOccurrenceId, UefiPhysicalInvocationId, UefiSystemTableOccurrenceId,
};

mod provider_projection;
pub use provider_projection::*;
mod handle_protocol_provider;
pub use handle_protocol_provider::*;
mod os_handoff;
pub use os_handoff::*;

static NEXT_LEDGER_AUTHORITY: AtomicU64 = AtomicU64::new(1);

fn claim_ledger_authority() -> Result<u64, ExternalRootDiagnostic> {
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
            && provenance.opaque_handle == self.image_handle_value
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
/// invocation. A concrete physical-input admission retains the non-null handle
/// value privately for exact provider invocation; no public handle/address
/// projection exists and the carrier cannot become storage authority.
#[must_use = "UEFI image-handle provenance is a linear physical-arrival input"]
pub struct UefiImageHandleProvenance {
    authority: u64,
    ledger: UefiApplicationBootstrapLedgerId,
    session: UefiFirmwareSessionId,
    invocation: UefiPhysicalInvocationId,
    occurrence: UefiImageHandleOccurrenceId,
    opaque_handle: Option<NonZeroU64>,
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

/// Exact numeric inputs to same-stack UEFI bootstrap planning.
///
/// These coordinates are not WCSU derivation evidence. Later compiler/runtime
/// producers must bind each value to the generated shell, checked adapter,
/// closed continuation/provider graph, and target reserve that produced it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UefiApplicationBootstrapSameStackDemandComponents {
    generated_shell_wcsu_bytes: u64,
    live_adapter_frames_wcsu_bytes: u64,
    maximum_nested_continuation_provider_wcsu_bytes: u64,
    target_reserve_bytes: u64,
}

impl UefiApplicationBootstrapSameStackDemandComponents {
    pub const fn new(
        generated_shell_wcsu_bytes: u64,
        live_adapter_frames_wcsu_bytes: u64,
        maximum_nested_continuation_provider_wcsu_bytes: u64,
        target_reserve_bytes: u64,
    ) -> Self {
        Self {
            generated_shell_wcsu_bytes,
            live_adapter_frames_wcsu_bytes,
            maximum_nested_continuation_provider_wcsu_bytes,
            target_reserve_bytes,
        }
    }

    pub const fn generated_shell_wcsu_bytes(self) -> u64 {
        self.generated_shell_wcsu_bytes
    }

    pub const fn live_adapter_frames_wcsu_bytes(self) -> u64 {
        self.live_adapter_frames_wcsu_bytes
    }

    pub const fn maximum_nested_continuation_provider_wcsu_bytes(self) -> u64 {
        self.maximum_nested_continuation_provider_wcsu_bytes
    }

    pub const fn target_reserve_bytes(self) -> u64 {
        self.target_reserve_bytes
    }
}

/// Planning result for the settled UEFI same-stack inequality.
///
/// The result binds the complete four-term demand to one physical-arrival
/// readiness and the exact target-owned numeric guarantee. The stronger
/// constructor additionally retains exact generated-wrapper evidence for the
/// live-adapter term; the other three terms remain unauthenticated numeric
/// inputs. Neither form is runtime stack/environment admission.
#[must_use = "UEFI same-stack budget plan must be retained for later adapter composition"]
pub struct UefiApplicationBootstrapSameStackBudgetPlan {
    readiness_authority: u64,
    ledger: UefiApplicationBootstrapLedgerId,
    session: UefiFirmwareSessionId,
    invocation: UefiPhysicalInvocationId,
    image_handle_occurrence: UefiImageHandleOccurrenceId,
    system_table_occurrence: UefiSystemTableOccurrenceId,
    phase_lease: UefiBootServicesPhaseLeaseId,
    phase_generation: u64,
    physical_calling_plan_commitment: [u8; 32],
    target_entry_stack_guarantee: omega_target::TargetEntryStackGuarantee,
    components: UefiApplicationBootstrapSameStackDemandComponents,
    generated_adapter_live_frame_demand: Option<GeneratedProgramStorageAdapterLiveFrameDemand>,
    required_entry_stack_bytes: u64,
}

impl std::fmt::Debug for UefiApplicationBootstrapSameStackBudgetPlan {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("UefiApplicationBootstrapSameStackBudgetPlan")
            .field("ledger", &self.ledger)
            .field("session", &self.session)
            .field("invocation", &self.invocation)
            .field("image_handle_occurrence", &self.image_handle_occurrence)
            .field("system_table_occurrence", &self.system_table_occurrence)
            .field("phase_lease", &self.phase_lease)
            .field("components", &self.components)
            .field(
                "required_entry_stack_bytes",
                &self.required_entry_stack_bytes,
            )
            .field(
                "guaranteed_entry_stack_bytes",
                &self
                    .target_entry_stack_guarantee
                    .guaranteed_available_bytes(),
            )
            .finish_non_exhaustive()
    }
}

impl UefiApplicationBootstrapSameStackBudgetPlan {
    pub const fn ledger_id(&self) -> UefiApplicationBootstrapLedgerId {
        self.ledger
    }

    pub const fn firmware_session(&self) -> UefiFirmwareSessionId {
        self.session
    }

    pub const fn physical_invocation(&self) -> UefiPhysicalInvocationId {
        self.invocation
    }

    pub const fn image_handle_occurrence(&self) -> UefiImageHandleOccurrenceId {
        self.image_handle_occurrence
    }

    pub const fn system_table_occurrence(&self) -> UefiSystemTableOccurrenceId {
        self.system_table_occurrence
    }

    pub const fn phase_lease_id(&self) -> UefiBootServicesPhaseLeaseId {
        self.phase_lease
    }

    pub const fn physical_calling_plan_commitment(&self) -> &[u8; 32] {
        &self.physical_calling_plan_commitment
    }

    pub const fn target_entry_stack_guarantee(&self) -> &omega_target::TargetEntryStackGuarantee {
        &self.target_entry_stack_guarantee
    }

    pub const fn components(&self) -> UefiApplicationBootstrapSameStackDemandComponents {
        self.components
    }

    pub const fn required_entry_stack_bytes(&self) -> u64 {
        self.required_entry_stack_bytes
    }

    pub const fn generated_adapter_live_frame_demand(
        &self,
    ) -> Option<&GeneratedProgramStorageAdapterLiveFrameDemand> {
        self.generated_adapter_live_frame_demand.as_ref()
    }

    pub const fn remaining_entry_stack_bytes(&self) -> u64 {
        self.target_entry_stack_guarantee
            .guaranteed_available_bytes()
            - self.required_entry_stack_bytes
    }

    /// Rejoin the planning result to the exact still-live adapter-readiness
    /// custody. Public report coordinates alone cannot substitute a readiness
    /// minted by another private firmware ledger.
    pub fn matches_exact_adapter_readiness(
        &self,
        readiness: &UefiApplicationBootstrapAdapterInvocationReadiness<'_>,
    ) -> bool {
        self.readiness_authority == readiness.arrival.image_handle.authority
            && self.ledger == readiness.ledger_id()
            && self.session == readiness.firmware_session()
            && self.invocation == readiness.physical_invocation()
            && self.image_handle_occurrence == readiness.image_handle_occurrence()
            && self.system_table_occurrence == readiness.system_table_occurrence()
            && self.phase_lease == readiness.arrival.system_table.phase_lease.lease
            && self.phase_generation == readiness.arrival.system_table.phase_lease.generation
            && self.physical_calling_plan_commitment == readiness.physical_calling_plan_commitment
            && &self.target_entry_stack_guarantee
                == readiness.arrival.physical_contract.guaranteed_entry_stack()
    }
}

/// Replay the exact UEFI physical plan and check the settled same-stack
/// bootstrap inequality:
///
/// `shell + live adapter frames + max nested continuation/provider + reserve
/// <= target guarantee`.
///
/// Every contributor must be explicit and nonzero. This closes the numeric
/// planning relation only; component provenance and runtime firmware
/// conformance remain required before invocation admission.
pub fn plan_uefi_application_bootstrap_same_stack_budget(
    readiness: &UefiApplicationBootstrapAdapterInvocationReadiness<'_>,
    components: UefiApplicationBootstrapSameStackDemandComponents,
) -> Result<UefiApplicationBootstrapSameStackBudgetPlan, ExternalRootDiagnostic> {
    plan_uefi_application_bootstrap_same_stack_budget_inner(readiness, components, None)
}

/// Plan the same four-term inequality while deriving the live adapter-frame
/// term from exact installed generated-wrapper evidence. The remaining three
/// terms deliberately stay explicit numeric planning inputs until their own
/// producer joins land.
pub fn plan_uefi_application_bootstrap_same_stack_budget_with_generated_adapter(
    readiness: &UefiApplicationBootstrapAdapterInvocationReadiness<'_>,
    generated_shell_wcsu_bytes: u64,
    generated_adapter_live_frame_demand: GeneratedProgramStorageAdapterLiveFrameDemand,
    maximum_nested_continuation_provider_wcsu_bytes: u64,
    target_reserve_bytes: u64,
) -> Result<UefiApplicationBootstrapSameStackBudgetPlan, ExternalRootDiagnostic> {
    let components = UefiApplicationBootstrapSameStackDemandComponents::new(
        generated_shell_wcsu_bytes,
        generated_adapter_live_frame_demand.bytes(),
        maximum_nested_continuation_provider_wcsu_bytes,
        target_reserve_bytes,
    );
    plan_uefi_application_bootstrap_same_stack_budget_inner(
        readiness,
        components,
        Some(generated_adapter_live_frame_demand),
    )
}

fn plan_uefi_application_bootstrap_same_stack_budget_inner(
    readiness: &UefiApplicationBootstrapAdapterInvocationReadiness<'_>,
    components: UefiApplicationBootstrapSameStackDemandComponents,
    generated_adapter_live_frame_demand: Option<GeneratedProgramStorageAdapterLiveFrameDemand>,
) -> Result<UefiApplicationBootstrapSameStackBudgetPlan, ExternalRootDiagnostic> {
    if !readiness
        .arrival
        .physical_contract
        .matches_exact_uefi_x64_physical_contract()
    {
        return Err(ExternalRootDiagnostic(
            "UEFI same-stack planning requires the exact target-owned physical contract".into(),
        ));
    }
    let expected = exact_uefi_x64_physical_boundary_entry_plan();
    if readiness.physical_calling_plan_commitment != expected.contract_commitment_digest() {
        return Err(ExternalRootDiagnostic(
            "UEFI same-stack planning physical calling-plan commitment drifted".into(),
        ));
    }
    let guarantee = readiness.arrival.physical_contract.guaranteed_entry_stack();
    if !guarantee.matches_exact_uefi_x64_entry_stack_guarantee()
        || guarantee.application()
            != readiness
                .arrival
                .physical_contract
                .guaranteed_entry_stack_application()
        || guarantee.required_alignment() != u64::from(expected.plan().call.stack_alignment)
    {
        return Err(ExternalRootDiagnostic(
            "UEFI same-stack planning target guarantee did not replay the exact physical contract"
                .into(),
        ));
    }

    let contributions = [
        (
            "generated shell WCSU",
            components.generated_shell_wcsu_bytes,
        ),
        (
            "live adapter-frame WCSU",
            components.live_adapter_frames_wcsu_bytes,
        ),
        (
            "maximum nested continuation/provider WCSU",
            components.maximum_nested_continuation_provider_wcsu_bytes,
        ),
        ("explicit target reserve", components.target_reserve_bytes),
    ];
    let mut required_entry_stack_bytes = 0_u64;
    for (name, bytes) in contributions {
        if bytes == 0 {
            return Err(ExternalRootDiagnostic(format!(
                "UEFI same-stack planning omitted {name}"
            )));
        }
        required_entry_stack_bytes =
            required_entry_stack_bytes
                .checked_add(bytes)
                .ok_or_else(|| {
                    ExternalRootDiagnostic(
                        "UEFI same-stack planning demand addition overflowed".into(),
                    )
                })?;
    }
    if required_entry_stack_bytes > guarantee.guaranteed_available_bytes() {
        return Err(ExternalRootDiagnostic(format!(
            "UEFI same-stack bootstrap requires {required_entry_stack_bytes} bytes but the selected target guarantees only {} bytes",
            guarantee.guaranteed_available_bytes(),
        )));
    }

    Ok(UefiApplicationBootstrapSameStackBudgetPlan {
        readiness_authority: readiness.arrival.image_handle.authority,
        ledger: readiness.ledger_id(),
        session: readiness.firmware_session(),
        invocation: readiness.physical_invocation(),
        image_handle_occurrence: readiness.image_handle_occurrence(),
        system_table_occurrence: readiness.system_table_occurrence(),
        phase_lease: readiness.arrival.system_table.phase_lease.lease,
        phase_generation: readiness.arrival.system_table.phase_lease.generation,
        physical_calling_plan_commitment: readiness.physical_calling_plan_commitment,
        target_entry_stack_guarantee: guarantee.clone(),
        components,
        generated_adapter_live_frame_demand,
        required_entry_stack_bytes,
    })
}

/// Exact address-free composition of the target-fixed UEFI physical arrival
/// and the build-selected semantic `ProgramStorageEntry::enter` continuation.
///
/// This is the first retained target-runtime adapter carrier. It owns the
/// private physical-arrival custody, the exact four-term same-stack plan, and
/// the independently checked semantic entry contract. It does not claim that
/// a physical shell was emitted or invoked, that the other three stack
/// contributors have acquired derivation evidence, or that either semantic
/// root exists yet.
#[must_use = "UEFI bootstrap adapter composition retains physical and semantic entry custody"]
pub struct UefiApplicationBootstrapAdapterComposition<'occurrence> {
    readiness: UefiApplicationBootstrapAdapterInvocationReadiness<'occurrence>,
    same_stack_budget: UefiApplicationBootstrapSameStackBudgetPlan,
    semantic_entry: OptimizedProgramStorageSemanticEntryContract,
    semantic_calling_plan_commitment: [u8; 32],
}

impl std::fmt::Debug for UefiApplicationBootstrapAdapterComposition<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("UefiApplicationBootstrapAdapterComposition")
            .field("ledger", &self.ledger_id())
            .field("physical_invocation", &self.physical_invocation())
            .field(
                "physical_requirement_identity",
                &self.physical_requirement_identity(),
            )
            .field(
                "semantic_requirement_identity",
                &self.semantic_requirement_identity(),
            )
            .field(
                "semantic_source_signature_identity",
                &self.semantic_entry.source_signature_identity(),
            )
            .field(
                "required_entry_stack_bytes",
                &self.same_stack_budget.required_entry_stack_bytes(),
            )
            .finish_non_exhaustive()
    }
}

impl UefiApplicationBootstrapAdapterComposition<'_> {
    pub const fn ledger_id(&self) -> UefiApplicationBootstrapLedgerId {
        self.readiness.ledger_id()
    }

    pub const fn physical_invocation(&self) -> UefiPhysicalInvocationId {
        self.readiness.physical_invocation()
    }

    pub fn physical_requirement_identity(&self) -> &str {
        self.readiness.physical_requirement_identity()
    }

    pub fn semantic_requirement_identity(&self) -> &str {
        self.semantic_entry.requirement_identity()
    }

    pub const fn semantic_source_signature_identity(&self) -> ProgramEntrySourceSignatureIdentity {
        self.semantic_entry.source_signature_identity()
    }

    pub const fn same_stack_budget(&self) -> &UefiApplicationBootstrapSameStackBudgetPlan {
        &self.same_stack_budget
    }

    pub const fn semantic_calling_plan_commitment(&self) -> &[u8; 32] {
        &self.semantic_calling_plan_commitment
    }
}

/// Recoverable adapter-composition rejection. No failure drops either the
/// physical arrival, stack plan, or semantic entry contract.
#[derive(Debug)]
#[must_use = "UEFI adapter-composition rejection retains every composition input"]
pub struct UefiApplicationBootstrapAdapterCompositionError<'occurrence> {
    readiness: UefiApplicationBootstrapAdapterInvocationReadiness<'occurrence>,
    same_stack_budget: UefiApplicationBootstrapSameStackBudgetPlan,
    semantic_entry: OptimizedProgramStorageSemanticEntryContract,
    diagnostic: ExternalRootDiagnostic,
}

impl<'occurrence> UefiApplicationBootstrapAdapterCompositionError<'occurrence> {
    pub const fn diagnostic(&self) -> &ExternalRootDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(
        self,
    ) -> (
        UefiApplicationBootstrapAdapterInvocationReadiness<'occurrence>,
        UefiApplicationBootstrapSameStackBudgetPlan,
        OptimizedProgramStorageSemanticEntryContract,
        ExternalRootDiagnostic,
    ) {
        (
            self.readiness,
            self.same_stack_budget,
            self.semantic_entry,
            self.diagnostic,
        )
    }
}

impl std::fmt::Display for UefiApplicationBootstrapAdapterCompositionError<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}

impl std::error::Error for UefiApplicationBootstrapAdapterCompositionError<'_> {}

/// Consume physical readiness, its exact same-stack plan, and one checked
/// semantic entry into the first address-free UEFI bootstrap-adapter carrier.
///
/// The join independently validates the semantic call plan and exact root
/// order, then requires its paired physical contract to be byte-for-byte the
/// contract already retained beneath the private firmware-ledger readiness.
pub fn compose_uefi_application_bootstrap_adapter<'occurrence>(
    readiness: UefiApplicationBootstrapAdapterInvocationReadiness<'occurrence>,
    same_stack_budget: UefiApplicationBootstrapSameStackBudgetPlan,
    semantic_entry: OptimizedProgramStorageSemanticEntryContract,
) -> Result<
    UefiApplicationBootstrapAdapterComposition<'occurrence>,
    Box<UefiApplicationBootstrapAdapterCompositionError<'occurrence>>,
> {
    let reject = |readiness, same_stack_budget, semantic_entry, message: &'static str| {
        Err(Box::new(UefiApplicationBootstrapAdapterCompositionError {
            readiness,
            same_stack_budget,
            semantic_entry,
            diagnostic: ExternalRootDiagnostic(message.into()),
        }))
    };

    if !same_stack_budget.matches_exact_adapter_readiness(&readiness) {
        return reject(
            readiness,
            same_stack_budget,
            semantic_entry,
            "UEFI bootstrap adapter stack plan belongs to different physical readiness custody",
        );
    }
    if semantic_entry.target() != omega_target::NativeTarget::uefi_x64()
        || semantic_entry.target_slot() != readiness.arrival.physical_contract.target_slot()
        || semantic_entry.physical_disposition()
            != OptimizedProgramStoragePhysicalEntryDisposition::PlannedNotInvokedV1
        || semantic_entry.physical_contract() != &readiness.arrival.physical_contract
        || !semantic_entry
            .physical_contract()
            .matches_exact_uefi_x64_physical_contract()
    {
        return reject(
            readiness,
            same_stack_budget,
            semantic_entry,
            "UEFI bootstrap adapter semantic entry does not retain the exact physical arrival contract",
        );
    }
    if semantic_entry.physical_contract().requirement_identity()
        == semantic_entry.requirement_identity()
        || semantic_entry.source_signature().receiver()
            != &ProgramEntrySourceReceiverSignature::Free
        || semantic_entry.source_signature().result() != ProgramEntrySourceResultSignature::Unit
    {
        return reject(
            readiness,
            same_stack_budget,
            semantic_entry,
            "UEFI bootstrap adapter conflates its physical and semantic entry surfaces",
        );
    }
    let [image, initial_storage] = semantic_entry.roots();
    if image.role() != ProgramStorageEntryRootRole::Image
        || image.parameter_index() != 0
        || initial_storage.role() != ProgramStorageEntryRootRole::InitialStorage
        || initial_storage.parameter_index() != 1
    {
        return reject(
            readiness,
            same_stack_budget,
            semantic_entry,
            "UEFI bootstrap adapter semantic roots are not exact Image then InitialStorage",
        );
    }
    let semantic_signature = CallSignature {
        parameters: vec![image.shape(), initial_storage.shape()],
        result: None,
    };
    let semantic_plan = match validate_boundary_entry_plan(
        semantic_entry.semantic_boundary_entry_plan().clone(),
        &semantic_signature,
    ) {
        Ok(plan) => plan,
        Err(_) => {
            return reject(
                readiness,
                same_stack_budget,
                semantic_entry,
                "UEFI bootstrap adapter semantic calling plan failed independent replay",
            );
        }
    };
    let Some(generated_adapter_live_frame_demand) =
        same_stack_budget.generated_adapter_live_frame_demand()
    else {
        return reject(
            readiness,
            same_stack_budget,
            semantic_entry,
            "UEFI bootstrap adapter composition lacks exact generated live-frame evidence",
        );
    };
    if semantic_plan.plan().call.policy != CallingPolicy::MicrosoftX64
        || semantic_plan.plan().call.parameters.len() != 2
        || semantic_plan.plan().call.result.is_some()
        || semantic_plan.contract_report_fingerprint()
            != semantic_entry.semantic_calling_plan_report_fingerprint()
        || semantic_plan.contract_report_fingerprint()
            == readiness
                .arrival
                .physical_contract
                .calling_plan_report_fingerprint()
        || generated_adapter_live_frame_demand.semantic_boundary_commitment()
            != semantic_plan.contract_commitment_digest()
        || generated_adapter_live_frame_demand.bytes()
            != same_stack_budget.components.live_adapter_frames_wcsu_bytes
        || generated_adapter_live_frame_demand.alignment()
            != u64::from(semantic_plan.plan().call.stack_alignment)
    {
        return reject(
            readiness,
            same_stack_budget,
            semantic_entry,
            "UEFI bootstrap adapter semantic and physical ABI identities are not distinct and exact",
        );
    }

    Ok(UefiApplicationBootstrapAdapterComposition {
        readiness,
        same_stack_budget,
        semantic_entry,
        semantic_calling_plan_commitment: semantic_plan.contract_commitment_digest(),
    })
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
    use omega_calling_conventions::{
        MachineRegister, ValueLocation, ValueShape, evaluate_ordinary_boundary_entry_plan,
    };
    use omega_effects::provider_plan::{
        BoundaryCallingPlanCommitment, ServiceEntryAuthorityFlow, ServiceEntryClaim, ServiceMethod,
        ServiceSchema,
    };
    use omega_program_entry_plan::{
        ProgramEntrySourceExtentValueLayout, SelectedProgramEntrySourceSignature,
        SelectedProgramStorageEntryPlan, UEFI_X64_IMAGE_HANDLE_TYPE_IDENTITY,
        UEFI_X64_PHYSICAL_REQUIREMENT_IDENTITY, UEFI_X64_STATUS_TYPE_IDENTITY,
        UEFI_X64_SYSTEM_TABLE_REFERENCE_TYPE_IDENTITY,
        bind_optimized_program_storage_semantic_entry_contract,
        exact_uefi_x64_physical_boundary_entry_plan,
        exact_uefi_x64_physical_contract_package_source_digest,
    };
    use omega_target::{
        ProgramEntryPhysicalContractPackage, UEFI_SYSTEM_TABLE_SIGNATURE,
        validate_uefi_system_table_occurrence,
    };
    use psi_language_semantics::{CarryPolicy, DomainPredicateBody};
    use psi_symbols::SymbolHandle;

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

    fn semantic_entry_contract(
        physical_contract: ProgramEntryPhysicalContractPlan,
    ) -> OptimizedProgramStorageSemanticEntryContract {
        const REQUIREMENT: &str = "ProgramStorageEntry::enter#uefi-bootstrap";
        const IMAGE_TYPE: &str = "Extent in Granted#image";
        const STORAGE_TYPE: &str = "Extent in Granted#initial-storage";
        const EXTENT_CARRIER: &str = "named(name(Extent))";
        const GRANTED_DOMAIN: &str = "Extent::Granted";

        let extent = ValueShape::integer(16, 8);
        let field = ValueShape::integer(8, 8);
        let semantic = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::MicrosoftX64,
            &CallSignature {
                parameters: vec![extent, extent],
                result: None,
            },
        )
        .unwrap();
        let claim = |parameter_index| ServiceEntryClaim {
            parameter_index,
            carrier_identity: EXTENT_CARRIER.into(),
            domain: GRANTED_DOMAIN.into(),
            predicate_body: DomainPredicateBody::Present,
            effective_carry: CarryPolicy::STRICT,
            authority_flow: ServiceEntryAuthorityFlow::Accepts,
        };
        let method = ServiceMethod {
            name: "enter".into(),
            requirement_owner: "ProgramStorageEntry".into(),
            requirement_identity: REQUIREMENT.into(),
            parameter_count: 2,
            parameter_type_identities: vec![IMAGE_TYPE.into(), STORAGE_TYPE.into()],
            entry_claims: vec![claim(0), claim(1)],
            calling_plan_report_fingerprint: Some(semantic.contract_report_fingerprint()),
            calling_plan_commitment: Some(BoundaryCallingPlanCommitment::from_digest(
                semantic.contract_commitment_digest(),
            )),
            ..Default::default()
        };
        let slot = TargetProfile::UefiX64.program_entry_slot();
        let selected = SelectedProgramStorageEntryPlan::from_target_slot(
            slot,
            ServiceSchema {
                trait_name: slot.boundary_schema.unwrap().into(),
                methods: vec![method],
                ..Default::default()
            },
            REQUIREMENT.into(),
        )
        .unwrap()
        .with_physical_contract(physical_contract)
        .unwrap();
        let extent_layout = |base| {
            ProgramEntrySourceExtentValueLayout::from_checked_record(
                SymbolHandle::from_arena_index(base),
                SymbolHandle::from_arena_index(base + 1),
                0,
                field,
                SymbolHandle::from_arena_index(base + 2),
                8,
                field,
                extent,
            )
            .unwrap()
        };
        let source = SelectedProgramEntrySourceSignature::from_checked_typed_entry(
            slot,
            SymbolHandle::from_arena_index(1),
            SymbolHandle::from_arena_index(2),
            "Bootstrap::continue".into(),
            "continue".into(),
            "Bootstrap::continue#uefi".into(),
            ProgramEntrySourceReceiverSignature::Free,
            vec![
                SelectedProgramEntrySourceSignature::visible_parameter(
                    ProgramStorageEntryRootRole::Image,
                    0,
                    IMAGE_TYPE.into(),
                    extent,
                    extent_layout(10),
                    false,
                    false,
                ),
                SelectedProgramEntrySourceSignature::visible_parameter(
                    ProgramStorageEntryRootRole::InitialStorage,
                    1,
                    STORAGE_TYPE.into(),
                    extent,
                    extent_layout(20),
                    false,
                    false,
                ),
            ],
        )
        .unwrap();
        bind_optimized_program_storage_semantic_entry_contract(
            omega_target::NativeTarget::uefi_x64(),
            &selected,
            &source,
            semantic.plan(),
        )
        .unwrap()
    }

    fn exact_semantic_entry_contract() -> OptimizedProgramStorageSemanticEntryContract {
        semantic_entry_contract(exact_physical_contract())
    }

    fn authenticated_adapter_budget(
        readiness: &UefiApplicationBootstrapAdapterInvocationReadiness<'_>,
    ) -> UefiApplicationBootstrapSameStackBudgetPlan {
        plan_uefi_application_bootstrap_same_stack_budget_with_generated_adapter(
            readiness,
            4 * 1024,
            crate::tests::generated_program_storage_adapter_live_frame_demand(),
            96 * 1024,
            16 * 1024,
        )
        .expect("generated wrapper live-frame evidence fits the UEFI guarantee")
    }

    fn adapter_readiness<'a>(
        ledger: &mut UefiApplicationFirmwareLedger<'a>,
        bytes: &'a [u8],
        base: u64,
    ) -> UefiApplicationBootstrapAdapterInvocationReadiness<'a> {
        let image_handle = ledger
            .admit_image_handle_occurrence(id(
                base,
                UefiImageHandleOccurrenceId::from_normalized_identity,
            ))
            .unwrap();
        let (integrity, provenance, lease) = inputs(ledger, bytes, base + 1, base + 2);
        let system_table =
            join_lifecycle_scoped_uefi_system_table(ledger, integrity, provenance, lease).unwrap();
        let arrival = join_uefi_application_physical_arrival(
            ledger,
            image_handle,
            system_table,
            exact_physical_contract(),
        )
        .unwrap();
        prepare_uefi_application_bootstrap_adapter_invocation(ledger, arrival).unwrap()
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
    fn same_stack_budget_replays_target_guarantee_and_all_four_contributors() {
        let bytes = valid_occurrence(120);
        let mut ledger = ledger(150);
        let readiness = adapter_readiness(&mut ledger, &bytes, 153);
        let components = UefiApplicationBootstrapSameStackDemandComponents::new(
            4 * 1024,
            8 * 1024,
            96 * 1024,
            16 * 1024,
        );
        let plan = plan_uefi_application_bootstrap_same_stack_budget(&readiness, components)
            .expect("four-term UEFI stack demand fits the target guarantee");

        assert_eq!(plan.ledger_id(), ledger.ledger_id());
        assert_eq!(plan.firmware_session(), ledger.firmware_session());
        assert_eq!(plan.physical_invocation(), ledger.physical_invocation());
        assert_eq!(
            plan.image_handle_occurrence(),
            readiness.image_handle_occurrence()
        );
        assert_eq!(
            plan.system_table_occurrence(),
            readiness.system_table_occurrence()
        );
        assert_eq!(
            plan.phase_lease_id(),
            readiness.arrival.system_table.phase_lease_id()
        );
        assert!(plan.matches_exact_adapter_readiness(&readiness));
        assert_eq!(plan.components(), components);
        assert_eq!(plan.required_entry_stack_bytes(), 124 * 1024);
        assert_eq!(plan.remaining_entry_stack_bytes(), 4 * 1024);
        assert_eq!(
            plan.target_entry_stack_guarantee()
                .guaranteed_available_bytes(),
            128 * 1024,
        );
        assert_eq!(plan.target_entry_stack_guarantee().required_alignment(), 16,);
        assert_eq!(
            plan.physical_calling_plan_commitment(),
            readiness.physical_calling_plan_commitment(),
        );

        let exact_boundary = plan_uefi_application_bootstrap_same_stack_budget(
            &readiness,
            UefiApplicationBootstrapSameStackDemandComponents::new(
                4 * 1024,
                8 * 1024,
                100 * 1024,
                16 * 1024,
            ),
        )
        .expect("the exact target-guarantee boundary is admitted");
        assert_eq!(exact_boundary.required_entry_stack_bytes(), 128 * 1024);
        assert_eq!(exact_boundary.remaining_entry_stack_bytes(), 0);

        let UefiApplicationBootstrapAdapterInvocationReadiness { arrival, .. } = readiness;
        let UefiApplicationPhysicalArrival { system_table, .. } = arrival;
        ledger
            .release_lifecycle_scoped_system_table(system_table)
            .unwrap();
    }

    #[test]
    fn adapter_composition_retains_exact_physical_semantic_and_stack_custody() {
        let bytes = valid_occurrence(120);
        let mut ledger = ledger(210);
        let readiness = adapter_readiness(&mut ledger, &bytes, 213);
        let budget = authenticated_adapter_budget(&readiness);
        let adapter = compose_uefi_application_bootstrap_adapter(
            readiness,
            budget,
            exact_semantic_entry_contract(),
        )
        .unwrap();

        assert_eq!(adapter.ledger_id(), ledger.ledger_id());
        assert_eq!(adapter.physical_invocation(), ledger.physical_invocation());
        assert_eq!(
            adapter.physical_requirement_identity(),
            UEFI_X64_PHYSICAL_REQUIREMENT_IDENTITY,
        );
        assert_eq!(
            adapter.semantic_requirement_identity(),
            "ProgramStorageEntry::enter#uefi-bootstrap",
        );
        assert_ne!(
            adapter.physical_requirement_identity(),
            adapter.semantic_requirement_identity(),
        );
        assert_eq!(
            adapter.same_stack_budget().required_entry_stack_bytes(),
            4 * 1024 + 72 + 96 * 1024 + 16 * 1024,
        );
        assert_ne!(
            adapter.semantic_source_signature_identity().bytes(),
            [0; 32]
        );
        assert_ne!(adapter.semantic_calling_plan_commitment(), &[0; 32]);
        assert_ne!(
            adapter.semantic_calling_plan_commitment(),
            &exact_uefi_x64_physical_boundary_entry_plan().contract_commitment_digest(),
        );

        let UefiApplicationBootstrapAdapterComposition { readiness, .. } = adapter;
        let UefiApplicationBootstrapAdapterInvocationReadiness { arrival, .. } = readiness;
        let UefiApplicationPhysicalArrival { system_table, .. } = arrival;
        ledger
            .release_lifecycle_scoped_system_table(system_table)
            .unwrap();
    }

    #[test]
    fn adapter_composition_rejects_missing_or_substituted_generated_frame_evidence() {
        let bytes = valid_occurrence(120);
        let mut ledger = ledger(215);
        let readiness = adapter_readiness(&mut ledger, &bytes, 218);
        let numeric_only = plan_uefi_application_bootstrap_same_stack_budget(
            &readiness,
            UefiApplicationBootstrapSameStackDemandComponents::new(
                4 * 1024,
                72,
                96 * 1024,
                16 * 1024,
            ),
        )
        .unwrap();
        let error = compose_uefi_application_bootstrap_adapter(
            readiness,
            numeric_only,
            exact_semantic_entry_contract(),
        )
        .expect_err("a numerically equal live-frame assertion is not generated evidence");
        assert!(
            error
                .diagnostic()
                .0
                .contains("lacks exact generated live-frame evidence")
        );
        let (readiness, _, _, _) = error.into_parts();

        let substituted = crate::tests::generated_program_storage_adapter_live_frame_demand()
            .with_semantic_boundary_commitment_for_test([0x5a; 32]);
        let budget = plan_uefi_application_bootstrap_same_stack_budget_with_generated_adapter(
            &readiness,
            4 * 1024,
            substituted,
            96 * 1024,
            16 * 1024,
        )
        .unwrap();
        let error = compose_uefi_application_bootstrap_adapter(
            readiness,
            budget,
            exact_semantic_entry_contract(),
        )
        .expect_err("generated evidence for another semantic ABI must reject");
        assert!(
            error
                .diagnostic()
                .0
                .contains("semantic and physical ABI identities are not distinct and exact")
        );
        let (readiness, _, _, _) = error.into_parts();
        let UefiApplicationBootstrapAdapterInvocationReadiness { arrival, .. } = readiness;
        let UefiApplicationPhysicalArrival { system_table, .. } = arrival;
        ledger
            .release_lifecycle_scoped_system_table(system_table)
            .unwrap();
    }

    #[test]
    fn adapter_composition_rejects_cross_ledger_stack_plan_and_returns_retry_inputs() {
        let first_bytes = valid_occurrence(120);
        let second_bytes = valid_occurrence(120);
        let mut first_ledger = ledger(220);
        let mut second_ledger = ledger(220);
        let first_readiness = adapter_readiness(&mut first_ledger, &first_bytes, 223);
        let second_readiness = adapter_readiness(&mut second_ledger, &second_bytes, 223);
        let first_budget = authenticated_adapter_budget(&first_readiness);

        let error = compose_uefi_application_bootstrap_adapter(
            second_readiness,
            first_budget,
            exact_semantic_entry_contract(),
        )
        .unwrap_err();
        assert!(
            error
                .diagnostic()
                .0
                .contains("different physical readiness")
        );
        let (second_readiness, first_budget, semantic_entry, _) = error.into_parts();
        assert!(!first_budget.matches_exact_adapter_readiness(&second_readiness));

        let UefiApplicationBootstrapAdapterInvocationReadiness {
            arrival: second_arrival,
            ..
        } = second_readiness;
        let UefiApplicationPhysicalArrival {
            system_table: second_table,
            ..
        } = second_arrival;
        second_ledger
            .release_lifecycle_scoped_system_table(second_table)
            .unwrap();

        let adapter = compose_uefi_application_bootstrap_adapter(
            first_readiness,
            first_budget,
            semantic_entry,
        )
        .unwrap();
        let UefiApplicationBootstrapAdapterComposition { readiness, .. } = adapter;
        let UefiApplicationBootstrapAdapterInvocationReadiness { arrival, .. } = readiness;
        let UefiApplicationPhysicalArrival { system_table, .. } = arrival;
        first_ledger
            .release_lifecycle_scoped_system_table(system_table)
            .unwrap();
    }

    #[test]
    fn adapter_composition_rejects_noncanonical_physical_contract_in_semantic_entry() {
        let bytes = valid_occurrence(120);
        let mut ledger = ledger(230);
        let readiness = adapter_readiness(&mut ledger, &bytes, 233);
        let budget = authenticated_adapter_budget(&readiness);
        let semantic_entry = semantic_entry_contract(physical_contract(
            "UefiPhysicalEntry::enter#lookalike",
            |_| {},
        ));

        let error = compose_uefi_application_bootstrap_adapter(readiness, budget, semantic_entry)
            .unwrap_err();
        assert!(
            error
                .diagnostic()
                .0
                .contains("exact physical arrival contract"),
        );
        let (readiness, budget, _, _) = error.into_parts();
        assert!(budget.matches_exact_adapter_readiness(&readiness));
        let UefiApplicationBootstrapAdapterInvocationReadiness { arrival, .. } = readiness;
        let UefiApplicationPhysicalArrival { system_table, .. } = arrival;
        ledger
            .release_lifecycle_scoped_system_table(system_table)
            .unwrap();
    }

    #[test]
    fn same_stack_budget_rejects_public_coordinate_substitution_across_private_ledgers() {
        let first_bytes = valid_occurrence(120);
        let second_bytes = valid_occurrence(120);
        let mut first_ledger = ledger(180);
        let mut second_ledger = ledger(180);
        let first_readiness = adapter_readiness(&mut first_ledger, &first_bytes, 183);
        let second_readiness = adapter_readiness(&mut second_ledger, &second_bytes, 183);
        assert_eq!(first_readiness.ledger_id(), second_readiness.ledger_id());
        assert_eq!(
            first_readiness.firmware_session(),
            second_readiness.firmware_session()
        );
        assert_eq!(
            first_readiness.physical_invocation(),
            second_readiness.physical_invocation()
        );
        assert_eq!(
            first_readiness.image_handle_occurrence(),
            second_readiness.image_handle_occurrence()
        );
        assert_eq!(
            first_readiness.system_table_occurrence(),
            second_readiness.system_table_occurrence()
        );

        let plan = plan_uefi_application_bootstrap_same_stack_budget(
            &first_readiness,
            UefiApplicationBootstrapSameStackDemandComponents::new(
                4 * 1024,
                8 * 1024,
                96 * 1024,
                16 * 1024,
            ),
        )
        .unwrap();
        assert!(plan.matches_exact_adapter_readiness(&first_readiness));
        assert!(!plan.matches_exact_adapter_readiness(&second_readiness));

        let UefiApplicationBootstrapAdapterInvocationReadiness {
            arrival: first_arrival,
            ..
        } = first_readiness;
        let UefiApplicationPhysicalArrival {
            system_table: first_table,
            ..
        } = first_arrival;
        first_ledger
            .release_lifecycle_scoped_system_table(first_table)
            .unwrap();
        let UefiApplicationBootstrapAdapterInvocationReadiness {
            arrival: second_arrival,
            ..
        } = second_readiness;
        let UefiApplicationPhysicalArrival {
            system_table: second_table,
            ..
        } = second_arrival;
        second_ledger
            .release_lifecycle_scoped_system_table(second_table)
            .unwrap();
    }

    #[test]
    fn same_stack_budget_rejects_every_omitted_contributor_and_overflow() {
        let bytes = valid_occurrence(120);
        let mut ledger = ledger(160);
        let readiness = adapter_readiness(&mut ledger, &bytes, 163);
        let exact = [4 * 1024, 8 * 1024, 96 * 1024, 16 * 1024];
        for omitted in 0..exact.len() {
            let mut values = exact;
            values[omitted] = 0;
            let error = plan_uefi_application_bootstrap_same_stack_budget(
                &readiness,
                UefiApplicationBootstrapSameStackDemandComponents::new(
                    values[0], values[1], values[2], values[3],
                ),
            )
            .unwrap_err();
            assert!(error.0.contains("omitted"));
        }

        let overflow = plan_uefi_application_bootstrap_same_stack_budget(
            &readiness,
            UefiApplicationBootstrapSameStackDemandComponents::new(u64::MAX, 1, 1, 1),
        )
        .unwrap_err();
        assert!(overflow.0.contains("overflowed"));

        let UefiApplicationBootstrapAdapterInvocationReadiness { arrival, .. } = readiness;
        let UefiApplicationPhysicalArrival { system_table, .. } = arrival;
        ledger
            .release_lifecycle_scoped_system_table(system_table)
            .unwrap();
    }

    #[test]
    fn same_stack_budget_rejects_demand_above_the_target_guarantee() {
        let bytes = valid_occurrence(120);
        let mut ledger = ledger(170);
        let readiness = adapter_readiness(&mut ledger, &bytes, 173);
        let error = plan_uefi_application_bootstrap_same_stack_budget(
            &readiness,
            UefiApplicationBootstrapSameStackDemandComponents::new(
                4 * 1024,
                8 * 1024,
                112 * 1024,
                16 * 1024,
            ),
        )
        .unwrap_err();
        assert!(error.0.contains("guarantees only 131072 bytes"));

        let UefiApplicationBootstrapAdapterInvocationReadiness { arrival, .. } = readiness;
        let UefiApplicationPhysicalArrival { system_table, .. } = arrival;
        ledger
            .release_lifecycle_scoped_system_table(system_table)
            .unwrap();
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
            "pubstructUefiImageHandleProvenance{authority:u64,ledger:UefiApplicationBootstrapLedgerId,session:UefiFirmwareSessionId,invocation:UefiPhysicalInvocationId,occurrence:UefiImageHandleOccurrenceId,opaque_handle:Option<NonZeroU64>,}"
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

        let composition = item_block(
            source,
            "pub struct UefiApplicationBootstrapAdapterComposition<'occurrence>",
        );
        let compact_composition = composition
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect::<String>();
        assert_eq!(
            compact_composition,
            "pubstructUefiApplicationBootstrapAdapterComposition<'occurrence>{readiness:UefiApplicationBootstrapAdapterInvocationReadiness<'occurrence>,same_stack_budget:UefiApplicationBootstrapSameStackBudgetPlan,semantic_entry:OptimizedProgramStorageSemanticEntryContract,semantic_calling_plan_commitment:[u8;32],}"
        );
        let composition_impl = item_block(
            source,
            "impl UefiApplicationBootstrapAdapterComposition<'_>",
        );
        assert_eq!(
            public_method_names(composition_impl),
            [
                "ledger_id",
                "physical_invocation",
                "physical_requirement_identity",
                "semantic_requirement_identity",
                "semantic_source_signature_identity",
                "same_stack_budget",
                "semantic_calling_plan_commitment",
            ]
        );
        assert_eq!(
            compact_source
                .matches("forUefiApplicationBootstrapAdapterComposition<'_>{")
                .count(),
            1,
            "adapter composition must implement only report-only Debug",
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
            "impl From<UefiApplicationBootstrapAdapterComposition",
            "impl Into<UefiApplicationBootstrapAdapterComposition",
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

//! Returning-application UEFI system-table lifecycle composition.
//!
//! Header integrity is deliberately weaker than permission to use firmware
//! services. This module joins that target-owned integrity evidence to the
//! exact physical-arrival occurrence and a current Boot-Services-live phase
//! lease. The result remains a metadata-only lifecycle carrier: service-field
//! projection belongs to a later provider-specific edge.

use std::sync::atomic::{AtomicU64, Ordering};

use omega_target::{
    TargetProfile, ValidatedUefiSystemTableHeaderIntegrity, ValidatedUefiSystemTableNativeLayout,
    plan_uefi_system_table_native_layout,
};

use crate::{
    ExternalRootDiagnostic, UefiApplicationBootstrapLedgerId, UefiBootServicesPhaseLeaseId,
    UefiFirmwareSessionId, UefiPhysicalInvocationId, UefiSystemTableOccurrenceId,
};

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

#[cfg(test)]
mod tests {
    use super::*;
    use omega_target::{UEFI_SYSTEM_TABLE_SIGNATURE, validate_uefi_system_table_occurrence};

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

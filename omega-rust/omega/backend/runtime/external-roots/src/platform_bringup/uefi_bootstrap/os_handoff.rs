//! Linear state ownership for the UEFI OS-loader handoff profile.
//!
//! This is deliberately separate from the returning `UefiApplication`
//! ledger. It models one target-bounded `GetMemoryMap` / `ExitBootServices`
//! cycle and retains every non-copy input across retry. Native provider
//! invocation and final-map policy are later boundaries.

use std::collections::BTreeSet;
use std::num::NonZeroU32;

use crate::{
    ExternalRootDiagnostic, UefiExitBootServicesReceiptId, UefiFirmwareSessionId,
    UefiMemoryMapKeyId, UefiMemoryMapSnapshotId, UefiOsHandoffAllocationRosterId,
    UefiOsHandoffBootServicesId, UefiOsHandoffId, UefiOsHandoffStackEvidenceId,
    UefiPhysicalInvocationId,
};

use super::UefiMemoryMapAcquisition;
use crate::platform_bringup::uefi_bootstrap::firmware_ledger::claim_ledger_authority;
use program_entry_plan::uefi_os_handoff_exhaustion_requires_error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UefiOsHandoffPhase {
    MapRequired,
    ExitAttempt,
    Exited,
    Exhausted,
}

/// Target-authored error returned when every bounded stale-key retry is spent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UefiErrorStatus(u64);

impl UefiErrorStatus {
    /// The planned OS-handoff invocation admits only a target-authored EFI
    /// error as the exhaustion result; success and warning values are
    /// rejected here, so `UefiOsHandoffLedger::new` receives an already
    /// gated carrier.
    pub fn from_target_status(status: u64) -> Result<Self, ExternalRootDiagnostic> {
        if !uefi_os_handoff_exhaustion_requires_error(status) {
            return Err(ExternalRootDiagnostic(
                "UEFI OS-handoff exhaustion status must be a target-authored EFI error".into(),
            ));
        }
        Ok(Self(status))
    }

    pub const fn value(self) -> u64 {
        self.0
    }
}

/// Private-authority ledger for one non-returning OS-loader invocation.
pub struct UefiOsHandoffLedger {
    authority: u64,
    handoff: UefiOsHandoffId,
    session: UefiFirmwareSessionId,
    invocation: UefiPhysicalInvocationId,
    boot_services: UefiOsHandoffBootServicesId,
    allocations: UefiOsHandoffAllocationRosterId,
    surviving_stack: UefiOsHandoffStackEvidenceId,
    remaining: u32,
    exhaustion_status: UefiErrorStatus,
    generation: u64,
    phase: UefiOsHandoffPhase,
    retired_maps: BTreeSet<(UefiMemoryMapSnapshotId, UefiMemoryMapKeyId)>,
}

impl std::fmt::Debug for UefiOsHandoffLedger {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("UefiOsHandoffLedger")
            .field("handoff", &self.handoff)
            .field("session", &self.session)
            .field("invocation", &self.invocation)
            .field("remaining", &self.remaining)
            .field("generation", &self.generation)
            .field("phase", &self.phase)
            .finish_non_exhaustive()
    }
}

impl UefiOsHandoffLedger {
    /// Admit the target-owned handoff inputs and create the first map-
    /// acquisition arrival. The attempt limit and exhaustion status are
    /// explicit deployment inputs, not policy selected by this adapter.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        handoff: UefiOsHandoffId,
        session: UefiFirmwareSessionId,
        invocation: UefiPhysicalInvocationId,
        boot_services: UefiOsHandoffBootServicesId,
        allocations: UefiOsHandoffAllocationRosterId,
        surviving_stack: UefiOsHandoffStackEvidenceId,
        attempt_limit: NonZeroU32,
        exhaustion_status: UefiErrorStatus,
    ) -> Result<(Self, UefiOsHandoffMapRequired), ExternalRootDiagnostic> {
        let authority = claim_ledger_authority()?;
        let remaining = attempt_limit.get();
        let ledger = Self {
            authority,
            handoff,
            session,
            invocation,
            boot_services,
            allocations,
            surviving_stack,
            remaining,
            exhaustion_status,
            generation: 1,
            phase: UefiOsHandoffPhase::MapRequired,
            retired_maps: BTreeSet::new(),
        };
        let arrival = ledger.map_required();
        Ok((ledger, arrival))
    }

    pub const fn handoff_id(&self) -> UefiOsHandoffId {
        self.handoff
    }

    pub const fn firmware_session(&self) -> UefiFirmwareSessionId {
        self.session
    }

    pub const fn physical_invocation(&self) -> UefiPhysicalInvocationId {
        self.invocation
    }

    /// Consume the current arrival and bind the acquisition evidence executed
    /// by the `get_memory_map` provider edge. The snapshot/key pair, its
    /// descriptor geometry, and the firmware-session and physical-invocation
    /// lineage all arrive sealed inside `UefiMemoryMapAcquisition`; a
    /// rejection preserves the complete arrival and the unspent acquisition.
    pub fn acquire_memory_map(
        &mut self,
        arrival: UefiOsHandoffMapRequired,
        acquisition: UefiMemoryMapAcquisition,
    ) -> Result<UefiOsHandoffMapAcquired, Box<UefiOsHandoffMapAcquisitionError>> {
        let reject = |arrival, acquisition, message: &'static str| {
            Err(Box::new(UefiOsHandoffMapAcquisitionError {
                arrival,
                acquisition,
                diagnostic: ExternalRootDiagnostic(message.into()),
            }))
        };
        if !self.matches_arrival(&arrival) {
            return reject(
                arrival,
                acquisition,
                "UEFI OS-handoff map arrival is foreign, stale, or has lost custody",
            );
        }
        if acquisition.physical_invocation() != self.invocation
            || acquisition.firmware_session() != self.session
        {
            return reject(
                arrival,
                acquisition,
                "UEFI OS-handoff map acquisition belongs to a different physical invocation or firmware session",
            );
        }
        if self
            .retired_maps
            .contains(&(acquisition.snapshot(), acquisition.map_key()))
        {
            return reject(
                arrival,
                acquisition,
                "UEFI OS-handoff cannot reacquire a retired snapshot/key pair",
            );
        }
        self.phase = UefiOsHandoffPhase::ExitAttempt;
        Ok(UefiOsHandoffMapAcquired {
            arrival,
            acquisition,
        })
    }

    /// Apply exactly one target-provider result. A stale key retires the map
    /// and either returns all live custody with a smaller measure or reaches
    /// the target-authored exhaustion status. Success consumes Boot Services
    /// and transfers the exact allocation lineage and surviving stack.
    pub fn apply_exit_boot_services_result(
        &mut self,
        acquired: UefiOsHandoffMapAcquired,
        result: UefiExitBootServicesProviderResult,
    ) -> Result<UefiOsHandoffProgress, Box<UefiOsHandoffTransitionError>> {
        if !self.matches_acquired(&acquired) {
            return Err(Box::new(UefiOsHandoffTransitionError {
                acquired,
                result,
                diagnostic: ExternalRootDiagnostic(
                    "UEFI ExitBootServices attempt is foreign, stale, reordered, or incomplete"
                        .into(),
                ),
            }));
        }
        let Some(next_generation) = self.generation.checked_add(1) else {
            return Err(Box::new(UefiOsHandoffTransitionError {
                acquired,
                result,
                diagnostic: ExternalRootDiagnostic("UEFI OS-handoff generation overflowed".into()),
            }));
        };
        match result.kind {
            UefiExitBootServicesProviderResultKind::Succeeded { receipt } => {
                self.phase = UefiOsHandoffPhase::Exited;
                self.generation = next_generation;
                Ok(UefiOsHandoffProgress::Complete(UefiOsHandoffComplete {
                    handoff: self.handoff,
                    session: self.session,
                    invocation: self.invocation,
                    allocations: self.allocations,
                    final_map: acquired.acquisition.snapshot(),
                    surviving_stack: self.surviving_stack,
                    receipt,
                }))
            }
            UefiExitBootServicesProviderResultKind::StaleMapKey => {
                self.retired_maps.insert((
                    acquired.acquisition.snapshot(),
                    acquired.acquisition.map_key(),
                ));
                self.generation = next_generation;
                if self.remaining == 1 {
                    self.remaining = 0;
                    self.phase = UefiOsHandoffPhase::Exhausted;
                    Ok(UefiOsHandoffProgress::Exhausted(UefiOsHandoffExhausted {
                        handoff: self.handoff,
                        session: self.session,
                        invocation: self.invocation,
                        boot_services: self.boot_services,
                        allocations: self.allocations,
                        surviving_stack: self.surviving_stack,
                        status: self.exhaustion_status,
                    }))
                } else {
                    self.remaining -= 1;
                    self.phase = UefiOsHandoffPhase::MapRequired;
                    Ok(UefiOsHandoffProgress::Retry(self.map_required()))
                }
            }
        }
    }

    fn map_required(&self) -> UefiOsHandoffMapRequired {
        UefiOsHandoffMapRequired {
            authority: self.authority,
            handoff: self.handoff,
            session: self.session,
            invocation: self.invocation,
            boot_services: self.boot_services,
            allocations: self.allocations,
            surviving_stack: self.surviving_stack,
            remaining: self.remaining,
            generation: self.generation,
        }
    }

    fn matches_common(&self, arrival: &UefiOsHandoffMapRequired) -> bool {
        arrival.authority == self.authority
            && arrival.handoff == self.handoff
            && arrival.session == self.session
            && arrival.invocation == self.invocation
            && arrival.boot_services == self.boot_services
            && arrival.allocations == self.allocations
            && arrival.surviving_stack == self.surviving_stack
            && arrival.remaining == self.remaining
            && arrival.generation == self.generation
    }

    fn matches_arrival(&self, arrival: &UefiOsHandoffMapRequired) -> bool {
        self.phase == UefiOsHandoffPhase::MapRequired
            && self.remaining != 0
            && self.matches_common(arrival)
    }

    fn matches_acquired(&self, acquired: &UefiOsHandoffMapAcquired) -> bool {
        self.phase == UefiOsHandoffPhase::ExitAttempt
            && self.remaining != 0
            && self.matches_common(&acquired.arrival)
    }
}

/// Complete arrival contract for one map acquisition attempt.
#[must_use = "UEFI OS-handoff arrival retains all live retry custody"]
pub struct UefiOsHandoffMapRequired {
    authority: u64,
    handoff: UefiOsHandoffId,
    session: UefiFirmwareSessionId,
    invocation: UefiPhysicalInvocationId,
    boot_services: UefiOsHandoffBootServicesId,
    allocations: UefiOsHandoffAllocationRosterId,
    surviving_stack: UefiOsHandoffStackEvidenceId,
    remaining: u32,
    generation: u64,
}

impl UefiOsHandoffMapRequired {
    pub const fn remaining_attempts(&self) -> u32 {
        self.remaining
    }

    pub const fn allocation_roster(&self) -> UefiOsHandoffAllocationRosterId {
        self.allocations
    }

    pub const fn surviving_stack(&self) -> UefiOsHandoffStackEvidenceId {
        self.surviving_stack
    }
}

/// Executed acquisition evidence paired with all live attempt custody.
#[must_use = "acquired UEFI map must be attempted or returned intact"]
pub struct UefiOsHandoffMapAcquired {
    arrival: UefiOsHandoffMapRequired,
    acquisition: UefiMemoryMapAcquisition,
}

impl std::fmt::Debug for UefiOsHandoffMapAcquired {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("UefiOsHandoffMapAcquired")
            .field("handoff", &self.arrival.handoff)
            .field("remaining", &self.arrival.remaining)
            .field("snapshot", &self.acquisition.snapshot())
            .field("key", &self.acquisition.map_key())
            .finish_non_exhaustive()
    }
}

impl UefiOsHandoffMapAcquired {
    pub const fn snapshot(&self) -> UefiMemoryMapSnapshotId {
        self.acquisition.snapshot()
    }

    /// The exact physical key the most recent `GetMemoryMap` wrote; the
    /// `ExitBootServices` binding carries it as the `RDX` operand identity.
    pub const fn map_key(&self) -> UefiMemoryMapKeyId {
        self.acquisition.map_key()
    }

    /// Occupied map bytes, descriptor stride, and descriptor version observed
    /// by the acquiring execution; the OS-entry plan consumes the same
    /// geometry when it walks the transferred map.
    pub const fn map_bytes(&self) -> usize {
        self.acquisition.map_bytes()
    }

    pub const fn descriptor_size(&self) -> usize {
        self.acquisition.descriptor_size()
    }

    pub const fn descriptor_version(&self) -> u32 {
        self.acquisition.descriptor_version()
    }

    /// Report identity of the handoff attempt this map belongs to. The
    /// ExitBootServices provider edge joins it to live invocation custody
    /// before binding the physical key operand.
    pub const fn handoff_id(&self) -> UefiOsHandoffId {
        self.arrival.handoff
    }

    pub const fn physical_invocation(&self) -> UefiPhysicalInvocationId {
        self.arrival.invocation
    }
}

/// Opaque result supplied by the target-specific provider adapter for one
/// exact acquired map. External code cannot construct either outcome; the
/// `exit_boot_services` provider edge is the sole issuance boundary and the
/// ledger consumes each result exactly once.
pub struct UefiExitBootServicesProviderResult {
    kind: UefiExitBootServicesProviderResultKind,
}

impl UefiExitBootServicesProviderResult {
    /// Sole stale-key issuance route. `pub(super)` keeps minting inside the
    /// UEFI bootstrap module family: only the executed provider edge may
    /// classify a firmware status into this result.
    pub(super) fn stale_map_key() -> Self {
        Self {
            kind: UefiExitBootServicesProviderResultKind::StaleMapKey,
        }
    }

    /// Sole success issuance route, binding the receipt the provider edge
    /// derived from its exact executed operands.
    pub(super) fn succeeded(receipt: UefiExitBootServicesReceiptId) -> Self {
        Self {
            kind: UefiExitBootServicesProviderResultKind::Succeeded { receipt },
        }
    }
}

enum UefiExitBootServicesProviderResultKind {
    StaleMapKey,
    Succeeded {
        receipt: UefiExitBootServicesReceiptId,
    },
}

/// Exact outcome of one state transition.
pub enum UefiOsHandoffProgress {
    Retry(UefiOsHandoffMapRequired),
    Exhausted(UefiOsHandoffExhausted),
    Complete(UefiOsHandoffComplete),
}

/// Recoverable exhaustion keeps Boot Services and every other live input.
#[must_use = "UEFI handoff exhaustion retains firmware-live retry custody"]
pub struct UefiOsHandoffExhausted {
    handoff: UefiOsHandoffId,
    session: UefiFirmwareSessionId,
    invocation: UefiPhysicalInvocationId,
    boot_services: UefiOsHandoffBootServicesId,
    allocations: UefiOsHandoffAllocationRosterId,
    surviving_stack: UefiOsHandoffStackEvidenceId,
    status: UefiErrorStatus,
}

impl UefiOsHandoffExhausted {
    pub const fn handoff_id(&self) -> UefiOsHandoffId {
        self.handoff
    }

    pub const fn firmware_session(&self) -> UefiFirmwareSessionId {
        self.session
    }

    pub const fn physical_invocation(&self) -> UefiPhysicalInvocationId {
        self.invocation
    }

    pub const fn boot_services(&self) -> UefiOsHandoffBootServicesId {
        self.boot_services
    }

    pub const fn allocation_roster(&self) -> UefiOsHandoffAllocationRosterId {
        self.allocations
    }

    pub const fn surviving_stack(&self) -> UefiOsHandoffStackEvidenceId {
        self.surviving_stack
    }

    pub const fn status(&self) -> UefiErrorStatus {
        self.status
    }
}

/// Non-returning success custody. Boot Services are intentionally absent;
/// allocation lineage, final snapshot, stack evidence, and provider receipt
/// survive under the exact handoff occurrence.
#[must_use = "successful UEFI OS handoff owns transferred program custody"]
pub struct UefiOsHandoffComplete {
    handoff: UefiOsHandoffId,
    session: UefiFirmwareSessionId,
    invocation: UefiPhysicalInvocationId,
    allocations: UefiOsHandoffAllocationRosterId,
    final_map: UefiMemoryMapSnapshotId,
    surviving_stack: UefiOsHandoffStackEvidenceId,
    receipt: UefiExitBootServicesReceiptId,
}

impl UefiOsHandoffComplete {
    pub const fn handoff_id(&self) -> UefiOsHandoffId {
        self.handoff
    }

    pub const fn firmware_session(&self) -> UefiFirmwareSessionId {
        self.session
    }

    pub const fn physical_invocation(&self) -> UefiPhysicalInvocationId {
        self.invocation
    }

    pub const fn allocation_roster(&self) -> UefiOsHandoffAllocationRosterId {
        self.allocations
    }

    pub const fn final_map(&self) -> UefiMemoryMapSnapshotId {
        self.final_map
    }

    pub const fn surviving_stack(&self) -> UefiOsHandoffStackEvidenceId {
        self.surviving_stack
    }

    pub const fn receipt(&self) -> UefiExitBootServicesReceiptId {
        self.receipt
    }
}

/// Recoverable map-acquisition rejection. Both the arrival and the unspent
/// acquisition evidence return intact.
pub struct UefiOsHandoffMapAcquisitionError {
    arrival: UefiOsHandoffMapRequired,
    acquisition: UefiMemoryMapAcquisition,
    diagnostic: ExternalRootDiagnostic,
}

impl std::fmt::Debug for UefiOsHandoffMapAcquisitionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("UefiOsHandoffMapAcquisitionError")
            .field("diagnostic", &self.diagnostic)
            .finish_non_exhaustive()
    }
}

impl UefiOsHandoffMapAcquisitionError {
    pub const fn diagnostic(&self) -> &ExternalRootDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(
        self,
    ) -> (
        UefiOsHandoffMapRequired,
        UefiMemoryMapAcquisition,
        ExternalRootDiagnostic,
    ) {
        (self.arrival, self.acquisition, self.diagnostic)
    }
}

/// Recoverable exit-transition rejection retaining both acquired custody and
/// the unconsumed provider result.
pub struct UefiOsHandoffTransitionError {
    acquired: UefiOsHandoffMapAcquired,
    result: UefiExitBootServicesProviderResult,
    diagnostic: ExternalRootDiagnostic,
}

impl std::fmt::Debug for UefiOsHandoffTransitionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("UefiOsHandoffTransitionError")
            .field("diagnostic", &self.diagnostic)
            .finish_non_exhaustive()
    }
}

impl UefiOsHandoffTransitionError {
    pub const fn diagnostic(&self) -> &ExternalRootDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(
        self,
    ) -> (
        UefiOsHandoffMapAcquired,
        UefiExitBootServicesProviderResult,
        ExternalRootDiagnostic,
    ) {
        (self.acquired, self.result, self.diagnostic)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ExternalRootDiagnostic, NonZeroU32, UefiErrorStatus, UefiExitBootServicesProviderResult,
        UefiExitBootServicesProviderResultKind, UefiExitBootServicesReceiptId,
        UefiFirmwareSessionId, UefiMemoryMapAcquisition, UefiMemoryMapKeyId,
        UefiMemoryMapSnapshotId, UefiOsHandoffAllocationRosterId, UefiOsHandoffBootServicesId,
        UefiOsHandoffId, UefiOsHandoffLedger, UefiOsHandoffMapAcquired, UefiOsHandoffMapRequired,
        UefiOsHandoffProgress, UefiOsHandoffStackEvidenceId, UefiPhysicalInvocationId,
    };

    fn id<T>(value: u64, make: fn(u64) -> Result<T, ExternalRootDiagnostic>) -> T {
        make(value).unwrap()
    }

    fn status() -> UefiErrorStatus {
        UefiErrorStatus::from_target_status(1_u64 << 63).unwrap()
    }

    fn stale_key() -> UefiExitBootServicesProviderResult {
        UefiExitBootServicesProviderResult {
            kind: UefiExitBootServicesProviderResultKind::StaleMapKey,
        }
    }

    fn succeeded(receipt: UefiExitBootServicesReceiptId) -> UefiExitBootServicesProviderResult {
        UefiExitBootServicesProviderResult {
            kind: UefiExitBootServicesProviderResultKind::Succeeded { receipt },
        }
    }

    fn ledger(base: u64, attempts: u32) -> (UefiOsHandoffLedger, UefiOsHandoffMapRequired) {
        UefiOsHandoffLedger::new(
            id(base, UefiOsHandoffId::from_normalized_identity),
            id(base + 1, UefiFirmwareSessionId::from_normalized_identity),
            id(base + 2, UefiPhysicalInvocationId::from_normalized_identity),
            id(
                base + 3,
                UefiOsHandoffBootServicesId::from_normalized_identity,
            ),
            id(
                base + 4,
                UefiOsHandoffAllocationRosterId::from_normalized_identity,
            ),
            id(
                base + 5,
                UefiOsHandoffStackEvidenceId::from_normalized_identity,
            ),
            NonZeroU32::new(attempts).unwrap(),
            status(),
        )
        .unwrap()
    }

    fn acquisition(ledger: &UefiOsHandoffLedger, base: u64) -> UefiMemoryMapAcquisition {
        UefiMemoryMapAcquisition::for_test(
            ledger.firmware_session(),
            ledger.physical_invocation(),
            id(base, UefiMemoryMapSnapshotId::from_normalized_identity),
            id(base + 1, UefiMemoryMapKeyId::from_normalized_identity),
            96,
            48,
            1,
        )
    }

    fn acquire(
        ledger: &mut UefiOsHandoffLedger,
        arrival: UefiOsHandoffMapRequired,
        base: u64,
    ) -> UefiOsHandoffMapAcquired {
        let acquisition = acquisition(ledger, base);
        ledger.acquire_memory_map(arrival, acquisition).unwrap()
    }

    #[test]
    fn successful_exit_consumes_boot_services_and_transfers_exact_lineage() {
        let (mut ledger, arrival) = ledger(10, 2);
        let allocations = arrival.allocation_roster();
        let stack = arrival.surviving_stack();
        let acquired = acquire(&mut ledger, arrival, 20);
        let snapshot = acquired.snapshot();
        let receipt = id(22, UefiExitBootServicesReceiptId::from_normalized_identity);
        let UefiOsHandoffProgress::Complete(complete) = ledger
            .apply_exit_boot_services_result(acquired, succeeded(receipt))
            .unwrap()
        else {
            panic!("successful provider result must complete the handoff")
        };
        assert_eq!(complete.allocation_roster(), allocations);
        assert_eq!(complete.final_map(), snapshot);
        assert_eq!(complete.surviving_stack(), stack);
        assert_eq!(complete.receipt(), receipt);
    }

    #[test]
    fn stale_key_returns_all_live_custody_with_a_strictly_smaller_measure() {
        let (mut ledger, arrival) = ledger(30, 2);
        let allocations = arrival.allocation_roster();
        let stack = arrival.surviving_stack();
        let acquired = acquire(&mut ledger, arrival, 40);
        let UefiOsHandoffProgress::Retry(retry) = ledger
            .apply_exit_boot_services_result(acquired, stale_key())
            .unwrap()
        else {
            panic!("the first stale key must retry")
        };
        assert_eq!(retry.remaining_attempts(), 1);
        assert_eq!(retry.allocation_roster(), allocations);
        assert_eq!(retry.surviving_stack(), stack);
        let reacquired = acquire(&mut ledger, retry, 42);
        assert_ne!(reacquired.snapshot().normalized_identity(), 40);
    }

    #[test]
    fn exhaustion_returns_authored_error_with_boot_services_live() {
        let (mut ledger, arrival) = ledger(50, 1);
        let boot_services = arrival.boot_services;
        let allocations = arrival.allocation_roster();
        let stack = arrival.surviving_stack();
        let acquired = acquire(&mut ledger, arrival, 60);
        let UefiOsHandoffProgress::Exhausted(exhausted) = ledger
            .apply_exit_boot_services_result(acquired, stale_key())
            .unwrap()
        else {
            panic!("the final stale key must exhaust")
        };
        assert_eq!(exhausted.boot_services(), boot_services);
        assert_eq!(exhausted.allocation_roster(), allocations);
        assert_eq!(exhausted.surviving_stack(), stack);
        assert_eq!(exhausted.status(), status());
    }

    #[test]
    fn stale_key_without_exact_decrement_rejects_and_returns_arrival() {
        let (mut ledger, arrival) = ledger(70, 2);
        let acquired = acquire(&mut ledger, arrival, 80);
        let UefiOsHandoffProgress::Retry(mut retry) = ledger
            .apply_exit_boot_services_result(acquired, stale_key())
            .unwrap()
        else {
            panic!("stale key must retry")
        };
        retry.remaining = 2;
        let error = ledger
            .acquire_memory_map(retry, acquisition(&ledger, 82))
            .unwrap_err();
        assert!(error.diagnostic().0.contains("stale"));
        let (mut retry, _acquisition, _) = error.into_parts();
        retry.remaining = 1;
        let _acquired = acquire(&mut ledger, retry, 82);
    }

    #[test]
    fn lost_or_cross_ledger_allocation_lineage_rejects_before_provider_use() {
        let (mut owner, mut arrival) = ledger(90, 2);
        let exact_allocations = arrival.allocations;
        arrival.allocations = id(
            999,
            UefiOsHandoffAllocationRosterId::from_normalized_identity,
        );
        let error = owner
            .acquire_memory_map(arrival, acquisition(&owner, 100))
            .unwrap_err();
        let (mut arrival, _acquisition, _) = error.into_parts();
        arrival.allocations = exact_allocations;
        let _acquired = acquire(&mut owner, arrival, 100);

        let (mut foreign, foreign_arrival) = ledger(90, 2);
        let error = foreign
            .acquire_memory_map(
                UefiOsHandoffMapRequired {
                    authority: owner.authority,
                    ..foreign_arrival
                },
                acquisition(&foreign, 102),
            )
            .unwrap_err();
        assert!(error.diagnostic().0.contains("foreign"));
    }

    #[test]
    fn completed_handoff_cannot_reenter_a_boot_services_provider() {
        let (mut ledger, arrival) = ledger(110, 1);
        let authority = arrival.authority;
        let boot_services = arrival.boot_services;
        let allocations = arrival.allocations;
        let surviving_stack = arrival.surviving_stack;
        let acquired = acquire(&mut ledger, arrival, 120);
        let _complete = ledger
            .apply_exit_boot_services_result(
                acquired,
                succeeded(id(
                    122,
                    UefiExitBootServicesReceiptId::from_normalized_identity,
                )),
            )
            .unwrap();
        let forged = UefiOsHandoffMapRequired {
            authority,
            handoff: ledger.handoff,
            session: ledger.session,
            invocation: ledger.invocation,
            boot_services,
            allocations,
            surviving_stack,
            remaining: 1,
            generation: ledger.generation,
        };
        let error = ledger
            .acquire_memory_map(forged, acquisition(&ledger, 123))
            .unwrap_err();
        assert!(error.diagnostic().0.contains("stale"));
    }

    #[test]
    fn status_and_retired_map_inputs_fail_closed() {
        assert!(UefiErrorStatus::from_target_status(0).is_err());
        let (mut ledger, arrival) = ledger(130, 2);
        let acquired = acquire(&mut ledger, arrival, 140);
        let UefiOsHandoffProgress::Retry(retry) = ledger
            .apply_exit_boot_services_result(acquired, stale_key())
            .unwrap()
        else {
            panic!("stale key must retry")
        };
        let error = ledger
            .acquire_memory_map(retry, acquisition(&ledger, 140))
            .unwrap_err();
        assert!(error.diagnostic().0.contains("retired"));
    }

    #[test]
    fn provider_outcomes_and_linear_custody_have_no_public_fabrication_surface() {
        let production = include_str!("os_handoff.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("production OS-handoff source");
        let compact = production
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect::<String>();
        assert!(compact.contains(
            "pubstructUefiExitBootServicesProviderResult{kind:UefiExitBootServicesProviderResultKind,}"
        ));
        for forbidden in [
            "pubfnstale_map_key",
            "pubfnsucceeded",
            "implCloneforUefiOsHandoff",
            "psi_extents::Extent",
        ] {
            assert!(
                !compact.contains(forbidden),
                "forbidden handoff authority surface appeared: {forbidden}"
            );
        }
    }
}

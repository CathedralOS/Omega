//! UEFI exit-boot-services tests.

use super::{
    LifecycleScopedUefiExitBootServicesProvider, UefiExitBootServicesAttemptOutcome,
    admit_uefi_exit_boot_services_execution, bind_uefi_exit_boot_services_invocation,
    execute_uefi_exit_boot_services, join_lifecycle_scoped_uefi_exit_boot_services_provider,
    prepare_uefi_exit_boot_services_invocation,
};
use crate::ExternalRootDiagnostic;
use crate::platform_bringup::uefi_bootstrap::{
    LifecycleScopedUefiBootServicesProjection, UefiApplicationFirmwareLedger,
    UefiOsHandoffMapAcquired,
};
use crate::{
    UefiApplicationBootstrapLedgerId, UefiBootServicesPhaseLeaseId, UefiErrorStatus,
    UefiFirmwareSessionId, UefiMemoryMapAcquisition, UefiMemoryMapSnapshotId,
    UefiOsHandoffAllocationRosterId, UefiOsHandoffBootServicesId, UefiOsHandoffId,
    UefiOsHandoffLedger, UefiOsHandoffMapRequired, UefiOsHandoffProgress,
    UefiOsHandoffStackEvidenceId, UefiSystemTableOccurrenceId,
    join_lifecycle_scoped_uefi_system_table, join_uefi_application_physical_arrival,
    prepare_uefi_application_bootstrap_adapter_invocation, project_uefi_application_boot_services,
};
use crate::{
    UefiBootServicesTableOccurrenceId, UefiImageHandleOccurrenceId, UefiMemoryMapKeyId,
    UefiPhysicalInvocationId,
};
use program_entry_plan::{
    ProgramEntryPhysicalContractPlan, UEFI_X64_IMAGE_HANDLE_TYPE_IDENTITY,
    UEFI_X64_PHYSICAL_REQUIREMENT_IDENTITY, UEFI_X64_STATUS_TYPE_IDENTITY,
    UEFI_X64_SYSTEM_TABLE_REFERENCE_TYPE_IDENTITY, exact_uefi_x64_physical_boundary_entry_plan,
    exact_uefi_x64_physical_contract_package_source_digest,
};
use std::ffi::c_void;
use std::num::{NonZeroU32, NonZeroU64};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use target::{
    ProgramEntryPhysicalContractPackage, UEFI_BOOT_SERVICES_SIGNATURE, UEFI_SYSTEM_TABLE_SIGNATURE,
    UefiBootServicesNativeField, plan_uefi_system_table_native_layout,
    validate_uefi_boot_services_occurrence, validate_uefi_system_table_occurrence,
};
use target::{TargetProfile, plan_uefi_boot_services_native_layout};

// The fabricated firmware answers with raw `EFI_STATUS` codes; the edge under
// test classifies them only through the planned leg's status table.
const EFI_STATUS_ERROR_BIT: u64 = 1_u64 << 63;
const EFI_SUCCESS: u64 = 0;
const EFI_INVALID_PARAMETER: u64 = EFI_STATUS_ERROR_BIT | 2;

static FIRMWARE_TEST_LOCK: Mutex<()> = Mutex::new(());
static FAKE_STATUS: AtomicU64 = AtomicU64::new(0);
static OBSERVED_HANDLE: AtomicUsize = AtomicUsize::new(0);
static OBSERVED_MAP_KEY: AtomicUsize = AtomicUsize::new(0);
static OBSERVED_CALLS: AtomicUsize = AtomicUsize::new(0);

unsafe extern "efiapi" fn fake_exit_boot_services(
    image_handle: *mut c_void,
    map_key: usize,
) -> u64 {
    OBSERVED_CALLS.fetch_add(1, Ordering::SeqCst);
    OBSERVED_HANDLE.store(image_handle as usize, Ordering::SeqCst);
    OBSERVED_MAP_KEY.store(map_key, Ordering::SeqCst);
    FAKE_STATUS.load(Ordering::SeqCst)
}

fn fake_exit_boot_services_address() -> u64 {
    fake_exit_boot_services as *const () as usize as u64
}

fn id<T>(value: u64, constructor: impl FnOnce(u64) -> Result<T, ExternalRootDiagnostic>) -> T {
    constructor(value).unwrap()
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for (index, byte) in bytes.iter().copied().enumerate() {
        let byte = if (16..20).contains(&index) { 0 } else { byte };
        crc ^= u32::from(byte);
        for _ in 0..8 {
            let mask = 0_u32.wrapping_sub(crc & 1);
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    !crc
}

fn table(signature: u64, size: usize, pointer_offset: usize, pointer: u64) -> Vec<u8> {
    let mut bytes = vec![0; size];
    bytes[0..8].copy_from_slice(&signature.to_le_bytes());
    bytes[8..12].copy_from_slice(&((2_u32 << 16) | 100).to_le_bytes());
    bytes[12..16].copy_from_slice(&(size as u32).to_le_bytes());
    bytes[pointer_offset..pointer_offset + 8].copy_from_slice(&pointer.to_le_bytes());
    let crc = crc32(&bytes);
    bytes[16..20].copy_from_slice(&crc.to_le_bytes());
    bytes
}

/// Fabricated Boot Services table carrying the `ExitBootServices` row at its
/// exact target-layout offset.
fn boot_table(service_address: u64) -> Vec<u8> {
    let exit_offset = plan_uefi_boot_services_native_layout(TargetProfile::UefiX64)
        .unwrap()
        .field_layout(UefiBootServicesNativeField::ExitBootServices)
        .unwrap()
        .byte_offset() as usize;
    table(
        UEFI_BOOT_SERVICES_SIGNATURE,
        376,
        exit_offset,
        service_address,
    )
}

fn physical_contract() -> ProgramEntryPhysicalContractPlan {
    let expected = exact_uefi_x64_physical_boundary_entry_plan();
    ProgramEntryPhysicalContractPlan::new(
        TargetProfile::UefiX64.program_entry_slot(),
        UEFI_X64_PHYSICAL_REQUIREMENT_IDENTITY.into(),
        ProgramEntryPhysicalContractPackage::UefiX64,
        exact_uefi_x64_physical_contract_package_source_digest(),
        1,
        vec![
            UEFI_X64_IMAGE_HANDLE_TYPE_IDENTITY.into(),
            UEFI_X64_SYSTEM_TABLE_REFERENCE_TYPE_IDENTITY.into(),
        ],
        UEFI_X64_STATUS_TYPE_IDENTITY.into(),
        expected.contract_report_fingerprint(),
        expected.plan().clone(),
    )
    .unwrap()
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

/// The firmware ledger minted by `ledger(base)` names `base + 2` as its
/// physical invocation; every helper here shares that convention so the
/// handoff ledger and provider edge join the same invocation.
const INVOCATION_OFFSET: u64 = 2;

fn projection<'a>(
    ledger: &mut UefiApplicationFirmwareLedger<'a>,
    bytes: &'a [u8],
    base: u64,
) -> LifecycleScopedUefiBootServicesProjection<'a> {
    projection_with_image_handle(ledger, bytes, base, true)
}

fn projection_with_image_handle<'a>(
    ledger: &mut UefiApplicationFirmwareLedger<'a>,
    bytes: &'a [u8],
    base: u64,
    retain_physical_value: bool,
) -> LifecycleScopedUefiBootServicesProjection<'a> {
    let occurrence = id(base, UefiImageHandleOccurrenceId::from_normalized_identity);
    let image = if retain_physical_value {
        ledger.admit_image_handle_physical_input(
            occurrence,
            NonZeroU64::new(0x1000_0000 + base).unwrap(),
        )
    } else {
        ledger.admit_image_handle_occurrence(occurrence)
    }
    .unwrap();
    let integrity = validate_uefi_system_table_occurrence(
        plan_uefi_system_table_native_layout(TargetProfile::UefiX64).unwrap(),
        bytes,
    )
    .unwrap();
    let provenance = ledger
        .admit_system_table_occurrence(
            id(
                base + 1,
                UefiSystemTableOccurrenceId::from_normalized_identity,
            ),
            integrity.table_bytes(),
        )
        .unwrap();
    let lease = ledger
        .acquire_boot_services_phase_lease(id(
            base + 2,
            UefiBootServicesPhaseLeaseId::from_normalized_identity,
        ))
        .unwrap();
    let scoped =
        join_lifecycle_scoped_uefi_system_table(ledger, integrity, provenance, lease).unwrap();
    let arrival =
        join_uefi_application_physical_arrival(ledger, image, scoped, physical_contract()).unwrap();
    let readiness = prepare_uefi_application_bootstrap_adapter_invocation(ledger, arrival).unwrap();
    project_uefi_application_boot_services(ledger, readiness).unwrap()
}

fn provider<'system_table, 'boot_services>(
    ledger: &mut UefiApplicationFirmwareLedger<'system_table>,
    system: &'system_table [u8],
    boot: &'boot_services [u8],
    base: u64,
    boot_address: u64,
) -> LifecycleScopedUefiExitBootServicesProvider<'system_table, 'boot_services> {
    let projection = projection(ledger, system, base);
    let integrity = validate_uefi_boot_services_occurrence(
        plan_uefi_boot_services_native_layout(TargetProfile::UefiX64).unwrap(),
        boot,
    )
    .unwrap();
    join_lifecycle_scoped_uefi_exit_boot_services_provider(
        ledger,
        projection,
        integrity,
        id(
            base + 3,
            UefiBootServicesTableOccurrenceId::from_normalized_identity,
        ),
        NonZeroU64::new(boot_address).unwrap(),
    )
    .unwrap()
}

fn handoff(
    base: u64,
    invocation: u64,
    attempts: u32,
) -> (UefiOsHandoffLedger, UefiOsHandoffMapRequired) {
    UefiOsHandoffLedger::new(
        id(base, UefiOsHandoffId::from_normalized_identity),
        id(base + 1, UefiFirmwareSessionId::from_normalized_identity),
        id(
            invocation,
            UefiPhysicalInvocationId::from_normalized_identity,
        ),
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
        UefiErrorStatus::from_target_status(EFI_STATUS_ERROR_BIT).unwrap(),
    )
    .unwrap()
}

fn acquire(
    ledger: &mut UefiOsHandoffLedger,
    arrival: UefiOsHandoffMapRequired,
    base: u64,
) -> UefiOsHandoffMapAcquired {
    let acquisition = UefiMemoryMapAcquisition::for_test(
        ledger.firmware_session(),
        ledger.physical_invocation(),
        id(base, UefiMemoryMapSnapshotId::from_normalized_identity),
        id(base + 1, UefiMemoryMapKeyId::from_normalized_identity),
        96,
        48,
        1,
    );
    ledger.acquire_memory_map(arrival, acquisition).unwrap()
}

#[test]
fn successful_attempt_consumes_boot_services_and_completes_the_handoff() {
    let _firmware = FIRMWARE_TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    FAKE_STATUS.store(EFI_SUCCESS, Ordering::SeqCst);
    OBSERVED_CALLS.store(0, Ordering::SeqCst);
    let boot_address = 0x401000;
    let system = table(UEFI_SYSTEM_TABLE_SIGNATURE, 120, 96, boot_address);
    let boot = boot_table(fake_exit_boot_services_address());
    let mut firmware = ledger(10);
    let provider = provider(&mut firmware, &system, &boot, 13, boot_address);
    let invocation = prepare_uefi_exit_boot_services_invocation(provider).unwrap();
    let (mut handoff, arrival) = handoff(50, 10 + INVOCATION_OFFSET, 2);
    let allocations = arrival.allocation_roster();
    let stack = arrival.surviving_stack();
    let acquired = acquire(&mut handoff, arrival, 40);
    let bound = bind_uefi_exit_boot_services_invocation(invocation, &acquired).unwrap();
    assert_eq!(bound.map_key().normalized_identity(), 41);
    // SAFETY: the bound service address is the test-local efiapi fake baked
    // into the fabricated Boot Services table above.
    let executed = unsafe { execute_uefi_exit_boot_services(bound) }.unwrap();
    assert_eq!(OBSERVED_HANDLE.load(Ordering::SeqCst), 0x1000_0000 + 13);
    assert_eq!(OBSERVED_MAP_KEY.load(Ordering::SeqCst), 41);
    let UefiExitBootServicesAttemptOutcome::Exited {
        status_code,
        result,
    } = admit_uefi_exit_boot_services_execution(executed).unwrap()
    else {
        panic!("EFI_SUCCESS must exit the handoff attempt")
    };
    assert_eq!(status_code, EFI_SUCCESS);
    let UefiOsHandoffProgress::Complete(complete) = handoff
        .apply_exit_boot_services_result(acquired, result)
        .unwrap()
    else {
        panic!("successful provider result must complete the handoff")
    };
    assert_eq!(complete.allocation_roster(), allocations);
    assert_eq!(complete.surviving_stack(), stack);
    assert_eq!(complete.final_map().normalized_identity(), 40);
    assert_ne!(complete.receipt().normalized_identity(), 0);
    assert_eq!(OBSERVED_CALLS.load(Ordering::SeqCst), 1);
}

#[test]
fn stale_key_returns_complete_provider_custody_and_retries_to_success() {
    let _firmware = FIRMWARE_TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    FAKE_STATUS.store(EFI_INVALID_PARAMETER, Ordering::SeqCst);
    OBSERVED_CALLS.store(0, Ordering::SeqCst);
    let boot_address = 0x401000;
    let system = table(UEFI_SYSTEM_TABLE_SIGNATURE, 120, 96, boot_address);
    let boot = boot_table(fake_exit_boot_services_address());
    let mut firmware = ledger(10);
    let provider = provider(&mut firmware, &system, &boot, 13, boot_address);
    let invocation = prepare_uefi_exit_boot_services_invocation(provider).unwrap();
    let (mut handoff, arrival) = handoff(50, 10 + INVOCATION_OFFSET, 2);
    let acquired = acquire(&mut handoff, arrival, 40);
    let bound = bind_uefi_exit_boot_services_invocation(invocation, &acquired).unwrap();
    // SAFETY: the retained service address is the test-local fake.
    let executed = unsafe { execute_uefi_exit_boot_services(bound) }.unwrap();
    assert_eq!(OBSERVED_MAP_KEY.load(Ordering::SeqCst), 41);
    let UefiExitBootServicesAttemptOutcome::Retry {
        invocation,
        status_code,
        result,
    } = admit_uefi_exit_boot_services_execution(executed).unwrap()
    else {
        panic!("EFI_INVALID_PARAMETER must return provider custody for retry")
    };
    assert_eq!(status_code, EFI_INVALID_PARAMETER);
    let UefiOsHandoffProgress::Retry(retry) = handoff
        .apply_exit_boot_services_result(acquired, result)
        .unwrap()
    else {
        panic!("the first stale key must retry")
    };
    assert_eq!(retry.remaining_attempts(), 1);

    // The returned custody binds the fresh acquired map and drives the
    // successful second attempt through the same provider edge.
    let acquired = acquire(&mut handoff, retry, 42);
    let bound = bind_uefi_exit_boot_services_invocation(invocation, &acquired).unwrap();
    FAKE_STATUS.store(EFI_SUCCESS, Ordering::SeqCst);
    // SAFETY: same retained fake service address.
    let executed = unsafe { execute_uefi_exit_boot_services(bound) }.unwrap();
    assert_eq!(OBSERVED_MAP_KEY.load(Ordering::SeqCst), 43);
    let UefiExitBootServicesAttemptOutcome::Exited { result, .. } =
        admit_uefi_exit_boot_services_execution(executed).unwrap()
    else {
        panic!("the retried attempt must exit on success")
    };
    let UefiOsHandoffProgress::Complete(complete) = handoff
        .apply_exit_boot_services_result(acquired, result)
        .unwrap()
    else {
        panic!("the retried success must complete the handoff")
    };
    assert_eq!(complete.final_map().normalized_identity(), 42);
    assert_eq!(OBSERVED_CALLS.load(Ordering::SeqCst), 2);
}

#[test]
fn exhausted_attempt_keeps_provider_custody_releasable_for_firmware_return() {
    let _firmware = FIRMWARE_TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    FAKE_STATUS.store(EFI_INVALID_PARAMETER, Ordering::SeqCst);
    let boot_address = 0x401000;
    let system = table(UEFI_SYSTEM_TABLE_SIGNATURE, 120, 96, boot_address);
    let boot = boot_table(fake_exit_boot_services_address());
    let mut firmware = ledger(10);
    let provider = provider(&mut firmware, &system, &boot, 13, boot_address);
    let invocation = prepare_uefi_exit_boot_services_invocation(provider).unwrap();
    let (mut handoff, arrival) = handoff(50, 10 + INVOCATION_OFFSET, 1);
    let acquired = acquire(&mut handoff, arrival, 40);
    let bound = bind_uefi_exit_boot_services_invocation(invocation, &acquired).unwrap();
    // SAFETY: the retained service address is the test-local fake.
    let executed = unsafe { execute_uefi_exit_boot_services(bound) }.unwrap();
    let UefiExitBootServicesAttemptOutcome::Retry {
        invocation, result, ..
    } = admit_uefi_exit_boot_services_execution(executed).unwrap()
    else {
        panic!("stale key must return provider custody")
    };
    let UefiOsHandoffProgress::Exhausted(exhausted) = handoff
        .apply_exit_boot_services_result(acquired, result)
        .unwrap()
    else {
        panic!("the final stale key must exhaust")
    };
    assert_eq!(exhausted.boot_services().normalized_identity(), 53);
    assert_eq!(exhausted.status().value(), EFI_STATUS_ERROR_BIT);

    // Exhaustion leaves Boot Services live: the returned provider releases
    // its scope and the invocation can still begin its firmware return.
    let released = firmware
        .release_planned_uefi_exit_boot_services_invocation(invocation)
        .unwrap();
    assert_eq!(released.ledger, firmware.ledger_id());
    firmware.begin_firmware_return().unwrap();
}

#[test]
fn foreign_ledger_and_correspondence_drift_reject_before_any_binding() {
    let _firmware = FIRMWARE_TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let boot_address = 0x401000;
    let system = table(UEFI_SYSTEM_TABLE_SIGNATURE, 120, 96, boot_address);
    let boot = boot_table(fake_exit_boot_services_address());
    let mut owner = ledger(10);
    let foreign = ledger(60);
    let owner_projection = projection(&mut owner, &system, 13);
    let integrity = validate_uefi_boot_services_occurrence(
        plan_uefi_boot_services_native_layout(TargetProfile::UefiX64).unwrap(),
        &boot,
    )
    .unwrap();
    let error = join_lifecycle_scoped_uefi_exit_boot_services_provider(
        &foreign,
        owner_projection,
        integrity,
        id(
            16,
            UefiBootServicesTableOccurrenceId::from_normalized_identity,
        ),
        NonZeroU64::new(boot_address).unwrap(),
    )
    .unwrap_err();
    assert!(error.diagnostic().0.contains("different or inactive"));

    // A Boot Services occurrence address that does not correspond to the
    // projected System Table field rejects with custody returned. One
    // ledger admits exactly one image-handle occurrence, so the returned
    // projection threads through each rejection.
    let (returned_projection, integrity, occurrence, _, _) = error.into_parts();
    let error = join_lifecycle_scoped_uefi_exit_boot_services_provider(
        &owner,
        returned_projection,
        integrity,
        occurrence,
        NonZeroU64::new(boot_address + 8).unwrap(),
    )
    .unwrap_err();
    assert!(error.diagnostic().0.contains("does not correspond"));

    // A null service pointer cannot leave the Boot-Services-live phase.
    let null_boot = boot_table(0);
    let (returned_projection, _, occurrence, _, _) = error.into_parts();
    let integrity = validate_uefi_boot_services_occurrence(
        plan_uefi_boot_services_native_layout(TargetProfile::UefiX64).unwrap(),
        &null_boot,
    )
    .unwrap();
    let error = join_lifecycle_scoped_uefi_exit_boot_services_provider(
        &owner,
        returned_projection,
        integrity,
        occurrence,
        NonZeroU64::new(boot_address).unwrap(),
    )
    .unwrap_err();
    assert!(error.diagnostic().0.contains("is null"));
    let (returned_projection, _, _, _, _) = error.into_parts();
    let released = owner
        .release_lifecycle_scoped_boot_services_projection(returned_projection)
        .unwrap();
    assert_eq!(released.ledger, owner.ledger_id());
    owner.begin_firmware_return().unwrap();
}

#[test]
fn acquired_map_from_a_different_invocation_and_address_free_handle_reject() {
    let _firmware = FIRMWARE_TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let boot_address = 0x401000;
    let system = table(UEFI_SYSTEM_TABLE_SIGNATURE, 120, 96, boot_address);
    let boot = boot_table(fake_exit_boot_services_address());
    let mut firmware = ledger(10);
    let provider = provider(&mut firmware, &system, &boot, 13, boot_address);
    let invocation = prepare_uefi_exit_boot_services_invocation(provider).unwrap();

    // The acquired carrier under a foreign physical invocation cannot bind
    // this provider's operands; the planned custody returns unchanged.
    let (mut foreign_handoff, foreign_arrival) = handoff(70, 999, 1);
    let foreign_acquired = acquire(&mut foreign_handoff, foreign_arrival, 80);
    let error = bind_uefi_exit_boot_services_invocation(invocation, &foreign_acquired).unwrap_err();
    assert!(
        error
            .diagnostic()
            .0
            .contains("different physical invocation")
    );
    let (invocation, _) = error.into_parts();

    // A map acquired under the provider's invocation binds cleanly.
    let (mut owner_handoff, arrival) = handoff(50, 10 + INVOCATION_OFFSET, 1);
    let acquired = acquire(&mut owner_handoff, arrival, 90);
    let bound = bind_uefi_exit_boot_services_invocation(invocation, &acquired).unwrap();
    let released = firmware
        .release_bound_uefi_exit_boot_services_invocation(bound)
        .unwrap();
    assert_eq!(released.ledger, firmware.ledger_id());

    // Address-free image-handle provenance cannot cross the operand edge.
    // A firmware ledger admits one image-handle occurrence, so this case
    // needs its own invocation ledger.
    let mut address_free_firmware = ledger(60);
    let projection = projection_with_image_handle(&mut address_free_firmware, &system, 43, false);
    let integrity = validate_uefi_boot_services_occurrence(
        plan_uefi_boot_services_native_layout(TargetProfile::UefiX64).unwrap(),
        &boot,
    )
    .unwrap();
    let provider = join_lifecycle_scoped_uefi_exit_boot_services_provider(
        &address_free_firmware,
        projection,
        integrity,
        id(
            46,
            UefiBootServicesTableOccurrenceId::from_normalized_identity,
        ),
        NonZeroU64::new(boot_address).unwrap(),
    )
    .unwrap();
    let invocation = prepare_uefi_exit_boot_services_invocation(provider).unwrap();
    let (mut address_free_handoff, address_free_arrival) = handoff(90, 60 + INVOCATION_OFFSET, 1);
    let address_free_acquired = acquire(&mut address_free_handoff, address_free_arrival, 95);
    let error =
        bind_uefi_exit_boot_services_invocation(invocation, &address_free_acquired).unwrap_err();
    assert!(error.diagnostic().0.contains("image-handle value"));
}

#[test]
fn unknown_status_retains_executed_custody_for_release() {
    let _firmware = FIRMWARE_TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    FAKE_STATUS.store(EFI_STATUS_ERROR_BIT | 9, Ordering::SeqCst);
    let boot_address = 0x401000;
    let system = table(UEFI_SYSTEM_TABLE_SIGNATURE, 120, 96, boot_address);
    let boot = boot_table(fake_exit_boot_services_address());
    let mut firmware = ledger(10);
    let provider = provider(&mut firmware, &system, &boot, 13, boot_address);
    let invocation = prepare_uefi_exit_boot_services_invocation(provider).unwrap();
    let (mut handoff, arrival) = handoff(50, 10 + INVOCATION_OFFSET, 1);
    let acquired = acquire(&mut handoff, arrival, 40);
    let bound = bind_uefi_exit_boot_services_invocation(invocation, &acquired).unwrap();
    // SAFETY: the retained service address is the test-local fake.
    let executed = unsafe { execute_uefi_exit_boot_services(bound) }.unwrap();
    let error = admit_uefi_exit_boot_services_execution(executed).unwrap_err();
    assert!(error.diagnostic().0.contains("closed target table"));
    let (execution, _) = error.into_parts();
    let released = firmware
        .release_executed_uefi_exit_boot_services_invocation(execution)
        .unwrap();
    assert_eq!(released.ledger, firmware.ledger_id());
    firmware.begin_firmware_return().unwrap();
}

#[test]
fn success_leaves_no_provider_for_post_exit_use() {
    let _firmware = FIRMWARE_TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    FAKE_STATUS.store(EFI_SUCCESS, Ordering::SeqCst);
    let boot_address = 0x401000;
    let system = table(UEFI_SYSTEM_TABLE_SIGNATURE, 120, 96, boot_address);
    let boot = boot_table(fake_exit_boot_services_address());
    let mut firmware = ledger(10);
    let provider = provider(&mut firmware, &system, &boot, 13, boot_address);
    let invocation = prepare_uefi_exit_boot_services_invocation(provider).unwrap();
    let (mut handoff, arrival) = handoff(50, 10 + INVOCATION_OFFSET, 2);
    let acquired = acquire(&mut handoff, arrival, 40);
    let bound = bind_uefi_exit_boot_services_invocation(invocation, &acquired).unwrap();
    // SAFETY: the retained service address is the test-local fake.
    let executed = unsafe { execute_uefi_exit_boot_services(bound) }.unwrap();
    let UefiExitBootServicesAttemptOutcome::Exited { result, .. } =
        admit_uefi_exit_boot_services_execution(executed).unwrap()
    else {
        panic!("EFI_SUCCESS must exit the handoff attempt")
    };
    // The Exited variant carries no provider carrier: after consumption no
    // binding, execution, or release edge remains expressible for this
    // invocation. The ledger-side forged-arrival rejection is witnessed in
    // os_handoff.rs's completed_handoff_cannot_reenter test.
    let UefiOsHandoffProgress::Complete(_) = handoff
        .apply_exit_boot_services_result(acquired, result)
        .unwrap()
    else {
        panic!("success must complete the handoff")
    };
}

#[test]
fn provider_result_issuance_stays_module_family_private() {
    for source in [
        include_str!("../os_handoff.rs"),
        include_str!("../exit_boot_services.rs"),
        include_str!("provider_lifecycle.rs"),
        include_str!("invocation_planning.rs"),
        include_str!("execution.rs"),
        include_str!("../get_memory_map.rs"),
        include_str!("../get_memory_map/memory_map_buffer.rs"),
        include_str!("../get_memory_map/provider_lifecycle.rs"),
        include_str!("../get_memory_map/invocation_planning.rs"),
        include_str!("../get_memory_map/execution.rs"),
    ] {
        let production = source
            .split("#[cfg(test)]")
            .next()
            .expect("production source");
        let compact = production
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect::<String>();
        for forbidden in [
            "pubfnstale_map_key",
            "pubfnsucceeded",
            "pub(crate)fnstale_map_key",
            "pub(crate)fnsucceeded",
            "pubfnfor_test",
            "pub(crate)fnfor_test",
            "implCloneforUefiOsHandoff",
            "implCloneforUefiExitBootServices",
            "implCloneforUefiMemoryMap",
            "implCloneforUefiGetMemoryMap",
        ] {
            assert!(
                !compact.contains(forbidden),
                "forbidden handoff authority surface appeared: {forbidden}"
            );
        }
    }
    let provider_source = [
        include_str!("../exit_boot_services.rs"),
        include_str!("provider_lifecycle.rs"),
        include_str!("invocation_planning.rs"),
        include_str!("execution.rs"),
    ]
    .concat();
    let provider_source = provider_source.as_str();
    assert!(
        !provider_source.contains("UefiExitBootServicesProviderResultKind"),
        "the provider edge must mint results only through the ledger's private constructors"
    );
}

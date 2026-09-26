//! UEFI get-memory-map tests.

use super::{
    UefiGetMemoryMapAttemptOutcome, UefiMemoryMapAcquisition, UefiMemoryMapBuffer,
    admit_uefi_get_memory_map_execution, bind_uefi_get_memory_map_invocation,
    execute_uefi_get_memory_map, join_lifecycle_scoped_uefi_get_memory_map_provider,
    prepare_uefi_get_memory_map_invocation,
};
use crate::ExternalRootDiagnostic;
use crate::platform_bringup::uefi_bootstrap::{
    PlannedUefiExitBootServicesInvocation, UefiApplicationFirmwareLedger,
};
use crate::{
    LifecycleScopedUefiBootServicesProjection, UefiApplicationBootstrapLedgerId,
    UefiBootServicesPhaseLeaseId, UefiErrorStatus, UefiExitBootServicesAttemptOutcome,
    UefiOsHandoffAllocationRosterId, UefiOsHandoffBootServicesId, UefiOsHandoffId,
    UefiOsHandoffLedger, UefiOsHandoffMapRequired, UefiOsHandoffProgress,
    UefiOsHandoffStackEvidenceId, UefiSystemTableOccurrenceId,
    admit_uefi_exit_boot_services_execution, bind_uefi_exit_boot_services_invocation,
    execute_uefi_exit_boot_services, join_lifecycle_scoped_uefi_exit_boot_services_provider,
    join_lifecycle_scoped_uefi_system_table, join_uefi_application_physical_arrival,
    prepare_uefi_application_bootstrap_adapter_invocation,
    prepare_uefi_exit_boot_services_invocation, project_uefi_application_boot_services,
};
use crate::{
    UefiBootServicesTableOccurrenceId, UefiFirmwareSessionId, UefiImageHandleOccurrenceId,
    UefiMemoryMapKeyId, UefiMemoryMapSnapshotId, UefiPhysicalInvocationId,
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
    plan_uefi_boot_services_native_layout, plan_uefi_system_table_native_layout,
    validate_uefi_boot_services_occurrence, validate_uefi_system_table_occurrence,
};
use target::{TargetProfile, UefiBootServicesNativeField};

// The fabricated firmware answers with raw `EFI_STATUS` codes; the edge under
// test classifies them only through the planned leg's status table.
const EFI_STATUS_ERROR_BIT: u64 = 1_u64 << 63;
const EFI_SUCCESS: u64 = 0;
const EFI_INVALID_PARAMETER: u64 = EFI_STATUS_ERROR_BIT | 2;
const EFI_BUFFER_TOO_SMALL: u64 = EFI_STATUS_ERROR_BIT | 5;

static FIRMWARE_TEST_LOCK: Mutex<()> = Mutex::new(());
static FAKE_GET_STATUS: AtomicU64 = AtomicU64::new(0);
static FAKE_EXIT_STATUS: AtomicU64 = AtomicU64::new(0);
static FAKE_REQUIRED_BYTES: AtomicUsize = AtomicUsize::new(0);
static FAKE_MAP_BYTES: AtomicUsize = AtomicUsize::new(0);
static FAKE_MAP_KEY: AtomicUsize = AtomicUsize::new(0);
static FAKE_DESCRIPTOR_SIZE: AtomicUsize = AtomicUsize::new(0);
static FAKE_DESCRIPTOR_VERSION: AtomicU64 = AtomicU64::new(0);
static OBSERVED_GET_CALLS: AtomicUsize = AtomicUsize::new(0);
static OBSERVED_CAPACITY: AtomicUsize = AtomicUsize::new(0);
static OBSERVED_MAP_POINTER: AtomicUsize = AtomicUsize::new(0);
static OBSERVED_EXIT_KEY: AtomicUsize = AtomicUsize::new(0);

/// Firmware-shaped fake: reads the in-out size cell, writes the scripted
/// required extent on BufferTooSmall and the map bytes plus all output
/// cells on success.
unsafe extern "efiapi" fn fake_get_memory_map(
    memory_map_size: *mut usize,
    memory_map: *mut c_void,
    map_key: *mut usize,
    descriptor_size: *mut usize,
    descriptor_version: *mut u32,
) -> u64 {
    OBSERVED_GET_CALLS.fetch_add(1, Ordering::SeqCst);
    // SAFETY: the call cells belong to the bound test buffer for exactly
    // this call, matching the edge's safety contract.
    let capacity = unsafe { *memory_map_size };
    OBSERVED_CAPACITY.store(capacity, Ordering::SeqCst);
    OBSERVED_MAP_POINTER.store(memory_map as usize, Ordering::SeqCst);
    let status = FAKE_GET_STATUS.load(Ordering::SeqCst);
    if status == EFI_SUCCESS {
        let bytes = FAKE_MAP_BYTES.load(Ordering::SeqCst);
        // A misreporting "firmware" may claim an extent beyond capacity;
        // only capacity-bounded bytes are ever written so admission can
        // witness the over-capacity report without scribbling.
        let writable = bytes.min(capacity);
        if !memory_map.is_null() && writable != 0 {
            // SAFETY: the edge guarantees `capacity` writable bytes here.
            unsafe { std::ptr::write_bytes(memory_map.cast::<u8>(), 0x5a, writable) };
        }
        // SAFETY: same exclusive output-cell custody as above.
        unsafe {
            *memory_map_size = bytes;
            *map_key = FAKE_MAP_KEY.load(Ordering::SeqCst);
            *descriptor_size = FAKE_DESCRIPTOR_SIZE.load(Ordering::SeqCst);
            *descriptor_version = FAKE_DESCRIPTOR_VERSION.load(Ordering::SeqCst) as u32;
        }
    } else if status == EFI_BUFFER_TOO_SMALL {
        // SAFETY: same exclusive output-cell custody as above.
        unsafe { *memory_map_size = FAKE_REQUIRED_BYTES.load(Ordering::SeqCst) };
    }
    status
}

unsafe extern "efiapi" fn fake_exit_boot_services(
    _image_handle: *mut c_void,
    map_key: usize,
) -> u64 {
    OBSERVED_EXIT_KEY.store(map_key, Ordering::SeqCst);
    FAKE_EXIT_STATUS.load(Ordering::SeqCst)
}

fn fake_get_memory_map_address() -> u64 {
    fake_get_memory_map as *const () as usize as u64
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

/// Fabricated Boot Services table carrying both handoff-edge rows:
/// GetMemoryMap at its exact offset and ExitBootServices at its own.
fn boot_table(get_memory_map: u64, exit_boot_services: u64) -> Vec<u8> {
    let layout = plan_uefi_boot_services_native_layout(TargetProfile::UefiX64).unwrap();
    let get_offset = layout
        .field_layout(UefiBootServicesNativeField::GetMemoryMap)
        .unwrap()
        .byte_offset() as usize;
    let exit_offset = layout
        .field_layout(UefiBootServicesNativeField::ExitBootServices)
        .unwrap()
        .byte_offset() as usize;
    let mut bytes = table(
        UEFI_BOOT_SERVICES_SIGNATURE,
        376,
        get_offset,
        get_memory_map,
    );
    bytes[exit_offset..exit_offset + 8].copy_from_slice(&exit_boot_services.to_le_bytes());
    let crc = crc32(&bytes);
    bytes[16..20].copy_from_slice(&crc.to_le_bytes());
    bytes
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
/// handoff ledger and provider edges join the same invocation.
const INVOCATION_OFFSET: u64 = 2;

fn projection<'a>(
    ledger: &mut UefiApplicationFirmwareLedger<'a>,
    bytes: &'a [u8],
    base: u64,
) -> LifecycleScopedUefiBootServicesProjection<'a> {
    let occurrence = id(base, UefiImageHandleOccurrenceId::from_normalized_identity);
    let image = ledger
        .admit_image_handle_physical_input(occurrence, NonZeroU64::new(0x1000_0000 + base).unwrap())
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

/// The pending exit invocation the acquisition edge borrows: a complete
/// provider join and plan against the fabricated tables.
fn planned_exit<'system_table, 'boot_services>(
    ledger: &mut UefiApplicationFirmwareLedger<'system_table>,
    system: &'system_table [u8],
    boot: &'boot_services [u8],
    base: u64,
    boot_address: u64,
) -> PlannedUefiExitBootServicesInvocation<'system_table, 'boot_services> {
    let projection = projection(ledger, system, base);
    let integrity = validate_uefi_boot_services_occurrence(
        plan_uefi_boot_services_native_layout(TargetProfile::UefiX64).unwrap(),
        boot,
    )
    .unwrap();
    let provider = join_lifecycle_scoped_uefi_exit_boot_services_provider(
        ledger,
        projection,
        integrity,
        id(
            base + 3,
            UefiBootServicesTableOccurrenceId::from_normalized_identity,
        ),
        NonZeroU64::new(boot_address).unwrap(),
    )
    .unwrap();
    prepare_uefi_exit_boot_services_invocation(provider).unwrap()
}

/// The handoff ledger binds the *firmware ledger's* session and physical
/// invocation: `session`/`invocation` are the firmware base's +1/+2 ids so
/// executed acquisition evidence joins the same invocation.
fn handoff(
    base: u64,
    session: u64,
    invocation: u64,
    attempts: u32,
) -> (UefiOsHandoffLedger, UefiOsHandoffMapRequired) {
    UefiOsHandoffLedger::new(
        id(base, UefiOsHandoffId::from_normalized_identity),
        id(session, UefiFirmwareSessionId::from_normalized_identity),
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

fn script_success(key: usize, map_bytes: usize, descriptor_size: usize, version: u64) {
    FAKE_GET_STATUS.store(EFI_SUCCESS, Ordering::SeqCst);
    FAKE_MAP_BYTES.store(map_bytes, Ordering::SeqCst);
    FAKE_MAP_KEY.store(key, Ordering::SeqCst);
    FAKE_DESCRIPTOR_SIZE.store(descriptor_size, Ordering::SeqCst);
    FAKE_DESCRIPTOR_VERSION.store(version, Ordering::SeqCst);
}

/// One acquire leg: join the provider beneath the pending exit, plan,
/// bind the buffer, execute, and admit. Returns the admitted outcome.
fn acquire_outcome<'p, 's, 'b, 'buffer>(
    firmware: &UefiApplicationFirmwareLedger<'s>,
    pending_exit: &'p PlannedUefiExitBootServicesInvocation<'s, 'b>,
    buffer: &'buffer mut UefiMemoryMapBuffer,
) -> UefiGetMemoryMapAttemptOutcome<'p, 's, 'b, 'buffer> {
    let provider =
        join_lifecycle_scoped_uefi_get_memory_map_provider(firmware, pending_exit).unwrap();
    let invocation = prepare_uefi_get_memory_map_invocation(provider).unwrap();
    let bound = bind_uefi_get_memory_map_invocation(invocation, buffer).unwrap();
    // SAFETY: the retained service address is the test-local efiapi fake
    // baked into the fabricated Boot Services table.
    let executed = unsafe { execute_uefi_get_memory_map(bound) }.unwrap();
    admit_uefi_get_memory_map_execution(executed).unwrap()
}

#[test]
fn successful_acquisition_seals_key_and_geometry_and_feeds_exit() {
    let _firmware = FIRMWARE_TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    script_success(0x5AFE_0041, 96, 48, 1);
    FAKE_EXIT_STATUS.store(EFI_SUCCESS, Ordering::SeqCst);
    OBSERVED_GET_CALLS.store(0, Ordering::SeqCst);
    let boot_address = 0x401000;
    let system = table(UEFI_SYSTEM_TABLE_SIGNATURE, 120, 96, boot_address);
    let boot = boot_table(
        fake_get_memory_map_address(),
        fake_exit_boot_services_address(),
    );
    let mut firmware = ledger(10);
    let pending_exit = planned_exit(&mut firmware, &system, &boot, 13, boot_address);
    let (mut handoff, arrival) = handoff(50, 11, 10 + INVOCATION_OFFSET, 2);

    let mut buffer = UefiMemoryMapBuffer::with_capacity(128);
    let storage = buffer.map.as_ptr() as usize;
    let UefiGetMemoryMapAttemptOutcome::Acquired {
        buffer,
        acquisition,
        ..
    } = acquire_outcome(&firmware, &pending_exit, &mut buffer)
    else {
        panic!("scripted success must mint acquisition evidence")
    };
    assert_eq!(OBSERVED_GET_CALLS.load(Ordering::SeqCst), 1);
    assert_eq!(OBSERVED_CAPACITY.load(Ordering::SeqCst), 128);
    assert_eq!(OBSERVED_MAP_POINTER.load(Ordering::SeqCst), storage);
    assert_eq!(acquisition.map_key().normalized_identity(), 0x5AFE_0041);
    assert_eq!(acquisition.map_bytes(), 96);
    assert_eq!(acquisition.descriptor_size(), 48);
    assert_eq!(acquisition.descriptor_version(), 1);
    assert_eq!(acquisition.firmware_session(), handoff.firmware_session());
    assert_eq!(
        acquisition.physical_invocation(),
        pending_exit.physical_invocation()
    );
    assert!(buffer.occupied_bytes().iter().all(|byte| *byte == 0x5a));
    assert_eq!(buffer.occupied_map_bytes(), 96);

    // The same key and descriptor identity reach the exit binding; the
    // successful exit completes the handoff with this snapshot.
    let acquired = handoff.acquire_memory_map(arrival, acquisition).unwrap();
    assert_eq!(acquired.descriptor_size(), 48);
    assert_eq!(acquired.descriptor_version(), 1);
    let snapshot = acquired.snapshot();
    let bound = bind_uefi_exit_boot_services_invocation(pending_exit, &acquired).unwrap();
    assert_eq!(bound.map_key().normalized_identity(), 0x5AFE_0041);
    // SAFETY: the retained service address is the test-local fake.
    let executed = unsafe { execute_uefi_exit_boot_services(bound) }.unwrap();
    assert_eq!(OBSERVED_EXIT_KEY.load(Ordering::SeqCst), 0x5AFE_0041);
    let UefiExitBootServicesAttemptOutcome::Exited { result, .. } =
        admit_uefi_exit_boot_services_execution(executed).unwrap()
    else {
        panic!("EFI_SUCCESS must exit the handoff attempt")
    };
    let UefiOsHandoffProgress::Complete(complete) = handoff
        .apply_exit_boot_services_result(acquired, result)
        .unwrap()
    else {
        panic!("successful provider result must complete the handoff")
    };
    assert_eq!(complete.final_map(), snapshot);
}

#[test]
fn buffer_too_small_reports_required_then_grow_and_rebind_succeeds() {
    let _firmware = FIRMWARE_TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    FAKE_GET_STATUS.store(EFI_BUFFER_TOO_SMALL, Ordering::SeqCst);
    FAKE_REQUIRED_BYTES.store(192, Ordering::SeqCst);
    let boot_address = 0x401000;
    let system = table(UEFI_SYSTEM_TABLE_SIGNATURE, 120, 96, boot_address);
    let boot = boot_table(
        fake_get_memory_map_address(),
        fake_exit_boot_services_address(),
    );
    let mut firmware = ledger(10);
    let pending_exit = planned_exit(&mut firmware, &system, &boot, 13, boot_address);

    // The zero-capacity probe shape: null map pointer, empty size cell.
    let mut probe = UefiMemoryMapBuffer::default();
    let UefiGetMemoryMapAttemptOutcome::BufferTooSmall {
        buffer: probe,
        required_map_bytes,
        ..
    } = acquire_outcome(&firmware, &pending_exit, &mut probe)
    else {
        panic!("undersized buffer must return the required extent")
    };
    assert_eq!(required_map_bytes, 192);
    assert_eq!(OBSERVED_CAPACITY.load(Ordering::SeqCst), 0);
    assert_eq!(OBSERVED_MAP_POINTER.load(Ordering::SeqCst), 0);
    probe.grow(required_map_bytes);
    assert_eq!(probe.capacity(), 192);
    assert_eq!(probe.occupied_map_bytes(), 0);

    // The same custody rebinds after growth; a too-small report that
    // fits the new capacity is a contract violation, witnessed below.
    FAKE_GET_STATUS.store(EFI_SUCCESS, Ordering::SeqCst);
    script_success(0x5AFE_0042, 96, 48, 1);
    let UefiGetMemoryMapAttemptOutcome::Acquired {
        buffer: probe,
        acquisition,
        ..
    } = acquire_outcome(&firmware, &pending_exit, probe)
    else {
        panic!("the grown buffer must acquire")
    };
    assert_eq!(acquisition.map_key().normalized_identity(), 0x5AFE_0042);
    assert_eq!(probe.occupied_map_bytes(), 96);
    assert_eq!(OBSERVED_CAPACITY.load(Ordering::SeqCst), 192);
}

#[test]
fn stale_exit_key_reacquires_a_fresh_map_and_retries_to_success() {
    let _firmware = FIRMWARE_TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    script_success(0x5AFE_0043, 96, 48, 1);
    FAKE_EXIT_STATUS.store(EFI_INVALID_PARAMETER, Ordering::SeqCst);
    // The call counter is process-global: this test asserts its own exact
    // acquisition count, so it starts from zero like its scripted siblings.
    OBSERVED_GET_CALLS.store(0, Ordering::SeqCst);
    let boot_address = 0x401000;
    let system = table(UEFI_SYSTEM_TABLE_SIGNATURE, 120, 96, boot_address);
    let boot = boot_table(
        fake_get_memory_map_address(),
        fake_exit_boot_services_address(),
    );
    let mut firmware = ledger(10);
    let pending_exit = planned_exit(&mut firmware, &system, &boot, 13, boot_address);
    let (mut handoff, arrival) = handoff(50, 11, 10 + INVOCATION_OFFSET, 2);
    let mut buffer = UefiMemoryMapBuffer::with_capacity(128);

    // First attempt: key A exits stale; the handoff retires it and the
    // exit edge returns the complete provider custody for retry.
    let UefiGetMemoryMapAttemptOutcome::Acquired {
        buffer: map,
        acquisition,
        ..
    } = acquire_outcome(&firmware, &pending_exit, &mut buffer)
    else {
        panic!("the first acquisition must succeed")
    };
    let acquired = handoff.acquire_memory_map(arrival, acquisition).unwrap();
    let bound = bind_uefi_exit_boot_services_invocation(pending_exit, &acquired).unwrap();
    // SAFETY: the retained service address is the test-local fake.
    let executed = unsafe { execute_uefi_exit_boot_services(bound) }.unwrap();
    assert_eq!(OBSERVED_EXIT_KEY.load(Ordering::SeqCst), 0x5AFE_0043);
    let UefiExitBootServicesAttemptOutcome::Retry {
        invocation: pending_exit,
        result,
        ..
    } = admit_uefi_exit_boot_services_execution(executed).unwrap()
    else {
        panic!("stale key must return provider custody")
    };
    let UefiOsHandoffProgress::Retry(retry) = handoff
        .apply_exit_boot_services_result(acquired, result)
        .unwrap()
    else {
        panic!("the first stale key must retry")
    };

    // A reacquired map carrying the retired key rejects before custody is
    // spent: the acquisition evidence returns with the arrival intact.
    let UefiGetMemoryMapAttemptOutcome::Acquired {
        buffer: map,
        acquisition,
        ..
    } = acquire_outcome(&firmware, &pending_exit, map)
    else {
        panic!("the second acquisition must succeed")
    };
    let error = handoff.acquire_memory_map(retry, acquisition).unwrap_err();
    assert!(error.diagnostic().0.contains("retired"));
    let (retry, _stale_acquisition, _) = error.into_parts();

    // Firmware's newest key feeds the retried exit and completes.
    script_success(0x5AFE_0044, 96, 48, 1);
    let UefiGetMemoryMapAttemptOutcome::Acquired { acquisition, .. } =
        acquire_outcome(&firmware, &pending_exit, map)
    else {
        panic!("the fresh acquisition must succeed")
    };
    let acquired = handoff.acquire_memory_map(retry, acquisition).unwrap();
    assert_eq!(acquired.map_key().normalized_identity(), 0x5AFE_0044);
    let bound = bind_uefi_exit_boot_services_invocation(pending_exit, &acquired).unwrap();
    FAKE_EXIT_STATUS.store(EFI_SUCCESS, Ordering::SeqCst);
    // SAFETY: same retained fake service address.
    let executed = unsafe { execute_uefi_exit_boot_services(bound) }.unwrap();
    assert_eq!(OBSERVED_EXIT_KEY.load(Ordering::SeqCst), 0x5AFE_0044);
    let UefiExitBootServicesAttemptOutcome::Exited { result, .. } =
        admit_uefi_exit_boot_services_execution(executed).unwrap()
    else {
        panic!("the retried attempt must exit on success")
    };
    let UefiOsHandoffProgress::Complete(_) = handoff
        .apply_exit_boot_services_result(acquired, result)
        .unwrap()
    else {
        panic!("the retried success must complete the handoff")
    };
    assert_eq!(OBSERVED_GET_CALLS.load(Ordering::SeqCst), 3);
}

#[test]
fn foreign_invocation_and_session_reject_before_acquisition_use() {
    let _firmware = FIRMWARE_TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let boot_address = 0x401000;
    let system = table(UEFI_SYSTEM_TABLE_SIGNATURE, 120, 96, boot_address);
    let boot = boot_table(
        fake_get_memory_map_address(),
        fake_exit_boot_services_address(),
    );
    let mut owner = ledger(10);
    let foreign = ledger(60);
    let pending_exit = planned_exit(&mut owner, &system, &boot, 13, boot_address);

    // A firmware ledger that does not own the borrowed invocation cannot
    // join the acquisition edge.
    let error =
        join_lifecycle_scoped_uefi_get_memory_map_provider(&foreign, &pending_exit).unwrap_err();
    assert!(error.0.contains("different or inactive"));

    // Evidence minted under a different physical invocation or firmware
    // session cannot feed this handoff's arrival even when every other
    // identity matches; both legs reject before custody is spent.
    let (mut handoff, mut arrival) = handoff(50, 11, 10 + INVOCATION_OFFSET, 2);
    for (session, invocation) in [
        (
            handoff.firmware_session(),
            id(999, UefiPhysicalInvocationId::from_normalized_identity),
        ),
        (
            id(999, UefiFirmwareSessionId::from_normalized_identity),
            handoff.physical_invocation(),
        ),
    ] {
        let foreign_acquisition = UefiMemoryMapAcquisition::for_test(
            session,
            invocation,
            id(70, UefiMemoryMapSnapshotId::from_normalized_identity),
            id(71, UefiMemoryMapKeyId::from_normalized_identity),
            96,
            48,
            1,
        );
        let error = handoff
            .acquire_memory_map(arrival, foreign_acquisition)
            .unwrap_err();
        assert!(
            error
                .diagnostic()
                .0
                .contains("different physical invocation")
        );
        let (returned_arrival, _acquisition, _) = error.into_parts();
        arrival = returned_arrival;
    }

    // The same arrival with exact-invocation evidence still acquires.
    script_success(0x5AFE_0045, 96, 48, 1);
    let mut buffer = UefiMemoryMapBuffer::with_capacity(128);
    let UefiGetMemoryMapAttemptOutcome::Acquired { acquisition, .. } =
        acquire_outcome(&owner, &pending_exit, &mut buffer)
    else {
        panic!("the exact-invocation acquisition must succeed")
    };
    let _acquired = handoff.acquire_memory_map(arrival, acquisition).unwrap();
}

#[test]
fn rejected_and_malformed_statuses_retain_executed_custody() {
    let _firmware = FIRMWARE_TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let boot_address = 0x401000;
    let system = table(UEFI_SYSTEM_TABLE_SIGNATURE, 120, 96, boot_address);
    let boot = boot_table(
        fake_get_memory_map_address(),
        fake_exit_boot_services_address(),
    );
    let mut firmware = ledger(10);
    let pending_exit = planned_exit(&mut firmware, &system, &boot, 13, boot_address);
    let mut buffer = UefiMemoryMapBuffer::with_capacity(128);
    let mut pending = &pending_exit;
    let mut map: &mut UefiMemoryMapBuffer = &mut buffer;

    // InvalidParameter and statuses outside the closed target table both
    // reject with executed custody; each release returns the identical
    // pending-exit borrow and the buffer.
    for (status, fragment) in [
        (EFI_INVALID_PARAMETER, "rejecting status"),
        (EFI_STATUS_ERROR_BIT | 9, "outside its closed target table"),
    ] {
        FAKE_GET_STATUS.store(status, Ordering::SeqCst);
        let provider =
            join_lifecycle_scoped_uefi_get_memory_map_provider(&firmware, pending).unwrap();
        let invocation = prepare_uefi_get_memory_map_invocation(provider).unwrap();
        let bound = bind_uefi_get_memory_map_invocation(invocation, map).unwrap();
        // SAFETY: the retained service address is the test-local fake.
        let executed = unsafe { execute_uefi_get_memory_map(bound) }.unwrap();
        let error = admit_uefi_get_memory_map_execution(executed).unwrap_err();
        assert!(
            error.diagnostic().0.contains(fragment),
            "missing {fragment}"
        );
        let (executed, _) = error.into_parts();
        let (returned_pending, returned_map) = executed.into_pending_exit_invocation();
        assert!(std::ptr::eq(returned_pending, pending));
        pending = returned_pending;
        map = returned_map;
    }

    // A successful status whose sealed cells violate the firmware
    // contract never mints acquisition evidence.
    FAKE_GET_STATUS.store(EFI_SUCCESS, Ordering::SeqCst);
    for (key, bytes, descriptor_size, version, fragment) in [
        (0, 96, 48, 1, "zero map key"),
        (0x5AFE_0050, 96, 32, 1, "descriptor geometry"),
        (0x5AFE_0051, 100, 48, 1, "descriptor geometry"),
        (0x5AFE_0052, 256, 48, 1, "empty or over-capacity"),
        (0x5AFE_0053, 96, 48, 0, "zero descriptor version"),
    ] {
        script_success(key, bytes, descriptor_size, version);
        let provider =
            join_lifecycle_scoped_uefi_get_memory_map_provider(&firmware, pending).unwrap();
        let invocation = prepare_uefi_get_memory_map_invocation(provider).unwrap();
        let bound = bind_uefi_get_memory_map_invocation(invocation, map).unwrap();
        // SAFETY: the retained service address is the test-local fake.
        let executed = unsafe { execute_uefi_get_memory_map(bound) }.unwrap();
        let error = admit_uefi_get_memory_map_execution(executed).unwrap_err();
        assert!(
            error.diagnostic().0.contains(fragment),
            "missing {fragment}"
        );
        let (executed, _) = error.into_parts();
        let (returned_pending, returned_map) = executed.into_pending_exit_invocation();
        pending = returned_pending;
        map = returned_map;
    }

    // BufferTooSmall without a real growth requirement rejects the same
    // way; the post-execution cell-drift check cannot fire because the
    // executed carrier still exclusively holds the buffer between
    // execution and admission.
    FAKE_GET_STATUS.store(EFI_BUFFER_TOO_SMALL, Ordering::SeqCst);
    FAKE_REQUIRED_BYTES.store(64, Ordering::SeqCst);
    let provider = join_lifecycle_scoped_uefi_get_memory_map_provider(&firmware, pending).unwrap();
    let invocation = prepare_uefi_get_memory_map_invocation(provider).unwrap();
    let bound = bind_uefi_get_memory_map_invocation(invocation, map).unwrap();
    // SAFETY: the retained service address is the test-local fake.
    let executed = unsafe { execute_uefi_get_memory_map(bound) }.unwrap();
    let error = admit_uefi_get_memory_map_execution(executed).unwrap_err();
    assert!(
        error
            .diagnostic()
            .0
            .contains("did not report a growth requirement")
    );
    let (executed, _) = error.into_parts();
    let (pending, map) = executed.into_pending_exit_invocation();

    // After every rejection the same custody still drives a clean
    // acquisition.
    script_success(0x5AFE_0054, 96, 48, 1);
    let UefiGetMemoryMapAttemptOutcome::Acquired { acquisition, .. } =
        acquire_outcome(&firmware, pending, map)
    else {
        panic!("custody returned by rejections must still acquire")
    };
    assert_eq!(acquisition.map_key().normalized_identity(), 0x5AFE_0054);
}

#[test]
fn custody_releases_return_the_identical_pending_exit_invocation() {
    let _firmware = FIRMWARE_TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let boot_address = 0x401000;
    let system = table(UEFI_SYSTEM_TABLE_SIGNATURE, 120, 96, boot_address);
    let boot = boot_table(
        fake_get_memory_map_address(),
        fake_exit_boot_services_address(),
    );
    let mut firmware = ledger(10);
    let pending_exit = planned_exit(&mut firmware, &system, &boot, 13, boot_address);
    let mut buffer = UefiMemoryMapBuffer::with_capacity(128);

    let provider =
        join_lifecycle_scoped_uefi_get_memory_map_provider(&firmware, &pending_exit).unwrap();
    let pending = provider.into_pending_exit_invocation();
    assert!(std::ptr::eq(pending, &pending_exit));

    let provider = join_lifecycle_scoped_uefi_get_memory_map_provider(&firmware, pending).unwrap();
    let invocation = prepare_uefi_get_memory_map_invocation(provider).unwrap();
    let pending = invocation.into_pending_exit_invocation();
    assert!(std::ptr::eq(pending, &pending_exit));

    let provider = join_lifecycle_scoped_uefi_get_memory_map_provider(&firmware, pending).unwrap();
    let invocation = prepare_uefi_get_memory_map_invocation(provider).unwrap();
    let bound = bind_uefi_get_memory_map_invocation(invocation, &mut buffer).unwrap();
    let (pending, _buffer) = bound.into_pending_exit_invocation();
    assert!(std::ptr::eq(pending, &pending_exit));
}

#[test]
fn acquisition_issuance_stays_module_family_private() {
    let production_source = [
        include_str!("../get_memory_map.rs"),
        include_str!("memory_map_buffer.rs"),
        include_str!("provider_lifecycle.rs"),
        include_str!("invocation_planning.rs"),
        include_str!("execution.rs"),
    ]
    .concat();
    // Module-internal field visibility is no public projection.
    let production_source = production_source.replace("pub(super) ", "");
    // The route file declares its tests module; the concept files carry no tests.
    let production = production_source.as_str();
    let compact = production
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect::<String>();
    assert!(compact.contains(
        "pubstructUefiMemoryMapAcquisition{session:UefiFirmwareSessionId,invocation:UefiPhysicalInvocationId,"
    ));
    for forbidden in [
        "pubfnfor_test",
        "pub(crate)fnfor_test",
        "implCloneforUefiMemoryMap",
        "implCloneforUefiGetMemoryMap",
        "UefiExitBootServicesProviderResultKind",
    ] {
        assert!(
            !compact.contains(forbidden),
            "forbidden acquisition authority surface appeared: {forbidden}"
        );
    }
}

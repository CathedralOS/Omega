//! OS handoff cycle tests.

use super::{
    ExternalRootDiagnostic, PlannedUefiExitBootServicesInvocation, UefiApplicationFirmwareLedger,
    UefiMemoryMapBuffer, UefiOsHandoffCycleRejection, UefiOsHandoffCycleResolution,
    UefiOsHandoffLedger, UefiOsHandoffMapRequired, drive_uefi_os_handoff_cycle,
};
use crate::{
    LifecycleScopedUefiBootServicesProjection, UefiApplicationBootstrapLedgerId,
    UefiBootServicesPhaseLeaseId, UefiBootServicesTableOccurrenceId, UefiErrorStatus,
    UefiFirmwareSessionId, UefiMemoryMapAcquisition, UefiMemoryMapKeyId, UefiMemoryMapSnapshotId,
    UefiOsHandoffAllocationRosterId, UefiOsHandoffBootServicesId, UefiOsHandoffId,
    UefiOsHandoffStackEvidenceId, UefiPhysicalInvocationId, UefiSystemTableOccurrenceId,
    bind_uefi_exit_boot_services_invocation,
    join_lifecycle_scoped_uefi_exit_boot_services_provider,
    join_lifecycle_scoped_uefi_system_table, join_uefi_application_physical_arrival,
    prepare_uefi_application_bootstrap_adapter_invocation,
    prepare_uefi_exit_boot_services_invocation, project_uefi_application_boot_services,
};
use program_entry_plan::{
    ProgramEntryPhysicalContractPlan, UEFI_X64_IMAGE_HANDLE_TYPE_IDENTITY,
    UEFI_X64_PHYSICAL_REQUIREMENT_IDENTITY, UEFI_X64_STATUS_TYPE_IDENTITY,
    UEFI_X64_SYSTEM_TABLE_REFERENCE_TYPE_IDENTITY, exact_uefi_x64_physical_boundary_entry_plan,
    exact_uefi_x64_physical_contract_package_source_digest, plan_uefi_os_handoff_invocation,
};
use std::collections::VecDeque;
use std::ffi::c_void;
use std::num::{NonZeroU32, NonZeroU64};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use target::{
    ProgramEntryPhysicalContractPackage, TargetProfile, UEFI_BOOT_SERVICES_SIGNATURE,
    UEFI_SYSTEM_TABLE_SIGNATURE, UefiBootServicesNativeField,
    plan_uefi_boot_services_native_layout, plan_uefi_system_table_native_layout,
    validate_uefi_boot_services_occurrence, validate_uefi_system_table_occurrence,
};

const EFI_STATUS_ERROR_BIT: u64 = 1_u64 << 63;
const EFI_SUCCESS: u64 = 0;
const EFI_INVALID_PARAMETER: u64 = EFI_STATUS_ERROR_BIT | 2;
const EFI_BUFFER_TOO_SMALL: u64 = EFI_STATUS_ERROR_BIT | 5;

/// One scripted `GetMemoryMap` firmware response.
#[derive(Clone, Copy)]
enum FakeGetResponse {
    Success {
        key: usize,
        map_bytes: usize,
        descriptor_size: usize,
        version: u64,
    },
    BufferTooSmall {
        required: usize,
    },
    Raw(u64),
}

static FIRMWARE_TEST_LOCK: Mutex<()> = Mutex::new(());
static GET_RESPONSES: Mutex<VecDeque<FakeGetResponse>> = Mutex::new(VecDeque::new());
static EXIT_RESPONSES: Mutex<VecDeque<u64>> = Mutex::new(VecDeque::new());
static OBSERVED_GET_CALLS: AtomicUsize = AtomicUsize::new(0);
static OBSERVED_GET_CAPACITIES: Mutex<Vec<usize>> = Mutex::new(Vec::new());
static OBSERVED_EXIT_KEYS: Mutex<Vec<usize>> = Mutex::new(Vec::new());

fn script_get(responses: &[FakeGetResponse]) {
    *GET_RESPONSES.lock().unwrap() = responses.iter().copied().collect();
}

fn script_exit(statuses: &[u64]) {
    *EXIT_RESPONSES.lock().unwrap() = statuses.iter().copied().collect();
}

fn reset_observers() {
    OBSERVED_GET_CALLS.store(0, Ordering::SeqCst);
    OBSERVED_GET_CAPACITIES.lock().unwrap().clear();
    OBSERVED_EXIT_KEYS.lock().unwrap().clear();
}

/// Each call consumes the next scripted step until one remains, which then
/// repeats — a trailing success or failure answer stays live for every
/// later call in the same drive.
fn next_get_response() -> FakeGetResponse {
    let mut responses = GET_RESPONSES.lock().unwrap();
    let response = *responses.front().expect("scripted GetMemoryMap response");
    if responses.len() > 1 {
        responses.pop_front();
    }
    response
}

fn next_exit_status() -> u64 {
    let mut responses = EXIT_RESPONSES.lock().unwrap();
    let status = *responses.front().expect("scripted ExitBootServices status");
    if responses.len() > 1 {
        responses.pop_front();
    }
    status
}

/// Firmware-shaped fake: reads the in-out size cell, writes the scripted
/// required extent on BufferTooSmall and the map bytes plus every output
/// cell on success.
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
    OBSERVED_GET_CAPACITIES.lock().unwrap().push(capacity);
    match next_get_response() {
        FakeGetResponse::Success {
            key,
            map_bytes,
            descriptor_size: stride,
            version,
        } => {
            let writable = map_bytes.min(capacity);
            if !memory_map.is_null() && writable != 0 {
                // SAFETY: the edge guarantees `capacity` writable bytes.
                unsafe { std::ptr::write_bytes(memory_map.cast::<u8>(), 0x5a, writable) };
            }
            // SAFETY: same exclusive output-cell custody as above.
            unsafe {
                *memory_map_size = map_bytes;
                *map_key = key;
                *descriptor_size = stride;
                *descriptor_version = version as u32;
            }
            EFI_SUCCESS
        }
        FakeGetResponse::BufferTooSmall { required } => {
            // SAFETY: same exclusive output-cell custody as above.
            unsafe { *memory_map_size = required };
            EFI_BUFFER_TOO_SMALL
        }
        FakeGetResponse::Raw(status) => status,
    }
}

unsafe extern "efiapi" fn fake_exit_boot_services(
    _image_handle: *mut c_void,
    map_key: usize,
) -> u64 {
    OBSERVED_EXIT_KEYS.lock().unwrap().push(map_key);
    next_exit_status()
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

/// Fabricated Boot Services table carrying both handoff-edge rows at their
/// exact target-layout offsets.
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

fn projection_with_image_handle<'a>(
    ledger: &mut UefiApplicationFirmwareLedger<'a>,
    bytes: &'a [u8],
    base: u64,
    retain_physical_value: bool,
) -> LifecycleScopedUefiBootServicesProjection<'a> {
    let occurrence = id(
        base,
        crate::UefiImageHandleOccurrenceId::from_normalized_identity,
    );
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

/// The pending exit invocation the cycle borrows for acquisition and
/// consumes at the exit binding.
fn planned_exit<'system_table, 'boot_services>(
    ledger: &mut UefiApplicationFirmwareLedger<'system_table>,
    system: &'system_table [u8],
    boot: &'boot_services [u8],
    base: u64,
    boot_address: u64,
    retain_image_handle_value: bool,
) -> PlannedUefiExitBootServicesInvocation<'system_table, 'boot_services> {
    let projection = projection_with_image_handle(ledger, system, base, retain_image_handle_value);
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

fn success(key: usize) -> FakeGetResponse {
    FakeGetResponse::Success {
        key,
        map_bytes: 96,
        descriptor_size: 48,
        version: 1,
    }
}

#[test]
fn cycle_acquires_then_exits_with_the_acquired_key() {
    let _firmware = FIRMWARE_TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    script_get(&[success(0x5AFE_1001)]);
    script_exit(&[EFI_SUCCESS]);
    reset_observers();
    let boot_address = 0x401000;
    let system = table(UEFI_SYSTEM_TABLE_SIGNATURE, 120, 96, boot_address);
    let boot = boot_table(
        fake_get_memory_map_address(),
        fake_exit_boot_services_address(),
    );
    let mut firmware = ledger(10);
    let pending_exit = planned_exit(&mut firmware, &system, &boot, 13, boot_address, true);
    let (mut handoff, arrival) = handoff(50, 11, 10 + INVOCATION_OFFSET, 2);
    let allocations = arrival.allocation_roster();
    let stack = arrival.surviving_stack();
    let mut map = UefiMemoryMapBuffer::with_capacity(128);

    // SAFETY: the fabricated tables retain the test-local efiapi fakes as
    // the service addresses; the buffer is exclusively owned for the drive.
    let resolution = unsafe {
        drive_uefi_os_handoff_cycle(&firmware, &mut handoff, arrival, pending_exit, &mut map)
    }
    .unwrap();
    let UefiOsHandoffCycleResolution::Complete { completion, buffer } = resolution else {
        panic!("a successful exit must complete the handoff")
    };
    assert_eq!(OBSERVED_GET_CALLS.load(Ordering::SeqCst), 1);
    assert_eq!(*OBSERVED_EXIT_KEYS.lock().unwrap(), [0x5AFE_1001]);
    assert_eq!(completion.physical_invocation().normalized_identity(), 12);
    assert_eq!(completion.allocation_roster(), allocations);
    assert_eq!(completion.surviving_stack(), stack);
    assert_ne!(completion.receipt().normalized_identity(), 0);
    // The buffer still holds the final map's occupied prefix.
    assert_eq!(buffer.occupied_map_bytes(), 96);
    assert!(buffer.occupied_bytes().iter().all(|byte| *byte == 0x5a));
}

#[test]
fn grow_and_stale_key_retries_carry_the_newest_key_to_exit() {
    let _firmware = FIRMWARE_TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    script_get(&[
        FakeGetResponse::BufferTooSmall { required: 192 },
        success(0x5AFE_1002),
        success(0x5AFE_1003),
    ]);
    script_exit(&[EFI_INVALID_PARAMETER, EFI_SUCCESS]);
    reset_observers();
    let boot_address = 0x401000;
    let system = table(UEFI_SYSTEM_TABLE_SIGNATURE, 120, 96, boot_address);
    let boot = boot_table(
        fake_get_memory_map_address(),
        fake_exit_boot_services_address(),
    );
    let mut firmware = ledger(10);
    let pending_exit = planned_exit(&mut firmware, &system, &boot, 13, boot_address, true);
    let (mut handoff, arrival) = handoff(50, 11, 10 + INVOCATION_OFFSET, 2);
    let mut map = UefiMemoryMapBuffer::default();

    // SAFETY: same fabricated custody as above.
    let resolution = unsafe {
        drive_uefi_os_handoff_cycle(&firmware, &mut handoff, arrival, pending_exit, &mut map)
    }
    .unwrap();
    let UefiOsHandoffCycleResolution::Complete { completion, buffer } = resolution else {
        panic!("the retried success must complete the handoff")
    };
    // The zero-capacity probe grew to the firmware requirement without
    // spending a handoff attempt; the stale first key drove a fresh
    // acquisition, so the exit saw exactly the two newest keys in order.
    assert_eq!(OBSERVED_GET_CALLS.load(Ordering::SeqCst), 3);
    assert_eq!(*OBSERVED_GET_CAPACITIES.lock().unwrap(), [0, 192, 192]);
    assert_eq!(
        *OBSERVED_EXIT_KEYS.lock().unwrap(),
        [0x5AFE_1002, 0x5AFE_1003]
    );
    assert_eq!(buffer.capacity(), 192);
    assert_eq!(buffer.occupied_map_bytes(), 96);
    assert_eq!(completion.physical_invocation().normalized_identity(), 12);
}

#[test]
fn exhausted_cycle_returns_live_provider_and_buffer_custody() {
    let _firmware = FIRMWARE_TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    script_get(&[
        FakeGetResponse::BufferTooSmall { required: 192 },
        success(0x5AFE_1004),
    ]);
    script_exit(&[EFI_INVALID_PARAMETER]);
    reset_observers();
    let boot_address = 0x401000;
    let system = table(UEFI_SYSTEM_TABLE_SIGNATURE, 120, 96, boot_address);
    let boot = boot_table(
        fake_get_memory_map_address(),
        fake_exit_boot_services_address(),
    );
    let mut firmware = ledger(10);
    let pending_exit = planned_exit(&mut firmware, &system, &boot, 13, boot_address, true);
    let (mut handoff, arrival) = handoff(50, 11, 10 + INVOCATION_OFFSET, 1);
    let mut map = UefiMemoryMapBuffer::default();

    // SAFETY: same fabricated custody as above.
    let resolution = unsafe {
        drive_uefi_os_handoff_cycle(&firmware, &mut handoff, arrival, pending_exit, &mut map)
    }
    .unwrap();
    let UefiOsHandoffCycleResolution::Exhausted {
        exhaustion,
        pending_exit,
        buffer,
    } = resolution
    else {
        panic!("the single stale key must exhaust the bounded cycle")
    };
    // The grow retry never spent the only attempt: acquisition still ran
    // the probe and the grown call, and one stale exit exhausted the loop.
    assert_eq!(*OBSERVED_GET_CAPACITIES.lock().unwrap(), [0, 192]);
    assert_eq!(*OBSERVED_EXIT_KEYS.lock().unwrap(), [0x5AFE_1004]);
    assert_eq!(exhaustion.status().value(), EFI_STATUS_ERROR_BIT);
    assert_eq!(exhaustion.boot_services().normalized_identity(), 53);
    assert_eq!(buffer.occupied_map_bytes(), 96);

    // Exhaustion leaves Boot Services live: the returned pending-exit
    // custody still releases so the invocation can begin its firmware
    // return.
    let released = firmware
        .release_planned_uefi_exit_boot_services_invocation(pending_exit)
        .unwrap();
    assert_eq!(released.ledger, firmware.ledger_id());
    firmware.begin_firmware_return().unwrap();
}

#[test]
fn acquisition_rejection_returns_complete_custody_for_a_redrive() {
    let _firmware = FIRMWARE_TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    script_get(&[FakeGetResponse::Raw(EFI_INVALID_PARAMETER)]);
    script_exit(&[EFI_SUCCESS]);
    reset_observers();
    let boot_address = 0x401000;
    let system = table(UEFI_SYSTEM_TABLE_SIGNATURE, 120, 96, boot_address);
    let boot = boot_table(
        fake_get_memory_map_address(),
        fake_exit_boot_services_address(),
    );
    let mut firmware = ledger(10);
    let pending_exit = planned_exit(&mut firmware, &system, &boot, 13, boot_address, true);
    let (mut handoff, arrival) = handoff(50, 11, 10 + INVOCATION_OFFSET, 2);
    let mut map = UefiMemoryMapBuffer::with_capacity(128);

    // SAFETY: same fabricated custody as above.
    let rejection = unsafe {
        drive_uefi_os_handoff_cycle(&firmware, &mut handoff, arrival, pending_exit, &mut map)
    }
    .unwrap_err();
    let UefiOsHandoffCycleRejection::MapAcquisition {
        pending_exit,
        buffer,
        arrival,
        diagnostic,
    } = *rejection
    else {
        panic!("a rejected acquisition status must return unspent custody")
    };
    assert!(diagnostic.0.contains("rejecting status"));
    assert_eq!(arrival.remaining_attempts(), 2);

    // The returned custody still drives a clean cycle to completion.
    script_get(&[success(0x5AFE_1005)]);
    // SAFETY: the same fabricated custody re-drives with a fixed script.
    let resolution = unsafe {
        drive_uefi_os_handoff_cycle(&firmware, &mut handoff, arrival, pending_exit, buffer)
    }
    .unwrap();
    let UefiOsHandoffCycleResolution::Complete { completion, .. } = resolution else {
        panic!("the re-driven custody must complete the handoff")
    };
    assert_eq!(*OBSERVED_EXIT_KEYS.lock().unwrap(), [0x5AFE_1005]);
    assert_eq!(completion.physical_invocation().normalized_identity(), 12);
}

#[test]
fn foreign_arrival_rejects_after_acquisition_with_unspent_evidence() {
    let _firmware = FIRMWARE_TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    script_get(&[success(0x5AFE_1006)]);
    script_exit(&[EFI_SUCCESS]);
    reset_observers();
    let boot_address = 0x401000;
    let system = table(UEFI_SYSTEM_TABLE_SIGNATURE, 120, 96, boot_address);
    let boot = boot_table(
        fake_get_memory_map_address(),
        fake_exit_boot_services_address(),
    );
    let mut firmware = ledger(10);
    let pending_exit = planned_exit(&mut firmware, &system, &boot, 13, boot_address, true);
    // An arrival minted by a different handoff ledger carries no custody
    // in this one; the acquisition executes first, then the ledger
    // rejects it with the unspent evidence returned.
    let (_foreign_handoff, foreign_arrival) = handoff(90, 91, 92, 2);
    let (mut handoff, _live_arrival) = handoff(50, 11, 10 + INVOCATION_OFFSET, 2);
    let mut map = UefiMemoryMapBuffer::with_capacity(128);

    // SAFETY: same fabricated custody as above.
    let rejection = unsafe {
        drive_uefi_os_handoff_cycle(
            &firmware,
            &mut handoff,
            foreign_arrival,
            pending_exit,
            &mut map,
        )
    }
    .unwrap_err();
    let UefiOsHandoffCycleRejection::MapAdmission {
        acquisition,
        diagnostic,
        ..
    } = *rejection
    else {
        panic!("a foreign arrival must reject at ledger admission")
    };
    assert!(diagnostic.0.contains("foreign, stale, or has lost custody"));
    assert_eq!(OBSERVED_GET_CALLS.load(Ordering::SeqCst), 1);
    assert!(OBSERVED_EXIT_KEYS.lock().unwrap().is_empty());
    assert_eq!(acquisition.map_key().normalized_identity(), 0x5AFE_1006);
    assert_eq!(acquisition.physical_invocation().normalized_identity(), 12);
}

#[test]
fn exit_status_outside_the_closed_table_returns_executed_custody() {
    let _firmware = FIRMWARE_TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    script_get(&[success(0x5AFE_1007)]);
    script_exit(&[EFI_STATUS_ERROR_BIT | 9]);
    reset_observers();
    let boot_address = 0x401000;
    let system = table(UEFI_SYSTEM_TABLE_SIGNATURE, 120, 96, boot_address);
    let boot = boot_table(
        fake_get_memory_map_address(),
        fake_exit_boot_services_address(),
    );
    let mut firmware = ledger(10);
    let pending_exit = planned_exit(&mut firmware, &system, &boot, 13, boot_address, true);
    let (mut handoff, arrival) = handoff(50, 11, 10 + INVOCATION_OFFSET, 2);
    let mut map = UefiMemoryMapBuffer::with_capacity(128);

    // SAFETY: same fabricated custody as above.
    let rejection = unsafe {
        drive_uefi_os_handoff_cycle(&firmware, &mut handoff, arrival, pending_exit, &mut map)
    }
    .unwrap_err();
    let UefiOsHandoffCycleRejection::ExitAdmission {
        execution,
        diagnostic,
        ..
    } = *rejection
    else {
        panic!("an unknown exit status must return executed custody")
    };
    assert!(diagnostic.0.contains("closed target table"));
    assert_eq!(execution.map_key().normalized_identity(), 0x5AFE_1007);
    assert_eq!(*OBSERVED_EXIT_KEYS.lock().unwrap(), [0x5AFE_1007]);

    // The executed custody still releases through the firmware ledger.
    let released = firmware
        .release_executed_uefi_exit_boot_services_invocation(execution)
        .unwrap();
    assert_eq!(released.ledger, firmware.ledger_id());
    firmware.begin_firmware_return().unwrap();
}

#[test]
fn address_free_image_handle_rejects_at_exit_binding_with_acquired_intact() {
    let _firmware = FIRMWARE_TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    script_get(&[success(0x5AFE_1008)]);
    script_exit(&[EFI_SUCCESS]);
    reset_observers();
    let boot_address = 0x401000;
    let system = table(UEFI_SYSTEM_TABLE_SIGNATURE, 120, 96, boot_address);
    let boot = boot_table(
        fake_get_memory_map_address(),
        fake_exit_boot_services_address(),
    );
    let mut firmware = ledger(10);
    // The pending exit retains no physical image-handle value, so the exit
    // operand edge cannot bind RCX; the acquired attempt returns intact.
    let pending_exit = planned_exit(&mut firmware, &system, &boot, 13, boot_address, false);
    let (mut handoff, arrival) = handoff(50, 11, 10 + INVOCATION_OFFSET, 2);
    let mut map = UefiMemoryMapBuffer::with_capacity(128);

    // SAFETY: same fabricated custody as above.
    let rejection = unsafe {
        drive_uefi_os_handoff_cycle(&firmware, &mut handoff, arrival, pending_exit, &mut map)
    }
    .unwrap_err();
    let UefiOsHandoffCycleRejection::ExitBinding {
        pending_exit,
        acquired,
        diagnostic,
        ..
    } = *rejection
    else {
        panic!("an address-free image handle must reject at exit binding")
    };
    assert!(diagnostic.0.contains("image-handle value"));
    assert_eq!(acquired.map_key().normalized_identity(), 0x5AFE_1008);
    assert!(OBSERVED_EXIT_KEYS.lock().unwrap().is_empty());

    let released = firmware
        .release_planned_uefi_exit_boot_services_invocation(pending_exit)
        .unwrap();
    assert_eq!(released.ledger, firmware.ledger_id());
    firmware.begin_firmware_return().unwrap();
}

#[test]
fn drifted_pending_exit_leg_rejects_before_any_firmware_call_with_custody_intact() {
    let _firmware = FIRMWARE_TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    script_get(&[success(0x5AFE_1009)]);
    script_exit(&[EFI_SUCCESS]);
    reset_observers();
    let boot_address = 0x401000;
    let system = table(UEFI_SYSTEM_TABLE_SIGNATURE, 120, 96, boot_address);
    let boot = boot_table(
        fake_get_memory_map_address(),
        fake_exit_boot_services_address(),
    );
    let mut firmware = ledger(10);
    let mut pending_exit = planned_exit(&mut firmware, &system, &boot, 13, boot_address, true);
    // Drift the retained leg to the exact plan's other row. It still replays
    // as an exact GetMemoryMap leg, so only the provider-row gate can catch
    // that the pending exit no longer carries the ExitBootServices contract.
    let exact = plan_uefi_os_handoff_invocation(TargetProfile::UefiX64).unwrap();
    let exit_leg = std::mem::replace(&mut pending_exit.plan, exact.get_memory_map().clone());
    assert_eq!(exit_leg, *exact.exit_boot_services());
    assert!(pending_exit.plan.matches_exact_uefi_x64_plan());

    // The exit operand edge refuses the drifted leg and returns the planned
    // custody unchanged; the acquired attempt it was offered stays unspent.
    let (mut binding_handoff, binding_arrival) = handoff(90, 11, 10 + INVOCATION_OFFSET, 1);
    let acquired = binding_handoff
        .acquire_memory_map(
            binding_arrival,
            UefiMemoryMapAcquisition::for_test(
                binding_handoff.firmware_session(),
                binding_handoff.physical_invocation(),
                id(95, UefiMemoryMapSnapshotId::from_normalized_identity),
                id(96, UefiMemoryMapKeyId::from_normalized_identity),
                96,
                48,
                1,
            ),
        )
        .unwrap();
    let error = bind_uefi_exit_boot_services_invocation(pending_exit, &acquired).unwrap_err();
    assert!(error.diagnostic().0.contains("drifted"));
    assert_eq!(acquired.map_key().normalized_identity(), 96);
    let (pending_exit, _) = error.into_parts();

    // The cycle refuses the same custody at its first stage, before the
    // acquisition provider is even joined: no firmware call happens and the
    // pending exit, buffer, and arrival all return.
    let (mut handoff, arrival) = handoff(50, 11, 10 + INVOCATION_OFFSET, 2);
    let mut map = UefiMemoryMapBuffer::with_capacity(128);
    // SAFETY: same fabricated custody as the other cycle tests.
    let rejection = unsafe {
        drive_uefi_os_handoff_cycle(&firmware, &mut handoff, arrival, pending_exit, &mut map)
    }
    .unwrap_err();
    let UefiOsHandoffCycleRejection::MapAcquisition {
        mut pending_exit,
        buffer,
        arrival,
        diagnostic,
    } = *rejection
    else {
        panic!("a drifted pending exit leg must reject before acquisition")
    };
    assert!(
        diagnostic
            .0
            .contains("drifted pending ExitBootServices plan")
    );
    assert_eq!(OBSERVED_GET_CALLS.load(Ordering::SeqCst), 0);
    assert!(OBSERVED_EXIT_KEYS.lock().unwrap().is_empty());
    assert_eq!(arrival.remaining_attempts(), 2);
    assert_eq!(buffer.capacity(), 128);

    // Restoring the exact ExitBootServices leg lets the identical custody
    // drive to completion with the acquired key.
    pending_exit.plan = exit_leg;
    // SAFETY: the same fabricated custody re-drives with the exact leg.
    let resolution = unsafe {
        drive_uefi_os_handoff_cycle(&firmware, &mut handoff, arrival, pending_exit, buffer)
    }
    .unwrap();
    let UefiOsHandoffCycleResolution::Complete { completion, .. } = resolution else {
        panic!("the repaired custody must complete the handoff")
    };
    assert_eq!(OBSERVED_GET_CALLS.load(Ordering::SeqCst), 1);
    assert_eq!(*OBSERVED_EXIT_KEYS.lock().unwrap(), [0x5AFE_1009]);
    assert_eq!(completion.physical_invocation().normalized_identity(), 12);
}

#[test]
fn the_cycle_composes_edges_without_minting_evidence() {
    let production = include_str!("../os_handoff_cycle.rs")
        .split("#[cfg(test)]")
        .next()
        .expect("production cycle source");
    let compact = production
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect::<String>();
    for forbidden in [
        "for_test",
        "stale_map_key(",
        "::succeeded(",
        "UefiMemoryMapAcquisition{",
        "UefiExitBootServicesProviderResult{",
        "UefiOsHandoffMapRequired{",
        "UefiOsHandoffMapAcquired{",
    ] {
        assert!(
            !compact.contains(forbidden),
            "forbidden handoff evidence authority appeared: {forbidden}"
        );
    }
}

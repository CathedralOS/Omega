//! Handle protocol provider tests.

use super::{
    BoundUefiHandleProtocolInvocation, ExternalRootDiagnostic,
    LifecycleScopedUefiBootServicesProjection, MachineRegister, NonZeroU64, TargetProfile,
    UEFI_LOADED_IMAGE_PROTOCOL_GUID, UefiApplicationFirmwareLedger,
    UefiBootServicesTableOccurrenceId, UefiHandleProtocolExecutionStatus,
    UefiHandleProtocolInterfaceOutputSlot, UefiImageHandleOccurrenceId, UefiPhysicalInvocationId,
    UefiProtocolGuid, admit_uefi_loaded_image_handle_protocol_execution,
    bind_uefi_loaded_image_handle_protocol_invocation, execute_uefi_loaded_image_handle_protocol,
    join_lifecycle_scoped_uefi_handle_protocol_provider, plan_uefi_boot_services_native_layout,
    prepare_uefi_loaded_image_handle_protocol_invocation,
};
use crate::{
    UefiApplicationBootstrapLedgerId, UefiBootServicesPhaseLeaseId, UefiFirmwareSessionId,
    UefiSystemTableOccurrenceId, join_lifecycle_scoped_uefi_system_table,
    join_uefi_application_physical_arrival, prepare_uefi_application_bootstrap_adapter_invocation,
    project_uefi_application_boot_services,
};
use program_entry_plan::{
    ProgramEntryPhysicalContractPlan, UEFI_X64_IMAGE_HANDLE_TYPE_IDENTITY,
    UEFI_X64_PHYSICAL_REQUIREMENT_IDENTITY, UEFI_X64_STATUS_TYPE_IDENTITY,
    UEFI_X64_SYSTEM_TABLE_REFERENCE_TYPE_IDENTITY, UefiHandleProtocolStatus,
    exact_uefi_x64_physical_boundary_entry_plan,
    exact_uefi_x64_physical_contract_package_source_digest,
};
use std::ffi::c_void;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use target::{
    ProgramEntryPhysicalContractPackage, UEFI_BOOT_SERVICES_SIGNATURE,
    UEFI_LOADED_IMAGE_PROTOCOL_REVISION, UEFI_SYSTEM_TABLE_SIGNATURE,
    plan_uefi_system_table_native_layout, validate_uefi_boot_services_occurrence,
    validate_uefi_system_table_occurrence,
};

static FIRMWARE_TEST_LOCK: Mutex<()> = Mutex::new(());
static FAKE_STATUS: AtomicU64 = AtomicU64::new(0);
static FAKE_INTERFACE: AtomicUsize = AtomicUsize::new(0);
static OBSERVED_HANDLE: AtomicUsize = AtomicUsize::new(0);
static OBSERVED_PROTOCOL_DATA1: AtomicU64 = AtomicU64::new(0);
static OBSERVED_OUTPUT_SLOT: AtomicUsize = AtomicUsize::new(0);

unsafe extern "efiapi" fn fake_handle_protocol(
    handle: *mut c_void,
    protocol: *const UefiProtocolGuid,
    interface: *mut *mut c_void,
) -> u64 {
    OBSERVED_HANDLE.store(handle as usize, Ordering::SeqCst);
    OBSERVED_OUTPUT_SLOT.store(interface as usize, Ordering::SeqCst);
    // SAFETY: execution tests invoke the fake only with the exact static
    // GUID pointer retained by the bound carrier.
    let data1 = unsafe { (*protocol).data1 };
    OBSERVED_PROTOCOL_DATA1.store(u64::from(data1), Ordering::SeqCst);
    let output = FAKE_INTERFACE.load(Ordering::SeqCst) as *mut c_void;
    // SAFETY: the bound carrier passes its live opaque output slot.
    unsafe { *interface = output };
    FAKE_STATUS.load(Ordering::SeqCst)
}

#[repr(align(8))]
struct LoadedImageBytes([u8; 96]);

#[repr(align(8))]
struct MisalignedLoadedImageBytes([u8; 97]);

fn loaded_image_bytes(image_base: u64, image_size: u64) -> Box<LoadedImageBytes> {
    let mut bytes = Box::new(LoadedImageBytes([0; 96]));
    bytes.0[0..4].copy_from_slice(&UEFI_LOADED_IMAGE_PROTOCOL_REVISION.to_le_bytes());
    bytes.0[64..72].copy_from_slice(&image_base.to_le_bytes());
    bytes.0[72..80].copy_from_slice(&image_size.to_le_bytes());
    bytes
}

fn fake_handle_protocol_address() -> u64 {
    fake_handle_protocol as *const () as usize as u64
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

fn bound_invocation<'system_table, 'boot_services, 'output>(
    ledger: &mut UefiApplicationFirmwareLedger<'system_table>,
    system: &'system_table [u8],
    boot: &'boot_services [u8],
    base: u64,
    boot_address: u64,
    interface_output: &'output mut UefiHandleProtocolInterfaceOutputSlot,
) -> BoundUefiHandleProtocolInvocation<'system_table, 'boot_services, 'output> {
    let projection = projection(ledger, system, base);
    let integrity = validate_uefi_boot_services_occurrence(
        plan_uefi_boot_services_native_layout(TargetProfile::UefiX64).unwrap(),
        boot,
    )
    .unwrap();
    let provider = join_lifecycle_scoped_uefi_handle_protocol_provider(
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
    let invocation = prepare_uefi_loaded_image_handle_protocol_invocation(provider).unwrap();
    bind_uefi_loaded_image_handle_protocol_invocation(invocation, interface_output).unwrap()
}

#[test]
fn exact_handle_protocol_success_establishes_non_root_loaded_image_correspondence() {
    let _firmware = FIRMWARE_TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let boot_address = 0x401000;
    let system = table(UEFI_SYSTEM_TABLE_SIGNATURE, 120, 96, boot_address);
    let boot = table(
        UEFI_BOOT_SERVICES_SIGNATURE,
        376,
        152,
        fake_handle_protocol_address(),
    );
    let loaded_image = loaded_image_bytes(0x100000, 0x20000);
    FAKE_STATUS.store(0, Ordering::SeqCst);
    FAKE_INTERFACE.store(loaded_image.0.as_ptr() as usize, Ordering::SeqCst);
    let mut ledger = ledger(10);
    let mut interface_output = UefiHandleProtocolInterfaceOutputSlot::empty();
    let output_slot_address = (&raw mut interface_output.0) as usize;
    let invocation = bound_invocation(
        &mut ledger,
        &system,
        &boot,
        20,
        boot_address,
        &mut interface_output,
    );
    assert_eq!(
        invocation.argument_destinations(),
        [
            MachineRegister::X86Rcx,
            MachineRegister::X86Rdx,
            MachineRegister::X86R8,
        ]
    );
    assert_eq!(invocation._handle.get(), 0x1000_0014);
    assert_eq!(invocation._service.get(), fake_handle_protocol_address());
    assert!(std::ptr::eq(
        invocation.protocol,
        &UEFI_LOADED_IMAGE_PROTOCOL_GUID
    ));
    // SAFETY: the fake service and retained handle satisfy the executor's
    // test contract, and `loaded_image` remains live through admission.
    let execution = unsafe { execute_uefi_loaded_image_handle_protocol(invocation) }.unwrap();
    assert_eq!(
        execution.status(),
        UefiHandleProtocolExecutionStatus::Success
    );
    assert_eq!(execution.status_code(), 0);
    assert!(!execution.interface_output_is_null());
    assert_eq!(OBSERVED_HANDLE.load(Ordering::SeqCst), 0x1000_0014);
    assert_eq!(
        OBSERVED_PROTOCOL_DATA1.load(Ordering::SeqCst),
        u64::from(UEFI_LOADED_IMAGE_PROTOCOL_GUID.data1)
    );
    assert_eq!(
        OBSERVED_OUTPUT_SLOT.load(Ordering::SeqCst),
        output_slot_address
    );
    let loaded = admit_uefi_loaded_image_handle_protocol_execution(execution).unwrap();
    assert_eq!(
        (
            loaded.image_base(),
            loaded.image_size(),
            loaded.image_end_exclusive()
        ),
        (0x100000, 0x20000, 0x120000)
    );
    ledger
        .release_lifecycle_scoped_loaded_image_correspondence(loaded)
        .unwrap();
    ledger.begin_firmware_return().unwrap();
}

#[test]
fn concrete_operand_binding_rejects_address_free_stale_and_mismatched_inputs() {
    let boot_address = 0x411000;
    let system = table(UEFI_SYSTEM_TABLE_SIGNATURE, 120, 96, boot_address);
    let boot = table(UEFI_BOOT_SERVICES_SIGNATURE, 376, 152, 0x412000);
    let mut address_free_ledger = ledger(130);
    let address_free_projection =
        projection_with_image_handle(&mut address_free_ledger, &system, 140, false);
    let integrity = validate_uefi_boot_services_occurrence(
        plan_uefi_boot_services_native_layout(TargetProfile::UefiX64).unwrap(),
        &boot,
    )
    .unwrap();
    let provider = join_lifecycle_scoped_uefi_handle_protocol_provider(
        &address_free_ledger,
        address_free_projection,
        integrity,
        id(
            143,
            UefiBootServicesTableOccurrenceId::from_normalized_identity,
        ),
        NonZeroU64::new(boot_address).unwrap(),
    )
    .unwrap();
    let invocation = prepare_uefi_loaded_image_handle_protocol_invocation(provider).unwrap();
    let mut output = UefiHandleProtocolInterfaceOutputSlot::empty();
    let error =
        bind_uefi_loaded_image_handle_protocol_invocation(invocation, &mut output).unwrap_err();
    assert!(error.diagnostic().0.contains("physical image-handle value"));
    let (invocation, _, _) = error.into_parts();
    address_free_ledger
        .release_planned_uefi_handle_protocol_invocation(invocation)
        .unwrap();

    let system = table(UEFI_SYSTEM_TABLE_SIGNATURE, 120, 96, boot_address);
    let boot = table(UEFI_BOOT_SERVICES_SIGNATURE, 376, 152, 0x412000);
    let mut ledger = ledger(160);
    let projection = projection(&mut ledger, &system, 170);
    let integrity = validate_uefi_boot_services_occurrence(
        plan_uefi_boot_services_native_layout(TargetProfile::UefiX64).unwrap(),
        &boot,
    )
    .unwrap();
    let provider = join_lifecycle_scoped_uefi_handle_protocol_provider(
        &ledger,
        projection,
        integrity,
        id(
            173,
            UefiBootServicesTableOccurrenceId::from_normalized_identity,
        ),
        NonZeroU64::new(boot_address).unwrap(),
    )
    .unwrap();
    let invocation = prepare_uefi_loaded_image_handle_protocol_invocation(provider).unwrap();
    let mut stale_output = UefiHandleProtocolInterfaceOutputSlot(7_usize as *mut c_void);
    let error = bind_uefi_loaded_image_handle_protocol_invocation(invocation, &mut stale_output)
        .unwrap_err();
    assert!(error.diagnostic().0.contains("must be zero"));
    let (invocation, output, _) = error.into_parts();
    output.0 = std::ptr::null_mut();
    let invocation = bind_uefi_loaded_image_handle_protocol_invocation(invocation, output).unwrap();
    ledger
        .release_bound_uefi_handle_protocol_invocation(invocation)
        .unwrap();
}

#[test]
fn address_mismatch_and_null_service_reject_with_complete_custody() {
    let boot_address = 0x501000;
    let system = table(UEFI_SYSTEM_TABLE_SIGNATURE, 120, 96, boot_address);
    let boot = table(UEFI_BOOT_SERVICES_SIGNATURE, 376, 152, 0x502000);
    let mut owner = ledger(40);
    let projected = projection(&mut owner, &system, 50);
    let integrity = validate_uefi_boot_services_occurrence(
        plan_uefi_boot_services_native_layout(TargetProfile::UefiX64).unwrap(),
        &boot,
    )
    .unwrap();
    let error = join_lifecycle_scoped_uefi_handle_protocol_provider(
        &owner,
        projected,
        integrity,
        id(
            53,
            UefiBootServicesTableOccurrenceId::from_normalized_identity,
        ),
        NonZeroU64::new(boot_address + 8).unwrap(),
    )
    .unwrap_err();
    let (projected, integrity, occurrence, _, _) = error.into_parts();
    let provider = join_lifecycle_scoped_uefi_handle_protocol_provider(
        &owner,
        projected,
        integrity,
        occurrence,
        NonZeroU64::new(boot_address).unwrap(),
    )
    .unwrap();
    owner
        .release_lifecycle_scoped_handle_protocol_provider(provider)
        .unwrap();

    let system = table(UEFI_SYSTEM_TABLE_SIGNATURE, 120, 96, boot_address);
    let boot = table(UEFI_BOOT_SERVICES_SIGNATURE, 376, 152, 0);
    let mut ledger = ledger(70);
    let projection = projection(&mut ledger, &system, 80);
    let integrity = validate_uefi_boot_services_occurrence(
        plan_uefi_boot_services_native_layout(TargetProfile::UefiX64).unwrap(),
        &boot,
    )
    .unwrap();
    let error = join_lifecycle_scoped_uefi_handle_protocol_provider(
        &ledger,
        projection,
        integrity,
        id(
            83,
            UefiBootServicesTableOccurrenceId::from_normalized_identity,
        ),
        NonZeroU64::new(boot_address).unwrap(),
    )
    .unwrap_err();
    assert!(error.diagnostic().0.contains("pointer is null"));
    let (projection, _, _, _, _) = error.into_parts();
    ledger
        .release_lifecycle_scoped_boot_services_projection(projection)
        .unwrap();
}

#[test]
fn closed_and_unknown_statuses_return_complete_execution_custody() {
    let _firmware = FIRMWARE_TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let statuses = [
        (
            UefiHandleProtocolStatus::InvalidParameter,
            UefiHandleProtocolExecutionStatus::InvalidParameter,
        ),
        (
            UefiHandleProtocolStatus::Unsupported,
            UefiHandleProtocolExecutionStatus::Unsupported,
        ),
    ];
    for (index, (planned, expected)) in statuses.into_iter().enumerate() {
        let boot_address = 0x601000 + (index as u64 * 0x1000);
        let system = table(UEFI_SYSTEM_TABLE_SIGNATURE, 120, 96, boot_address);
        let boot = table(
            UEFI_BOOT_SERVICES_SIGNATURE,
            376,
            152,
            fake_handle_protocol_address(),
        );
        let mut ledger = ledger(300 + index as u64 * 20);
        let mut output = UefiHandleProtocolInterfaceOutputSlot::empty();
        let retained_error_output = loaded_image_bytes(0x100000, 0x20000);
        let invocation = bound_invocation(
            &mut ledger,
            &system,
            &boot,
            310 + index as u64 * 20,
            boot_address,
            &mut output,
        );
        let code = invocation.invocation.plan.status_code(planned);
        FAKE_STATUS.store(code, Ordering::SeqCst);
        FAKE_INTERFACE.store(retained_error_output.0.as_ptr() as usize, Ordering::SeqCst);
        // SAFETY: the exact test fake is the retained service and performs
        // no output dereference beyond the bound slot.
        let execution = unsafe { execute_uefi_loaded_image_handle_protocol(invocation) }.unwrap();
        let error = admit_uefi_loaded_image_handle_protocol_execution(execution).unwrap_err();
        assert_eq!(error.status(), expected);
        assert_eq!(error.status_code(), code);
        let (execution, _) = error.into_parts();
        assert!(!execution.interface_output_is_null());
        ledger
            .release_executed_uefi_handle_protocol_invocation(execution)
            .unwrap();
    }

    let boot_address = 0x604000;
    let system = table(UEFI_SYSTEM_TABLE_SIGNATURE, 120, 96, boot_address);
    let boot = table(
        UEFI_BOOT_SERVICES_SIGNATURE,
        376,
        152,
        fake_handle_protocol_address(),
    );
    let mut unknown_status_ledger = ledger(350);
    let mut output = UefiHandleProtocolInterfaceOutputSlot::empty();
    let invocation = bound_invocation(
        &mut unknown_status_ledger,
        &system,
        &boot,
        360,
        boot_address,
        &mut output,
    );
    FAKE_STATUS.store(0x1234, Ordering::SeqCst);
    FAKE_INTERFACE.store(0, Ordering::SeqCst);
    // SAFETY: the exact test fake is the retained service.
    let execution = unsafe { execute_uefi_loaded_image_handle_protocol(invocation) }.unwrap();
    let error = admit_uefi_loaded_image_handle_protocol_execution(execution).unwrap_err();
    assert_eq!(error.status(), UefiHandleProtocolExecutionStatus::Unknown);
    assert_eq!(error.status_code(), 0x1234);
    let (execution, _) = error.into_parts();
    unknown_status_ledger
        .release_executed_uefi_handle_protocol_invocation(execution)
        .unwrap();

    let boot_address = 0x706000;
    let system = table(UEFI_SYSTEM_TABLE_SIGNATURE, 120, 96, boot_address);
    let boot = table(
        UEFI_BOOT_SERVICES_SIGNATURE,
        376,
        152,
        fake_handle_protocol_address(),
    );
    let misaligned = MisalignedLoadedImageBytes([0; 97]);
    let mut alignment_ledger = ledger(500);
    let mut output = UefiHandleProtocolInterfaceOutputSlot::empty();
    let invocation = bound_invocation(
        &mut alignment_ledger,
        &system,
        &boot,
        510,
        boot_address,
        &mut output,
    );
    FAKE_STATUS.store(0, Ordering::SeqCst);
    FAKE_INTERFACE.store(
        // SAFETY: the 97-byte allocation leaves a readable 96-byte suffix.
        unsafe { misaligned.0.as_ptr().add(1) } as usize,
        Ordering::SeqCst,
    );
    // SAFETY: the deliberately misaligned interface remains readable for
    // the required 96-byte prefix and is rejected before target decoding.
    let execution = unsafe { execute_uefi_loaded_image_handle_protocol(invocation) }.unwrap();
    let error = admit_uefi_loaded_image_handle_protocol_execution(execution).unwrap_err();
    assert!(error.diagnostic().0.contains("not aligned"));
    let (execution, _) = error.into_parts();
    alignment_ledger
        .release_executed_uefi_handle_protocol_invocation(execution)
        .unwrap();
}

#[test]
fn null_stale_and_malformed_outputs_cannot_establish_correspondence() {
    let _firmware = FIRMWARE_TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    for (index, loaded_image) in [
        None,
        Some(loaded_image_bytes(0, 0x20000)),
        Some(loaded_image_bytes(u64::MAX - 1, 2)),
    ]
    .into_iter()
    .enumerate()
    {
        let boot_address = 0x701000 + index as u64 * 0x1000;
        let system = table(UEFI_SYSTEM_TABLE_SIGNATURE, 120, 96, boot_address);
        let boot = table(
            UEFI_BOOT_SERVICES_SIGNATURE,
            376,
            152,
            fake_handle_protocol_address(),
        );
        let mut ledger = ledger(400 + index as u64 * 20);
        let mut output = UefiHandleProtocolInterfaceOutputSlot::empty();
        let invocation = bound_invocation(
            &mut ledger,
            &system,
            &boot,
            410 + index as u64 * 20,
            boot_address,
            &mut output,
        );
        FAKE_STATUS.store(0, Ordering::SeqCst);
        FAKE_INTERFACE.store(
            loaded_image
                .as_ref()
                .map_or(0, |bytes| bytes.0.as_ptr() as usize),
            Ordering::SeqCst,
        );
        // SAFETY: non-null test buffers retain the exact readable prefix
        // for the duration of receipt admission.
        let execution = unsafe { execute_uefi_loaded_image_handle_protocol(invocation) }.unwrap();
        if index == 1 {
            execution.invocation.interface_output.0 = std::ptr::null_mut();
        }
        let error = admit_uefi_loaded_image_handle_protocol_execution(execution).unwrap_err();
        let diagnostic = &error.diagnostic().0;
        match index {
            0 => assert!(diagnostic.contains("null Loaded Image interface")),
            1 => assert!(diagnostic.contains("output slot drifted")),
            2 => assert!(diagnostic.contains("wraps")),
            _ => unreachable!(),
        }
        let (execution, _) = error.into_parts();
        ledger
            .release_executed_uefi_handle_protocol_invocation(execution)
            .unwrap();
    }

    let boot_address = 0x705000;
    let system = table(UEFI_SYSTEM_TABLE_SIGNATURE, 120, 96, boot_address);
    let boot = table(
        UEFI_BOOT_SERVICES_SIGNATURE,
        376,
        152,
        fake_handle_protocol_address(),
    );
    let mut bad_revision = loaded_image_bytes(0x100000, 0x20000);
    bad_revision.0[0..4].copy_from_slice(&0_u32.to_le_bytes());
    let mut ledger = ledger(470);
    let mut output = UefiHandleProtocolInterfaceOutputSlot::empty();
    let invocation = bound_invocation(&mut ledger, &system, &boot, 480, boot_address, &mut output);
    FAKE_STATUS.store(0, Ordering::SeqCst);
    FAKE_INTERFACE.store(bad_revision.0.as_ptr() as usize, Ordering::SeqCst);
    // SAFETY: `bad_revision` retains a readable exact-size layout buffer.
    let execution = unsafe { execute_uefi_loaded_image_handle_protocol(invocation) }.unwrap();
    let error = admit_uefi_loaded_image_handle_protocol_execution(execution).unwrap_err();
    assert!(error.diagnostic().0.contains("revision"));
    let (execution, _) = error.into_parts();
    ledger
        .release_executed_uefi_handle_protocol_invocation(execution)
        .unwrap();
}

#[test]
fn provider_public_surface_has_no_raw_function_or_authority_projection() {
    let source = include_str!("../handle_protocol_provider.rs")
        .split("#[cfg(test)]")
        .next()
        .unwrap();
    let compact: String = source
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect();
    for forbidden in [
        "pubfnhandle_protocol",
        "pubfnfunction",
        "pubfnbytes",
        "psi_extents::Extent",
        "NativeExecution",
        "PhysicalShell",
        "UefiHandleProtocolLoadedImageOutcome",
        "admit_uefi_loaded_image_handle_protocol_outcome",
        "pubfninterface_address",
        "pubfnservice_address",
        "pubfnhandle_address",
    ] {
        assert!(
            !compact.contains(forbidden),
            "forbidden provider API appeared: {forbidden}"
        );
    }
    assert!(!compact.contains("implCloneforLifecycleScopedUefiHandleProtocolProvider"));
    assert!(!compact.contains("implCloneforLifecycleScopedUefiLoadedImageCorrespondence"));
    assert!(!compact.contains("implCloneforPlannedUefiHandleProtocolInvocation"));
    assert!(!compact.contains("implCloneforBoundUefiHandleProtocolInvocation"));
    let execution_source = include_str!("../handle_protocol_provider/execution.rs");
    let execution_compact: String = execution_source
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect();
    assert!(!execution_compact.contains("implCloneforUefiHandleProtocolInterfaceOutputSlot"));
    assert!(!execution_compact.contains("implCloneforExecutedUefiHandleProtocolInvocation"));
    assert!(execution_compact.contains(
        "admit_uefi_loaded_image_handle_protocol_execution<'system_table,'boot_services,'output>(execution:ExecutedUefiHandleProtocolInvocation"
    ));
}

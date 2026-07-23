use super::{TargetOperationDomain, TargetOperationKind};
use crate::{
    HostOperationKey, RuntimeStorageRegion, RuntimeTextReadSource, StateGuardLowering,
    StateGuardOperator, TargetDataObjectHandle,
};
use omega_core::arena::HandleSpan;

#[test]
fn operation_kinds_expose_host_boundary_domain() {
    let operation = TargetOperationKind::HostOperation {
        operation_key: HostOperationKey::default(),
        operands: HandleSpan::empty(),
    };

    assert_eq!(
        operation.semantic_domain(),
        TargetOperationDomain::HostBoundary
    );
    assert!(operation.crosses_host_boundary());
    assert!(!operation.touches_runtime_storage());
}

#[test]
fn operation_kinds_expose_runtime_storage_domains() {
    let guard = TargetOperationKind::EvaluateDispatchGuard {
        guard_lowering: StateGuardLowering::CompareRuntimeValue,
        operator: StateGuardOperator::Equal,
        storage_region: RuntimeStorageRegion::Machine,
        byte_offset: 0,
        byte_size: 8,
        expected_value: 1,
        has_storage: true,
        is_float: false,
    };
    let copy = TargetOperationKind::CopyPlaces {
        source: crate::Place::at(RuntimeStorageRegion::Machine, 0),
        target: crate::Place::at(RuntimeStorageRegion::RuntimeFrame, 8),
        byte_count: 8,
    };
    let read = TargetOperationKind::ReadRuntimeTextLine {
        buffer: TargetDataObjectHandle::invalid(),
        target_region: RuntimeStorageRegion::RuntimeFrame,
        target_offset: 0,
        byte_capacity: 64,
        is_bounded_buffer: false,
        source: RuntimeTextReadSource::HostOperation {
            operation_key: HostOperationKey::default(),
        },
    };

    assert_eq!(
        guard.semantic_domain(),
        TargetOperationDomain::GuardEvaluation
    );
    assert_eq!(copy.semantic_domain(), TargetOperationDomain::RuntimeCopy);
    assert_eq!(read.semantic_domain(), TargetOperationDomain::RuntimeRead);
    assert!(guard.touches_runtime_storage());
    assert!(copy.touches_runtime_storage());
    assert!(read.touches_runtime_storage());
    assert!(read.crosses_host_boundary());
}

#[test]
fn operation_kinds_expose_control_domains() {
    assert_eq!(
        TargetOperationKind::EnterFunction.semantic_domain(),
        TargetOperationDomain::FunctionBoundary
    );
    assert_eq!(
        TargetOperationKind::SetDispatchState { dispatch_index: 3 }.semantic_domain(),
        TargetOperationDomain::DispatchControl
    );
    let idt_load = TargetOperationKind::GeneratedIdtLoad {
        pointer_register: omega_calling_conventions::MachineRegister::X86Rcx,
        materialized: omega_external_roots::MaterializedIdtId::from_normalized_identity(1)
            .expect("materialized IDT identity"),
        descriptor: omega_external_roots::IdtDestinationId::from_normalized_identity(2)
            .expect("IDT destination identity"),
        descriptor_fingerprint: 6,
        content_fingerprint: 3,
        root_ledger_fingerprint: 4,
        control: omega_external_roots::IdtControlId::from_normalized_identity(5)
            .expect("IDT control identity"),
    };
    assert_eq!(
        idt_load.semantic_domain(),
        TargetOperationDomain::MachineControl
    );
    assert!(!idt_load.crosses_host_boundary());
    assert!(!idt_load.touches_runtime_storage());
}

#[test]
fn generated_idt_writer_is_an_address_free_runtime_write() {
    let writer = TargetOperationKind::GeneratedIdtWriter {
        pointer_register: omega_calling_conventions::MachineRegister::X86Rdi,
        context: omega_external_roots::IdtWriterContextId::from_normalized_identity(9)
            .expect("writer context identity"),
        preparation: omega_external_roots::IdtWriterPreparationId::from_normalized_identity(1)
            .expect("writer preparation identity"),
        installed_code: omega_external_roots::InstalledCodeId::from_normalized_identity(2)
            .expect("installed code identity"),
        artifact: omega_external_roots::ArtifactId::from_normalized_identity(3)
            .expect("artifact identity"),
        destination: omega_external_roots::IdtDestinationId::from_normalized_identity(4)
            .expect("destination identity"),
        writer_fingerprint: 5,
        placement_fingerprint: 6,
        initial_content_fingerprint: 7,
        root_binding_fingerprint: 8,
        byte_len: 16,
        little_endian: true,
        context_abi: crate::GENERATED_IDT_WRITER_CONTEXT_ABI_V1,
        context_fingerprint: 10,
        source_slot_count: 1,
        steps: vec![crate::GeneratedIdtWriterStep {
            container_byte_offset: 0,
            container_width_bits: 64,
            destination_lsb: 0,
            source_lsb: 0,
            width: 64,
            source_slot: 0,
        }]
        .into(),
    };
    assert_eq!(
        writer.semantic_domain(),
        TargetOperationDomain::RuntimeWrite
    );
    assert!(!writer.crosses_host_boundary());
    assert!(writer.touches_runtime_storage());
}

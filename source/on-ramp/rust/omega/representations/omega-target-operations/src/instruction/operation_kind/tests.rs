use super::{TargetOperationDomain, TargetOperationKind};
use crate::{
    HostOperationKey, RuntimeStorageRegion, RuntimeTextReadSource, StateGuardLowering,
    StateGuardOperator, TargetDataObjectHandle,
};
use psi_arena::HandleSpan;

#[test]
fn operation_kinds_expose_host_boundary_domain() {
    let operation = TargetOperationKind::HostOperation {
        operation_key: HostOperationKey::default(),
        operands: HandleSpan::empty(),
        provenance: None,
    };

    assert_eq!(
        operation.semantic_domain(),
        TargetOperationDomain::HostBoundary
    );
    assert!(operation.crosses_host_boundary());
    assert!(!operation.touches_runtime_storage());
    assert_eq!(
        operation.host_operation_key(),
        Some(HostOperationKey::default())
    );
}

#[test]
fn outgoing_stack_address_is_compiler_function_boundary_mechanics() {
    for operation in [
        TargetOperationKind::ReserveOutgoingStackFrame { byte_count: 72 },
        TargetOperationKind::WriteOutgoingStackU64 {
            stack_byte_offset: 32,
            value: 1,
        },
        TargetOperationKind::CopyEntryIndirectU64ToOutgoingStack {
            source_register: omega_calling_conventions::MachineRegister::X86Rcx,
            source_byte_offset: 0,
            stack_byte_offset: 32,
        },
        TargetOperationKind::LoadOutgoingStackAddress {
            register: omega_calling_conventions::MachineRegister::X86Rdx,
            stack_byte_offset: 48,
        },
        TargetOperationKind::ReleaseOutgoingStackFrame { byte_count: 72 },
    ] {
        assert_eq!(
            operation.semantic_domain(),
            TargetOperationDomain::FunctionBoundary
        );
        assert!(!operation.crosses_host_boundary());
        assert!(!operation.touches_runtime_storage());
    }
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
        role: omega_abstract_operations::CopyPlacesRole::Ordinary,
    };
    let read = TargetOperationKind::ReadRuntimeTextLine {
        buffer: TargetDataObjectHandle::invalid(),
        target_region: RuntimeStorageRegion::RuntimeFrame,
        target_offset: 0,
        byte_capacity: 64,
        target: crate::RuntimeTextReadTarget::StringDescriptor,
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
    assert_eq!(read.host_operation_key(), Some(HostOperationKey::default()));
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
}

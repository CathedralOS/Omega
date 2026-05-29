use super::{AssignedOperationDomain, AssignedOperationKind};
use crate::TargetValueOperandHandle;
use omega_core::arena::HandleSpan;
use omega_target_operations::{
    HostOperationKey, RuntimeStorageRegion, RuntimeTextReadSource, StateGuardLowering,
    StateGuardOperator, TargetDataObjectHandle,
};

#[test]
fn operation_kinds_expose_host_boundary_domain() {
    let operation = AssignedOperationKind::HostOperation {
        operation_key: HostOperationKey::default(),
        operands: HandleSpan::empty(),
    };

    assert_eq!(
        operation.semantic_domain(),
        AssignedOperationDomain::HostBoundary
    );
    assert!(operation.crosses_host_boundary());
    assert!(!operation.touches_runtime_storage());
}

#[test]
fn operation_kinds_expose_runtime_storage_domains() {
    let guard = AssignedOperationKind::EvaluateDispatchGuard {
        guard_lowering: StateGuardLowering::CompareRuntimeValue,
        operator: StateGuardOperator::Equal,
        storage_region: RuntimeStorageRegion::Machine,
        byte_offset: 0,
        byte_size: 8,
        expected_value: 1,
        has_storage: true,
    };
    let copy = AssignedOperationKind::CopyRuntimeStorage {
        source_region: RuntimeStorageRegion::Machine,
        source_offset: 0,
        target_region: RuntimeStorageRegion::RuntimeFrame,
        target_offset: 8,
        byte_count: 8,
    };
    let read = AssignedOperationKind::ReadRuntimeTextLine {
        buffer: TargetDataObjectHandle::invalid(),
        target_region: RuntimeStorageRegion::RuntimeFrame,
        target_offset: 0,
        byte_capacity: 64,
        source: RuntimeTextReadSource::HostOperation {
            operation_key: HostOperationKey::default(),
        },
    };

    assert_eq!(
        guard.semantic_domain(),
        AssignedOperationDomain::GuardEvaluation
    );
    assert_eq!(copy.semantic_domain(), AssignedOperationDomain::RuntimeCopy);
    assert_eq!(read.semantic_domain(), AssignedOperationDomain::RuntimeRead);
    assert!(guard.touches_runtime_storage());
    assert!(copy.touches_runtime_storage());
    assert!(read.touches_runtime_storage());
    assert!(read.crosses_host_boundary());
}

#[test]
fn operation_kinds_expose_control_domains() {
    assert_eq!(
        AssignedOperationKind::EnterFunction.semantic_domain(),
        AssignedOperationDomain::FunctionBoundary
    );
    assert_eq!(
        AssignedOperationKind::SetDispatchState { dispatch_index: 3 }.semantic_domain(),
        AssignedOperationDomain::DispatchControl
    );
}

#[test]
fn operation_kinds_accept_assigned_runtime_value_handles() {
    let operation = AssignedOperationKind::CompareRuntimeValues {
        left: TargetValueOperandHandle::invalid(),
        right: TargetValueOperandHandle::invalid(),
        byte_size: 8,
        operator: StateGuardOperator::Equal,
    };

    assert_eq!(
        operation.semantic_domain(),
        AssignedOperationDomain::GuardEvaluation
    );
}

#[test]
fn operation_kind_conversion_preserves_semantic_queries() {
    let target_read = omega_target_operations::TargetOperationKind::ReadRuntimeTextLine {
        buffer: TargetDataObjectHandle::invalid(),
        target_region: RuntimeStorageRegion::RuntimeFrame,
        target_offset: 0,
        byte_capacity: 64,
        source: RuntimeTextReadSource::HostOperation {
            operation_key: HostOperationKey::default(),
        },
    };
    let assigned_read = AssignedOperationKind::from(target_read.clone());

    assert_eq!(
        assigned_read.semantic_domain(),
        target_read.semantic_domain()
    );
    assert_eq!(
        assigned_read.crosses_host_boundary(),
        target_read.crosses_host_boundary()
    );
    assert_eq!(
        assigned_read.touches_runtime_storage(),
        target_read.touches_runtime_storage()
    );

    let assigned_copy = AssignedOperationKind::CopyRuntimeStorage {
        source_region: RuntimeStorageRegion::Machine,
        source_offset: 0,
        target_region: RuntimeStorageRegion::RuntimeFrame,
        target_offset: 8,
        byte_count: 8,
    };
    let target_copy = omega_target_operations::TargetOperationKind::from(assigned_copy.clone());

    assert_eq!(
        target_copy.semantic_domain(),
        assigned_copy.semantic_domain()
    );
    assert_eq!(
        target_copy.crosses_host_boundary(),
        assigned_copy.crosses_host_boundary()
    );
    assert_eq!(
        target_copy.touches_runtime_storage(),
        assigned_copy.touches_runtime_storage()
    );
}

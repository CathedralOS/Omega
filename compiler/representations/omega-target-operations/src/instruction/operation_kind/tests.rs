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
    let copy = TargetOperationKind::CopyRuntimeStorage {
        source_region: RuntimeStorageRegion::Machine,
        source_offset: 0,
        target_region: RuntimeStorageRegion::RuntimeFrame,
        target_offset: 8,
        byte_count: 8,
    };
    let read = TargetOperationKind::ReadRuntimeTextLine {
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
}

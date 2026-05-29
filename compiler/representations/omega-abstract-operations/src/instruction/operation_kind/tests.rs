use super::{AbstractOperationDomain, AbstractOperationKind};
use crate::{
    AbstractDataObjectHandle, RuntimeStorageRegion, StateGuardLowering, StateGuardOperator,
};
use omega_core::arena::HandleSpan;

#[test]
fn operation_kinds_expose_host_boundary_domain() {
    let operation = AbstractOperationKind::HostOperation {
        operation_ordinal: 0,
        operands: HandleSpan::empty(),
    };

    assert_eq!(
        operation.semantic_domain(),
        AbstractOperationDomain::HostBoundary
    );
    assert!(operation.crosses_host_boundary());
    assert!(!operation.touches_runtime_storage());
}

#[test]
fn operation_kinds_expose_runtime_storage_domains() {
    let guard = AbstractOperationKind::EvaluateDispatchGuard {
        guard_lowering: StateGuardLowering::CompareRuntimeValue,
        operator: StateGuardOperator::Equal,
        storage_region: RuntimeStorageRegion::Machine,
        byte_offset: 0,
        byte_size: 8,
        expected_value: 1,
        has_storage: true,
    };
    let copy = AbstractOperationKind::CopyRuntimeStorage {
        source_region: RuntimeStorageRegion::Machine,
        source_offset: 0,
        target_region: RuntimeStorageRegion::RuntimeFrame,
        target_offset: 8,
        byte_count: 8,
    };
    let read = AbstractOperationKind::ReadRuntimeTextLine {
        buffer: AbstractDataObjectHandle::invalid(),
        target_region: RuntimeStorageRegion::RuntimeFrame,
        target_offset: 0,
        byte_capacity: 64,
    };

    assert_eq!(
        guard.semantic_domain(),
        AbstractOperationDomain::GuardEvaluation
    );
    assert_eq!(copy.semantic_domain(), AbstractOperationDomain::RuntimeCopy);
    assert_eq!(read.semantic_domain(), AbstractOperationDomain::RuntimeRead);
    assert!(guard.touches_runtime_storage());
    assert!(copy.touches_runtime_storage());
    assert!(read.touches_runtime_storage());
}

#[test]
fn operation_kinds_expose_control_domains() {
    assert_eq!(
        AbstractOperationKind::EnterFunction.semantic_domain(),
        AbstractOperationDomain::FunctionBoundary
    );
    assert_eq!(
        AbstractOperationKind::SetDispatchState { dispatch_index: 3 }.semantic_domain(),
        AbstractOperationDomain::DispatchControl
    );
}

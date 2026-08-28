use super::{AbstractOperationDomain, AbstractOperationKind};
use crate::{
    AbstractDataObjectHandle, RuntimeStorageRegion, StateGuardLowering, StateGuardOperator,
};
use psi_arena::HandleSpan;

#[test]
fn operation_kinds_expose_host_boundary_domain() {
    let operation = AbstractOperationKind::HostOperation {
        operation_ordinal: 0,
        operands: HandleSpan::empty(),
        provenance: None,
    };

    assert_eq!(
        operation.semantic_domain(),
        AbstractOperationDomain::HostBoundary
    );
    assert!(operation.crosses_host_boundary());
    assert!(!operation.touches_runtime_storage());
}

#[test]
fn outgoing_stack_address_is_compiler_function_boundary_mechanics() {
    for operation in [
        AbstractOperationKind::ReserveOutgoingStackFrame { byte_count: 72 },
        AbstractOperationKind::WriteOutgoingStackU64 {
            stack_byte_offset: 32,
            value: 1,
        },
        AbstractOperationKind::CopyEntryIndirectU64ToOutgoingStack {
            source_register: omega_calling_conventions::MachineRegister::X86Rcx,
            source_byte_offset: 0,
            stack_byte_offset: 32,
        },
        AbstractOperationKind::LoadOutgoingStackAddress {
            register: omega_calling_conventions::MachineRegister::X86Rcx,
            stack_byte_offset: 32,
        },
        AbstractOperationKind::ReleaseOutgoingStackFrame { byte_count: 72 },
    ] {
        assert_eq!(
            operation.semantic_domain(),
            AbstractOperationDomain::FunctionBoundary
        );
        assert!(!operation.crosses_host_boundary());
        assert!(!operation.touches_runtime_storage());
    }
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
        is_float: false,
    };
    let copy = AbstractOperationKind::CopyPlaces {
        source: crate::Place::at(RuntimeStorageRegion::Machine, 0),
        target: crate::Place::at(RuntimeStorageRegion::RuntimeFrame, 8),
        byte_count: 8,
        role: super::CopyPlacesRole::Ordinary,
    };
    let read = AbstractOperationKind::ReadRuntimeTextLine {
        buffer: AbstractDataObjectHandle::invalid(),
        target_region: RuntimeStorageRegion::RuntimeFrame,
        target_offset: 0,
        byte_capacity: 64,
        target: crate::RuntimeTextReadTarget::StringDescriptor,
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
    assert!(read.crosses_host_boundary());
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

use omega_calling_conventions::HostOperationKey;
use omega_target_operations::{InstructionOperand, RuntimeTextReadSource, SelectedInstructionKind};
use psi_arena::HandleSpan;

pub(super) fn host_read_operation_key(kind: &SelectedInstructionKind) -> Option<HostOperationKey> {
    if !kind.crosses_host_boundary() {
        return None;
    }

    let SelectedInstructionKind::ReadRuntimeTextLine {
        source: RuntimeTextReadSource::HostOperation { operation_key },
        ..
    } = kind
    else {
        return None;
    };

    Some(*operation_key)
}

pub(super) fn host_operation(
    kind: &SelectedInstructionKind,
) -> Option<(HostOperationKey, HandleSpan<InstructionOperand>)> {
    if !kind.crosses_host_boundary() {
        return None;
    }

    let SelectedInstructionKind::HostOperation {
        operation_key,
        operands,
        ..
    } = kind
    else {
        return None;
    };

    Some((*operation_key, *operands))
}

pub(super) fn host_operation_operands(
    kind: &SelectedInstructionKind,
) -> Option<HandleSpan<InstructionOperand>> {
    host_operation(kind).map(|(_, operands)| operands)
}

#[cfg(test)]
mod tests {
    use super::{host_operation, host_operation_operands, host_read_operation_key};
    use omega_calling_conventions::HostOperationKey;
    use omega_target_operations::{
        RuntimeStorageRegion, RuntimeTextReadSource, RuntimeTextReadTarget,
        SelectedInstructionKind, TargetDataObjectHandle,
    };
    use psi_arena::HandleSpan;

    #[test]
    fn host_read_query_recognizes_host_backed_runtime_text_reads() {
        let operation_key = HostOperationKey::default();
        let kind = SelectedInstructionKind::ReadRuntimeTextLine {
            buffer: TargetDataObjectHandle::invalid(),
            target_region: RuntimeStorageRegion::RuntimeFrame,
            target_offset: 0,
            byte_capacity: 64,
            target: RuntimeTextReadTarget::StringDescriptor,
            source: RuntimeTextReadSource::HostOperation { operation_key },
        };

        assert_eq!(host_read_operation_key(&kind), Some(operation_key));
        assert!(host_operation(&kind).is_none());
    }

    #[test]
    fn host_operation_query_recognizes_selected_host_operations() {
        let operation_key = HostOperationKey::default();
        let kind = SelectedInstructionKind::HostOperation {
            operation_key,
            operands: HandleSpan::empty(),
            provenance: None,
        };

        assert_eq!(
            host_operation(&kind),
            Some((operation_key, HandleSpan::empty()))
        );
        assert_eq!(host_operation_operands(&kind), Some(HandleSpan::empty()));
        assert!(host_read_operation_key(&kind).is_none());
    }

    #[test]
    fn boundary_markers_without_payload_do_not_look_like_host_calls() {
        let kind = SelectedInstructionKind::BeginPlatformCall;

        assert!(kind.crosses_host_boundary());
        assert!(host_read_operation_key(&kind).is_none());
        assert!(host_operation(&kind).is_none());
    }
}

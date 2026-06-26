use omega_assigned_target_operations::{RuntimeTextReadSource, SelectedInstructionKind};
use omega_calling_conventions::HostOperationKey;
use omega_core::arena::HandleSpan;
use omega_target_operations::InstructionOperand;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SelectedHostOperation {
    pub operation_key: HostOperationKey,
    pub operands: HandleSpan<InstructionOperand>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SelectedHostTextRead<'instruction> {
    pub target_offset: usize,
    pub byte_capacity: usize,
    pub source: &'instruction RuntimeTextReadSource,
    pub operation_key: HostOperationKey,
    /// Owned `[u8; N]` carrier target: read into its inline bytes + write only len.
    pub is_bounded_buffer: bool,
}

pub(crate) fn selected_host_operation(
    kind: &SelectedInstructionKind,
) -> Option<SelectedHostOperation> {
    if !kind.crosses_host_boundary() {
        return None;
    }

    let SelectedInstructionKind::HostOperation {
        operation_key,
        operands,
    } = kind
    else {
        return None;
    };

    Some(SelectedHostOperation {
        operation_key: *operation_key,
        operands: *operands,
    })
}

pub(crate) fn selected_host_text_read(
    kind: &SelectedInstructionKind,
) -> Option<SelectedHostTextRead<'_>> {
    if !kind.crosses_host_boundary() {
        return None;
    }

    let SelectedInstructionKind::ReadRuntimeTextLine {
        target_offset,
        byte_capacity,
        source,
        is_bounded_buffer,
        ..
    } = kind
    else {
        return None;
    };
    let RuntimeTextReadSource::HostOperation { operation_key } = source;

    Some(SelectedHostTextRead {
        target_offset: *target_offset,
        byte_capacity: *byte_capacity,
        source,
        operation_key: *operation_key,
        is_bounded_buffer: *is_bounded_buffer,
    })
}

#[cfg(test)]
mod tests {
    use super::{selected_host_operation, selected_host_text_read};
    use omega_assigned_target_operations::{
        RuntimeStorageRegion, RuntimeTextReadSource, SelectedInstructionKind,
    };
    use omega_calling_conventions::HostOperationKey;
    use omega_core::arena::HandleSpan;
    use omega_target_operations::TargetDataObjectHandle;

    #[test]
    fn selected_host_text_read_extracts_encoding_payload() {
        let operation_key = HostOperationKey::default();
        let kind = SelectedInstructionKind::ReadRuntimeTextLine {
            buffer: TargetDataObjectHandle::invalid(),
            target_region: RuntimeStorageRegion::RuntimeFrame,
            target_offset: 8,
            byte_capacity: 64,
            source: RuntimeTextReadSource::HostOperation { operation_key },
        };

        let read = selected_host_text_read(&kind).expect("host text read");
        assert_eq!(read.target_offset, 8);
        assert_eq!(read.byte_capacity, 64);
        assert_eq!(read.operation_key, operation_key);
        assert!(selected_host_operation(&kind).is_none());
    }

    #[test]
    fn selected_host_operation_extracts_encoding_payload() {
        let operation_key = HostOperationKey::default();
        let kind = SelectedInstructionKind::HostOperation {
            operation_key,
            operands: HandleSpan::empty(),
        };

        let operation = selected_host_operation(&kind).expect("host operation");
        assert_eq!(operation.operation_key, operation_key);
        assert_eq!(operation.operands, HandleSpan::empty());
        assert!(selected_host_text_read(&kind).is_none());
    }

    #[test]
    fn boundary_marker_without_payload_is_not_selected_host_payload() {
        let kind = SelectedInstructionKind::BeginPlatformCall;

        assert!(kind.crosses_host_boundary());
        assert!(selected_host_operation(&kind).is_none());
        assert!(selected_host_text_read(&kind).is_none());
    }
}

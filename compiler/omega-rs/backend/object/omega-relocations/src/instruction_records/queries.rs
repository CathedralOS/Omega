use omega_calling_conventions::HostOperationKey;
use omega_core::arena::HandleSpan;
use omega_target_operations::{
    InstructionOperand, RuntimeStorageRegion, RuntimeTextReadSource, SelectedInstructionKind,
    TargetDataObjectHandle,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct SelectedHostTextRead {
    pub buffer: TargetDataObjectHandle,
    pub target_region: RuntimeStorageRegion,
    pub operation_key: HostOperationKey,
    /// Owned `[u8; N]` carrier target: r14 relocates to the carrier's own region
    /// (not a separate buffer) and there is no `{ptr, len}` descriptor write.
    pub is_bounded_buffer: bool,
}

pub(super) fn selected_host_operation(
    instruction: &SelectedInstructionKind,
) -> Option<(HostOperationKey, HandleSpan<InstructionOperand>)> {
    if !instruction.crosses_host_boundary() {
        return None;
    }

    let SelectedInstructionKind::HostOperation {
        operation_key,
        operands,
    } = instruction
    else {
        return None;
    };

    Some((*operation_key, *operands))
}

pub(super) fn selected_host_text_read(
    instruction: &SelectedInstructionKind,
) -> Option<SelectedHostTextRead> {
    if !instruction.crosses_host_boundary() {
        return None;
    }

    let SelectedInstructionKind::ReadRuntimeTextLine {
        buffer,
        target_region,
        source: RuntimeTextReadSource::HostOperation { operation_key },
        is_bounded_buffer,
        ..
    } = instruction
    else {
        return None;
    };

    Some(SelectedHostTextRead {
        buffer: *buffer,
        target_region: *target_region,
        operation_key: *operation_key,
        is_bounded_buffer: *is_bounded_buffer,
    })
}

#[cfg(test)]
mod tests {
    use super::{selected_host_operation, selected_host_text_read};
    use omega_calling_conventions::HostOperationKey;
    use omega_core::arena::HandleSpan;
    use omega_target_operations::{
        RuntimeStorageRegion, RuntimeTextReadSource, SelectedInstructionKind,
        TargetDataObjectHandle,
    };

    #[test]
    fn selected_host_text_read_extracts_host_payload() {
        let operation_key = HostOperationKey::default();
        let instruction = SelectedInstructionKind::ReadRuntimeTextLine {
            buffer: TargetDataObjectHandle::invalid(),
            target_region: RuntimeStorageRegion::RuntimeFrame,
            target_offset: 0,
            byte_capacity: 64,
            is_bounded_buffer: false,
            source: RuntimeTextReadSource::HostOperation { operation_key },
        };

        let read = selected_host_text_read(&instruction).expect("host text read");
        assert_eq!(read.operation_key, operation_key);
        assert_eq!(read.buffer, TargetDataObjectHandle::invalid());
        assert_eq!(read.target_region, RuntimeStorageRegion::RuntimeFrame);
        assert!(selected_host_operation(&instruction).is_none());
    }

    #[test]
    fn selected_host_operation_extracts_host_payload() {
        let operation_key = HostOperationKey::default();
        let instruction = SelectedInstructionKind::HostOperation {
            operation_key,
            operands: HandleSpan::empty(),
        };

        assert_eq!(
            selected_host_operation(&instruction),
            Some((operation_key, HandleSpan::empty()))
        );
        assert!(selected_host_text_read(&instruction).is_none());
    }
}

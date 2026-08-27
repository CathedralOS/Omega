use omega_calling_conventions::HostOperationKey;
use omega_target_operations::{
    InstructionOperand, RuntimeStorageRegion, RuntimeTextReadSource, RuntimeTextReadTarget,
    SelectedInstructionKind, TargetDataObjectHandle,
};
use psi_arena::HandleSpan;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct SelectedHostTextRead {
    pub buffer: TargetDataObjectHandle,
    pub target_region: RuntimeStorageRegion,
    pub target_offset: usize,
    pub operation_key: HostOperationKey,
    pub target: RuntimeTextReadTarget,
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
        ..
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
        target_offset,
        source: RuntimeTextReadSource::HostOperation { operation_key },
        target,
        ..
    } = instruction
    else {
        return None;
    };

    Some(SelectedHostTextRead {
        buffer: *buffer,
        target_region: *target_region,
        target_offset: *target_offset,
        operation_key: *operation_key,
        target: *target,
    })
}

#[cfg(test)]
mod tests {
    use super::{selected_host_operation, selected_host_text_read};
    use omega_calling_conventions::HostOperationKey;
    use omega_target_operations::{
        RuntimeStorageRegion, RuntimeTextReadSource, RuntimeTextReadTarget,
        SelectedInstructionKind, TargetDataObjectHandle,
    };
    use psi_arena::HandleSpan;

    #[test]
    fn selected_host_text_read_extracts_host_payload() {
        let operation_key = HostOperationKey::default();
        let instruction = SelectedInstructionKind::ReadRuntimeTextLine {
            buffer: TargetDataObjectHandle::invalid(),
            target_region: RuntimeStorageRegion::RuntimeFrame,
            target_offset: 0,
            byte_capacity: 64,
            target: RuntimeTextReadTarget::StringDescriptor,
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
            provenance: None,
        };

        assert_eq!(
            selected_host_operation(&instruction),
            Some((operation_key, HandleSpan::empty()))
        );
        assert!(selected_host_text_read(&instruction).is_none());
    }
}

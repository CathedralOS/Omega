use crate::{
    OptimizedProgramStorageSemanticWrapperContinuationDisposition,
    OptimizedProgramStorageSemanticWrapperEncodingDisposition,
    OptimizedProgramStorageSemanticWrapperPlan,
    OptimizedProgramStorageSemanticWrapperRelocationKind,
    OptimizedProgramStorageSemanticWrapperStep,
};
use calling_conventions::MachineRegister;
use isa_x86_64::{
    X86_64SemanticUnitWrapperArgumentBinding, X86_64SemanticUnitWrapperCopy,
    X86_64SemanticUnitWrapperEncodingPolicy, X86_64SemanticUnitWrapperEncodingRequest,
    X86_64SemanticUnitWrapperReceiverSlot,
};

use super::OptimizedProgramStorageSemanticWrapperEncodingError;

pub(super) fn project_request(
    source: &OptimizedProgramStorageSemanticWrapperPlan,
) -> Result<
    X86_64SemanticUnitWrapperEncodingRequest,
    OptimizedProgramStorageSemanticWrapperEncodingError,
> {
    use OptimizedProgramStorageSemanticWrapperStep as Step;
    let receiver = source.receiver();
    let mismatch =
        || OptimizedProgramStorageSemanticWrapperEncodingError::SemanticStepShapeMismatch;
    // The incoming boundary plan never carries a receiver, so both shapes
    // start with the same prologue and the same four Extent copies; only the
    // provisioned residence and the shifted outgoing binds diverge.
    let [
        Step::EnterFunction,
        Step::ReserveOutgoingStackFrame {
            byte_count: reserve,
        },
        first_copy,
        second_copy,
        third_copy,
        fourth_copy,
        tail @ ..,
    ] = source.steps()
    else {
        return Err(mismatch());
    };
    let (provision, receiver_binding, extent_bindings, call, release) = match tail {
        [
            first_binding,
            second_binding,
            call,
            release,
            Step::ReturnUnit,
        ] if receiver.is_none() => (None, None, [first_binding, second_binding], call, release),
        [
            provision,
            receiver_binding,
            first_binding,
            second_binding,
            call,
            release,
            Step::ReturnUnit,
        ] if receiver.is_some() => (
            Some(provision),
            Some(receiver_binding),
            [first_binding, second_binding],
            call,
            release,
        ),
        _ => return Err(mismatch()),
    };
    let Step::ReleaseOutgoingStackFrame {
        byte_count: release,
    } = release
    else {
        return Err(mismatch());
    };
    let Step::CallPrivateTerminalContinuation { disposition, .. } = call else {
        return Err(mismatch());
    };
    let copies = [first_copy, second_copy, third_copy, fourth_copy]
        .map(project_copy)
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?
        .try_into()
        .map_err(|_| mismatch())?;
    let argument_bindings = extent_bindings
        .map(project_binding)
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?
        .try_into()
        .map_err(|_| mismatch())?;
    let receiver_slot = match (receiver, provision, receiver_binding) {
        (None, None, None) => None,
        (Some(receiver), Some(provision), Some(receiver_binding)) => {
            let Step::ProvisionReceiverMutableStorage {
                outgoing_stack_byte_offset,
                slot_byte_count,
            } = provision
            else {
                return Err(mismatch());
            };
            let Step::BindOutgoingReceiverAddress {
                register,
                outgoing_stack_byte_offset: bind_offset,
                byte_count,
                alignment,
            } = receiver_binding
            else {
                return Err(mismatch());
            };
            if *outgoing_stack_byte_offset != receiver.outgoing_stack_byte_offset()
                || *slot_byte_count != receiver.slot_byte_count()
                || *bind_offset != receiver.outgoing_stack_byte_offset()
                || *byte_count != receiver.byte_count()
                || *alignment != receiver.alignment()
                || *register != MachineRegister::X86Rcx
            {
                return Err(mismatch());
            }
            Some(X86_64SemanticUnitWrapperReceiverSlot {
                byte_count: receiver.byte_count(),
                alignment: receiver.alignment(),
                slot_byte_count: receiver.slot_byte_count(),
                outgoing_stack_byte_offset: receiver.outgoing_stack_byte_offset(),
                register: *register,
            })
        }
        _ => return Err(mismatch()),
    };
    let relocation = source.relocation();
    let expected_call_index = if receiver.is_some() { 10 } else { 8 };
    if source.encoding_disposition()
        != OptimizedProgramStorageSemanticWrapperEncodingDisposition::TargetEncodingRequiredV1
        || relocation.call_step_index() != expected_call_index
        || relocation.kind()
            != OptimizedProgramStorageSemanticWrapperRelocationKind::X86Relative32PrivateContinuationV1
        || relocation.continuation()
            != OptimizedProgramStorageSemanticWrapperContinuationDisposition::PrivateTerminalSymbolRequiredV1
        || *disposition
            != OptimizedProgramStorageSemanticWrapperContinuationDisposition::PrivateTerminalSymbolRequiredV1
        || *reserve != source.outgoing_frame_byte_count()
        || *release != source.outgoing_release_byte_count()
    {
        return Err(mismatch());
    }
    Ok(X86_64SemanticUnitWrapperEncodingRequest {
        target: source.source().target(),
        policy: X86_64SemanticUnitWrapperEncodingPolicy::MicrosoftX64CallerSavedOnlyNoControlStateMutationV1,
        shadow_byte_count: source.shadow_byte_count(),
        outgoing_frame_byte_count: *reserve,
        outgoing_release_byte_count: *release,
        pre_call_stack_alignment: source.pre_call_stack_alignment(),
        copies,
        argument_bindings,
        receiver: receiver_slot,
        relocation_field_byte_width: relocation.byte_width(),
        relocation_addend: relocation.addend(),
    })
}

fn project_copy(
    step: &OptimizedProgramStorageSemanticWrapperStep,
) -> Result<X86_64SemanticUnitWrapperCopy, OptimizedProgramStorageSemanticWrapperEncodingError> {
    let OptimizedProgramStorageSemanticWrapperStep::CopyIncomingIndirectExtentWord {
        source_register,
        source_byte_offset,
        outgoing_stack_byte_offset,
        ..
    } = step
    else {
        return Err(OptimizedProgramStorageSemanticWrapperEncodingError::SemanticStepShapeMismatch);
    };
    Ok(X86_64SemanticUnitWrapperCopy {
        source_register: *source_register,
        source_byte_offset: u32::from(*source_byte_offset),
        outgoing_stack_byte_offset: *outgoing_stack_byte_offset,
    })
}

fn project_binding(
    step: &OptimizedProgramStorageSemanticWrapperStep,
) -> Result<
    X86_64SemanticUnitWrapperArgumentBinding,
    OptimizedProgramStorageSemanticWrapperEncodingError,
> {
    let OptimizedProgramStorageSemanticWrapperStep::BindOutgoingExtentCopyAddress {
        register,
        outgoing_stack_byte_offset,
        ..
    } = step
    else {
        return Err(OptimizedProgramStorageSemanticWrapperEncodingError::SemanticStepShapeMismatch);
    };
    Ok(X86_64SemanticUnitWrapperArgumentBinding {
        register: *register,
        outgoing_stack_byte_offset: *outgoing_stack_byte_offset,
    })
}

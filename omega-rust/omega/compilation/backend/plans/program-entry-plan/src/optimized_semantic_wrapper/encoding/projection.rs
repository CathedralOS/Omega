use crate::{
    OptimizedProgramStorageSemanticWrapperContinuationDisposition,
    OptimizedProgramStorageSemanticWrapperEncodingDisposition,
    OptimizedProgramStorageSemanticWrapperPlan,
    OptimizedProgramStorageSemanticWrapperRelocationKind,
    OptimizedProgramStorageSemanticWrapperStep,
};
use isa_x86_64::{
    X86_64SemanticUnitWrapperArgumentBinding, X86_64SemanticUnitWrapperCopy,
    X86_64SemanticUnitWrapperEncodingPolicy, X86_64SemanticUnitWrapperEncodingRequest,
};

use super::OptimizedProgramStorageSemanticWrapperEncodingError;

pub(super) fn project_request(
    source: &OptimizedProgramStorageSemanticWrapperPlan,
) -> Result<
    X86_64SemanticUnitWrapperEncodingRequest,
    OptimizedProgramStorageSemanticWrapperEncodingError,
> {
    use OptimizedProgramStorageSemanticWrapperStep as Step;
    let mismatch =
        || OptimizedProgramStorageSemanticWrapperEncodingError::SemanticStepShapeMismatch;
    // The same two installed roots are copied before their exact outgoing
    // pointers are bound; no other storage or argument is introduced.
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
    let [
        first_binding,
        second_binding,
        call,
        release,
        Step::ReturnUnit,
    ] = tail
    else {
        return Err(mismatch());
    };
    let extent_bindings = [first_binding, second_binding];
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
    let relocation = source.relocation();
    let expected_call_index = 8;
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

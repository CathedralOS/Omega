use super::error::OptimizedProgramStorageSemanticWrapperEncodingError;
use isa_x86_64::{
    X86_64SemanticUnitWrapperArgumentBinding, X86_64SemanticUnitWrapperCopy,
    X86_64SemanticUnitWrapperEncodingPolicy, X86_64SemanticUnitWrapperEncodingRequest,
};
use program_entry_plan::{
    OptimizedProgramStorageSemanticWrapperContinuationDisposition,
    OptimizedProgramStorageSemanticWrapperEncodingDisposition,
    OptimizedProgramStorageSemanticWrapperPlan,
    OptimizedProgramStorageSemanticWrapperRelocationKind,
    OptimizedProgramStorageSemanticWrapperStep,
};

pub(crate) fn project_request(
    source: &OptimizedProgramStorageSemanticWrapperPlan,
) -> Result<
    X86_64SemanticUnitWrapperEncodingRequest,
    OptimizedProgramStorageSemanticWrapperEncodingError,
> {
    use OptimizedProgramStorageSemanticWrapperStep as Step;
    let [
        Step::EnterFunction,
        Step::ReserveOutgoingStackFrame {
            byte_count: reserve,
        },
        first_copy,
        second_copy,
        third_copy,
        fourth_copy,
        first_binding,
        second_binding,
        Step::CallPrivateTerminalContinuation { disposition, .. },
        Step::ReleaseOutgoingStackFrame {
            byte_count: release,
        },
        Step::ReturnUnit,
    ] = source.steps()
    else {
        return Err(OptimizedProgramStorageSemanticWrapperEncodingError::SemanticStepShapeMismatch);
    };
    let copies = [first_copy, second_copy, third_copy, fourth_copy]
        .map(project_copy)
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?
        .try_into()
        .map_err(|_| {
            OptimizedProgramStorageSemanticWrapperEncodingError::SemanticStepShapeMismatch
        })?;
    let argument_bindings = [first_binding, second_binding]
        .map(project_binding)
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?
        .try_into()
        .map_err(|_| {
            OptimizedProgramStorageSemanticWrapperEncodingError::SemanticStepShapeMismatch
        })?;
    let relocation = source.relocation();
    if source.encoding_disposition()
        != OptimizedProgramStorageSemanticWrapperEncodingDisposition::TargetEncodingRequiredV1
        || relocation.call_step_index() != 8
        || relocation.kind()
            != OptimizedProgramStorageSemanticWrapperRelocationKind::X86Relative32PrivateContinuationV1
        || relocation.continuation()
            != OptimizedProgramStorageSemanticWrapperContinuationDisposition::PrivateTerminalSymbolRequiredV1
        || *disposition
            != OptimizedProgramStorageSemanticWrapperContinuationDisposition::PrivateTerminalSymbolRequiredV1
        || *reserve != source.outgoing_frame_byte_count()
        || *release != source.outgoing_release_byte_count()
    {
        return Err(
            OptimizedProgramStorageSemanticWrapperEncodingError::SemanticStepShapeMismatch,
        );
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

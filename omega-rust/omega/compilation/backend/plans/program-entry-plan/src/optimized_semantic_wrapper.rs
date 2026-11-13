/*
We plan, validate, project, and encode the address-free semantic
`ProgramStorage` wrapper recipe for the `UefiX64` `ProgramStorageApplication`
slot, copying the two incoming indirect `Extent` arguments into caller-owned
stack storage and replaying the x86-64 ISA template before continuation
resolution.
*/

use crate::{
    OptimizedProgramStoragePhysicalEntryDisposition, OptimizedProgramStorageSemanticEntryContract,
    ProgramEntryExtentFieldRole, ProgramEntrySignatureIdentity, ProgramStorageEntryDiagnostic,
    ProgramStorageEntryRootRole,
};
use calling_conventions::{
    CallingPolicy, IndirectPointerLocation, MachineRegister, ValueLocation, ValuePlacement,
};
use isa_x86_64::{
    ValidatedX86_64SemanticUnitWrapperTemplate, X86_64SemanticUnitWrapperArgumentBinding,
    X86_64SemanticUnitWrapperCopy, X86_64SemanticUnitWrapperEncodingError,
    X86_64SemanticUnitWrapperEncodingPolicy, X86_64SemanticUnitWrapperEncodingRequest,
    encode_x86_64_semantic_unit_wrapper_template, validate_x86_64_semantic_unit_wrapper_template,
};

const SHADOW_BYTE_COUNT: u32 = 32;
const OUTGOING_FRAME_BYTE_COUNT: u32 = 72;
const PRE_CALL_STACK_ALIGNMENT: u16 = 16;
const EXTENT_BYTE_COUNT: u16 = 16;
const EXTENT_ALIGNMENT: u16 = 8;

pub fn plan_optimized_program_storage_semantic_wrapper(
    source: OptimizedProgramStorageSemanticEntryContract,
) -> Result<OptimizedProgramStorageSemanticWrapperPlan, ProgramStorageEntryDiagnostic> {
    validate_contract_surface(&source)?;
    let source_signature_identity = source.source_signature_identity();
    let fingerprint = source.semantic_calling_plan_report_fingerprint();
    let frame_byte_count = OUTGOING_FRAME_BYTE_COUNT;
    let steps = expected_steps(fingerprint);
    let call_step_index = steps
        .iter()
        .position(|step| {
            matches!(
                step,
                OptimizedProgramStorageSemanticWrapperStep::CallPrivateTerminalContinuation { .. }
            )
        })
        .expect("the canonical recipe always emits one continuation call");
    let plan = OptimizedProgramStorageSemanticWrapperPlan {
        source,
        source_signature_identity,
        shadow_byte_count: SHADOW_BYTE_COUNT,
        outgoing_frame_byte_count: frame_byte_count,
        outgoing_release_byte_count: frame_byte_count,
        pre_call_stack_alignment: PRE_CALL_STACK_ALIGNMENT,
        steps,
        relocation: expected_relocation(call_step_index),
        encoding_disposition:
            OptimizedProgramStorageSemanticWrapperEncodingDisposition::TargetEncodingRequiredV1,
        physical_disposition: OptimizedProgramStoragePhysicalEntryDisposition::PlannedNotInvokedV1,
    };
    validate_optimized_program_storage_semantic_wrapper(&plan)?;
    Ok(plan)
}

pub(crate) fn validate_optimized_program_storage_semantic_wrapper(
    plan: &OptimizedProgramStorageSemanticWrapperPlan,
) -> Result<(), ProgramStorageEntryDiagnostic> {
    validate_contract_surface(&plan.source)?;
    let expected_frame = OUTGOING_FRAME_BYTE_COUNT;
    if plan.source_signature_identity != plan.source.source_signature_identity()
        || plan.shadow_byte_count != SHADOW_BYTE_COUNT
        || plan.outgoing_frame_byte_count != expected_frame
        || plan.outgoing_release_byte_count != plan.outgoing_frame_byte_count
        || plan.pre_call_stack_alignment != PRE_CALL_STACK_ALIGNMENT
        || plan.encoding_disposition
            != OptimizedProgramStorageSemanticWrapperEncodingDisposition::TargetEncodingRequiredV1
        || plan.physical_disposition
            != OptimizedProgramStoragePhysicalEntryDisposition::PlannedNotInvokedV1
    {
        return Err(ProgramStorageEntryDiagnostic(
            "optimized semantic ProgramStorage wrapper frame or source custody drifted".into(),
        ));
    }
    replay_steps(plan)?;
    let call_step_index = plan
        .steps
        .iter()
        .position(|step| {
            matches!(
                step,
                OptimizedProgramStorageSemanticWrapperStep::CallPrivateTerminalContinuation { .. }
            )
        })
        .ok_or(ProgramStorageEntryDiagnostic(
            "optimized semantic ProgramStorage wrapper has no continuation call".into(),
        ))?;
    if plan.relocation != expected_relocation(call_step_index) {
        return Err(ProgramStorageEntryDiagnostic(
            "optimized semantic ProgramStorage wrapper call relocation drifted".into(),
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptimizedProgramStorageSemanticWrapperContinuationDisposition {
    PrivateTerminalSymbolRequiredV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptimizedProgramStorageSemanticWrapperRelocationKind {
    X86Relative32PrivateContinuationV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptimizedProgramStorageSemanticWrapperEncodingDisposition {
    TargetEncodingRequiredV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OptimizedProgramStorageSemanticWrapperRelocationRequirement {
    call_step_index: usize,
    byte_width: u8,
    addend: i64,
    kind: OptimizedProgramStorageSemanticWrapperRelocationKind,
    continuation: OptimizedProgramStorageSemanticWrapperContinuationDisposition,
}

impl OptimizedProgramStorageSemanticWrapperRelocationRequirement {
    pub const fn call_step_index(&self) -> usize {
        self.call_step_index
    }

    pub const fn byte_width(&self) -> u8 {
        self.byte_width
    }

    pub const fn addend(&self) -> i64 {
        self.addend
    }

    pub const fn kind(&self) -> OptimizedProgramStorageSemanticWrapperRelocationKind {
        self.kind
    }

    pub const fn continuation(
        &self,
    ) -> OptimizedProgramStorageSemanticWrapperContinuationDisposition {
        self.continuation
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizedProgramStorageSemanticWrapperStep {
    EnterFunction,
    ReserveOutgoingStackFrame {
        byte_count: u32,
    },
    CopyIncomingIndirectExtentWord {
        role: ProgramStorageEntryRootRole,
        parameter_index: usize,
        field: ProgramEntryExtentFieldRole,
        source_register: MachineRegister,
        source_byte_offset: u16,
        outgoing_stack_byte_offset: u32,
    },
    BindOutgoingExtentCopyAddress {
        role: ProgramStorageEntryRootRole,
        parameter_index: usize,
        register: MachineRegister,
        outgoing_stack_byte_offset: u32,
        byte_count: u16,
        alignment: u16,
    },
    CallPrivateTerminalContinuation {
        calling_policy: CallingPolicy,
        semantic_calling_plan_report_fingerprint: u64,
        disposition: OptimizedProgramStorageSemanticWrapperContinuationDisposition,
    },
    ReleaseOutgoingStackFrame {
        byte_count: u32,
    },
    ReturnUnit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptimizedProgramStorageSemanticWrapperPlan {
    source: OptimizedProgramStorageSemanticEntryContract,
    source_signature_identity: ProgramEntrySignatureIdentity,
    shadow_byte_count: u32,
    outgoing_frame_byte_count: u32,
    outgoing_release_byte_count: u32,
    pre_call_stack_alignment: u16,
    steps: Vec<OptimizedProgramStorageSemanticWrapperStep>,
    relocation: OptimizedProgramStorageSemanticWrapperRelocationRequirement,
    encoding_disposition: OptimizedProgramStorageSemanticWrapperEncodingDisposition,
    physical_disposition: OptimizedProgramStoragePhysicalEntryDisposition,
}

impl OptimizedProgramStorageSemanticWrapperPlan {
    pub const fn source(&self) -> &OptimizedProgramStorageSemanticEntryContract {
        &self.source
    }

    pub const fn source_signature_identity(&self) -> ProgramEntrySignatureIdentity {
        self.source_signature_identity
    }

    pub const fn shadow_byte_count(&self) -> u32 {
        self.shadow_byte_count
    }

    pub const fn outgoing_frame_byte_count(&self) -> u32 {
        self.outgoing_frame_byte_count
    }

    pub const fn outgoing_release_byte_count(&self) -> u32 {
        self.outgoing_release_byte_count
    }

    pub const fn pre_call_stack_alignment(&self) -> u16 {
        self.pre_call_stack_alignment
    }

    pub fn steps(&self) -> &[OptimizedProgramStorageSemanticWrapperStep] {
        &self.steps
    }

    pub const fn relocation(&self) -> &OptimizedProgramStorageSemanticWrapperRelocationRequirement {
        &self.relocation
    }

    pub const fn encoding_disposition(
        &self,
    ) -> OptimizedProgramStorageSemanticWrapperEncodingDisposition {
        self.encoding_disposition
    }

    pub const fn physical_disposition(&self) -> OptimizedProgramStoragePhysicalEntryDisposition {
        self.physical_disposition
    }
}

#[derive(Debug)]
#[must_use = "target wrapper encoding custody must be retained through continuation resolution"]
pub struct StagedOptimizedProgramStorageSemanticWrapperEncoding {
    source: OptimizedProgramStorageSemanticWrapperPlan,
    request: X86_64SemanticUnitWrapperEncodingRequest,
    template: ValidatedX86_64SemanticUnitWrapperTemplate,
}

impl StagedOptimizedProgramStorageSemanticWrapperEncoding {
    pub const fn source(&self) -> &OptimizedProgramStorageSemanticWrapperPlan {
        &self.source
    }

    pub const fn request(&self) -> X86_64SemanticUnitWrapperEncodingRequest {
        self.request
    }

    pub const fn template(&self) -> &ValidatedX86_64SemanticUnitWrapperTemplate {
        &self.template
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizedProgramStorageSemanticWrapperEncodingError {
    InvalidSemanticPlan,
    SemanticStepShapeMismatch,
    Target(X86_64SemanticUnitWrapperEncodingError),
    TemplateMismatch,
}

diagnostics::failed_display!(
    OptimizedProgramStorageSemanticWrapperEncodingError,
    "optimized ProgramStorage semantic wrapper encoding failed"
);

impl std::error::Error for OptimizedProgramStorageSemanticWrapperEncodingError {}

pub fn select_optimized_program_storage_semantic_wrapper_encoding(
    source: OptimizedProgramStorageSemanticWrapperPlan,
) -> Result<
    StagedOptimizedProgramStorageSemanticWrapperEncoding,
    OptimizedProgramStorageSemanticWrapperEncodingError,
> {
    validate_optimized_program_storage_semantic_wrapper(&source)
        .map_err(|_| OptimizedProgramStorageSemanticWrapperEncodingError::InvalidSemanticPlan)?;
    let request = project_request(&source)?;
    let template = encode_x86_64_semantic_unit_wrapper_template(request)
        .map_err(OptimizedProgramStorageSemanticWrapperEncodingError::Target)?;
    let staged = StagedOptimizedProgramStorageSemanticWrapperEncoding {
        source,
        request,
        template,
    };
    validate_optimized_program_storage_semantic_wrapper_encoding(&staged)?;
    Ok(staged)
}

pub(crate) fn validate_optimized_program_storage_semantic_wrapper_encoding(
    staged: &StagedOptimizedProgramStorageSemanticWrapperEncoding,
) -> Result<(), OptimizedProgramStorageSemanticWrapperEncodingError> {
    validate_optimized_program_storage_semantic_wrapper(&staged.source)
        .map_err(|_| OptimizedProgramStorageSemanticWrapperEncodingError::InvalidSemanticPlan)?;
    let expected_request = project_request(&staged.source)?;
    if staged.request != expected_request {
        return Err(OptimizedProgramStorageSemanticWrapperEncodingError::TemplateMismatch);
    }
    let expected =
        validate_x86_64_semantic_unit_wrapper_template(expected_request, staged.template.bytes())
            .map_err(OptimizedProgramStorageSemanticWrapperEncodingError::Target)?;
    if staged.template != expected {
        return Err(OptimizedProgramStorageSemanticWrapperEncodingError::TemplateMismatch);
    }
    Ok(())
}

fn project_request(
    source: &OptimizedProgramStorageSemanticWrapperPlan,
) -> Result<
    X86_64SemanticUnitWrapperEncodingRequest,
    OptimizedProgramStorageSemanticWrapperEncodingError,
> {
    use OptimizedProgramStorageSemanticWrapperStep as Step;
    let mismatch =
        || OptimizedProgramStorageSemanticWrapperEncodingError::SemanticStepShapeMismatch;
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

fn expected_steps(report_fingerprint: u64) -> Vec<OptimizedProgramStorageSemanticWrapperStep> {
    use OptimizedProgramStorageSemanticWrapperStep::*;
    let frame_byte_count = OUTGOING_FRAME_BYTE_COUNT;
    let mut steps = vec![
        EnterFunction,
        ReserveOutgoingStackFrame {
            byte_count: frame_byte_count,
        },
        copy(
            ProgramStorageEntryRootRole::Image,
            0,
            ProgramEntryExtentFieldRole::Base,
            MachineRegister::X86Rcx,
            0,
            32,
        ),
        copy(
            ProgramStorageEntryRootRole::Image,
            0,
            ProgramEntryExtentFieldRole::Length,
            MachineRegister::X86Rcx,
            8,
            40,
        ),
        copy(
            ProgramStorageEntryRootRole::InitialStorage,
            1,
            ProgramEntryExtentFieldRole::Base,
            MachineRegister::X86Rdx,
            0,
            48,
        ),
        copy(
            ProgramStorageEntryRootRole::InitialStorage,
            1,
            ProgramEntryExtentFieldRole::Length,
            MachineRegister::X86Rdx,
            8,
            56,
        ),
    ];
    steps.push(bind(
        ProgramStorageEntryRootRole::Image,
        0,
        MachineRegister::X86Rcx,
        32,
    ));
    steps.push(bind(
        ProgramStorageEntryRootRole::InitialStorage,
        1,
        MachineRegister::X86Rdx,
        48,
    ));
    steps.push(CallPrivateTerminalContinuation {
        calling_policy: CallingPolicy::MicrosoftX64,
        semantic_calling_plan_report_fingerprint: report_fingerprint,
        disposition:
            OptimizedProgramStorageSemanticWrapperContinuationDisposition::PrivateTerminalSymbolRequiredV1,
    });
    steps.push(ReleaseOutgoingStackFrame {
        byte_count: frame_byte_count,
    });
    steps.push(ReturnUnit);
    steps
}

fn copy(
    role: ProgramStorageEntryRootRole,
    parameter_index: usize,
    field: ProgramEntryExtentFieldRole,
    source_register: MachineRegister,
    source_byte_offset: u16,
    outgoing_stack_byte_offset: u32,
) -> OptimizedProgramStorageSemanticWrapperStep {
    OptimizedProgramStorageSemanticWrapperStep::CopyIncomingIndirectExtentWord {
        role,
        parameter_index,
        field,
        source_register,
        source_byte_offset,
        outgoing_stack_byte_offset,
    }
}

fn bind(
    role: ProgramStorageEntryRootRole,
    parameter_index: usize,
    register: MachineRegister,
    outgoing_stack_byte_offset: u32,
) -> OptimizedProgramStorageSemanticWrapperStep {
    OptimizedProgramStorageSemanticWrapperStep::BindOutgoingExtentCopyAddress {
        role,
        parameter_index,
        register,
        outgoing_stack_byte_offset,
        byte_count: EXTENT_BYTE_COUNT,
        alignment: EXTENT_ALIGNMENT,
    }
}

fn expected_relocation(
    call_step_index: usize,
) -> OptimizedProgramStorageSemanticWrapperRelocationRequirement {
    OptimizedProgramStorageSemanticWrapperRelocationRequirement {
        call_step_index,
        byte_width: 4,
        addend: 0,
        kind: OptimizedProgramStorageSemanticWrapperRelocationKind::X86Relative32PrivateContinuationV1,
        continuation: OptimizedProgramStorageSemanticWrapperContinuationDisposition::PrivateTerminalSymbolRequiredV1,
    }
}

fn validate_contract_surface(
    contract: &OptimizedProgramStorageSemanticEntryContract,
) -> Result<(), ProgramStorageEntryDiagnostic> {
    if contract.target_slot() != target::TargetProfile::UefiX64.program_entry_slot()
        || contract.target() != contract.target_slot().owner.native_target()
        || contract.semantic_boundary_entry_plan().call.policy != CallingPolicy::MicrosoftX64
        || contract
            .semantic_boundary_entry_plan()
            .call
            .result
            .is_some()
        || contract.physical_disposition()
            != OptimizedProgramStoragePhysicalEntryDisposition::PlannedNotInvokedV1
    {
        return Err(ProgramStorageEntryDiagnostic(
            "optimized semantic ProgramStorage wrapper requires one non-invoked UEFI Microsoft-x64 Unit contract"
                .into(),
        ));
    }
    let [image, storage] = contract.roots();
    validate_root_placement(
        image.role(),
        image.parameter_index(),
        image.placement(),
        ProgramStorageEntryRootRole::Image,
        0,
        MachineRegister::X86Rcx,
        32,
    )?;
    validate_root_placement(
        storage.role(),
        storage.parameter_index(),
        storage.placement(),
        ProgramStorageEntryRootRole::InitialStorage,
        1,
        MachineRegister::X86Rdx,
        48,
    )
}

fn validate_root_placement(
    actual_role: ProgramStorageEntryRootRole,
    actual_index: usize,
    placement: &ValuePlacement,
    expected_role: ProgramStorageEntryRootRole,
    expected_index: usize,
    expected_register: MachineRegister,
    expected_copy_offset: u32,
) -> Result<(), ProgramStorageEntryDiagnostic> {
    if actual_role != expected_role
        || actual_index != expected_index
        || placement.shape.byte_size != u32::from(EXTENT_BYTE_COUNT)
        || placement.shape.alignment != EXTENT_ALIGNMENT
        || !matches!(
            placement.locations.as_slice(),
            [ValueLocation::Indirect {
                pointer: IndirectPointerLocation::Register(register),
                copy_stack_byte_offset: Some(copy_offset),
                byte_size,
                alignment: EXTENT_ALIGNMENT,
            }] if *register == expected_register && *copy_offset == expected_copy_offset
                && *byte_size == u32::from(EXTENT_BYTE_COUNT)
        )
    {
        return Err(ProgramStorageEntryDiagnostic(format!(
            "optimized semantic ProgramStorage {expected_role:?} placement drifted"
        )));
    }
    Ok(())
}

fn replay_steps(
    plan: &OptimizedProgramStorageSemanticWrapperPlan,
) -> Result<(), ProgramStorageEntryDiagnostic> {
    let fingerprint = plan.source.semantic_calling_plan_report_fingerprint();
    if plan.steps != expected_steps(fingerprint) {
        return Err(ProgramStorageEntryDiagnostic(
            "optimized semantic ProgramStorage wrapper action sequence drifted".into(),
        ));
    }
    Ok(())
}

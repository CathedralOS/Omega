//! Selection and replay of the target-owned semantic ProgramStorage wrapper.
//!
//! This boundary consumes the address-free semantic plan by value and projects
//! every retained action into one named target encoding policy. It still owns
//! no continuation symbol, resolved call, object, physical bootstrap, image,
//! installation, or publication authority.

use omega_program_storage::{
    OptimizedProgramStorageSemanticWrapperContinuationDisposition,
    OptimizedProgramStorageSemanticWrapperEncodingDisposition,
    OptimizedProgramStorageSemanticWrapperPlan,
    OptimizedProgramStorageSemanticWrapperRelocationKind,
    OptimizedProgramStorageSemanticWrapperStep,
    validate_optimized_program_storage_semantic_wrapper,
};
use omega_terminal_isa_x86_64::{
    ValidatedX86_64SemanticUnitWrapperTemplate, X86_64SemanticUnitWrapperArgumentBinding,
    X86_64SemanticUnitWrapperCopy, X86_64SemanticUnitWrapperEncodingError,
    X86_64SemanticUnitWrapperEncodingPolicy, X86_64SemanticUnitWrapperEncodingRequest,
    encode_x86_64_semantic_unit_wrapper_template, validate_x86_64_semantic_unit_wrapper_template,
};

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

impl std::fmt::Display for OptimizedProgramStorageSemanticWrapperEncodingError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "optimized ProgramStorage semantic wrapper encoding failed: {self:?}"
        )
    }
}

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

pub fn validate_optimized_program_storage_semantic_wrapper_encoding(
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

#[cfg(test)]
mod tests {
    use super::*;
    use omega_calling_conventions::{
        CallSignature, CallingPolicy, ValidatedBoundaryEntryPlan, ValueShape,
        evaluate_ordinary_boundary_entry_plan,
    };
    use omega_effects::provider_plan::{
        ServiceEntryAuthorityFlow, ServiceEntryClaim, ServiceMethod, ServiceSchema,
    };
    use omega_program_storage::{
        ProgramEntryPhysicalContractPlan, ProgramEntrySourceExtentValueLayout,
        ProgramEntrySourceReceiverSignature, ProgramStorageEntryRootRole,
        SelectedProgramEntrySourceSignature, SelectedProgramStorageEntryPlan,
        bind_optimized_program_storage_semantic_entry_contract,
        plan_optimized_program_storage_semantic_wrapper,
    };
    use omega_terminal_isa_x86_64::{
        X86_64_SEMANTIC_UNIT_WRAPPER_CALL_OPCODE_OFFSET,
        X86_64_SEMANTIC_UNIT_WRAPPER_FUNCTION_BYTE_COUNT,
        X86_64_SEMANTIC_UNIT_WRAPPER_NEXT_INSTRUCTION_OFFSET,
        X86_64_SEMANTIC_UNIT_WRAPPER_REL32_FIELD_OFFSET,
    };
    use psi_language_semantics::{CarryPolicy, DomainPredicateBody};
    use psi_symbols::SymbolHandle;

    const REQUIREMENT: &str = "ProgramStorageEntry::enter#encoding";
    const EXTENT_SHAPE: ValueShape = ValueShape::integer(16, 8);
    const WORD_SHAPE: ValueShape = ValueShape::integer(8, 8);

    fn extent_layout(base: u32) -> ProgramEntrySourceExtentValueLayout {
        ProgramEntrySourceExtentValueLayout::from_checked_record(
            SymbolHandle::from_arena_index(base),
            SymbolHandle::from_arena_index(base + 1),
            0,
            WORD_SHAPE,
            SymbolHandle::from_arena_index(base + 2),
            8,
            WORD_SHAPE,
            EXTENT_SHAPE,
        )
        .unwrap()
    }

    fn semantic() -> ValidatedBoundaryEntryPlan {
        evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::MicrosoftX64,
            &CallSignature {
                parameters: vec![EXTENT_SHAPE, EXTENT_SHAPE],
                result: None,
            },
        )
        .unwrap()
    }

    fn wrapper() -> OptimizedProgramStorageSemanticWrapperPlan {
        let slot = omega_target::TargetProfile::UefiX64.program_entry_slot();
        let semantic = semantic();
        let claim = |parameter_index| ServiceEntryClaim {
            parameter_index,
            carrier_identity: "named(name(Extent))".into(),
            domain: "Extent::Granted".into(),
            predicate_body: DomainPredicateBody::Present,
            effective_carry: CarryPolicy::STRICT,
            authority_flow: ServiceEntryAuthorityFlow::Accepts,
        };
        let storage = SelectedProgramStorageEntryPlan::from_target_slot(
            slot,
            ServiceSchema {
                trait_name: slot.boundary_schema.unwrap().into(),
                methods: vec![ServiceMethod {
                    name: "enter".into(),
                    requirement_owner: "ProgramStorageEntry".into(),
                    requirement_identity: REQUIREMENT.into(),
                    parameter_count: 2,
                    parameter_type_identities: vec!["ImageExtent".into(), "StorageExtent".into()],
                    entry_claims: vec![claim(0), claim(1)],
                    calling_plan_fingerprint: Some(semantic.contract_fingerprint()),
                    ..Default::default()
                }],
                ..Default::default()
            },
            REQUIREMENT.into(),
        )
        .unwrap();
        let pointer = ValueShape::integer(8, 8);
        let physical = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::MicrosoftX64,
            &CallSignature {
                parameters: vec![pointer, pointer],
                result: Some(pointer),
            },
        )
        .unwrap();
        let storage = storage
            .with_physical_contract(
                ProgramEntryPhysicalContractPlan::new(
                    slot,
                    "UefiPhysicalEntry::enter#encoding".into(),
                    omega_target::ProgramEntryPhysicalContractPackage::UefiX64,
                    1,
                    vec!["EfiImageHandle".into(), "&EfiSystemTable".into()],
                    "EfiStatus".into(),
                    physical.contract_fingerprint(),
                    physical.plan().clone(),
                )
                .unwrap(),
            )
            .unwrap();
        let source = SelectedProgramEntrySourceSignature::from_checked_typed_entry(
            slot,
            SymbolHandle::from_arena_index(1),
            SymbolHandle::from_arena_index(2),
            "Boot::launch".into(),
            "launch".into(),
            "Boot::launch#encoding".into(),
            ProgramEntrySourceReceiverSignature::Free,
            vec![
                SelectedProgramEntrySourceSignature::visible_parameter(
                    ProgramStorageEntryRootRole::Image,
                    0,
                    "ImageExtent".into(),
                    EXTENT_SHAPE,
                    extent_layout(10),
                    false,
                    false,
                ),
                SelectedProgramEntrySourceSignature::visible_parameter(
                    ProgramStorageEntryRootRole::InitialStorage,
                    1,
                    "StorageExtent".into(),
                    EXTENT_SHAPE,
                    extent_layout(20),
                    false,
                    false,
                ),
            ],
        )
        .unwrap();
        let contract = bind_optimized_program_storage_semantic_entry_contract(
            omega_target::NativeTarget::uefi_x64(),
            &storage,
            &source,
            semantic.plan(),
        )
        .unwrap();
        plan_optimized_program_storage_semantic_wrapper(contract).unwrap()
    }

    #[test]
    fn semantic_plan_selects_the_explicit_compact_target_encoding() {
        let staged = select_optimized_program_storage_semantic_wrapper_encoding(wrapper()).unwrap();
        validate_optimized_program_storage_semantic_wrapper_encoding(&staged).unwrap();
        assert_eq!(
            staged.request().policy,
            X86_64SemanticUnitWrapperEncodingPolicy::MicrosoftX64CallerSavedOnlyNoControlStateMutationV1
        );
        assert_eq!(
            staged.template().bytes().len(),
            X86_64_SEMANTIC_UNIT_WRAPPER_FUNCTION_BYTE_COUNT
        );
        assert_eq!(
            staged.template().relocation().opcode_function_byte_offset,
            X86_64_SEMANTIC_UNIT_WRAPPER_CALL_OPCODE_OFFSET
        );
        assert_eq!(
            staged.template().relocation().field_function_byte_offset,
            X86_64_SEMANTIC_UNIT_WRAPPER_REL32_FIELD_OFFSET
        );
        assert_eq!(
            staged
                .template()
                .relocation()
                .next_instruction_function_byte_offset,
            X86_64_SEMANTIC_UNIT_WRAPPER_NEXT_INSTRUCTION_OFFSET
        );
        assert_ne!(
            u32::from(X86_64_SEMANTIC_UNIT_WRAPPER_CALL_OPCODE_OFFSET),
            113
        );
    }

    #[test]
    fn retained_target_request_drift_fails_closed() {
        let mut staged =
            select_optimized_program_storage_semantic_wrapper_encoding(wrapper()).unwrap();
        staged.request.outgoing_frame_byte_count = 88;
        assert_eq!(
            validate_optimized_program_storage_semantic_wrapper_encoding(&staged),
            Err(OptimizedProgramStorageSemanticWrapperEncodingError::TemplateMismatch)
        );
    }
}

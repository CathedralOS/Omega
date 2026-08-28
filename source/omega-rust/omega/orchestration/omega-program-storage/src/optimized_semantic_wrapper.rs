//! Address-free recipe for the clean semantic ProgramStorage wrapper.
//!
//! This carrier consumes only the validated semantic entry contract. It fixes
//! the Microsoft-x64 root forwarding and one unresolved compiler-private call
//! slot, but owns no Terminal identity, symbol, bytes, runtime values, physical
//! bootstrap, process entry, image, installation, or publication authority.

use omega_calling_conventions::{
    CallingPolicy, IndirectPointerLocation, MachineRegister, ValueLocation, ValuePlacement,
};

use crate::{
    OptimizedProgramStoragePhysicalEntryDisposition, OptimizedProgramStorageSemanticEntryContract,
    ProgramEntrySourceExtentFieldRole, ProgramEntrySourceSignatureIdentity,
    ProgramStorageEntryDiagnostic, ProgramStorageEntryRootRole,
};

const SHADOW_BYTE_COUNT: u32 = 32;
const OUTGOING_FRAME_BYTE_COUNT: u32 = 72;
const PRE_CALL_STACK_ALIGNMENT: u16 = 16;
const EXTENT_BYTE_COUNT: u16 = 16;
const EXTENT_ALIGNMENT: u16 = 8;
const CALL_STEP_INDEX: usize = 8;
const CALL_INSTRUCTION_FUNCTION_BYTE_OFFSET: u32 = 113;
const CALL_RELOCATION_FUNCTION_BYTE_OFFSET: u32 = 114;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptimizedProgramStorageSemanticWrapperContinuationDisposition {
    PrivateTerminalSymbolRequiredV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptimizedProgramStorageSemanticWrapperRelocationKind {
    X86Relative32PrivateContinuationV1,
}

/// One symbolic relocation requirement. The downstream object join must bind
/// its target to the exact private Terminal entry symbol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OptimizedProgramStorageSemanticWrapperRelocationRequirement {
    call_step_index: usize,
    call_instruction_function_byte_offset: u32,
    relocation_function_byte_offset: u32,
    byte_width: u8,
    addend: i64,
    kind: OptimizedProgramStorageSemanticWrapperRelocationKind,
    continuation: OptimizedProgramStorageSemanticWrapperContinuationDisposition,
}

impl OptimizedProgramStorageSemanticWrapperRelocationRequirement {
    pub const fn call_step_index(&self) -> usize {
        self.call_step_index
    }

    pub const fn call_instruction_function_byte_offset(&self) -> u32 {
        self.call_instruction_function_byte_offset
    }

    pub const fn relocation_function_byte_offset(&self) -> u32 {
        self.relocation_function_byte_offset
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

/// One compiler-owned action in the exact receiver-free semantic wrapper.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizedProgramStorageSemanticWrapperStep {
    EnterFunction,
    ReserveOutgoingStackFrame {
        byte_count: u32,
    },
    CopyIncomingIndirectExtentWord {
        role: ProgramStorageEntryRootRole,
        parameter_index: usize,
        field: ProgramEntrySourceExtentFieldRole,
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
        semantic_calling_plan_fingerprint: u64,
        disposition: OptimizedProgramStorageSemanticWrapperContinuationDisposition,
    },
    ReleaseOutgoingStackFrame {
        byte_count: u32,
    },
    ReturnUnit,
}

/// Exact address-free recipe for the clean semantic ProgramStorage wrapper.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptimizedProgramStorageSemanticWrapperPlan {
    source: OptimizedProgramStorageSemanticEntryContract,
    source_signature_identity: ProgramEntrySourceSignatureIdentity,
    shadow_byte_count: u32,
    outgoing_frame_byte_count: u32,
    outgoing_release_byte_count: u32,
    pre_call_stack_alignment: u16,
    steps: [OptimizedProgramStorageSemanticWrapperStep; 11],
    relocation: OptimizedProgramStorageSemanticWrapperRelocationRequirement,
    physical_disposition: OptimizedProgramStoragePhysicalEntryDisposition,
}

impl OptimizedProgramStorageSemanticWrapperPlan {
    pub const fn source(&self) -> &OptimizedProgramStorageSemanticEntryContract {
        &self.source
    }

    pub const fn source_signature_identity(&self) -> ProgramEntrySourceSignatureIdentity {
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

    pub const fn steps(&self) -> &[OptimizedProgramStorageSemanticWrapperStep; 11] {
        &self.steps
    }

    pub const fn relocation(&self) -> &OptimizedProgramStorageSemanticWrapperRelocationRequirement {
        &self.relocation
    }

    pub const fn physical_disposition(&self) -> OptimizedProgramStoragePhysicalEntryDisposition {
        self.physical_disposition
    }
}

/// Construct the pure semantic wrapper recipe without selecting a Terminal
/// call target or emitting bytes.
pub fn plan_optimized_program_storage_semantic_wrapper(
    source: OptimizedProgramStorageSemanticEntryContract,
) -> Result<OptimizedProgramStorageSemanticWrapperPlan, ProgramStorageEntryDiagnostic> {
    validate_contract_surface(&source)?;
    let source_signature_identity = source.source_signature_identity();
    let fingerprint = source.semantic_calling_plan_fingerprint();
    let plan = OptimizedProgramStorageSemanticWrapperPlan {
        source,
        source_signature_identity,
        shadow_byte_count: SHADOW_BYTE_COUNT,
        outgoing_frame_byte_count: OUTGOING_FRAME_BYTE_COUNT,
        outgoing_release_byte_count: OUTGOING_FRAME_BYTE_COUNT,
        pre_call_stack_alignment: PRE_CALL_STACK_ALIGNMENT,
        steps: expected_steps(fingerprint),
        relocation: expected_relocation(),
        physical_disposition: OptimizedProgramStoragePhysicalEntryDisposition::PlannedNotInvokedV1,
    };
    validate_optimized_program_storage_semantic_wrapper(&plan)?;
    Ok(plan)
}

/// Independently replay the retained contract, frame geometry, action order,
/// call slot, and symbolic relocation requirement.
pub fn validate_optimized_program_storage_semantic_wrapper(
    plan: &OptimizedProgramStorageSemanticWrapperPlan,
) -> Result<(), ProgramStorageEntryDiagnostic> {
    validate_contract_surface(&plan.source)?;
    if plan.source_signature_identity != plan.source.source_signature_identity()
        || plan.shadow_byte_count != SHADOW_BYTE_COUNT
        || plan.outgoing_frame_byte_count != OUTGOING_FRAME_BYTE_COUNT
        || plan.outgoing_release_byte_count != plan.outgoing_frame_byte_count
        || plan.pre_call_stack_alignment != PRE_CALL_STACK_ALIGNMENT
        || plan.physical_disposition
            != OptimizedProgramStoragePhysicalEntryDisposition::PlannedNotInvokedV1
    {
        return Err(ProgramStorageEntryDiagnostic(
            "optimized semantic ProgramStorage wrapper frame or source custody drifted".into(),
        ));
    }
    replay_steps(plan)?;
    if plan.relocation != expected_relocation() {
        return Err(ProgramStorageEntryDiagnostic(
            "optimized semantic ProgramStorage wrapper call relocation drifted".into(),
        ));
    }
    Ok(())
}

fn validate_contract_surface(
    contract: &OptimizedProgramStorageSemanticEntryContract,
) -> Result<(), ProgramStorageEntryDiagnostic> {
    if contract.target() != omega_target::NativeTarget::uefi_x64()
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
        || placement.shape.byte_size != EXTENT_BYTE_COUNT
        || placement.shape.alignment != EXTENT_ALIGNMENT
        || !matches!(
            placement.locations.as_slice(),
            [ValueLocation::Indirect {
                pointer: IndirectPointerLocation::Register(register),
                copy_stack_byte_offset: Some(copy_offset),
                byte_size: EXTENT_BYTE_COUNT,
                alignment: EXTENT_ALIGNMENT,
            }] if *register == expected_register && *copy_offset == expected_copy_offset
        )
    {
        return Err(ProgramStorageEntryDiagnostic(format!(
            "optimized semantic ProgramStorage {expected_role:?} placement drifted"
        )));
    }
    Ok(())
}

fn expected_steps(fingerprint: u64) -> [OptimizedProgramStorageSemanticWrapperStep; 11] {
    use OptimizedProgramStorageSemanticWrapperStep::*;
    [
        EnterFunction,
        ReserveOutgoingStackFrame {
            byte_count: OUTGOING_FRAME_BYTE_COUNT,
        },
        copy(ProgramStorageEntryRootRole::Image, 0, ProgramEntrySourceExtentFieldRole::Base, MachineRegister::X86Rcx, 0, 32),
        copy(ProgramStorageEntryRootRole::Image, 0, ProgramEntrySourceExtentFieldRole::Length, MachineRegister::X86Rcx, 8, 40),
        copy(ProgramStorageEntryRootRole::InitialStorage, 1, ProgramEntrySourceExtentFieldRole::Base, MachineRegister::X86Rdx, 0, 48),
        copy(ProgramStorageEntryRootRole::InitialStorage, 1, ProgramEntrySourceExtentFieldRole::Length, MachineRegister::X86Rdx, 8, 56),
        bind(ProgramStorageEntryRootRole::Image, 0, MachineRegister::X86Rcx, 32),
        bind(ProgramStorageEntryRootRole::InitialStorage, 1, MachineRegister::X86Rdx, 48),
        CallPrivateTerminalContinuation {
            calling_policy: CallingPolicy::MicrosoftX64,
            semantic_calling_plan_fingerprint: fingerprint,
            disposition: OptimizedProgramStorageSemanticWrapperContinuationDisposition::PrivateTerminalSymbolRequiredV1,
        },
        ReleaseOutgoingStackFrame {
            byte_count: OUTGOING_FRAME_BYTE_COUNT,
        },
        ReturnUnit,
    ]
}

fn copy(
    role: ProgramStorageEntryRootRole,
    parameter_index: usize,
    field: ProgramEntrySourceExtentFieldRole,
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

fn expected_relocation() -> OptimizedProgramStorageSemanticWrapperRelocationRequirement {
    OptimizedProgramStorageSemanticWrapperRelocationRequirement {
        call_step_index: CALL_STEP_INDEX,
        call_instruction_function_byte_offset: CALL_INSTRUCTION_FUNCTION_BYTE_OFFSET,
        relocation_function_byte_offset: CALL_RELOCATION_FUNCTION_BYTE_OFFSET,
        byte_width: 4,
        addend: 0,
        kind: OptimizedProgramStorageSemanticWrapperRelocationKind::X86Relative32PrivateContinuationV1,
        continuation: OptimizedProgramStorageSemanticWrapperContinuationDisposition::PrivateTerminalSymbolRequiredV1,
    }
}

fn replay_steps(
    plan: &OptimizedProgramStorageSemanticWrapperPlan,
) -> Result<(), ProgramStorageEntryDiagnostic> {
    use OptimizedProgramStorageSemanticWrapperStep as Step;
    let [
        Step::EnterFunction,
        Step::ReserveOutgoingStackFrame {
            byte_count: reserve,
        },
        image_base,
        image_length,
        storage_base,
        storage_length,
        image_address,
        storage_address,
        Step::CallPrivateTerminalContinuation {
            calling_policy,
            semantic_calling_plan_fingerprint,
            disposition,
        },
        Step::ReleaseOutgoingStackFrame {
            byte_count: release,
        },
        Step::ReturnUnit,
    ] = &plan.steps
    else {
        return Err(ProgramStorageEntryDiagnostic(
            "optimized semantic ProgramStorage wrapper action sequence drifted".into(),
        ));
    };
    if *reserve != OUTGOING_FRAME_BYTE_COUNT
        || !replay_copy(
            image_base,
            ProgramStorageEntryRootRole::Image,
            0,
            ProgramEntrySourceExtentFieldRole::Base,
            MachineRegister::X86Rcx,
            0,
            32,
        )
        || !replay_copy(
            image_length,
            ProgramStorageEntryRootRole::Image,
            0,
            ProgramEntrySourceExtentFieldRole::Length,
            MachineRegister::X86Rcx,
            8,
            40,
        )
        || !replay_copy(
            storage_base,
            ProgramStorageEntryRootRole::InitialStorage,
            1,
            ProgramEntrySourceExtentFieldRole::Base,
            MachineRegister::X86Rdx,
            0,
            48,
        )
        || !replay_copy(
            storage_length,
            ProgramStorageEntryRootRole::InitialStorage,
            1,
            ProgramEntrySourceExtentFieldRole::Length,
            MachineRegister::X86Rdx,
            8,
            56,
        )
        || !replay_bind(
            image_address,
            ProgramStorageEntryRootRole::Image,
            0,
            MachineRegister::X86Rcx,
            32,
        )
        || !replay_bind(
            storage_address,
            ProgramStorageEntryRootRole::InitialStorage,
            1,
            MachineRegister::X86Rdx,
            48,
        )
        || *calling_policy != CallingPolicy::MicrosoftX64
        || *semantic_calling_plan_fingerprint
            != plan.source.semantic_calling_plan_fingerprint()
        || *disposition
            != OptimizedProgramStorageSemanticWrapperContinuationDisposition::PrivateTerminalSymbolRequiredV1
        || *release != OUTGOING_FRAME_BYTE_COUNT
    {
        return Err(ProgramStorageEntryDiagnostic(
            "optimized semantic ProgramStorage wrapper action sequence drifted".into(),
        ));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn replay_copy(
    step: &OptimizedProgramStorageSemanticWrapperStep,
    expected_role: ProgramStorageEntryRootRole,
    expected_parameter_index: usize,
    expected_field: ProgramEntrySourceExtentFieldRole,
    expected_register: MachineRegister,
    expected_source_offset: u16,
    expected_stack_offset: u32,
) -> bool {
    matches!(
        step,
        OptimizedProgramStorageSemanticWrapperStep::CopyIncomingIndirectExtentWord {
            role,
            parameter_index,
            field,
            source_register,
            source_byte_offset,
            outgoing_stack_byte_offset,
        } if *role == expected_role
            && *parameter_index == expected_parameter_index
            && *field == expected_field
            && *source_register == expected_register
            && *source_byte_offset == expected_source_offset
            && *outgoing_stack_byte_offset == expected_stack_offset
    )
}

fn replay_bind(
    step: &OptimizedProgramStorageSemanticWrapperStep,
    expected_role: ProgramStorageEntryRootRole,
    expected_parameter_index: usize,
    expected_register: MachineRegister,
    expected_stack_offset: u32,
) -> bool {
    matches!(
        step,
        OptimizedProgramStorageSemanticWrapperStep::BindOutgoingExtentCopyAddress {
            role,
            parameter_index,
            register,
            outgoing_stack_byte_offset,
            byte_count,
            alignment,
        } if *role == expected_role
            && *parameter_index == expected_parameter_index
            && *register == expected_register
            && *outgoing_stack_byte_offset == expected_stack_offset
            && *byte_count == EXTENT_BYTE_COUNT
            && *alignment == EXTENT_ALIGNMENT
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ProgramEntryPhysicalContractPlan, ProgramEntrySourceExtentValueLayout,
        ProgramEntrySourceReceiverSignature, SelectedProgramEntrySourceSignature,
        SelectedProgramStorageEntryPlan, bind_optimized_program_storage_semantic_entry_contract,
    };
    use omega_calling_conventions::{
        CallSignature, ValidatedBoundaryEntryPlan, ValueShape,
        evaluate_ordinary_boundary_entry_plan,
    };
    use omega_effects::provider_plan::{
        ServiceEntryAuthorityFlow, ServiceEntryClaim, ServiceMethod, ServiceSchema,
    };
    use psi_language_semantics::{CarryPolicy, DomainPredicateBody};
    use psi_symbols::SymbolHandle;

    const REQUIREMENT: &str = "ProgramStorageEntry::enter#recipe";
    const EXTENT_CARRIER: &str = "named(name(Extent))";
    const GRANTED_DOMAIN: &str = "Extent::Granted";
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

    fn contract() -> OptimizedProgramStorageSemanticEntryContract {
        let slot = omega_target::TargetProfile::UefiX64.program_entry_slot();
        let semantic = semantic();
        let claim = |parameter_index| ServiceEntryClaim {
            parameter_index,
            carrier_identity: EXTENT_CARRIER.into(),
            domain: GRANTED_DOMAIN.into(),
            predicate_body: DomainPredicateBody::Present,
            effective_carry: CarryPolicy::STRICT,
            authority_flow: ServiceEntryAuthorityFlow::Accepts,
        };
        let selected = SelectedProgramStorageEntryPlan::from_target_slot(
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
        let selected = selected
            .with_physical_contract(
                ProgramEntryPhysicalContractPlan::new(
                    slot,
                    "UefiPhysicalEntry::enter#recipe".into(),
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
            "Boot::launch#recipe".into(),
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
        bind_optimized_program_storage_semantic_entry_contract(
            omega_target::NativeTarget::uefi_x64(),
            &selected,
            &source,
            semantic.plan(),
        )
        .unwrap()
    }

    #[test]
    fn exact_semantic_wrapper_recipe_is_address_free_and_balanced() {
        let contract = contract();
        let fingerprint = contract.semantic_calling_plan_fingerprint();
        let source_identity = contract.source_signature_identity();
        let plan = plan_optimized_program_storage_semantic_wrapper(contract).unwrap();

        validate_optimized_program_storage_semantic_wrapper(&plan).unwrap();
        assert_eq!(plan.source_signature_identity(), source_identity);
        assert_eq!(plan.shadow_byte_count(), 32);
        assert_eq!(plan.outgoing_frame_byte_count(), 72);
        assert_eq!(plan.outgoing_release_byte_count(), 72);
        assert_eq!(plan.pre_call_stack_alignment(), 16);
        assert_eq!(plan.steps(), &expected_steps(fingerprint));
        assert_eq!(plan.relocation(), &expected_relocation());
        assert_eq!(plan.relocation().call_step_index(), 8);
        assert_eq!(
            plan.relocation().call_instruction_function_byte_offset(),
            113
        );
        assert_eq!(plan.relocation().relocation_function_byte_offset(), 114);
        assert_eq!(plan.relocation().byte_width(), 4);
        assert_eq!(plan.relocation().addend(), 0);
        assert_eq!(
            plan.physical_disposition(),
            OptimizedProgramStoragePhysicalEntryDisposition::PlannedNotInvokedV1
        );
    }

    #[test]
    fn step_order_root_register_and_frame_corruption_fail_closed() {
        let mut plan = plan_optimized_program_storage_semantic_wrapper(contract()).unwrap();
        plan.steps.swap(2, 4);
        assert!(validate_optimized_program_storage_semantic_wrapper(&plan).is_err());

        let mut plan = plan_optimized_program_storage_semantic_wrapper(contract()).unwrap();
        plan.steps[2] = copy(
            ProgramStorageEntryRootRole::Image,
            0,
            ProgramEntrySourceExtentFieldRole::Base,
            MachineRegister::X86Rdx,
            0,
            32,
        );
        assert!(validate_optimized_program_storage_semantic_wrapper(&plan).is_err());

        let mut plan = plan_optimized_program_storage_semantic_wrapper(contract()).unwrap();
        plan.outgoing_release_byte_count = 88;
        assert!(validate_optimized_program_storage_semantic_wrapper(&plan).is_err());

        let mut plan = plan_optimized_program_storage_semantic_wrapper(contract()).unwrap();
        plan.steps[9] = OptimizedProgramStorageSemanticWrapperStep::ReleaseOutgoingStackFrame {
            byte_count: 56,
        };
        assert!(validate_optimized_program_storage_semantic_wrapper(&plan).is_err());
    }

    #[test]
    fn private_call_fingerprint_and_relocation_corruption_fail_closed() {
        let mut plan = plan_optimized_program_storage_semantic_wrapper(contract()).unwrap();
        plan.steps[CALL_STEP_INDEX] =
            OptimizedProgramStorageSemanticWrapperStep::CallPrivateTerminalContinuation {
                calling_policy: CallingPolicy::MicrosoftX64,
                semantic_calling_plan_fingerprint: 0,
                disposition: OptimizedProgramStorageSemanticWrapperContinuationDisposition::PrivateTerminalSymbolRequiredV1,
            };
        assert!(validate_optimized_program_storage_semantic_wrapper(&plan).is_err());

        for corrupt in [
            |relocation: &mut OptimizedProgramStorageSemanticWrapperRelocationRequirement| {
                relocation.relocation_function_byte_offset = 115;
            },
            |relocation: &mut OptimizedProgramStorageSemanticWrapperRelocationRequirement| {
                relocation.byte_width = 8;
            },
            |relocation: &mut OptimizedProgramStorageSemanticWrapperRelocationRequirement| {
                relocation.addend = -4;
            },
        ] {
            let mut plan = plan_optimized_program_storage_semantic_wrapper(contract()).unwrap();
            corrupt(&mut plan.relocation);
            assert!(validate_optimized_program_storage_semantic_wrapper(&plan).is_err());
        }
    }
}

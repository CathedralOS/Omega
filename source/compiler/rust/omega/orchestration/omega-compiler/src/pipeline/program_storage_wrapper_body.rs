//! Address-free body template for the receiver-free program-storage wrapper.
//!
//! The exact source continuation ABI and its encoded inbound realization do
//! not coexist until after the first backend pass. This sealed carrier joins
//! those facts and records the compiler-private operation sequence that a
//! later phase-aligned insertion pass must lower. It deliberately does not
//! claim that a function, call, relocation, object entry, or native execution
//! exists.
//!
//! Installation-owned caller-frame/operand carriers are not inputs here:
//! wrapper code must copy launch-time values arriving through RCX/RDX, never
//! bake recorded installation geometry into executable bytes.

use super::{
    ProgramStorageEntryContinuationAbiPlan, ProgramStorageEntryContinuationInboundPlan,
    ProgramStorageEntryContinuationReceiverAbiPlan, ProgramStorageEntryDiagnostic,
    ProgramStorageEntryRootRole, ProgramStorageEntryWrapperReceiverTransfer,
    ProgramStorageEntryWrapperTransferPlan,
};
use omega_calling_conventions::{IndirectPointerLocation, MachineRegister};
use omega_control_flow::MachineFunctionIdentity;
use std::ops::Range;
use std::sync::Arc;

const FRAME_BYTE_COUNT: u32 = 72;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProgramStorageEntryWrapperBodyTemplateStep {
    EnterFunction,
    ReserveOutgoingStackFrame {
        byte_count: u32,
    },
    CopyEntryIndirectU64ToOutgoingStack {
        role: ProgramStorageEntryRootRole,
        source_register: MachineRegister,
        source_byte_offset: u32,
        stack_byte_offset: u32,
    },
    LoadOutgoingStackAddress {
        role: ProgramStorageEntryRootRole,
        register: MachineRegister,
        stack_byte_offset: u32,
    },
    CallSourceContinuation {
        target: MachineFunctionIdentity,
    },
    ReleaseOutgoingStackFrame {
        byte_count: u32,
    },
    ReturnUnit,
}

/// Exact post-encoding template that a future phase-aligned backend pass can
/// insert as one compiler-generated function.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramStorageEntryWrapperBodyTemplatePlan {
    target: omega_target::NativeTarget,
    wrapper_identity: MachineFunctionIdentity,
    wrapper_symbol: String,
    continuation_identity: MachineFunctionIdentity,
    continuation_symbol: String,
    continuation_text_range: Range<usize>,
    steps: [ProgramStorageEntryWrapperBodyTemplateStep; 11],
}

impl ProgramStorageEntryWrapperBodyTemplatePlan {
    pub const fn target(&self) -> omega_target::NativeTarget {
        self.target
    }

    pub const fn wrapper_identity(&self) -> MachineFunctionIdentity {
        self.wrapper_identity
    }

    pub fn wrapper_symbol(&self) -> &str {
        &self.wrapper_symbol
    }

    pub const fn continuation_identity(&self) -> MachineFunctionIdentity {
        self.continuation_identity
    }

    pub fn continuation_symbol(&self) -> &str {
        &self.continuation_symbol
    }

    pub const fn continuation_text_range(&self) -> &Range<usize> {
        &self.continuation_text_range
    }

    pub const fn steps(&self) -> &[ProgramStorageEntryWrapperBodyTemplateStep; 11] {
        &self.steps
    }
}

pub(super) fn plan_program_storage_entry_wrapper_body_template(
    transfer: &ProgramStorageEntryWrapperTransferPlan,
    abi: &ProgramStorageEntryContinuationAbiPlan,
    inbound: &ProgramStorageEntryContinuationInboundPlan,
) -> Result<ProgramStorageEntryWrapperBodyTemplatePlan, ProgramStorageEntryDiagnostic> {
    let wrapper_identity = transfer.wrapper_identity();
    let continuation_identity = transfer.continuation_identity();
    let wrapper_symbol = omega_object_file::entry_symbol_name(inbound.target());
    if transfer.receiver() != &ProgramStorageEntryWrapperReceiverTransfer::Free
        || abi.receiver() != &ProgramStorageEntryContinuationReceiverAbiPlan::Free
        || inbound.target() != omega_target::NativeTarget::uefi_x64()
        || abi.target() != inbound.target()
        || wrapper_identity
            .program_storage_entry_continuation()
            .is_none()
        || continuation_identity.source_key().is_none()
        || abi.continuation_identity() != continuation_identity
        || inbound.continuation_identity() != continuation_identity
        || abi.normalized_callable_identity() != inbound.normalized_callable_identity()
        || abi.call() != inbound.call()
        || inbound.call().result.is_some()
    {
        return Err(ProgramStorageEntryDiagnostic(
            "program-storage wrapper body template requires one exact receiver-free UEFI/Microsoft source ABI"
                .into(),
        ));
    }
    let [image_transfer, storage_transfer] = transfer.roots();
    let [image, storage] = inbound.arguments();
    for (index, (transfer_root, argument, role, register)) in [
        (
            image_transfer,
            image,
            ProgramStorageEntryRootRole::Image,
            MachineRegister::X86Rcx,
        ),
        (
            storage_transfer,
            storage,
            ProgramStorageEntryRootRole::InitialStorage,
            MachineRegister::X86Rdx,
        ),
    ]
    .into_iter()
    .enumerate()
    {
        if transfer_root.role() != role
            || transfer_root.source_parameter_index() != index
            || argument.role() != role
            || argument.visible_parameter_index() != index
            || argument.call_parameter_index() != index
            || argument.pointer() != IndirectPointerLocation::Register(register)
            || argument.shape().byte_size != 16
            || argument.shape().alignment != 8
            || abi.call().parameters.get(index) != Some(argument.placement())
        {
            return Err(ProgramStorageEntryDiagnostic(format!(
                "program-storage wrapper body {role:?} transfer drifted from its exact inbound ABI row"
            )));
        }
    }
    if inbound.continuation_symbol().is_empty() || inbound.continuation_text_range().is_empty() {
        return Err(ProgramStorageEntryDiagnostic(
            "program-storage wrapper body lost the exact nonempty source function interval".into(),
        ));
    }

    let steps = expected_steps(continuation_identity);
    let plan = ProgramStorageEntryWrapperBodyTemplatePlan {
        target: inbound.target(),
        wrapper_identity,
        wrapper_symbol,
        continuation_identity,
        continuation_symbol: inbound.continuation_symbol().to_owned(),
        continuation_text_range: inbound.continuation_text_range().clone(),
        steps: steps.clone(),
    };
    validate_template(&plan, &steps)?;
    Ok(plan)
}

fn expected_steps(
    continuation: MachineFunctionIdentity,
) -> [ProgramStorageEntryWrapperBodyTemplateStep; 11] {
    use ProgramStorageEntryWrapperBodyTemplateStep::*;
    [
        EnterFunction,
        ReserveOutgoingStackFrame {
            byte_count: FRAME_BYTE_COUNT,
        },
        CopyEntryIndirectU64ToOutgoingStack {
            role: ProgramStorageEntryRootRole::Image,
            source_register: MachineRegister::X86Rcx,
            source_byte_offset: 0,
            stack_byte_offset: 32,
        },
        CopyEntryIndirectU64ToOutgoingStack {
            role: ProgramStorageEntryRootRole::Image,
            source_register: MachineRegister::X86Rcx,
            source_byte_offset: 8,
            stack_byte_offset: 40,
        },
        CopyEntryIndirectU64ToOutgoingStack {
            role: ProgramStorageEntryRootRole::InitialStorage,
            source_register: MachineRegister::X86Rdx,
            source_byte_offset: 0,
            stack_byte_offset: 48,
        },
        CopyEntryIndirectU64ToOutgoingStack {
            role: ProgramStorageEntryRootRole::InitialStorage,
            source_register: MachineRegister::X86Rdx,
            source_byte_offset: 8,
            stack_byte_offset: 56,
        },
        LoadOutgoingStackAddress {
            role: ProgramStorageEntryRootRole::Image,
            register: MachineRegister::X86Rcx,
            stack_byte_offset: 32,
        },
        LoadOutgoingStackAddress {
            role: ProgramStorageEntryRootRole::InitialStorage,
            register: MachineRegister::X86Rdx,
            stack_byte_offset: 48,
        },
        CallSourceContinuation {
            target: continuation,
        },
        ReleaseOutgoingStackFrame {
            byte_count: FRAME_BYTE_COUNT,
        },
        ReturnUnit,
    ]
}

fn validate_template(
    plan: &ProgramStorageEntryWrapperBodyTemplatePlan,
    expected: &[ProgramStorageEntryWrapperBodyTemplateStep; 11],
) -> Result<(), ProgramStorageEntryDiagnostic> {
    if plan.target != omega_target::NativeTarget::uefi_x64()
        || plan.wrapper_identity.program_storage_entry_continuation()
            != plan.continuation_identity.source_key()
        || plan.wrapper_symbol != omega_object_file::entry_symbol_name(plan.target)
        || plan.continuation_symbol.is_empty()
        || plan.continuation_text_range.is_empty()
        || plan.steps != *expected
    {
        return Err(ProgramStorageEntryDiagnostic(
            "program-storage wrapper body template identity, interval, or exact operation sequence drifted"
                .into(),
        ));
    }
    Ok(())
}

/// Consume the retained phase-alignment facts through the backend's exact
/// second-pass lowering seam, then independently replay the generated
/// function, object linkage, and source-call relocation before the caller may
/// publish the rebuilt plan.
pub(super) fn insert_and_validate_program_storage_entry_wrapper(
    template: &ProgramStorageEntryWrapperBodyTemplatePlan,
    backend: &mut omega_backend_plan::BackendPlan,
) -> Result<(), ProgramStorageEntryDiagnostic> {
    validate_template(template, &expected_steps(template.continuation_identity))?;
    omega_backend_pipeline::insert_program_storage_entry_wrapper(
        backend,
        omega_backend_pipeline::ProgramStorageEntryWrapperInsertion {
            wrapper_identity: template.wrapper_identity,
            wrapper_symbol: Arc::from(template.wrapper_symbol.as_str()),
            continuation_identity: template.continuation_identity,
        },
    )
    .map_err(|error| ProgramStorageEntryDiagnostic(error.message))?;
    replay_emitted_wrapper(template, backend)
}

fn replay_emitted_wrapper(
    template: &ProgramStorageEntryWrapperBodyTemplatePlan,
    backend: &omega_backend_plan::BackendPlan,
) -> Result<(), ProgramStorageEntryDiagnostic> {
    use omega_object_file::{RelocationKind, RelocationOrigin, SectionKind};

    let mut wrappers = backend
        .encoded_machine
        .code
        .functions
        .iter()
        .filter(|(_, function)| function.identity == template.wrapper_identity);
    let Some((_, wrapper)) = wrappers.next() else {
        return Err(ProgramStorageEntryDiagnostic(
            "inserted program-storage wrapper has no encoded function".into(),
        ));
    };
    if wrappers.next().is_some()
        || wrapper.symbol.as_ref() != template.wrapper_symbol
        || wrapper.byte_count == 0
    {
        return Err(ProgramStorageEntryDiagnostic(
            "inserted program-storage wrapper identity or entry symbol is ambiguous".into(),
        ));
    }
    let instructions = backend
        .encoded_machine
        .code
        .instructions
        .span(wrapper.instructions)
        .ok_or_else(|| {
            ProgramStorageEntryDiagnostic(
                "inserted program-storage wrapper has an invalid instruction span".into(),
            )
        })?;
    if instructions.len() != template.steps.len() {
        return Err(ProgramStorageEntryDiagnostic(format!(
            "inserted program-storage wrapper has {} instruction rows instead of {}",
            instructions.len(),
            template.steps.len()
        )));
    }
    let wrapper_byte_end = wrapper
        .byte_offset
        .checked_add(wrapper.byte_count)
        .ok_or_else(|| {
            ProgramStorageEntryDiagnostic(
                "inserted program-storage wrapper byte interval overflows".into(),
            )
        })?;
    let encoded_bytes = backend
        .encoded_machine
        .code
        .bytes
        .storage_slice()
        .get(wrapper.byte_offset..wrapper_byte_end)
        .ok_or_else(|| {
            ProgramStorageEntryDiagnostic(
                "inserted program-storage wrapper byte interval is invalid".into(),
            )
        })?;
    let retained_kinds = instructions
        .iter()
        .map(|instruction| {
            instruction.compiler_validation_kind.clone().ok_or_else(|| {
                ProgramStorageEntryDiagnostic(
                    "inserted program-storage wrapper lost a validation row".into(),
                )
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    validate_emitted_rows(template, &retained_kinds, encoded_bytes)?;

    let (wrapper_symbol_handle, wrapper_symbol) =
        omega_object_file::object_function_symbol(&backend.object, template.wrapper_identity)
            .ok_or_else(|| {
                ProgramStorageEntryDiagnostic(
                    "inserted program-storage wrapper has no exact object entry linkage".into(),
                )
            })?;
    let (source_symbol_handle, source_symbol) =
        omega_object_file::object_function_symbol(&backend.object, template.continuation_identity)
            .ok_or_else(|| {
                ProgramStorageEntryDiagnostic(
                    "inserted program-storage wrapper call target has no exact Source linkage"
                        .into(),
                )
            })?;
    if wrapper_symbol_handle != backend.object.layout.entry_symbol
        || wrapper_symbol.name != template.wrapper_symbol
        || (source_symbol.offset..source_symbol.offset + source_symbol.size)
            != template.continuation_text_range
    {
        return Err(ProgramStorageEntryDiagnostic(
            "inserted wrapper entry or retained Source interval drifted during object planning"
                .into(),
        ));
    }
    let call = &instructions[8];
    let call_byte_offset = call.bytes.start().arena_index() as usize - 1;
    let call_index = call.selected_instruction_index;
    let matches = backend
        .relocations
        .records()
        .filter(|(_, record)| {
            record.origin
                == RelocationOrigin::Instruction {
                    function_symbol_handle: wrapper_symbol_handle,
                    selected_instruction_index: call_index,
                }
        })
        .map(|(_, record)| record)
        .collect::<Vec<_>>();
    let [relocation] = matches.as_slice() else {
        return Err(ProgramStorageEntryDiagnostic(
            "inserted wrapper call does not own exactly one relocation".into(),
        ));
    };
    if relocation.section != SectionKind::Text
        || relocation.offset != call_byte_offset + 1
        || relocation.byte_width != 4
        || relocation.symbol_handle != source_symbol_handle
        || relocation.addend != 0
        || relocation.kind != RelocationKind::X86_64Relative32
    {
        return Err(ProgramStorageEntryDiagnostic(
            "inserted wrapper call relocation drifted from its exact Source target".into(),
        ));
    }
    Ok(())
}

fn expected_validation_kinds(
    continuation: MachineFunctionIdentity,
) -> [omega_machine_bytes::CompilerInstructionValidationKind; 11] {
    use omega_machine_bytes::CompilerInstructionValidationKind as Kind;
    [
        Kind::FunctionEnter,
        Kind::OutgoingStackFrameReserve { byte_count: 72 },
        Kind::EntryIndirectU64ToOutgoingStackCopy {
            source_register: MachineRegister::X86Rcx,
            source_byte_offset: 0,
            stack_byte_offset: 32,
        },
        Kind::EntryIndirectU64ToOutgoingStackCopy {
            source_register: MachineRegister::X86Rcx,
            source_byte_offset: 8,
            stack_byte_offset: 40,
        },
        Kind::EntryIndirectU64ToOutgoingStackCopy {
            source_register: MachineRegister::X86Rdx,
            source_byte_offset: 0,
            stack_byte_offset: 48,
        },
        Kind::EntryIndirectU64ToOutgoingStackCopy {
            source_register: MachineRegister::X86Rdx,
            source_byte_offset: 8,
            stack_byte_offset: 56,
        },
        Kind::OutgoingStackAddressLoad {
            register: MachineRegister::X86Rcx,
            stack_byte_offset: 32,
        },
        Kind::OutgoingStackAddressLoad {
            register: MachineRegister::X86Rdx,
            stack_byte_offset: 48,
        },
        Kind::InternalFunctionCall {
            target: continuation,
        },
        Kind::OutgoingStackFrameRelease { byte_count: 72 },
        Kind::FunctionReturn,
    ]
}

fn expected_emitted_bytes() -> Result<Vec<u8>, ProgramStorageEntryDiagnostic> {
    let mut bytes = Vec::new();
    bytes.extend(omega_isa_x86_64::encode_function_enter_bytes());
    bytes.extend(
        omega_isa_x86_64::encode_outgoing_stack_frame_reserve_bytes(72)
            .map_err(|error| ProgramStorageEntryDiagnostic(error.message))?,
    );
    for (source_register, source_byte_offset, stack_byte_offset) in [
        (MachineRegister::X86Rcx, 0, 32),
        (MachineRegister::X86Rcx, 8, 40),
        (MachineRegister::X86Rdx, 0, 48),
        (MachineRegister::X86Rdx, 8, 56),
    ] {
        bytes.extend(
            omega_isa_x86_64::encode_entry_indirect_u64_to_outgoing_stack_copy_bytes(
                source_register,
                source_byte_offset,
                stack_byte_offset,
            )
            .map_err(|error| ProgramStorageEntryDiagnostic(error.message))?,
        );
    }
    bytes.extend(
        omega_isa_x86_64::encode_outgoing_stack_address_load_bytes(MachineRegister::X86Rcx, 32)
            .map_err(|error| ProgramStorageEntryDiagnostic(error.message))?,
    );
    bytes.extend(
        omega_isa_x86_64::encode_outgoing_stack_address_load_bytes(MachineRegister::X86Rdx, 48)
            .map_err(|error| ProgramStorageEntryDiagnostic(error.message))?,
    );
    bytes.extend(omega_isa_x86_64::encode_internal_function_call_bytes());
    bytes.extend(
        omega_isa_x86_64::encode_outgoing_stack_frame_release_bytes(72)
            .map_err(|error| ProgramStorageEntryDiagnostic(error.message))?,
    );
    bytes.extend(omega_isa_x86_64::encode_return_bytes());
    Ok(bytes)
}

fn validate_emitted_rows(
    template: &ProgramStorageEntryWrapperBodyTemplatePlan,
    retained_kinds: &[omega_machine_bytes::CompilerInstructionValidationKind],
    encoded_bytes: &[u8],
) -> Result<(), ProgramStorageEntryDiagnostic> {
    if retained_kinds != expected_validation_kinds(template.continuation_identity) {
        return Err(ProgramStorageEntryDiagnostic(
            "inserted program-storage wrapper validation rows drifted from the template".into(),
        ));
    }
    if encoded_bytes != expected_emitted_bytes()? {
        return Err(ProgramStorageEntryDiagnostic(
            "inserted program-storage wrapper bytes drifted from the canonical template".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use psi_symbols::SymbolHandle;

    fn template() -> ProgramStorageEntryWrapperBodyTemplatePlan {
        let key = omega_control_flow::StateKey {
            machine: SymbolHandle::from_arena_index(1),
            state: SymbolHandle::from_arena_index(2),
            segment_index: 0,
        };
        let continuation_identity = MachineFunctionIdentity::source(key);
        let wrapper_identity = MachineFunctionIdentity::program_storage_entry_wrapper(key).unwrap();
        ProgramStorageEntryWrapperBodyTemplatePlan {
            target: omega_target::NativeTarget::uefi_x64(),
            wrapper_identity,
            wrapper_symbol: omega_object_file::entry_symbol_name(
                omega_target::NativeTarget::uefi_x64(),
            ),
            continuation_identity,
            continuation_symbol: "__omega_source".into(),
            continuation_text_range: 16..64,
            steps: expected_steps(continuation_identity),
        }
    }

    #[test]
    fn exact_phase_alignment_template_is_admitted() {
        let template = template();
        validate_template(&template, &expected_steps(template.continuation_identity)).unwrap();
    }

    #[test]
    fn identity_call_order_and_frame_corruption_reject() {
        let exact = template();
        for (index, replacement) in [
            (
                2,
                ProgramStorageEntryWrapperBodyTemplateStep::CopyEntryIndirectU64ToOutgoingStack {
                    role: ProgramStorageEntryRootRole::Image,
                    source_register: MachineRegister::X86Rdx,
                    source_byte_offset: 0,
                    stack_byte_offset: 32,
                },
            ),
            (
                8,
                ProgramStorageEntryWrapperBodyTemplateStep::CallSourceContinuation {
                    target: exact.wrapper_identity,
                },
            ),
            (
                9,
                ProgramStorageEntryWrapperBodyTemplateStep::ReleaseOutgoingStackFrame {
                    byte_count: 88,
                },
            ),
        ] {
            let mut drifted = exact.clone();
            drifted.steps[index] = replacement;
            assert!(
                validate_template(&drifted, &expected_steps(exact.continuation_identity)).is_err()
            );
        }

        let mut wrong_identity = exact.clone();
        wrong_identity.continuation_identity = wrong_identity.wrapper_identity;
        assert!(
            validate_template(
                &wrong_identity,
                &expected_steps(exact.continuation_identity)
            )
            .is_err()
        );
    }

    #[test]
    fn emitted_opcode_validation_and_row_count_corruption_reject() {
        let template = template();
        let exact_kinds = expected_validation_kinds(template.continuation_identity);
        let exact_bytes = expected_emitted_bytes().unwrap();
        validate_emitted_rows(&template, &exact_kinds, &exact_bytes).unwrap();

        let mut opcode_tamper = exact_bytes.clone();
        opcode_tamper[0] ^= 1;
        assert!(validate_emitted_rows(&template, &exact_kinds, &opcode_tamper).is_err());

        let mut target_tamper = exact_kinds.clone();
        target_tamper[8] =
            omega_machine_bytes::CompilerInstructionValidationKind::InternalFunctionCall {
                target: template.wrapper_identity,
            };
        assert!(validate_emitted_rows(&template, &target_tamper, &exact_bytes).is_err());
        assert!(validate_emitted_rows(&template, &exact_kinds[..10], &exact_bytes).is_err());
    }
}

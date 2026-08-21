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
    let wrapper_symbol = omega_object_file::private_function_symbol_name(wrapper_identity)
        .ok_or_else(|| {
            ProgramStorageEntryDiagnostic(
                "program-storage wrapper body template has no canonical private symbol".into(),
            )
        })?;
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
        || plan.wrapper_symbol
            != omega_object_file::private_function_symbol_name(plan.wrapper_identity)
                .unwrap_or_default()
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
            wrapper_symbol: omega_object_file::private_function_symbol_name(wrapper_identity)
                .unwrap(),
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
}

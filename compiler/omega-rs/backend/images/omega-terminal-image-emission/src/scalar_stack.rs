//! Top-level scalar-stack evidence validation.
//!
//! This module dispatches retained control-flow forms to their exact replay
//! owners and validates balanced linear x86-64/AArch64 stack mutations, calls,
//! and returns. It does not infer stack evidence, select ABI policy, or emit
//! instructions.

use omega_target::Architecture;
use omega_terminal_machine_code::{
    TerminalScalarControlAffineCleanupRecord, TerminalScalarControlFlowEvidence,
    TerminalScalarStackEvidence,
};
use psi_core::MachineId;

use super::scalar_call_stack::validate_scalar_call_stack;
use super::scalar_conditional_stack::validate_conditional_tree_scalar_stack;
use super::scalar_division_stack::validate_linear_scalar_division_stack;
use super::scalar_shared_convergence::validate_boolean_shared_convergence_stack;
use super::scalar_stack_mutation::{
    aarch64_control_flow_instruction, aarch64_unsupported_sp_write, replay_scalar_mutation,
    validate_aarch64_scalar_mutation, validate_x86_scalar_mutation,
};
use super::unit_stack::aarch64_stack_adjustment_at;
use super::{
    TerminalObjectError, TerminalObjectScalarCallStack, TerminalObjectScalarStack,
    validate_internal_call_site,
};

pub(super) fn validate_scalar_stack(
    architecture: Architecture,
    machine: MachineId,
    bytes: &[u8],
    calls: &[omega_terminal_machine_code::TerminalInternalCallRelocation],
    evidence: &TerminalScalarStackEvidence,
    scalar_affine_cleanup: Option<&omega_terminal_machine_code::TerminalUnitAffineCleanupRecord>,
    scalar_control_affine_cleanups: &[TerminalScalarControlAffineCleanupRecord],
    scalar_structural_parameter_homes: &[omega_terminal_machine_code::TerminalUnitParameterHomeRecord],
) -> Result<
    (
        TerminalObjectScalarStack,
        Vec<TerminalObjectScalarCallStack>,
    ),
    TerminalObjectError,
> {
    if evidence.stack_alignment != 16 {
        return Err(TerminalObjectError::InvalidScalarStackAlignment {
            machine,
            alignment: evidence.stack_alignment,
        });
    }
    if let TerminalScalarControlFlowEvidence::BooleanSharedConvergence {
        decisions,
        joins,
        structural_conditions,
        merge_offset,
    } = &evidence.control_flow
    {
        if scalar_affine_cleanup.is_none() || !scalar_control_affine_cleanups.is_empty() {
            return Err(TerminalObjectError::InvalidUnitAffineCleanupEvidence(
                machine,
            ));
        }
        return validate_boolean_shared_convergence_stack(
            architecture,
            machine,
            bytes,
            calls,
            evidence,
            decisions,
            joins,
            structural_conditions,
            *merge_offset,
            scalar_affine_cleanup,
            scalar_structural_parameter_homes,
        );
    }
    if let TerminalScalarControlFlowEvidence::ConditionalTree {
        decisions,
        crash_leaves,
        branches,
    } = &evidence.control_flow
    {
        if scalar_affine_cleanup.is_some()
            || crash_leaves.iter().any(|crash| *crash) && !scalar_control_affine_cleanups.is_empty()
        {
            return Err(TerminalObjectError::InvalidUnitAffineCleanupEvidence(
                machine,
            ));
        }
        return validate_conditional_tree_scalar_stack(
            architecture,
            machine,
            bytes,
            calls,
            evidence,
            decisions,
            crash_leaves,
            branches,
            scalar_control_affine_cleanups,
        );
    }
    if let TerminalScalarControlFlowEvidence::LinearWithDivisionBranches { ref branches } =
        evidence.control_flow
    {
        if scalar_affine_cleanup.is_some() || !scalar_control_affine_cleanups.is_empty() {
            return Err(TerminalObjectError::InvalidUnitAffineCleanupEvidence(
                machine,
            ));
        }
        return validate_linear_scalar_division_stack(
            architecture,
            machine,
            bytes,
            calls,
            evidence,
            branches,
        );
    }
    if !scalar_control_affine_cleanups.is_empty() {
        return Err(TerminalObjectError::InvalidUnitAffineCleanupEvidence(
            machine,
        ));
    }
    if evidence
        .mutations
        .windows(2)
        .any(|pair| pair[0].offset >= pair[1].offset)
    {
        return Err(TerminalObjectError::NonCanonicalScalarStackMutationOrder(
            machine,
        ));
    }
    let mut claimed = evidence
        .mutations
        .iter()
        .map(|mutation| (mutation.offset, *mutation))
        .collect::<std::collections::BTreeMap<_, _>>();
    if claimed.len() != evidence.mutations.len() {
        return Err(TerminalObjectError::NonCanonicalScalarStackMutationOrder(
            machine,
        ));
    }
    let mut call_sites = std::collections::BTreeMap::new();
    for call in calls {
        validate_internal_call_site(architecture, machine, bytes, *call)?;
        let call_start = match architecture {
            Architecture::X86_64 => call.offset - 1,
            Architecture::Aarch64 => call.offset,
        };
        call_sites.insert(call_start, *call);
    }
    let mut validated_calls = Vec::with_capacity(calls.len());
    let mut depth = 0_u32;
    let mut peak = 0_u32;
    match architecture {
        Architecture::X86_64 => {
            let mut decoder =
                iced_x86::Decoder::with_ip(64, bytes, 0, iced_x86::DecoderOptions::NONE);
            let mut info_factory = iced_x86::InstructionInfoFactory::new();
            let mut saw_return = false;
            while decoder.can_decode() {
                let instruction = decoder.decode();
                let offset = usize::try_from(instruction.ip()).expect("function-relative x86 IP");
                if instruction.is_invalid() {
                    return Err(TerminalObjectError::InvalidScalarInstructionEncoding {
                        machine,
                        offset,
                    });
                }
                if instruction.mnemonic() == iced_x86::Mnemonic::Ret {
                    if offset.checked_add(instruction.len()) != Some(bytes.len()) || saw_return {
                        return Err(TerminalObjectError::NonLinearScalarControlFlow {
                            machine,
                            offset,
                        });
                    }
                    saw_return = true;
                    continue;
                }
                if instruction.mnemonic() == iced_x86::Mnemonic::Call {
                    let call = call_sites.remove(&offset).ok_or(
                        TerminalObjectError::UntypedScalarInternalCall { machine, offset },
                    )?;
                    let call_evidence = call.scalar_stack.ok_or(
                        TerminalObjectError::MissingScalarCallStackEvidence {
                            caller: machine,
                            owner: call.owner,
                        },
                    )?;
                    let validated = validate_scalar_call_stack(
                        architecture,
                        machine,
                        bytes,
                        call,
                        call_evidence,
                        evidence,
                        depth,
                        scalar_affine_cleanup,
                    )?;
                    peak = peak.max(validated.caller_live_bytes);
                    validated_calls.push(validated);
                    continue;
                }
                if instruction.flow_control() != iced_x86::FlowControl::Next {
                    return Err(TerminalObjectError::NonLinearScalarControlFlow {
                        machine,
                        offset,
                    });
                }
                let stack_kind = match instruction.mnemonic() {
                    iced_x86::Mnemonic::Sub
                        if instruction.op0_register() == iced_x86::Register::RSP =>
                    {
                        Some(true)
                    }
                    iced_x86::Mnemonic::Add
                        if instruction.op0_register() == iced_x86::Register::RSP =>
                    {
                        Some(false)
                    }
                    iced_x86::Mnemonic::Push | iced_x86::Mnemonic::Pop => Some(false),
                    _ => None,
                };
                if stack_kind.is_some() {
                    let mutation = claimed.remove(&offset).ok_or(
                        TerminalObjectError::UnclaimedScalarStackMutation { machine, offset },
                    )?;
                    validate_x86_scalar_mutation(machine, bytes, &instruction, mutation)?;
                    replay_scalar_mutation(machine, offset, mutation.kind, &mut depth, &mut peak)?;
                    continue;
                }
                let info = info_factory.info(&instruction);
                if info.used_registers().iter().any(|register| {
                    matches!(
                        register.register(),
                        iced_x86::Register::RSP
                            | iced_x86::Register::ESP
                            | iced_x86::Register::SP
                            | iced_x86::Register::SPL
                    ) && matches!(
                        register.access(),
                        iced_x86::OpAccess::Write
                            | iced_x86::OpAccess::CondWrite
                            | iced_x86::OpAccess::ReadWrite
                            | iced_x86::OpAccess::ReadCondWrite
                    )
                }) {
                    return Err(TerminalObjectError::UnsupportedScalarStackMutation {
                        machine,
                        offset,
                    });
                }
            }
            if !saw_return {
                return Err(TerminalObjectError::MissingBalancedScalarReturn(machine));
            }
        }
        Architecture::Aarch64 => {
            if !bytes.len().is_multiple_of(4) {
                return Err(TerminalObjectError::InvalidScalarInstructionEncoding {
                    machine,
                    offset: bytes.len() - bytes.len() % 4,
                });
            }
            let mut saw_return = false;
            for offset in (0..bytes.len()).step_by(4) {
                let encoded = u32::from_le_bytes(
                    bytes[offset..offset + 4]
                        .try_into()
                        .expect("four-byte AArch64 word"),
                );
                if encoded == 0xd65f_03c0 {
                    if offset + 4 != bytes.len() || saw_return {
                        return Err(TerminalObjectError::NonLinearScalarControlFlow {
                            machine,
                            offset,
                        });
                    }
                    saw_return = true;
                    continue;
                }
                if encoded == 0x9400_0000 {
                    let call = call_sites.remove(&offset).ok_or(
                        TerminalObjectError::UntypedScalarInternalCall { machine, offset },
                    )?;
                    let call_evidence = call.scalar_stack.ok_or(
                        TerminalObjectError::MissingScalarCallStackEvidence {
                            caller: machine,
                            owner: call.owner,
                        },
                    )?;
                    let validated = validate_scalar_call_stack(
                        architecture,
                        machine,
                        bytes,
                        call,
                        call_evidence,
                        evidence,
                        depth,
                        scalar_affine_cleanup,
                    )?;
                    peak = peak.max(validated.caller_live_bytes);
                    validated_calls.push(validated);
                    continue;
                }
                if aarch64_control_flow_instruction(encoded) {
                    return Err(TerminalObjectError::NonLinearScalarControlFlow {
                        machine,
                        offset,
                    });
                }
                if aarch64_stack_adjustment_at(bytes, offset) {
                    let mutation = claimed.remove(&offset).ok_or(
                        TerminalObjectError::UnclaimedScalarStackMutation { machine, offset },
                    )?;
                    validate_aarch64_scalar_mutation(machine, encoded, mutation)?;
                    replay_scalar_mutation(machine, offset, mutation.kind, &mut depth, &mut peak)?;
                } else if aarch64_unsupported_sp_write(encoded) {
                    return Err(TerminalObjectError::UnsupportedScalarStackMutation {
                        machine,
                        offset,
                    });
                }
            }
            if !saw_return {
                return Err(TerminalObjectError::MissingBalancedScalarReturn(machine));
            }
        }
    }
    if let Some((&offset, _)) = claimed.first_key_value() {
        return Err(TerminalObjectError::InvalidScalarStackEvidence { machine, offset });
    }
    if let Some((_, call)) = call_sites.first_key_value() {
        return Err(TerminalObjectError::InvalidInternalCallSite {
            caller: machine,
            owner: call.owner,
            offset: call.offset,
        });
    }
    if depth != 0 {
        return Err(TerminalObjectError::MissingBalancedScalarReturn(machine));
    }
    Ok((
        TerminalObjectScalarStack {
            local_peak_bytes: peak,
            stack_alignment: evidence.stack_alignment,
        },
        validated_calls,
    ))
}

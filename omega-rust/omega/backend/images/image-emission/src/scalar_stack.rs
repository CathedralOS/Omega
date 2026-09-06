//! Top-level scalar-stack evidence validation.
//!
//! This module dispatches retained control-flow forms to their exact replay
//! owners and validates balanced linear x86-64/AArch64 stack mutations, calls,
//! and returns. It does not infer stack evidence, select ABI policy, or emit
//! instructions.

use machine_code::{
    ScalarControlAffineCleanupRecord, ScalarControlFlowEvidence, ScalarStackEvidence,
    SemanticCodeAttribution,
};
use semantic_vocabulary::MachineId;
use target::Architecture;
use target_operations::TerminalPsiProvenance;

use super::scalar_call_stack::validate_scalar_call_stack;
use super::scalar_conditional_stack::validate_conditional_tree_scalar_stack;
use super::scalar_division_stack::validate_linear_scalar_division_stack;
use super::scalar_shared_convergence::validate_boolean_shared_convergence_stack;
use super::scalar_stack_mutation::{
    aarch64_control_flow_instruction, aarch64_unsupported_sp_write, replay_scalar_mutation,
    validate_aarch64_scalar_mutation, validate_x86_scalar_mutation,
};
use super::unit_stack::aarch64_stack_adjustment_at;
use super::{ObjectError, ObjectScalarCallStack, ObjectScalarStack, validate_internal_call_site};

pub(super) fn validate_scalar_stack(
    architecture: Architecture,
    machine: MachineId,
    bytes: &[u8],
    calls: &[machine_code::InternalCallRelocation],
    dynamic_parameter_calls: &[machine_code::DynamicParameterCallRecord],
    provenance: &TerminalPsiProvenance,
    attribution: &[SemanticCodeAttribution],
    evidence: &ScalarStackEvidence,
    scalar_affine_cleanup: Option<&machine_code::UnitAffineCleanupRecord>,
    scalar_control_affine_cleanups: &[ScalarControlAffineCleanupRecord],
    scalar_structural_parameter_homes: &[machine_code::UnitParameterHomeRecord],
) -> Result<(ObjectScalarStack, Vec<ObjectScalarCallStack>), ObjectError> {
    if evidence.stack_alignment != 16 {
        return Err(ObjectError::InvalidScalarStackAlignment {
            machine,
            alignment: evidence.stack_alignment,
        });
    }
    if let ScalarControlFlowEvidence::Acyclic { blocks } = &evidence.control_flow {
        if !calls.is_empty()
            || !dynamic_parameter_calls.is_empty()
            || scalar_affine_cleanup.is_some()
            || !scalar_control_affine_cleanups.is_empty()
            || !scalar_structural_parameter_homes.is_empty()
        {
            return Err(ObjectError::InvalidScalarConditionalEvidence { machine, offset: 0 });
        }
        let reconstructed = super::scalar_control_flow::reconstruct_scalar_control_flow(
            architecture,
            machine,
            bytes,
            attribution.iter(),
        )?;
        if reconstructed != *blocks {
            return Err(ObjectError::InvalidScalarConditionalEvidence { machine, offset: 0 });
        }
        return super::scalar_control_flow::validate_stack(
            architecture,
            machine,
            bytes,
            evidence,
            blocks,
        )
        .map(|stack| (stack, Vec::new()));
    }
    if let ScalarControlFlowEvidence::BooleanSharedConvergence {
        decisions,
        joins,
        return_edges,
        fallthrough_return_edge,
        structural_conditions,
        merge_offset,
    } = &evidence.control_flow
    {
        if !dynamic_parameter_calls.is_empty() {
            return Err(ObjectError::InvalidDynamicParameterCallEvidence {
                caller: machine,
                operation: dynamic_parameter_calls[0].psi_operation,
            });
        }
        if scalar_affine_cleanup.is_none() || !scalar_control_affine_cleanups.is_empty() {
            return Err(ObjectError::InvalidUnitAffineCleanupEvidence(machine));
        }
        return validate_boolean_shared_convergence_stack(
            architecture,
            machine,
            bytes,
            calls,
            provenance,
            attribution,
            evidence,
            decisions,
            joins,
            return_edges,
            *fallthrough_return_edge,
            structural_conditions,
            *merge_offset,
            scalar_affine_cleanup,
            scalar_structural_parameter_homes,
        );
    }
    if let ScalarControlFlowEvidence::ConditionalTree {
        decisions,
        crash_leaves,
        branches,
    } = &evidence.control_flow
    {
        if !dynamic_parameter_calls.is_empty() {
            return Err(ObjectError::InvalidDynamicParameterCallEvidence {
                caller: machine,
                operation: dynamic_parameter_calls[0].psi_operation,
            });
        }
        if scalar_affine_cleanup.is_some()
            || crash_leaves.iter().any(|crash| *crash) && !scalar_control_affine_cleanups.is_empty()
        {
            return Err(ObjectError::InvalidUnitAffineCleanupEvidence(machine));
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
    if let ScalarControlFlowEvidence::LinearWithDivisionBranches { ref branches } =
        evidence.control_flow
    {
        if !dynamic_parameter_calls.is_empty() {
            return Err(ObjectError::InvalidDynamicParameterCallEvidence {
                caller: machine,
                operation: dynamic_parameter_calls[0].psi_operation,
            });
        }
        if scalar_affine_cleanup.is_some() || !scalar_control_affine_cleanups.is_empty() {
            return Err(ObjectError::InvalidUnitAffineCleanupEvidence(machine));
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
        return Err(ObjectError::InvalidUnitAffineCleanupEvidence(machine));
    }
    if evidence
        .mutations
        .windows(2)
        .any(|pair| pair[0].offset >= pair[1].offset)
    {
        return Err(ObjectError::NonCanonicalScalarStackMutationOrder(machine));
    }
    let mut claimed = evidence
        .mutations
        .iter()
        .map(|mutation| (mutation.offset, *mutation))
        .collect::<std::collections::BTreeMap<_, _>>();
    if claimed.len() != evidence.mutations.len() {
        return Err(ObjectError::NonCanonicalScalarStackMutationOrder(machine));
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
    let mut dynamic_call_sites = dynamic_parameter_calls
        .iter()
        .map(|call| (call.indirect_call_offset, call))
        .collect::<std::collections::BTreeMap<_, _>>();
    if dynamic_call_sites.len() != dynamic_parameter_calls.len() {
        return Err(ObjectError::InvalidDynamicParameterCallEvidence {
            caller: machine,
            operation: dynamic_parameter_calls[0].psi_operation,
        });
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
                    return Err(ObjectError::InvalidScalarInstructionEncoding { machine, offset });
                }
                if instruction.mnemonic() == iced_x86::Mnemonic::Ret {
                    if offset.checked_add(instruction.len()) != Some(bytes.len()) || saw_return {
                        return Err(ObjectError::NonLinearScalarControlFlow { machine, offset });
                    }
                    saw_return = true;
                    continue;
                }
                if instruction.mnemonic() == iced_x86::Mnemonic::Call {
                    if let Some(call) = dynamic_call_sites.remove(&offset) {
                        let caller_live_bytes = depth
                            .checked_add(8)
                            .ok_or(ObjectError::ScalarStackArithmeticOverflow(machine))?;
                        if !caller_live_bytes.is_multiple_of(evidence.stack_alignment) {
                            return Err(ObjectError::InvalidDynamicParameterCallEvidence {
                                caller: machine,
                                operation: call.psi_operation,
                            });
                        }
                        peak = peak.max(caller_live_bytes);
                        continue;
                    }
                    let call = call_sites
                        .remove(&offset)
                        .ok_or(ObjectError::UntypedScalarInternalCall { machine, offset })?;
                    let call_evidence =
                        call.scalar_stack
                            .ok_or(ObjectError::MissingScalarCallStackEvidence {
                                caller: machine,
                                owner: call.owner,
                            })?;
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
                    return Err(ObjectError::NonLinearScalarControlFlow { machine, offset });
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
                    let mutation = claimed
                        .remove(&offset)
                        .ok_or(ObjectError::UnclaimedScalarStackMutation { machine, offset })?;
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
                    return Err(ObjectError::UnsupportedScalarStackMutation { machine, offset });
                }
            }
            if !saw_return {
                return Err(ObjectError::MissingBalancedScalarReturn(machine));
            }
        }
        Architecture::Aarch64 => {
            if !bytes.len().is_multiple_of(4) {
                return Err(ObjectError::InvalidScalarInstructionEncoding {
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
                        return Err(ObjectError::NonLinearScalarControlFlow { machine, offset });
                    }
                    saw_return = true;
                    continue;
                }
                if encoded & 0xffff_fc1f == 0xd63f_0000 {
                    let call = dynamic_call_sites
                        .remove(&offset)
                        .ok_or(ObjectError::UntypedScalarInternalCall { machine, offset })?;
                    if !depth.is_multiple_of(evidence.stack_alignment) {
                        return Err(ObjectError::InvalidDynamicParameterCallEvidence {
                            caller: machine,
                            operation: call.psi_operation,
                        });
                    }
                    peak = peak.max(depth);
                    continue;
                }
                if encoded == 0x9400_0000 {
                    let call = call_sites
                        .remove(&offset)
                        .ok_or(ObjectError::UntypedScalarInternalCall { machine, offset })?;
                    let call_evidence =
                        call.scalar_stack
                            .ok_or(ObjectError::MissingScalarCallStackEvidence {
                                caller: machine,
                                owner: call.owner,
                            })?;
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
                    return Err(ObjectError::NonLinearScalarControlFlow { machine, offset });
                }
                if aarch64_stack_adjustment_at(bytes, offset) {
                    let mutation = claimed
                        .remove(&offset)
                        .ok_or(ObjectError::UnclaimedScalarStackMutation { machine, offset })?;
                    validate_aarch64_scalar_mutation(machine, encoded, mutation)?;
                    replay_scalar_mutation(machine, offset, mutation.kind, &mut depth, &mut peak)?;
                } else if aarch64_unsupported_sp_write(encoded) {
                    return Err(ObjectError::UnsupportedScalarStackMutation { machine, offset });
                }
            }
            if !saw_return {
                return Err(ObjectError::MissingBalancedScalarReturn(machine));
            }
        }
    }
    if let Some((&offset, _)) = claimed.first_key_value() {
        return Err(ObjectError::InvalidScalarStackEvidence { machine, offset });
    }
    if let Some((_, call)) = call_sites.first_key_value() {
        return Err(ObjectError::InvalidInternalCallSite {
            caller: machine,
            owner: call.owner,
            offset: call.offset,
        });
    }
    if let Some((_, call)) = dynamic_call_sites.first_key_value() {
        return Err(ObjectError::InvalidDynamicParameterCallEvidence {
            caller: machine,
            operation: call.psi_operation,
        });
    }
    if depth != 0 {
        return Err(ObjectError::MissingBalancedScalarReturn(machine));
    }
    Ok((
        ObjectScalarStack {
            local_peak_bytes: peak,
            stack_alignment: evidence.stack_alignment,
        },
        validated_calls,
    ))
}

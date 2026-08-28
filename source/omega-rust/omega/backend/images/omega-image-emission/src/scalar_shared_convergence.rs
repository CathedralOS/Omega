//! Exact Boolean shared-convergence stack replay.
//!
//! This module validates shared structural-condition identity, field geometry,
//! exact target reads and joins, then replays every prefix, leaf, and shared
//! cleanup region. It does not infer the conditional graph, select layouts, or
//! emit instructions.

use omega_calling_conventions::ValueLocation;
use omega_machine_code::{
    ScalarConditionalBranchEvidence, ScalarConditionalCondition, ScalarJoinBranchEvidence,
    ScalarStackEvidence,
};
use omega_target::Architecture;
use psi_core::MachineId;

use super::scalar_conditional_regions::collect_conditional_tree_regions;
use super::scalar_conditional_stack::replay_scalar_conditional_region;
use super::scalar_division_stack::decode_exact_x86_instruction;
use super::structural_condition_layout::replay_boolean_field_offset;
use super::structural_condition_read::{
    condition_stack_depth_before, replay_aarch64_boolean_structural_read,
    replay_x86_boolean_structural_read,
};
use super::{ObjectError, ObjectScalarCallStack, ObjectScalarStack, validate_internal_call_site};

#[allow(clippy::too_many_arguments)]
pub(super) fn validate_boolean_shared_convergence_stack(
    architecture: Architecture,
    machine: MachineId,
    bytes: &[u8],
    calls: &[omega_machine_code::InternalCallRelocation],
    evidence: &ScalarStackEvidence,
    decisions: &[ScalarConditionalBranchEvidence],
    joins: &[ScalarJoinBranchEvidence],
    structural_conditions: &[omega_machine_code::BooleanStructuralConditionEvidence],
    merge_offset: usize,
    cleanup: Option<&omega_machine_code::UnitAffineCleanupRecord>,
    parameter_homes: &[omega_machine_code::UnitParameterHomeRecord],
) -> Result<(ObjectScalarStack, Vec<ObjectScalarCallStack>), ObjectError> {
    let invalid = || ObjectError::InvalidScalarConditionalEvidence {
        machine,
        offset: decisions
            .first()
            .map_or(0, |decision| decision.branch_offset),
    };
    if decisions.is_empty()
        || joins.len() != decisions.len()
        || decisions
            .windows(2)
            .any(|pair| pair[0].branch_offset >= pair[1].branch_offset)
        || joins
            .windows(2)
            .any(|pair| pair[0].join_offset >= pair[1].join_offset)
        || merge_offset >= bytes.len()
        || evidence.cleanup_preservation.is_none()
        || evidence
            .mutations
            .windows(2)
            .any(|pair| pair[0].offset >= pair[1].offset)
    {
        return Err(invalid());
    }
    let shared_cleanup = cleanup.ok_or_else(invalid)?;
    let mut structural_types = std::collections::BTreeMap::new();
    if shared_cleanup.structural_types.is_empty()
        || shared_cleanup
            .structural_types
            .windows(2)
            .any(|pair| pair[0].id >= pair[1].id)
        || shared_cleanup.structural_types.iter().any(|declaration| {
            structural_types
                .insert(declaration.id, declaration)
                .is_some()
        })
    {
        return Err(invalid());
    }
    let mut prefixes = Vec::with_capacity(decisions.len());
    let mut leaves = Vec::with_capacity(decisions.len() + 1);
    collect_conditional_tree_regions(
        architecture,
        machine,
        bytes,
        0,
        merge_offset,
        decisions,
        &mut prefixes,
        &mut leaves,
    )?;
    if leaves.len() != decisions.len() + 1 {
        return Err(invalid());
    }
    let expression_prefixes = prefixes
        .iter()
        .filter(|(_, _, condition)| *condition == ScalarConditionalCondition::Expression)
        .map(|(start, end, _)| (*start, *end))
        .collect::<std::collections::BTreeSet<_>>();
    let mut previous_end = None;
    let mut structural_identity = None;
    let mut operations = std::collections::BTreeSet::new();
    for condition in structural_conditions {
        let end = condition
            .code_offset
            .checked_add(condition.byte_count)
            .ok_or_else(invalid)?;
        if condition.reads.is_empty()
            || condition.byte_count == 0
            || condition.byte_count != condition.bytes.len()
            || end > merge_offset
            || !expression_prefixes.contains(&(condition.code_offset, end))
            || previous_end.is_some_and(|previous| previous > condition.code_offset)
            || bytes.get(condition.code_offset..end) != Some(condition.bytes.as_slice())
        {
            return Err(invalid());
        }
        previous_end = Some(end);
        let mut previous_read_end = None;
        for read in &condition.reads {
            let read_end = read
                .code_offset
                .checked_add(read.byte_count)
                .ok_or_else(invalid)?;
            let identity = (read.source, read.field, read.field_byte_offset);
            if structural_identity.is_some_and(|expected| expected != identity)
                || !operations.insert(read.psi_operation)
                || read.byte_count == 0
                || read.code_offset < condition.code_offset
                || read_end > end
                || previous_read_end.is_some_and(|previous| previous > read.code_offset)
            {
                return Err(invalid());
            }
            previous_read_end = Some(read_end);
            let mut homes = parameter_homes
                .iter()
                .filter(|home| home.place == read.source);
            let home = homes.next().ok_or_else(invalid)?;
            if homes.next().is_some()
                || home.byte_offset != 0
                || home.shape != home.source.shape
                || home.indirect
                    != matches!(
                        home.source.locations.as_slice(),
                        [ValueLocation::Indirect { .. }]
                    )
            {
                return Err(invalid());
            }
            let (canonical_offset, canonical_shape) =
                replay_boolean_field_offset(home.structural_type, read.field, &structural_types)
                    .ok_or_else(invalid)?;
            if read.field_byte_offset != canonical_offset || home.shape != canonical_shape {
                return Err(invalid());
            }
            let stack_depth =
                condition_stack_depth_before(evidence, condition.code_offset, read.code_offset)
                    .ok_or_else(invalid)?;
            let expected = match architecture {
                Architecture::X86_64 => {
                    replay_x86_boolean_structural_read(&home.source, canonical_offset, stack_depth)
                }
                Architecture::Aarch64 => replay_aarch64_boolean_structural_read(
                    &home.source,
                    canonical_offset,
                    stack_depth,
                ),
            }
            .ok_or_else(invalid)?;
            if expected.len() != read.byte_count
                || bytes.get(read.code_offset..read_end) != Some(expected.as_slice())
            {
                return Err(invalid());
            }
            structural_identity = Some(identity);
        }
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
    let mut validated_calls = Vec::with_capacity(calls.len());
    let mut peak = 0;
    for (start, end, condition) in prefixes {
        let prefix_peak = replay_scalar_conditional_region(
            architecture,
            machine,
            bytes,
            start,
            end,
            false,
            &mut claimed,
            &mut call_sites,
            condition == ScalarConditionalCondition::Expression,
            evidence,
            &mut validated_calls,
            None,
        )?;
        if condition == ScalarConditionalCondition::Parameter && prefix_peak != 0 {
            return Err(invalid());
        }
        peak = peak.max(prefix_peak);
    }
    for (index, (start, end)) in leaves.into_iter().enumerate() {
        let value_end = if let Some(join) = joins.get(index) {
            let join_end = join
                .join_offset
                .checked_add(join.join_byte_count)
                .ok_or_else(invalid)?;
            if join.join_offset < start || join_end != end {
                return Err(invalid());
            }
            match architecture {
                Architecture::X86_64 => {
                    let instruction = decode_exact_x86_instruction(
                        machine,
                        bytes,
                        join.join_offset,
                        join.join_byte_count,
                    )?;
                    if instruction.mnemonic() != iced_x86::Mnemonic::Jmp
                        || usize::try_from(instruction.near_branch_target()).ok()
                            != Some(merge_offset)
                    {
                        return Err(invalid());
                    }
                }
                Architecture::Aarch64 => {
                    if join.join_byte_count != 4 || !join.join_offset.is_multiple_of(4) {
                        return Err(invalid());
                    }
                    let encoded = u32::from_le_bytes(
                        bytes[join.join_offset..join_end]
                            .try_into()
                            .map_err(|_| invalid())?,
                    );
                    let words = merge_offset
                        .checked_sub(join.join_offset)
                        .filter(|distance| distance.is_multiple_of(4))
                        .map(|distance| distance / 4)
                        .and_then(|words| u32::try_from(words).ok())
                        .filter(|words| *words <= 0x01ff_ffff)
                        .ok_or_else(invalid)?;
                    if encoded != 0x1400_0000 | words {
                        return Err(invalid());
                    }
                }
            }
            join.join_offset
        } else {
            if index != decisions.len() || end != merge_offset {
                return Err(invalid());
            }
            end
        };
        peak = peak.max(replay_scalar_conditional_region(
            architecture,
            machine,
            bytes,
            start,
            value_end,
            false,
            &mut claimed,
            &mut call_sites,
            false,
            evidence,
            &mut validated_calls,
            None,
        )?);
    }
    peak = peak.max(replay_scalar_conditional_region(
        architecture,
        machine,
        bytes,
        merge_offset,
        bytes.len(),
        true,
        &mut claimed,
        &mut call_sites,
        true,
        evidence,
        &mut validated_calls,
        cleanup,
    )?);
    if let Some((&offset, _)) = claimed.first_key_value() {
        return Err(ObjectError::InvalidScalarStackEvidence { machine, offset });
    }
    if let Some((&offset, call)) = call_sites.first_key_value() {
        return Err(ObjectError::InvalidInternalCallSite {
            caller: machine,
            owner: call.owner,
            offset,
        });
    }
    Ok((
        ObjectScalarStack {
            local_peak_bytes: peak,
            stack_alignment: evidence.stack_alignment,
        },
        validated_calls,
    ))
}

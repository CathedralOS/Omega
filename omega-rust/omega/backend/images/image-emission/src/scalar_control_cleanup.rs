//! Conditional scalar-cleanup evidence replay.
//!
//! This module binds per-leaf cleanup records to the exact conditional-tree
//! regions, provenance edges, and preservation bytes they claim. It does not
//! choose cleanup actions, infer stack frames, or emit instructions.

use machine_code::{
    ScalarControlAffineCleanupRecord, ScalarControlFlowEvidence, ScalarStackEvidence,
    UnitAffineCleanupRecord,
};
use semantic_vocabulary::MachineId;
use target::Architecture;
use target_operations::{CallSiteOwner, TerminalPsiProvenance};

use super::ObjectError;
use super::scalar_cleanup_preservation::validate_scalar_cleanup_preservation_record;
use super::scalar_conditional_regions::collect_conditional_tree_regions;

pub(super) fn validate_scalar_control_cleanup_evidence(
    architecture: Architecture,
    machine: MachineId,
    provenance: &TerminalPsiProvenance,
    bytes: &[u8],
    stack: &ScalarStackEvidence,
    records: &[ScalarControlAffineCleanupRecord],
) -> Result<(), ObjectError> {
    let invalid = || ObjectError::InvalidUnitAffineCleanupEvidence(machine);
    let ScalarControlFlowEvidence::ConditionalTree {
        ref decisions,
        ref crash_leaves,
        ref branches,
    } = stack.control_flow
    else {
        return if records.is_empty() {
            Ok(())
        } else {
            Err(invalid())
        };
    };
    if records.is_empty() {
        return if stack.cleanup_preservation.is_none() {
            Ok(())
        } else {
            Err(invalid())
        };
    }
    if !branches.is_empty() {
        return Err(invalid());
    }
    if crash_leaves.iter().any(|crash| *crash) {
        return Err(invalid());
    }
    let mut prefixes = Vec::new();
    let mut leaf_regions = Vec::new();
    collect_conditional_tree_regions(
        architecture,
        machine,
        bytes,
        0,
        bytes.len(),
        decisions,
        &mut prefixes,
        &mut leaf_regions,
    )?;
    if records.len() != leaf_regions.len()
        || crash_leaves.len() != leaf_regions.len()
        || stack.cleanup_preservation.is_some()
    {
        return Err(invalid());
    }
    let mut edges = std::collections::BTreeSet::new();
    for (record, (leaf_start, leaf_end)) in records.iter().zip(leaf_regions) {
        let cleanup = &record.cleanup;
        let cleanup_end = cleanup
            .code_offset
            .checked_add(cleanup.byte_count)
            .ok_or_else(invalid)?;
        if cleanup.code_offset < leaf_start
            || cleanup_end != leaf_end
            || !edges.insert(cleanup.psi_edge)
        {
            return Err(invalid());
        }
        let position = provenance
            .edges
            .iter()
            .position(|edge| *edge == cleanup.psi_edge)
            .ok_or_else(invalid)?;
        if provenance.edges[position + 1..].contains(&cleanup.psi_edge) {
            return Err(invalid());
        }
        validate_scalar_cleanup_preservation_record(
            architecture,
            machine,
            bytes,
            cleanup,
            record.preservation,
            leaf_end,
        )?;
    }
    if records.windows(2).any(|pair| {
        pair[0]
            .cleanup
            .code_offset
            .checked_add(pair[0].cleanup.byte_count)
            .is_none_or(|end| end > pair[1].cleanup.code_offset)
    }) || records[1..].iter().any(|record| {
        record.cleanup.structural_types != records[0].cleanup.structural_types
            || record.cleanup.locals != records[0].cleanup.locals
            || record.cleanup.actions != records[0].cleanup.actions
    }) {
        return Err(invalid());
    }
    Ok(())
}

pub(super) fn cleanup_for_owner(
    records: &[ScalarControlAffineCleanupRecord],
    owner: CallSiteOwner,
) -> Option<&UnitAffineCleanupRecord> {
    let CallSiteOwner::CleanupAction { edge, .. } = owner else {
        return None;
    };
    records
        .iter()
        .find(|record| record.cleanup.psi_edge == edge)
        .map(|record| &record.cleanup)
}

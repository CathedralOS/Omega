//! Acyclic terminal stack-demand composition.
//!
//! This module composes the exact caller-owned peaks retained by target
//! emission. Sequential calls take a maximum; one active caller prefix adds to
//! the selected callee peak. It does not infer new stack or call-site facts.

use semantic_vocabulary::MachineId;

use super::{ObjectArtifact, ObjectError, ObjectFunction, StackDemand, UnitStackDemand};

/// Compose the exact caller-owned peaks retained by the target emitter for a
/// selected Unit entry. Cycles and any reachable non-Unit function fail closed.
pub fn derive_unit_stack_demand(
    artifact: &ObjectArtifact,
    entry: MachineId,
) -> Result<UnitStackDemand, ObjectError> {
    derive_stack_demand(artifact, entry)
}

/// Compose byte-validated stack evidence for the currently admitted terminal
/// function slices. Unit and branch-free scalar functions retain the acyclic
/// internal-call closure.
pub fn derive_stack_demand(
    artifact: &ObjectArtifact,
    entry: MachineId,
) -> Result<StackDemand, ObjectError> {
    let functions = artifact
        .functions
        .iter()
        .map(|function| (function.machine, function))
        .collect::<std::collections::BTreeMap<_, _>>();
    if !functions.contains_key(&entry) {
        return Err(ObjectError::EntryFunctionMissing(entry));
    }
    // Object construction has already replayed these complete, exact native
    // bodies. A nonreturning `exit_group` leaf has no return frame to encode,
    // but still contributes an exact zero bytes rather than an unknown stack.
    let stackless_nonreturning = functions
        .values()
        .filter(|function| function.unit_stack.is_none() && function.scalar_stack.is_none())
        .filter(|function| {
            let mut matching = artifact
                .boundary_settlements
                .iter()
                .filter(|row| row.machine == function.machine);
            let Some(row) = matching.next() else {
                return false;
            };
            matching.next().is_none()
                && matches!(
                    row.settlement.realization,
                    target_operations::BoundaryRealization::LinuxExitGroupI32(_)
                )
                && row.settlement.code_offset == 0
                && row.settlement.byte_count == function.byte_count
        })
        .map(|function| function.machine)
        .collect::<std::collections::BTreeSet<_>>();
    let mut active = std::collections::BTreeSet::new();
    let mut memoized = std::collections::BTreeMap::new();
    let mut contributing_machines = std::collections::BTreeSet::new();
    let mut admitted_contribution_report_identities = std::collections::BTreeSet::new();
    let mut admitted_contribution_commitments = std::collections::BTreeSet::new();
    let ceiling_bytes = derive_terminal_stack_peak(
        entry,
        artifact,
        &functions,
        &mut active,
        &mut memoized,
        &mut contributing_machines,
        &mut admitted_contribution_report_identities,
        &mut admitted_contribution_commitments,
        &stackless_nonreturning,
    )?;
    Ok(StackDemand {
        psi: artifact.psi,
        target: artifact.target,
        entry,
        ceiling_bytes,
        stack_alignment: 16,
        contributing_machines,
        admitted_contribution_report_identities,
        admitted_contribution_commitments,
    })
}

fn derive_terminal_stack_peak(
    machine: MachineId,
    artifact: &ObjectArtifact,
    functions: &std::collections::BTreeMap<MachineId, &ObjectFunction>,
    active: &mut std::collections::BTreeSet<MachineId>,
    memoized: &mut std::collections::BTreeMap<MachineId, u64>,
    contributing_machines: &mut std::collections::BTreeSet<MachineId>,
    admitted_contribution_report_identities: &mut std::collections::BTreeSet<
        task_plans::AdmittedStackContributionReportId,
    >,
    admitted_contribution_commitments: &mut std::collections::BTreeSet<
        task_plans::SameStackContributionCommitment,
    >,
    stackless_nonreturning: &std::collections::BTreeSet<MachineId>,
) -> Result<u64, ObjectError> {
    if let Some(peak) = memoized.get(&machine) {
        contributing_machines.insert(machine);
        return Ok(*peak);
    }
    if !active.insert(machine) {
        return Err(ObjectError::TerminalStackCycle(machine));
    }
    contributing_machines.insert(machine);
    let function =
        functions
            .get(&machine)
            .copied()
            .ok_or(ObjectError::UnknownInternalCallTarget {
                caller: machine,
                target: machine,
            })?;
    let mut peak = match (function.unit_stack, function.scalar_stack) {
        (Some(_), Some(_)) => {
            return Err(ObjectError::ConflictingTerminalStackEvidence(machine));
        }
        (Some(stack), None) => u64::from(stack.local_peak_bytes),
        (None, Some(stack)) => u64::from(stack.local_peak_bytes),
        (None, None) if stackless_nonreturning.contains(&machine) => 0,
        (None, None) => return Err(ObjectError::UnaccountedTerminalStack(machine)),
    };
    for call in &function.unit_call_stacks {
        let callee_peak = derive_terminal_stack_peak(
            call.target,
            artifact,
            functions,
            active,
            memoized,
            contributing_machines,
            admitted_contribution_report_identities,
            admitted_contribution_commitments,
            stackless_nonreturning,
        )?;
        let caller_live = u64::from(call.caller_live_bytes);
        let composed = caller_live.checked_add(callee_peak).ok_or(
            ObjectError::TerminalStackCompositionOverflow {
                caller: machine,
                owner: call.owner,
            },
        )?;
        peak = peak.max(composed);
    }
    for call in &function.scalar_call_stacks {
        let callee_peak = derive_terminal_stack_peak(
            call.target,
            artifact,
            functions,
            active,
            memoized,
            contributing_machines,
            admitted_contribution_report_identities,
            admitted_contribution_commitments,
            stackless_nonreturning,
        )?;
        let caller_live = u64::from(call.caller_live_bytes);
        let composed = caller_live.checked_add(callee_peak).ok_or(
            ObjectError::TerminalStackCompositionOverflow {
                caller: machine,
                owner: call.owner,
            },
        )?;
        peak = peak.max(composed);
    }
    for call in artifact
        .foreign_calls
        .iter()
        .filter(|call| call.machine == machine)
    {
        let contribution = &call.same_stack_contribution;
        let composed = u64::from(call.caller_live_bytes)
            .checked_add(contribution.bytes())
            .ok_or(ObjectError::TerminalStackCompositionOverflow {
                caller: machine,
                owner: call.owner,
            })?;
        peak = peak.max(composed);
        admitted_contribution_report_identities.insert(contribution.report_identity());
        admitted_contribution_commitments.insert(contribution.commitment());
    }
    active.remove(&machine);
    memoized.insert(machine, peak);
    Ok(peak)
}

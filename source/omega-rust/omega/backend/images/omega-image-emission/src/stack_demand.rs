//! Acyclic terminal stack-demand composition.
//!
//! This module composes the exact caller-owned peaks retained by target
//! emission. Sequential calls take a maximum; one active caller prefix adds to
//! the selected callee peak. It does not infer new stack or call-site facts.

use psi_core::MachineId;

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
    let mut active = std::collections::BTreeSet::new();
    let mut memoized = std::collections::BTreeMap::new();
    let mut contributing_machines = std::collections::BTreeSet::new();
    let ceiling_bytes = derive_terminal_stack_peak(
        entry,
        &functions,
        &mut active,
        &mut memoized,
        &mut contributing_machines,
    )?;
    Ok(StackDemand {
        psi: artifact.psi,
        target: artifact.target,
        entry,
        ceiling_bytes,
        stack_alignment: 16,
        contributing_machines,
    })
}

fn derive_terminal_stack_peak(
    machine: MachineId,
    functions: &std::collections::BTreeMap<MachineId, &ObjectFunction>,
    active: &mut std::collections::BTreeSet<MachineId>,
    memoized: &mut std::collections::BTreeMap<MachineId, u64>,
    contributing_machines: &mut std::collections::BTreeSet<MachineId>,
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
        (None, None) => return Err(ObjectError::UnaccountedTerminalStack(machine)),
    };
    for call in &function.unit_call_stacks {
        let callee_peak = derive_terminal_stack_peak(
            call.target,
            functions,
            active,
            memoized,
            contributing_machines,
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
            functions,
            active,
            memoized,
            contributing_machines,
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
    active.remove(&machine);
    memoized.insert(machine, peak);
    Ok(peak)
}

//! Exclude encoded software faults under the installation's remaining premises.
//! Source/target replay proves ordinary memory accesses use their retained loans
//! and layouts. This pass does not provision those loans or the stack.
use crate::{FunctionFragmentObjectArtifactError as Error, ObjectArtifact};
use installation_evidence::InstalledBodyFaultPremises;
use selected_instructions::{MachineEncodedTrapBehavior as Trap, SelectedInstructionKind as Kind};
use selected_instructions::{SelectedInstruction, SelectedInstructionId, SelectedInstructionPlan};
use semantic_vocabulary::MachineId;
mod ram;

pub(super) fn derive(body: &ObjectArtifact) -> Result<Option<InstalledBodyFaultPremises>, Error> {
    let execution = body.installed_entry_execution().ok_or(Error::Mismatch(
        "installed body stack demand has no exact execution domain",
    ))?;
    if execution.scope().fatal_entries().is_empty() {
        return Ok(None);
    }
    if execution.scope().interrupt_delivery_assumption().is_none() {
        return Err(Error::Mismatch(
            "installed fatal closure lacks its delivery condition",
        ));
    }
    if !body.foreign_calls.is_empty() || !body.private_functions.is_empty() {
        return Err(Error::Mismatch(
            "installed quiet closure has an unresolved foreign or private execution dependency",
        ));
    }
    let replay = body.fragment_replay.as_ref().ok_or(Error::Mismatch(
        "installed fault analysis requires the exact retained native pipeline",
    ))?;
    let current = replay.0.source().source().source().source();
    let selected = current.selected_plan();
    let physical = current.machine().machine().plan();
    if selected.psi != body.psi()
        || selected.entry != body.entry()
        || physical.target != body.target()
    {
        return Err(Error::Mismatch(
            "installed fault analysis changed its body or target",
        ));
    }
    let correlated = correlate_native_instruction_sources(
        selected,
        physical.functions.iter().flat_map(|function| {
            function
                .instructions()
                .map(move |instruction| (function.machine, instruction.instruction))
        }),
    )?;
    let mut machines: Vec<_> = selected
        .functions
        .iter()
        .map(|function| function.machine)
        .collect();
    machines.sort_unstable();
    let mut physical_machines: Vec<_> = physical
        .functions
        .iter()
        .map(|function| function.machine)
        .collect();
    physical_machines.sort_unstable();
    if machines != physical_machines || machines.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(Error::Mismatch(
            "installed fault analysis changed its exact function roster",
        ));
    }
    ram::validate(
        current.optimized_target().optimized().plan(),
        selected,
        current
            .optimized_target()
            .lowering_facts()
            .installed_interrupts(),
        execution,
        &machines,
    )?;
    // Call allocation clobbers are permissions, not executed CPU state.
    // The shared collector includes actual callees, decoded ordinary forms
    // and generated frame save/restores before checking this remaining premise.
    crate::observe_native_body_cpu_state(
        body,
        execution.entry().entry().inbound().state.preemption,
    )?
    .require_without_floating_state()?;
    let mut correlated = correlated.into_iter();
    let framed = replay.0.source().source();
    for function in &physical.functions {
        let fragment = framed
            .fragments()
            .function(function.machine)
            .ok_or(Error::Mismatch(
                "installed fault analysis has no exact framed function",
            ))?;
        let relative_branches = framed
            .validate_relative_branch_rows(
                function.machine,
                fragment.instructions().filter(|row| row.branch.is_some()),
            )
            .map_err(|error| Error::InstalledRelativeBranches {
                machine: function.machine,
                source: error,
            })?;
        for instruction in function.instructions() {
            let (machine, source) = correlated.next().ok_or(Error::Mismatch(
                "installed physical instruction has no exact selected source",
            ))?;
            let fail = |reason: &str| Error::InstalledFaultSurface {
                machine,
                instruction: instruction.instruction,
                detail: format!(
                    "{reason}; selected={:?}, encoded={:?}",
                    source.kind, instruction.alternative.encoded
                ),
            };
            if machine != function.machine {
                return Err(fail("physical instruction changed its owning machine"));
            }
            if let Some(target) = source.kind.call_target() {
                let Some(target) = target.machine() else {
                    return Err(fail("dynamic call has no closed fault analysis"));
                };
                if machines.binary_search(&target).is_err() {
                    return Err(fail("callee is outside the exact retained body closure"));
                }
            }
            let effects = &instruction.alternative.encoded;
            match effects.trap {
                Trap::NeverV1 => {}
                Trap::MayArchitecturalFaultV1 => {
                    // Match the operation whose pointer and extent are checked,
                    // not merely an encoded memory effect: an external accessor
                    // has the same MOV footprint without ordinary RAM custody.
                    let memory = matches!(
                        source.kind,
                        Kind::CopyBytes
                            | Kind::ZeroBytes
                            | Kind::LoadEncodedScalar
                            | Kind::StoreEncodedScalar
                            | Kind::ScanByteSequence { .. }
                            | Kind::CompareByteSequences
                            | Kind::LoadPacked { .. }
                            | Kind::StorePacked { .. }
                            | Kind::Store { .. }
                            | Kind::Load8Indexed
                            | Kind::Load8 { .. }
                            | Kind::Load16 { .. }
                            | Kind::Load32 { .. }
                            | Kind::Load64 { .. }
                            | Kind::Store64 { .. }
                            | Kind::AtomicAccess { .. }
                            | Kind::ReturnUnit
                            | Kind::ReturnScalar
                            | Kind::ReturnAggregate { .. }
                    );
                    let privileged = match source.kind {
                        Kind::PrivilegedCrash | Kind::PortWrite { .. } => true,
                        Kind::RegisterAccess { operation } => matches!(operation,
                            terminal_psi::RegisterOperation::ProcessorHalt
                            | terminal_psi::RegisterOperation::InterruptControl(language_core::inline_assembly::AsmInterruptControlKind::Disable)
                            | terminal_psi::RegisterOperation::MemoryFence(_)
                            | terminal_psi::RegisterOperation::PortWrite),
                        _ => false,
                    };
                    // The retained source CFG and target decoder establish the
                    // destination inside this function after frame reflow.
                    // Executable residence remains a separate installation duty.
                    let relative = relative_branches
                        .binary_search(&instruction.instruction)
                        .is_ok();
                    if !memory && !privileged && !relative && source.kind.call_target().is_none() {
                        return Err(fail(
                            "architectural fault source is not discharged by checked RAM/stack or CPL0",
                        ));
                    }
                }
                _ => {
                    return Err(fail(
                        "instruction retains an architectural trap or hosted failure route",
                    ));
                }
            }
        }
    }
    if correlated.next().is_some() {
        return Err(Error::Mismatch(
            "installed physical closure omits a selected instruction",
        ));
    }
    Ok(Some(InstalledBodyFaultPremises {
        physical_program_identity: physical.identity.bytes(),
    }))
}

/// Correlate ordinary physical rows with their selected sources. Instruction
/// numbers are local to one function, including that function's terminators.
/// These are inspection data, not code, backing or fault-exclusion admission.
pub fn correlate_native_instruction_sources(
    selected: &SelectedInstructionPlan,
    physical: impl IntoIterator<Item = (MachineId, SelectedInstructionId)>,
) -> Result<Vec<(MachineId, &SelectedInstruction)>, Error> {
    let mut instructions: Vec<_> = selected
        .functions
        .iter()
        .flat_map(|function| {
            function.blocks.iter().flat_map(move |block| {
                block
                    .instructions
                    .iter()
                    .chain(std::iter::once(
                        selected_instructions::terminator_instruction(&block.terminator),
                    ))
                    .map(move |instruction| ((function.machine, instruction.id), instruction))
            })
        })
        .collect();
    instructions.sort_unstable_by_key(|row| row.0);
    if instructions.windows(2).any(|pair| pair[0].0 == pair[1].0) {
        return Err(Error::Mismatch(
            "installed fault analysis has duplicate selected instructions",
        ));
    }
    let mut seen = vec![false; instructions.len()];
    let mut correlated = Vec::with_capacity(instructions.len());
    for owner in physical {
        let index = instructions
            .binary_search_by_key(&owner, |row| row.0)
            .map_err(|_| {
                Error::Mismatch("installed physical instruction has no exact selected source")
            })?;
        if std::mem::replace(&mut seen[index], true) {
            return Err(Error::Mismatch(
                "installed physical instruction is duplicated",
            ));
        }
        let ((machine, _), source) = instructions[index];
        correlated.push((machine, source));
    }
    if seen.iter().any(|present| !present) {
        return Err(Error::Mismatch(
            "installed physical closure omits a selected instruction",
        ));
    }
    Ok(correlated)
}

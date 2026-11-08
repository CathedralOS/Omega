//! Close RAM-producing boundaries using the already checked source/call graph.
//! This does not infer origins from instruction widths or create mapping rights.
use crate::FunctionFragmentObjectArtifactError as Error;
use abstract_operations::{AbstractOperation as O, AbstractOperationPlan};
use provider_admission::{AdmittedInstalledEntryExecution, AdmittedInstalledInterruptCohort};
use selected_instructions::SelectedInstructionPlan;
use semantic_vocabulary::MachineId;
use target_operations::{CallSiteOwner, NativeCallTarget};

pub(super) fn validate(
    source: &AbstractOperationPlan,
    selected: &SelectedInstructionPlan,
    cohort: &AdmittedInstalledInterruptCohort,
    execution: &AdmittedInstalledEntryExecution,
    machines: &[MachineId],
) -> Result<(), Error> {
    if source.psi != cohort.psi() || source.entry != execution.entry().entry().machine() {
        return Err(Error::Mismatch(
            "installed RAM closure changed its admitted entry",
        ));
    }
    let mut bodies: Vec<_> = source.functions.iter().collect();
    bodies.sort_unstable_by_key(|body| body.machine);
    if bodies
        .windows(2)
        .any(|pair| pair[0].machine == pair[1].machine)
    {
        return Err(Error::Mismatch("installed RAM source body is duplicated"));
    }
    let mut helpers: Vec<_> = cohort
        .helpers()
        .iter()
        .filter(|helper| {
            helper.installation() == execution.scope().operation()
                && helper.callback_application() == execution.entry().placement.application_index
        })
        .collect();
    helpers.sort_unstable_by_key(|helper| helper.operation());
    if helpers
        .windows(2)
        .any(|pair| pair[0].operation() == pair[1].operation())
    {
        return Err(Error::Mismatch(
            "installed RAM helper identity is duplicated",
        ));
    }
    for function in &selected.functions {
        let body = bodies
            .binary_search_by_key(&function.machine, |body| body.machine)
            .ok()
            .map(|index| bodies[index])
            .ok_or(Error::Mismatch(
                "installed RAM closure has no exact original body",
            ))?;
        let mut calls: Vec<_> = function.calls.iter().collect();
        calls.sort_unstable_by_key(|call| call.owner);
        for operation in &body.operations {
            let fail = |reason| Error::InstalledMemorySource {
                machine: body.machine,
                operation: operation.psi_operation(),
                reason,
            };
            let owner = match operation {
                O::EstablishAccessReference { .. } => {
                    return Err(fail(
                        "external placed backing needs its own mapped-residence proof",
                    ));
                }
                O::BoundaryCall { psi_operation, .. } => {
                    if let Ok(index) =
                        helpers.binary_search_by_key(psi_operation, |helper| helper.operation())
                    {
                        match helpers[index].kind() {
                            language_core::static_interrupts::StaticInterruptOperation::Context
                            | language_core::static_interrupts::StaticInterruptOperation::BeginCompletion
                            | language_core::static_interrupts::StaticInterruptOperation::FinishCompletion => continue,
                            language_core::static_interrupts::StaticInterruptOperation::Install => {
                                return Err(fail("nested installation is not a closed Context source"));
                            }
                        }
                    }
                    CallSiteOwner::Operation(*psi_operation)
                }
                O::ExternalCompletion { psi_edge, .. } => {
                    CallSiteOwner::ExternalCompletion(*psi_edge)
                }
                _ => continue,
            };
            let first = calls.partition_point(|call| call.owner < owner);
            let last = calls.partition_point(|call| call.owner <= owner);
            let [call] = &calls[first..last] else {
                return Err(fail(
                    "boundary has no unique concrete RAM-preserving body call",
                ));
            };
            if !matches!(call.call.target, NativeCallTarget::Machine(machine)
                if machines.binary_search(&machine).is_ok())
            {
                return Err(fail(
                    "boundary may introduce backing outside the installed body closure",
                ));
            }
        }
    }
    // Ordinary source replay checks each reference/view projection, bounds and
    // transported call actual. Within this closed graph, their roots are the
    // exact admitted Context, owned local activation storage or image literals.
    // The installing Scope separately proves current nested Context payloads
    // reside in its physical activation; frame/image and receiver requirements
    // still own capacity, permissions, alignment and continued validity.
    Ok(())
}

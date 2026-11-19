//! Check that each installed memory obligation has exactly one admitted source.

use native_vocabulary::emission::{EmissionProgramFacts, RamSource};
use provider_admission::{AdmittedInstalledEntryExecution, AdmittedInstalledInterruptCohort};
use semantic_vocabulary::MachineId;
use target_vocabulary::{CallSiteOwner, NativeCallTarget};

pub fn validate(
    source: &EmissionProgramFacts,
    selected: &machine_code::call_origins::PhysicalCallOrigins,
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
            helper.installation_machine() == execution.scope().machine()
                && helper.installation() == execution.scope().operation()
                && helper.callback_application() == execution.entry().placement.application_index
        })
        .collect();
    helpers.sort_unstable_by_key(|helper| (helper.machine(), helper.operation()));
    if helpers.windows(2).any(|pair| {
        (pair[0].machine(), pair[0].operation()) == (pair[1].machine(), pair[1].operation())
    }) {
        return Err(Error::Mismatch(
            "installed RAM helper identity is duplicated",
        ));
    }
    for function in selected.functions.iter() {
        let body = bodies
            .binary_search_by_key(&function.machine, |body| body.machine)
            .ok()
            .map(|index| bodies[index])
            .ok_or(Error::Mismatch(
                "installed RAM closure has no exact original body",
            ))?;
        let mut calls: Vec<_> = function.calls.iter().collect();
        calls.sort_unstable_by_key(|call| call.owner);
        for obligation in &body.ram_obligations {
            let fail = |reason| Error::InstalledMemorySource {
                machine: body.machine,
                operation: obligation.operation,
                reason,
            };
            let owner = match obligation.source {
                RamSource::PlacedBacking => {
                    return Err(fail(
                        "external placed backing needs its own mapped-residence proof",
                    ));
                }
                RamSource::Boundary(operation) => {
                    let psi_operation = &operation;
                    if let Ok(index) = helpers
                        .binary_search_by_key(&(body.machine, *psi_operation), |helper| {
                            (helper.machine(), helper.operation())
                        })
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
                RamSource::ExternalCompletion(psi_edge) => {
                    CallSiteOwner::ExternalCompletion(psi_edge)
                }
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
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    Mismatch(&'static str),
    InstalledMemorySource {
        machine: MachineId,
        operation: Option<semantic_vocabulary::OperationId>,
        reason: &'static str,
    },
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Mismatch(reason) => f.write_str(reason),
            Self::InstalledMemorySource {
                machine,
                operation,
                reason,
            } => write!(
                f,
                "installed handler {machine:?} operation {operation:?} has no closed RAM source: {reason}"
            ),
        }
    }
}
impl std::error::Error for Error {}

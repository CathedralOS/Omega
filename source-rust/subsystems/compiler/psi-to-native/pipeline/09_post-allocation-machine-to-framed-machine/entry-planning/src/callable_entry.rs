//! Select a callable entry's transport from its final physical ABI and CPU effects.
use calling_conventions::BoundaryEntryPlan;
use function_identity::MachineFunctionIdentity;
use physical_recipes::callable_entry::{
    CallableContext, CallableEntry, CallableInstructions, EntryRequestError, PrivateEntry,
    PrivateEntryParts, identity_transport, native_entry_request_from_abi,
};
use physical_recipes::native_entry_bridge::{InstalledInterruptEntryPlan, NativeEntryBridgeError};

pub type PlannedCallableEntryTransport =
    CallableEntry<crate::PhysicalRecipeOrigin, crate::body_cpu::PlannedBodyCpuState>;
pub type PlannedPrivateEntry =
    PrivateEntry<crate::PhysicalRecipeOrigin, crate::body_cpu::PlannedBodyCpuState>;

#[derive(Debug)]
pub enum CallableEntryError {
    Mismatch(&'static str),
    NativeEntryBridge(NativeEntryBridgeError),
    Cpu(String),
    Request(EntryRequestError),
}
use CallableEntryError as Error;
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Mismatch(reason) => f.write_str(reason),
            Self::Cpu(reason) => f.write_str(reason),
            Self::NativeEntryBridge(error) => write!(f, "native entry transport: {error}"),
            Self::Request(error) => error.fmt(f),
        }
    }
}
impl std::error::Error for Error {}

pub fn plan_private_function_entry(
    identity: MachineFunctionIdentity,
    body: &crate::PhysicalFrameProgram,
    installations: installation_admission::EntryInstallationResources<'_>,
    inbound: &BoundaryEntryPlan,
    inbound_ordinals: &[u32],
    installed_plan: Option<InstalledInterruptEntryPlan>,
) -> Result<PlannedPrivateEntry, Error> {
    let transport = plan_entry_transport(
        body,
        body.instructions.entry,
        installations,
        inbound,
        inbound_ordinals,
        installed_plan.map(|plan| (identity, plan)),
    )?;
    Ok(PrivateEntry::from_planner(
        crate::PhysicalRecipeOrigin(()),
        PrivateEntryParts {
            identity,
            transport,
        },
    ))
}

pub fn plan_callable_entry_transport(
    body: &crate::PhysicalFrameProgram,
    installations: installation_admission::EntryInstallationResources<'_>,
    inbound: &BoundaryEntryPlan,
    inbound_ordinals: &[u32],
) -> Result<PlannedCallableEntryTransport, Error> {
    plan_entry_transport(body, body.instructions.entry, installations, inbound, inbound_ordinals, None)
}

/// A prepared worker is an ordinary reached machine in the physical unit.
/// Its own ABI and CPU footprint govern its entry even when it is not the root.
pub fn plan_fixed_worker_entry(
    body: &crate::PhysicalFrameProgram,
    worker: semantic_vocabulary::MachineId,
    installations: installation_admission::EntryInstallationResources<'_>,
) -> Result<PlannedCallableEntryTransport, Error> {
    let abi = body.instructions.contracts.function_abi(worker)
        .ok_or(Error::Mismatch("fixed worker has no physical ABI"))?;
    let signature = calling_conventions::CallSignature {
        parameters: abi.call_plan.parameters.iter().map(|p| p.shape).collect(),
        result: abi.call_plan.result.as_ref().map(|r| r.shape),
    };
    let inbound = calling_conventions::evaluate_ordinary_boundary_entry_plan(
        calling_conventions::CallingPolicy::native_for_target(body.instructions.target),
        &signature,
    ).map_err(|_| Error::Mismatch("fixed worker has no ordinary entry ABI"))?;
    let ordinals = (0..signature.parameters.len())
        .map(|index| u32::try_from(index).map_err(|_| Error::Mismatch("fixed worker parameter ordinal overflows")))
        .collect::<Result<Vec<_>, _>>()?;
    plan_entry_transport(body, worker, installations, inbound.plan(), &ordinals, None)
}

fn plan_entry_transport(
    body: &crate::PhysicalFrameProgram,
    machine: semantic_vocabulary::MachineId,
    installations: installation_admission::EntryInstallationResources<'_>,
    inbound: &BoundaryEntryPlan,
    inbound_ordinals: &[u32],
    installed_plan: Option<(MachineFunctionIdentity, InstalledInterruptEntryPlan)>,
) -> Result<PlannedCallableEntryTransport, Error> {
    let source = &body.instructions;
    let abi = source
        .contracts
        .function_abi(machine)
        .ok_or(Error::Mismatch("callable body entry plan missing"))?;
    let cpu = crate::body_cpu::plan(body, machine, inbound.state.preemption)
        .map_err(|error| Error::Cpu(error.to_string()))?;
    let request = native_entry_request_from_abi(
        source.machine.target,
        abi,
        cpu.effects().clone(),
        inbound,
        inbound_ordinals,
    )
    .map_err(Error::Request)?;
    let (bridge, effects) = match installed_plan {
        Some((identity, plan)) => {
            let handler = installations.handler().ok_or(Error::Mismatch(
                "installed private entry has no retained execution domain",
            ))?;
            if handler.function != identity
                || handler.contract.inbound() != inbound
                || handler.contract.inbound_ordinals() != inbound_ordinals
            {
                return Err(Error::Mismatch(
                    "installed private entry changed its execution domain",
                ));
            }
            let bridge =
                crate::native_entry_bridge::plan_installed_interrupt_entry_bridge(request, plan)
                    .map_err(Error::NativeEntryBridge)?;
            let effects = bridge.complete_effects().clone();
            (Some(bridge), effects)
        }
        None => {
            if installations.handler().is_some() {
                return Err(Error::Mismatch(
                    "a privileged installed body cannot become an ordinary callable entry",
                ));
            }
            if identity_transport(&request) {
                let effects = request
                    .checked_complete_effects(&calling_conventions::StateEffectsEvidence::empty())
                    .map_err(Error::NativeEntryBridge)?;
                (None, effects)
            } else {
                let bridge = crate::native_entry_bridge::plan_native_entry_bridge(request)
                    .map_err(Error::NativeEntryBridge)?;
                let effects = bridge.complete_effects().clone();
                (Some(bridge), effects)
            }
        }
    };
    Ok(CallableEntry::from_planner(
        crate::PhysicalRecipeOrigin(()),
        CallableInstructions {
            context: CallableContext {
                psi: source.psi,
                entry: machine,
                inbound: inbound.clone(),
                ordinals: inbound_ordinals.to_vec(),
                effects,
            },
            cpu,
            bridge,
        },
    ))
}

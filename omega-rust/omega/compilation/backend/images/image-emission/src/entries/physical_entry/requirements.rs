//! Address-independent requirements of the selected physical arrival.
//!
//! Producing an image establishes none of its runtime mappings. The receiving
//! loader/entry must admit the exact contract and bind its mapped image occurrence
//! before invocation. This row retains the compiler-owned residence that must
//! survive source handoff and every suspended installed Scope.

use super::{PhysicalEntryPartitions, StackSwitch, invalid, original_bss};
use crate::{ObjectArtifact, StackDemand};
use calling_conventions::BoundaryEntryPlan;
use diagnostics::Diagnostic;
use machine_emission::native_entry_bridge::NativeEntryBridge;
use program_entry_plan::ProgramEntryPhysicalContractPlan;
use semantic_vocabulary::{MachineId, OperationId};

mod cpu;
pub use cpu::InstalledPhysicalCpuRequirement;

/// Actual incoming-stack accesses of the emitted checked switch. The selected
/// CallPlan separately retains its complete ABI argument/shadow-area contract.
/// Returning invocations retain the return slot until the physical return;
/// copied stack inputs remain readable until the switch has copied them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhysicalIncomingStackRequirement {
    pub bytes_below_entry_rsp: u64,
    pub readable_bytes_from_entry_rsp: u64,
    /// Alignment at the caller, before its CALL pushes the return address.
    pub caller_alignment: u64,
    /// `(entry_rsp + caller_rsp_bias) % caller_alignment == 0`.
    pub caller_rsp_bias: u64,
}

/// Where the selected target stack capacity is realized. This route requires
/// the exact incoming accesses and private image backing, never the full target
/// capacity on the firmware stack. Only the checked private switch is emitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhysicalEntryStackRequirement {
    CheckedPrivateImage {
        incoming: PhysicalIncomingStackRequirement,
        minimum_private_stack_bytes: u64,
    },
}

/// A requirement, never a claim that firmware or a loader has satisfied it.
///
/// The receiver binds this row to the exact native artifact and relocated image:
/// code must be executable, read-only data readable, and the retained BSS below
/// readable and writable with the emitted alignment. That backing and the code
/// remain valid for the complete physical invocation, including post-firmware
/// handoff, suspended callers and the listed installations. No user-visible root
/// is introduced by this requirement. Source loans and final address replay still
/// establish which accesses may use it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalEntryRequirements {
    psi: terminal_psi::TerminalPsiIdentity,
    target: target::NativeTarget,
    semantic_entry: MachineId,
    adapter: MachineId,
    contract: ProgramEntryPhysicalContractPlan,
    stack: PhysicalEntryStackRequirement,
    retained_storage: PhysicalEntryPartitions,
    installed_scopes: Vec<OperationId>,
    installed_cpu: Option<InstalledPhysicalCpuRequirement>,
}

impl PhysicalEntryRequirements {
    pub const fn psi(&self) -> terminal_psi::TerminalPsiIdentity {
        self.psi
    }
    pub const fn target(&self) -> target::NativeTarget {
        self.target
    }
    pub const fn semantic_entry(&self) -> MachineId {
        self.semantic_entry
    }
    pub const fn adapter(&self) -> MachineId {
        self.adapter
    }
    pub fn contract(&self) -> &ProgramEntryPhysicalContractPlan {
        &self.contract
    }
    pub const fn stack(&self) -> PhysicalEntryStackRequirement {
        self.stack
    }
    /// Offsets in the final image BSS section, before loader relocation.
    pub const fn retained_storage(&self) -> PhysicalEntryPartitions {
        self.retained_storage
    }
    pub fn installed_scopes(&self) -> &[OperationId] {
        &self.installed_scopes
    }
    /// Required execution regime for the actual installed transitions using
    /// this residence. The receiver must admit it; an image does not set it.
    pub const fn installed_cpu(&self) -> Option<InstalledPhysicalCpuRequirement> {
        self.installed_cpu
    }
}

pub(super) fn partitions(
    existing_bss_bytes: u64,
    plan: &BoundaryEntryPlan,
    demand: &StackDemand,
    bridge: &NativeEntryBridge,
    switch: &StackSwitch,
    arrival: &target::TargetEntryStackGuarantee,
    installed_reserve: u64,
) -> Result<PhysicalEntryPartitions, Diagnostic> {
    let stack_demand = demand
        .ceiling_bytes()
        .checked_add(u64::from(bridge.frame_bytes()))
        .and_then(|bytes| bytes.checked_add(u64::from(bridge.call_stack_bias())))
        .and_then(|bytes| bytes.checked_add(switch.stack_overhead_bytes()))
        .and_then(|bytes| bytes.checked_add(arrival.admission_reserve_bytes()))
        .and_then(|bytes| bytes.checked_add(installed_reserve))
        .ok_or_else(|| invalid("private entry stack demand overflows"))?;
    PhysicalEntryPartitions::derive(
        existing_bss_bytes,
        u64::from(plan.call.stack_alignment),
        stack_demand,
        arrival.guaranteed_available_bytes(),
    )
}

pub(super) fn derive(
    artifact: &ObjectArtifact,
    semantic_entry: MachineId,
    contract: &ProgramEntryPhysicalContractPlan,
    demand: &StackDemand,
    bridge: &NativeEntryBridge,
    switch: &StackSwitch,
    installed_reserve: u64,
) -> Result<PhysicalEntryRequirements, Diagnostic> {
    if contract.target_slot().owner != target::TargetProfile::UefiX64
        || contract.target_slot().owner.native_target() != artifact.target()
    {
        return Err(invalid(
            "physical memory requirement changed its selected target contract",
        ));
    }
    let arrival = contract
        .guaranteed_entry_stack()
        .ok_or_else(|| invalid("physical memory requirement lost its arrival contract"))?;
    if !arrival.admits_demand(
        switch.incoming.bytes_below_entry_rsp,
        switch.incoming.caller_alignment,
    ) || switch.incoming.caller_alignment
        != u64::from(contract.boundary_entry_plan().call.stack_alignment)
    {
        return Err(invalid(
            "physical switch incoming demand changed its target contract",
        ));
    }
    let stack = PhysicalEntryStackRequirement::CheckedPrivateImage {
        incoming: switch.incoming,
        minimum_private_stack_bytes: arrival.guaranteed_available_bytes(),
    };
    let retained_storage = partitions(
        original_bss(artifact)?.map_or(0, |(_, bytes, _)| bytes as u64),
        contract.boundary_entry_plan(),
        demand,
        bridge,
        switch,
        arrival,
        installed_reserve,
    )?;
    let source = artifact.composition_source.as_deref().unwrap_or(artifact);
    let mut installed_scopes = Vec::new();
    if let Some(replay) = &source.fragment_replay {
        let target = replay
            .0
            .source()
            .source()
            .source()
            .source()
            .optimized_target();
        for requirement in target.installed_stack_requirements() {
            let Some(arrival) = &requirement.arrival else {
                continue;
            };
            if arrival.physical_entry.semantic_entry != semantic_entry
                || arrival.physical_entry.adapter != artifact.entry()
                || requirement.psi != artifact.psi()
                || requirement.target != artifact.target()
            {
                return Err(invalid(
                    "physical memory requirement changed its installed occurrence",
                ));
            }
            installed_scopes.push(requirement.operation);
        }
    }
    // Each entry in one Scope contributes a demand; the retained lifetime names
    // that Scope once, independently of the number of handlers it installs.
    installed_scopes.sort_unstable();
    installed_scopes.dedup();
    let installed_cpu = cpu::derive(artifact, &installed_scopes)?;
    Ok(PhysicalEntryRequirements {
        psi: artifact.psi(),
        target: artifact.target(),
        semantic_entry,
        adapter: artifact.entry(),
        contract: contract.clone(),
        stack,
        retained_storage,
        installed_scopes,
        installed_cpu,
    })
}

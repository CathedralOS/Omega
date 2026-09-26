//! Checked resource derivation obligations, axes and envelopes.

use crate::checked_trees::checked_trees::facts::contract_plans::{
    CrashPlan, MachineContractCommitment,
};
use language_semantics::TerminationGuarantee;
use std::hash::Hash;
use symbols::SymbolHandle;

/// Exact downstream derivation still required for one checked resource axis.
///
/// These are positive structural obligations, not zero-valued resource
/// claims. Checked Psi has neither target stack closure nor a selected fuel
/// schedule/instruction footprint, so claiming a numeric realization here
/// would promote backend or installation authority into the wrong stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CheckedResourceDerivationObligation {
    TerminalAndTargetStackClosure,
    TerminalControlAndFuelSchedule,
    SelectedInstructionMachineStateFootprint,
}

impl CheckedResourceDerivationObligation {
    const fn identity_tag(self) -> u8 {
        match self {
            Self::TerminalAndTargetStackClosure => 1,
            Self::TerminalControlAndFuelSchedule => 2,
            Self::SelectedInstructionMachineStateFootprint => 3,
        }
    }
}

/// One independently replayable checked resource-axis anchor. Every field is
/// source-handle-free and redundantly binds the exact machine entry and
/// normalized contract so later carriage cannot swap a valid axis between
/// implementations or between entries of one machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedResourceAxisAnchor {
    machine: SymbolHandle,
    entry: SymbolHandle,
    contract_report_fingerprint: u64,
    contract_commitment: MachineContractCommitment,
    derivation_obligation: CheckedResourceDerivationObligation,
    pub(crate) report_fingerprint: u64,
}

impl CheckedResourceAxisAnchor {
    fn from_checked_contract(
        machine: SymbolHandle,
        entry: SymbolHandle,
        contract_report_fingerprint: u64,
        contract_commitment: MachineContractCommitment,
        derivation_obligation: CheckedResourceDerivationObligation,
    ) -> Self {
        let report_fingerprint = checked_resource_axis_report_fingerprint(
            machine,
            entry,
            contract_report_fingerprint,
            contract_commitment,
            derivation_obligation,
        );
        Self {
            machine,
            entry,
            contract_report_fingerprint,
            contract_commitment,
            derivation_obligation,
            report_fingerprint,
        }
    }

    pub const fn machine(&self) -> SymbolHandle {
        self.machine
    }

    pub const fn contract_report_fingerprint(&self) -> u64 {
        self.contract_report_fingerprint
    }

    pub const fn contract_commitment(&self) -> MachineContractCommitment {
        self.contract_commitment
    }

    pub const fn entry(&self) -> SymbolHandle {
        self.entry
    }

    pub const fn derivation_obligation(&self) -> CheckedResourceDerivationObligation {
        self.derivation_obligation
    }

    pub const fn report_fingerprint(&self) -> u64 {
        self.report_fingerprint
    }

    fn validate(
        &self,
        machine: SymbolHandle,
        entry: SymbolHandle,
        contract_report_fingerprint: u64,
        contract_commitment: MachineContractCommitment,
        expected_obligation: CheckedResourceDerivationObligation,
    ) -> Result<(), &'static str> {
        if self.machine != machine
            || self.entry != entry
            || self.contract_report_fingerprint != contract_report_fingerprint
            || self.contract_commitment != contract_commitment
        {
            return Err(
                "checked resource axis does not bind the enclosing exact machine entry contract",
            );
        }
        if self.derivation_obligation != expected_obligation {
            return Err(
                "checked resource envelope substituted or fused an independent resource axis",
            );
        }
        if self.contract_report_fingerprint == 0 || self.contract_commitment.is_zero() {
            return Err("checked resource axis requires a nonzero exact contract identity");
        }
        if self.report_fingerprint
            != checked_resource_axis_report_fingerprint(
                self.machine,
                self.entry,
                self.contract_report_fingerprint,
                self.contract_commitment,
                self.derivation_obligation,
            )
        {
            return Err("checked resource axis report fingerprint does not replay");
        }
        Ok(())
    }
}

/// Checked-Psi resource-envelope prerequisite for one concrete machine entry.
///
/// It records exactly which source-independent derivations remain necessary;
/// it intentionally contains no numeric ceiling, target realization, provider
/// receipt, callback placement, or installation authority. Its report
/// fingerprint is a compilation-local summary/join key, never artifact or
/// admission authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedEntryResourceEnvelope {
    machine: SymbolHandle,
    entry: SymbolHandle,
    contract_report_fingerprint: u64,
    contract_commitment: MachineContractCommitment,
    pub(crate) stack: CheckedResourceAxisAnchor,
    pub(crate) logical_structural_work: CheckedResourceAxisAnchor,
    pub(crate) machine_state: CheckedResourceAxisAnchor,
    report_fingerprint: u64,
}

impl CheckedEntryResourceEnvelope {
    pub fn from_checked_contract(
        machine: SymbolHandle,
        entry: SymbolHandle,
        contract_report_fingerprint: u64,
        contract_commitment: MachineContractCommitment,
    ) -> Self {
        let stack = CheckedResourceAxisAnchor::from_checked_contract(
            machine,
            entry,
            contract_report_fingerprint,
            contract_commitment,
            CheckedResourceDerivationObligation::TerminalAndTargetStackClosure,
        );
        let logical_structural_work = CheckedResourceAxisAnchor::from_checked_contract(
            machine,
            entry,
            contract_report_fingerprint,
            contract_commitment,
            CheckedResourceDerivationObligation::TerminalControlAndFuelSchedule,
        );
        let machine_state = CheckedResourceAxisAnchor::from_checked_contract(
            machine,
            entry,
            contract_report_fingerprint,
            contract_commitment,
            CheckedResourceDerivationObligation::SelectedInstructionMachineStateFootprint,
        );
        let report_fingerprint = checked_resource_envelope_report_fingerprint(
            machine,
            entry,
            contract_report_fingerprint,
            contract_commitment,
            stack.report_fingerprint,
            logical_structural_work.report_fingerprint,
            machine_state.report_fingerprint,
        );
        Self {
            machine,
            entry,
            contract_report_fingerprint,
            contract_commitment,
            stack,
            logical_structural_work,
            machine_state,
            report_fingerprint,
        }
    }

    pub const fn machine(&self) -> SymbolHandle {
        self.machine
    }

    pub const fn contract_report_fingerprint(&self) -> u64 {
        self.contract_report_fingerprint
    }

    pub const fn contract_commitment(&self) -> MachineContractCommitment {
        self.contract_commitment
    }

    pub const fn entry(&self) -> SymbolHandle {
        self.entry
    }

    pub const fn stack(&self) -> &CheckedResourceAxisAnchor {
        &self.stack
    }

    pub const fn logical_structural_work(&self) -> &CheckedResourceAxisAnchor {
        &self.logical_structural_work
    }

    pub const fn machine_state(&self) -> &CheckedResourceAxisAnchor {
        &self.machine_state
    }

    pub const fn report_fingerprint(&self) -> u64 {
        self.report_fingerprint
    }

    pub fn validate(&self) -> Result<(), &'static str> {
        if self.contract_report_fingerprint == 0 || self.contract_commitment.is_zero() {
            return Err("checked resource envelope requires a nonzero exact contract identity");
        }
        self.stack.validate(
            self.machine,
            self.entry,
            self.contract_report_fingerprint,
            self.contract_commitment,
            CheckedResourceDerivationObligation::TerminalAndTargetStackClosure,
        )?;
        self.logical_structural_work.validate(
            self.machine,
            self.entry,
            self.contract_report_fingerprint,
            self.contract_commitment,
            CheckedResourceDerivationObligation::TerminalControlAndFuelSchedule,
        )?;
        self.machine_state.validate(
            self.machine,
            self.entry,
            self.contract_report_fingerprint,
            self.contract_commitment,
            CheckedResourceDerivationObligation::SelectedInstructionMachineStateFootprint,
        )?;
        if self.report_fingerprint
            != checked_resource_envelope_report_fingerprint(
                self.machine,
                self.entry,
                self.contract_report_fingerprint,
                self.contract_commitment,
                self.stack.report_fingerprint,
                self.logical_structural_work.report_fingerprint,
                self.machine_state.report_fingerprint,
            )
        {
            return Err("checked resource envelope report fingerprint does not replay");
        }
        Ok(())
    }
}

/// Sealed declaration-order roster of the resource prerequisites for one
/// machine's entries. The private roster report fingerprint is compilation-
/// local summary evidence: it detects deletion and reordering during checked-
/// fact carriage but grants no artifact or installation authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedMachineResourceEnvelopes {
    pub(crate) machine: SymbolHandle,
    pub(crate) contract_report_fingerprint: u64,
    pub(crate) contract_commitment: MachineContractCommitment,
    pub(crate) entries: Vec<CheckedEntryResourceEnvelope>,
    roster_report_fingerprint: u64,
}

impl CheckedMachineResourceEnvelopes {
    pub fn from_checked_contract_entries(
        machine: SymbolHandle,
        contract_report_fingerprint: u64,
        contract_commitment: MachineContractCommitment,
        entries: impl IntoIterator<Item = SymbolHandle>,
    ) -> Self {
        let entries = entries
            .into_iter()
            .map(|entry| {
                CheckedEntryResourceEnvelope::from_checked_contract(
                    machine,
                    entry,
                    contract_report_fingerprint,
                    contract_commitment,
                )
            })
            .collect::<Vec<_>>();
        let roster_report_fingerprint = checked_resource_roster_report_fingerprint(
            machine,
            contract_report_fingerprint,
            contract_commitment,
            &entries,
        );
        Self {
            machine,
            contract_report_fingerprint,
            contract_commitment,
            entries,
            roster_report_fingerprint,
        }
    }

    pub fn iter(&self) -> std::slice::Iter<'_, CheckedEntryResourceEnvelope> {
        self.entries.iter()
    }

    pub fn get(&self, entry: SymbolHandle) -> Option<&CheckedEntryResourceEnvelope> {
        self.entries.iter().find(|resource| resource.entry == entry)
    }

    pub const fn len(&self) -> usize {
        self.entries.len()
    }

    pub const fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn validate(&self) -> Result<(), &'static str> {
        if self.contract_report_fingerprint == 0 || self.contract_commitment.is_zero() {
            return Err("checked resource roster requires a nonzero exact contract identity");
        }
        for (index, resource) in self.entries.iter().enumerate() {
            resource.validate()?;
            if resource.machine != self.machine
                || resource.contract_report_fingerprint != self.contract_report_fingerprint
                || resource.contract_commitment != self.contract_commitment
            {
                return Err(
                    "checked entry resource envelope does not bind its exact machine contract",
                );
            }
            if self.entries[..index]
                .iter()
                .any(|prior| prior.entry == resource.entry)
            {
                return Err("checked resource roster retained a duplicate machine entry");
            }
            if resource
                != &CheckedEntryResourceEnvelope::from_checked_contract(
                    self.machine,
                    resource.entry,
                    self.contract_report_fingerprint,
                    self.contract_commitment,
                )
            {
                return Err("checked entry resource envelope does not independently replay");
            }
        }
        if self.roster_report_fingerprint
            != checked_resource_roster_report_fingerprint(
                self.machine,
                self.contract_report_fingerprint,
                self.contract_commitment,
                &self.entries,
            )
        {
            return Err("checked resource roster report fingerprint does not replay");
        }
        Ok(())
    }
}

fn checked_resource_axis_report_fingerprint(
    machine: SymbolHandle,
    entry: SymbolHandle,
    contract_report_fingerprint: u64,
    contract_commitment: MachineContractCommitment,
    derivation_obligation: CheckedResourceDerivationObligation,
) -> u64 {
    let mut bytes = b"omega.checked.resource-axis.v1".to_vec();
    bytes.extend(machine.arena_index().to_le_bytes());
    bytes.extend(machine.generation().to_le_bytes());
    bytes.extend(entry.arena_index().to_le_bytes());
    bytes.extend(entry.generation().to_le_bytes());
    bytes.extend(contract_report_fingerprint.to_le_bytes());
    bytes.extend(contract_commitment.as_bytes());
    bytes.push(derivation_obligation.identity_tag());
    checked_resource_report_fingerprint(bytes)
}

fn checked_resource_envelope_report_fingerprint(
    machine: SymbolHandle,
    entry: SymbolHandle,
    contract_report_fingerprint: u64,
    contract_commitment: MachineContractCommitment,
    stack: u64,
    logical_structural_work: u64,
    machine_state: u64,
) -> u64 {
    let mut bytes = b"omega.checked.resource-envelope.v1".to_vec();
    bytes.extend(machine.arena_index().to_le_bytes());
    bytes.extend(machine.generation().to_le_bytes());
    bytes.extend(entry.arena_index().to_le_bytes());
    bytes.extend(entry.generation().to_le_bytes());
    bytes.extend(contract_report_fingerprint.to_le_bytes());
    bytes.extend(contract_commitment.as_bytes());
    bytes.extend(stack.to_le_bytes());
    bytes.extend(logical_structural_work.to_le_bytes());
    bytes.extend(machine_state.to_le_bytes());
    checked_resource_report_fingerprint(bytes)
}

fn checked_resource_roster_report_fingerprint(
    machine: SymbolHandle,
    contract_report_fingerprint: u64,
    contract_commitment: MachineContractCommitment,
    entries: &[CheckedEntryResourceEnvelope],
) -> u64 {
    let mut bytes = b"omega.checked.resource-roster.v1".to_vec();
    bytes.extend(machine.arena_index().to_le_bytes());
    bytes.extend(machine.generation().to_le_bytes());
    bytes.extend(contract_report_fingerprint.to_le_bytes());
    bytes.extend(contract_commitment.as_bytes());
    bytes.extend(
        u64::try_from(entries.len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    for entry in entries {
        bytes.extend(entry.entry.arena_index().to_le_bytes());
        bytes.extend(entry.entry.generation().to_le_bytes());
        bytes.extend(entry.report_fingerprint.to_le_bytes());
    }
    checked_resource_report_fingerprint(bytes)
}

fn checked_resource_report_fingerprint(bytes: impl IntoIterator<Item = u8>) -> u64 {
    const OFFSET: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x100000001b3;
    bytes.into_iter().fold(OFFSET, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(PRIME)
    })
}

/// Complete currently-checkable implementation envelope for one concrete
/// machine. These axes are evidence, not a replacement public contract
/// commitment. The resource field is an exact derivation-obligation anchor,
/// not a claim that downstream resource realizations already exist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RealizedMachineContractEnvelope {
    pub machine: SymbolHandle,
    pub contract_report_fingerprint: u64,
    pub contract_commitment: MachineContractCommitment,
    pub effective_service_reach: Vec<String>,
    /// Concrete reach with installation-selected upper-bound contributions
    /// removed. Root composition unions resolved rows into this provenance-
    /// preserving base.
    pub concrete_service_reach: Vec<String>,
    /// Installation-selected reach requirements still awaiting provider-row
    /// substitution. These are implementation evidence and therefore do not
    /// enter the machine's published contract identity.
    pub unresolved_installation_reaches: Vec<crate::flow_effects::InstallationReachRequirement>,
    pub effective_synchronous_invocations: Vec<String>,
    pub checked_may_suspend: bool,
    pub checked_may_block: bool,
    pub checked_termination: TerminationGuarantee,
    pub checked_crash: CrashPlan,
    pub mutation: Vec<crate::checked_trees::StateWriteFramePlan>,
    pub capabilities: Vec<crate::flow_effects::CapabilityFlowFact>,
    /// One row per concrete entry, in typed declaration order.
    pub resources: CheckedMachineResourceEnvelopes,
}

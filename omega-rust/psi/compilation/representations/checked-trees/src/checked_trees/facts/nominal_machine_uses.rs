use symbols::SymbolHandle;
use typed_trees::expression::ExpressionHandle;
use typed_trees::statement::StatementHandle;

use super::contract_plans::CheckedEntryResourceEnvelope;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NominalMachineUseSite {
    Statement(StatementHandle),
    Expression(ExpressionHandle),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckedMachineContractEnvelopeIdentity {
    pub contract_report_fingerprint: u64,
    pub contract_commitment: crate::MachineContractCommitment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckedMachineContractRefinement {
    pub published_requirement_report_fingerprint: u64,
    pub published_requirement_commitment: crate::MachineContractCommitment,
    pub selected_actual_report_fingerprint: u64,
    pub selected_actual_commitment: crate::MachineContractCommitment,
}

/// Source-independent receipt for the exact checked per-entry resource anchor
/// selected by one nominal callback use.
///
/// All three axis fingerprints remain separate. Later target/backend stages
/// must structurally rejoin this receipt before attaching independently
/// derived stack, fuel, or machine-state evidence; this compact checked row
/// alone grants no artifact, provisioning, or installation authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckedCallbackResourceReceipt {
    machine: SymbolHandle,
    entry: SymbolHandle,
    contract_report_fingerprint: u64,
    contract_commitment: crate::MachineContractCommitment,
    stack_report_fingerprint: u64,
    logical_structural_work_report_fingerprint: u64,
    machine_state_report_fingerprint: u64,
    envelope_report_fingerprint: u64,
}

impl CheckedCallbackResourceReceipt {
    pub fn try_from_entry_envelope(
        envelope: &CheckedEntryResourceEnvelope,
    ) -> Result<Self, &'static str> {
        envelope.validate()?;
        Ok(Self {
            machine: envelope.machine(),
            entry: envelope.entry(),
            contract_report_fingerprint: envelope.contract_report_fingerprint(),
            contract_commitment: envelope.contract_commitment(),
            stack_report_fingerprint: envelope.stack().report_fingerprint(),
            logical_structural_work_report_fingerprint: envelope
                .logical_structural_work()
                .report_fingerprint(),
            machine_state_report_fingerprint: envelope.machine_state().report_fingerprint(),
            envelope_report_fingerprint: envelope.report_fingerprint(),
        })
    }

    pub const fn machine(self) -> SymbolHandle {
        self.machine
    }

    pub const fn entry(self) -> SymbolHandle {
        self.entry
    }

    pub const fn contract_report_fingerprint(self) -> u64 {
        self.contract_report_fingerprint
    }

    pub const fn contract_commitment(self) -> crate::MachineContractCommitment {
        self.contract_commitment
    }

    pub const fn stack_report_fingerprint(self) -> u64 {
        self.stack_report_fingerprint
    }

    pub const fn logical_structural_work_report_fingerprint(self) -> u64 {
        self.logical_structural_work_report_fingerprint
    }

    pub const fn machine_state_report_fingerprint(self) -> u64 {
        self.machine_state_report_fingerprint
    }

    pub const fn envelope_report_fingerprint(self) -> u64 {
        self.envelope_report_fingerprint
    }

    pub fn validate(self) -> Result<(), &'static str> {
        let replayed = CheckedEntryResourceEnvelope::from_checked_contract(
            self.machine,
            self.entry,
            self.contract_report_fingerprint,
            self.contract_commitment,
        );
        self.validate_against(&replayed)
    }

    pub fn validate_against(
        self,
        envelope: &CheckedEntryResourceEnvelope,
    ) -> Result<(), &'static str> {
        envelope.validate()?;
        if self.machine != envelope.machine()
            || self.entry != envelope.entry()
            || self.contract_report_fingerprint != envelope.contract_report_fingerprint()
            || self.contract_commitment != envelope.contract_commitment()
            || self.stack_report_fingerprint != envelope.stack().report_fingerprint()
            || self.logical_structural_work_report_fingerprint
                != envelope.logical_structural_work().report_fingerprint()
            || self.machine_state_report_fingerprint
                != envelope.machine_state().report_fingerprint()
            || self.envelope_report_fingerprint != envelope.report_fingerprint()
        {
            return Err(
                "checked callback resource receipt does not bind its exact per-entry envelope",
            );
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedNominalMachineUse {
    /// Stable occurrence number assigned when the checked roster is frozen.
    /// Pruning retains this number; it is not an index into the remaining vector.
    pub application_index: u32,
    pub site: NominalMachineUseSite,
    pub registration_operation: SymbolHandle,
    /// Retained exact public requirement owner; grants no provider placement.
    pub registration_requirement_trait: Option<SymbolHandle>,
    pub static_machine_ordinal: u32,
    pub selected_machine: SymbolHandle,
    pub selected_entry: SymbolHandle,
    pub satisfaction_trait: SymbolHandle,
    pub satisfaction_requirement: SymbolHandle,
    pub canonical_requirement_overload: String,
    pub published_requirement_envelope: CheckedMachineContractEnvelopeIdentity,
    pub selected_actual_envelope: CheckedMachineContractEnvelopeIdentity,
    /// Present when the nominal boundary declares a calling policy. Psi keeps
    /// the checked entry's resources, not an evaluated target ABI. Realization
    /// must supply the exact calling application for this requirement.
    pub callback_entry: Option<CheckedCallbackResourceReceipt>,
    /// Receipt for the callable-refinement judgment already discharged by
    /// static-machine admission. The endpoints remain explicit so consumers
    /// never infer the relationship merely from two nearby identities.
    pub refinement: CheckedMachineContractRefinement,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NominalMachineUseFacts {
    pub uses: Vec<CheckedNominalMachineUse>,
}

impl NominalMachineUseFacts {
    pub fn try_with_uses(
        uses: impl IntoIterator<Item = CheckedNominalMachineUse>,
    ) -> Result<Self, String> {
        let mut retained = Vec::new();
        for nominal_use in uses {
            if nominal_use
                .published_requirement_envelope
                .contract_commitment
                .is_zero()
                || nominal_use
                    .selected_actual_envelope
                    .contract_commitment
                    .is_zero()
            {
                return Err("nominal machine use retained an empty envelope identity".to_owned());
            }
            if let Some(receipt) = nominal_use.callback_entry {
                receipt.validate()?;
                if receipt.machine() != nominal_use.selected_machine
                    || receipt.entry() != nominal_use.selected_entry
                    || receipt.contract_report_fingerprint()
                        != nominal_use
                            .selected_actual_envelope
                            .contract_report_fingerprint
                    || receipt.contract_commitment()
                        != nominal_use.selected_actual_envelope.contract_commitment
                {
                    return Err(
                        "nominal callback resource receipt does not bind its selected actual entry envelope"
                            .to_owned(),
                    );
                }
            }
            if nominal_use
                .refinement
                .published_requirement_report_fingerprint
                != nominal_use
                    .published_requirement_envelope
                    .contract_report_fingerprint
                || nominal_use.refinement.selected_actual_report_fingerprint
                    != nominal_use
                        .selected_actual_envelope
                        .contract_report_fingerprint
                || nominal_use.refinement.published_requirement_commitment
                    != nominal_use
                        .published_requirement_envelope
                        .contract_commitment
                || nominal_use.refinement.selected_actual_commitment
                    != nominal_use.selected_actual_envelope.contract_commitment
            {
                return Err(
                    "nominal machine use refinement receipt does not bind its envelope identities"
                        .to_owned(),
                );
            }
            if let Some(existing) = retained
                .iter()
                .find(|existing: &&CheckedNominalMachineUse| {
                    existing.site == nominal_use.site
                        && existing.static_machine_ordinal == nominal_use.static_machine_ordinal
                })
            {
                if *existing != nominal_use {
                    return Err(format!(
                        "nominal machine use site {:?} ordinal {} has conflicting admitted identities",
                        nominal_use.site, nominal_use.static_machine_ordinal
                    ));
                }
                continue;
            }
            retained.push(nominal_use);
        }
        for (application_index, usage) in retained.iter_mut().enumerate() {
            usage.application_index = u32::try_from(application_index)
                .map_err(|_| "nominal machine application count exceeds u32".to_owned())?;
        }
        Ok(Self { uses: retained })
    }

    pub fn for_site(
        &self,
        site: NominalMachineUseSite,
        static_machine_ordinal: u32,
    ) -> Option<&CheckedNominalMachineUse> {
        self.uses.iter().find(|nominal_use| {
            nominal_use.site == site && nominal_use.static_machine_ordinal == static_machine_ordinal
        })
    }
}

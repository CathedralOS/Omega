use psi_symbols::SymbolHandle;
use psi_typed_trees::expression::ExpressionHandle;
use psi_typed_trees::statement::StatementHandle;

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

/// Exact target-closed calling-plan application selected for a nominal
/// callback use. The target-owned plan and materialized signature remain
/// outside Psi; this identity is the fail-closed join key used by later thunk
/// placement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckedCallbackPlacementIdentity {
    pub boundary_calling_plan_report_fingerprint: u64,
    pub boundary_calling_plan_commitment:
        psi_typed_trees::typed_trees::BoundaryCallingPlanCommitment,
    /// Exact checked resource anchor selected for this callback entry. This is
    /// compilation-local derivation custody, not a numeric resource claim or
    /// installation receipt.
    pub resource_receipt: CheckedCallbackResourceReceipt,
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
    pub site: NominalMachineUseSite,
    pub registration_operation: SymbolHandle,
    pub static_machine_ordinal: u32,
    pub selected_machine: SymbolHandle,
    pub selected_entry: SymbolHandle,
    pub satisfaction_trait: SymbolHandle,
    pub satisfaction_requirement: SymbolHandle,
    pub canonical_requirement_overload: String,
    pub published_requirement_envelope: CheckedMachineContractEnvelopeIdentity,
    pub selected_actual_envelope: CheckedMachineContractEnvelopeIdentity,
    /// Present only when the nominal requirement owns an evaluated boundary
    /// calling plan. Ordinary nominal machine parameters are not callbacks.
    pub callback_placement: Option<CheckedCallbackPlacementIdentity>,
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
            if nominal_use
                .callback_placement
                .is_some_and(|placement| placement.boundary_calling_plan_commitment.is_zero())
            {
                return Err(
                    "nominal callback use retained an empty boundary calling-plan identity"
                        .to_owned(),
                );
            }
            if let Some(placement) = nominal_use.callback_placement {
                placement.resource_receipt.validate()?;
                if placement.resource_receipt.machine() != nominal_use.selected_machine
                    || placement.resource_receipt.entry() != nominal_use.selected_entry
                    || placement.resource_receipt.contract_report_fingerprint()
                        != nominal_use
                            .selected_actual_envelope
                            .contract_report_fingerprint
                    || placement.resource_receipt.contract_commitment()
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MachineContractCommitment;

    fn nominal_use(selected_machine_index: u32) -> CheckedNominalMachineUse {
        CheckedNominalMachineUse {
            site: NominalMachineUseSite::Expression(ExpressionHandle::from_arena_index(1)),
            registration_operation: SymbolHandle::from_arena_index(2),
            static_machine_ordinal: 0,
            selected_machine: SymbolHandle::from_arena_index(selected_machine_index),
            selected_entry: SymbolHandle::from_arena_index(4),
            satisfaction_trait: SymbolHandle::from_arena_index(5),
            satisfaction_requirement: SymbolHandle::from_arena_index(6),
            canonical_requirement_overload: "Handler::call".to_owned(),
            published_requirement_envelope: CheckedMachineContractEnvelopeIdentity {
                contract_report_fingerprint: 7,
                contract_commitment: MachineContractCommitment::from_digest([7; 32]),
            },
            selected_actual_envelope: CheckedMachineContractEnvelopeIdentity {
                contract_report_fingerprint: 8,
                contract_commitment: MachineContractCommitment::from_digest([8; 32]),
            },
            callback_placement: None,
            refinement: CheckedMachineContractRefinement {
                published_requirement_report_fingerprint: 7,
                published_requirement_commitment: MachineContractCommitment::from_digest([7; 32]),
                selected_actual_report_fingerprint: 8,
                selected_actual_commitment: MachineContractCommitment::from_digest([8; 32]),
            },
        }
    }

    #[test]
    fn exact_duplicate_rows_collapse_and_remain_queryable() {
        let row = nominal_use(3);
        let facts = NominalMachineUseFacts::try_with_uses([row.clone(), row.clone()])
            .expect("an exact repeated observation should be harmless");

        assert_eq!(facts.uses, vec![row.clone()]);
        assert_eq!(facts.for_site(row.site, 0), Some(&row));
    }

    #[test]
    fn one_authored_slot_cannot_retain_two_nominal_identities() {
        let first = nominal_use(3);
        let second = nominal_use(7);

        let message = NominalMachineUseFacts::try_with_uses([first, second])
            .expect_err("the same site and ordinal must have one admitted identity");

        assert!(message.contains("conflicting admitted identities"));
    }

    #[test]
    fn refinement_receipt_must_bind_the_retained_envelope_identities() {
        let first = nominal_use(3);
        let mut second = first.clone();
        second.refinement.selected_actual_report_fingerprint = 9;

        let message = NominalMachineUseFacts::try_with_uses([first, second])
            .expect_err("a refinement receipt cannot cite a different endpoint");

        assert!(message.contains("does not bind its envelope identities"));
    }

    #[test]
    fn callback_placement_identity_must_be_nonzero() {
        let mut row = nominal_use(3);
        let resource = CheckedEntryResourceEnvelope::from_checked_contract(
            row.selected_machine,
            row.selected_entry,
            row.selected_actual_envelope.contract_report_fingerprint,
            row.selected_actual_envelope.contract_commitment,
        );
        row.callback_placement = Some(CheckedCallbackPlacementIdentity {
            boundary_calling_plan_report_fingerprint: 0,
            boundary_calling_plan_commitment:
                psi_typed_trees::typed_trees::BoundaryCallingPlanCommitment::from_digest([0; 32]),
            resource_receipt: CheckedCallbackResourceReceipt::try_from_entry_envelope(&resource)
                .expect("canonical resource receipt"),
        });

        let message = NominalMachineUseFacts::try_with_uses([row])
            .expect_err("an empty target-plan join key must fail closed");

        assert!(message.contains("empty boundary calling-plan identity"));
    }

    #[test]
    fn callback_resource_receipt_must_bind_the_selected_actual_entry() {
        let mut row = nominal_use(3);
        let foreign_entry = CheckedEntryResourceEnvelope::from_checked_contract(
            row.selected_machine,
            SymbolHandle::from_arena_index(9),
            row.selected_actual_envelope.contract_report_fingerprint,
            row.selected_actual_envelope.contract_commitment,
        );
        row.callback_placement = Some(CheckedCallbackPlacementIdentity {
            boundary_calling_plan_report_fingerprint: 10,
            boundary_calling_plan_commitment:
                psi_typed_trees::typed_trees::BoundaryCallingPlanCommitment::from_digest([10; 32]),
            resource_receipt: CheckedCallbackResourceReceipt::try_from_entry_envelope(
                &foreign_entry,
            )
            .expect("canonical but foreign resource receipt"),
        });

        let message = NominalMachineUseFacts::try_with_uses([row])
            .expect_err("a callback cannot substitute another entry's resource receipt");

        assert!(message.contains("does not bind its selected actual entry envelope"));
    }

    #[test]
    fn callback_resource_receipt_rejects_compact_equal_contract_substitution() {
        let mut row = nominal_use(3);
        let expected = row.selected_actual_envelope.contract_commitment;
        let substituted = crate::MachineContractCommitment::from_digest([0x77; 32]);
        assert_ne!(expected, substituted);
        let foreign = CheckedEntryResourceEnvelope::from_checked_contract(
            row.selected_machine,
            row.selected_entry,
            row.selected_actual_envelope.contract_report_fingerprint,
            substituted,
        );
        let foreign_receipt = CheckedCallbackResourceReceipt::try_from_entry_envelope(&foreign)
            .expect("independently canonical compact-equal resource receipt");
        row.callback_placement = Some(CheckedCallbackPlacementIdentity {
            boundary_calling_plan_report_fingerprint: 10,
            boundary_calling_plan_commitment:
                psi_typed_trees::typed_trees::BoundaryCallingPlanCommitment::from_digest([10; 32]),
            resource_receipt: foreign_receipt,
        });

        let message = NominalMachineUseFacts::try_with_uses([row])
            .expect_err("compact-equal resource contract substitution must reject");
        assert!(message.contains("does not bind its selected actual entry envelope"));
    }
}

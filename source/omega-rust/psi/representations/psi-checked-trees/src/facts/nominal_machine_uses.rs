use psi_symbols::SymbolHandle;
use psi_typed_trees::expression::ExpressionHandle;
use psi_typed_trees::statement::StatementHandle;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NominalMachineUseSite {
    Statement(StatementHandle),
    Expression(ExpressionHandle),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckedMachineContractEnvelopeIdentity {
    pub contract_fingerprint: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckedMachineContractRefinement {
    pub published_requirement_fingerprint: u64,
    pub selected_actual_fingerprint: u64,
}

/// Exact evaluated boundary-entry plan selected for a nominal callback use.
/// The target-owned plan remains outside Psi; this identity is the fail-closed
/// join key used by later thunk placement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckedCallbackPlacementIdentity {
    pub boundary_calling_plan_fingerprint: u64,
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
                .contract_fingerprint
                == 0
                || nominal_use.selected_actual_envelope.contract_fingerprint == 0
            {
                return Err("nominal machine use retained an empty envelope identity".to_owned());
            }
            if nominal_use
                .callback_placement
                .is_some_and(|placement| placement.boundary_calling_plan_fingerprint == 0)
            {
                return Err(
                    "nominal callback use retained an empty boundary calling-plan identity"
                        .to_owned(),
                );
            }
            if nominal_use.refinement.published_requirement_fingerprint
                != nominal_use
                    .published_requirement_envelope
                    .contract_fingerprint
                || nominal_use.refinement.selected_actual_fingerprint
                    != nominal_use.selected_actual_envelope.contract_fingerprint
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
                contract_fingerprint: 7,
            },
            selected_actual_envelope: CheckedMachineContractEnvelopeIdentity {
                contract_fingerprint: 8,
            },
            callback_placement: None,
            refinement: CheckedMachineContractRefinement {
                published_requirement_fingerprint: 7,
                selected_actual_fingerprint: 8,
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
        second.refinement.selected_actual_fingerprint = 9;

        let message = NominalMachineUseFacts::try_with_uses([first, second])
            .expect_err("a refinement receipt cannot cite a different endpoint");

        assert!(message.contains("does not bind its envelope identities"));
    }

    #[test]
    fn callback_placement_identity_must_be_nonzero() {
        let mut row = nominal_use(3);
        row.callback_placement = Some(CheckedCallbackPlacementIdentity {
            boundary_calling_plan_fingerprint: 0,
        });

        let message = NominalMachineUseFacts::try_with_uses([row])
            .expect_err("an empty target-plan join key must fail closed");

        assert!(message.contains("empty boundary calling-plan identity"));
    }
}

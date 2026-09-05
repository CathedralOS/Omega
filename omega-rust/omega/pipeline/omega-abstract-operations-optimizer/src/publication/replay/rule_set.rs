//! Selected catalog reconstruction and canonical whole-run rule identity.

use crate::{OrderedRuleRegistry, built_in_psi_registries};
use omega_optimization_core::{OptimizationRuleSetIdentity, OptimizationSelections};

use crate::OptimizedAbstractProjectionError;

pub(super) struct RebuiltRuleSchedule {
    pub(super) registries: Vec<OrderedRuleRegistry>,
    pub(super) ordered_rule_set: OptimizationRuleSetIdentity,
}

pub(super) fn rebuild(
    selections: &OptimizationSelections,
) -> Result<RebuiltRuleSchedule, OptimizedAbstractProjectionError> {
    let registries =
        built_in_psi_registries(selections).map_err(OptimizedAbstractProjectionError::Registry)?;
    let ordered_rules = registries
        .iter()
        .flat_map(|registry| registry.contracts())
        .map(|contract| contract.identity())
        .collect::<Vec<_>>();
    let ordered_rule_set = OptimizationRuleSetIdentity::from_ordered_rules(&ordered_rules)
        .map_err(|_| OptimizedAbstractProjectionError::CommitReplayMismatch)?;
    Ok(RebuiltRuleSchedule {
        registries,
        ordered_rule_set,
    })
}

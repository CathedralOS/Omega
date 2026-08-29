use omega_effects::provider_plan::{ProviderPlan, ProviderPlanDigest};
use psi_diagnostics::Diagnostic;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderGrantSelectorKind {
    PlanName,
    ProviderSlot,
}

fn selected_subjects_coincide(plan: &ProviderPlan, slot_plan: &ProviderPlan) -> bool {
    plan == slot_plan
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedSelectedProviderGrant {
    pub selector: String,
    pub selector_kind: ProviderGrantSelectorKind,
    pub selected_plan: ProviderPlan,
    pub selected_plan_digest: ProviderPlanDigest,
    pub selected_plan_report_identity: u64,
}

impl ResolvedSelectedProviderGrant {
    pub fn commitment(&self) -> String {
        match self.selector_kind {
            ProviderGrantSelectorKind::PlanName => {
                format!("provider plan: {}", self.selected_plan.name)
            }
            ProviderGrantSelectorKind::ProviderSlot => {
                format!("provider slot: {}", self.selected_plan.schema.trait_name)
            }
        }
    }

    /// Replay this grant against one complete selected plan. The compact
    /// identity remains a report coordinate; exact structural equality and
    /// the domain-separated digest retain admission authority.
    pub fn replays_selected_plan(&self, plan: &ProviderPlan) -> bool {
        self.selected_plan_report_identity == plan.report_fingerprint()
            && self.selected_plan_digest == plan.identity_digest()
            && self.selected_plan == *plan
    }
}

/// Resolve trust selectors against the complete candidate inventory and the
/// already-selected provider closure.
pub fn resolve_selected_provider_grants(
    candidates: &[ProviderPlan],
    selected: &omega_effects::SelectedProviderPlanFacts,
    root_grants: &[String],
) -> Result<Vec<ResolvedSelectedProviderGrant>, Diagnostic> {
    let mut resolved = Vec::new();
    for grant in root_grants {
        let plan_name_candidates = candidates
            .iter()
            .filter(|plan| plan.name == *grant)
            .collect::<Vec<_>>();
        let slot_is_known = candidates
            .iter()
            .any(|plan| plan.schema.trait_name == *grant);
        if plan_name_candidates.is_empty() && !slot_is_known {
            continue;
        }
        if plan_name_candidates.len() > 1 {
            return Err(Diagnostic::error(format!(
                "root grant `{grant}` names {} exact provider plan candidates",
                plan_name_candidates.len(),
            )));
        }

        let selected_by_plan_name = selected
            .plans()
            .iter()
            .filter(|plan| plan.name == *grant)
            .collect::<Vec<_>>();
        let selected_by_slot = selected
            .plans()
            .iter()
            .filter(|plan| plan.schema.trait_name == *grant)
            .collect::<Vec<_>>();
        if selected_by_plan_name.len() > 1 || selected_by_slot.len() > 1 {
            return Err(Diagnostic::error(format!(
                "root grant `{grant}` resolves to multiple selected provider plans",
            )));
        }
        let plan_name_plan = selected_by_plan_name.first().copied();
        let slot_plan = selected_by_slot.first().copied();
        if !plan_name_candidates.is_empty() && plan_name_plan.is_none() {
            return Err(Diagnostic::error(format!(
                "root grant `{grant}` names an unselected provider plan",
            )));
        }
        if slot_is_known && slot_plan.is_none() && plan_name_plan.is_none() {
            return Err(Diagnostic::error(format!(
                "root grant `{grant}` names a provider slot with no selected provider plan",
            )));
        }
        let (plan, selector_kind) = match (plan_name_plan, slot_plan) {
            (Some(plan), Some(slot_plan)) if selected_subjects_coincide(plan, slot_plan) => {
                (plan, ProviderGrantSelectorKind::PlanName)
            }
            (Some(_), Some(_)) => {
                return Err(Diagnostic::error(format!(
                    "root grant `{grant}` names distinct provider plan and slot subjects",
                )));
            }
            (Some(plan), None) => (plan, ProviderGrantSelectorKind::PlanName),
            (None, Some(plan)) if plan_name_candidates.is_empty() => {
                (plan, ProviderGrantSelectorKind::ProviderSlot)
            }
            (None, Some(_)) => {
                return Err(Diagnostic::error(format!(
                    "root grant `{grant}` names an unselected provider plan and a different selected provider slot",
                )));
            }
            (None, None) => {
                return Err(Diagnostic::error(format!(
                    "root grant `{grant}` names a provider plan or slot with no selected provider plan",
                )));
            }
        };
        let exact_candidate_matches = candidates
            .iter()
            .filter(|candidate| *candidate == plan)
            .count();
        if exact_candidate_matches != 1 {
            return Err(Diagnostic::error(format!(
                "root grant `{grant}` selected provider plan `{}` resolves to {exact_candidate_matches} exact candidate rows",
                plan.name,
            )));
        }
        resolved.push(ResolvedSelectedProviderGrant {
            selector: grant.clone(),
            selector_kind,
            selected_plan: plan.clone(),
            selected_plan_digest: plan.identity_digest(),
            selected_plan_report_identity: plan.report_fingerprint(),
        });
    }
    Ok(resolved)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_and_slot_coincidence_rejects_compact_equal_structural_substitution() {
        let mut plan = ProviderPlan::default();
        plan.schema
            .methods
            .push(omega_effects::provider_plan::ServiceMethod {
                requirement_owner: "Pair".to_owned(),
                ..Default::default()
            });
        let mut substituted = plan.clone();
        substituted.schema.methods[0].requirement_owner = "OtherPair".to_owned();

        assert_eq!(plan.report_fingerprint(), substituted.report_fingerprint());
        assert_ne!(plan.identity_digest(), substituted.identity_digest());
        assert!(!selected_subjects_coincide(&plan, &substituted));
    }
}

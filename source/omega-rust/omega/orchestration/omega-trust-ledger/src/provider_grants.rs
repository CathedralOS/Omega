use omega_effects::provider_plan::ProviderPlan;
use psi_diagnostics::Diagnostic;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderGrantSelectorKind {
    PlanName,
    ProviderSlot,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedSelectedProviderGrant {
    pub selector: String,
    pub selector_kind: ProviderGrantSelectorKind,
    pub selected_plan_identity: u64,
    pub selected_plan_name: String,
    pub selected_slot_name: String,
}

impl ResolvedSelectedProviderGrant {
    pub fn commitment(&self) -> String {
        match self.selector_kind {
            ProviderGrantSelectorKind::PlanName => {
                format!("provider plan: {}", self.selected_plan_name)
            }
            ProviderGrantSelectorKind::ProviderSlot => {
                format!("provider slot: {}", self.selected_slot_name)
            }
        }
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
            (Some(plan), Some(slot_plan))
                if plan.identity_fingerprint() == slot_plan.identity_fingerprint() =>
            {
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
            selected_plan_identity: plan.identity_fingerprint(),
            selected_plan_name: plan.name.clone(),
            selected_slot_name: plan.schema.trait_name.clone(),
        });
    }
    Ok(resolved)
}

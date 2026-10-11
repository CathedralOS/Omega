//! Build-owned provider choices beside a canonical portable program.
//!
//! These records retain authored selection, not entry backing, external host
//! bindings, permissions or native admission. A failed target projection leaves
//! the portable semantics usable and refuses that target's proposed selection.
//! Receivers still admit candidate rows against the exact canonical artifact.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalTargetProviderProposal {
    pub target: String,
    pub terminal_psi: terminal_psi::TerminalPsiIdentity,
    /// The complete selected optimization set, including Psi already executed
    /// and pending Omega selections. This is not a substitute for native PCC.
    /// Terminal interpretation applies no pending Omega optimization.
    pub optimization_selection: optimization_core::OptimizationSelectionIdentity,
    pub selection: Result<TerminalTargetProviderSelection, Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalTargetProviderSelection {
    /// Complete selected plans, including external or intrinsic rows. A
    /// checked candidate subset cannot masquerade as the whole provider plan.
    pub plans: Vec<TerminalTargetProviderPlan>,
    /// Exact emitted checked candidates. Selecting these grants no authority
    /// to supply external rows that the complete plans may additionally need.
    pub checked_candidates: Vec<terminal_installation::ProviderInstallationSelection>,
}

/// One whole selected plan and its exact selecting declaration. The owner
/// belongs to this plan, not a parallel publication index or a reconstructed
/// package/display path. Ownerless external leaves remain ownerless; every
/// checked adapter requires an exact owner. This carries no native authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalTargetProviderPlan {
    pub plan: effects::provider_plan::ProviderPlan,
    pub provider_declaration: Option<String>,
}

impl TerminalTargetProviderSelection {
    /// Join every checked coordinate of the complete selected plans to its
    /// exact catalog row before ordinary installation admission. The supplied
    /// dense IDs disambiguate physical bodies; canonical identities establish
    /// the plan association. This grants no external-binding authority.
    pub fn admit(
        &self,
        artifact: terminal_codec::TerminalArtifactView<'_>,
        target: &str,
    ) -> Result<terminal_installation::AdmittedProviderSelections, String> {
        let module = artifact.module();
        // Receiving policy is public input, unlike the source planner's sealed
        // facts. Check whole-plan coverage and competing slots before joining
        // canonical candidates; no individual rows combine partial providers.
        effects::SelectedProviderPlanFacts::from_selected_plans(
            self.plans
                .iter()
                .map(|selected| selected.plan.clone())
                .collect(),
        )?;
        let target_members =
            terminal_installation::admit_target_machine_selections(artifact, target);
        for selection in &self.checked_candidates {
            let callable = terminal_psi::TargetMachineCallable::Machine(selection.candidate);
            for family in module.target_machine_families.iter().filter(|family| {
                family
                    .members
                    .iter()
                    .any(|member| member.callable == callable)
            }) {
                let member = target_members
                    .member(family.canonical)
                    .map_err(|error| format!("provider target membership rejected: {error:?}"))?;
                if member.callable() != callable {
                    return Err(
                        "checked provider candidate belongs to another deployment target"
                            .to_owned(),
                    );
                }
            }
        }
        let mut consumed = vec![false; self.checked_candidates.len()];
        for selected_plan in &self.plans {
            let plan = &selected_plan.plan;
            if !plan.target.is_empty() && plan.target != target {
                return Err(
                    "selected provider plan belongs to another deployment target".to_owned(),
                );
            }
            for row in &plan.rows {
                let effects::provider_plan::ProviderBinding::CheckedAdapter {
                    machine_identity,
                    ..
                } = &row.binding
                else {
                    continue;
                };
                let selected_machine = plan.identity_text(*machine_identity);
                let provider_declaration = selected_plan
                    .provider_declaration
                    .as_deref()
                    .filter(|identity| !identity.is_empty())
                    .ok_or_else(|| {
                        "checked provider plan has no exact selecting declaration".to_owned()
                    })?;
                // This companion's producer retains declaration-level plans,
                // not executable specialization/family aliases. Such aliases
                // must be published with their own checked relationships before
                // they can replace these exact coordinates.
                let requirement = row.requirement_identity.as_str();
                let mut matches =
                    self.checked_candidates
                        .iter()
                        .enumerate()
                        .filter(|(_, selection)| {
                            module.provider_candidates.iter().any(|candidate| {
                                candidate.boundary == selection.boundary
                                    && candidate.candidate == selection.candidate
                                    && module.identities.text(candidate.provider_identity)
                                        == Some(selection.provider_identity.as_str())
                                    && selection.provider_identity == plan.provider_type
                                    && module
                                        .identities
                                        .text(candidate.provider_declaration.identity())
                                        == Some(selection.provider_declaration.as_str())
                                    && selection.provider_declaration == provider_declaration
                                    && module.identities.text(candidate.requirement_identity)
                                        == Some(requirement)
                                    && module.identities.text(candidate.candidate_identity)
                                        == Some(selected_machine)
                            })
                        });
                let Some((position, _)) = matches.next() else {
                    return Err(format!(
                        "complete selected provider `{}` has no exact checked installation for `{requirement}`",
                        plan.provider_type
                    ));
                };
                if matches.next().is_some() {
                    return Err(format!(
                        "selected provider requirement `{requirement}` has duplicate checked installations"
                    ));
                }
                // Several whole schemas may deliberately share a required
                // parent coordinate. They must agree on its one exact row.
                consumed[position] = true;
            }
        }
        if consumed.iter().any(|used| !used) {
            return Err(
                "checked installation contains rows outside the complete selected plans".to_owned(),
            );
        }
        terminal_installation::admit_provider_installation(artifact, &self.checked_candidates, &[])
            .map_err(|error| format!("catalog admission rejected: {error:?}"))
    }
}

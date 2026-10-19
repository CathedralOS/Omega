//! Build-owned provider choices beside a canonical portable program.
//!
//! Plans retain authored selection, not entry backing, external host bindings
//! or native admission. Canonical candidates own the complete source-to-instance
//! association. Publication and receiving policy use that same roster projection:
//! a supplied subset cannot omit an uncalled instance or merge choosing owners.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalTargetProviderProposal {
    pub target: String,
    pub terminal_psi: terminal_psi::TerminalPsiIdentity,
    /// Complete selected optimization set; pending Omega work is not applied
    /// by Terminal interpretation and this identity supplies no native authority.
    pub optimization_selection: optimization_core::OptimizationSelectionIdentity,
    pub selection: Result<TerminalTargetProviderSelection, Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalTargetProviderSelection {
    /// Complete plans, including external or intrinsic rows. Checked candidates
    /// cannot masquerade as the whole plan or authorize its external bindings.
    pub plans: Vec<TerminalTargetProviderPlan>,
    pub assumed_bindings: Vec<effects::AssumedBindingContractSelection>,
    pub checked_candidates: Vec<terminal_installation::ProviderInstallationSelection>,
}

/// One whole selected plan with its exact selecting declaration, not a parallel
/// index or a reconstructed package/display path. External leaves may be ownerless;
/// every checked adapter requires an exact owner. This carries no native authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalTargetProviderPlan {
    pub plan: effects::provider_plan::ProviderPlan,
    pub provider_declaration: Option<String>,
}

impl TerminalTargetProviderSelection {
    /// Project complete checked rosters from canonical associations. This
    /// constructs a proposal only; receiving target admission remains required.
    pub fn from_plans(
        plans: Vec<TerminalTargetProviderPlan>,
        artifact: terminal_codec::TerminalArtifactView<'_>,
    ) -> Result<Self, String> {
        let checked_candidates = checked_candidate_roster(&plans, artifact)?;
        Ok(Self {
            plans,
            assumed_bindings: Vec::new(),
            checked_candidates,
        })
    }

    /// Require the artifact's complete roster, target membership and ordinary
    /// catalog admission. Public policy cannot supply its own specialization
    /// expansion or combine partial providers into a whole selected plan.
    pub fn admit(
        &self,
        artifact: terminal_codec::TerminalArtifactView<'_>,
        target: &str,
    ) -> Result<terminal_installation::AdmittedProviderSelections, String> {
        let expected = checked_candidate_roster(&self.plans, artifact)?;
        let mut supplied = self.checked_candidates.clone();
        supplied.sort();
        if supplied != expected {
            return Err(
                "checked installations differ from the complete canonical selected-provider roster"
                    .into(),
            );
        }
        if self
            .plans
            .iter()
            .any(|selected| !selected.plan.target.is_empty() && selected.plan.target != target)
        {
            return Err("selected provider plan belongs to another deployment target".into());
        }
        let module = artifact.module();
        let target_members =
            terminal_installation::admit_target_machine_selections(artifact, target);
        for selection in &supplied {
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
                        "checked provider candidate belongs to another deployment target".into(),
                    );
                }
            }
        }
        let dependencies = matching_folded_constant_dependencies(
            artifact,
            target,
            self.plans.iter().map(|selected| &selected.plan),
        )?;
        let installation = terminal_installation::admit_provider_installation(
            artifact,
            &supplied,
            &effects::SelectedProviderPlanFacts::from_selected_plans(
                self.plans
                    .iter()
                    .map(|selected| selected.plan.clone())
                    .collect(),
            )?
            .with_assumed_bindings(self.assumed_bindings.clone())
            .assumed_binding_coordinates(module)?
            .into_iter()
            .map(|(required, supplied_requirement)| {
                terminal_installation::AssumedBindingSelection {
                    required,
                    supplied_requirement,
                }
            })
            .collect::<Vec<_>>(),
        )
        .map_err(|error| format!("catalog admission rejected: {error:?}"))?;
        if artifact.module().folded_constants.target.is_some() {
            installation.admit_folded_constant_closure(artifact, target, &dependencies)
        } else {
            Ok(installation)
        }
    }
}

/// Reconstruct the primary historical realization applications from current
/// whole normalized plans. Supplemental proof scopes remain immutable semantic
/// provenance, validated against their own catalog associations at ingress.
pub fn matching_folded_constant_dependencies<'plan>(
    artifact: terminal_codec::TerminalArtifactView<'_>,
    target: &str,
    plans: impl IntoIterator<Item = &'plan effects::provider_plan::ProviderPlan>,
) -> Result<Vec<Vec<u8>>, String> {
    let module = artifact.module();
    let closure = &module.folded_constants;
    closure.validate(module).map_err(str::to_owned)?;
    let Some(required_target) = &closure.target else {
        return Ok(Vec::new());
    };
    if required_target != target {
        return Err(format!(
            "folded constant target closure is `{required_target}`, not `{target}`"
        ));
    }
    let plans: Vec<_> = plans.into_iter().collect();
    let mut matched = Vec::new();
    for dependency in closure
        .dependencies
        .iter()
        .filter(|dependency| dependency.target == target)
    {
        let requirement = module
            .identities
            .text(dependency.requirement)
            .ok_or("folded constant requirement identity is absent")?;
        let mut matches = 0;
        let mut preimage = None;
        for plan in plans.iter().copied().filter(|plan| {
            plan.target == target && plan.identity_digest().as_bytes() == &dependency.plan
        }) {
            for row in plan
                .rows
                .iter()
                .filter(|row| plan.identity_text(row.requirement_identity) == requirement)
            {
                let corresponds = match (&dependency.candidate, &row.binding) {
                    (
                        Some(association),
                        effects::provider_plan::ProviderBinding::CheckedAdapter {
                            machine_identity,
                            machine_package_identity,
                        },
                    ) => module.provider_candidates.iter().any(|candidate| {
                        candidate.boundary == association.boundary
                            && candidate.candidate == association.machine
                            && candidate.provider_declaration == association.declaration
                            && candidate.source_requirement_identity == dependency.requirement
                            && module.identities.text(candidate.provider_identity)
                                == Some(plan.provider_type.as_str())
                            && module.identities.text(candidate.source_candidate_identity)
                                == Some(plan.identity_text(*machine_identity))
                            && candidate.source_candidate_package_identity
                                == *machine_package_identity
                    }),
                    (None, effects::provider_plan::ProviderBinding::CompilerIntrinsic { .. }) => {
                        true
                    }
                    _ => false,
                };
                if corresponds {
                    matches += 1;
                    preimage = Some(plan.identity_preimage());
                }
            }
        }
        if matches != 1 {
            return Err(format!(
                "folded constant realization `{requirement}` differs from the selected provider plan"
            ));
        }
        matched.push(preimage.expect("one matching normalized plan"));
    }
    matched.sort();
    matched.dedup();
    Ok(matched)
}

fn checked_candidate_roster(
    plans: &[TerminalTargetProviderPlan],
    artifact: terminal_codec::TerminalArtifactView<'_>,
) -> Result<Vec<terminal_installation::ProviderInstallationSelection>, String> {
    effects::SelectedProviderPlanFacts::from_selected_plans(
        plans.iter().map(|selected| selected.plan.clone()).collect(),
    )?;
    let module = artifact.module();
    let mut selections = Vec::new();
    for selected in plans {
        let plan = &selected.plan;
        for row in &plan.rows {
            let effects::provider_plan::ProviderBinding::CheckedAdapter {
                machine_identity,
                machine_package_identity,
            } = &row.binding
            else {
                continue;
            };
            let owner = selected
                .provider_declaration
                .as_deref()
                .filter(|owner| !owner.is_empty())
                .ok_or("checked provider plan has no exact selecting declaration")?;
            let requirement = plan.identity_text(row.requirement_identity);
            let realization = plan.identity_text(*machine_identity);
            let matching = module
                .provider_candidates
                .iter()
                .filter(|candidate| {
                    module
                        .identities
                        .text(candidate.provider_declaration.identity())
                        == Some(owner)
                        && candidate.source_candidate_package_identity == *machine_package_identity
                        && module.identities.text(candidate.provider_identity)
                            == Some(plan.provider_type.as_str())
                        && module
                            .identities
                            .text(candidate.source_requirement_identity)
                            == Some(requirement)
                        && module.identities.text(candidate.source_candidate_identity)
                            == Some(realization)
                })
                .collect::<Vec<_>>();
            if matching.is_empty() {
                return Err(format!(
                    "selected provider requirement `{requirement}` has no emitted exact declaration candidate"
                ));
            }
            // A template can supply many distinct closed requirements, never
            // two candidates for the same coordinate under one choosing owner.
            let mut requirements = matching
                .iter()
                .map(|candidate| candidate.requirement_identity)
                .collect::<Vec<_>>();
            requirements.sort();
            if requirements.windows(2).any(|pair| pair[0] == pair[1]) {
                return Err(format!(
                    "selected provider requirement `{requirement}` has duplicate canonical realizations"
                ));
            }
            selections.extend(matching.into_iter().map(|candidate| {
                terminal_installation::ProviderInstallationSelection {
                    boundary: candidate.boundary,
                    candidate: candidate.candidate,
                    provider_identity: plan.provider_type.clone(),
                    provider_declaration: owner.to_owned(),
                }
            }));
        }
    }
    // Required parent schemas may share exactly one checked coordinate. Only
    // identical choices collapse; competing owners remain distinct for admission.
    selections.sort();
    selections.dedup();
    Ok(selections)
}

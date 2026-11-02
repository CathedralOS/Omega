//! Build-owned provider choices beside a canonical portable program.
//!
//! Plans retain authored selection, not entry backing, external host bindings
//! or native admission. Canonical candidates own the complete source-to-instance
//! association. Publication and receiving policy use that same roster projection:
//! a supplied subset cannot omit an uncalled instance or merge choosing owners.

mod synchronous_calls;
#[cfg(test)]
mod tests;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalTargetProviderProposal {
    pub target: String,
    pub terminal_psi: terminal_psi::TerminalPsiIdentity,
    /// Complete selected optimization set; pending Omega work is not applied
    /// by Terminal interpretation and this identity supplies no native authority.
    pub optimization_selection: optimization_core::OptimizationSelectionIdentity,
    /// Exact Build-bound ordinary entry proof declarations in this artifact. None denotes a
    /// reusable declaration product; Some is a complete deployment roster,
    /// including a provider-only deployment with no ProgramEntry.
    pub application_roots: Option<Vec<terminal_psi::ProofDeclarationId>>,
    /// An authored entry or provider row requires this deployment scope even
    /// when another target is the primary value consumer.
    pub application_required: bool,
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
    /// Row-aligned exact numeric external realization declarations selected while source
    /// custody exists. Callable spelling and numeric meaning cannot distinguish
    /// a retained portable declaration from its target-scoped copy. None means
    /// this row has no authored numeric realization, never permission to guess.
    /// Host/placement rows retain their independent execution authority.
    pub row_numeric_realizations: Vec<Option<String>>,
}

impl TerminalTargetProviderSelection {
    /// Project complete checked rosters from canonical associations. This
    /// constructs a proposal only; receiving target admission remains required.
    pub fn from_plans(
        plans: Vec<TerminalTargetProviderPlan>,
        artifact: terminal_codec::TerminalArtifactView<'_>,
    ) -> Result<Self, String> {
        let checked_candidates = checked_candidate_roster(&plans, artifact.module())?;
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
        let expected = checked_candidate_roster(&self.plans, artifact.module())?;
        let mut supplied = self.checked_candidates.clone();
        supplied.sort();
        if supplied != expected {
            return Err(
                "checked installations differ from the complete canonical selected-provider roster"
                    .into(),
            );
        }
        let selected = effects::SelectedProviderPlanFacts::from_selected_plans(
            self.plans
                .iter()
                .map(|selected| selected.plan.clone())
                .collect(),
        )?
        .with_assumed_bindings(self.assumed_bindings.clone());
        admit_checked_roster(artifact, target, &selected, &supplied, &self.plans)
    }
}

/// Bind the complete, already-selected native plans to canonical candidates.
/// Unlike a public ordinary provider proposal, this input retains toolchain
/// plans and inert source uses. Inert uses contribute no invented candidate;
/// every emitted closed instance still enters ordinary catalog admission.
pub fn admit_selected_checked_providers(
    artifact: terminal_codec::TerminalArtifactView<'_>,
    target: &str,
    selected: &effects::SelectedProviderPlanFacts,
    declarations: &[TerminalTargetProviderPlan],
) -> Result<terminal_installation::AdmittedProviderSelections, String> {
    require_selected_provider_declarations(selected, declarations)?;
    let mut supplied = Vec::new();
    for matched in selected_checked_candidates(declarations, artifact.module())? {
        supplied.extend(matched.installations());
    }
    supplied.sort();
    supplied.dedup();
    admit_checked_roster(artifact, target, selected, &supplied, declarations)
}

fn admit_checked_roster(
    artifact: terminal_codec::TerminalArtifactView<'_>,
    target: &str,
    selected: &effects::SelectedProviderPlanFacts,
    supplied: &[terminal_installation::ProviderInstallationSelection],
    declarations: &[TerminalTargetProviderPlan],
) -> Result<terminal_installation::AdmittedProviderSelections, String> {
    if selected
        .plans()
        .iter()
        .any(|plan| !plan.target.is_empty() && plan.target != target)
    {
        return Err("selected provider plan belongs to another deployment target".into());
    }
    let module = artifact.module();
    let target_members = terminal_installation::admit_target_machine_selections(artifact, target);
    for selection in supplied {
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
    let dependencies =
        matching_folded_constant_dependencies(artifact, target, selected.plans().iter())?;
    let installation = terminal_installation::admit_provider_installation(
        artifact,
        supplied,
        &selected
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
    synchronous_calls::validate(artifact, target, selected, declarations)?;
    if artifact.module().folded_constants.target.is_some() {
        installation.admit_folded_constant_closure(artifact, target, &dependencies)
    } else {
        Ok(installation)
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
    module: &terminal_psi::TerminalModule,
) -> Result<Vec<terminal_installation::ProviderInstallationSelection>, String> {
    effects::SelectedProviderPlanFacts::from_selected_plans(
        plans.iter().map(|selected| selected.plan.clone()).collect(),
    )?;
    let mut selections = Vec::new();
    for matched in selected_checked_candidates(plans, module)? {
        if matched.candidates.is_empty() {
            return Err(format!(
                "selected provider requirement `{}` has no emitted exact declaration candidate",
                matched
                    .selected
                    .plan
                    .identity_text(matched.row.requirement_identity),
            ));
        }
        selections.extend(matched.installations());
    }
    // Required parent schemas may share exactly one checked coordinate. Only
    // identical choices collapse; competing owners remain distinct for admission.
    selections.sort();
    selections.dedup();
    Ok(selections)
}

/// Replace source-side checked specialization projections with the canonical
/// whole-provider associations. Source-only consumers retain their own facts;
/// native production and reload use this same Terminal-owned join.
/// Source selection may contain inert whole-program uses absent from Terminal.
/// They create no closed instances here. Native admission still requires exact
/// supply for every actual demand; this projection grants no installation.
pub fn close_checked_provider_specializations(
    selected: effects::SelectedProviderPlanFacts,
    declarations: &[TerminalTargetProviderPlan],
    module: &terminal_psi::TerminalModule,
) -> Result<effects::SelectedProviderPlanFacts, String> {
    require_selected_provider_declarations(&selected, declarations)?;
    let selected = selected.without_checked_specializations();
    let mut rows = selected.specialized_requirement_rows().to_vec();
    let mut identities = selected.identities().clone().into_builder();
    for matched in selected_checked_candidates(declarations, module)? {
        let plan = &matched.selected.plan;
        for candidate in matched.candidates {
            let text = |identity| {
                module
                    .identities
                    .text(identity)
                    .filter(|text| !text.is_empty())
                    .ok_or_else(|| {
                        "canonical provider association has an empty callable identity".to_owned()
                    })
            };
            let requirement = text(candidate.requirement_identity)?;
            let template = plan.identities.text(matched.row.requirement_identity);
            // Nongeneric rows already name their executable coordinate directly.
            if requirement == template
                && candidate.candidate_identity == candidate.source_candidate_identity
            {
                continue;
            }
            rows.push(effects::SelectedRequirementSpecialization {
                provider_plan_report_identity: plan.report_fingerprint(),
                selected_requirement_identity: identities.intern(template),
                selected_machine_identity: identities
                    .intern(text(candidate.source_candidate_identity)?),
                requirement_identity: identities.intern(requirement),
                machine_identity: identities.intern(text(candidate.candidate_identity)?),
            });
        }
    }
    selected.with_specialized_requirement_rows(rows, &identities.build())
}

/// Join whole selected plans to their exact declaration roster.
/// Display names in diagnostics never participate in this comparison.
pub fn require_selected_provider_declarations(
    selected: &effects::SelectedProviderPlanFacts,
    declarations: &[TerminalTargetProviderPlan],
) -> Result<(), String> {
    if selected.plans().len() != declarations.len()
        || selected.plans().iter().any(|plan| {
            declarations
                .iter()
                .filter(|declaration| &declaration.plan == plan)
                .count()
                != 1
        })
    {
        let unmatched = selected
            .plans()
            .iter()
            .filter(|plan| {
                declarations
                    .iter()
                    .filter(|declaration| &declaration.plan == *plan)
                    .count()
                    != 1
            })
            .map(|plan| plan.name.as_str())
            .collect::<Vec<_>>();
        let extra = declarations
            .iter()
            .filter(|declaration| !selected.plans().contains(&declaration.plan))
            .map(|declaration| declaration.plan.name.as_str())
            .collect::<Vec<_>>();
        return Err(format!(
            "native provider declaration roster differs from the complete selected plans: {} plans, {} declarations; without unique exact declaration: {unmatched:?}; unselected declarations: {extra:?}",
            selected.plans().len(),
            declarations.len(),
        ));
    }
    Ok(())
}
struct SelectedCheckedCandidate<'plan, 'terminal> {
    selected: &'plan TerminalTargetProviderPlan,
    row: &'plan effects::provider_plan::ProviderPlanRow,
    owner: &'plan str,
    candidates: Vec<&'terminal terminal_psi::ProviderCandidateConformance>,
}

impl SelectedCheckedCandidate<'_, '_> {
    fn installations(
        &self,
    ) -> impl Iterator<Item = terminal_installation::ProviderInstallationSelection> + '_ {
        self.candidates.iter().map(|candidate| {
            terminal_installation::ProviderInstallationSelection {
                boundary: candidate.boundary,
                candidate: candidate.candidate,
                provider_identity: self.selected.plan.provider_type.clone(),
                provider_declaration: self.owner.to_owned(),
            }
        })
    }
}
fn selected_checked_candidates<'plan, 'terminal>(
    plans: &'plan [TerminalTargetProviderPlan],
    module: &'terminal terminal_psi::TerminalModule,
) -> Result<Vec<SelectedCheckedCandidate<'plan, 'terminal>>, String> {
    let mut selections = Vec::new();
    for selected in plans {
        let plan = &selected.plan;
        if !plan.rows.iter().any(|row| {
            matches!(
                row.binding,
                effects::provider_plan::ProviderBinding::CheckedAdapter { .. }
            )
        }) {
            continue;
        }
        let owner = selected
            .provider_declaration
            .as_deref()
            .filter(|owner| !owner.is_empty())
            .ok_or("checked provider plan has no exact selecting declaration")?;
        for projected in selected_provider_candidate_rows(plan, owner, module)? {
            selections.push(SelectedCheckedCandidate {
                selected,
                row: projected.row,
                owner,
                candidates: projected
                    .candidate_positions
                    .into_iter()
                    .map(|position| &module.provider_candidates[position])
                    .collect(),
            });
        }
    }
    Ok(selections)
}

/// One original selected requirement and all of its closed realizations in the
/// supplied immutable module. Positions have meaning only beside that module.
pub struct SelectedProviderCandidateRow<'plan> {
    pub row: &'plan effects::provider_plan::ProviderPlanRow,
    pub candidate_positions: Vec<usize>,
}

/// Project the same exact association for component verification and native
/// selection. A template may have many applications; selecting owner, package,
/// receiver and target stay part of the join even when bodies are shared.
/// Empty rows remain descriptive absence, never an invented installation.
pub fn selected_provider_candidate_rows<'plan>(
    plan: &'plan effects::provider_plan::ProviderPlan,
    owner: &str,
    module: &terminal_psi::TerminalModule,
) -> Result<Vec<SelectedProviderCandidateRow<'plan>>, String> {
    if owner.is_empty() {
        return Err("checked provider plan has no exact selecting declaration".into());
    }
    let mut selections = Vec::new();
    for row in &plan.rows {
        let effects::provider_plan::ProviderBinding::CheckedAdapter {
            machine_identity,
            machine_package_identity,
        } = &row.binding
        else {
            continue;
        };
        let requirement = plan.identity_text(row.requirement_identity);
        let realization = plan.identity_text(*machine_identity);
        let matching = module
            .provider_candidates
            .iter()
            .enumerate()
            .filter(|(_, candidate)| {
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
                        && receiving_application_matches(plan, row, module, candidate.receiving_application)
                        // The portable catalog retains every member of the
                        // remaining legacy families. A target-keyed plan joins
                        // only its exact member before checking uniqueness;
                        // other targets are not competing implementations.
                        && module.target_machine_families.iter().all(|family| {
                            let callable = terminal_psi::TargetMachineCallable::Machine(
                                candidate.candidate,
                            );
                            !family.members.iter().any(|member| member.callable == callable)
                                || family.members.iter().any(|member| {
                                    member.target == plan.target && member.callable == callable
                                })
                        })
            })
            .map(|(position, _)| position)
            .collect::<Vec<_>>();
        // A template can supply many distinct closed requirements, never
        // two candidates for the same coordinate under one choosing owner.
        let mut requirements = matching
            .iter()
            .map(|position| module.provider_candidates[*position].requirement_identity)
            .collect::<Vec<_>>();
        requirements.sort();
        if requirements.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(format!(
                "selected provider requirement `{requirement}` has duplicate canonical realizations: {:?}",
                matching
                    .iter()
                    .map(|position| &module.provider_candidates[*position])
                    .map(|candidate| (
                        candidate.candidate,
                        module.identities.text(candidate.candidate_identity),
                        module
                            .machine(candidate.candidate)
                            .and_then(|machine| machine.declaration.as_ref())
                            .and_then(|declaration| module
                                .identities
                                .text(declaration.semantic_declaration)),
                        module.identities.text(candidate.receiving_application),
                    ))
                    .collect::<Vec<_>>()
            ));
        }
        selections.push(SelectedProviderCandidateRow {
            row,
            candidate_positions: matching,
        });
    }
    // The obligation roster is issued independently of candidates. Checking
    // only the candidate list cannot notice one missing uncalled instance.
    for selected in &selections {
        let requirement_template = plan.identity_text(selected.row.requirement_identity);
        let method = plan
            .schema
            .declaration
            .methods
            .iter()
            .find(|method| {
                plan.schema.identity_text(method.requirement_identity) == requirement_template
            })
            .ok_or("selected provider row lost its exact requirement declaration")?;
        for obligations in module
            .provider_instance_requirements
            .iter()
            .filter(|obligations| {
                obligations.package == method.requirement_owner_package_identity
                    && module.identities.text(obligations.template) == Some(requirement_template)
                    && obligations
                        .selecting_declaration
                        .as_ref()
                        .is_none_or(|declaration| {
                            module.identities.text(declaration.identity()) == Some(owner)
                                && match selected.row.binding {
                                    effects::provider_plan::ProviderBinding::CheckedAdapter {
                                        machine_identity,
                                        ..
                                    } => {
                                        module
                                            .identities
                                            .text(obligations.source_candidate_identity)
                                            == Some(plan.identity_text(machine_identity))
                                    }
                                    _ => false,
                                }
                        })
            })
        {
            for required in &obligations.required_instances {
                if !receiving_application_matches(
                    plan,
                    selected.row,
                    module,
                    required.receiving_application,
                ) {
                    continue;
                }
                let count = selected
                    .candidate_positions
                    .iter()
                    .filter(|position| {
                        let candidate = &module.provider_candidates[**position];
                        candidate.requirement_identity == required.requirement_identity
                            && (required.candidate_identity.is_absent()
                                || candidate.candidate_identity == required.candidate_identity)
                            && candidate.receiving_application == required.receiving_application
                            && candidate.receiving_region_arguments
                                == required.receiving_region_arguments
                    })
                    .count();
                if count != 1 {
                    return Err(format!(
                        "selected provider plan `{}` resolves retained closed requirement `{}` to {count} canonical provider instances; exactly one must serve it",
                        plan.name,
                        module
                            .identities
                            .text(required.requirement_identity)
                            .unwrap_or("<unknown requirement>"),
                    ));
                }
            }
        }
    }
    Ok(selections)
}

fn receiving_application_matches(
    plan: &effects::provider_plan::ProviderPlan,
    row: &effects::provider_plan::ProviderPlanRow,
    module: &terminal_psi::TerminalModule,
    receiving_application: terminal_psi::TerminalIdentityIndex,
) -> bool {
    let requirement = plan.identity_text(row.requirement_identity);
    let surface = &plan.schema.declaration;
    let Some(method) = surface
        .methods
        .iter()
        .find(|method| surface.identity_text(method.requirement_identity) == requirement)
    else {
        return false;
    };
    let views = || {
        surface.receiver_applications.iter().filter(|application| {
            surface.identity_text(application.trait_name) == method.requirement_owner
                && application.trait_package_identity == method.requirement_owner_package_identity
                && application
                    .requirement_identities
                    .iter()
                    .any(|identity| surface.identity_text(*identity) == requirement)
        })
    };
    if !surface.receiver_applications.is_empty() && views().next().is_none() {
        return false;
    }
    if receiving_application == terminal_psi::TerminalIdentityIndex::default() {
        // Non-applied declarations have no closed receiver coordinate. They
        // cannot supply a parameterized or region-applied receiver selection.
        return views().all(|application| {
            application.argument_identities.is_empty()
                && application.lifetime_argument_identities.is_empty()
        });
    }
    let Some(receiving) = module
        .identities
        .text(receiving_application)
        .filter(|text| !text.is_empty())
    else {
        return false;
    };
    views().any(|application| surface.identity_text(application.application) == receiving)
}

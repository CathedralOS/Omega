/*
We bind Build-selected target provider plans to canonical Terminal candidate
rosters, closing checked specialization projections, synchronous invocation
cycles, and folded-constant dependencies so publication and receiving
admission enforce the same complete source-to-instance association.
*/

use crate::synchronous_invocations::{
    SynchronousBoundaryTarget, validate_selected_synchronous_invocation_cycles,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalTargetProviderProposal {
    pub target: String,
    pub terminal_psi: terminal_psi::TerminalPsiIdentity,
    pub optimization_selection: optimization_core::OptimizationSelectionIdentity,
    pub application_roots: Option<Vec<terminal_psi::ProofDeclarationId>>,
    pub application_required: bool,
    pub selection: Result<TerminalTargetProviderSelection, Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalTargetProviderSelection {
    pub plans: Vec<TerminalTargetProviderPlan>,
    pub assumed_bindings: Vec<effects::AssumedBindingContractSelection>,
    pub checked_candidates: Vec<terminal_installation::ProviderInstallationSelection>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalTargetProviderPlan {
    pub plan: effects::provider_plan::ProviderPlan,
    pub provider_declaration: Option<String>,
    pub row_numeric_realizations: Vec<Option<String>>,
}

// A retained provider plan belongs to a deployment already selected by its
// caller. Missing identity is not a request to select the compiler's host.
fn selected_plan_target(plan: &effects::provider_plan::ProviderPlan) -> Result<&str, String> {
    if plan.target.is_empty() {
        return Err("selected provider plan has no deployment target".into());
    }
    Ok(&plan.target)
}

pub fn selected_checked_adapter_header<'module>(
    plan: &effects::provider_plan::ProviderPlan,
    row: &effects::provider_plan::ProviderPlanRow,
    module: &'module terminal_psi::TerminalModule,
) -> Result<&'module terminal_psi::ProofCallableHeader, String> {
    let effects::provider_plan::ProviderBinding::CheckedAdapter {
        machine_identity,
        machine_package_identity,
    } = &row.binding
    else {
        return Err("selected row is not a checked adapter".into());
    };
    if *machine_package_identity != plan.origin_package_identity {
        return Err("checked adapter differs from its retained package identity".into());
    }
    let callable = plan.identity_text(*machine_identity);
    if callable.is_empty() {
        return Err("checked adapter has no complete callable identity".into());
    }
    let target = selected_plan_target(plan)?;
    let mut matches = module
        .scalar_applications
        .proof_scope
        .callable_headers
        .iter()
        .map(|(_, header)| header)
        .filter(|header| {
            header.product_eligible
                && header.package == *machine_package_identity
                && header
                    .source_target
                    .as_deref()
                    .is_none_or(|source| source == target)
                && header
                    .callable
                    .as_ref()
                    .is_ok_and(|identity| module.identities.text(*identity) == Some(callable))
        });
    let header = matches.next().ok_or_else(|| {
        format!("checked adapter `{callable}` has no exact canonical declaration header")
    })?;
    if matches.next().is_some() {
        return Err(format!(
            "checked adapter `{callable}` has multiple exact canonical declaration headers"
        ));
    }
    Ok(header)
}

pub struct SelectedCheckedInvocationRow<'plan, 'module> {
    pub plan: &'plan effects::provider_plan::ProviderPlan,
    pub row: &'plan effects::provider_plan::ProviderPlanRow,
    pub headers: Vec<&'module terminal_psi::ProofCallableHeader>,
}

pub fn selected_checked_invocation_rows<'plan, 'module>(
    plans: &'plan [TerminalTargetProviderPlan],
    module: &'module terminal_psi::TerminalModule,
) -> Result<Vec<SelectedCheckedInvocationRow<'plan, 'module>>, String> {
    let scope = &module.scalar_applications.proof_scope;
    let mut rows = Vec::new();
    for selected in selected_checked_candidates(plans, module)? {
        let plan = &selected.selected.plan;
        let effects::provider_plan::ProviderBinding::CheckedAdapter {
            machine_package_identity,
            ..
        } = &selected.row.binding
        else {
            return Err("canonical checked invocation row lost its adapter binding".into());
        };
        if *machine_package_identity != plan.origin_package_identity {
            return Err("checked adapter differs from its retained package identity".into());
        }
        let target = selected_plan_target(plan)?;
        let mut headers = Vec::new();
        for candidate in selected.candidates {
            let callable = terminal_psi::TargetMachineCallable::Machine(candidate.candidate);
            let declaration = scope.callable_declaration(callable).ok_or_else(|| {
                "checked invocation candidate has no canonical proof declaration".to_owned()
            })?;
            let mut matches = scope
                .callable_headers
                .iter()
                .map(|(_, header)| header)
                .filter(|header| header.declaration == declaration);
            let header = matches.next().ok_or_else(|| {
                "checked invocation candidate has no canonical callable header".to_owned()
            })?;
            if matches.next().is_some() {
                return Err("checked invocation candidate has duplicate callable headers".into());
            }
            if !header.product_eligible
                || !header.supplies_checked_body
                || header.package != *machine_package_identity
                || header
                    .source_target
                    .as_deref()
                    .is_some_and(|source| source != target)
                || !header
                    .callable
                    .as_ref()
                    .is_ok_and(|identity| *identity == candidate.candidate_identity)
            {
                return Err("checked invocation candidate header differs from its selected body, package or target".into());
            }
            headers.push(header);
        }
        rows.push(SelectedCheckedInvocationRow {
            plan,
            row: selected.row,
            headers,
        });
    }
    Ok(rows)
}

impl TerminalTargetProviderSelection {
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
    for plan in selected.plans() {
        if selected_plan_target(plan)? != target {
            return Err("selected provider plan belongs to another deployment target".into());
        }
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
    validate_synchronous_calls(artifact, target, selected, declarations)?;
    if artifact.module().folded_constants.target.is_some() {
        installation.admit_folded_constant_closure(artifact, target, &dependencies)
    } else {
        Ok(installation)
    }
}

fn validate_synchronous_calls(
    artifact: terminal_codec::TerminalArtifactView<'_>,
    target: &str,
    selected: &effects::SelectedProviderPlanFacts,
    declarations: &[TerminalTargetProviderPlan],
) -> Result<(), String> {
    let module = artifact.module();
    let candidates = selected_checked_candidates(declarations, module)?;
    let target_members = terminal_installation::admit_target_machine_selections(artifact, target);
    let calls =
        terminal_installation::SynchronousCallGraph::new(module, Some(&target_members), None)?;
    validate_selected_synchronous_invocation_cycles(selected.plans(), |plan, row| {
        let failure = |reason: String| {
            diagnostics::Diagnostic::error(format!(
                "selected provider `{}` synchronous invocation admission failed: {reason}",
                plan.name,
            ))
        };
        let matched = candidates
            .iter()
            .find(|candidate| &candidate.selected.plan == plan && candidate.row == row)
            .ok_or_else(|| {
                failure("checked row lost its canonical selection association".into())
            })?;
        let mut targets = Vec::new();
        for candidate in &matched.candidates {
            let reached = calls.from_root(candidate.candidate).map_err(&failure)?;
            for boundary in reached.boundaries {
                let declaration = module.boundary_machine(boundary).ok_or_else(|| {
                    failure("invoked boundary has no canonical declaration".into())
                })?;
                let requirement = module
                    .identities
                    .text(declaration.identity)
                    .ok_or_else(|| failure("invoked boundary has no canonical identity".into()))?;
                let supplied = selected
                    .assumed_supplier_identity(requirement)
                    .map_err(&failure)?;
                let mut slots = candidates
                    .iter()
                    .filter(|matched| {
                        matched.candidates.iter().any(|candidate| {
                            module.identities.text(candidate.requirement_identity) == Some(supplied)
                        })
                    })
                    .map(|matched| &matched.selected.plan)
                    .collect::<Vec<_>>();
                if let Some((plan, _)) = selected.selected_requirement_row(supplied) {
                    slots.push(plan);
                }
                slots.dedup();
                match slots.as_slice() {
                    [] => {}
                    [slot] => targets.push(slot_target(slot)),
                    _ => {
                        return Err(failure(format!(
                            "invoked requirement `{requirement}` has competing selected slots"
                        )));
                    }
                }
            }
        }
        Ok(targets)
    })
    .map_err(|diagnostics| {
        diagnostics
            .into_iter()
            .map(|diagnostic| diagnostic.message)
            .collect::<Vec<_>>()
            .join("; ")
    })
}

fn slot_target(plan: &effects::provider_plan::ProviderPlan) -> SynchronousBoundaryTarget {
    SynchronousBoundaryTarget::Selected {
        plan: plan.identity_digest(),
        label: plan.name.clone(),
    }
}

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

/// A selected row uses the complete canonical service contract, rather than
/// a same-shaped schema supplied beside genuine candidate coordinates.
fn validate_selected_service_schemas(
    plans: &[TerminalTargetProviderPlan],
    module: &terminal_psi::TerminalModule,
) -> Result<(), String> {
    for selected in plans {
        let plan = &selected.plan;
        let mut choices = module.provider_selections.iter().filter(|choice| {
            choice.target == plan.target
                && module.identities.text(choice.provider_declaration)
                    == selected.provider_declaration.as_deref()
                && module.identities.text(choice.provider_identity)
                    == Some(plan.provider_type.as_str())
                && choice.provider_package == plan.provider_type_package_identity
        });
        let mut matched = None;
        for choice in &mut choices {
            let application = choice
                .provider_application
                .map(|identity| {
                    module
                        .identities
                        .text(identity)
                        .filter(|value| !value.is_empty())
                        .ok_or_else(|| {
                            "selected provider has an unresolved static value application"
                                .to_owned()
                        })
                })
                .transpose()?;
            if application != plan.application_identity() {
                continue;
            }
            let choice = module.provider_selection(
                &choice.target,
                choice.schema_declaration,
                choice.schema_application,
            )?;
            let schema =
                module.service_schema(choice.schema_declaration, choice.schema_application)?;
            if schema.surface.as_ref() != plan.schema.declaration.as_ref() {
                continue;
            }
            if matched.replace(choice).is_some() {
                return Err(
                    "provider plan has competing canonical selected schema applications".into(),
                );
            }
        }
        if matched.is_none() {
            return Err(
                "provider plan differs from its complete canonical chosen service contract".into(),
            );
        }
    }
    Ok(())
}

fn checked_candidate_roster(
    plans: &[TerminalTargetProviderPlan],
    module: &terminal_psi::TerminalModule,
) -> Result<Vec<terminal_installation::ProviderInstallationSelection>, String> {
    validate_selected_service_schemas(plans, module)?;
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
    selections.sort();
    selections.dedup();
    Ok(selections)
}

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

pub struct SelectedProviderCandidateRow<'plan> {
    pub row: &'plan effects::provider_plan::ProviderPlanRow,
    pub candidate_positions: Vec<usize>,
}

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
                    && receiving_application_matches(
                        plan,
                        row,
                        module,
                        candidate.declaration_application,
                    )
                    && module.target_machine_families.iter().all(|family| {
                        let callable =
                            terminal_psi::TargetMachineCallable::Machine(candidate.candidate);
                        !family
                            .members
                            .iter()
                            .any(|member| member.callable == callable)
                            || family.members.iter().any(|member| {
                                member.target == plan.target && member.callable == callable
                            })
                    })
            })
            .map(|(position, _)| position)
            .collect::<Vec<_>>();
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
                    required.declaration_application,
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
                            && candidate.declaration_application == required.declaration_application
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
    let receiving = if receiving_application == terminal_psi::TerminalIdentityIndex::default() {
        None
    } else {
        let Some(receiving) = module.identities.text(receiving_application) else {
            return false;
        };
        Some(receiving)
    };
    plan.schema.declaration.matches_receiving_application(
        plan.identity_text(row.declared_requirement_identity),
        receiving,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use effects::provider_plan::{
        ProviderBinding, ProviderIdentityTable, ProviderPlan, ProviderPlanRow, ServiceMethod,
        ServiceSchema,
    };
    use terminal_psi::{
        NominalDeclarationIdentity, ProviderCandidateConformance, TerminalIdentityIndex,
        TerminalIdentityTableBuilder, TerminalModule,
    };

    fn fixture() -> (TerminalTargetProviderPlan, TerminalModule) {
        let mut identities = ProviderIdentityTable::default();
        let requirement = identities.intern("Parent::read");
        let machine = identities.intern("Shared::read");
        let mut schema_identities = ProviderIdentityTable::default();
        let trait_name = schema_identities.intern("Child");
        let schema_requirement = schema_identities.intern("Parent::read");
        let parent_name = schema_identities.intern("Parent");
        let parent_application = schema_identities.intern("Parent<u64>");
        let argument = schema_identities.intern("u64");
        let selected = TerminalTargetProviderPlan {
            provider_declaration: Some("FirstMap".into()),
            row_numeric_realizations: vec![None],
            plan: ProviderPlan {
                name: "Child selected".into(),
                provider_type: "First".into(),
                target: "macos_arm64".into(),
                identities,
                schema: ServiceSchema::from_declaration(std::sync::Arc::new(
                    effects::provider_plan::ServiceSurface {
                        trait_name,
                        receiver_applications: vec![
                            effects::provider_plan::ServiceReceiverApplication {
                                application: trait_name,
                                trait_name,
                                trait_package_identity: None,
                                argument_identities: vec![],
                                lifetime_argument_identities: vec![],
                                requirement_identities: vec![schema_requirement],
                            },
                            effects::provider_plan::ServiceReceiverApplication {
                                application: parent_application,
                                trait_name: parent_name,
                                trait_package_identity: None,
                                argument_identities: vec![argument],
                                lifetime_argument_identities: vec![],
                                requirement_identities: vec![schema_requirement],
                            },
                        ],
                        identities: schema_identities,
                        methods: vec![ServiceMethod {
                            name: "read".into(),
                            requirement_owner: "Parent".into(),
                            requirement_identity: schema_requirement,
                            completion: semantic_vocabulary::BoundaryCompletion::NormalReturn,
                            ..ServiceMethod::default()
                        }],
                        ..effects::provider_plan::ServiceSurface::default()
                    },
                )),
                rows: vec![ProviderPlanRow {
                    method: "read".into(),
                    requirement_identity: requirement,
                    declared_requirement_identity: requirement,
                    calling_signature: None,
                    requirement_lifetime_partition: Vec::new(),
                    admitted_same_stack_bytes: 0,
                    binding: ProviderBinding::CheckedAdapter {
                        machine_identity: machine,
                        machine_package_identity: None,
                    },
                }],
                ..ProviderPlan::default()
            },
        };
        let mut identities = TerminalIdentityTableBuilder::default();
        let mut module = TerminalModule::for_entry(semantic_vocabulary::MachineId::new(1).unwrap());
        for (ordinal, (owner, tuple)) in [
            ("FirstMap", "4"),
            ("FirstMap", "8"),
            ("SecondMap", "4"),
            ("SecondMap", "8"),
        ]
        .into_iter()
        .enumerate()
        {
            module
                .provider_candidates
                .push(ProviderCandidateConformance {
                    boundary: semantic_vocabulary::BoundaryMachineId::new((ordinal % 2 + 1) as u64)
                        .unwrap(),
                    requirement_identity: identities.intern(&format!("Parent::read<{tuple}>")),
                    source_requirement_identity: identities.intern("Parent::read"),
                    declaration_application: identities.intern("Parent<u64>"),
                    receiving_application: identities.intern("Parent<u64>"),
                    receiving_region_arguments: Vec::new(),
                    provider_identity: identities.intern("First"),
                    provider_declaration: NominalDeclarationIdentity::new(identities.intern(owner)),
                    candidate_identity: identities.intern(&format!("Shared::read<{tuple}>")),
                    source_candidate_identity: identities.intern("Shared::read"),
                    source_candidate_package_identity: None,
                    candidate: semantic_vocabulary::MachineId::new((ordinal % 2 + 1) as u64)
                        .unwrap(),
                    signature: terminal_psi::ProviderSignature::empty(),
                    refinement: terminal_psi::ProviderRefinement::empty(),
                });
        }
        module.identities = identities.build();
        (selected, module)
    }

    fn invocation_fixture() -> (TerminalTargetProviderPlan, TerminalModule) {
        let (selected, mut module) = fixture();
        let candidates = module.provider_candidates[..2].to_vec();
        module
            .provider_instance_requirements
            .push(terminal_psi::ProviderInstanceRequirements {
                template: candidates[0].source_requirement_identity,
                package: None,
                selecting_declaration: Some(candidates[0].provider_declaration.clone()),
                source_candidate_identity: candidates[0].source_candidate_identity,
                required_instances: candidates
                    .iter()
                    .map(|candidate| terminal_psi::RequiredProviderInstance {
                        requirement_identity: candidate.requirement_identity,
                        candidate_identity: candidate.candidate_identity,
                        declaration_application: candidate.declaration_application,
                        receiving_application: candidate.receiving_application,
                        receiving_region_arguments: candidate.receiving_region_arguments.clone(),
                    })
                    .collect(),
            });
        let scope = &mut module.scalar_applications.proof_scope;
        for (index, candidate) in candidates.iter().enumerate() {
            let declaration = terminal_psi::ProofDeclarationId(index as u32 + 1);
            scope
                .callables
                .insert(terminal_psi::ProofCallableAssociation::Callable {
                    declaration,
                    callable: terminal_psi::TargetMachineCallable::Machine(candidate.candidate),
                });
            scope
                .callable_headers
                .insert(terminal_psi::ProofCallableHeader {
                    declaration,
                    callable: Ok(candidate.candidate_identity),
                    package: None,
                    source_target: Some("macos_arm64".into()),
                    product_eligible: true,
                    supplies_checked_body: true,
                    type_parameter_count: 0,
                    synchronous_invocations: Ok(Vec::new()),
                    compiler_evaluation: Err("not an evaluation fixture".into()),
                });
        }
        scope
            .callable_headers
            .insert(terminal_psi::ProofCallableHeader {
                declaration: terminal_psi::ProofDeclarationId(3),
                callable: Ok(candidates[0].source_candidate_identity),
                package: None,
                source_target: None,
                product_eligible: true,
                supplies_checked_body: true,
                type_parameter_count: 1,
                synchronous_invocations: Err("open template invocation".into()),
                compiler_evaluation: Err("not an evaluation fixture".into()),
            });
        (selected, module)
    }

    #[test]
    fn selected_provider_headers_require_the_retained_deployment() {
        let (mut selected, module) = invocation_fixture();
        let row = &selected.plan.rows[0];
        assert!(selected_checked_adapter_header(&selected.plan, row, &module).is_ok());
        assert!(selected_checked_invocation_rows(std::slice::from_ref(&selected), &module).is_ok());

        selected.plan.target.clear();
        let row = &selected.plan.rows[0];
        assert_eq!(
            selected_checked_adapter_header(&selected.plan, row, &module).unwrap_err(),
            "selected provider plan has no deployment target"
        );
        assert_eq!(
            selected_checked_invocation_rows(&[selected], &module)
                .err()
                .unwrap(),
            "selected provider plan has no deployment target"
        );
    }

    #[test]
    fn selected_provider_roster_does_not_treat_a_missing_target_as_a_wildcard() {
        let (mut declaration, mut module) = invocation_fixture();
        module.provider_candidates.sort_by_key(|candidate| {
            (
                candidate.boundary,
                module.identities.text(candidate.provider_identity),
                module
                    .identities
                    .text(candidate.provider_declaration.identity()),
                module.identities.text(candidate.candidate_identity),
                candidate.candidate,
            )
        });
        let optimization =
            terminal_codec::build_identity_optimization_execution_record(&module).unwrap();
        let artifact =
            terminal_codec::CanonicalTerminalArtifact::from_parts(module, optimization, None)
                .unwrap();
        declaration.plan.target.clear();
        let selected =
            effects::SelectedProviderPlanFacts::from_selected_plans(vec![declaration.plan.clone()])
                .unwrap();
        assert_eq!(
            admit_checked_roster(
                artifact.view(),
                "macos_arm64",
                &selected,
                &[],
                &[declaration]
            )
            .unwrap_err(),
            "selected provider plan has no deployment target"
        );
    }

    #[test]
    fn invocation_rows_consume_all_closed_headers_instead_of_the_open_template() {
        let (selected, mut module) = invocation_fixture();
        let plans = [selected];
        let rows = selected_checked_invocation_rows(&plans, &module).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].headers.len(), 2);
        assert_eq!(
            rows[0].headers[1].declaration,
            terminal_psi::ProofDeclarationId(2)
        );
        crate::synchronous_invocations::validate_declared_synchronous_invocation_cycles(
            &plans, &module,
        )
        .unwrap();
        let handle = module
            .scalar_applications
            .proof_scope
            .callable_headers
            .iter()
            .find_map(|(handle, header)| {
                (header.declaration == terminal_psi::ProofDeclarationId(2)).then_some(handle)
            })
            .unwrap();
        module
            .scalar_applications
            .proof_scope
            .callable_headers
            .get_mut(handle)
            .synchronous_invocations = Err("missing uncalled summary".into());
        let errors =
            crate::synchronous_invocations::validate_declared_synchronous_invocation_cycles(
                &plans, &module,
            )
            .unwrap_err();
        assert!(
            errors
                .iter()
                .any(|error| error.message == "missing uncalled summary")
        );
    }

    #[test]
    fn an_uncalled_closed_invocation_still_forms_a_selected_cycle() {
        let (selected, mut module) = invocation_fixture();
        let mut identities = module.identities.clone().into_builder();
        let declaration = identities.intern("Parent");
        let application = identities.intern("Parent<u64>");
        module.identities = identities.build();
        let headers = &mut module.scalar_applications.proof_scope.callable_headers;
        let handle = headers
            .iter()
            .find_map(|(handle, header)| {
                (header.declaration == terminal_psi::ProofDeclarationId(2)).then_some(handle)
            })
            .unwrap();
        headers.get_mut(handle).synchronous_invocations =
            Ok(vec![terminal_psi::DeclaredSynchronousBoundaryTarget {
                declaration,
                package: None,
                application: Some(application),
            }]);
        let errors =
            crate::synchronous_invocations::validate_declared_synchronous_invocation_cycles(
                &[selected],
                &module,
            )
            .unwrap_err();
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains("cyclic synchronous"))
        );
    }

    #[test]
    fn closed_invocation_headers_retain_selected_eligibility_and_instance_obligations() {
        let (selected, module) = invocation_fixture();
        for mismatch in [
            "origin",
            "package",
            "target",
            "callable",
            "product",
            "body",
            "header",
            "association",
            "instance",
            "duplicate",
        ] {
            let mut selected = selected.clone();
            let mut changed = module.clone();
            let header = changed
                .scalar_applications
                .proof_scope
                .callable_headers
                .iter()
                .next()
                .unwrap()
                .0;
            match mismatch {
                "origin" => {
                    selected.plan.origin_package_identity =
                        semantic_vocabulary::PackageKeyIdentity::from_digest([9; 32])
                }
                "package" => {
                    changed
                        .scalar_applications
                        .proof_scope
                        .callable_headers
                        .get_mut(header)
                        .package = semantic_vocabulary::PackageKeyIdentity::from_digest([9; 32])
                }
                "target" => {
                    changed
                        .scalar_applications
                        .proof_scope
                        .callable_headers
                        .get_mut(header)
                        .source_target = Some("windows_x86_64".into())
                }
                "callable" => {
                    changed
                        .scalar_applications
                        .proof_scope
                        .callable_headers
                        .get_mut(header)
                        .callable = Ok(TerminalIdentityIndex::ABSENT)
                }
                "product" => {
                    changed
                        .scalar_applications
                        .proof_scope
                        .callable_headers
                        .get_mut(header)
                        .product_eligible = false
                }
                "body" => {
                    changed
                        .scalar_applications
                        .proof_scope
                        .callable_headers
                        .get_mut(header)
                        .supplies_checked_body = false
                }
                "header" => {
                    changed.scalar_applications.proof_scope.callable_headers = Default::default()
                }
                "association" => {
                    changed.scalar_applications.proof_scope.callables = Default::default()
                }
                "instance" => {
                    let mut identities = changed.identities.clone().into_builder();
                    let other_application = identities.intern("Parent<bool>");
                    for candidate in &mut changed.provider_candidates {
                        candidate.receiving_application = other_application;
                    }
                    changed.identities = identities.build();
                }
                _ => {
                    let duplicate = changed
                        .scalar_applications
                        .proof_scope
                        .callable_headers
                        .get(header)
                        .clone();
                    changed
                        .scalar_applications
                        .proof_scope
                        .callable_headers
                        .insert(duplicate);
                }
            }
            assert!(
                selected_checked_invocation_rows(&[selected], &changed).is_err(),
                "{mismatch}"
            );
        }
    }

    #[test]
    fn canonical_instances_replace_source_rows_and_keep_uncalled_inherited_coordinates() {
        let (declaration, module) = fixture();
        let selected =
            effects::SelectedProviderPlanFacts::from_selected_plans(vec![declaration.plan.clone()])
                .unwrap();
        let mut identities = module.identities.clone().into_builder();
        let stale = effects::SelectedRequirementSpecialization {
            provider_plan_report_identity: declaration.plan.report_fingerprint(),
            selected_requirement_identity: identities.intern("Parent::read"),
            selected_machine_identity: identities.intern("Shared::read"),
            requirement_identity: identities.intern("Parent::read<999>"),
            machine_identity: identities.intern("StaleSourceBody"),
        };
        let selected = selected
            .with_specialized_requirement_rows(vec![stale], &identities.build())
            .unwrap();
        let closed =
            close_checked_provider_specializations(selected, &[declaration], &module).unwrap();
        assert!(
            closed
                .specialized_requirement_row("Parent::read<999>")
                .is_none()
        );
        assert_eq!(closed.specialized_requirement_rows().len(), 2);
        for tuple in ["4", "8"] {
            let row = closed
                .specialized_requirement_row(&format!("Parent::read<{tuple}>"))
                .unwrap();
            assert_eq!(
                closed.identities().text(row.machine_identity),
                Some(format!("Shared::read<{tuple}>").as_str())
            );
        }
        let stripped = closed.clone().without_checked_specializations();
        assert!(stripped.specialized_requirement_rows().is_empty());
        assert_eq!(stripped.plans(), closed.plans());
        assert!(stripped.recovery_candidate().validate().is_ok());
    }

    #[test]
    fn canonical_projection_preserves_partial_toolchain_plan_custody() {
        let (mut declaration, module) = fixture();
        let mut missing = declaration.plan.schema.declaration.methods[0].clone();
        missing.name = "unavailable".into();
        let surface = std::sync::Arc::make_mut(&mut declaration.plan.schema.declaration);
        missing.requirement_identity = surface.identities.intern("Parent::unavailable");
        for application in &mut surface.receiver_applications {
            application
                .requirement_identities
                .push(missing.requirement_identity);
        }
        surface.methods.push(missing);
        assert!(
            effects::SelectedProviderPlanFacts::from_selected_plans(vec![declaration.plan.clone()])
                .is_err()
        );
        let selected = effects::SelectedProviderPlanFacts::default()
            .with_toolchain_settled_plan(declaration.plan.clone())
            .unwrap();
        let closed = close_checked_provider_specializations(
            selected,
            std::slice::from_ref(&declaration),
            &module,
        )
        .unwrap();
        assert!(closed.is_toolchain_settled(&declaration.plan));
        assert_eq!(closed.specialized_requirement_rows().len(), 2);
    }

    #[test]
    fn canonical_projection_joins_exact_target_before_rejecting_duplicate_coordinates() {
        let (mut declaration, mut module) = fixture();
        let first = module.provider_candidates[0].clone();
        let mut alternate = first.clone();
        alternate.candidate = semantic_vocabulary::MachineId::new(5).unwrap();
        module.provider_candidates = vec![first.clone(), alternate.clone()];
        let callable = |candidate| terminal_psi::TargetMachineCallable::Machine(candidate);
        module.target_machine_families = vec![terminal_psi::TargetMachineFamily {
            canonical: terminal_psi::TargetMachineCallable::Boundary(first.boundary),
            head_contract: [0; 32],
            members: vec![
                terminal_psi::TargetMachineFamilyMember {
                    target: "macos_arm64".into(),
                    callable: callable(first.candidate),
                    supply: terminal_psi::TargetMachineSupply::CheckedBody,
                },
                terminal_psi::TargetMachineFamilyMember {
                    target: "windows_x86_64".into(),
                    callable: callable(alternate.candidate),
                    supply: terminal_psi::TargetMachineSupply::CheckedBody,
                },
            ],
        }];
        for (target, expected) in [
            ("macos_arm64", first.candidate),
            ("windows_x86_64", alternate.candidate),
        ] {
            declaration.plan.target = target.into();
            let selected = checked_candidate_roster(std::slice::from_ref(&declaration), &module)
                .expect("distinct target members are not duplicates");
            assert_eq!(selected.len(), 1);
            assert_eq!(selected[0].candidate, expected);
        }
        module.provider_candidates.push(alternate);
        assert!(
            checked_candidate_roster(std::slice::from_ref(&declaration), &module)
                .unwrap_err()
                .contains("duplicate canonical realizations")
        );
        declaration.plan.target = "linux_arm64".into();
        assert!(
            checked_candidate_roster(std::slice::from_ref(&declaration), &module)
                .unwrap_err()
                .contains("no emitted exact declaration candidate")
        );
    }

    #[test]
    fn canonical_join_rejects_missing_owner_template_and_duplicate_coordinate() {
        let (declaration, module) = fixture();
        for mismatch in [
            "owner",
            "requirement",
            "body",
            "receiver",
            "package",
            "duplicate",
        ] {
            let mut changed = module.clone();
            match mismatch {
                "owner" => changed.provider_candidates.retain(|row| {
                    module.identities.text(row.provider_declaration.identity()) != Some("FirstMap")
                }),
                "requirement" => changed.provider_candidates.iter_mut().for_each(|row| {
                    row.source_requirement_identity = TerminalIdentityIndex::ABSENT
                }),
                "body" => changed
                    .provider_candidates
                    .iter_mut()
                    .for_each(|row| row.source_candidate_identity = TerminalIdentityIndex::ABSENT),
                "receiver" => {
                    let mut identities = changed.identities.clone().into_builder();
                    let substituted = identities.intern("Parent<bool>");
                    for row in &mut changed.provider_candidates {
                        row.declaration_application = substituted;
                        row.receiving_application = substituted;
                    }
                    changed.identities = identities.build();
                }
                "package" => {
                    for row in &mut changed.provider_candidates {
                        row.source_candidate_package_identity =
                            semantic_vocabulary::PackageKeyIdentity::from_digest([9; 32]);
                    }
                }
                _ => changed
                    .provider_candidates
                    .push(changed.provider_candidates[0].clone()),
            }
            let error = checked_candidate_roster(std::slice::from_ref(&declaration), &changed)
                .expect_err(mismatch);
            assert!(
                error.contains(if mismatch == "duplicate" {
                    "duplicate canonical realizations"
                } else {
                    "no emitted exact declaration candidate"
                }),
                "{mismatch}: {error}"
            );
        }
    }

    #[test]
    fn component_and_native_projection_retain_the_same_complete_closed_roster() {
        let (selected, module) = fixture();
        let projected =
            selected_provider_candidate_rows(&selected.plan, "FirstMap", &module).unwrap();
        assert_eq!(projected.len(), 1);
        assert_eq!(projected[0].candidate_positions, vec![0, 1]);
        let installations =
            checked_candidate_roster(std::slice::from_ref(&selected), &module).unwrap();
        assert_eq!(installations.len(), projected[0].candidate_positions.len());
        for (installation, position) in installations.iter().zip(&projected[0].candidate_positions)
        {
            assert_eq!(
                installation.boundary,
                module.provider_candidates[*position].boundary
            );
            assert_eq!(
                installation.candidate,
                module.provider_candidates[*position].candidate
            );
            assert_eq!(installation.provider_declaration, "FirstMap");
        }
        assert!(selected_provider_candidate_rows(&selected.plan, "", &module).is_err());
    }

    #[test]
    fn inherited_requirement_uses_its_declaring_receiver_not_the_child_view() {
        let (mut selected, mut module) = fixture();
        let surface = std::sync::Arc::make_mut(&mut selected.plan.schema.declaration);
        let child_application = surface.identities.intern("Child<u8>");
        let argument = surface.identities.intern("u8");
        surface.receiver_applications[0].application = child_application;
        surface.receiver_applications[0].argument_identities = vec![argument];
        surface.receiver_applications[1].application = surface.receiver_applications[1].trait_name;
        surface.receiver_applications[1].argument_identities.clear();
        for candidate in &mut module.provider_candidates {
            candidate.declaration_application = TerminalIdentityIndex::ABSENT;
            candidate.receiving_application = TerminalIdentityIndex::ABSENT;
        }
        assert_eq!(
            checked_candidate_roster(std::slice::from_ref(&selected), &module)
                .unwrap()
                .len(),
            2
        );

        let mut identities = module.identities.clone().into_builder();
        let substituted_child = identities.intern("Child<u8>");
        module.identities = identities.build();
        for candidate in &mut module.provider_candidates {
            candidate.declaration_application = substituted_child;
            candidate.receiving_application = substituted_child;
        }
        assert!(checked_candidate_roster(std::slice::from_ref(&selected), &module).is_err());
    }
}

/*
We bind Build-selected target provider plans to canonical Terminal candidate
rosters, closing checked specialization projections, synchronous invocation
cycles, and folded-constant dependencies so publication and receiving
admission enforce the same complete source-to-instance association.
*/

mod synchronous_invocations;

use build_providers::plans::ProviderPlan;
use synchronous_invocations::{
    SynchronousBoundaryTarget, validate_selected_synchronous_invocation_cycles,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalTargetProviderProposal {
    pub target: String,
    pub terminal_psi: terminal_psi::TerminalPsiIdentity,
    pub optimization_selection: optimization_selections::OptimizationSelectionIdentity,
    pub application_roots: Option<Vec<terminal_psi::ProofDeclarationId>>,
    pub application_required: bool,
    pub selection: Result<TerminalTargetProviderSelection, Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalTargetProviderSelection {
    pub plans: Vec<ProviderPlan>,
    pub checked_candidates: Vec<psi_provider_selections::ProviderInstallationSelection>,
}

// A retained provider plan belongs to a deployment already selected by its
// caller. Missing identity is not a request to select the compiler's host.
fn selected_plan_target(plan: &build_providers::plans::ProviderPlan) -> Result<&str, String> {
    if plan.target.is_empty() {
        return Err("selected provider plan has no deployment target".into());
    }
    Ok(&plan.target)
}

pub fn selected_checked_adapter_header<'module>(
    plan: &build_providers::plans::ProviderPlan,
    row: &build_providers::plans::ProviderPlanRow,
    module: &'module terminal_psi::TerminalModule,
) -> Result<&'module terminal_psi::ProofCallableHeader, String> {
    let build_providers::plans::ProviderBinding::CheckedAdapter {
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
        .scalars.scalar_applications
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
    pub plan: &'plan build_providers::plans::ProviderPlan,
    pub row: &'plan build_providers::plans::ProviderPlanRow,
    pub candidates: Vec<SelectedCheckedInvocationCandidate<'module>>,
}

/// The exact selected application and the callable header authenticated
/// against that candidate.
pub struct SelectedCheckedInvocationCandidate<'module> {
    pub conformance: &'module terminal_psi::ProviderCandidateConformance,
    pub header: &'module terminal_psi::ProofCallableHeader,
}

/// Rows follow the input plan and binding order. Only checked adapters
/// contribute rows.
pub fn selected_checked_invocation_rows<'plan, 'module>(
    plans: &'plan [ProviderPlan],
    module: &'module terminal_psi::TerminalModule,
) -> Result<Vec<SelectedCheckedInvocationRow<'plan, 'module>>, String> {
    let scope = &module.scalars.scalar_applications.proof_scope;
    let mut rows = Vec::new();
    for selected in selected_checked_candidates(plans, module)? {
        let plan = selected.selected;
        let build_providers::plans::ProviderBinding::CheckedAdapter {
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
        let mut candidates = Vec::with_capacity(selected.candidates.len());
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
            candidates.push(SelectedCheckedInvocationCandidate {
                conformance: candidate,
                header,
            });
        }
        rows.push(SelectedCheckedInvocationRow {
            plan,
            row: selected.row,
            candidates,
        });
    }
    Ok(rows)
}

impl TerminalTargetProviderSelection {
    pub fn from_plans(
        plans: Vec<ProviderPlan>,
        artifact: terminal_codec::TerminalArtifactView<'_>,
    ) -> Result<Self, String> {
        validate_selected_service_schemas(&plans, artifact.module())?;
        build_providers::selections::SelectedProviderPlanFacts::from_selected_plans(plans.clone())?;
        let checked_candidates = checked_candidate_roster(&plans, artifact.module())?;
        Ok(Self {
            plans,
            checked_candidates,
        })
    }

    pub fn admit(
        &self,
        artifact: terminal_codec::TerminalArtifactView<'_>,
        target: &str,
    ) -> Result<psi_provider_selections::AdmittedProviderSelections, String> {
        validate_selected_service_schemas(&self.plans, artifact.module())?;
        let selected = build_providers::selections::SelectedProviderPlanFacts::from_selected_plans(
            self.plans.clone(),
        )?;
        let expected = checked_candidate_roster(&self.plans, artifact.module())?;
        let mut supplied = self.checked_candidates.iter().collect::<Vec<_>>();
        supplied.sort();
        if !supplied.into_iter().eq(&expected) {
            return Err(
                "checked installations differ from the complete canonical selected-provider roster"
                    .into(),
            );
        }
        admit_checked_roster(artifact, target, &selected, &expected)
    }
}

pub fn admit_selected_checked_providers(
    artifact: terminal_codec::TerminalArtifactView<'_>,
    target: &str,
    selected: &build_providers::selections::SelectedProviderPlanFacts,
) -> Result<psi_provider_selections::AdmittedProviderSelections, String> {
    let mut supplied = Vec::new();
    for matched in selected_checked_candidates(selected.plans(), artifact.module())? {
        supplied.extend(matched.installations());
    }
    supplied.sort();
    supplied.dedup();
    admit_checked_roster(artifact, target, selected, &supplied)
}

fn admit_checked_roster(
    artifact: terminal_codec::TerminalArtifactView<'_>,
    target: &str,
    selected: &build_providers::selections::SelectedProviderPlanFacts,
    supplied: &[psi_provider_selections::ProviderInstallationSelection],
) -> Result<psi_provider_selections::AdmittedProviderSelections, String> {
    for plan in selected.plans() {
        if selected_plan_target(plan)? != target {
            return Err("selected provider plan belongs to another deployment target".into());
        }
    }
    let dependencies =
        matching_folded_constant_dependencies(artifact, target, selected.plans().iter())?;
    let installation = psi_provider_selections::admit_provider_installation(
        artifact,
        supplied,
    )
    .map_err(|error| format!("catalog admission rejected: {error:?}"))?;
    validate_synchronous_calls(artifact, selected)?;
    if artifact.module().scalars.folded_constants.target.is_some() {
        installation.admit_folded_constant_closure(artifact, target, &dependencies)
    } else {
        Ok(installation)
    }
}

fn validate_synchronous_calls(
    artifact: terminal_codec::TerminalArtifactView<'_>,
    selected: &build_providers::selections::SelectedProviderPlanFacts,
) -> Result<(), String> {
    let module = artifact.module();
    let candidates = selected_checked_candidates(selected.plans(), module)?;
    let calls = terminal_semantics::synchronous_calls::SynchronousCallGraph::new(module, None)?;
    // Index each selected slot by its candidates' requirement text once, so a
    // reached boundary looks up its slots instead of rescanning every candidate.
    let mut requirement_slots = candidates
        .iter()
        .flat_map(|matched| {
            matched.candidates.iter().filter_map(|candidate| {
                let requirement = module.identities.text(candidate.requirement_identity)?;
                Some((requirement, matched.selected))
            })
        })
        .collect::<Vec<_>>();
    requirement_slots.sort_by_key(|(requirement, _)| *requirement);
    requirement_slots.dedup_by(|left, right| left.0 == right.0 && std::ptr::eq(left.1, right.1));
    validate_selected_synchronous_invocation_cycles(selected.plans(), |plan, row| {
        let failure = |reason: String| {
            diagnostics::Diagnostic::error(format!(
                "selected provider `{}` synchronous invocation admission failed: {reason}",
                plan.name,
            ))
        };
        // The cycle check hands back references into `selected.plans()`, the
        // same rows the candidates borrow, after rejecting repeated plans.
        let matched = candidates
            .iter()
            .find(|candidate| {
                std::ptr::eq(candidate.selected, plan) && std::ptr::eq(candidate.row, row)
            })
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
                let first = requirement_slots.partition_point(|(text, _)| *text < requirement);
                let mut slots = requirement_slots[first..]
                    .iter()
                    .take_while(|(text, _)| *text == requirement)
                    .map(|(_, plan)| *plan)
                    .collect::<Vec<_>>();
                if let Some((plan, _)) = selected.selected_requirement_row(requirement) {
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

fn slot_target(plan: &build_providers::plans::ProviderPlan) -> SynchronousBoundaryTarget {
    SynchronousBoundaryTarget::Selected {
        plan: plan.identity_digest(),
        label: plan.name.clone(),
    }
}

pub fn matching_folded_constant_dependencies<'plan>(
    artifact: terminal_codec::TerminalArtifactView<'_>,
    target: &str,
    plans: impl IntoIterator<Item = &'plan build_providers::plans::ProviderPlan>,
) -> Result<Vec<Vec<u8>>, String> {
    let module = artifact.module();
    let closure = &module.scalars.folded_constants;
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
                        build_providers::plans::ProviderBinding::CheckedAdapter {
                            machine_identity,
                            machine_package_identity,
                        },
                    ) => module
                        .interfaces
                        .provider_candidates
                        .iter()
                        .any(|candidate| {
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
                    (None, build_providers::plans::ProviderBinding::CompilerIntrinsic { .. }) => {
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
pub fn validate_selected_service_schemas(
    plans: &[ProviderPlan],
    module: &terminal_psi::TerminalModule,
) -> Result<(), String> {
    for selected in plans {
        let plan = selected;
        let mut choices = module.interfaces.provider_selections.iter().filter(|choice| {
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

/// Callers validate `plans` as one selection before asking for its roster.
fn checked_candidate_roster(
    plans: &[ProviderPlan],
    module: &terminal_psi::TerminalModule,
) -> Result<Vec<psi_provider_selections::ProviderInstallationSelection>, String> {
    let mut selections = Vec::new();
    for matched in selected_checked_candidates(plans, module)? {
        if matched.candidates.is_empty() {
            return Err(format!(
                "selected provider requirement `{}` has no emitted exact declaration candidate",
                matched
                    .selected
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
    selected: build_providers::selections::SelectedProviderPlanFacts,
    module: &terminal_psi::TerminalModule,
) -> Result<build_providers::selections::SelectedProviderPlanFacts, String> {
    let selected = selected.without_checked_specializations();
    let mut rows = selected.specialized_requirement_rows().to_vec();
    let mut identities = selected.identities().clone().into_builder();
    for matched in selected_checked_candidates(selected.plans(), module)? {
        let plan = matched.selected;
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
            rows.push(
                build_providers::selections::SelectedRequirementSpecialization {
                    provider_plan_report_identity: plan.report_fingerprint(),
                    selected_requirement_identity: identities.intern(template),
                    selected_machine_identity: identities
                        .intern(text(candidate.source_candidate_identity)?),
                    requirement_identity: identities.intern(requirement),
                    machine_identity: identities.intern(text(candidate.candidate_identity)?),
                },
            );
        }
    }
    selected.with_specialized_requirement_rows(rows, &identities.build())
}

struct SelectedCheckedCandidate<'plan, 'terminal> {
    selected: &'plan ProviderPlan,
    row: &'plan build_providers::plans::ProviderPlanRow,
    owner: &'plan str,
    candidates: Vec<&'terminal terminal_psi::ProviderCandidateConformance>,
}

impl SelectedCheckedCandidate<'_, '_> {
    fn installations(
        &self,
    ) -> impl Iterator<Item = psi_provider_selections::ProviderInstallationSelection> + '_ {
        self.candidates.iter().map(|candidate| {
            psi_provider_selections::ProviderInstallationSelection {
                boundary: candidate.boundary,
                candidate: candidate.candidate,
                provider_identity: self.selected.provider_type.clone(),
                provider_declaration: self.owner.to_owned(),
            }
        })
    }
}

fn selected_checked_candidates<'plan, 'terminal>(
    plans: &'plan [ProviderPlan],
    module: &'terminal terminal_psi::TerminalModule,
) -> Result<Vec<SelectedCheckedCandidate<'plan, 'terminal>>, String> {
    let mut selections = Vec::new();
    for selected in plans {
        let plan = selected;
        if !plan.rows.iter().any(|row| {
            matches!(
                row.binding,
                build_providers::plans::ProviderBinding::CheckedAdapter { .. }
            )
        }) {
            continue;
        }
        let owner = selected
            .provider_declaration
            .as_deref()
            .filter(|owner| !owner.is_empty())
            .ok_or("checked provider plan has no exact selecting declaration")?;
        for projected in selected_provider_candidate_rows(plan, module)? {
            selections.push(SelectedCheckedCandidate {
                selected,
                row: projected.row,
                owner,
                candidates: projected
                    .candidate_positions
                    .into_iter()
                    .map(|position| &module.interfaces.provider_candidates[position])
                    .collect(),
            });
        }
    }
    Ok(selections)
}

pub struct SelectedProviderCandidateRow<'plan> {
    pub row: &'plan build_providers::plans::ProviderPlanRow,
    pub candidate_positions: Vec<usize>,
}

pub fn selected_provider_candidate_rows<'plan>(
    plan: &'plan build_providers::plans::ProviderPlan,
    module: &terminal_psi::TerminalModule,
) -> Result<Vec<SelectedProviderCandidateRow<'plan>>, String> {
    use terminal_semantics::provider_candidates::{
        CheckedProviderQuery, CheckedProviderRow, selected_candidate_positions,
    };
    let owner = plan
        .provider_declaration
        .as_deref()
        .filter(|owner| !owner.is_empty())
        .ok_or("checked provider plan has no exact selecting declaration")?;
    let rows = plan
        .rows
        .iter()
        .filter_map(|row| {
            let build_providers::plans::ProviderBinding::CheckedAdapter {
                machine_identity,
                machine_package_identity,
            } = row.binding
            else {
                return None;
            };
            Some((
                row,
                CheckedProviderRow {
                    requirement: plan.identity_text(row.requirement_identity),
                    declared_requirement: plan.identity_text(row.declared_requirement_identity),
                    machine: plan.identity_text(machine_identity),
                    machine_package: machine_package_identity,
                },
            ))
        })
        .collect::<Vec<_>>();
    let (rows, query_rows): (Vec<_>, Vec<_>) = rows.into_iter().unzip();
    let positions = selected_candidate_positions(
        CheckedProviderQuery {
            name: &plan.name,
            declaration: owner,
            provider: &plan.provider_type,
            surface: &plan.schema.declaration,
            rows: &query_rows,
        },
        module,
    )?;
    Ok(rows
        .into_iter()
        .zip(positions)
        .map(|(row, candidate_positions)| SelectedProviderCandidateRow {
            row,
            candidate_positions,
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use build_providers::plans::{
        ProviderBinding, ProviderIdentityTable, ProviderPlan, ProviderPlanRow, ServiceMethod,
        ServiceSchema,
    };
    use terminal_psi::{
        NominalDeclarationIdentity, ProviderCandidateConformance, TerminalIdentityIndex,
        TerminalIdentityTableBuilder, TerminalModule,
    };

    fn fixture() -> (ProviderPlan, TerminalModule) {
        let mut identities = ProviderIdentityTable::default();
        let requirement = identities.intern("Parent::read");
        let machine = identities.intern("Shared::read");
        let mut schema_identities = ProviderIdentityTable::default();
        let trait_name = schema_identities.intern("Child");
        let schema_requirement = schema_identities.intern("Parent::read");
        let parent_name = schema_identities.intern("Parent");
        let parent_application = schema_identities.intern("Parent<u64>");
        let argument = schema_identities.intern("u64");
        let selected = ProviderPlan {
            provider_declaration: Some("FirstMap".into()),
            row_numeric_realizations: vec![None],
            name: "Child selected".into(),
            provider_type: "First".into(),
            target: "macos_arm64".into(),
            identities,
            schema: ServiceSchema::from_declaration(std::sync::Arc::new(
                build_providers::plans::ServiceSurface {
                    trait_name,
                    receiver_applications: vec![
                        build_providers::plans::ServiceReceiverApplication {
                            application: trait_name,
                            trait_name,
                            trait_package_identity: None,
                            argument_identities: vec![],
                            lifetime_argument_identities: vec![],
                            requirement_identities: vec![schema_requirement],
                        },
                        build_providers::plans::ServiceReceiverApplication {
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
                    ..build_providers::plans::ServiceSurface::default()
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
                .interfaces.provider_candidates
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

    fn invocation_fixture() -> (ProviderPlan, TerminalModule) {
        let (selected, mut module) = fixture();
        let candidates = module.interfaces.provider_candidates[..2].to_vec();
        module
            .interfaces.provider_instance_requirements
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
        let scope = &mut module.scalars.scalar_applications.proof_scope;
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
        let row = &selected.rows[0];
        assert!(selected_checked_adapter_header(&selected, row, &module).is_ok());
        assert!(selected_checked_invocation_rows(std::slice::from_ref(&selected), &module).is_ok());

        selected.target.clear();
        let row = &selected.rows[0];
        assert_eq!(
            selected_checked_adapter_header(&selected, row, &module).unwrap_err(),
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
        module.interfaces.provider_candidates.sort_by_key(|candidate| {
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
        declaration.target.clear();
        let selected =
            build_providers::selections::SelectedProviderPlanFacts::from_selected_plans(vec![
                declaration.clone(),
            ])
            .unwrap();
        assert_eq!(
            admit_checked_roster(artifact.view(), "macos_arm64", &selected, &[],).unwrap_err(),
            "selected provider plan has no deployment target"
        );
    }

    #[test]
    fn invocation_rows_consume_all_closed_headers_instead_of_the_open_template() {
        let (selected, module) = invocation_fixture();
        let plans = [selected];
        let rows = selected_checked_invocation_rows(&plans, &module).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].candidates.len(), 2);
        let closed = &rows[0].candidates[1];
        assert_eq!(
            closed.header.declaration,
            terminal_psi::ProofDeclarationId(2)
        );
        assert!(std::ptr::eq(
            closed.conformance,
            &module.interfaces.provider_candidates[1]
        ));
        assert_eq!(
            closed.header.callable,
            Ok(closed.conformance.candidate_identity)
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
                .scalars.scalar_applications
                .proof_scope
                .callable_headers
                .iter()
                .next()
                .unwrap()
                .0;
            match mismatch {
                "origin" => {
                    selected.origin_package_identity =
                        semantic_vocabulary::PackageKeyIdentity::from_digest([9; 32])
                }
                "package" => {
                    changed
                        .scalars.scalar_applications
                        .proof_scope
                        .callable_headers
                        .get_mut(header)
                        .package = semantic_vocabulary::PackageKeyIdentity::from_digest([9; 32])
                }
                "target" => {
                    changed
                        .scalars.scalar_applications
                        .proof_scope
                        .callable_headers
                        .get_mut(header)
                        .source_target = Some("windows_x86_64".into())
                }
                "callable" => {
                    changed
                        .scalars.scalar_applications
                        .proof_scope
                        .callable_headers
                        .get_mut(header)
                        .callable = Ok(TerminalIdentityIndex::ABSENT)
                }
                "product" => {
                    changed
                        .scalars.scalar_applications
                        .proof_scope
                        .callable_headers
                        .get_mut(header)
                        .product_eligible = false
                }
                "body" => {
                    changed
                        .scalars.scalar_applications
                        .proof_scope
                        .callable_headers
                        .get_mut(header)
                        .supplies_checked_body = false
                }
                "header" => {
                    changed.scalars.scalar_applications.proof_scope.callable_headers = Default::default()
                }
                "association" => {
                    changed.scalars.scalar_applications.proof_scope.callables = Default::default()
                }
                "instance" => {
                    let mut identities = changed.identities.clone().into_builder();
                    let other_application = identities.intern("Parent<bool>");
                    for candidate in &mut changed.interfaces.provider_candidates {
                        candidate.receiving_application = other_application;
                    }
                    changed.identities = identities.build();
                }
                _ => {
                    let duplicate = changed
                        .scalars.scalar_applications
                        .proof_scope
                        .callable_headers
                        .get(header)
                        .clone();
                    changed
                        .scalars.scalar_applications
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
    fn saved_checked_provider_specializations_reproject_without_renumbering() {
        let (declaration, module) = fixture();
        let selected =
            build_providers::selections::SelectedProviderPlanFacts::from_selected_plans(vec![
                declaration,
            ])
            .unwrap();
        let original = close_checked_provider_specializations(selected, &module).unwrap();
        let recovered = original
            .clone()
            .without_checked_specializations()
            .recovery_candidate()
            .validate()
            .unwrap();
        let projected = close_checked_provider_specializations(recovered, &module).unwrap();
        assert_eq!(
            projected.identities().entries(),
            original.identities().entries()
        );
        assert_eq!(projected, original);
    }

    #[test]
    fn canonical_instances_replace_source_rows_and_keep_uncalled_inherited_coordinates() {
        let (declaration, module) = fixture();
        let selected =
            build_providers::selections::SelectedProviderPlanFacts::from_selected_plans(vec![
                declaration.clone(),
            ])
            .unwrap();
        let mut identities = module.identities.clone().into_builder();
        let stale = build_providers::selections::SelectedRequirementSpecialization {
            provider_plan_report_identity: declaration.report_fingerprint(),
            selected_requirement_identity: identities.intern("Parent::read"),
            selected_machine_identity: identities.intern("Shared::read"),
            requirement_identity: identities.intern("Parent::read<999>"),
            machine_identity: identities.intern("StaleSourceBody"),
        };
        let selected = selected
            .with_specialized_requirement_rows(vec![stale], &identities.build())
            .unwrap();
        let closed = close_checked_provider_specializations(selected, &module).unwrap();
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
        let mut missing = declaration.schema.declaration.methods[0].clone();
        missing.name = "unavailable".into();
        let surface = std::sync::Arc::make_mut(&mut declaration.schema.declaration);
        missing.requirement_identity = surface.identities.intern("Parent::unavailable");
        for application in &mut surface.receiver_applications {
            application
                .requirement_identities
                .push(missing.requirement_identity);
        }
        surface.methods.push(missing);
        assert!(
            build_providers::selections::SelectedProviderPlanFacts::from_selected_plans(vec![
                declaration.clone()
            ])
            .is_err()
        );
        let selected = build_providers::selections::SelectedProviderPlanFacts::default()
            .with_toolchain_settled_plan(declaration.clone())
            .unwrap();
        let closed = close_checked_provider_specializations(selected, &module).unwrap();
        assert!(closed.is_toolchain_settled(&declaration));
        assert_eq!(closed.specialized_requirement_rows().len(), 2);
    }

    #[test]
    fn canonical_projection_rejects_duplicate_coordinates_on_every_target() {
        let (mut declaration, mut module) = fixture();
        let mut alternate = module.interfaces.provider_candidates[0].clone();
        alternate.candidate = semantic_vocabulary::MachineId::new(5).unwrap();
        module.interfaces.provider_candidates.push(alternate);
        for target in ["macos_arm64", "windows_x86_64", "linux_arm64"] {
            declaration.target = target.into();
            assert!(
                checked_candidate_roster(std::slice::from_ref(&declaration), &module)
                    .unwrap_err()
                    .contains("duplicate canonical realizations")
            );
        }
    }

    #[test]
    fn selected_closure_retains_selecting_declaration_custody() {
        let (first, module) = fixture();
        let mut second = first.clone();
        second.provider_declaration = Some("SecondMap".to_owned());
        // The implementing code and its ABI meaning are unchanged, but the
        // two declarations select different canonical candidate rosters.
        assert_eq!(first.identity_digest(), second.identity_digest());
        assert_ne!(
            checked_candidate_roster(std::slice::from_ref(&first), &module).unwrap(),
            checked_candidate_roster(std::slice::from_ref(&second), &module).unwrap(),
        );
        let first =
            build_providers::selections::SelectedProviderPlanFacts::from_selected_plans(vec![
                first,
            ])
            .unwrap();
        let second =
            build_providers::selections::SelectedProviderPlanFacts::from_selected_plans(vec![
                second,
            ])
            .unwrap();
        assert_ne!(
            first.identity_digest(),
            second.identity_digest(),
            "a selected closure must retain the declaration whose checked candidates it uses",
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
                "owner" => changed.interfaces.provider_candidates.retain(|row| {
                    module.identities.text(row.provider_declaration.identity()) != Some("FirstMap")
                }),
                "requirement" => changed.interfaces.provider_candidates.iter_mut().for_each(|row| {
                    row.source_requirement_identity = TerminalIdentityIndex::ABSENT
                }),
                "body" => changed
                    .interfaces.provider_candidates
                    .iter_mut()
                    .for_each(|row| row.source_candidate_identity = TerminalIdentityIndex::ABSENT),
                "receiver" => {
                    let mut identities = changed.identities.clone().into_builder();
                    let substituted = identities.intern("Parent<bool>");
                    for row in &mut changed.interfaces.provider_candidates {
                        row.declaration_application = substituted;
                        row.receiving_application = substituted;
                    }
                    changed.identities = identities.build();
                }
                "package" => {
                    for row in &mut changed.interfaces.provider_candidates {
                        row.source_candidate_package_identity =
                            semantic_vocabulary::PackageKeyIdentity::from_digest([9; 32]);
                    }
                }
                _ => changed
                    .interfaces.provider_candidates
                    .push(changed.interfaces.provider_candidates[0].clone()),
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
        let (mut selected, module) = fixture();
        let projected = selected_provider_candidate_rows(&selected, &module).unwrap();
        assert_eq!(projected.len(), 1);
        assert_eq!(projected[0].candidate_positions, vec![0, 1]);
        let installations =
            checked_candidate_roster(std::slice::from_ref(&selected), &module).unwrap();
        assert_eq!(installations.len(), projected[0].candidate_positions.len());
        for (installation, position) in installations.iter().zip(&projected[0].candidate_positions)
        {
            assert_eq!(
                installation.boundary,
                module.interfaces.provider_candidates[*position].boundary
            );
            assert_eq!(
                installation.candidate,
                module.interfaces.provider_candidates[*position].candidate
            );
            assert_eq!(installation.provider_declaration, "FirstMap");
        }
        selected.provider_declaration = None;
        assert!(selected_provider_candidate_rows(&selected, &module).is_err());
    }

    #[test]
    fn inherited_requirement_uses_its_declaring_receiver_not_the_child_view() {
        let (mut selected, mut module) = fixture();
        let surface = std::sync::Arc::make_mut(&mut selected.schema.declaration);
        let child_application = surface.identities.intern("Child<u8>");
        let argument = surface.identities.intern("u8");
        surface.receiver_applications[0].application = child_application;
        surface.receiver_applications[0].argument_identities = vec![argument];
        surface.receiver_applications[1].application = surface.receiver_applications[1].trait_name;
        surface.receiver_applications[1].argument_identities.clear();
        for candidate in &mut module.interfaces.provider_candidates {
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
        for candidate in &mut module.interfaces.provider_candidates {
            candidate.declaration_application = substituted_child;
            candidate.receiving_application = substituted_child;
        }
        assert!(checked_candidate_roster(std::slice::from_ref(&selected), &module).is_err());
    }
}

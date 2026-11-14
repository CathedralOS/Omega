//! Derive provider plans from the canonical contracts and exact choices in Psi.
//!
//! The source producer has already normalized the selected static value and
//! checked its conformance. This operation neither reopens source nor treats
//! retaining a candidate or a descriptor as an invocation.

use std::sync::Arc;

use backend_plan::TerminalTargetProviderPlan;
use effects::provider_plan::{
    ProviderBinding, ProviderIdentityTable, ProviderPlan, ProviderPlanRow, ServiceSchema,
};
use semantic_vocabulary::{BoundaryMachineId, PackageKeyIdentity};
use terminal_psi::{TerminalModule, TerminalProviderSelection, TerminalServiceSchema};

use super::realized_supply::{
    ProviderPlanRealizationInput, RealizedTargetSupply, TargetSupplyRealizationInput,
};

/// Actual normalized products of the Build, calling, and permission owners.
/// No default constructor turns an unfinished projection into empty supply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetSupplyExecutionInputs {
    pub calling_applications: Vec<calling_conventions::BoundaryCallingApplication>,
    pub external_bindings: Vec<calling_conventions::ExternalBindingRow>,
    pub evaluated_native_bindings: Vec<effects::provider_plan::EvaluatedNativeBinding>,
    pub evaluated_binding_identities: terminal_psi::TerminalIdentityTable,
    pub terminal_authority_permissions: Vec<effects::ServiceTerminalAuthorityPermission>,
    pub terminal_authority_permission_identities: terminal_psi::TerminalIdentityTable,
}

pub struct TargetSupplyRealizationRequest<'a> {
    pub module: &'a TerminalModule,
    pub target: &'a str,
    pub reached: &'a [BoundaryMachineId],
    pub calling_applications: Vec<calling_conventions::BoundaryCallingApplication>,
    pub external_bindings: Vec<calling_conventions::ExternalBindingRow>,
    pub evaluated_native_bindings: Vec<effects::provider_plan::EvaluatedNativeBinding>,
    pub evaluated_binding_identities: terminal_psi::TerminalIdentityTable,
    pub terminal_authority_permissions: Vec<effects::ServiceTerminalAuthorityPermission>,
    pub terminal_authority_permission_identities: terminal_psi::TerminalIdentityTable,
}

impl<'a> TargetSupplyRealizationRequest<'a> {
    pub fn from_execution_inputs(
        module: &'a TerminalModule,
        target: &'a str,
        reached: &'a [BoundaryMachineId],
        inputs: TargetSupplyExecutionInputs,
    ) -> Self {
        Self {
            module,
            target,
            reached,
            calling_applications: inputs.calling_applications,
            external_bindings: inputs.external_bindings,
            evaluated_native_bindings: inputs.evaluated_native_bindings,
            evaluated_binding_identities: inputs.evaluated_binding_identities,
            terminal_authority_permissions: inputs.terminal_authority_permissions,
            terminal_authority_permission_identities: inputs
                .terminal_authority_permission_identities,
        }
    }
}

fn text(
    module: &TerminalModule,
    index: terminal_psi::TerminalIdentityIndex,
) -> Result<&str, String> {
    module
        .identities
        .text(index)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "provider realization contains an unresolved canonical declaration".into())
}

struct Candidate<'a> {
    provider: terminal_psi::TerminalIdentityIndex,
    owner: terminal_psi::TerminalIdentityIndex,
    requirement: &'a str,
    source_callable: &'a str,
    row: &'a terminal_psi::ProviderCandidateConformance,
}

fn candidates(module: &TerminalModule) -> Result<Vec<Candidate<'_>>, String> {
    let mut rows = Vec::with_capacity(module.provider_candidates.len());
    for row in &module.provider_candidates {
        let body = module
            .machine(row.candidate)
            .ok_or("canonical provider candidate has no executable body")?;
        let declared = body
            .declaration
            .as_ref()
            .ok_or("canonical provider candidate has no checked declaration")?;
        if declared.callable != row.candidate_identity {
            return Err("canonical provider candidate changes its checked body identity".into());
        }
        module
            .boundary_machine(row.boundary)
            .ok_or("canonical provider candidate lost its required boundary")?;
        text(module, row.provider_identity)?;
        text(module, row.provider_declaration.identity())?;
        rows.push(Candidate {
            provider: row.provider_identity,
            owner: row.provider_declaration.identity(),
            requirement: text(module, row.source_requirement_identity)?,
            source_callable: text(module, row.source_candidate_identity)?,
            row,
        });
    }
    rows.sort_by_key(|row| {
        (
            row.provider,
            row.owner,
            row.requirement,
            row.source_callable,
        )
    });
    Ok(rows)
}

fn chosen_candidates<'a, 'rows>(
    choice: &TerminalProviderSelection,
    candidates: &'rows [Candidate<'a>],
) -> &'rows [Candidate<'a>] {
    let key = (choice.provider_identity, choice.provider_declaration);
    let begin = candidates.partition_point(|row| (row.provider, row.owner) < key);
    let end = begin + candidates[begin..].partition_point(|row| (row.provider, row.owner) == key);
    &candidates[begin..end]
}

fn schema_is_demanded(
    module: &TerminalModule,
    schema: &TerminalServiceSchema,
    candidates: &[Candidate<'_>],
    reached: &[BoundaryMachineId],
) -> Result<bool, String> {
    for &boundary in reached {
        let declaration = module
            .boundary_machine(boundary)
            .ok_or("provider demand names an absent boundary")?;
        let requirement = text(module, declaration.identity)?;
        if schema
            .surface
            .methods
            .iter()
            .any(|method| schema.surface.identity_text(method.requirement_identity) == requirement)
            || candidates.iter().any(|candidate| {
                candidate.row.boundary == boundary
                    && schema.surface.methods.iter().any(|method| {
                        schema.surface.identity_text(method.requirement_identity)
                            == candidate.requirement
                    })
            })
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn attach_calling(
    schema: &mut ServiceSchema,
    target: &str,
    applications: &[calling_conventions::BoundaryCallingApplication],
) -> Result<(), String> {
    use effects::provider_plan::{
        BoundaryCallingPlanCommitment, BoundaryCallingPlanPromise, ServiceCallingApplication,
        ServiceCallingPlan,
    };
    let profile = target::TargetProfile::from_canonical_target_name(target)
        .map_err(|error| error.to_string())?;
    let native_target = profile.native_realization();
    if native_target.is_none() && !applications.is_empty() {
        return Err("calling applications require a profile with a native realization".into());
    }
    for application in applications
        .iter()
        .filter(|row| Some(row.target) == native_target)
    {
        let Some(requirement) = schema
            .declaration
            .identities
            .id_at(&application.requirement_identity)
        else {
            continue;
        };
        if !schema
            .declaration
            .methods
            .iter()
            .any(|method| method.requirement_identity == requirement)
        {
            continue;
        }
        if application.recomputed_commitment()? != application.commitment {
            return Err("provider calling application changes its complete commitment".into());
        }
        let row = ServiceCallingApplication {
            requirement,
            abi: application.abi.clone(),
            plan: ServiceCallingPlan {
                report_fingerprint: application.report_fingerprint,
                commitment: BoundaryCallingPlanCommitment::from_digest(application.commitment),
                promise: BoundaryCallingPlanPromise::from_digest(
                    application.plan.contract_commitment_digest(),
                ),
            },
        };
        if schema
            .calling_applications
            .iter()
            .any(|existing| existing.abi == row.abi && existing != &row)
        {
            return Err("provider calling declaration has competing selected applications".into());
        }
        if !schema.calling_applications.contains(&row) {
            schema.calling_applications.push(row);
        }
    }
    schema
        .validate_calling_applications()
        .map_err(str::to_owned)
}

fn checked_plan(
    module: &TerminalModule,
    choice: &TerminalProviderSelection,
    schema: &TerminalServiceSchema,
    candidates: &[Candidate<'_>],
    calling: &[calling_conventions::BoundaryCallingApplication],
    lifetime_families: &[&str],
) -> Result<ProviderPlanRealizationInput, String> {
    let provider = text(module, choice.provider_identity)?;
    let owner = text(module, choice.provider_declaration)?;
    let mut schema_value = ServiceSchema::from_declaration(Arc::clone(&schema.surface));
    attach_calling(&mut schema_value, &choice.target, calling)?;
    let mut identities = ProviderIdentityTable::default();
    let mut rows = Vec::with_capacity(schema.surface.methods.len());
    let mut package = None::<Option<PackageKeyIdentity>>;
    for method in &schema.surface.methods {
        let requirement = schema.surface.identity_text(method.requirement_identity);
        let begin = candidates.partition_point(|row| row.requirement < requirement);
        let end = begin + candidates[begin..].partition_point(|row| row.requirement == requirement);
        let selected = &candidates[begin..end];
        let first = selected.first().ok_or_else(|| {
            format!("chosen provider `{provider}` has no proved row for `{requirement}`")
        })?;
        if selected.iter().any(|row| {
            row.source_callable != first.source_callable
                || row.row.source_candidate_package_identity
                    != first.row.source_candidate_package_identity
        }) {
            return Err(format!(
                "chosen provider `{provider}` has competing exact realizations for `{requirement}`"
            ));
        }
        if lifetime_families.binary_search(&requirement).is_ok() {
            return Err(
                "selected provider lifetime family needs its exact retained partition coordinate"
                    .into(),
            );
        }
        let row_package = first.row.source_candidate_package_identity;
        if package.is_some_and(|previous| previous != row_package) {
            return Err("chosen map needs an unrepresented cross-package adapter closure".into());
        }
        package = Some(row_package);
        rows.push(ProviderPlanRow {
            method: method.name.clone(),
            requirement_identity: identities.intern(requirement),
            declared_requirement_identity: identities.intern(requirement),
            calling_signature: None,
            requirement_lifetime_partition: Vec::new(),
            admitted_same_stack_bytes: 0,
            binding: ProviderBinding::CheckedAdapter {
                machine_identity: identities.intern(first.source_callable),
                machine_package_identity: row_package,
            },
        });
    }
    let provider_application = choice
        .provider_application
        .map(|identity| text(module, identity))
        .transpose()?
        .map(|identity| identities.intern(identity))
        .unwrap_or_default();
    let count = rows.len();
    let plan = ProviderPlan {
        name: crate::satisfies_plan_name(
            &choice.target,
            schema.surface.identity_text(schema.surface.trait_name),
            provider,
        ),
        provider_type: provider.to_owned(),
        provider_application,
        provider_type_package_identity: choice.provider_package,
        target: choice.target.clone(),
        schema: schema_value,
        identities,
        rows,
        origin_package_identity: package.flatten(),
        origin_package: owner.to_owned(),
        digest_memo: Default::default(),
    };
    let canonical = backend_plan::selected_provider_candidate_rows(&plan, owner, module)?;
    if canonical.len() != count
        || canonical
            .iter()
            .any(|row| row.candidate_positions.is_empty())
    {
        return Err("chosen plan does not rejoin every canonical checked candidate".into());
    }
    Ok(ProviderPlanRealizationInput {
        composition_mode: choice.composition_mode,
        schema_declaration: choice.schema_declaration,
        schema_application: choice.schema_application,
        declaration: TerminalTargetProviderPlan {
            plan,
            provider_declaration: Some(owner.to_owned()),
            row_numeric_realizations: vec![None; count],
        },
        row_executions: vec![None; count],
    })
}

fn direct_leaves(
    module: &TerminalModule,
    target: &str,
    calling: &[calling_conventions::BoundaryCallingApplication],
    external: &[calling_conventions::ExternalBindingRow],
    reached: &[BoundaryMachineId],
) -> Result<Vec<backend_plan::DirectBoundaryLeaf>, String> {
    let mut leaves = Vec::new();
    let mut declarations = module
        .boundary_machines
        .iter()
        .filter_map(|boundary| {
            boundary.declaration.as_ref().map(|source| {
                (
                    (source.source.machine.identity(), source.source.callable),
                    boundary,
                )
            })
        })
        .collect::<Vec<_>>();
    declarations.sort_by_key(|(key, _)| *key);
    let mut requirements = module
        .boundary_machines
        .iter()
        .filter_map(|boundary| {
            boundary.declaration.as_ref().map(|source| {
                (
                    (boundary.identity, source.source.machine.identity()),
                    boundary,
                )
            })
        })
        .collect::<Vec<_>>();
    requirements.sort_by_key(|(key, _)| *key);
    let mut schemas = module.service_schemas.iter().collect::<Vec<_>>();
    schemas.sort_by_key(|schema| schema.declaration);
    let mut foreign_bindings = external
        .iter()
        .filter(|row| row.target_name == target)
        .collect::<Vec<_>>();
    foreign_bindings.sort_by_key(|row| row.requirement_identity.as_str());
    for row in &module.external_providers {
        if row
            .target
            .as_deref()
            .is_some_and(|declared| declared != target)
        {
            continue;
        }
        // A source-free external header is retained independently of a direct
        // call. A leaf exists only when Psi also published its exact boundary.
        let key = (row.realization.machine.identity(), row.realization.callable);
        let begin = declarations.partition_point(|(candidate, _)| *candidate < key);
        let end = begin + declarations[begin..].partition_point(|(candidate, _)| *candidate == key);
        let mut declared = declarations[begin..end]
            .iter()
            .map(|(_, boundary)| *boundary)
            .filter(|boundary| {
                boundary
                    .declaration
                    .as_ref()
                    .is_some_and(|declaration| declaration.source == row.realization)
            });
        let Some(declaration) = declared.next() else {
            continue;
        };
        if declared.next().is_some() {
            return Err("native external header has competing direct callable declarations".into());
        }
        row.validate(&module.identities)?;
        let requirement = text(module, row.requirement_identity)?;
        let key = (row.requirement_identity, row.requirement.identity());
        let begin = requirements.partition_point(|(candidate, _)| *candidate < key);
        let end = begin + requirements[begin..].partition_point(|(candidate, _)| *candidate == key);
        let [(_, required)] = &requirements[begin..end] else {
            return Err("native conformance has no unique exact required boundary".into());
        };
        let owner = required.owner.identity();
        let begin = schemas.partition_point(|schema| schema.declaration < owner);
        let end = begin + schemas[begin..].partition_point(|schema| schema.declaration == owner);
        let mut matching_schemas = schemas[begin..end].iter().copied().filter(|schema| {
            schema.surface.trait_package_identity == row.requirement_package
                && schema.surface.methods.iter().any(|method| {
                    schema.surface.identity_text(method.requirement_identity) == requirement
                })
        });
        let schema = matching_schemas
            .next()
            .ok_or("native conformance has no complete canonical service schema")?;
        if matching_schemas.next().is_some() {
            return Err("native conformance has competing complete service applications".into());
        }
        module.service_schema(schema.declaration, schema.application)?;
        let method = schema
            .surface
            .methods
            .iter()
            .find(|method| schema.surface.identity_text(method.requirement_identity) == requirement)
            .ok_or("canonical native schema lost its required method")?;
        let callable = text(module, row.realization.callable)?;
        if text(module, declaration.identity)? != callable {
            return Err("native direct boundary changes the selected source callable".into());
        }
        let mut identities = ProviderIdentityTable::default();
        let requirement_id = identities.intern(requirement);
        let provider = row
            .provider
            .as_ref()
            .map(|provider| text(module, provider.identity))
            .transpose()?;
        let mut schema_value = ServiceSchema::from_declaration(Arc::clone(&schema.surface));
        attach_calling(&mut schema_value, target, calling)?;
        let begin = foreign_bindings
            .partition_point(|binding| binding.requirement_identity.as_str() < callable);
        let end = begin
            + foreign_bindings[begin..]
                .partition_point(|binding| binding.requirement_identity == callable);
        let foreign = &foreign_bindings[begin..end];
        let (binding, mechanism, execution) = match &row.binding {
            language_semantics::ExternalBindingIdentity::CompilerIntrinsic => (
                ProviderBinding::CompilerIntrinsic {
                    machine: callable.to_owned(),
                },
                crate::DirectBoundaryMechanism::Intrinsic,
                backend_plan::DirectBoundaryExecution::Intrinsic(
                    effects::external_providers::canonical_external_execution(row, target)?,
                ),
            ),
            language_semantics::ExternalBindingIdentity::Syscall { number } => {
                let execution = match foreign {
                    [binding]
                        if binding.binding
                            == (calling_conventions::ExternalBindingKind::Syscall {
                                number: *number,
                            }) =>
                    {
                        Ok((**binding).clone())
                    }
                    [] => Err(vec![diagnostics::Diagnostic::error(
                        "selected native syscall needs its exact evaluated Build receipt and calling application",
                    )]),
                    _ => {
                        return Err(
                            "native syscall has competing or incompatible physical binding rows"
                                .into(),
                        );
                    }
                };
                (
                    ProviderBinding::Syscall { number: *number },
                    crate::DirectBoundaryMechanism::Foreign(execution.clone()),
                    backend_plan::DirectBoundaryExecution::Foreign(execution),
                )
            }
            _ if reached.contains(&declaration.id) => {
                return Err("reached native table leaf needs its exact evaluated storage/calling realization".into());
            }
            _ => continue,
        };
        let original = ProviderPlan {
            name: format!("direct-source:{target}:{callable}"),
            provider_type: provider.unwrap_or("").to_owned(),
            provider_application: Default::default(),
            provider_type_package_identity: row.provider.as_ref().and_then(|owner| owner.package),
            target: target.to_owned(),
            schema: schema_value,
            identities,
            rows: vec![ProviderPlanRow {
                method: method.name.clone(),
                requirement_identity: requirement_id,
                declared_requirement_identity: requirement_id,
                calling_signature: None,
                requirement_lifetime_partition: Vec::new(),
                admitted_same_stack_bytes: 0,
                binding,
            }],
            origin_package_identity: row.realization_package,
            origin_package: text(module, row.realization.semantic_declaration)?.to_owned(),
            digest_memo: Default::default(),
        };
        let input = crate::DirectBoundaryInput::from_checked(0, row, &module.identities, mechanism)
            .map_err(|error| error.message)?;
        leaves.push(
            input
                .finish(&original, execution)
                .map_err(|error| error.message)?,
        );
    }
    Ok(leaves)
}

pub fn realize_target_supply(
    request: TargetSupplyRealizationRequest<'_>,
) -> Result<RealizedTargetSupply, String> {
    let module = request.module;
    target::TargetProfile::from_canonical_target_name(request.target)
        .map_err(|error| error.to_string())?;
    let candidates = candidates(module)?;
    let mut lifetime_families = module
        .closed_conformance_applications
        .iter()
        .flat_map(|application| &application.schemas)
        .filter(|schema| schema.lifetime_parameter_count != 0)
        .map(|schema| text(module, schema.public_requirement_identity))
        .collect::<Result<Vec<_>, _>>()?;
    lifetime_families.sort_unstable();
    lifetime_families.dedup();
    let mut plans = Vec::new();
    let mut covered = vec![false; request.reached.len()];
    for choice in module
        .provider_selections
        .iter()
        .filter(|choice| choice.target == request.target)
    {
        let schema = module.service_schema(choice.schema_declaration, choice.schema_application)?;
        let chosen = chosen_candidates(choice, &candidates);
        if !schema_is_demanded(module, schema, chosen, request.reached)? {
            continue;
        }
        let plan = checked_plan(
            module,
            choice,
            schema,
            chosen,
            &request.calling_applications,
            &lifetime_families,
        )?;
        for (ordinal, boundary) in request.reached.iter().enumerate() {
            let row = module
                .boundary_machine(*boundary)
                .ok_or("provider demand names an absent boundary")?;
            let actual_identity = text(module, row.identity)?;
            covered[ordinal] |= chosen
                .iter()
                .any(|candidate| candidate.row.boundary == *boundary)
                || schema.surface.methods.iter().any(|method| {
                    schema.surface.identity_text(method.requirement_identity) == actual_identity
                });
        }
        plans.push(plan);
    }
    let direct_boundary_leaves = direct_leaves(
        module,
        request.target,
        &request.calling_applications,
        &request.external_bindings,
        request.reached,
    )?;
    for (ordinal, boundary) in request.reached.iter().enumerate() {
        if covered[ordinal] {
            continue;
        }
        let declaration = module
            .boundary_machine(*boundary)
            .and_then(|boundary| boundary.declaration.as_ref())
            .ok_or("reached boundary has no genuine checked declaration")?;
        if !direct_boundary_leaves.iter().any(|leaf| {
            module
                .identities
                .text(declaration.source.machine.identity())
                == Some(leaf.declaration_identity.as_str())
                && module.identities.text(declaration.source.callable)
                    == Some(leaf.callable_identity.as_str())
        }) {
            return Err(
                "reached requirement has no normalized chosen provider or exact native leaf".into(),
            );
        }
    }
    RealizedTargetSupply::from_checked(
        module,
        TargetSupplyRealizationInput {
            reached: request.reached.to_vec(),
            target: request.target.to_owned(),
            plans,
            direct_boundary_leaves,
            calling_applications: request.calling_applications,
            external_bindings: request.external_bindings,
            evaluated_native_bindings: request.evaluated_native_bindings,
            evaluated_binding_identities: request.evaluated_binding_identities,
            terminal_authority_permissions: request.terminal_authority_permissions,
            terminal_authority_permission_identities: request
                .terminal_authority_permission_identities,
        },
    )
}

pub(super) fn family_realizations(
    module: &TerminalModule,
    target: &str,
    reached: &[BoundaryMachineId],
    plans: &effects::SelectedProviderPlanFacts,
) -> Result<Vec<backend_plan::SelectedFamilyRealizationSource>, String> {
    use terminal_psi::TargetMachineCallable;
    let mut associations = module
        .provider_candidates
        .iter()
        .map(|candidate| {
            Ok((
                text(module, candidate.requirement_identity)?,
                text(module, candidate.source_requirement_identity)?,
                text(module, candidate.provider_identity)?,
                text(module, candidate.source_candidate_identity)?,
            ))
        })
        .collect::<Result<Vec<_>, String>>()?;
    associations.sort_unstable();
    associations.dedup();
    let mut planned = plans
        .plans()
        .iter()
        .flat_map(|plan| {
            plan.rows
                .iter()
                .map(move |row| (plan.identities.text(row.requirement_identity), plan))
        })
        .collect::<Vec<_>>();
    planned.sort_by_key(|(required, _)| *required);
    let mut adapters = plans
        .plans()
        .iter()
        .flat_map(|plan| {
            plan.rows.iter().filter_map(move |row| match &row.binding {
                ProviderBinding::CheckedAdapter {
                    machine_identity, ..
                } => Some((
                    (
                        plan.identities.text(row.requirement_identity),
                        plan.provider_type.as_str(),
                        plan.identities.text(*machine_identity),
                    ),
                    plan,
                )),
                _ => None,
            })
        })
        .collect::<Vec<_>>();
    adapters.sort_by_key(|(key, _)| *key);
    let mut result = Vec::new();
    for family in &module.target_machine_families {
        let TargetMachineCallable::Boundary(head_id) = family.canonical else {
            continue;
        };
        let mut selected = family
            .members
            .iter()
            .filter(|member| member.target == target);
        let member = selected.next();
        if selected.next().is_some() {
            return Err("target family has competing exact target members".into());
        }
        let demanded = reached.contains(&head_id)
            || member.is_some_and(|member| matches!(member.callable, TargetMachineCallable::Boundary(id) if reached.contains(&id)));
        if !demanded {
            continue;
        }
        let member = member.ok_or("reached target family has no member for the selected target")?;
        let member_id = match member.callable {
            TargetMachineCallable::Boundary(id) => id,
            TargetMachineCallable::Machine(machine) => {
                // Ordinary machine members need no provider-row alias. Target
                // installation already retains their exact concrete call edge.
                module
                    .machine(machine)
                    .ok_or("reached family member has no checked machine")?;
                continue;
            }
        };
        let head = module
            .boundary_machine(head_id)
            .and_then(|boundary| boundary.declaration.as_ref())
            .ok_or("reached target family has no checked head contract")?;
        let member = module
            .boundary_machine(member_id)
            .and_then(|boundary| boundary.declaration.as_ref())
            .ok_or("reached target family has no checked member contract")?;
        let mut external = module.external_providers.iter().filter(|source| {
            source.realization == member.source
                && source
                    .target
                    .as_deref()
                    .is_none_or(|source| source == target)
        });
        let (Some(external), None) = (external.next(), external.next()) else {
            return Err("reached boundary family has no unique admitted external member".into());
        };
        external.validate(&module.identities)?;
        let actual = text(module, external.requirement_identity)?;
        let begin = planned.partition_point(|(required, _)| *required < actual);
        let end = begin + planned[begin..].partition_point(|(required, _)| *required == actual);
        let mut matching = planned[begin..end]
            .iter()
            .map(|(required, plan)| (*plan, *required))
            .collect::<Vec<_>>();
        let begin = associations.partition_point(|(required, _, _, _)| *required < actual);
        let end = begin
            + associations[begin..].partition_point(|(required, _, _, _)| *required == actual);
        for (_, required, provider, callable) in &associations[begin..end] {
            let key = (*required, *provider, *callable);
            let begin = adapters.partition_point(|(planned, _)| *planned < key);
            let end = begin + adapters[begin..].partition_point(|(planned, _)| *planned == key);
            for (_, plan) in &adapters[begin..end] {
                if !matching.iter().any(|(previous, previous_required)| {
                    std::ptr::eq(*previous, *plan) && previous_required == required
                }) {
                    matching.push((*plan, *required));
                }
            }
        }
        let [(plan, required)] = matching.as_slice() else {
            return Err("reached boundary family has no unique exact selected provider row".into());
        };
        result.push(backend_plan::SelectedFamilyRealizationSource {
            head_declaration_identity: text(module, head.source.semantic_declaration)?.to_owned(),
            head_callable_identity: text(module, head.source.callable)?.to_owned(),
            target: target.to_owned(),
            selected_requirement_identity: (*required).to_owned(),
            provider_plan_report_identity: plan.report_fingerprint(),
            provider_plan_digest: plan.identity_digest(),
        });
    }
    Ok(result)
}

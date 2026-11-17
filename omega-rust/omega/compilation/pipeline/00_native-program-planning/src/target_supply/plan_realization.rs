//! Derive provider plans from the canonical contracts and exact choices in Psi.
//!
//! The source producer has already normalized the selected static value and
//! checked its conformance. This operation neither reopens source nor treats
//! retaining a candidate or a descriptor as an invocation.

use std::sync::Arc;

use effects::provider_plan::{
    ProviderBinding, ProviderIdentityTable, ProviderPlan, ProviderPlanRow, ServiceSchema,
};
use semantic_vocabulary::BoundaryMachineId;
use terminal_psi::TerminalModule;

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
    logical: &'a terminal_semantics::logical_providers::ClosedLogicalProviders,
    module: &'a TerminalModule,
    target: &'a str,
    reached: &'a [BoundaryMachineId],
    calling_applications: Vec<calling_conventions::BoundaryCallingApplication>,
    external_bindings: Vec<calling_conventions::ExternalBindingRow>,
    evaluated_native_bindings: Vec<effects::provider_plan::EvaluatedNativeBinding>,
    evaluated_binding_identities: terminal_psi::TerminalIdentityTable,
    terminal_authority_permissions: Vec<effects::ServiceTerminalAuthorityPermission>,
    terminal_authority_permission_identities: terminal_psi::TerminalIdentityTable,
}

impl<'a> TargetSupplyRealizationRequest<'a> {
    pub fn from_execution_inputs(
        module: &'a TerminalModule,
        logical: &'a terminal_semantics::logical_providers::ClosedLogicalProviders,
        inputs: TargetSupplyExecutionInputs,
    ) -> Self {
        Self {
            module,
            logical,
            target: logical.target(),
            reached: logical.requirements(),
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

pub use terminal_semantics::logical_providers::ProviderSelectionError as TargetSupplyRealizationError;

fn checked_plan(
    module: &TerminalModule,
    logical: &terminal_semantics::logical_providers::LogicalProvider,
    calling: &[calling_conventions::BoundaryCallingApplication],
) -> Result<ProviderPlanRealizationInput, TargetSupplyRealizationError> {
    let choice = &logical.choice;
    let schema = &logical.schema;
    let provider = text(module, choice.provider_identity)?;
    let owner = text(module, choice.provider_declaration)?;
    let mut schema_value = ServiceSchema::from_declaration(Arc::clone(&schema.surface));
    attach_calling(&mut schema_value, &choice.target, calling)?;
    let mut identities = ProviderIdentityTable::default();
    let rows = schema.surface.methods.iter().zip(&logical.rows).map(|(method, row)| ProviderPlanRow {
        method: method.name.clone(),
        requirement_identity: identities.intern(&row.requirement),
        declared_requirement_identity: identities.intern(&row.requirement),
        calling_signature: None,
        requirement_lifetime_partition: Vec::new(),
        admitted_same_stack_bytes: 0,
        binding: ProviderBinding::CheckedAdapter {
            machine_identity: identities.intern(&row.machine),
            machine_package_identity: row.machine_package,
        },
    }).collect::<Vec<_>>();
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
        provider_declaration: Some(owner.to_owned()),
        row_numeric_realizations: vec![None; count],
        provider_application,
        provider_type_package_identity: choice.provider_package,
        target: choice.target.clone(),
        schema: schema_value,
        identities,
        rows,
        origin_package_identity: logical.origin_package,
        origin_package: owner.to_owned(),
        ..ProviderPlan::default()
    };
    Ok(ProviderPlanRealizationInput {
        composition_mode: choice.composition_mode,
        schema_declaration: choice.schema_declaration,
        schema_application: choice.schema_application,
        declaration: plan,
        row_executions: vec![None; count],
    })
}

fn direct_leaves(
    module: &TerminalModule,
    target: &str,
    calling: &[calling_conventions::BoundaryCallingApplication],
    external: &[calling_conventions::ExternalBindingRow],
    evaluated: &[effects::provider_plan::EvaluatedNativeBinding],
    evaluated_identities: &terminal_psi::TerminalIdentityTable,
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
            declaration_contracts::ExternalBindingIdentity::CompilerIntrinsic => (
                ProviderBinding::CompilerIntrinsic {
                    machine: callable.to_owned(),
                },
                crate::DirectBoundaryMechanism::Intrinsic,
                backend_plan::DirectBoundaryExecution::Intrinsic(
                    effects::external_providers::canonical_external_execution(row, target)?,
                ),
            ),
            declaration_contracts::ExternalBindingIdentity::Syscall { number } => {
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
            ..ProviderPlan::default()
        };
        let input = crate::DirectBoundaryInput::from_checked(0, row, &module.identities, mechanism)
            .map_err(|error| error.message)?;
        leaves.push(
            input
                .finish(&original, execution)
                .map_err(|error| error.message)?,
        );
    }
    for &id in reached {
        let boundary = module
            .boundary_machine(id)
            .ok_or("plain native demand names an absent boundary")?;
        if boundary.kind != semantic_vocabulary::BoundaryRequirementKind::Trait {
            continue;
        }
        let requirement = text(module, boundary.identity)?;
        let mut bindings = external
            .iter()
            .filter(|row| row.target_name == target && row.requirement_identity == requirement);
        let Some(binding) = bindings.next() else {
            continue;
        };
        if bindings.next().is_some() {
            return Err("plain native requirement has competing retained external bindings".into());
        }
        let schema = backend_plan::DirectBoundaryLeaf::declared_schema(module, id)?;
        let mut receipts = evaluated.iter().filter(|row| {
            row.target().target_name() == target
                && row.receipt().build_origin().is_some_and(|origin| {
                    evaluated_identities.text(origin.requirement_identity) == Some(requirement)
                })
        });
        let (Some(receipt), None) = (receipts.next(), receipts.next()) else {
            return Err("plain native requirement has no unique evaluated Build supply".into());
        };
        let signature = backend_plan::DirectBoundaryLeaf::declared_calling_abi(module, id)?;
        let declared_requirement = text(module, signature.requirement_identity)?;
        let mut methods = schema.surface.methods.iter().filter(|method| {
            schema.surface.identity_text(method.requirement_identity) == declared_requirement
        });
        let (Some(method), None) = (methods.next(), methods.next()) else {
            return Err("plain native requirement lost its exact complete service method".into());
        };
        let mut schema_value = ServiceSchema::from_declaration(Arc::clone(&schema.surface));
        let abi = signature.identity(&module.identities)?;
        let selected_calling = calling
            .iter()
            .filter(|application| application.abi == abi)
            .cloned()
            .collect::<Vec<_>>();
        attach_calling(&mut schema_value, target, &selected_calling)?;
        let mut identities = ProviderIdentityTable::default();
        let requirement_id = identities.intern(requirement);
        let declared_requirement_id = identities.intern(declared_requirement);
        let original = ProviderPlan {
            name: format!("direct-build:{target}:{requirement}"),
            target: target.to_owned(),
            schema: schema_value,
            identities,
            rows: vec![ProviderPlanRow {
                method: method.name.clone(),
                requirement_identity: requirement_id,
                declared_requirement_identity: declared_requirement_id,
                calling_signature: Some(signature.id),
                requirement_lifetime_partition: Vec::new(),
                admitted_same_stack_bytes: 0,
                binding: receipt.provider_binding(),
            }],
            origin_package_identity: receipt.receipt().producer_package(),
            origin_package: text(module, boundary.owner.identity())?.to_owned(),
            ..ProviderPlan::default()
        };
        let input = crate::DirectBoundaryInput::from_boundary(
            module,
            id,
            crate::DirectBoundaryMechanism::Foreign(Ok(binding.clone())),
        )
        .map_err(|error| error.message)?;
        let leaf = input
            .finish(
                &original,
                backend_plan::DirectBoundaryExecution::Foreign(Ok(binding.clone())),
            )
            .map_err(|error| error.message)?;
        leaf.validate_plain_execution(module, evaluated, evaluated_identities)?;
        leaves.push(leaf);
    }

    Ok(leaves)
}

pub fn realize_target_supply(
    request: TargetSupplyRealizationRequest<'_>,
) -> Result<RealizedTargetSupply, TargetSupplyRealizationError> {
    let module = request.module;
    target::TargetProfile::from_canonical_target_name(request.target)
        .map_err(|error| error.to_string())?;
    request.logical.require_program(terminal_codec::terminal_psi_identity(module)
        .map_err(|error| format!("native planning lost its canonical input identity: {error:?}"))?)?;
    if request.target != request.logical.target() || request.reached != request.logical.requirements() {
        return Err("native planning changed its closed logical target or requirements".into());
    }
    let logical = request.logical.providers();
    let mut plans = Vec::with_capacity(logical.len());
    let mut covered = vec![false; request.reached.len()];
    for selected in logical {
        plans.push(checked_plan(module, selected, &request.calling_applications)?);
        for (ordinal, boundary) in request.reached.iter().enumerate() {
            covered[ordinal] |= selected.covered.contains(boundary);
        }
    }
    let mut direct_boundary_leaves = direct_leaves(
        module,
        request.target,
        &request.calling_applications,
        &request.external_bindings,
        &request.evaluated_native_bindings,
        &request.evaluated_binding_identities,
        request.reached,
    )?;
    let primitive_schemas = crate::compiler_primitive_schemas(module)?;
    for leaf in crate::direct_compiler_primitive_leaves(
        module,
        &primitive_schemas,
        request.target,
        request.reached.iter().copied(),
    )
    .map_err(|diagnostics| {
        diagnostics
            .into_iter()
            .map(|diagnostic| diagnostic.message)
            .collect::<Vec<_>>()
            .join("; ")
    })? {
        if !direct_boundary_leaves.iter().any(|known| {
            known.declaration_identity == leaf.declaration_identity
                && known.callable_identity == leaf.callable_identity
        }) {
            direct_boundary_leaves.push(leaf);
        }
    }
    let mut missing = Vec::new();
    for (ordinal, boundary) in request.reached.iter().enumerate() {
        if covered[ordinal] {
            continue;
        }
        let declaration = module
            .boundary_machine(*boundary)
            .and_then(|boundary| boundary.declaration.as_ref())
            .ok_or("reached boundary has no genuine checked declaration")?;
        if !direct_boundary_leaves.iter().any(|leaf| {
            module.identities.text(
                if module.boundary_machine(*boundary).is_some_and(|row| {
                    row.kind == semantic_vocabulary::BoundaryRequirementKind::Trait
                        || row.declaration.as_ref().is_some_and(|declaration| {
                            declaration.execution
                                != terminal_psi::CheckedBoundaryExecution::Provider
                        })
                }) {
                    declaration.source.semantic_declaration
                } else {
                    declaration.source.machine.identity()
                },
            ) == Some(leaf.declaration_identity.as_str())
                && module.identities.text(declaration.source.callable)
                    == Some(leaf.callable_identity.as_str())
        }) {
            missing.push(*boundary);
        }
    }
    if !missing.is_empty() {
        // Keep every failed coordinate until the source-owning caller can join
        // its retained diagnostic labels. Labels never participate in supply.
        return Err(TargetSupplyRealizationError::MissingSupplies {
            target: request.target.to_owned(),
            boundaries: missing,
        });
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
    .map_err(TargetSupplyRealizationError::ExistingDiagnostics)
}

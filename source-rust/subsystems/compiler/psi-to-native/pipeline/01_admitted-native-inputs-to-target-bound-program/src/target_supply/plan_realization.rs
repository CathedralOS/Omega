//! Derive provider plans from the canonical contracts and exact choices in Psi.
//!
//! The source producer has already normalized the selected static value and
//! checked its conformance. This operation neither reopens source nor treats
//! retaining a candidate or a descriptor as an invocation.

use std::sync::Arc;

use build_providers::plans::{
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
    pub components: Option<PublishedComponentSuppliers>,
    pub calling_applications: Vec<calling_conventions::BoundaryCallingApplication>,
    pub external_bindings: Vec<calling_conventions::ExternalBindingRow>,
    pub evaluated_native_bindings: Vec<build_providers::plans::EvaluatedNativeBinding>,
    pub evaluated_binding_identities: terminal_psi::TerminalIdentityTable,
    pub terminal_authority_permissions:
        Vec<build_providers::acceptance::ServiceTerminalAuthorityPermission>,
    pub terminal_authority_permission_identities: terminal_psi::TerminalIdentityTable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishedComponentSuppliers {
    pub receiver: Arc<terminal_codec::CanonicalTerminalArtifact>,
    pub suppliers: std::collections::BTreeMap<
        semantic_vocabulary::PackageKeyIdentity,
        Arc<component_description::VerifiedComponent>,
    >,
}

pub struct TargetSupplyRealizationRequest<'a> {
    components: Option<PublishedComponentSuppliers>,
    logical: &'a terminal_semantics::logical_providers::ClosedLogicalProviders,
    module: &'a TerminalModule,
    /// The identity of `module`, taken from its sealed artifact or hashed
    /// once by the caller that holds only the module.
    program: terminal_psi::TerminalPsiIdentity,
    target: &'a str,
    reached: &'a [BoundaryMachineId],
    calling_applications: Vec<calling_conventions::BoundaryCallingApplication>,
    external_bindings: Vec<calling_conventions::ExternalBindingRow>,
    evaluated_native_bindings: Vec<build_providers::plans::EvaluatedNativeBinding>,
    evaluated_binding_identities: terminal_psi::TerminalIdentityTable,
    terminal_authority_permissions:
        Vec<build_providers::acceptance::ServiceTerminalAuthorityPermission>,
    terminal_authority_permission_identities: terminal_psi::TerminalIdentityTable,
}

impl<'a> TargetSupplyRealizationRequest<'a> {
    /// Realize supply for an accepted artifact. The artifact's sealed identity
    /// names its module, so realization never re-hashes the program.
    pub fn from_execution_inputs(
        artifact: terminal_codec::TerminalArtifactView<'a>,
        logical: &'a terminal_semantics::logical_providers::ClosedLogicalProviders,
        inputs: TargetSupplyExecutionInputs,
    ) -> Self {
        Self::for_identified_module(artifact.module(), artifact.identity(), logical, inputs)
    }

    /// `program` must be the identity of `module`: either its artifact's sealed
    /// identity or `terminal_psi_identity(module)`.
    pub(crate) fn for_identified_module(
        module: &'a TerminalModule,
        program: terminal_psi::TerminalPsiIdentity,
        logical: &'a terminal_semantics::logical_providers::ClosedLogicalProviders,
        inputs: TargetSupplyExecutionInputs,
    ) -> Self {
        Self {
            module,
            program,
            logical,
            target: logical.target(),
            reached: logical.requirements(),
            components: inputs.components,
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
    use build_providers::plans::{
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
    let mut rows = Vec::with_capacity(logical.rows.len());
    let mut row_realizations = Vec::with_capacity(logical.rows.len());
    let mut row_executions = Vec::with_capacity(logical.rows.len());
    for (method, row) in schema.surface.methods.iter().zip(&logical.rows) {
        let (binding, declaration, execution) = match &row.realization {
            terminal_semantics::logical_providers::LogicalRealization::Checked(_) => (
                ProviderBinding::CheckedAdapter {
                    machine_identity: identities.intern(&row.machine),
                    machine_package_identity: row.machine_package,
                },
                None,
                None,
            ),
            terminal_semantics::logical_providers::LogicalRealization::External(position) => {
                let supply = module
                    .interfaces
                    .external_providers
                    .get(*position)
                    .ok_or("logical provider lost its checked external supply")?;
                let execution =
                    provider_correspondence::external_providers::canonical_external_execution(
                        supply,
                        &choice.target,
                    )?
                    .ok_or("selected external provider has no canonical execution")?;
                (
                    ProviderBinding::CompilerIntrinsic {
                        machine: row.machine.clone(),
                    },
                    Some(text(module, supply.realization.machine.identity())?.to_owned()),
                    Some(execution),
                )
            }
        };
        rows.push(ProviderPlanRow {
            method: method.name.clone(),
            requirement_identity: identities.intern(&row.requirement),
            declared_requirement_identity: identities.intern(&row.requirement),
            calling_signature: None,
            requirement_lifetime_partition: Vec::new(),
            admitted_same_stack_bytes: 0,
            binding,
        });
        row_realizations.push(declaration);
        row_executions.push(execution);
    }
    let provider_application = choice
        .provider_application
        .map(|identity| text(module, identity))
        .transpose()?
        .map(|identity| identities.intern(identity))
        .unwrap_or_default();
    let plan = ProviderPlan {
        name: crate::satisfies_plan_name(
            &choice.target,
            schema.surface.identity_text(schema.surface.trait_name),
            provider,
        ),
        provider_type: provider.to_owned(),
        provider_declaration: Some(owner.to_owned()),
        row_numeric_realizations: row_realizations,
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
        component: None,
        composition_mode: choice.composition_mode,
        schema_declaration: choice.schema_declaration,
        schema_application: choice.schema_application,
        declaration: plan,
        row_executions,
    })
}

fn direct_leaves(
    module: &TerminalModule,
    target: &str,
    calling: &[calling_conventions::BoundaryCallingApplication],
    external: &[calling_conventions::ExternalBindingRow],
    evaluated: &[build_providers::plans::EvaluatedNativeBinding],
    evaluated_identities: &terminal_psi::TerminalIdentityTable,
    reached: &[BoundaryMachineId],
) -> Result<Vec<direct_boundary_bindings::DirectBoundaryLeaf>, String> {
    let mut leaves = Vec::new();
    let mut declarations = module
        .interfaces.boundary_machines
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
        .interfaces.boundary_machines
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
    let mut schemas = module.interfaces.service_schemas.iter().collect::<Vec<_>>();
    schemas.sort_by_key(|schema| schema.declaration);
    let mut foreign_bindings = external
        .iter()
        .filter(|row| row.target_name == target)
        .collect::<Vec<_>>();
    foreign_bindings.sort_by_key(|row| row.requirement_identity.as_str());
    for row in &module.interfaces.external_providers {
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
            semantic_vocabulary::ExternalBindingIdentity::CompilerIntrinsic => (
                ProviderBinding::CompilerIntrinsic {
                    machine: callable.to_owned(),
                },
                crate::DirectBoundaryMechanism::Intrinsic,
                direct_boundary_bindings::DirectBoundaryExecution::Intrinsic(
                    provider_correspondence::external_providers::canonical_external_execution(row, target)?,
                ),
            ),
            semantic_vocabulary::ExternalBindingIdentity::Syscall { number } => {
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
                    direct_boundary_bindings::DirectBoundaryExecution::Foreign(execution),
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
        let schema = direct_boundary_bindings::DirectBoundaryLeaf::declared_schema(module, id)?;
        let mut receipts = evaluated.iter().filter(|row| {
            row.target().target_name() == target
                && row.receipt().build_origin().is_some_and(|origin| {
                    evaluated_identities.text(origin.requirement_identity) == Some(requirement)
                })
        });
        let (Some(receipt), None) = (receipts.next(), receipts.next()) else {
            return Err("plain native requirement has no unique evaluated Build supply".into());
        };
        let signature = direct_boundary_bindings::DirectBoundaryLeaf::declared_calling_abi(module, id)?;
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
                direct_boundary_bindings::DirectBoundaryExecution::Foreign(Ok(binding.clone())),
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
    let program = request.program;
    request.logical.require_program(program)?;
    if request.target != request.logical.target()
        || request.reached != request.logical.requirements()
    {
        return Err("native planning changed its closed logical target or requirements".into());
    }
    let logical = request.logical.providers();
    let mut plans = Vec::with_capacity(logical.len());
    // Each provider names the reached boundaries it covers. Look them up by
    // sorted boundary instead of rescanning every covered list per boundary.
    let mut reached_order = request
        .reached
        .iter()
        .enumerate()
        .map(|(ordinal, boundary)| (*boundary, ordinal))
        .collect::<Vec<_>>();
    reached_order.sort_unstable();
    let mut covered = vec![false; request.reached.len()];
    for selected in logical {
        let mut plan = checked_plan(module, selected, &request.calling_applications)?;
        if let Some(components) = &request.components
            && selected.choice.composition_mode
                == semantic_vocabulary::source_selections::CompositionMode::Independent
        {
            if components.receiver.view().identity() != request.logical.program() {
                return Err("component imports belong to another receiving publication".into());
            }
            let supplier = selected
                .choice
                .provider_package
                .and_then(|package| components.suppliers.get(&package))
                .ok_or("independent provider has no separately verified supplier package")?;
            plan.component = Some(verified_optimization_input::ComponentImport::admit(
                components.receiver.view(),
                Arc::clone(supplier),
                &plan.declaration,
            )?);
        }
        plans.push(plan);
        for boundary in &selected.covered {
            let first = reached_order.partition_point(|(reached, _)| reached < boundary);
            for (_, ordinal) in reached_order[first..]
                .iter()
                .take_while(|(reached, _)| reached == boundary)
            {
                covered[*ordinal] = true;
            }
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
    let primitive_leaves = crate::direct_compiler_primitive_leaves(
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
    })?;
    // A primitive leaf joins only when no earlier leaf has its key.
    let mut known = direct_boundary_leaves
        .iter()
        .map(super::direct_leaf_key)
        .collect::<Vec<_>>();
    known.sort_unstable();
    let mut joins = vec![false; primitive_leaves.len()];
    for (joined, leaf) in joins.iter_mut().zip(&primitive_leaves) {
        let key = super::direct_leaf_key(leaf);
        if let Err(position) = known.binary_search(&key) {
            known.insert(position, key);
            *joined = true;
        }
    }
    direct_boundary_leaves.extend(
        primitive_leaves
            .into_iter()
            .zip(joins)
            .filter_map(|(leaf, joined)| joined.then_some(leaf)),
    );
    let leaf_keys = super::sorted_leaf_keys(&direct_boundary_leaves);
    let mut missing = Vec::new();
    for (ordinal, boundary) in request.reached.iter().enumerate() {
        if covered[ordinal] {
            continue;
        }
        let row = module
            .boundary_machine(*boundary)
            .ok_or("reached boundary has no genuine checked declaration")?;
        let declaration = row
            .declaration
            .as_ref()
            .ok_or("reached boundary has no genuine checked declaration")?;
        if !super::boundary_leaf_key(&module.identities, row.kind, declaration)
            .is_some_and(|key| !super::entries_with_key(&leaf_keys, key).is_empty())
        {
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
        program,
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

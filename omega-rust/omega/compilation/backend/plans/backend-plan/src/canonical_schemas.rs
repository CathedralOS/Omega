//! Native primitive planning reads the public contract retained in the accepted
//! module. A direct primitive does not require an unrelated selected provider
//! just to recover that contract's nominal result and progress information.

use effects::provider_plan::{ServiceMethod, ServiceSchema, ServiceSurface};
use language_core::sealed_intrinsics::HostedOperation;
use language_semantics::service_surface::ServiceReceiverApplication;
use semantic_vocabulary::BoundaryMachineId;
use terminal_psi::{CheckedBoundaryExecution, TerminalModule};

/// Project the complete supported hosted requirement, preserving declared type
/// identities even when its physical result occupies no storage. The caller
/// supplies an already admitted declaration; this is not source authentication
/// or an authority grant. This native schema uses the canonical module's service
/// application and package-qualified type coordinates, rather than source
/// display paths and ordinary type coordinates.
pub fn hosted_requirement_schema(
    module: &TerminalModule,
    operation: HostedOperation,
    requirement: BoundaryMachineId,
) -> Result<ServiceSchema, String> {
    let boundary = module
        .boundary_machine(requirement)
        .ok_or("hosted schema has no canonical requirement")?;
    let declaration = boundary
        .declaration
        .as_ref()
        .ok_or("hosted schema has no complete declared callable")?;
    let method_name = match operation {
        HostedOperation::ProcessExit => "exit_process",
        _ => return Err("hosted operation has no complete canonical schema projection".into()),
    };
    // The supported canonical contract has no receiver arguments, structural
    // claim, or progress-profile parameter. Do not invent empty rows for an
    // interface whose declaration actually carries one of those obligations.
    if !boundary.structural_parameters.is_empty()
        || !boundary.result_reference_regions.is_empty()
        || boundary
            .result_lifetimes
            .as_ref()
            .is_some_and(|groups| !groups.is_empty())
        || !boundary.requires.is_empty()
        || !boundary.normal_qualifications.is_empty()
        || !boundary.scalar_domain_requirements.is_empty()
        || !boundary.scalar_requires.is_empty()
        || !boundary.scalar_ensures.is_empty()
        || !boundary.crash_routes.is_empty()
        || !boundary.reference_outputs.is_empty()
        || !boundary.program_local_root_introductions.is_empty()
        || !boundary.content_guarantees.is_empty()
        || !boundary.unconditional_termination
        || !declaration.method_arguments.is_empty()
        || declaration.source.specialization.is_some()
        || declaration.source.parameter_types.len() != boundary.parameter_order.len()
    {
        return Err("hosted schema needs its complete canonical contract projection".into());
    }
    let owner_service = boundary
        .owning_service
        .ok_or("hosted schema lost its declaring service")?;
    let owner_declaration = module
        .services
        .iter()
        .find(|service| service.id == owner_service)
        .ok_or("hosted schema lost its declaring service")?;
    if !owner_declaration.parents.is_empty() {
        return Err("hosted schema needs its complete inherited service projection".into());
    }
    requirement_schema(
        module,
        boundary,
        text(module, owner_declaration.identity)?,
        text(module, owner_declaration.identity)?,
        method_name,
    )
}

fn text(
    module: &TerminalModule,
    identity: terminal_psi::TerminalIdentityIndex,
) -> Result<&str, String> {
    module
        .identities
        .text(identity)
        .filter(|text| !text.is_empty())
        .ok_or_else(|| "primitive schema lost a declared identity".to_owned())
}

/// Hosted and numeric primitives use this function to build their callable schemas.
fn requirement_schema(
    module: &TerminalModule,
    boundary: &terminal_psi::BoundaryMachineDeclaration,
    schema_name: &str,
    owner: &str,
    method_name: &str,
) -> Result<ServiceSchema, String> {
    let declaration = boundary
        .declaration
        .as_ref()
        .ok_or("primitive schema has no checked declaration")?;
    let text = |identity| text(module, identity);
    let service_name = |id| {
        let service = module
            .services
            .iter()
            .find(|service| service.id == id)
            .ok_or_else(|| "primitive schema lost a declared service".to_owned())?;
        text(service.identity)
    };
    let mut surface = ServiceSurface::default();
    surface.trait_name = surface.identities.intern(schema_name);
    let parameter_type_identities = declaration
        .source
        .parameter_types
        .iter()
        .map(|identity| text(*identity).map(|text| surface.identities.intern(text)))
        .collect::<Result<Vec<_>, _>>()?;
    let result_type_identity = surface
        .identities
        .intern(text(declaration.source.result_type)?);
    let requirement_identity = surface.identities.intern(text(boundary.identity)?);
    if boundary.kind == semantic_vocabulary::BoundaryRequirementKind::Trait {
        surface
            .receiver_applications
            .push(ServiceReceiverApplication {
                application: surface.trait_name,
                trait_name: surface.trait_name,
                trait_package_identity: None,
                argument_identities: Vec::new(),
                lifetime_argument_identities: Vec::new(),
                requirement_identities: vec![requirement_identity],
            });
    }
    let services = |row: &[semantic_vocabulary::ServiceId]| {
        let mut names = row
            .iter()
            .map(|id| service_name(*id).map(str::to_owned))
            .collect::<Result<Vec<_>, _>>()?;
        names.sort_unstable();
        names.dedup();
        Ok::<_, String>(names)
    };
    surface.methods.push(ServiceMethod {
        completion: boundary.completion,
        name: method_name.into(),
        requirement_owner: owner.into(),
        requirement_identity,
        parameter_count: boundary.parameter_order.len(),
        declared_type_signature: language_semantics::service_surface::declared_type_signature(
            declaration
                .source
                .parameter_types
                .iter()
                .map(|&identity| text(identity))
                .collect::<Result<Vec<_>, _>>()?
                .into_iter(),
            text(declaration.source.result_type)?,
        )
        .map_err(|reason| format!("primitive declared type signature: {reason:?}"))?,
        parameter_type_identities,
        has_result: !boundary.result.is_unit(),
        result_type_identity,
        service_reach: services(&boundary.published_service_ceiling)?,
        synchronous_invocations: services(&boundary.published_synchronous_entries)?,
        may_suspend: declaration.source.published_may_suspend,
        may_block: declaration.source.published_may_block,
        terminates_guarantee: boundary.unconditional_termination,
        ..Default::default()
    });
    Ok(ServiceSchema::from_declaration(std::sync::Arc::new(
        surface,
    )))
}

pub fn compiler_primitive_schemas(module: &TerminalModule) -> Result<Vec<ServiceSchema>, String> {
    let mut projected = Vec::new();
    let mut requirements = Vec::new();
    for boundary in &module.boundary_machines {
        let Some(declaration) = &boundary.declaration else {
            continue;
        };
        let requirement = match declaration.execution {
            CheckedBoundaryExecution::HostedPrimitive { requirement, .. } => requirement,
            CheckedBoundaryExecution::NumericPrimitive => boundary.id,
            _ => continue,
        };
        if requirements.contains(&requirement) {
            continue;
        }
        projected.push(match declaration.execution {
            CheckedBoundaryExecution::HostedPrimitive { operation, .. } => {
                hosted_requirement_schema(module, operation, requirement)?
            }
            CheckedBoundaryExecution::NumericPrimitive => {
                numeric_requirement_schema(module, boundary)?
            }
            _ => unreachable!("only direct primitives reach schema projection"),
        });
        requirements.push(requirement);
    }
    Ok(projected)
}

pub(crate) fn numeric_requirement_schema(
    module: &TerminalModule,
    boundary: &terminal_psi::BoundaryMachineDeclaration,
) -> Result<ServiceSchema, String> {
    terminal_semantics::numeric_primitives::validate_boundary(module, boundary)
        .map_err(str::to_owned)?;
    let meaning = boundary
        .numeric_intrinsic
        .ok_or("numeric primitive has no complete top-level requirement")?;
    let declaration = terminal_semantics::numeric_primitives::declaration_path(meaning)?;
    let (owner, method) = declaration
        .rsplit_once("::")
        .ok_or("numeric primitive has no declaration owner")?;
    requirement_schema(
        module,
        boundary,
        text(module, boundary.identity)?,
        owner,
        method,
    )
}

/*
We project complete supplied terminal permission meaning and its checked
service context independently of provider demand.

This projection neither accepts permissions nor replaces native permission
matching. Different accepted roles may supply disjoint permissions for the same
service, so we group them by the exact checked schema rather than the role.
*/

use super::declarations::{rejected, resolve_services};
use crate::capture::PackageReviewInput;
use crate::capture::semantics::services;
use crate::record::{
    PackagePolicyTerminalPermission, PackagePolicyTerminalPermissions, PackagePolicyTerminalService,
};
use diagnostics::Diagnostic;
use semantic_vocabulary::PackageKeyIdentity;
use target::TargetProfile;

pub(crate) fn project_source_terminal_permission_policy<'a>(
    input: impl Into<PackageReviewInput<'a>>,
    target: TargetProfile,
    package: PackageKeyIdentity,
) -> Result<PackagePolicyTerminalPermissions, Vec<Diagnostic>> {
    let input = input.into();
    let services = input
        .service_declarations
        .is_none()
        .then(|| input.project_service_declarations())
        .transpose()?;
    let compilation = &PackageReviewInput {
        service_declarations: input.service_declarations.or(services.as_ref()),
        ..input
    };
    if compilation.custody.sources().package_identity() != Some(package)
        || compilation.custody.execution().selected_target_profile != Some(target)
        || compilation.custody.execution().selected_native_target() != Some(target.native_target())
    {
        return Err(rejected(
            "policy package or target differs from the checked root activation",
        ));
    }
    let mut projected: Vec<PackagePolicyTerminalService> = Vec::new();
    let mut signatures = services::SignatureProjection::new(*compilation);
    for resolved in resolve_services(compilation)? {
        if resolved.permissions.is_empty() {
            continue;
        }
        let (static_parameters, lifetime_parameter_count) =
            services::declaration_parameters(&mut signatures, resolved.symbol)?;
        let methods = resolved
            .schema
            .declaration
            .methods
            .iter()
            .zip(&resolved.requirements)
            .map(|(method, requirement)| {
                services::project_declaration(
                    &mut signatures,
                    resolved.schema,
                    resolved.symbol,
                    *requirement,
                    method,
                    &resolved.schema.declaration.identities,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let permissions = resolved
            .permissions
            .into_iter()
            .map(|permission| PackagePolicyTerminalPermission {
                requirement: permission.requirement,
                permitted: permission.supplied.permitted().clone(),
            })
            .collect::<Vec<_>>();
        if let Some(prior) = projected
            .iter_mut()
            .find(|prior| prior.service == resolved.service)
        {
            if prior.methods != methods
                || prior.static_parameters != static_parameters
                || prior.lifetime_parameter_count != lifetime_parameter_count
            {
                return Err(rejected(
                    "one service retains inconsistent checked method contexts",
                ));
            }
            prior.permissions.extend(permissions);
        } else {
            projected.push(PackagePolicyTerminalService {
                service: resolved.service,
                static_parameters,
                lifetime_parameter_count,
                methods,
                permissions,
            });
        }
    }
    projected.sort_by(|left, right| left.service.cmp(&right.service));
    for service in &mut projected {
        service
            .permissions
            .sort_by(|left, right| left.requirement.cmp(&right.requirement));
    }
    let policy = PackagePolicyTerminalPermissions {
        package,
        target,
        services: projected,
    };
    Ok(policy)
}

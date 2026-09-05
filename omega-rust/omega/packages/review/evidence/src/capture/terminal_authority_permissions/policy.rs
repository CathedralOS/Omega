//! Complete supplied permission meaning, independent of provider demand.

use super::declarations::{rejected, resolve_services};
use crate::capture::semantics::services;
use crate::record::{
    PackagePolicyTerminalPermission, PackagePolicyTerminalPermissions, PackagePolicyTerminalService,
};
use compiler::CheckedCompilation;
use diagnostics::Diagnostic;
use semantic_vocabulary::PackageKeyIdentity;
use target::TargetProfile;

/// Capture supplied permissions and their complete checked service context.
/// This neither accepts permissions nor replaces native permission matching.
pub fn project_checked_terminal_permission_policy(
    compilation: &CheckedCompilation,
    target: TargetProfile,
    package: PackageKeyIdentity,
) -> Result<PackagePolicyTerminalPermissions, Vec<Diagnostic>> {
    if compilation.package_identity() != Some(package)
        || compilation.selected_target_profile() != Some(target)
        || compilation.selected_native_target() != Some(target.native_target())
    {
        return Err(rejected(
            "policy package or target differs from the checked root activation",
        ));
    }
    let mut projected: Vec<PackagePolicyTerminalService> = Vec::new();
    for resolved in resolve_services(compilation)? {
        if resolved.permissions.is_empty() {
            continue;
        }
        let (static_parameters, lifetime_parameter_count) =
            services::declaration_parameters(compilation, resolved.symbol)?;
        let methods = resolved
            .schema
            .methods
            .iter()
            .zip(&resolved.requirements)
            .map(|(method, requirement)| {
                services::project_declaration(compilation, resolved.symbol, *requirement, method)
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
        // Different accepted roles may supply disjoint permissions for the
        // same service. Group them by the exact checked schema, not the role.
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
    policy.validate_canonical_structure().map_err(rejected)?;
    Ok(policy)
}

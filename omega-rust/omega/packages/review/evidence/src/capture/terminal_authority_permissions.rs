//! Explicit D45 service permissions rejoined to resolved semantic bindings.
//!
//! Supplied permissions are not inferred from filenames, provider names,
//! semantic roles, or broad risk labels.

mod declarations;
mod policy;
mod requirements;

pub use policy::project_checked_terminal_permission_policy;

use super::source::ProjectedReviewRow;
use super::source::locations::project_nested_declaration_source_location;
use crate::record::{PackageReviewSourceLocationRole, PackageReviewTerminalAuthorityPermission};
use omega_compiler::CheckedCompilation;
use psi_diagnostics::Diagnostic;

pub(crate) fn project_terminal_authority_permissions(
    compilation: &CheckedCompilation,
) -> Result<Vec<ProjectedReviewRow<PackageReviewTerminalAuthorityPermission>>, Vec<Diagnostic>> {
    let mut projected = Vec::new();
    for service in declarations::resolve_services(compilation)? {
        for permission in service.permissions {
            projected.push(ProjectedReviewRow {
                row: PackageReviewTerminalAuthorityPermission {
                    service: service.service.clone(),
                    service_schema: permission.supplied.service_schema(),
                    requirement_identity: permission.supplied.requirement_identity().to_owned(),
                    permitted: permission.supplied.permitted().clone(),
                },
                declaration: service.symbol,
                nested_source_locations: vec![project_nested_declaration_source_location(
                    compilation,
                    permission.requirement_symbol,
                    PackageReviewSourceLocationRole::ProviderRequirementDeclaration,
                    "terminal-authority permission requirement",
                )?],
            });
        }
    }

    projected.sort_by(|left, right| left.row.canonical_cmp(&right.row));
    if projected.windows(2).any(|rows| {
        rows[0].row.service == rows[1].row.service
            && rows[0].row.service_schema == rows[1].row.service_schema
            && rows[0].row.requirement_identity == rows[1].row.requirement_identity
    }) {
        return Err(vec![Diagnostic::error(
            "resolved accepted semantic bindings repeat an exact terminal-authority permission",
        )]);
    }
    Ok(projected)
}

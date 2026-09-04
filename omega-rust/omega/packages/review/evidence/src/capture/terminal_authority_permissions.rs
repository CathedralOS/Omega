//! Projection of explicit D45 service permissions from resolved semantic bindings.
//!
//! This seam only rejoins owner-supplied rows to checked schema and
//! requirement declarations. It deliberately has no filename, provider,
//! method-name, semantic-role, or broad risk-label classifier.

use super::semantics::declarations::nominal_identity;
use super::source::ProjectedReviewRow;
use super::source::locations::project_nested_declaration_source_location;
use crate::record::{PackageReviewSourceLocationRole, PackageReviewTerminalAuthorityPermission};
use omega_compiler::CheckedCompilation;
use psi_diagnostics::Diagnostic;

pub(crate) fn project_terminal_authority_permissions(
    compilation: &CheckedCompilation,
) -> Result<Vec<ProjectedReviewRow<PackageReviewTerminalAuthorityPermission>>, Vec<Diagnostic>> {
    let mut projected = Vec::new();
    for accepted in compilation.resolved_semantic_bindings() {
        let Some(resolved) = compilation.resolved_semantic_binding(accepted.role()) else {
            return Err(vec![Diagnostic::error(format!(
                "resolved accepted semantic binding {:?} lost its exact checked declaration",
                accepted.role(),
            ))]);
        };
        if resolved.accepted() != accepted {
            return Err(vec![Diagnostic::error(format!(
                "resolved accepted semantic binding {:?} disagrees with its retained policy row",
                accepted.role(),
            ))]);
        }

        let service_definitions = compilation
            .traits()
            .iter()
            .filter(|definition| definition.symbol == resolved.declaration_symbol())
            .collect::<Vec<_>>();
        let [service_definition] = service_definitions.as_slice() else {
            return Err(vec![Diagnostic::error(format!(
                "resolved accepted semantic binding {:?} names {} exact checked service declarations instead of one",
                accepted.role(),
                service_definitions.len(),
            ))]);
        };
        let Some(schema) = omega_effects::provider_plan::ServiceSchema::from_typed(
            &compilation.typed,
            service_definition,
        ) else {
            return Err(vec![Diagnostic::error(format!(
                "resolved accepted semantic binding {:?} no longer names a boundary service schema",
                accepted.role(),
            ))]);
        };
        let schema_digest =
            omega_package_compilation::accepted_service_schema_digest(accepted.role(), &schema);
        if schema_digest != accepted.normalized_schema_digest() {
            return Err(vec![Diagnostic::error(format!(
                "resolved accepted semantic binding {:?} changed normalized service schema during package review",
                accepted.role(),
            ))]);
        }
        let service = nominal_identity(compilation, resolved.declaration_symbol())?;

        for permission in accepted.terminal_authority_permissions() {
            if permission.service_schema() != schema_digest {
                return Err(vec![Diagnostic::error(format!(
                    "terminal-authority permission `{}` is not keyed by its resolved accepted service schema",
                    permission.requirement_identity(),
                ))]);
            }
            let methods = schema
                .methods
                .iter()
                .filter(|method| method.requirement_identity == permission.requirement_identity())
                .collect::<Vec<_>>();
            let [method] = methods.as_slice() else {
                return Err(vec![Diagnostic::error(format!(
                    "terminal-authority permission `{}` resolves to {} exact methods in its accepted service schema instead of one",
                    permission.requirement_identity(),
                    methods.len(),
                ))]);
            };

            let mut requirement_symbols = Vec::new();
            for owner in compilation.traits() {
                if compilation
                    .typed
                    .symbols
                    .symbol_package_identity(owner.symbol)
                    != method.requirement_owner_package_identity
                {
                    continue;
                }
                for requirement in compilation.trait_machine_signatures(owner) {
                    if compilation
                        .typed
                        .symbols
                        .symbol_package_identity(requirement.symbol)
                        == method.requirement_owner_package_identity
                        && compilation
                            .normalized_trait_requirement_overload_identity(owner, requirement)
                            .identity()
                            == permission.requirement_identity()
                    {
                        requirement_symbols.push(requirement.symbol);
                    }
                }
            }
            let [requirement_symbol] = requirement_symbols.as_slice() else {
                return Err(vec![Diagnostic::error(format!(
                    "terminal-authority permission `{}` resolves to {} exact checked requirement declarations instead of one",
                    permission.requirement_identity(),
                    requirement_symbols.len(),
                ))]);
            };

            projected.push(ProjectedReviewRow {
                row: PackageReviewTerminalAuthorityPermission {
                    service: service.clone(),
                    service_schema: permission.service_schema(),
                    requirement_identity: permission.requirement_identity().to_owned(),
                    permitted: permission.permitted().clone(),
                },
                declaration: resolved.declaration_symbol(),
                nested_source_locations: vec![project_nested_declaration_source_location(
                    compilation,
                    *requirement_symbol,
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

//! Rejoin supplied permissions to the complete live service declaration.

use crate::capture::PackageReviewInput;
use crate::capture::refusal;
use crate::capture::semantics::declarations::{nominal_identity, provider_requirement_identity};
use crate::record::{PackageReviewNominalIdentity, PackageReviewNominalOwner};
use diagnostics::Diagnostic;
use effects::ServiceTerminalAuthorityPermission;
use effects::provider_plan::ServiceSchema;
use provider_planning::ProviderSchemaDeclaration;
use symbols::SymbolHandle;

pub(super) struct ResolvedService<'a> {
    pub symbol: SymbolHandle,
    pub service: PackageReviewNominalIdentity,
    pub schema: &'a ServiceSchema,
    pub requirements: Vec<SymbolHandle>,
    pub permissions: Vec<ResolvedPermission<'a>>,
}

pub(super) struct ResolvedPermission<'a> {
    pub supplied: &'a ServiceTerminalAuthorityPermission,
    pub requirement: PackageReviewNominalIdentity,
}

pub(super) fn resolve_services<'a>(
    compilation: &'a PackageReviewInput<'_>,
) -> Result<Vec<ResolvedService<'a>>, Vec<Diagnostic>> {
    let mut services = Vec::new();
    for accepted in compilation.custody.execution().accepted_semantic_bindings() {
        let resolved = compilation
            .custody
            .execution()
            .resolved_semantic_binding(accepted.role())
            .ok_or_else(|| rejected("accepted binding lost its exact checked declaration"))?;
        if resolved.accepted() != accepted {
            return Err(rejected(
                "accepted binding differs from its retained policy",
            ));
        }
        let catalog = compilation.service_declarations.ok_or_else(|| {
            rejected("candidate has no independent service declaration projection")
        })?;
        let source = catalog
            .rejoin(resolved.declaration())
            .map_err(|diagnostic| vec![diagnostic])?;
        let symbol = source.symbol;
        let schema = &source.declaration.schema;
        // Preserve the existing UEFI semantic-only binding exception. The
        // normalized review below is context, never a replacement permission
        // digest or a second source of target-entry ABI authority.
        let digest = package_compilation::accepted_service_schema_digest(accepted.role(), &schema);
        if digest != accepted.normalized_schema_digest() {
            return Err(rejected(
                "accepted binding changed its normalized service schema",
            ));
        }
        let service = nominal_identity(compilation, symbol)?;
        // ProcessExit binds a package-owned provider of the core declaration;
        // its package is not the declaration's owner. The resolved binding has
        // already rejoined the exact core symbol and selected provider plan.
        // Retain that toolchain source identity in permission evidence rather
        // than relabeling the service as package-owned.
        let expected_owner = match accepted.role() {
            package_compilation::AcceptedSemanticBindingRole::ProcessExitExitProcessI32 => {
                matches!(
                    service.owner(),
                    PackageReviewNominalOwner::ToolchainSource(_)
                )
            }
            _ => service.owner() == PackageReviewNominalOwner::Package(accepted.package()),
        };
        if !expected_owner {
            return Err(rejected("accepted service changed its exact package owner"));
        }
        let requirements = source.requirements.to_vec();
        let mut permissions = Vec::new();
        for supplied in accepted.terminal_authority_permissions() {
            if supplied.service_schema() != digest {
                return Err(rejected(
                    "permission names a different accepted service schema",
                ));
            }
            let matches = schema
                .declaration
                .methods
                .iter()
                .zip(&requirements)
                .filter(|(method, _)| {
                    accepted
                        .terminal_authority_permission_identities()
                        .text(supplied.requirement_identity())
                        == Some(schema.identity_text(method.requirement_identity))
                })
                .collect::<Vec<_>>();
            let [(_, requirement_symbol)] = matches.as_slice() else {
                return Err(rejected(
                    "permission does not name one exact service requirement",
                ));
            };
            let requirement_symbol = **requirement_symbol;
            permissions.push(ResolvedPermission {
                supplied,
                requirement: provider_requirement_identity(
                    compilation,
                    ProviderSchemaDeclaration::BoundaryTrait(symbol),
                    requirement_symbol,
                )?,
            });
        }
        services.push(ResolvedService {
            symbol,
            service,
            schema,
            requirements,
            permissions,
        });
    }
    Ok(services)
}

refusal!(pub(super) reason, "terminal permission review rejects {reason}");

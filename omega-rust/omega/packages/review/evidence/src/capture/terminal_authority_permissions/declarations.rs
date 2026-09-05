//! Rejoin supplied permissions to the complete live service declaration.

use crate::capture::semantics::declarations::{nominal_identity, provider_requirement_identity};
use crate::record::{PackageReviewNominalIdentity, PackageReviewNominalOwner};
use omega_compiler::CheckedCompilation;
use omega_effects::ServiceTerminalAuthorityPermission;
use omega_effects::provider_plan::ServiceSchema;
use omega_provider_planning::plans::ProviderSchemaDeclaration;
use psi_diagnostics::Diagnostic;
use psi_symbols::SymbolHandle;

pub(super) struct ResolvedService<'a> {
    pub symbol: SymbolHandle,
    pub service: PackageReviewNominalIdentity,
    pub schema: ServiceSchema,
    pub requirements: Vec<SymbolHandle>,
    pub permissions: Vec<ResolvedPermission<'a>>,
}

pub(super) struct ResolvedPermission<'a> {
    pub supplied: &'a ServiceTerminalAuthorityPermission,
    pub requirement_symbol: SymbolHandle,
    pub requirement: PackageReviewNominalIdentity,
}

pub(super) fn resolve_services(
    compilation: &CheckedCompilation,
) -> Result<Vec<ResolvedService<'_>>, Vec<Diagnostic>> {
    let mut services = Vec::new();
    for accepted in compilation.resolved_semantic_bindings() {
        let resolved = compilation
            .resolved_semantic_binding(accepted.role())
            .ok_or_else(|| rejected("accepted binding lost its exact checked declaration"))?;
        if resolved.accepted() != accepted {
            return Err(rejected(
                "accepted binding differs from its retained policy",
            ));
        }
        let symbol = resolved.declaration_symbol();
        let definitions = compilation
            .traits()
            .iter()
            .filter(|definition| definition.symbol == symbol)
            .collect::<Vec<_>>();
        let [definition] = definitions.as_slice() else {
            return Err(rejected("accepted service has no unique exact trait"));
        };
        let schema = ServiceSchema::from_typed(&compilation.typed, definition)
            .ok_or_else(|| rejected("accepted declaration is not a boundary service schema"))?;
        // Preserve the existing UEFI semantic-only binding exception. The
        // normalized review below is context, never a replacement permission
        // digest or a second source of target-entry ABI authority.
        let digest =
            omega_package_compilation::accepted_service_schema_digest(accepted.role(), &schema);
        if digest != accepted.normalized_schema_digest() {
            return Err(rejected(
                "accepted binding changed its normalized service schema",
            ));
        }
        let service = nominal_identity(compilation, symbol)?;
        if service.owner() != PackageReviewNominalOwner::Package(accepted.package()) {
            return Err(rejected("accepted service changed its exact package owner"));
        }
        let requirements = super::requirements::resolve(compilation, symbol, &schema)?;
        let mut permissions = Vec::new();
        for supplied in accepted.terminal_authority_permissions() {
            if supplied.service_schema() != digest {
                return Err(rejected(
                    "permission names a different accepted service schema",
                ));
            }
            let matches = schema
                .methods
                .iter()
                .zip(&requirements)
                .filter(|(method, _)| {
                    method.requirement_identity == supplied.requirement_identity()
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
                requirement_symbol,
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

pub(super) fn rejected(reason: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(format!(
        "terminal permission review rejects {reason}"
    ))]
}

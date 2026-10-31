//! Retain source calling-owner judgments without granting or choosing supply.
//!
//! Surface projection legitimately includes ordinary inherited traits. The
//! later physical calling request still requires the original boundary owner,
//! exact purpose/package occurrence and unique declared signature. Record these
//! refusals now; only that request consumes them, never census preparation.

use super::ServiceRequirementSource;
use crate::provider_declarations::ProviderPlanningContext;
use diagnostics::Diagnostic;
use language_semantics::service_surface::{ServiceMethod, ServiceSurface};

pub(super) fn prepare(
    context: ProviderPlanningContext<'_>,
    surface: &ServiceSurface,
    method: &ServiceMethod,
    source: ServiceRequirementSource,
) -> Result<(), Diagnostic> {
    let program = context.typed;
    let schema_name = surface.identity_text(surface.trait_name);
    let roots = program
        .traits()
        .iter()
        .filter(|declaration| {
            declaration.is_boundary
                && context.contains(declaration.symbol)
                && program.trait_declaration_path(declaration) == schema_name
                && program.symbols.symbol_package_identity(declaration.symbol)
                    == surface.trait_package_identity
        })
        .count();
    if roots != 1 {
        return Err(Diagnostic::error(format!(
            "selected source boundary entry schema `{schema_name}` resolves to {roots} exact typed boundary traits"
        )));
    }
    let owners = program
        .traits()
        .iter()
        .filter(|declaration| {
            context.contains(declaration.symbol)
                && program.trait_declaration_path(declaration) == method.requirement_owner
                && program.symbols.symbol_package_identity(declaration.symbol)
                    == method.requirement_owner_package_identity
        })
        .collect::<Vec<_>>();
    let [owner] = owners.as_slice() else {
        return Err(Diagnostic::error(format!(
            "selected source boundary entry requirement owner `{}` resolves to {} exact typed traits",
            method.requirement_owner,
            owners.len()
        )));
    };
    if !owner.is_boundary {
        return Err(Diagnostic::error(format!(
            "selected source boundary entry requirement owner `{}` is not an exact boundary trait",
            method.requirement_owner
        )));
    }
    let requirement_identity = surface.identity_text(method.requirement_identity);
    let signatures = program
        .trait_machine_signatures(owner)
        .iter()
        .filter(|signature| {
            signature.name.as_str() == method.name
                && program.trait_requirement_callable_identity_text(owner, signature)
                    == requirement_identity
        })
        .collect::<Vec<_>>();
    let [signature] = signatures.as_slice() else {
        return Err(Diagnostic::error(format!(
            "selected source boundary entry requirement `{}` binds {} exact typed signatures for `{}` / `{requirement_identity}`",
            method.requirement_owner,
            signatures.len(),
            method.name
        )));
    };
    if owner.symbol != source.declaring_trait || signature.symbol != source.symbol {
        return Err(Diagnostic::error(
            "service calling row substituted its exact declaring owner or signature",
        ));
    }
    Ok(())
}

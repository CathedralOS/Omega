//! The published Psi and source-issued declarations that belong to one deployment.

use crate::{PublishedPsi, source_state};
use checked_program_to_psi::provider_supply;
use diagnostics::Diagnostic;
use std::sync::Arc;

/// Source choices for one deployment, judged by the same stage that published the Psi.
pub struct PublishedTargetProgram {
    pub(crate) subject: Option<semantic_vocabulary::PackageKeyIdentity>,
    pub(crate) artifact: PublishedPsi,
    pub(crate) providers: provider_supply::PublishedTargetProviders,
    pub(crate) exclusions: Vec<behavior_exclusion_requests::AuthoredBehaviorExclusion>,
    pub(crate) runtime_declarations: RuntimePermissionDeclarations,
}

/// The checked declarations used to publish this particular deployment.
#[derive(Clone)]
pub struct RuntimePermissionDeclarations {
    pub(crate) source: Arc<source_state::FrozenTypedTrees>,
    pub(crate) has_root_grants: bool,
    pub(crate) has_assumed_bindings: bool,
}

impl RuntimePermissionDeclarations {
    pub fn requirement_path(
        &self,
        package: semantic_vocabulary::PackageKeyIdentity,
        service_path: &str,
        callable: &str,
    ) -> Result<String, Vec<Diagnostic>> {
        let source = &self.source;
        let symbols = &source.symbols;
        let mut services = source.traits().iter().filter(|service| {
            service.is_boundary
                && symbols.symbol_package_identity(service.symbol) == Some(package)
                && symbols.display_path(service.symbol, "::") == service_path
        });
        let (Some(service), None) = (services.next(), services.next()) else {
            return Err(vec![Diagnostic::error(format!(
                "runtime permission has no unique checked service declaration for `{service_path}`"
            ))]);
        };
        let signatures = source.trait_machine_signatures(service);
        let mut methods = signatures.iter().filter(|method| {
            symbols.symbol_package_identity(method.symbol) == Some(package)
                && source.trait_requirement_callable_identity_text(service, method) == callable
        });
        let (Some(method), None) = (methods.next(), methods.next()) else {
            return Err(vec![Diagnostic::error(format!(
                "runtime permission has no unique checked method `{callable}` on `{service_path}`"
            ))]);
        };
        Ok(symbols.display_path(method.symbol, "::"))
    }
}

impl PublishedTargetProgram {
    pub fn subject(&self) -> Option<semantic_vocabulary::PackageKeyIdentity> {
        self.subject
    }

    pub fn runtime_permission_declarations(
        &self,
    ) -> Result<RuntimePermissionDeclarations, Vec<Diagnostic>> {
        if self.runtime_declarations.has_root_grants {
            return Err(vec![Diagnostic::error(
                "the accepted policy schema cannot compare this Build's authored root grants",
            )]);
        }
        if self.runtime_declarations.has_assumed_bindings {
            return Err(vec![Diagnostic::error(
                "the checked Build's assumed binding lacks its canonical review request",
            )]);
        }
        Ok(self.runtime_declarations.clone())
    }

    pub fn into_parts(
        self,
    ) -> (
        PublishedPsi,
        provider_supply::PublishedTargetProviders,
        Vec<behavior_exclusion_requests::AuthoredBehaviorExclusion>,
    ) {
        (self.artifact, self.providers, self.exclusions)
    }
}

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
    pub(crate) exclusions: Vec<build_model::behavior_exclusions::AuthoredBehaviorExclusion>,
    pub(crate) runtime_declarations: RuntimePermissionDeclarations,
}

/// The checked declarations used to publish this particular deployment.
#[derive(Clone)]
pub struct RuntimePermissionDeclarations {
    pub(crate) source: Arc<source_state::FrozenTypedTrees>,
    pub(crate) root_grants: Arc<[symbols::root_grants::AuthoredRootGrant]>,
}

impl RuntimePermissionDeclarations {
    /// Only the root's own checked Build may authorize a provider for the locked product.
    pub fn root_provider_grants(
        &self,
        root: semantic_vocabulary::PackageKeyIdentity,
    ) -> Result<&[symbols::root_grants::AuthoredRootGrant], Vec<Diagnostic>> {
        for grant in self.root_grants.iter() {
            if self
                .source
                .symbols
                .symbol_package_identity(grant.selecting_machine)
                != Some(root)
                || grant.subject_package != Some(root)
            {
                return Err(vec![Diagnostic::error(
                    "a locked root Build cannot grant a dependency's claims through its own source",
                ).with_source_span(grant.source_span)]);
            }
        }
        Ok(&self.root_grants)
    }

    pub fn checked_source(&self) -> &source_state::TypedTrees {
        &self.source
    }
}

impl PublishedTargetProgram {
    pub fn independent_packages(
        &self,
    ) -> Result<std::collections::BTreeSet<semantic_vocabulary::PackageKeyIdentity>, String> {
        self.providers.independent_packages()
    }

    pub fn subject(&self) -> Option<semantic_vocabulary::PackageKeyIdentity> {
        self.subject
    }

    pub fn runtime_permission_declarations(&self) -> &RuntimePermissionDeclarations {
        &self.runtime_declarations
    }

    pub fn into_parts(
        self,
    ) -> (
        PublishedPsi,
        provider_supply::PublishedTargetProviders,
        Vec<build_model::behavior_exclusions::AuthoredBehaviorExclusion>,
    ) {
        (self.artifact, self.providers, self.exclusions)
    }
}

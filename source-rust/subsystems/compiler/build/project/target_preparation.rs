//! Target preparation retains the accepted program's real invocation root.
//! Provider choices are already canonical Psi rows; settlement never reopens
//! source declarations or treats retained provider bodies as separate entries.

use diagnostics::Diagnostic;
use products::CheckedProductSource;
use review_evidence::permission_acceptance::{
    PreparedRuntimePolicy, RuntimePermissionInput, TerminalPermissionAcceptance,
};
use source_compiler::provider_supply::{SourceProviderDemand, SourceProviderFacts};
use std::sync::Arc;

pub(crate) struct PreparedProductTarget {
    pub(crate) profile: Option<target::TargetProfile>,
    pub(crate) demand: SourceProviderFacts<SourceProviderDemand>,
    pub(crate) consent: Option<PreparedRuntimeConsent>,
    pub(crate) advisories: Option<Vec<Diagnostic>>,
}

/// The lock rows and source policy captured for this particular product root.
pub(crate) struct PreparedRuntimeConsent {
    accepted: Arc<TerminalPermissionAcceptance>,
    declarations: PreparedRuntimePolicy,
}

impl PreparedRuntimeConsent {
    pub(crate) fn validate(
        &self,
        checked: &products::CheckedCompilation,
    ) -> Result<(), Vec<Diagnostic>> {
        self.accepted
            .validate(&RuntimePermissionInput::from_compilation(
                &self.declarations,
                checked,
            ))
    }
}

impl PreparedProductTarget {
    pub(crate) fn capture(
        source: &CheckedProductSource,
        profile: Option<target::TargetProfile>,
        permissions: Option<&Arc<TerminalPermissionAcceptance>>,
        report: Option<products::RequestedCompileProduct>,
    ) -> Result<Self, Vec<Diagnostic>> {
        source
            .terminal_publication
            .require_source_owner(&source.program)
            .map_err(|error| vec![error])?;
        let config = &source.computed_build_config.config;
        let demand = source.terminal_publication.prepare_provider_demand(
            profile.map(|profile| profile.target_name()),
            &config.provider_selections,
            &config.native_bindings,
            source.sources.package_identity().is_some(),
        )?;
        // Keep the original declaration census or its refusal. Grant admission
        // reads the prepared result only when this product needs that evidence.
        if !config.grants.is_empty() {
            let _ = source.provider_inventory.trust_declarations();
        }
        let consent = permissions.map(|accepted| PreparedRuntimeConsent {
            accepted: Arc::clone(accepted),
            declarations: PreparedRuntimePolicy::capture(source, profile, &demand),
        });
        Ok(Self {
            profile,
            demand,
            consent,
            advisories: report
                .filter(|kind| {
                    products::ProductReportPlan::reports_advisories(*kind, &source.admission)
                })
                .map(|_| source_advisories(source)),
        })
    }
}

/// The advisory diagnostics of the accepted source this compilation checked
/// (`AcceptedTerminalSource::advisories`), each placed in its source file.
/// They are warnings about a program that was accepted, so a compilation
/// with no retained judgment simply has none.
fn source_advisories(source: &CheckedProductSource) -> Vec<Diagnostic> {
    let Ok(accepted) = source.terminal_publication.accepted() else {
        return Vec::new();
    };
    let mut advisories = accepted.advisories().to_vec();
    if let Some(sources) = accepted.source().symbols.source_map() {
        crate::checked::checking::attribute_diagnostics(&mut advisories, &sources, None);
    }
    advisories
}

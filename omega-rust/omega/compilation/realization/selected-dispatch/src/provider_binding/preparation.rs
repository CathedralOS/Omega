//! Save the declarations needed to choose executable implementations. Target
//! binding can then check grants, calls and callbacks without reading source.

use diagnostics::Diagnostic;
use std::sync::Arc;

pub struct PreparedBindingDeclarations {
    pub(super) owner: Result<(), Vec<Diagnostic>>,
    pub(super) source_owner: std::sync::Weak<checked_trees::CheckedTrees>,
    pub trust: Arc<language_semantics::trust_declarations::TrustDeclarations>,
    pub(super) external:
        Result<Arc<psi_terminal_production::PreparedExternalProviderDeclarations>, Vec<Diagnostic>>,
    pub(super) issuers: Result<Vec<language_semantics::BoundaryQualificationIssuer>, &'static str>,
    pub(super) services: Arc<typed_trees::service_declarations::PreparedServiceCatalog>,
    pub has_proof_obligations: bool,
}

impl PreparedBindingDeclarations {
    /// Capture checks while source is available, but report each saved failure
    /// at its original binding step. An unused declaration must not fail early.
    pub fn capture(
        program: &Arc<checked_trees::CheckedTrees>,
        inventory: &Arc<psi_terminal_production::PreparedProviderInventory>,
        exact_toolchain_sources: &[(source::SourceId, [u8; 32])],
        requires_terminal_checks: bool,
    ) -> Self {
        let owner = if Arc::ptr_eq(inventory.program(), program) {
            Ok(())
        } else {
            Err(vec![Diagnostic::error(
                "selected provider inventory belongs to another checked program",
            )])
        };
        let external = (|| {
            owner.clone()?;
            let external = psi_terminal_production::PreparedExternalProviderDeclarations::prepare(
                Arc::clone(program),
                exact_toolchain_sources,
            )
            .map_err(|error| {
                vec![Diagnostic::error(format!(
                    "external provider declarations: {error:?}"
                ))]
            })?;
            external
                .require_owner(program, exact_toolchain_sources)
                .map_err(|error| {
                    vec![Diagnostic::error(format!(
                        "external provider declarations: {error:?}"
                    ))]
                })?;
            Ok(Arc::new(external))
        })();
        Self {
            owner,
            source_owner: Arc::downgrade(program),
            trust: Arc::new(inventory.trust_declarations().clone()),
            external,
            issuers: program
                .facts
                .semantic
                .admitted_qualification_issuers()
                .map(|issuers| issuers.into_iter().cloned().collect()),
            services: inventory.retained_service_catalog(),
            has_proof_obligations: program.facts.proof_applications.has_obligations
                || requires_terminal_checks,
        }
    }
}

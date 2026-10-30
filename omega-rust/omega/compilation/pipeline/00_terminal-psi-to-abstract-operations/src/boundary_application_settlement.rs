//! Select realizations from canonical Terminal demands and checked candidates.
//! Psi retains conformance and specialization facts; target Build rows select
//! them here. No checked application scope or source replay crosses this edge.

use diagnostics::Diagnostic;

/// Evidence needed to settle boundary applications for one selected target.
/// No compiler coordinator or mutable checking context is retained.
pub struct BoundaryApplicationSettlement<'input> {
    pub selected_provider_plans: &'input effects::SelectedProviderPlanFacts,
    pub selected_provider_provenance:
        Vec<crate::provider_admission::CompilerIntrinsicPlanProvenance<'input>>,
    pub calling_applications: Vec<calling_conventions::BoundaryCallingApplication>,
}

impl BoundaryApplicationSettlement<'_> {
    /// Rejoin the artifact-bound occurrence roster to exact selected realizations.
    /// Demand, realization and opaque-edge errors retain that evaluation order.
    pub fn settle(
        &self,
        artifact: &terminal_codec::CanonicalTerminalArtifact,
        selected: &effects::SelectedProviderPlanFacts,
    ) -> Result<boundary_applications::TerminalBoundaryApplicationCoverage, Vec<Diagnostic>> {
        let demands = boundary_applications::TerminalBoundaryApplicationDemands::new(
            artifact.manifest().semantic(),
            artifact
                .semantic_module()
                .boundary_application_demands
                .clone(),
        )
        .map_err(|message| vec![Diagnostic::error(message)])?;
        let realizations = provider_selection::select_boundary_applications(
            &self.selected_provider_provenance,
            selected,
            artifact.semantic_module(),
            &demands,
        )?;
        let opaque_applications =
            project_opaque_applications(&self.calling_applications, artifact.semantic_module())
                .map_err(|message| vec![Diagnostic::error(message)])?;
        boundary_applications::TerminalBoundaryApplicationCoverage::new(demands, realizations)
            .map_err(|message| vec![Diagnostic::error(message)])
            .map(|coverage| coverage.with_opaque_applications(opaque_applications))
    }
}

/// Retain one strong application commitment per requirement edge and shape
/// coordinate. Producer and consumer artifacts compare this source-free set;
/// arena identities are deliberately absent.
fn project_opaque_applications(
    calling_applications: &[calling_conventions::BoundaryCallingApplication],
    module: &terminal_psi::TerminalModule,
) -> Result<boundary_applications::BoundaryOpaqueRepresentationApplications, &'static str> {
    let rows = calling_applications
        .iter()
        .flat_map(|application| {
            application.opaque_applications.iter().map(|use_| {
                module
                    .identities
                    .index_of(&application.requirement_identity)
                    .ok_or(
                        "boundary opaque application names a requirement absent from the module identity table",
                    )
                    .map(|requirement_identity| {
                        boundary_applications::BoundaryOpaqueRepresentationApplication {
                            requirement_identity,
                            shape_root: use_.shape_root,
                            application_report_fingerprint: use_.application_report_fingerprint,
                            selected_application_commitment: use_
                                .selected_application_commitment,
                        }
                    })
            })
        })
        .collect::<Result<_, _>>()?;
    boundary_applications::BoundaryOpaqueRepresentationApplications::new(rows)
}

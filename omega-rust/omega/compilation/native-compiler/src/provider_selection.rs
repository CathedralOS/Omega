//! Select the native providers for one published program and target.
use backend_plan::PublishedProviderDemand;
use diagnostics::Diagnostic;
pub use native_program_planning::{
    SelectedExecution, SelectedProviderRealization, SelectedProviderResult,
};

pub fn select_published_providers(
    demand: PublishedProviderDemand,
) -> Result<SelectedExecution, Vec<Diagnostic>> {
    use native_program_planning::{
        NativeProgramPlanningError, target_supply::TargetSupplyRealizationError,
    };
    let (profile, logical, bodies, labels, origins, review, folded_entry) = match demand {
        PublishedProviderDemand::Unscoped { artifact, roots } => {
            return native_program_planning::plan_unscoped_execution(
                artifact.semantic_module(),
                &roots,
            );
        }
        PublishedProviderDemand::Target {
            profile,
            logical,
            bodies,
            labels,
            origins,
            review,
            folded_entry,
        } => (
            profile,
            logical,
            bodies,
            labels,
            origins,
            review,
            folded_entry,
        ),
    };
    let artifact = std::sync::Arc::clone(&origins.artifact);
    let module = artifact.semantic_module();
    native_program_planning::plan_published_target_execution(origins, profile, &logical, &bodies, review, folded_entry)
        .map_err(|error| match error {
            NativeProgramPlanningError::Supply(TargetSupplyRealizationError::MissingSupplies { target, boundaries }) => {
                boundaries.iter().map(|boundary| {
                    let message = match labels.binary_search_by_key(boundary, |(key, _)| *key) {
                        Ok(position) => format!(
                            "boundary requirement `{}` has no provider on target `{target}`: the Build selects none and the target package declares no default",
                            labels[position].1,
                        ),
                        Err(_) => format!(
                            "reached boundary {} has no normalized chosen provider or exact native leaf on target `{target}`",
                            boundary.get(),
                        ),
                    };
                    Diagnostic::error(message)
                }).collect()
            }
            NativeProgramPlanningError::Supply(error) => vec![Diagnostic::error(error.diagnostic(module))],
            NativeProgramPlanningError::Admission(diagnostics) => diagnostics,
        })
}

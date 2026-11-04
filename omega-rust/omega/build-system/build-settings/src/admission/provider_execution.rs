//! Retain selected Build implementations through code production and reuse.
//!
//! Psi's checked program stays target-neutral. These Omega selections live
//! beside it, participate in the complete Pure key, and are admitted against
//! the actual Terminal artifact before its existing executor starts.

use diagnostics::Diagnostic;

#[derive(Clone, PartialEq, Eq, Default)]
pub(crate) struct BuildProviderPlans {
    selected: effects::SelectedProviderPlanFacts,
    declarations: Vec<backend_plan::TerminalTargetProviderPlan>,
}

pub(crate) struct BuildProviderInstallation {
    pub(crate) checked: terminal_installation::AdmittedProviderSelections,
    pub(crate) intrinsics: provider_admission::provider_admission::AdmittedTerminalIntrinsics,
}

impl BuildProviderPlans {
    pub(crate) fn capture(
        checked: &checked_trees::CheckedTrees,
        selected: effects::SelectedProviderPlanFacts,
        provenance: &[provider_planning::SelectedProviderReviewProvenance],
    ) -> Result<Self, Vec<Diagnostic>> {
        let toolchain =
            source::source_identity::toolchain_source_identities(checked.symbols.source_files())
                .map_err(|reason| vec![Diagnostic::error(reason)])?;
        let declarations =
            provider_planning::provider_declarations::retain_selected_provider_declarations(
                provider_planning::ProviderPlanningContext::new(
                    checked,
                    source::DependencyScope::Build,
                ),
                &toolchain,
                &selected,
                provenance,
            )?;
        Ok(Self {
            selected,
            declarations,
        })
    }

    pub(crate) fn admit(
        &self,
        artifact: terminal_codec::TerminalExecutionView<'_>,
    ) -> Result<BuildProviderInstallation, Vec<Diagnostic>> {
        use provider_admission::provider_admission as admission;
        let module = artifact.module();
        admission::validate_external_provider_owners(
            &module.external_providers,
            &module.identities,
            &self.declarations,
        )
        .map_err(failure)?;
        let adapters =
            admission::retain_selected_provider_adapters(&self.selected, &self.declarations)
                .map_err(failure)?;
        let checked =
            provider_admission::provider_installation::admit_terminal_provider_installation(
                artifact, &adapters,
            )
            .map_err(|reason| {
                failure(format!("Build selected provider installation: {reason:?}"))
            })?;
        let program = terminal_interpreter::TerminalProgram::borrow(artifact);
        let supplies = program
            .admit_runtime_supplies(terminal_interpreter::TerminalRuntimeSupplyRequest {
                installation: Some(&checked),
                native_code_layouts: None,
                host_imports: &[],
                entry: None,
                x86_flags: None,
                memory_ordering: None,
                target_machines: None,
                target_layouts: None,
            })
            .map_err(|reason| failure(format!("Build selected provider supplies: {reason:?}")))?;
        let demand = program
            .execution_requirements(Some(&supplies))
            .map_err(|reason| {
                failure(format!(
                    "Build selected provider execution closure: {reason:?}"
                ))
            })?;
        let demanded = demand
            .imports
            .iter()
            .filter_map(|slot| {
                terminal_psi::find_boundary_machine(&module.boundary_machines, *slot)
                    .and_then(|boundary| module.identities.text(boundary.identity))
                    .map(str::to_owned)
            })
            .collect();
        // Build has no admitted host/device review identities. Numeric rows
        // close their own sealed meaning through the exact external catalog;
        // a demanded nonnumeric intrinsic remains explicitly unsettled.
        let unreviewed = self
            .selected
            .plans()
            .iter()
            .map(|plan| vec![None; plan.rows.len()])
            .collect::<Vec<_>>();
        let intrinsics = admission::admit_terminal_intrinsics(
            artifact,
            &self.selected,
            self.declarations
                .iter()
                .zip(&unreviewed)
                .map(
                    |(declaration, executions)| admission::CompilerIntrinsicPlanProvenance {
                        plan: &declaration.plan,
                        semantic_schema: &declaration.plan.schema,
                        row_executions: executions,
                        row_numeric_realizations: &declaration.row_numeric_realizations,
                    },
                ),
            &demanded,
        )?;
        Ok(BuildProviderInstallation {
            checked,
            intrinsics,
        })
    }
}

fn failure(reason: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::error(reason)]
}

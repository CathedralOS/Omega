/*
Retain source-issued Build provider declarations and selected plans beside the
accepted module. Before the Terminal interpreter starts, admit their actual
adapters, numeric intrinsics and runtime supplies against that exact execution
view. Source selection alone supplies no host/device authority.
*/

use diagnostics::Diagnostic;

#[derive(Clone, PartialEq, Eq, Default)]
pub(crate) struct BuildProviderPlans {
    selected: effects::SelectedProviderPlanFacts,
    declarations: Vec<backend_plan::TerminalTargetProviderPlan>,
    packages: Option<package_compilation::PackageCompilationInputs>,
}

pub(crate) struct BuildProviderInstallation {
    pub(crate) checked: terminal_installation::AdmittedProviderSelections,
    pub(crate) intrinsics: provider_admission::provider_admission::AdmittedTerminalIntrinsics,
}

impl BuildProviderPlans {
    pub(crate) fn for_packages(
        packages: Option<&package_compilation::PackageCompilationInputs>,
    ) -> Self {
        Self {
            packages: packages.cloned(),
            ..Self::default()
        }
    }

    pub(crate) fn admit(
        &self,
        artifact: terminal_codec::TerminalExecutionView<'_>,
    ) -> Result<BuildProviderInstallation, Vec<Diagnostic>> {
        use provider_admission::provider_admission as admission;
        let module = artifact.module();
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
        if let Some(packages) = &self.packages {
            let entry = module
                .entry
                .ok_or_else(|| failure("Build declaration admission has no entry"))?;
            terminal_installation::admit_declaration_selections(
                artifact,
                None,
                entry,
                &checked,
                semantic_vocabulary::compiler_evaluation::DeclarationCustody::Package(
                    packages.root(),
                ),
                packages,
            )
            .map_err(failure)?;
        }
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
        // A selection is complete only against what the module actually
        // calls. A provider the module demands and no plan supplies would
        // otherwise surface as a failed call after authored code has run.
        if !demand.missing_providers.is_empty() {
            return Err(failure(format!(
                "Build program demands providers no selected plan supplies: {:?}",
                demand.missing_providers
            )));
        }
        let demanded = demand
            .imports
            .iter()
            .map(|slot| {
                let boundary =
                    terminal_psi::find_boundary_machine(&module.boundary_machines, *slot)
                        .ok_or_else(|| {
                            failure(format!("Build import {slot:?} has no boundary declaration"))
                        })?;
                module
                    .identities
                    .text(boundary.identity)
                    .filter(|identity| !identity.is_empty())
                    .map(str::to_owned)
                    .ok_or_else(|| {
                        failure(format!("Build import {slot:?} has no callable identity"))
                    })
            })
            .collect::<Result<std::collections::BTreeSet<_>, _>>()?;
        let unreviewed = self
            .selected
            .plans()
            .iter()
            .map(|plan| vec![None; plan.rows.len()])
            .collect::<Vec<_>>();
        let intrinsics = admission::admit_terminal_intrinsics(
            artifact,
            &self.selected,
            &self.declarations,
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

/*
Retain source-issued Build provider declarations and selected plans beside the
accepted module. Before the Terminal interpreter starts, admit their actual
adapters, numeric intrinsics and runtime supplies against that exact execution
view. Source selection alone supplies no host/device authority.
*/

use diagnostics::Diagnostic;

#[derive(Clone, PartialEq, Eq, Default)]
pub(crate) struct BuildProviderPlans {
    selected: selected_provider_plans::SelectedProviderPlanFacts,
    packages: Option<package_compilation::PackageCompilationInputs>,
    supplied_scope: Option<declaration_scope::PackageDeclarationScope>,
    dependency_edges: Vec<semantic_vocabulary::dependency_reach::DependencyServiceEdge>,
}

pub(crate) struct BuildProviderInstallation {
    pub(crate) checked: psi_provider_selections::AdmittedProviderSelections,
    pub(crate) intrinsics: numeric_intrinsic_admission::AdmittedTerminalIntrinsics,
}

impl BuildProviderPlans {
    pub(crate) fn for_packages(
        packages: Option<&package_compilation::PackageCompilationInputs>,
        libraries: &[package_compilation::source_imports::SuppliedBuildLibrary],
    ) -> Self {
        use build_declarations::DependencyPurpose;
        let mut dependency_edges = packages
            .map(|inputs| inputs.execution_service_edges(DependencyPurpose::Build))
            .unwrap_or_default();
        for library in libraries {
            let dependency = library.dependency();
            dependency_edges.push(
                semantic_vocabulary::dependency_reach::DependencyServiceEdge {
                    requester: packages.map(|inputs| inputs.root()),
                    package: dependency.inputs.root(),
                    alias: dependency.alias.clone(),
                    max_reach: library.max_reach().clone(),
                },
            );
            dependency_edges.extend(
                dependency
                    .inputs
                    .execution_service_edges(DependencyPurpose::Product),
            );
        }
        Self {
            packages: packages.cloned(),
            dependency_edges,
            supplied_scope:
                package_compilation::source_imports::SuppliedBuildLibrary::declaration_scope(
                    libraries, None,
                ),
            ..Self::default()
        }
    }

    pub(crate) fn selection_scope(
        &self,
        purpose: build_declarations::DependencyPurpose,
    ) -> Option<declaration_scope::PackageDeclarationScope> {
        self.packages
            .as_ref()
            .map(|packages| packages.declaration_scope(purpose))
            .or_else(|| self.supplied_scope.clone())
    }

    pub(crate) fn admit(
        &self,
        program: &source_compiler::publication::ProducedTerminalProgram,
    ) -> Result<BuildProviderInstallation, Vec<Diagnostic>> {
        use provider_admission as admission;
        let artifact = program.view();
        let module = artifact.module();
        let adapters =
            admission::retain_selected_provider_adapters(&self.selected).map_err(failure)?;
        let checked =
            provider_admission::provider_installation::admit_terminal_provider_installation(
                artifact, &adapters,
            )
            .map_err(|reason| {
                failure(format!("Build selected provider installation: {reason:?}"))
            })?;
        let entry = module
            .entry
            .ok_or_else(|| failure("Build declaration admission has no entry"))?;
        let implementations = checked
            .providers()
            .chain(
                checked
                    .assumed_bindings()
                    .iter()
                    .map(|row| (row.required, row.implementation)),
            )
            .map(|(boundary, machine)| {
                terminal_semantics::callable_values::CallableBoundaryImplementation {
                    boundary,
                    machine,
                }
            })
            .collect::<Vec<_>>();
        let selections = program
            .source_selections()
            .and_then(|selections| selections.questions_for(artifact))
            .ok_or_else(|| failure("Build lost the source choices for its executable"))?;
        psi_provider_selections::dependency_reach::check(
            module,
            entry,
            &implementations,
            None,
            &self.dependency_edges,
            &selections.dependency_calls,
        )?;
        let authority = self.selection_scope(build_declarations::DependencyPurpose::Build);
        let pending = evaluation_admission::admit_declaration_selections(
            artifact,
            None,
            entry,
            &checked,
            self.packages
                .as_ref()
                .map(package_compilation::PackageCompilationInputs::root),
            authority
                .as_ref()
                .map(|a| a as &dyn declaration_scope::DeclarationSelectionAuthority),
        )
        .map_err(failure)?;
        program
            .source_selections()
            .ok_or_else(|| failure("Build lost its source-issued declarations"))?
            .admit_captured_execution(artifact, pending.entry, &pending.calls)
            .map_err(failure)?;
        let program = terminal_interpreter::TerminalProgram::borrow(artifact);
        let supplies = program
            .admit_runtime_supplies(terminal_interpreter::TerminalRuntimeSupplyRequest {
                installation: Some(&checked),
                native_code_layouts: None,
                host_imports: &[],
                external_completions: &[],
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
                let boundary = terminal_psi::find_boundary_machine(
                    &module.interfaces.boundary_machines,
                    *slot,
                )
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
        let intrinsics = numeric_intrinsic_admission::admit_terminal_intrinsics(
            artifact,
            &self.selected,
            self.selected
                .plans()
                .iter()
                .zip(&unreviewed)
                .map(|(declaration, executions)| {
                    numeric_intrinsic_admission::CompilerIntrinsicPlanProvenance {
                        plan: declaration,
                        semantic_schema: &declaration.schema,
                        row_executions: executions,
                    }
                }),
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

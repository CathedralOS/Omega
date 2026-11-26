//! Admit Build's published provider choices and reachable imports before entry.
//! Package authority comes from the supplied Build graph. Selecting a provider
//! cannot grant that graph a host operation.

use diagnostics::Diagnostic;

#[derive(Clone, PartialEq, Eq, Default)]
pub(crate) struct BuildProviderScope {
    packages: Option<package_compilation::PackageCompilationInputs>,
    supplied_scope: Option<declaration_scope::PackageDeclarationScope>,
    supplied_edges: Vec<(semantic_vocabulary::PackageKeyIdentity, String)>,
}

pub(crate) struct BuildProviderInstallation {
    pub(crate) checked: psi_provider_selections::AdmittedProviderSelections,
    pub(crate) intrinsics: numeric_intrinsic_admission::AdmittedTerminalIntrinsics,
}

impl BuildProviderScope {
    pub(crate) fn for_packages(
        packages: Option<&package_compilation::PackageCompilationInputs>,
        libraries: &[package_compilation::source_imports::SuppliedBuildLibrary],
    ) -> Self {
        Self {
            packages: packages.cloned(),
            supplied_edges: libraries
                .iter()
                .map(|library| {
                    let dependency = library.dependency();
                    (dependency.inputs.root(), dependency.alias.clone())
                })
                .collect(),
            supplied_scope:
                package_compilation::source_imports::SuppliedBuildLibrary::declaration_scope(
                    libraries, None,
                ),
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
        let artifact = program.view();
        let module = artifact.module();
        let entry = module
            .entry
            .ok_or_else(|| failure("Build declaration admission has no entry"))?;
        let checked =
            psi_provider_selections::logical_reach::admit_interpreted_providers(artifact)?;
        super::library_reach::check(module, entry, &checked, &self.supplied_edges)?;
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
            return Err(demand
                .missing_providers
                .iter()
                .map(|slot| {
                    let requirement = module
                        .boundary_machine(*slot)
                        .and_then(|declaration| module.identities.text(declaration.identity));
                    Diagnostic::error(format!(
                        "Build requires `{}` before entry; no interpreter provider was supplied",
                        requirement.unwrap_or("<unknown boundary>"),
                    ))
                })
                .collect());
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
        let intrinsics = numeric_intrinsic_admission::admit_terminal_intrinsics(
            artifact,
            &selected_provider_plans::SelectedProviderPlanFacts::default(),
            std::iter::empty(),
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

//! Admit Build's published provider choices and reachable imports before entry.
//! Package authority comes from the supplied Build graph. Selecting a provider
//! cannot grant that graph a host operation.

use diagnostics::Diagnostic;

#[derive(Clone, PartialEq, Eq, Default)]
pub(crate) struct BuildProviderScope {
    owner: Option<semantic_vocabulary::PackageKeyIdentity>,
    packages: Option<package_compilation::PackageCompilationInputs>,
    supplied_scope: Option<declaration_scope::PackageDeclarationScope>,
    dependency_edges: Vec<semantic_vocabulary::dependency_reach::DependencyServiceEdge>,
}

pub(crate) struct BuildProviderInstallation {
    pub(crate) imports: Vec<semantic_vocabulary::BoundaryMachineId>,
    pub(crate) diagnostic_output_bytes: Option<u64>,
    pub(crate) supplies: terminal_interpreter::TerminalRuntimeSupplies,
    pub(crate) awaited: Vec<semantic_vocabulary::BoundaryMachineId>,
}

impl BuildProviderScope {
    pub(crate) fn for_packages(
        packages: Option<&package_compilation::PackageCompilationInputs>,
        libraries: &[package_compilation::source_imports::SuppliedBuildLibrary],
        owner: Option<semantic_vocabulary::PackageKeyIdentity>,
    ) -> Self {
        use build_declarations::DependencyPurpose;
        let mut dependency_edges = packages
            .map(|inputs| inputs.execution_service_edges(DependencyPurpose::Build))
            .unwrap_or_default();
        for library in libraries {
            let dependency = library.dependency();
            dependency_edges.push(
                semantic_vocabulary::dependency_reach::DependencyServiceEdge {
                    requester: owner,
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
            owner,
            packages: packages.cloned(),
            dependency_edges,
            supplied_scope:
                package_compilation::source_imports::SuppliedBuildLibrary::declaration_scope(
                    libraries, owner,
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
        diagnostic_output_bytes: Option<u64>,
        filesystem_scope: &package_compilation::BuildMachineFilesystemScope,
    ) -> Result<BuildProviderInstallation, Vec<Diagnostic>> {
        let artifact = program.view();
        let module = artifact.module();
        let entry = module
            .entry
            .ok_or_else(|| failure("Build declaration admission has no entry"))?;
        let checked =
            psi_provider_selections::logical_reach::admit_interpreted_providers(artifact)?;
        let implementations = checked
            .providers()

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
            self.owner,
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
        let requests = program
            .build_requests()
            .calls_for(artifact)
            .ok_or_else(|| failure("Build requests lost their artifact custody"))?;
        let bindings = program
            .build_bindings()
            .calls_for(artifact)
            .ok_or_else(|| failure("Build bindings lost their artifact custody"))?;
        let queries = program
            .build_bindings()
            .queries_for(artifact)
            .ok_or_else(|| failure("Build queries lost their artifact custody"))?;
        let filesystem = program
            .build_filesystem()
            .calls_for(artifact)
            .ok_or_else(|| failure("Build filesystem calls lost their artifact custody"))?;
        let mut awaited = Vec::new();
        for call in bindings.iter() {
            if let source_compiler::representations::declaration_selection::BuildSelectionRequest::ProviderReadiness(request) = &call.request {
                awaited.extend_from_slice(&request.requirement.boundaries);
            }
        }
        awaited.sort_unstable();
        awaited.dedup();

        let mut host_imports = requests
            .iter()
            .map(|call| call.site.boundary)
            .chain(bindings.iter().map(|call| call.site.boundary))
            .chain(queries.iter().map(|call| call.site.boundary))
            .chain(filesystem.iter().filter_map(|call| {
                let boundary = call.site.boundary;
                let optional = call.action == source_compiler::publication::TerminalBuildFilesystemOperation::ReadAll;
                let selected = implementations.iter().any(|provider| provider.boundary == boundary)
                    || awaited.binary_search(&boundary).is_ok();
                (!optional || !selected).then_some(boundary)
            }))
            .collect::<Vec<_>>();
        host_imports.sort_unstable();
        host_imports.dedup();

        let program = terminal_interpreter::TerminalProgram::borrow(artifact);
        let mut supplies = program
            .admit_runtime_supplies(terminal_interpreter::TerminalRuntimeSupplyRequest {
                installation: Some(&checked),
                native_code_layouts: None,
                host_imports: &host_imports,
                external_completions: &[],
                entry: None,
                x86_flags: None,
                memory_ordering: None,
                target_machines: None,
                target_layouts: None,
            })
            .map_err(|reason| failure(format!("Build selected provider supplies: {reason:?}")))?;
        for &boundary in &awaited {
            supplies = program
                .defer_component(supplies, boundary)
                .map_err(|error| failure(format!("Build awaited provider: {error:?}")))?;
        }
        let demand = program
            .execution_requirements(Some(&supplies))
            .map_err(|reason| {
                failure(format!(
                    "Build selected provider execution closure: {reason:?}"
                ))
            })?;
        use source_compiler::publication::TerminalBuildRequestOperation as Action;
        if diagnostic_output_bytes.is_none()
            && let Some(call) = requests.iter().find(|row| {
                row.action == Action::SandboxWrite && demand.imports.contains(&row.site.boundary)
            })
        {
            let name = module
                .boundary_machine(call.site.boundary)
                .and_then(|decl| module.identities.text(decl.identity))
                .unwrap_or("<unknown boundary>");
            return Err(failure(format!(
                "Build requires `{name}` before entry; the caller supplied no bounded StandardOutput diagnostic sink"
            )));
        }
        if !filesystem_scope.has_captured_files()
            && filesystem.iter().any(|call| {
                call.action
                    == source_compiler::publication::TerminalBuildFilesystemOperation::ReadAll
                    && demand.imports.contains(&call.site.boundary)
            })
        {
            return Err(failure(
                "Build requires Filesystem::read_all before entry, but the caller supplied no captured filesystem",
            ));
        }
        let missing = demand
            .missing_providers
            .iter()
            .chain(demand.imports.iter().filter(|slot| {
                host_imports.binary_search(slot).is_err() && awaited.binary_search(slot).is_err()
            }));
        let errors: Vec<_> = missing.map(|slot| {
            let requirement = module.boundary_machine(*slot)
                .and_then(|declaration| module.identities.text(declaration.identity));
            Diagnostic::error(format!(
                "Build requires `{}` before entry; no interpreter provider or host binding was supplied",
                requirement.unwrap_or("<unknown boundary>"),
            ))
        }).collect();
        if !errors.is_empty() {
            return Err(errors);
        }
        Ok(BuildProviderInstallation {
            imports: demand.imports,
            supplies,
            awaited,
            diagnostic_output_bytes,
        })
    }
}

fn failure(reason: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::error(reason)]
}

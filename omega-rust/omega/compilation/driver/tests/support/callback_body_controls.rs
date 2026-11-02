//! Compare canonical body headers and callback roots with the checked Source
//! oracle, then repeat selection after releasing Source and decoding Artifact.

use effects::provider_plan::ProviderBinding;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Header {
    callable: Option<String>,
    package: Option<semantic_vocabulary::PackageKeyIdentity>,
    target: Option<String>,
    eligible: bool,
    body: bool,
}

pub(super) struct PreparedCallbackBodies {
    headers: Vec<Header>,
    unavailable_bodies: Vec<Header>,
    selected: effects::SelectedProviderPlanFacts,
    providers: std::sync::Arc<[backend_plan::TerminalTargetProviderPlan]>,
    declarations: terminal_psi::CallbackDeclarationRoster,
    calling: Vec<terminal_psi::CallbackRequirementId>,
    roots: Vec<terminal_psi::CallbackUseId>,
}

pub(super) fn prepare(
    checked: &compiler::CheckedCompilation,
) -> Result<PreparedCallbackBodies, String> {
    let execution = checked.execution();
    let program = execution.provider_source.program();
    let typed = &program.typed;
    let mut headers = Vec::new();
    let mut unavailable_bodies = Vec::new();
    for machine in typed.machines() {
        let callable = typed.machine_callable_identity(machine);
        let header = Header {
            callable: callable
                .is_valid()
                .then(|| typed.callable_identity_text(callable)),
            package: typed.symbols.symbol_package_identity(machine.symbol),
            target: typed.source_target_context(machine).map(str::to_owned),
            eligible: !machine.target_family_interface
                && typed.symbols.symbol_is_in_dependency_scope(
                    machine.symbol,
                    source::DependencyScope::Product,
                ),
            body: machine.supplies_checked_body(),
        };
        if header.body
            && program
                .facts
                .declaration_descriptions
                .for_machine(machine.symbol)
                .is_err()
        {
            unavailable_bodies.push(header.clone());
        }
        headers.push(header);
    }
    headers.sort();
    let declarations = execution.external_providers.callbacks.declarations.clone();
    let calling = execution.external_providers.callbacks.calling_requirements(
        checked
            .providers()
            .boundary_calling_plan_realizations
            .iter()
            .filter(|realization| realization.boundary_arguments.is_empty())
            .map(|realization| (realization.boundary_trait, realization.requirement_machine)),
    );
    let selected = execution.provider_source.plans().clone();
    let roots = if declarations.uses.is_empty() {
        Vec::new()
    } else {
        // Independent old reference route: preserve the original exact adapter
        // lookup and Source instance expansion, not the new canonical predicate.
        let mut rows = Vec::new();
        for plan in selected.plans() {
            for row in &plan.rows {
                if !matches!(row.binding, ProviderBinding::CheckedAdapter { .. }) {
                    continue;
                }
                rows.push(provider_selection::SelectedCallbackRow {
                    plan,
                    row,
                    checked_adapter_body: provider_planning::exact_checked_adapter(
                        provider_planning::ProviderPlanningContext::new(
                            typed,
                            source::DependencyScope::Product,
                        ),
                        plan,
                        row,
                    )
                    .map(|machine| machine.supplies_checked_body())
                    .map_err(|error| format!("{error:?}")),
                });
            }
        }
        let instances = execution
            .provider_source
            .project_instances()
            .map_err(|errors| format!("reference callback instances: {errors:?}"))?;
        let providers = provider_selection::CallbackProviderDeclarations::prepared(instances, rows);
        provider_selection::select_callback_declarations(&declarations, &calling, &providers)
            .map_err(|errors| format!("reference callback selection: {errors:?}"))?
    };
    Ok(PreparedCallbackBodies {
        headers,
        unavailable_bodies,
        selected,
        declarations,
        calling,
        roots,
        providers: std::sync::Arc::clone(&checked.providers().provider_declarations),
    })
}

impl PreparedCallbackBodies {
    fn select(
        &self,
        module: &terminal_psi::TerminalModule,
    ) -> Result<Vec<terminal_psi::CallbackUseId>, String> {
        let providers = provider_selection::CallbackProviderDeclarations::prepare(
            &self.selected,
            &self.declarations,
            &self.providers,
            module,
        )
        .map_err(|errors| format!("canonical callback preparation: {errors:?}"))?;
        provider_selection::select_callback_declarations(
            &self.declarations,
            &self.calling,
            &providers,
        )
        .map_err(|errors| format!("canonical callback selection: {errors:?}"))
    }

    pub(super) fn inspect(&self, module: &terminal_psi::TerminalModule) -> Result<(), String> {
        let mut headers = Vec::new();
        let mut unavailable_bodies = Vec::new();
        let scope = &module.scalar_applications.proof_scope;
        for (_, row) in scope.callable_headers.iter() {
            let header = Header {
                callable: row
                    .callable
                    .as_ref()
                    .ok()
                    .and_then(|identity| module.identities.text(*identity))
                    .map(str::to_owned),
                package: row.package,
                target: row.source_target.clone(),
                eligible: row.product_eligible,
                body: row.supplies_checked_body,
            };
            if header.body
                && matches!(
                    scope.declaration(row.declaration),
                    Some(terminal_psi::ProofScopeDeclaration::Unavailable(_))
                )
            {
                unavailable_bodies.push(header.clone());
            }
            headers.push(header);
        }
        headers.sort();
        if headers != self.headers {
            return Err("canonical callable headers differ from original Source body/package/target/scope facts".into());
        }
        for header in &self.unavailable_bodies {
            let position = unavailable_bodies.iter().position(|candidate| candidate == header)
                .ok_or("a body header acquired an invented full declaration or lost its independent unavailable description")?;
            unavailable_bodies.remove(position);
        }
        if self.select(module)? != self.roots {
            return Err("Source-free callback roots differ from original checked selection".into());
        }
        // A selected checked registrar actually consumed by a nominal use must
        // not silently become foreign/unchecked when its header is omitted.
        for (_, usage) in self.declarations.uses.iter() {
            let declaration = self.declarations.declaration(usage);
            let Ok(language_semantics::callback_declarations::CallbackRegistrationDeclaration::External(identity)) = declaration.registration else { continue; };
            let Some((plan, row)) = self.selected.selected_requirement_row(&identity) else {
                continue;
            };
            let ProviderBinding::CheckedAdapter {
                machine_identity,
                machine_package_identity,
            } = &row.binding
            else {
                continue;
            };
            let mut omitted = module.clone();
            let handles = omitted
                .scalar_applications
                .proof_scope
                .callable_headers
                .iter()
                .filter_map(|(handle, header)| {
                    (header.product_eligible
                        && header.package == *machine_package_identity
                        && header
                            .source_target
                            .as_deref()
                            .is_none_or(|target| target == plan.target)
                        && header.callable.as_ref().is_ok_and(|identity| {
                            module.identities.text(*identity)
                                == Some(plan.identity_text(*machine_identity))
                        }))
                    .then_some(handle)
                })
                .collect::<Vec<_>>();
            let [handle] = handles.as_slice() else {
                return Err("callback omission control needs one exact consumed header".into());
            };
            omitted
                .scalar_applications
                .proof_scope
                .callable_headers
                .free(*handle);
            let optimization =
                terminal_codec::build_identity_optimization_execution_record(&omitted)
                    .map_err(|error| format!("callback-header envelope: {error:?}"))?;
            let omitted =
                terminal_codec::CanonicalTerminalArtifact::from_parts(omitted, optimization, None)
                    .map_err(|error| format!("callback-header artifact: {error:?}"))?;
            let omitted =
                terminal_codec::CanonicalTerminalArtifact::from_bytes(&omitted.to_bytes())
                    .map_err(|error| format!("callback-header ingress: {error:?}"))?;
            match self.select(omitted.semantic_module()) {
                Err(reason) if reason.contains("has no exact canonical checked-body header") => {}
                other => {
                    return Err(format!(
                        "omitted consumed callback header did not refuse: {other:?}"
                    ));
                }
            }
            break;
        }
        Ok(())
    }
}

//! Close authored selections into source-free target proposals.
//!
//! Reuse ordinary plan derivation and whole-provider selection; this is not
//! another defaults engine. Normalized Build declaration identities join the
//! canonical candidate catalog, including independently owned named maps.
//! Canonical candidates retain their original selected-row coordinates beside
//! closed instances. One shared projection enumerates complete rosters both at
//! publication and at receiving admission; a proposal cannot truncate them.
//! Publication exports no source-index companion. No provider display path
//! is a semantic lookup key at this crossing. Unavailable target selection is
//! retained as a refusal, not a reason to invalidate other portable targets.

use crate::{ProviderPlanningContext, ProviderSelection, ProviderSelectionSubject};
use diagnostics::Diagnostic;
use symbols::SymbolHandle;

/// Original interface inventory for whole selections, including inherited
/// members. Terminal production projects these declaration roots onto complete
/// originals and retained concrete applications; it does not invent calls.
pub fn provider_selection_roots<'selection>(
    context: ProviderPlanningContext<'_>,
    selections: impl IntoIterator<Item = &'selection ProviderSelection>,
) -> Result<Vec<SymbolHandle>, Vec<Diagnostic>> {
    let mut roots = Vec::new();
    for selection in selections {
        match &selection.subject {
            ProviderSelectionSubject::BoundaryTrait(identity) => {
                let definition = context
                    .typed
                    .trait_definition_by_symbol(identity.symbol)
                    .ok_or_else(|| {
                        vec![Diagnostic::error(
                            "provider selection lost its exact trait declaration",
                        )]
                    })?;
                let members =
                    checked_trees::typed_trees::static_applications::boundary_interface_members(
                        context.typed,
                        definition.symbol,
                    )
                    .ok_or_else(|| {
                        vec![Diagnostic::error(
                            "provider selection lost its original boundary member roster",
                        )]
                    })?;
                roots.extend(members.into_iter().map(|(_, member)| member));
            }
            ProviderSelectionSubject::BoundaryRequirement(identity) => roots.push(identity.symbol),
            ProviderSelectionSubject::BoundaryOperatorFamily(family) => {
                roots.extend(family.coordinates().iter().map(|row| row.symbol));
            }
        }
    }
    if roots
        .iter()
        .any(|symbol| !symbol.is_valid() || !context.contains(*symbol))
    {
        return Err(vec![Diagnostic::error(
            "provider selection lost its exact product requirement owner",
        )]);
    }
    roots.sort_by_key(|symbol| (symbol.arena_index(), symbol.generation()));
    roots.dedup();
    Ok(roots)
}

/// Immutable checked input and its exact produced artifact. Derivation never
/// reruns Build, checking, specialization or foreign-binding evaluation.
pub struct TargetProviderPublicationRequest<'program> {
    pub checked: &'program checked_trees::CheckedTrees,
    pub artifact: terminal_codec::TerminalArtifactView<'program>,
    pub exact_toolchain_sources: &'program [(source::SourceId, [u8; 32])],
    pub defaults: &'program [crate::TargetProviderDefaults],
    pub overrides: &'program [ProviderSelection],
    pub settlement: &'program crate::ProviderSettlement,
    pub targets: &'program [String],
    pub optimization_selection: optimization_core::OptimizationSelectionIdentity,
}

impl TargetProviderPublicationRequest<'_> {
    fn context(&self) -> ProviderPlanningContext<'_> {
        ProviderPlanningContext::new(self.checked, source::DependencyScope::Product)
    }

    /// Produce every requested target row, preserving a named failure for an
    /// unavailable realization. No row authorizes native or host execution.
    pub fn publish(&self) -> Vec<backend_plan::TerminalTargetProviderProposal> {
        self.targets
            .iter()
            .map(|target| backend_plan::TerminalTargetProviderProposal {
                target: target.clone(),
                terminal_psi: self.artifact.identity(),
                optimization_selection: self.optimization_selection,
                selection: self.publish_target(target).map_err(|diagnostics| {
                    diagnostics
                        .into_iter()
                        .map(|diagnostic| diagnostic.message)
                        .collect()
                }),
            })
            .collect()
    }

    fn publish_target(
        &self,
        target: &str,
    ) -> Result<backend_plan::TerminalTargetProviderSelection, Vec<Diagnostic>> {
        let profile = target::TargetProfile::from_canonical_target_name(target).map_err(|_| {
            vec![Diagnostic::error(format!(
                "provider proposal has no receiving target catalog for `{target}`"
            ))]
        })?;
        let declarations = super::settle_provider_defaults(self.context(), self.defaults, profile);
        let candidates = crate::derive_satisfies_plans(crate::ProviderPlanDerivation::evaluated(
            self.context(),
            &self.settlement.boundary_calling_plan_realizations,
            Some(target),
            &self.settlement.evaluated_via_bindings,
            &declarations.origins,
        ));
        // Select within the retained demand domain, never by intersecting a
        // plan's rows. An unrelated unique provider is not an installation
        // obligation. Explicit declarations retain their complete schema even
        // when no runtime body calls it; every demanded schema remains whole.
        let module = self.artifact.module();
        let demanded = candidates.retain_demanded(|declaration, schema| {
            let symbol = declaration.symbol();
            let owner = self.declaration_identity(symbol)?;
            Ok(module.boundary_machines.iter().any(|boundary| {
                boundary.role == terminal_psi::BoundaryDeclarationRole::Callable
                    && module.identities.text(boundary.owner.identity()) == Some(owner.as_str())
            }) || declarations
                .provider_defaults
                .iter()
                .chain(self.overrides)
                .any(|selection| {
                    schema.methods.iter().any(|method| {
                        crate::service_schema::selection_matches_schema(
                            &selection.subject,
                            symbol,
                            schema.identity_text(method.requirement_identity),
                            schema,
                        )
                    })
                }))
        })?;
        let selected = crate::select_derived_provider_plans(
            self.context(),
            &demanded,
            Some(profile),
            &declarations,
            self.overrides,
        )?;
        let mut checked_candidates = Vec::new();
        let mut plans = Vec::new();
        for selection in &selected {
            let plan = &selection.derived.plan;
            let provenance = &selection.derived.provenance;
            if plan.rows.len() != provenance.row_requirements.len()
                || plan.rows.len() != provenance.row_realizations.len()
            {
                return Err(vec![Diagnostic::error(
                    "selected provider lost its complete source association",
                )]);
            }
            let provider_declaration = provenance
                .provider_type
                .map(|provider| self.declaration_identity(provider))
                .transpose()?;
            let mut checked_applications = Vec::new();
            let mut checked_application_identities =
                terminal_psi::TerminalIdentityTableBuilder::default();
            for (row_ordinal, row) in plan.rows.iter().enumerate() {
                let effects::provider_plan::ProviderBinding::CheckedAdapter {
                    machine_identity,
                    ..
                } = &row.binding
                else {
                    continue;
                };
                let provider = provider_declaration.as_deref().ok_or_else(|| {
                    vec![Diagnostic::error(
                        "checked provider row has no exact declaration owner",
                    )]
                })?;
                let requirement = provenance.row_requirements[row_ordinal];
                let applications = if self.checked.machine(requirement).is_some()
                    || self.checked.trait_machine_signature(requirement).is_some()
                {
                    let source =
                        crate::selected_applications::selected_checked_requirement_row_for_plan(
                            self.checked,
                            &self.checked.facts,
                            plan,
                            row_ordinal,
                            requirement,
                        )
                        .map_err(|diagnostic| vec![diagnostic])?;
                    if source.adapter().symbol != provenance.row_realizations[row_ordinal] {
                        return Err(vec![Diagnostic::error(
                            "selected provider application changed its original realization",
                        )]);
                    }
                    source.applications().map_err(|diagnostic| vec![diagnostic])?
                        .into_iter().map(|application| {
                            let requirement = self.checked.machine(application.requirement_instance())
                                .and_then(|machine| self.checked.machine_callable_identity_text(machine))
                                .or_else(|| self.checked.trait_machine_signature(application.requirement_instance())
                                    .map(|(definition, signature)| self.checked.trait_requirement_callable_identity_text(definition, signature)))
                                .filter(|identity| !identity.is_empty())
                                .ok_or_else(|| vec![Diagnostic::error("selected provider application lost its exact requirement identity")])?;
                            let realization = self.checked.machine(application.realization_instance())
                                .and_then(|machine| self.checked.machine_callable_identity_text(machine))
                                .filter(|identity| !identity.is_empty())
                                .ok_or_else(|| vec![Diagnostic::error("selected provider application lost its exact callable identity")])?;
                            Ok((requirement, realization))
                        }).collect::<Result<Vec<_>, Vec<Diagnostic>>>()?
                } else if self
                    .checked
                    .operator_definition_by_symbol(requirement)
                    .is_some_and(|operator| operator.is_boundary)
                {
                    // Legacy operator rows have no machine/member application
                    // telescope. Their exact original selected coordinate stays
                    // separate from the source requirement application owner.
                    vec![(
                        plan.identity_text(row.requirement_identity).to_owned(),
                        plan.identity_text(*machine_identity).to_owned(),
                    )]
                } else {
                    return Err(vec![Diagnostic::error(
                        "selected provider row lost its original requirement role",
                    )]);
                };
                for (requirement_identity, realization) in applications {
                    let mut matching = module.provider_candidates.iter().filter(|candidate| {
                        module
                            .identities
                            .text(candidate.provider_declaration.identity())
                            == Some(provider)
                            && module.identities.text(candidate.requirement_identity)
                                == Some(requirement_identity.as_str())
                            && module.identities.text(candidate.candidate_identity)
                                == Some(realization.as_str())
                    });
                    let candidate = matching.next().ok_or_else(|| {
                        vec![Diagnostic::error(format!("selected provider requirement `{requirement_identity}` has no emitted exact application candidate"))]
                    })?;
                    if matching.next().is_some() {
                        return Err(vec![Diagnostic::error(
                            "selected provider association does not name one surviving canonical row",
                        )]);
                    }
                    let provider_identity = self
                        .artifact
                        .module()
                        .identities
                        .text(candidate.provider_identity)
                        .ok_or_else(|| {
                            vec![Diagnostic::error(
                                "selected provider association lost its canonical identity",
                            )]
                        })?;
                    checked_candidates.push(terminal_installation::ProviderInstallationSelection {
                        boundary: candidate.boundary,
                        candidate: candidate.candidate,
                        provider_identity: provider_identity.to_owned(),
                        provider_declaration: provider.to_owned(),
                    });
                    checked_applications.push(effects::SelectedRequirementSpecialization {
                        provider_plan_report_identity: plan.report_fingerprint(),
                        selected_requirement_identity: checked_application_identities
                            .intern(plan.identity_text(row.requirement_identity)),
                        selected_machine_identity: checked_application_identities
                            .intern(plan.identity_text(*machine_identity)),
                        requirement_identity: checked_application_identities
                            .intern(&requirement_identity),
                        machine_identity: checked_application_identities.intern(&realization),
                    });
                }
            }
            plans.push(backend_plan::TerminalTargetProviderPlan {
                plan: plan.clone(),
                provider_declaration,
                checked_applications,
                checked_application_identities: checked_application_identities.build(),
            });
        }
        // The ordinary source owner checks complete original plan provenance,
        // operator-family coverage and independent composition. Concrete
        // application rows never replace those whole declaration-level plans.
        crate::selected_provider_plan_facts(self.context(), selected)?;
        checked_candidates.sort();
        // Required parent schemas can share the same exact selected row.
        // Collapse only identical tuples; competing providers remain distinct
        // and ordinary installation admission rejects the conflicting boundary.
        checked_candidates.dedup();
        Ok(backend_plan::TerminalTargetProviderSelection {
            plans,
            checked_candidates,
        })
    }

    fn declaration_identity(&self, symbol: SymbolHandle) -> Result<String, Vec<Diagnostic>> {
        let typed = &self.checked.typed;
        let identity = typed
            .exact_owner_nominal_identity(symbol, self.exact_toolchain_sources)
            .and_then(|identity| typed.type_identity_text(identity))
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "Build provider selection lost its exact declaration owner",
                )]
            })?;
        Ok(identity.to_owned())
    }
}

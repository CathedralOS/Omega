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

/// Complete interface roster for selections, including inherited methods.
/// Retaining only called coordinates would turn a whole-provider selection
/// into an unauthorized partial composition.
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
                checked_trees::typed_trees::trait_definition::AppliedTraitScope::abstract_scope(
                    definition,
                )
                .visit(context.typed, &mut |scope| {
                    roots.extend(
                        context
                            .typed
                            .trait_machine_signatures(scope.declaration)
                            .iter()
                            .map(|row| row.symbol),
                    );
                });
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
    pub context: ProviderPlanningContext<'program>,
    pub artifact: terminal_codec::TerminalArtifactView<'program>,
    pub exact_toolchain_sources: &'program [(source::SourceId, [u8; 32])],
    pub defaults: &'program [crate::TargetProviderDefaults],
    pub overrides: &'program [ProviderSelection],
    pub settlement: &'program crate::ProviderSettlement,
    pub targets: &'program [String],
    pub optimization_selection: optimization_core::OptimizationSelectionIdentity,
}

impl TargetProviderPublicationRequest<'_> {
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
        // A recognized profile realized outside this compiler, such as the
        // Alpha bootstrap target, publishes a named unavailable row like any
        // other unavailable realization; it is never asked for a native target.
        let native_target = profile.native_realization().ok_or_else(|| {
            vec![Diagnostic::error(format!(
                "target `{target}` has no provider realization in this compiler"
            ))]
        })?;
        let declarations = super::settle_provider_defaults(self.context, self.defaults, profile);
        let candidates = crate::derive_satisfies_plans(crate::ProviderPlanDerivation::evaluated(
            self.context,
            &self.settlement.boundary_calling_plan_realizations,
            Some(target),
            &self.settlement.evaluated_via_bindings,
            &declarations.origins,
        ))?;
        // Select within the retained demand domain, never by intersecting a
        // plan's rows. An unrelated unique provider is not an installation
        // obligation. Explicit declarations retain their complete schema even
        // when no runtime body calls it; every demanded schema remains whole.
        let module = self.artifact.module();
        let mut demanded = Vec::with_capacity(candidates.len());
        for candidate in candidates {
            let schema = candidate.provenance.schema.symbol();
            let owner = self.declaration_identity(schema)?;
            if module.boundary_machines.iter().any(|boundary| {
                boundary.role == terminal_psi::BoundaryDeclarationRole::Callable
                    && module.identities.text(boundary.owner.identity()) == Some(owner.as_str())
            }) || declarations
                .provider_defaults
                .iter()
                .chain(self.overrides)
                .any(|selection| {
                    candidate.plan.rows.iter().any(|row| {
                        selection.subject.selects_schema(
                            schema,
                            candidate.plan.identity_text(row.requirement_identity),
                        )
                    })
                })
            {
                demanded.push(candidate);
            }
        }
        let selected = crate::select_derived_provider_plans(
            &demanded,
            native_target,
            &declarations.provider_defaults,
            self.overrides,
        )?;
        // This ordinary owner checks authored provenance, operator-family
        // completeness and independent composition. Its returned provenance
        // keeps each whole plan with its selecting symbol; independently sorted
        // plan/owner vectors must never be positionally zipped.
        let (_, provenance) = crate::selected_provider_plan_facts(self.context, selected)?;
        let plans = provenance
            .into_iter()
            .map(|selected| {
                let provider_declaration = selected
                    .provider
                    .provider_type
                    .map(|provider| self.declaration_identity(provider))
                    .transpose()?;
                Ok(backend_plan::TerminalTargetProviderPlan {
                    plan: selected.plan,
                    provider_declaration,
                })
            })
            .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
        backend_plan::TerminalTargetProviderSelection::from_plans(plans, self.artifact)
            .map_err(|reason| vec![Diagnostic::error(reason)])
    }

    fn declaration_identity(&self, symbol: SymbolHandle) -> Result<String, Vec<Diagnostic>> {
        let typed = self.context.typed;
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

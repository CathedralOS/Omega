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

mod prepared;
pub use prepared::PreparedTargetProviderPublication;
use prepared::{PreparedCandidate, PreparedTarget};

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

/// Prepare source relationships before Terminal production. Artifact demand
/// remains unknown here; no candidate is selected or installed by preparation.
pub struct TargetProviderPreparationRequest<'program> {
    pub context: ProviderPlanningContext<'program>,
    pub exact_toolchain_sources: &'program [(source::SourceId, [u8; 32])],
    pub defaults: &'program [crate::TargetProviderDefaults],
    pub overrides: &'program [ProviderSelection],
    pub assumed_bindings: &'program [effects::AssumedBindingContractSelection],
    pub settlement: &'program crate::ProviderSettlement,
    pub targets: &'program [String],
}

impl TargetProviderPreparationRequest<'_> {
    /// Preserve preparation failures per target, including unavailable sibling
    /// targets. Neither a failure nor an unused candidate invalidates production.
    pub fn prepare(&self) -> PreparedTargetProviderPublication {
        let targets = self
            .targets
            .iter()
            .map(|target| (target.clone(), self.prepare_target(target)))
            .collect();
        PreparedTargetProviderPublication::new(targets, self.assumed_bindings.to_vec())
    }

    fn prepare_target(&self, target: &str) -> Result<PreparedTarget, Vec<Diagnostic>> {
        let profile = target::TargetProfile::from_canonical_target_name(target).map_err(|_| {
            vec![Diagnostic::error(format!(
                "provider proposal has no receiving target catalog for `{target}`"
            ))]
        })?;
        // A recognized profile realized outside this compiler, such as the
        // Alpha bootstrap target, publishes a named unavailable row like any
        // other unavailable realization; it is never asked for a native target.
        profile.native_realization().ok_or_else(|| {
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
        let mut prepared = Vec::with_capacity(candidates.len());
        for candidate in candidates {
            let schema = candidate.provenance.schema.symbol();
            let schema_owner = self.declaration_identity(schema)?;
            let explicitly_selected = declarations
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
                });
            // This identity was previously queried only after selection and
            // provenance admission. Preserve that failure boundary, even though
            // the source query itself now belongs to preparation.
            let provider_owner = candidate
                .provenance
                .provider_type
                .map(|provider| self.declaration_identity(provider))
                .transpose();
            prepared.push(PreparedCandidate {
                derived: candidate,
                schema_owner,
                provider_owner,
                explicitly_selected,
            });
        }
        let selection_declarations =
            crate::provider_planning::PreparedSelectionDeclarations::prepare(
                self.context,
                declarations.provider_defaults.iter().chain(self.overrides),
            );
        Ok(PreparedTarget {
            profile,
            candidates: prepared,
            defaults: declarations.provider_defaults,
            overrides: self.overrides.to_vec(),
            selection_declarations,
        })
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

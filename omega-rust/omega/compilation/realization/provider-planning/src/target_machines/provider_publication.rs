//! Close authored selections into source-free target proposals.
//!
//! Consume retained authored inventories and ordinary whole-provider selection;
//! publication never derives candidates or settles defaults. Build identities join the
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

pub use provider_selection::PreparedTargetProviderPublication;
use provider_selection::{PreparedProviderCandidate, PreparedTargetChoices};

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
    /// None is reusable Source checking; Some is the complete Build deployment
    /// entry roster, with provider-only deployments represented by an empty slice.
    pub application_roots: Option<&'program [(String, SymbolHandle)]>,
    pub application_scopes: &'program [String],
    pub overrides: &'program [ProviderSelection],
    pub assumed_bindings: &'program [effects::AssumedBindingContractSelection],
    pub independent_components:
        &'program [std::sync::Arc<component_description::VerifiedComponent>],
    pub targets: &'program [(
        String,
        Result<std::sync::Arc<super::AuthoredTargetProviders>, Vec<Diagnostic>>,
    )],
}

impl TargetProviderPreparationRequest<'_> {
    /// Preserve preparation failures per target, including unavailable sibling
    /// targets. Neither a failure nor an unused candidate invalidates production.
    pub fn prepare(&self) -> PreparedTargetProviderPublication {
        let targets = self
            .targets
            .iter()
            .map(|(target, inventory)| {
                let prepared = inventory
                    .as_ref()
                    .map_err(Clone::clone)
                    .and_then(|inventory| self.prepare_target(target, inventory));
                (target.clone(), prepared)
            })
            .collect();
        PreparedTargetProviderPublication::new(
            targets,
            self.assumed_bindings.to_vec(),
            self.application_roots.is_some(),
            self.application_scopes.to_vec(),
        )
    }

    fn prepare_target(
        &self,
        target: &str,
        inventory: &super::AuthoredTargetProviders,
    ) -> Result<PreparedTargetChoices, Vec<Diagnostic>> {
        let profile = inventory.target().ok_or_else(|| {
            vec![Diagnostic::error(
                "provider publication requires a target inventory",
            )]
        })?;
        if target::TargetProfile::from_canonical_target_name(target).ok() != Some(profile) {
            return Err(vec![Diagnostic::error(
                "provider publication target differs from its authored inventory",
            )]);
        }
        let candidates = inventory.candidates();
        let mut prepared = Vec::with_capacity(candidates.len());
        for candidate in candidates {
            let schema = candidate.provenance.schema.symbol();
            let schema_owner = self.declaration_identity(schema)?;
            let explicitly_selected =
                inventory
                    .defaults()
                    .iter()
                    .chain(self.overrides)
                    .any(|selection| {
                        selection
                            .target
                            .is_none_or(|selected| selected.canonical_name() == target)
                            && candidate.plan.rows.iter().any(|row| {
                                selection.subject.selects_schema(
                                    schema,
                                    candidate.plan.identity_text(row.requirement_identity),
                                    candidate.plan.schema.root_application_identity(),
                                )
                            })
                    });
            // Close nominal and numeric realization declarations while source
            // custody exists. Retain failure until selection and provenance
            // admission demand this candidate, preserving the prior boundary.
            let closed_declarations = crate::native_provider_sources::close_provider_declarations(
                self.context,
                self.exact_toolchain_sources,
                &candidate.plan,
                &candidate.provenance,
            );
            prepared.push(PreparedProviderCandidate {
                choice: provider_selection::ProviderCandidate {
                    plan: candidate.plan.clone(),
                    declarations: provider_selection::CandidateDeclarations {
                        schema: self.canonical_declaration(schema),
                        provider: candidate
                            .provenance
                            .provider_type
                            .map(|symbol| self.canonical_declaration(symbol)),
                    },
                    native_build: candidate
                        .provenance
                        .row_realizations
                        .iter()
                        .all(|row| row.native().is_some()),
                    owner: closed_declarations.map(|closed| {
                        provider_selection::PreparedProviderRealizations {
                            provider_declaration: closed.provider_declaration,
                            row_numeric_realizations: closed.row_numeric_realizations,
                        }
                    }),
                },
                schema_owner,
                explicitly_selected,
            });
        }
        let application_roots = self
            .application_roots
            .map(|roots| {
                roots
                    .iter()
                    .filter(|(scope, _)| scope == target)
                    .map(|(_, root)| {
                        self.canonical_declaration(*root)
                            .map_err(|reason| vec![Diagnostic::error(reason)])
                    })
                    .collect::<Result<Vec<_>, _>>()
            })
            .transpose()?;
        Ok(PreparedTargetChoices {
            application_roots,
            profile,
            independent_components: self.independent_components.to_vec(),
            candidates: prepared,
            defaults: inventory
                .defaults()
                .iter()
                .map(|choice| self.canonical_choice(choice))
                .collect(),
            overrides: self
                .overrides
                .iter()
                .map(|choice| self.canonical_choice(choice))
                .collect(),
        })
    }

    fn canonical_choice(
        &self,
        source: &ProviderSelection,
    ) -> provider_selection::ProviderDeclaration<provider_selection::CanonicalProviderDeclaration>
    {
        let mut choice = crate::provider_planning::selection_declarations::project_declaration(
            source,
            &|symbol| self.canonical_declaration(symbol),
            (),
        );
        crate::provider_planning::selection_declarations::retain_family_replay(
            self.context,
            source,
            &mut choice,
        );
        choice
    }

    fn canonical_declaration(
        &self,
        symbol: SymbolHandle,
    ) -> Result<provider_selection::CanonicalProviderDeclaration, String> {
        use provider_selection::CanonicalProviderDeclaration as Declaration;
        let typed = self.context.typed;
        let owner = self.declaration_identity(symbol).map_err(|diagnostics| {
            diagnostics
                .into_iter()
                .map(|diagnostic| diagnostic.message)
                .collect::<Vec<_>>()
                .join("; ")
        })?;
        match typed.symbols.get(symbol).kind {
            symbols::SymbolKind::Trait => Ok(Declaration::Trait(owner)),
            symbols::SymbolKind::Conformance => Ok(Declaration::Implementation(owner)),
            symbols::SymbolKind::Data => Ok(Declaration::Data(owner)),
            symbols::SymbolKind::Domain => Ok(Declaration::Domain(owner)),
            symbols::SymbolKind::BuiltinType => Ok(Declaration::Builtin(owner)),
            symbols::SymbolKind::Machine => {
                let machine = typed
                    .machine_by_symbol(symbol)
                    .ok_or("provider declaration has no exact machine")?;
                let callable = typed
                    .machine_callable_identity_text(machine)
                    .ok_or("provider declaration has no exact machine overload")?;
                Ok(Declaration::Machine { owner, callable })
            }
            symbols::SymbolKind::Operator => {
                let operator =
                    checked_trees::typed_trees::operator::declaration_by_symbol(typed, symbol)
                        .ok_or("provider declaration has no exact operator")?;
                let identity = typed.boundary_operator_requirement_callable_identity(operator);
                if !identity.is_valid() {
                    return Err("provider declaration has no exact operator overload".into());
                }
                Ok(Declaration::Operator {
                    owner,
                    callable: typed.callable_identity_text(identity),
                })
            }
            _ => Err("provider declaration has an unsupported nominal role".into()),
        }
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

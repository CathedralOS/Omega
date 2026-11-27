//! Admit the exact provider records chosen by source policy. No candidate
//! body, schema application or declaration is inferred from a name.

use crate::source_state::provider_declarations::ProviderPlanningContext;
use crate::source_state::service_declarations::projection;
use crate::source_state::trait_definition::AppliedTraitScope;
use semantic_vocabulary::MachineId;
use terminal_check_tables::Checked;

use super::Program;
use crate::declare::provider_selections::{
    ProviderSelections, SourceProviderSelection, SourceProviderSelections,
};

impl<'source> Program<'source> {
    pub(crate) fn prepare_provider_selections(
        &mut self,
        context: MachineId,
        selected: ProviderSelections<'_>,
    ) -> Checked<SourceProviderSelections<'source>> {
        let typed = self.typed;
        if !std::ptr::eq(typed, selected.source) {
            return Err(crate::read::unavailable(
                context,
                "provider choices belong to another frozen source owner",
            ));
        }
        let mut prepared = SourceProviderSelections {
            source: typed,
            rows: Vec::new(),
        };
        let symbol_key =
            |symbol: symbols::SymbolHandle| (symbol.arena_index(), symbol.generation());
        let supply_key = |row: &crate::SuppliedRequirement| {
            (
                symbol_key(row.provider),
                symbol_key(row.requirement),
                row.machine,
            )
        };
        let mut supplied_keys = self
            .supplied
            .iter()
            .map(supply_key)
            .collect::<std::collections::BTreeSet<_>>();
        for selected in selected.choices {
            if selected.target.is_empty()
                || !typed.symbols.symbols().nodes().is_valid(selected.provider)
            {
                return Err(crate::read::unavailable(
                    context,
                    "provider choice has a missing target or stale declaration",
                ));
            }
            let scope = selected.schema.scope(typed).ok_or_else(|| {
                crate::read::unavailable(
                    context,
                    "provider choice lost its complete schema application",
                )
            })?;
            let provider = typed.implementation_value_head(selected.provider);
            let declaration = typed.conformance_by_symbol(provider).ok_or_else(|| {
                crate::read::unavailable(
                    context,
                    "selected provider is not a complete named implementation record",
                )
            })?;
            let (provider_scope, provider_application, implementation_record) =
                if let Some(application) = selected.provider_application {
                    if !typed.product_operand_applications.is_valid(application) {
                        return Err(crate::read::unavailable(
                            context,
                            "provider choice has a stale complete static application",
                        ));
                    }
                    let actual = typed.product_operand_applications.get(application);
                    if actual.role
                        != Some(crate::declaration_selection::ProductOperandRole::Provider)
                    {
                        return Err(crate::read::unavailable(
                            context,
                            "provider choice application has a different source operand role",
                        ));
                    }
                    let implementation = actual.implementation.as_ref().ok_or_else(|| {
                        crate::read::unavailable(
                            context,
                            "provider choice application has no exact closed implementation record",
                        )
                    })?;
                    if implementation.declaration != provider {
                        return Err(crate::read::unavailable(
                            context,
                            "provider choice application selects another declaration",
                        ));
                    }
                    let provider_scope =
                        implementation.trait_scope.scope(typed).ok_or_else(|| {
                            crate::read::unavailable(
                                context,
                                "provider choice lost its actual closed implementation namespace",
                            )
                        })?;
                    let identity = typed
                        .product_operand_application_identity(application)
                        .map_err(|reason| crate::read::unavailable(context, reason))?
                        .filter(|identity| !identity.is_empty())
                        .ok_or_else(|| {
                            crate::read::unavailable(
                                context,
                                "provider choice has no complete portable static record identity",
                            )
                        })?;
                    (provider_scope, Some(identity), Some(implementation))
                } else {
                    if !declaration.type_parameters.is_empty()
                        || !declaration.lifetime_parameters.is_empty()
                    {
                        return Err(crate::read::unavailable(
                            context,
                            "selected provider requires its complete static application namespace",
                        ));
                    }
                    (
                        AppliedTraitScope::from_conformance(typed, declaration).ok_or_else(
                            || {
                                crate::read::unavailable(
                                    context,
                                    "provider has no complete declared schema application",
                                )
                            },
                        )?,
                        None,
                        None,
                    )
                };
            let supplied = provider_scope
                .declaring_scope(typed, scope.declaration.symbol)
                .ok_or_else(|| {
                    crate::read::unavailable(
                        context,
                        "provider does not declare the selected schema",
                    )
                })?;
            if !supplied.same_application(typed, &scope) {
                return Err(crate::read::unavailable(
                    context,
                    "provider supplies a different complete schema application",
                ));
            }
            // The shared reader forms the complete implementation and retains
            // every operation's actual refinement question and body root.
            if !self.applied_conformance_declaration(
                context,
                provider,
                implementation_record.zip(provider_application.as_deref()),
            )? {
                return Err(crate::read::unavailable(
                    context,
                    "provider choice has no exact implementation declaration",
                ));
            }
            let projected = projection::from_typed_application(
                ProviderPlanningContext::new(typed, self.purpose),
                scope.clone(),
            )
            .map_err(|diagnostic| crate::read::unavailable(context, diagnostic.message))?;
            projected
                .validate_correspondence()
                .map_err(|diagnostic| crate::read::unavailable(context, diagnostic.message))?;
            let application = scope
                .portable_application_identity(typed)
                .and_then(|identity| typed.type_identity_text(identity))
                .filter(|identity| !identity.is_empty())
                .ok_or_else(|| {
                    crate::read::unavailable(
                        context,
                        "provider schema has no canonical complete application identity",
                    )
                })?;
            if projected.declaration.root_application_identity() != Some(application.as_str()) {
                return Err(crate::read::unavailable(
                    context,
                    "provider schema projection changed its actual root application",
                ));
            }
            // The canonical conformance questions already retain each exact
            // receiving header and body. Publish those same supplies rather
            // than reading the choice again as a bare trait nomination.
            let questions = self
                .static_contracts
                .conformance_questions(provider, provider_application.as_deref())
                .ok_or_else(|| {
                    crate::read::unavailable(
                        context,
                        "provider lost its checked conformance questions",
                    )
                })?;
            for question in questions {
                let (
                    Some(candidate),
                    crate::callables::static_contracts::SourceImplementation::Machine(machine),
                ) = (&question.provider, question.implementation)
                else {
                    continue;
                };
                if !projected.declaration.matches_receiving_application(
                    &candidate.source_requirement,
                    Some(&question.application),
                ) {
                    continue;
                }
                let requirement = self
                    .boundaries
                    .rows
                    .get(candidate.boundary)
                    .ok_or_else(|| {
                        crate::read::unavailable(
                            context,
                            "provider lost its exact receiving header",
                        )
                    })?
                    .source;
                let row = crate::SuppliedRequirement {
                    provider,
                    requirement,
                    machine,
                };
                if supplied_keys.insert(supply_key(&row)) {
                    self.supplied.push(row);
                }
            }
            prepared.rows.push(SourceProviderSelection {
                target: selected.target.clone(),
                schema_declaration: typed
                    .normalized_hermetic_symbol_identity(scope.declaration.symbol)
                    .map_err(|reason| crate::read::unavailable(context, reason))?,
                schema_application: application,
                surface: projected.declaration,
                provider_declaration: provider,
                provider_identity: match &provider_application {
                    Some(identity) => identity.clone(),
                    None => typed
                        .normalized_hermetic_symbol_identity(provider)
                        .map_err(|reason| crate::read::unavailable(context, reason))?,
                },
                provider_application,
                provider_package: typed.symbols.symbol_package_identity(provider),
                provider_custody: crate::publication::selection_questions::declaration_custody(
                    typed, provider,
                )
                .map_err(|reason| crate::read::unavailable(context, reason.to_string()))?,
                composition_mode: selected.composition_mode,
            });
            self.admitted_declarations.include(provider);
            self.admitted_declarations.include(scope.declaration.symbol);
        }
        Ok(prepared)
    }
}

//! Normalize executed Build provider policy while its exact source is present.
//! This chooses source records; their full contracts are still questions of the
//! ordinary lowered judgment. No native/ABI ProviderPlan is fabricated here.

use build_time_evaluation::provider_declarations::{
    ProviderSelection, ProviderSelectionSubject, TargetProviderDefaults,
};
use diagnostics::Diagnostic;
use provider_planning::target_supply::{
    ProviderSupply, SupplyAuthority, UnchosenSupply, choose_supply,
};
use std::sync::Arc;
use symbols::SymbolHandle;
use typed_trees::provider_declarations::ProviderPlanningContext;
use typed_trees::service_declarations::projection;
use typed_trees::trait_definition::{AppliedTraitScope, TraitApplicationEnvironment};
use typed_trees::{FrozenTypedTrees, TypedTrees};

use source_compiler::TerminalProviderSelectionInput as ProviderSelectionInput;

type Slot = ((u32, u32), Option<String>);
type Provider = ((u32, u32), Option<String>);

fn key(symbol: SymbolHandle) -> (u32, u32) {
    (symbol.arena_index(), symbol.generation())
}
fn diagnostic(reason: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::error(reason)]
}
fn application(
    typed: &TypedTrees,
    scope: &AppliedTraitScope<'_>,
) -> Result<String, Vec<Diagnostic>> {
    scope
        .portable_application_identity(typed)
        .and_then(|identity| typed.type_identity_text(identity))
        .filter(|identity| !identity.is_empty())
        .ok_or_else(|| diagnostic("provider schema has no complete application identity"))
}

/// Source-normalized choices and their exact declaration/calling dependencies.
/// These inputs must be bound to this same typed owner before judgment.
pub struct PreparedProviderChoices {
    pub selections: Vec<ProviderSelectionInput>,
    pub declaration_roots: Vec<SymbolHandle>,
    pub calling_applications: Vec<TraitApplicationEnvironment>,
}

struct SourceCandidate {
    slot: Slot,
    provider_key: Provider,
    provider: SymbolHandle,
    scope: TraitApplicationEnvironment,
    target: String,
    complete: bool,
    surface: Arc<language_semantics::service_surface::ServiceSurface>,
}
fn shares_source_receiver(
    left: &language_semantics::service_surface::ServiceSurface,
    right: &language_semantics::service_surface::ServiceSurface,
) -> bool {
    left.receiver_applications.iter().any(|left_row| {
        right
            .receiver_applications
            .iter()
            .any(|right_row| left_row.eq_in(&left.identities, right_row, &right.identities))
    })
}

struct SourceDeclaration<'a> {
    source: &'a ProviderSelection,
    slot: (u32, u32),
    application: Option<String>,
    target: Option<String>,
    provider: Provider,
}
impl SourceDeclaration<'_> {
    fn slots(&self, available: &[Slot]) -> Vec<Slot> {
        match &self.application {
            Some(application) => vec![(self.slot, Some(application.clone()))],
            None => {
                let matched = available
                    .iter()
                    .filter(|slot| slot.0 == self.slot)
                    .cloned()
                    .collect::<Vec<_>>();
                if matched.is_empty() {
                    vec![(self.slot, None)]
                } else {
                    matched
                }
            }
        }
    }
}

fn declaration<'a>(
    typed: &TypedTrees,
    source: &'a ProviderSelection,
    retained_target: Option<&str>,
) -> Result<SourceDeclaration<'a>, Vec<Diagnostic>> {
    let authored_target = source.target.map(|target| target.canonical_name());
    if retained_target
        .zip(authored_target)
        .is_some_and(|(outer, inner)| outer != inner)
    {
        return Err(diagnostic(
            "provider default changed its exact source target",
        ));
    }
    let subject = match &source.subject {
        ProviderSelectionSubject::BoundaryTrait(subject) => subject,
        ProviderSelectionSubject::BoundaryRequirement(_) => {
            return Err(diagnostic(
                "source provider choice needs its exact standalone-requirement record projection",
            ));
        }
        ProviderSelectionSubject::BoundaryOperatorFamily(_) => {
            return Err(diagnostic(
                "source provider choice needs its exact boundary-operator record projection",
            ));
        }
    };
    for symbol in [subject.symbol, source.provider_type.symbol] {
        if !typed.symbols.symbols().nodes().is_valid(symbol) {
            return Err(diagnostic(
                "provider policy retains a missing or stale source declaration",
            ));
        }
    }
    if !typed
        .trait_definition_by_symbol(subject.symbol)
        .is_some_and(|owner| owner.is_boundary)
    {
        return Err(diagnostic(
            "provider selection subject is not its exact boundary schema",
        ));
    }
    if subject.applied || source.provider_type.applied {
        // These requests keep their complete source applications. They must be
        // projected by the static record owner rather than erased to the bare
        // template or interpreted from application display text.
        for identity in [subject, &source.provider_type] {
            if identity.applied
                && (!typed
                    .product_operand_applications
                    .is_valid(identity.application)
                    || typed
                        .product_operand_application_identity(identity.application)
                        .map_err(diagnostic)?
                        .is_none())
            {
                return Err(diagnostic(
                    "provider policy lost its exact complete static application",
                ));
            }
        }
        return Err(diagnostic(
            "source provider choice needs the retained applied static record namespace",
        ));
    }
    if subject.application.is_valid()
        || source.provider_type.application.is_valid()
        || subject.application_identity.is_some()
        || source.provider_type.application_identity.is_some()
    {
        return Err(diagnostic(
            "bare provider policy retains conflicting application metadata",
        ));
    }
    let provider = typed.implementation_value_head(source.provider_type.symbol);
    if typed.conformance_by_symbol(provider).is_none() {
        return Err(diagnostic(
            "provider position is not a complete named implementation record",
        ));
    }
    Ok(SourceDeclaration {
        source,
        slot: key(subject.symbol),
        application: subject.application_identity.clone(),
        target: retained_target.or(authored_target).map(str::to_owned),
        provider: (
            key(provider),
            source.provider_type.application_identity.clone(),
        ),
    })
}

pub fn prepare_with_source_defaults(
    source: &Arc<FrozenTypedTrees>,
    targets: &[target::TargetProfile],
    overrides: &[ProviderSelection],
    defaults: &[ProviderSelection],
    source_defaults: &[TargetProviderDefaults],
) -> Result<PreparedProviderChoices, Vec<Diagnostic>> {
    let typed: &TypedTrees = source;
    let context = ProviderPlanningContext::new(typed, source::DependencyScope::Product);
    let defaults = source_defaults
        .iter()
        .flat_map(|wrapper| {
            wrapper
                .selections
                .iter()
                .map(move |selection| declaration(typed, selection, wrapper.target.as_deref()))
        })
        .chain(
            defaults
                .iter()
                .map(|selection| declaration(typed, selection, None)),
        )
        .collect::<Result<Vec<_>, _>>()?;
    let overrides = overrides
        .iter()
        .map(|row| declaration(typed, row, None))
        .collect::<Result<Vec<_>, _>>()?;
    let mut subjects = defaults
        .iter()
        .chain(&overrides)
        .map(|row| row.slot)
        .collect::<Vec<_>>();
    subjects.sort_unstable();
    subjects.dedup();
    let mut candidates = Vec::new();
    for provider in typed.conformances() {
        if !context.contains(provider.symbol) {
            continue;
        }
        // Open records are not evaluated provider values. A selection of an
        // open record was diagnosed above; no eligible implementation roster.
        if !provider.type_parameters.is_empty() || !provider.lifetime_parameters.is_empty() {
            continue;
        }
        let Some(root) = AppliedTraitScope::from_conformance(typed, provider) else {
            return Err(diagnostic(
                "provider declaration has no complete source schema application",
            ));
        };
        if !root.declaration.is_boundary {
            continue;
        }
        for subject in &subjects {
            let mut matches = typed
                .traits()
                .iter()
                .filter(|schema| key(schema.symbol) == *subject);
            let Some(schema) = matches.next() else {
                continue;
            };
            if matches.next().is_some() {
                return Err(diagnostic(
                    "provider policy has an ambiguous source schema declaration",
                ));
            }
            let Some(scope) = root.declaring_scope(typed, schema.symbol) else {
                continue;
            };
            let projection = projection::from_typed_application(context, scope.clone())
                .ok_or_else(|| diagnostic("provider has no normalized source schema projection"))?;
            projection
                .validate_correspondence()
                .map_err(|error| vec![error])?;
            let mut complete = true;
            let mut target = None;
            for requirement in &projection.requirements {
                let mut rows = provider.rows.iter().filter(|row| {
                    row.declaring_trait == requirement.declaring_trait
                        && row.requirement == requirement.symbol
                });
                let row = match (rows.next(), rows.next()) {
                    (Some(row), None) => row,
                    _ => {
                        complete = false;
                        continue;
                    }
                };
                let machine = typed
                    .machine_by_symbol(row.realization_machine)
                    .ok_or_else(|| {
                        diagnostic("provider operation lost its exact realization declaration")
                    })?;
                let states = typed.machine_states(machine);
                if states.len() != 1 || states[0].symbol != row.realization_state {
                    return Err(diagnostic("provider operation lost its exact entry state"));
                }
                if machine.target_family_interface {
                    return Err(diagnostic(
                        "provider operation requires its exact target-family application",
                    ));
                }
                if let Some(actual) = typed.machine_authored_target(machine) {
                    if target
                        .as_ref()
                        .is_some_and(|known: &String| known != actual)
                    {
                        complete = false;
                    } else {
                        target = Some(actual.to_owned());
                    }
                }
            }
            let provider_symbol = typed.implementation_value_head(provider.symbol);
            candidates.push(SourceCandidate {
                slot: (key(schema.symbol), Some(application(typed, &scope)?)),
                provider_key: (key(provider_symbol), None),
                provider: provider_symbol,
                scope: scope.environment(),
                target: target.unwrap_or_default(),
                complete,
                surface: projection.declaration,
            });
        }
    }
    let mut available = candidates
        .iter()
        .map(|row| row.slot.clone())
        .collect::<Vec<_>>();
    available.sort_unstable();
    available.dedup();
    let mut selections = Vec::new();
    let mut roots = Vec::new();
    let mut applications = Vec::new();
    let mut seen_targets = std::collections::BTreeSet::new();
    for target in targets {
        if !seen_targets.insert(target.target_name()) {
            continue;
        }
        let applies = |row: &&SourceDeclaration<'_>| {
            row.target
                .as_deref()
                .is_none_or(|selected| selected == target.target_name())
        };
        let mut slots = defaults
            .iter()
            .chain(&overrides)
            .filter(applies)
            .flat_map(|row| row.slots(&available))
            .collect::<Vec<_>>();
        slots.sort_unstable();
        slots.dedup();
        let mut selected_candidates = Vec::<usize>::new();
        for slot in slots {
            let declarations = defaults
                .iter()
                .map(|row| (SupplyAuthority::TargetDefault, row))
                .chain(overrides.iter().map(|row| (SupplyAuthority::Build, row)))
                .filter(|(_, row)| applies(row) && row.slots(&available).contains(&slot))
                .collect::<Vec<_>>();
            let supplies = declarations
                .iter()
                .map(|(authority, row)| ProviderSupply {
                    authority: *authority,
                    selection: row.source.clone(),
                })
                .collect::<Vec<_>>();
            let selected = choose_supply(&supplies).map_err(|reason| {
                let detail = match reason {
                    UnchosenSupply::Unsupplied => {
                        "explicit source slot has no provider supply".to_owned()
                    }
                    UnchosenSupply::Competing(rows) => {
                        let category = match rows[0].authority {
                            SupplyAuthority::Build => "ConflictingOverrides",
                            SupplyAuthority::TargetDefault => "ConflictingDefaults",
                        };
                        let providers = rows
                            .iter()
                            .map(|row| row.selection.provider_type.authored_path.as_str())
                            .collect::<Vec<_>>()
                            .join(", ");
                        format!("{category}: {providers}")
                    }
                };
                diagnostic(format!(
                    "source provider selection for {}: {detail}",
                    target.target_name()
                ))
            })?;
            let origin = declarations
                .iter()
                .zip(&supplies)
                .find_map(|((_, row), supply)| std::ptr::eq(supply, selected).then_some(*row))
                .ok_or_else(|| {
                    diagnostic("canonical provider supply lost its source declaration")
                })?;
            if !available.contains(&slot) {
                return Err(diagnostic(format!(
                    "source provider selection for {}: UnknownSlot",
                    target.target_name()
                )));
            }
            let mut matching = candidates.iter().enumerate().filter(|(_, candidate)| {
                candidate.slot == slot
                    && candidate.provider_key == origin.provider
                    && (candidate.target.is_empty() || candidate.target == target.target_name())
            });
            let (Some((index, candidate)), None) = (matching.next(), matching.next()) else {
                return Err(diagnostic(format!(
                    "source provider selection for {} has no unique exact candidate for `{}`",
                    target.target_name(),
                    origin.source.provider_type.authored_path
                )));
            };
            if !candidate.complete {
                return Err(diagnostic(
                    "selected provider does not cover its complete source schema",
                ));
            }
            for &previous in &selected_candidates {
                let previous = &candidates[previous];
                if previous.slot != candidate.slot
                    && shares_source_receiver(&previous.surface, &candidate.surface)
                {
                    return Err(diagnostic(
                        "source provider choices with a shared receiver need their owner-bound calling application comparison",
                    ));
                }
            }
            selected_candidates.push(index);
            selections.push(ProviderSelectionInput {
                target: target.target_name().to_owned(),
                schema: candidate.scope.clone(),
                provider: candidate.provider,
                provider_application: None,
                composition_mode: origin.source.composition_mode,
            });
            roots.push(candidate.provider);
            let scope = candidate
                .scope
                .scope(typed)
                .ok_or_else(|| diagnostic("selected provider schema scope became stale"))?;
            if source
                .boundary_signatures()
                .calling()
                .requests()
                .any(|request| request.boundary == scope.declaration.symbol)
            {
                applications.push(candidate.scope.clone());
            }
        }
    }
    roots.sort_unstable_by_key(|symbol| key(*symbol));
    roots.dedup();
    Ok(PreparedProviderChoices {
        selections,
        declaration_roots: roots,
        calling_applications: applications,
    })
}

#[cfg(test)]
mod tests;

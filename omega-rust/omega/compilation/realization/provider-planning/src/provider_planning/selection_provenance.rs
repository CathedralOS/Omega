//! Source correspondence adapter for the shared whole-provider selector.

use crate::{DerivedProviderPlan, ProviderPlanProvenance};
use component_description::VerifiedComponent;
use effects::CompilerIntrinsicExecutionIdentity;
use effects::provider_plan::ProviderPlan;
use symbols::SymbolHandle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderSelectionProvenance {
    BuildOverride(Vec<crate::ProviderSelection>),
    TargetDefault(Vec<crate::ProviderSelection>),
    UniqueCoveringCandidate,
    NativeBuild,
}

impl ProviderSelectionProvenance {
    /// Composition policy is shared with canonical publication.
    pub fn composition_mode(&self) -> Result<crate::CompositionMode, String> {
        super::selection_declarations::project_provenance(self.clone()).composition_mode()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedProviderPlanWithProvenance {
    pub derived: DerivedProviderPlan,
    pub selected_by: ProviderSelectionProvenance,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedProviderReviewProvenance {
    pub plan: ProviderPlan,
    pub provider: ProviderPlanProvenance,
    pub selected_by: ProviderSelectionProvenance,
    /// Host and placement execution identity for each provider row. Numeric
    /// rows carry None: their meaning lives in Psi's external declarations.
    ///
    /// Selection initially leaves this empty because exact execution is not
    /// settled until after checking. The compiler must replace it with one
    /// row-aligned entry per plan row before publishing `CheckedCompilation`.
    /// `Some` is reserved for compiler-intrinsic rows whose selected
    /// execution has a closed identity; all other rows retain `None`.
    pub row_compiler_intrinsic_executions: Vec<Option<CompilerIntrinsicExecutionIdentity>>,
    /// Exact external leaves reached on every checked completion path. This
    /// does not turn an authored adapter into a compiler intrinsic.
    pub row_external_completion_leaves: Vec<Vec<SymbolHandle>>,
}

pub fn selected_provider_plan_facts(
    context: crate::ProviderPlanningContext<'_>,
    selected: Vec<SelectedProviderPlanWithProvenance>,
) -> Result<
    (
        effects::SelectedProviderPlanFacts,
        Vec<SelectedProviderReviewProvenance>,
    ),
    Vec<diagnostics::Diagnostic>,
> {
    selected_provider_plan_facts_with_independent_components(context, selected, &[])
}

/// Keep rootless Check/review on exact invocation declarations. No canonical
/// owner is required here; publication projects its own closed identities.
pub(crate) fn selected_provider_plan_facts_with_independent_components(
    context: crate::ProviderPlanningContext<'_>,
    selected: Vec<SelectedProviderPlanWithProvenance>,
    independent_components: &[VerifiedComponent],
) -> Result<
    (
        effects::SelectedProviderPlanFacts,
        Vec<SelectedProviderReviewProvenance>,
    ),
    Vec<diagnostics::Diagnostic>,
> {
    let mut choices = Vec::with_capacity(selected.len());
    for selected in selected {
        let mut provenance =
            super::selection_declarations::project_provenance(selected.selected_by);
        match &mut provenance {
            provider_selection::SelectionProvenance::BuildOverride(declarations)
            | provider_selection::SelectionProvenance::TargetDefault(declarations) => {
                for declaration in declarations {
                    let source = declaration.origin.clone();
                    super::selection_declarations::retain_family_replay(
                        context,
                        &source,
                        declaration,
                    );
                }
            }
            provider_selection::SelectionProvenance::UniqueCoveringCandidate
            | provider_selection::SelectionProvenance::NativeBuild => {}
        }
        choices.push(provider_selection::SelectedProvider {
            candidate: super::selection_declarations::source_candidate(selected.derived),
            selected_by: provenance,
        });
    }
    let (facts, selected) = provider_selection::admit(choices, independent_components)?;
    let provenance = selected
        .into_iter()
        .map(|selected| SelectedProviderReviewProvenance {
            plan: selected.candidate.plan,
            provider: selected.candidate.owner,
            selected_by: super::selection_declarations::restore_provenance(selected.selected_by),
            row_compiler_intrinsic_executions: Vec::new(),
            row_external_completion_leaves: Vec::new(),
        })
        .collect();
    Ok((facts, provenance))
}

fn authored_selection_declarations(
    provenance: &ProviderSelectionProvenance,
) -> &[crate::ProviderSelection] {
    match provenance {
        ProviderSelectionProvenance::BuildOverride(declarations)
        | ProviderSelectionProvenance::TargetDefault(declarations) => declarations,
        ProviderSelectionProvenance::UniqueCoveringCandidate
        | ProviderSelectionProvenance::NativeBuild => &[],
    }
}

fn same_family_declaration(
    left: &crate::ProviderSelection,
    right: &crate::ProviderSelection,
) -> bool {
    left.subject.same_declaration_as(&right.subject)
        && left.provider_type.symbol == right.provider_type.symbol
}

/// Atomic family replay. Selection resolved each coordinate as its own slot
/// without the typed program, so a retained family row that is partial,
/// padded, stale, or substituted could otherwise select its satisfiable
/// subset while the omitted coordinate falls to the unique covering
/// candidate. Here every family declaration is replayed once against the
/// canonical roster the typed program derives now, and every coordinate must
/// be realized by exactly one selected plan of the declared provider.
fn replay_operator_family_selections(
    context: crate::ProviderPlanningContext<'_>,
    selected: &[SelectedProviderPlanWithProvenance],
) -> Vec<diagnostics::Diagnostic> {
    let mut families: Vec<&crate::ProviderSelection> = Vec::new();
    for selected_plan in selected {
        for declaration in authored_selection_declarations(&selected_plan.selected_by) {
            if matches!(
                declaration.subject,
                crate::ProviderSelectionSubject::BoundaryOperatorFamily(_)
            ) && !families
                .iter()
                .any(|existing| same_family_declaration(existing, declaration))
            {
                families.push(declaration);
            }
        }
    }
    let mut diagnostics = Vec::new();
    for declaration in families {
        let crate::ProviderSelectionSubject::BoundaryOperatorFamily(family) = &declaration.subject
        else {
            continue;
        };
        diagnostics.extend(
            family
                .replay_against_typed(context)
                .into_iter()
                .map(diagnostics::Diagnostic::error),
        );
        for coordinate in family.coordinates() {
            let realized = selected
                .iter()
                .filter(|selected_plan| {
                    selected_plan.derived.provenance.schema.symbol() == coordinate.symbol
                        && selected_plan
                            .derived
                            .plan
                            .schema
                            .identity_text(selected_plan.derived.plan.schema.trait_name)
                            == coordinate.requirement_identity
                        && selected_plan.derived.provenance.provider_type
                            == Some(declaration.provider_type.symbol)
                        && authored_selection_declarations(&selected_plan.selected_by)
                            .iter()
                            .any(|retained| same_family_declaration(retained, declaration))
                })
                .count();
            if realized != 1 {
                diagnostics.push(diagnostics::Diagnostic::error(format!(
                    "boundary-operator family `{}` selection resolves coordinate `{}` to {realized} selected plans of provider `{}`; every coordinate is realized by exactly one",
                    family.authored_path,
                    coordinate.requirement_identity,
                    declaration.provider_type.authored_path,
                )));
            }
        }
    }
    diagnostics
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SelectedProviderPlanIndex {
    pub(crate) candidate: usize,
    pub(crate) selected_by: ProviderSelectionProvenance,
}

fn resolve_provider_selection_slots<'selection>(
    slot_keys: &[ProviderSelectionKey],
    declarations: impl IntoIterator<Item = &'selection crate::ProviderSelection>,
    owner: &str,
) -> (
    Vec<(ProviderSelectionKey, crate::ProviderSelection)>,
    Vec<diagnostics::Diagnostic>,
) {
    let mut resolved = Vec::new();
    let mut diagnostics = Vec::new();
    for declaration in declarations {
        for slot_key in selected_subject_keys(declaration) {
            if slot_keys.contains(&slot_key) {
                resolved.push((slot_key, declaration.clone()));
            } else {
                // These keys are derived candidate plans, not a declaration
                // inventory. Absence here proves missing supply, never that a
                // well-formed boundary declaration did not exist.
                let message = format!(
                    "{owner} selects provider `{}` for slot `{}`, but no candidate exists in the loaded dependency closure",
                    declaration.provider_type.authored_path, slot_key.1,
                );
                diagnostics.push(diagnostics::Diagnostic::error(message));
            }
        }
    }
    (resolved, diagnostics)
}

/// Defaults are supplier preferences rather than executable roots. Their
/// declaration-level consistency remains mandatory even for an undemanded slot;
/// an explicit owner override has the same precedence as ordinary selection.
pub(crate) fn validate_provider_default_declarations(
    selected_target: target::TargetProfile,
    defaults: &[crate::ProviderSelection],
    requested: &[crate::ProviderSelection],
) -> Result<(), Vec<diagnostics::Diagnostic>> {
    let applies = |selection: &&crate::ProviderSelection| {
        selection
            .target
            .is_none_or(|target| target.canonical_name() == selected_target.target_name())
    };
    let overrides: std::collections::BTreeSet<_> = requested
        .iter()
        .filter(applies)
        .flat_map(selected_subject_keys)
        .collect();
    let mut slots =
        std::collections::BTreeMap::<ProviderSelectionKey, Vec<ProviderSelectionKey>>::new();
    for selection in defaults.iter().filter(applies) {
        for key in selected_subject_keys(selection) {
            if !overrides.contains(&key) {
                slots
                    .entry(key)
                    .or_default()
                    .push(selected_provider_key(selection));
            }
        }
    }
    let mut diagnostics = Vec::new();
    for ((_, slot), mut providers) in slots {
        providers.sort_unstable();
        providers.dedup();
        if providers.len() > 1 {
            diagnostics.push(diagnostics::Diagnostic::error(format!(
                "slot `{slot}` has conflicting target-package defaults: {} -- a target supplies at most one default provider type per slot",
                providers.iter().map(|(_, provider)| format!("`{provider}`")).collect::<Vec<_>>().join(", "),
            )));
        }
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

pub(crate) fn select_provider_plan_indices(
    plans: &[effects::provider_plan::ProviderPlan],
    selected_target: target::TargetProfile,
    defaults: &[crate::ProviderSelection],
    requested: &[crate::ProviderSelection],
) -> Result<Vec<SelectedProviderPlanIndex>, Vec<diagnostics::Diagnostic>> {
    // Target inertness (the fail-canary host-portability convention): a
    // plan scoped to a NON-selected target is inert and never collides --
    // only plans that RESOLVE to the selected target participate.
    let applies = |target: &str| -> bool {
        if target.is_empty() {
            return true; // portable: every target
        }
        target == selected_target.target_name()
    };
    let mut diagnostics = Vec::new();
    let mut selected = Vec::new();
    let mut slot_keys: Vec<ProviderSelectionKey> = plans
        .iter()
        .filter(|plan| !plan.schema.methods.is_empty())
        .map(provider_slot_key)
        .collect();
    slot_keys.sort_unstable();
    slot_keys.dedup();

    // Provider selections arrive after ordinary name resolution. Preserve
    // that exact nominal identity: readable paths are diagnostic material and
    // may never repair or approximate a package-qualified identity.
    let applicable = |selection: &&crate::ProviderSelection| {
        selection
            .target
            .is_none_or(|target| target.canonical_name() == selected_target.target_name())
    };
    let (resolved_requests, request_diagnostics) =
        resolve_provider_selection_slots(&slot_keys, requested.iter().filter(applicable), "build");
    diagnostics.extend(request_diagnostics);
    let (resolved_defaults, default_diagnostics) = resolve_provider_selection_slots(
        &slot_keys,
        defaults.iter().filter(applicable),
        "target package",
    );
    diagnostics.extend(default_diagnostics);
    for slot_key in &slot_keys {
        let declarations = resolved_requests
            .iter()
            .filter(|(slot, _)| slot == slot_key)
            .map(|(_, declaration)| declaration)
            .collect::<Vec<_>>();
        if declarations.len() > 1 {
            let slot_name = &slot_key.1;
            diagnostics.push(diagnostics::Diagnostic::error(format!(
                "build declares provider selection for slot `{slot_name}` more than once: {}",
                declarations
                    .iter()
                    .map(|declaration| format!(
                        "`{} -> {}`",
                        declaration.subject.authored_path(),
                        declaration.provider_type.authored_path,
                    ))
                    .collect::<Vec<_>>()
                    .join(", "),
            )));
        }
    }

    // Authored whole choices settle before automatic slot inference. Among
    // automatic choices, a descendant's complete roster settles first. A
    // selected map can then supply an exact ancestor application without
    // selecting another map or unioning rows. This order is semantic, not the
    // alphabetical order of a trait's source name.
    let authored = |key: &ProviderSelectionKey| {
        resolved_requests
            .iter()
            .chain(&resolved_defaults)
            .any(|(slot, _)| slot == key)
    };
    let roster_size = |key: &ProviderSelectionKey| {
        plans
            .iter()
            .filter(|plan| provider_slot_key(plan) == *key && applies(&plan.target))
            .map(|plan| plan.schema.receiver_applications.len())
            .max()
            .unwrap_or(0)
    };
    let mut pending_slots = slot_keys
        .into_iter()
        .map(|key| {
            let is_authored = authored(&key);
            let receiver_count = roster_size(&key);
            (key, is_authored, receiver_count)
        })
        .collect::<Vec<_>>();
    pending_slots.sort_by(|left, right| {
        right
            .1
            .cmp(&left.1)
            .then_with(|| right.2.cmp(&left.2))
            .then_with(|| left.0.cmp(&right.0))
    });
    for (slot_key, _, _) in pending_slots {
        let slot_name = &slot_key.1;
        let explicit = resolved_requests
            .iter()
            .find(|(slot, _)| slot == &slot_key)
            .map(|(_, selection)| selection);
        let slot_defaults: Vec<_> = resolved_defaults
            .iter()
            .filter(|(slot, _)| slot == &slot_key)
            .map(|(_, selection)| selection)
            .collect();
        let candidates: Vec<(usize, &ProviderPlan)> = plans
            .iter()
            .enumerate()
            .filter(|(_, plan)| provider_slot_key(plan) == slot_key && applies(&plan.target))
            .collect();
        let covering: Vec<(usize, &ProviderPlan)> = candidates
            .iter()
            .copied()
            .filter(|(_, plan)| plan.covers_schema())
            .collect();

        let selected_declaration = if let Some(explicit) = explicit {
            // A slot-owner override intentionally replaces every target
            // default for this slot, including a default whose provider is
            // absent from the selected dependency closure.
            Some((
                "build",
                explicit,
                ProviderSelectionProvenance::BuildOverride(vec![explicit.clone()]),
            ))
        } else if let Some(first) = slot_defaults.first().copied() {
            let mut distinct_provider_types: Vec<ProviderSelectionKey> = slot_defaults
                .iter()
                .map(|selection| selected_provider_key(selection))
                .collect();
            distinct_provider_types.sort_unstable();
            distinct_provider_types.dedup();
            if distinct_provider_types.len() > 1 {
                diagnostics.push(diagnostics::Diagnostic::error(format!(
                    "slot `{slot_name}` has conflicting target-package defaults: {} -- a target supplies at most one default provider type per slot",
                    distinct_provider_types
                        .iter()
                        .map(|(_, provider)| format!("`{provider}`"))
                        .collect::<Vec<_>>()
                        .join(", "),
                )));
                continue;
            }
            Some((
                "target package",
                first,
                ProviderSelectionProvenance::TargetDefault(
                    slot_defaults
                        .iter()
                        .map(|selection| (*selection).clone())
                        .collect(),
                ),
            ))
        } else {
            None
        };

        if let Some((owner, declaration, selected_by)) = selected_declaration {
            let selected_provider = selected_provider_key(declaration);
            let matching: Vec<(usize, &ProviderPlan)> = candidates
                .iter()
                .copied()
                .filter(|(_, plan)| provider_plan_key(plan) == selected_provider)
                .collect();
            match matching.as_slice() {
                [(candidate, plan)] if plan.covers_schema() => {
                    selected.push(SelectedProviderPlanIndex {
                        candidate: *candidate,
                        selected_by,
                    });
                }
                [(_, plan)] => diagnostics.push(diagnostics::Diagnostic::error(format!(
                    "{owner} selects provider `{}` for slot `{slot_name}`, but candidate `{}` is partial ({}/{}) and cannot be selected",
                    declaration.provider_type.authored_path,
                    plan.name,
                    plan.rows.len(),
                    plan.schema.methods.len(),
                ))),
                [] => {
                    let wrong_target = plans.iter().any(|plan| {
                        provider_slot_key(plan) == slot_key
                            && provider_plan_key(plan) == selected_provider
                    });
                    diagnostics.push(diagnostics::Diagnostic::error(format!(
                        "{owner} selects provider `{}` for slot `{slot_name}`, but no {}candidate exists in the loaded dependency closure",
                        declaration.provider_type.authored_path,
                        if wrong_target { "selected-target " } else { "" },
                    )));
                }
                _ => diagnostics.push(diagnostics::Diagnostic::error(format!(
                    "{owner} selection `{}` for slot `{slot_name}` resolves to multiple provider candidates with the same exact identity",
                    declaration.provider_type.authored_path,
                ))),
            }
            continue;
        }

        if !candidates.is_empty() {
            let inherited = selected
                .iter()
                .filter(|selection| {
                    let whole = &plans[selection.candidate];
                    whole.covers_schema()
                        && candidates.iter().all(|(_, candidate)| {
                            whole.schema.supplies_receiver_schema(&candidate.schema)
                        })
                })
                .collect::<Vec<_>>();
            match inherited.as_slice() {
                [_] => continue,
                [] => {}
                _ => {
                    diagnostics.push(diagnostics::Diagnostic::error(format!(
                        "slot `{slot_name}` is supplied by multiple selected whole provider maps; an inherited receiver must retain one choosing owner",
                    )));
                    continue;
                }
            }
        }

        match covering.as_slice() {
            [] => {}
            [(candidate, _)] => selected.push(SelectedProviderPlanIndex {
                candidate: *candidate,
                selected_by: ProviderSelectionProvenance::UniqueCoveringCandidate,
            }),
            many => {
                let count = if many.len() == 2 {
                    "two".to_owned()
                } else {
                    many.len().to_string()
                };
                diagnostics.push(diagnostics::Diagnostic::error(format!(
                    "slot `{slot_name}` has {count} covering provider plans for the selected target: {} -- choose one in build.omg with `Build::select_provider<{slot_name}, ProviderType>(b);`",
                    many.iter()
                        .map(|(_, plan)| format!("`{}` [{:016x}]", plan.name, plan.report_fingerprint()))
                        .collect::<Vec<_>>()
                        .join(", "),
                )));
            }
        }
    }
    for (position, selection) in selected.iter().enumerate() {
        let whole = &plans[selection.candidate];
        for other in &selected[..position] {
            let other = &plans[other.candidate];
            if whole.schema.shares_receiver_application(&other.schema) {
                diagnostics.push(diagnostics::Diagnostic::error(format!(
                    "selected whole provider maps `{}` and `{}` supply the same exact receiver application; inherited rows cannot be mixed across choosing owners",
                    whole.name, other.name,
                )));
            }
        }
    }
    if diagnostics.is_empty() {
        Ok(selected)
    } else {
        Err(diagnostics)
    }
}

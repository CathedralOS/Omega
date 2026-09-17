//! Provider selection keys, provenance, indices and slot resolution.

use crate::provider_planning::independent_components::IndependentComponentJoin;
use crate::provider_planning::provenance_replay::validate_derived_provider_plan_provenance;
use crate::{DerivedProviderPlan, ProviderPlanProvenance};
use component_description::VerifiedComponent;
use effects::CompilerIntrinsicExecutionIdentity;
use effects::provider_plan::ProviderPlan;
use typed_trees::TypedTrees;

type ProviderSelectionKey = (Option<semantic_vocabulary::PackageKeyIdentity>, String);

fn provider_slot_key(plan: &effects::provider_plan::ProviderPlan) -> ProviderSelectionKey {
    (
        plan.schema.trait_package_identity,
        plan.schema.trait_name.clone(),
    )
}

fn selected_subject_keys(selection: &crate::ProviderSelection) -> Vec<ProviderSelectionKey> {
    match &selection.subject {
        crate::ProviderSelectionSubject::BoundaryTrait(identity)
        | crate::ProviderSelectionSubject::BoundaryRequirement(identity) => {
            vec![(identity.package, identity.canonical_path.clone())]
        }
        crate::ProviderSelectionSubject::BoundaryOperatorFamily(family) => family
            .coordinates()
            .iter()
            .map(|coordinate| (family.package, coordinate.requirement_identity.clone()))
            .collect(),
    }
}

fn selected_provider_key(selection: &crate::ProviderSelection) -> ProviderSelectionKey {
    (
        selection.provider_type.package,
        selection.provider_type.canonical_path.clone(),
    )
}

fn provider_plan_key(plan: &effects::provider_plan::ProviderPlan) -> ProviderSelectionKey {
    (
        plan.provider_type_package_identity,
        plan.provider_type.clone(),
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderSelectionProvenance {
    BuildOverride(Vec<crate::ProviderSelection>),
    TargetDefault(Vec<crate::ProviderSelection>),
    UniqueCoveringCandidate,
}

impl ProviderSelectionProvenance {
    /// Reconstruct the one build-owned composition choice retained by this
    /// selected plan. Automatic unique selection and target defaults are
    /// always fused: provider packages cannot independently componentize
    /// themselves.
    pub fn composition_mode(&self) -> Result<crate::CompositionMode, String> {
        match self {
            Self::UniqueCoveringCandidate => Ok(crate::CompositionMode::Fused),
            Self::TargetDefault(declarations) => {
                if declarations.iter().any(|declaration| {
                    declaration.composition_mode != crate::CompositionMode::Fused
                }) {
                    return Err(
                        "target-provider defaults cannot request independent composition; only the owner-controlled build may create that deployment cut"
                            .into(),
                    );
                }
                Ok(crate::CompositionMode::Fused)
            }
            Self::BuildOverride(declarations) => {
                let Some(first) = declarations.first() else {
                    return Err(
                        "selected provider plan has a build override without a declaration".into(),
                    );
                };
                if declarations
                    .iter()
                    .any(|declaration| declaration.composition_mode != first.composition_mode)
                {
                    return Err(
                        "selected provider plan has conflicting build-owned composition modes"
                            .into(),
                    );
                }
                Ok(first.composition_mode)
            }
        }
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
    /// Closed compiler-owned execution identity for each provider row.
    ///
    /// Selection initially leaves this empty because exact execution is not
    /// settled until after checking. The compiler must replace it with one
    /// row-aligned entry per plan row before publishing `CheckedCompilation`.
    /// `Some` is reserved for compiler-intrinsic rows whose selected
    /// execution has a closed identity; all other rows retain `None`.
    pub row_compiler_intrinsic_executions: Vec<Option<CompilerIntrinsicExecutionIdentity>>,
}

/// Replay selected provider provenance on a route that supplies no verified
/// component descriptions. Every `Independent` selection rejects at the
/// component-closure fence; routes that admit verified components call
/// [`selected_provider_plan_facts_with_independent_components`].
pub fn selected_provider_plan_facts(
    typed: &TypedTrees,
    evaluated_bindings: &crate::evaluated_via_bindings::EvaluatedViaBindingTable,
    selected: Vec<SelectedProviderPlanWithProvenance>,
) -> Result<
    (
        effects::SelectedProviderPlanFacts,
        Vec<SelectedProviderReviewProvenance>,
    ),
    Vec<diagnostics::Diagnostic>,
> {
    selected_provider_plan_facts_with_independent_components(
        typed,
        evaluated_bindings,
        selected,
        &[],
    )
}

/// Replay selected provider provenance and close every `Independent`
/// selection against the build's verified component descriptions.
///
/// Each independently composed plan must be realized by exactly one
/// `independent_components` entry (`VerifiedComponent::realizes_selected_plan`),
/// and each supplied component must realize one such plan; the selected
/// facts publish only when that join is complete.
pub fn selected_provider_plan_facts_with_independent_components(
    typed: &TypedTrees,
    evaluated_bindings: &crate::evaluated_via_bindings::EvaluatedViaBindingTable,
    mut selected: Vec<SelectedProviderPlanWithProvenance>,
    independent_components: &[VerifiedComponent],
) -> Result<
    (
        effects::SelectedProviderPlanFacts,
        Vec<SelectedProviderReviewProvenance>,
    ),
    Vec<diagnostics::Diagnostic>,
> {
    selected.sort_by(|left, right| {
        let left = &left.derived.plan;
        let right = &right.derived.plan;
        left.name
            .cmp(&right.name)
            .then_with(|| {
                left.origin_package_identity
                    .cmp(&right.origin_package_identity)
            })
            .then_with(|| {
                left.provider_type_package_identity
                    .cmp(&right.provider_type_package_identity)
            })
            .then_with(|| {
                left.schema
                    .trait_package_identity
                    .cmp(&right.schema.trait_package_identity)
            })
            .then_with(|| left.report_fingerprint().cmp(&right.report_fingerprint()))
    });

    let mut diagnostics = evaluated_bindings
        .validate_against_typed(typed)
        .err()
        .unwrap_or_default();
    let retained_target = evaluated_bindings
        .target()
        .map(target::TargetProfile::target_name)
        .unwrap_or_default();
    let mut independent_join = IndependentComponentJoin::new(independent_components);
    for selected_plan in &selected {
        let plan = &selected_plan.derived.plan;
        let provenance = &selected_plan.derived.provenance;
        let composition_mode = match selected_plan.selected_by.composition_mode() {
            Ok(mode) => mode,
            Err(reason) => {
                diagnostics.push(diagnostics::Diagnostic::error(reason));
                continue;
            }
        };
        // Component-closure fence. An `Independent` edge closes only when
        // exactly one verified component description realizes this plan
        // (`VerifiedComponent::realizes_selected_plan` matches the selected
        // provider type and every checked-adapter row against the verified
        // module's provider-candidate catalog). The join reads the verified
        // carrier itself, never a projection of its fields; a missing,
        // duplicated, or mismatched realization rejects here rather than
        // falling back to Fused.
        if composition_mode == crate::CompositionMode::Independent
            && let Err(diagnostic) = independent_join.realize(plan)
        {
            diagnostics.push(diagnostic);
        }
        let schema_symbol = provenance.schema.symbol();
        if plan.target != retained_target {
            diagnostics.push(diagnostics::Diagnostic::error(format!(
                "selected ProviderPlan `{}` target `{}` disagrees with evaluated-binding target `{retained_target}`",
                plan.name, plan.target,
            )));
        }
        diagnostics.extend(validate_derived_provider_plan_provenance(
            typed,
            evaluated_bindings,
            &selected_plan.derived,
        ));
        let declarations = match &selected_plan.selected_by {
            ProviderSelectionProvenance::BuildOverride(declarations)
            | ProviderSelectionProvenance::TargetDefault(declarations) => declarations,
            ProviderSelectionProvenance::UniqueCoveringCandidate => continue,
        };
        if declarations.is_empty() {
            diagnostics.push(diagnostics::Diagnostic::error(format!(
                "selected provider plan `{}` has an authored selection origin without a declaration",
                plan.name,
            )));
        }
        for declaration in declarations {
            let selecting_source = typed
                .symbols
                .symbol_provenance_source_span(declaration.selecting_machine);
            if !declaration
                .subject
                .selects_schema(schema_symbol, &plan.schema.trait_name)
                || Some(declaration.provider_type.symbol) != provenance.provider_type
                || selecting_source.is_none_or(|source| {
                    source.source_id != declaration.source_span.source_id
                        || declaration.source_span.span.start >= declaration.source_span.span.end
                })
            {
                diagnostics.push(diagnostics::Diagnostic::error(format!(
                    "selected provider plan `{}` has a selection declaration outside its exact schema, provider, or selecting-machine provenance",
                    plan.name,
                )));
            }
        }
    }
    diagnostics.extend(independent_join.finish());
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    let plans = selected
        .iter()
        .map(|selected| selected.derived.plan.clone())
        .collect::<Vec<_>>();
    let facts = effects::SelectedProviderPlanFacts::from_selected_plans(plans.clone())
        .map_err(|reason| vec![diagnostics::Diagnostic::error(reason)])?;
    if facts.plans() != plans
        || plans.iter().any(|plan| {
            facts
                .plan_by_exact_evidence(plan.report_fingerprint(), plan)
                .is_none()
        })
    {
        return Err(vec![diagnostics::Diagnostic::error(
            "selected provider semantic facts do not retain the exact plans aligned with provenance",
        )]);
    }
    let provenance = selected
        .into_iter()
        .map(|selected| SelectedProviderReviewProvenance {
            plan: selected.derived.plan,
            provider: selected.derived.provenance,
            selected_by: selected.selected_by,
            row_compiler_intrinsic_executions: Vec::new(),
        })
        .collect();
    Ok((facts, provenance))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SelectedProviderPlanIndex {
    pub(crate) candidate: usize,
    pub(crate) selected_by: ProviderSelectionProvenance,
}

fn resolve_provider_selection_slots(
    slot_keys: &[ProviderSelectionKey],
    declarations: &[crate::ProviderSelection],
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
                let message = match &declaration.subject {
                    crate::ProviderSelectionSubject::BoundaryTrait(identity) => format!(
                        "{owner} selects provider `{}` for unknown boundary slot `{}`; the slot must exist in the loaded dependency closure",
                        declaration.provider_type.authored_path, identity.authored_path,
                    ),
                    crate::ProviderSelectionSubject::BoundaryRequirement(identity) => format!(
                        "{owner} selects provider `{}` for unknown top-level boundary requirement `{}`; the requirement must exist in the loaded dependency closure",
                        declaration.provider_type.authored_path, identity.authored_path,
                    ),
                    crate::ProviderSelectionSubject::BoundaryOperatorFamily(_) => format!(
                        "{owner} selects provider `{}` for unknown boundary coordinate `{}` in subject `{}`; every selected coordinate must exist in the loaded dependency closure",
                        declaration.provider_type.authored_path,
                        slot_key.1,
                        declaration.subject.authored_path(),
                    ),
                };
                diagnostics.push(diagnostics::Diagnostic::error(message));
            }
        }
    }
    (resolved, diagnostics)
}

pub(crate) fn select_provider_plan_indices(
    plans: &[effects::provider_plan::ProviderPlan],
    selected_target: target::NativeTarget,
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
        target::NativeTarget::from_omega_target_name(Some(target))
            .is_ok_and(|resolved| resolved == selected_target)
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
    let (resolved_requests, request_diagnostics) =
        resolve_provider_selection_slots(&slot_keys, requested, "build");
    diagnostics.extend(request_diagnostics);
    let (resolved_defaults, default_diagnostics) =
        resolve_provider_selection_slots(&slot_keys, defaults, "target package");
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

    for slot_key in slot_keys {
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
                    "slot `{slot_name}` has {count} covering provider plans for the selected target: {} -- choose one in build.omg with `b.select_provider<{slot_name}, ProviderType>();`",
                    many.iter()
                        .map(|(_, plan)| format!("`{}` [{:016x}]", plan.name, plan.report_fingerprint()))
                        .collect::<Vec<_>>()
                        .join(", "),
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

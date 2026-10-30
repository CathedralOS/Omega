//! One whole-provider choice policy for invocation and canonical declarations.

use crate::{ProviderDeclaration, ProviderSubject, SelectionProvenance};
use effects::provider_plan::ProviderPlan;

type ProviderSelectionKey = (
    Option<semantic_vocabulary::PackageKeyIdentity>,
    String,
    Option<String>,
);

fn provider_slot_key(plan: &ProviderPlan) -> ProviderSelectionKey {
    (
        plan.schema.declaration.trait_package_identity,
        plan.schema
            .identity_text(plan.schema.declaration.trait_name)
            .to_owned(),
        plan.schema.root_application_identity().map(str::to_owned),
    )
}

fn selected_subject_keys<Declaration: PartialEq, Origin>(
    selection: &ProviderDeclaration<Declaration, Origin>,
    slots: &[ProviderSelectionKey],
) -> Vec<ProviderSelectionKey> {
    match &selection.subject {
        ProviderSubject::BoundaryTrait(identity)
        | ProviderSubject::BoundaryRequirement(identity) => {
            if let Some(application) = &identity.application_identity {
                return vec![(
                    identity.package,
                    identity.canonical_path.clone(),
                    Some(application.clone()),
                )];
            }
            // A bare declaration has no Type/Const telescope. Its public
            // regions remain universal; the checked provider's complete schema
            // owns each receiving view. Closed applications never use this lane.
            let matched = slots
                .iter()
                .filter(|slot| slot.0 == identity.package && slot.1 == identity.canonical_path)
                .cloned()
                .collect::<Vec<_>>();
            if matched.is_empty() {
                vec![(identity.package, identity.canonical_path.clone(), None)]
            } else {
                matched
            }
        }
        ProviderSubject::BoundaryOperatorFamily(family) => family
            .coordinates
            .iter()
            .map(|coordinate| {
                (
                    family.package,
                    coordinate.requirement_identity.clone(),
                    None,
                )
            })
            .collect(),
    }
}

fn selected_provider_key<Declaration, Origin>(
    selection: &ProviderDeclaration<Declaration, Origin>,
) -> ProviderSelectionKey {
    (
        selection.provider_type.package,
        selection.provider_type.canonical_path.clone(),
        selection.provider_type.application_identity.clone(),
    )
}

fn provider_plan_key(plan: &ProviderPlan) -> ProviderSelectionKey {
    (
        plan.provider_type_package_identity,
        plan.provider_type.clone(),
        plan.application_identity().map(str::to_owned),
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SelectedProviderPlanIndex<Declaration, Origin> {
    pub(crate) candidate: usize,
    pub(crate) selected_by: SelectionProvenance<Declaration, Origin>,
}

fn resolve_provider_selection_slots<
    'selection,
    Declaration: Clone + Eq + 'selection,
    Origin: Clone + Eq + 'selection,
>(
    slot_keys: &[ProviderSelectionKey],
    declarations: impl IntoIterator<Item = &'selection ProviderDeclaration<Declaration, Origin>>,
    owner: &str,
) -> (
    Vec<(
        ProviderSelectionKey,
        ProviderDeclaration<Declaration, Origin>,
    )>,
    Vec<diagnostics::Diagnostic>,
) {
    let mut resolved = Vec::new();
    let mut diagnostics = Vec::new();
    for declaration in declarations {
        for slot_key in selected_subject_keys(declaration, slot_keys) {
            if slot_keys.contains(&slot_key) {
                resolved.push((slot_key, declaration.clone()));
            } else {
                let message = match &declaration.subject {
                    ProviderSubject::BoundaryTrait(identity) => format!(
                        "{owner} selects provider `{}` for unknown boundary slot `{}`; the slot must exist in the loaded dependency closure",
                        declaration.provider_type.authored_path, identity.authored_path,
                    ),
                    ProviderSubject::BoundaryRequirement(identity) => format!(
                        "{owner} selects provider `{}` for unknown top-level boundary requirement `{}`; the requirement must exist in the loaded dependency closure",
                        declaration.provider_type.authored_path, identity.authored_path,
                    ),
                    ProviderSubject::BoundaryOperatorFamily(_) => format!(
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

pub(crate) fn select_provider_plan_indices<Declaration: Clone + Eq, Origin: Clone + Eq>(
    plans: &[effects::provider_plan::ProviderPlan],
    selected_target: target::TargetProfile,
    defaults: &[ProviderDeclaration<Declaration, Origin>],
    requested: &[ProviderDeclaration<Declaration, Origin>],
) -> Result<Vec<SelectedProviderPlanIndex<Declaration, Origin>>, Vec<diagnostics::Diagnostic>> {
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
        .filter(|plan| !plan.schema.declaration.methods.is_empty())
        .map(provider_slot_key)
        .collect();
    slot_keys.sort_unstable();
    slot_keys.dedup();

    // Provider selections arrive after ordinary name resolution. Preserve
    // that exact nominal identity: readable paths are diagnostic material and
    // may never repair or approximate a package-qualified identity.
    let applicable = |selection: &&ProviderDeclaration<Declaration, Origin>| {
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
            .map(|plan| plan.schema.declaration.receiver_applications.len())
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
                SelectionProvenance::BuildOverride(vec![explicit.clone()]),
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
                        .map(|(_, provider, application)| format!("`{provider}`{}", application.as_deref().map(|application| format!("<{application}>")).unwrap_or_default()))
                        .collect::<Vec<_>>()
                        .join(", "),
                )));
                continue;
            }
            Some((
                "target package",
                first,
                SelectionProvenance::TargetDefault(
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
                    plan.schema.declaration.methods.len(),
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
                selected_by: SelectionProvenance::UniqueCoveringCandidate,
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

//! Physical source dependencies use the same whole-map chooser as Native.
//!
//! The shared invocation graph supplies actual calls, never candidate roots.
//! Complete maps are selected for those demanded receiving applications; only
//! their exact member bindings extend invocation. Unresolved choices remain
//! explicit descriptions, not successful empty selection or deployment approval.
//! The original choices still cross the mandatory Native publication/admission
//! boundary, which resolves output-dependent ABI correspondence before storage.

mod calls;
mod choices;
mod inventory;
mod schemas;

use super::{ProviderPlanningContext, ProviderSelection, TargetProviderDefaults};
use language_semantics::declaration_selection::provider_choices::{self, Declaration as _, Error};
use schemas::Key;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;

/// Source uncertainty retains the active input coordinates so its errors never
/// masquerade as an empty successful choice. It is not installed authority.
#[derive(Debug)]
pub(crate) struct PendingSelection {
    pub candidate_applications: Vec<(Key, Key)>,
    pub errors: Vec<Error<Key, Key>>,
    pub calls: Vec<(SymbolHandle, Key)>,
}

pub(crate) enum SourceProviderDependencies {
    Complete {
        machines: Vec<SymbolHandle>,
    },
    Unresolved {
        machines: Vec<SymbolHandle>,
        pending: Vec<PendingSelection>,
    },
}

pub(crate) fn source_application_dependencies(
    typed: &TypedTrees,
    target: &str,
    roots: &[SymbolHandle],
    overrides: &[ProviderSelection],
    package_defaults: &[ProviderSelection],
    defaults: &[TargetProviderDefaults],
    topology: &semantic_vocabulary::proof_applications::ProofApplicationPlan<SymbolHandle>,
    operational: &flow_effects::OperationalPlan,
) -> Result<SourceProviderDependencies, String> {
    let context = ProviderPlanningContext::new(typed, source::DependencyScope::Product);
    let inventory = inventory::prepare(context, topology, target);
    let overrides = overrides
        .iter()
        .filter(|row| {
            row.target
                .is_none_or(|scope| scope.canonical_name() == target)
        })
        .map(|row| choices::Choice::new(typed, row))
        .collect::<Result<Vec<_>, _>>()?;
    let defaults = defaults
        .iter()
        .filter(|row| row.target.as_deref().is_none_or(|scope| scope == target))
        .flat_map(|row| &row.selections)
        .chain(package_defaults)
        .filter(|row| {
            row.target
                .is_none_or(|scope| scope.canonical_name() == target)
        })
        .map(|row| choices::Choice::new(typed, row))
        .collect::<Result<Vec<_>, _>>()?;
    let applications =
        semantic_vocabulary::proof_applications::ProofApplications::new(topology, operational);
    let mut dependencies = Vec::new();
    let mut export_roots = roots.to_vec();
    let all_slots = inventory
        .iter()
        .map(|candidate| candidate.schema.key.clone())
        .collect::<Vec<_>>();
    let exported_slots = if roots.is_empty() {
        overrides
            .iter()
            .flat_map(|choice| choice.slots(&all_slots))
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    loop {
        let reached = applications.invocation_reachability(target, &dependencies, &export_roots);
        let demands = calls::collect(typed, &reached.machines, &reached.calls);
        let mut active_slots = inventory
            .iter()
            .filter(|candidate| {
                exported_slots.contains(&candidate.schema.key)
                    || demands
                        .iter()
                        .any(|demand| can_serve(&candidate.schema, &demand.schema) != Some(false))
            })
            .map(|candidate| candidate.schema.key.clone())
            .collect::<Vec<_>>();
        for slot in demands
            .iter()
            .map(|demand| &demand.schema.key)
            .chain(&exported_slots)
        {
            if !active_slots.contains(slot) {
                active_slots.push(slot.clone());
            }
        }
        // An authored operator-family choice names its entire coordinate
        // roster. Describe every coordinate for coverage/choice, while only
        // actual calls below can activate executable member bodies.
        loop {
            let mut grew = false;
            for choice in overrides
                .iter()
                .chain(&defaults)
                .filter(|choice| choice.is_family())
            {
                let slots = choice.slots(&all_slots);
                if slots.iter().any(|slot| active_slots.contains(slot)) {
                    for slot in slots {
                        if !active_slots.contains(&slot) {
                            active_slots.push(slot);
                            grew = true;
                        }
                    }
                }
            }
            if !grew {
                break;
            }
        }
        let candidates = inventory
            .iter()
            .filter(|candidate| active_slots.contains(&candidate.schema.key))
            .collect::<Vec<_>>();
        let relevant = |choice: &&choices::Choice<'_>| {
            choice
                .slots(&active_slots)
                .iter()
                .any(|slot| active_slots.contains(slot))
        };
        let chosen_overrides = overrides.iter().filter(relevant).collect::<Vec<_>>();
        let chosen_defaults = defaults.iter().filter(relevant).collect::<Vec<_>>();
        let outcome =
            provider_choices::choose(&candidates, target, &chosen_defaults, &chosen_overrides);
        // Native ultimately rejects every errored settlement. Source may retain
        // unrelated definite descriptions, but a repeated override for this
        // very slot cannot lend the arbitrary first candidate any geometry.
        let invalid_slot = |key: &Key| {
            outcome.errors.iter().any(|error| match error {
                Error::RepeatedOverride { slot, .. } | Error::ConflictingDefaults { slot, .. } => {
                    slot == key
                }
                _ => false,
            })
        };
        let definite = outcome
            .choices
            .iter()
            .filter(|choice| !invalid_slot(&candidates[choice.candidate].schema.key))
            .map(|choice| candidates[choice.candidate])
            .collect::<Vec<_>>();
        let mut changed = false;
        if roots.is_empty() {
            for candidate in &definite {
                if !exported_slots.contains(&candidate.schema.key) {
                    continue;
                }
                for &(_, member) in &candidate.rows {
                    if !export_roots.contains(&member) {
                        export_roots.push(member);
                        changed = true;
                    }
                }
            }
        }
        let mut unresolved_calls = Vec::new();
        for demand in &demands {
            let matching = definite
                .iter()
                .copied()
                .filter(|candidate| can_serve(&candidate.schema, &demand.schema) == Some(true))
                .collect::<Vec<_>>();
            let member = match matching.as_slice() {
                [whole] => whole.member_for(typed, demand.called, demand.requirement),
                _ => None,
            };
            match member {
                Some(member) => {
                    if !dependencies.contains(&(demand.called, member)) {
                        dependencies.push((demand.called, member));
                        changed = true;
                    }
                }
                None => unresolved_calls.push((demand.called, demand.schema.key.clone())),
            }
        }
        if !changed {
            return Ok(
                if outcome.errors.is_empty() && unresolved_calls.is_empty() {
                    SourceProviderDependencies::Complete {
                        machines: reached.machines,
                    }
                } else {
                    SourceProviderDependencies::Unresolved {
                        machines: reached.machines,
                        pending: vec![PendingSelection {
                            candidate_applications: candidates
                                .iter()
                                .map(|candidate| {
                                    (candidate.schema.key.clone(), candidate.provider.clone())
                                })
                                .collect(),
                            errors: outcome.errors,
                            calls: unresolved_calls,
                        }],
                    }
                },
            );
        }
    }
}

fn can_serve(whole: &schemas::Schema, demanded: &schemas::Schema) -> Option<bool> {
    if whole.key == demanded.key {
        Some(true)
    } else {
        whole.supplies(demanded)
    }
}

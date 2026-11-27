use super::native_binding_declarations::NativeBindingSelection;
use super::provider_settlement::ChosenDefaultLayout;
use super::source_targets::{SourceTarget, target_named, target_owner};
use crate::RootBinding;
use crate::compile_time_invocations::FixedLayoutTargetScope;
use checked_program_to_psi::provider_choice_inputs::ProviderSelection;
use diagnostics::Diagnostic;
use typed_trees::source_state::TypedTrees;

/// The targets whose layouts this source prepares, each with the exact roots
/// whose storage it lays out: the entries bound for that target, the Build's
/// own provider selections for it, and the dependency defaults that target
/// chose (`chosen_defaults`). A default no target chose is never a root, so
/// an imported default cannot request geometry, or add its target, before a
/// target chooses it (wiki/spec/layouts/plans.md#layout-demand-and-validation).
/// `claimed` are the native bindings this compilation claims: each is a
/// layout root on its own target, because its requirement's signature is
/// read in full (`prepare_and_judge`) and a type there laid out by a policy
/// (`[CLayout<DarwinTimespec>; 2]`) has a layout only for the target the
/// binding names.
pub fn layout_target_scopes(
    typed: &TypedTrees,
    root_bindings: &[RootBinding],
    provider_selections: &[ProviderSelection],
    chosen_defaults: &[ChosenDefaultLayout],
    claimed: &[&NativeBindingSelection],
    targets: &[SourceTarget],
    checked_targets: &[&str],
) -> Result<Vec<FixedLayoutTargetScope>, Vec<Diagnostic>> {
    let mut profiles = Vec::new();
    for binding in root_bindings {
        let (owner, _) = binding.slot.rsplit_once("::").ok_or_else(|| {
            vec![Diagnostic::error(
                "fixed representation scope has an invalid Build root slot",
            )]
        })?;
        let profile = target_owner(targets, owner).map_err(|error| vec![error])?;
        include_profile(&mut profiles, profile);
    }
    for choice in provider_selections {
        if let Some(target) = choice.target {
            let profile =
                target_named(targets, target.canonical_name()).map_err(|error| vec![error])?;
            include_profile(&mut profiles, profile);
        }
    }
    for chosen in chosen_defaults {
        let profile = target_named(targets, &chosen.target).map_err(|error| vec![error])?;
        include_profile(&mut profiles, profile);
    }
    for selection in claimed {
        let profile = target_named(targets, selection.target.canonical_name())
            .map_err(|error| vec![error])?;
        include_profile(&mut profiles, profile);
    }
    if profiles.is_empty() {
        profiles.extend(targets.iter().filter(|profile| profile.layout.is_some()));
    }
    for target in checked_targets {
        let profile = target_named(targets, target).map_err(|error| vec![error])?;
        include_profile(&mut profiles, profile);
    }
    for machine in typed.machines() {
        for target in [
            typed.machine_authored_target(machine),
            typed.source_target_context(machine),
        ]
        .into_iter()
        .flatten()
        {
            let profile = target_named(targets, target).map_err(|error| vec![error])?;
            include_profile(&mut profiles, profile);
        }
    }
    let mut scopes = Vec::new();
    for profile in profiles {
        let mut entries = Vec::new();
        for binding in root_bindings {
            let (owner, _) = binding
                .slot
                .rsplit_once("::")
                .expect("root slots were validated above");
            let declared = target_owner(targets, owner).map_err(|error| vec![error])?;
            if declared.name == profile.name && !entries.contains(&binding.implementation_symbol) {
                entries.push(binding.implementation_symbol);
            }
        }
        for selection in claimed {
            if selection.target.canonical_name() != profile.name {
                continue;
            }
            for symbol in [selection.subject, selection.requirement] {
                if !entries.contains(&symbol) {
                    entries.push(symbol);
                }
            }
        }
        let mut providers = provider_selections
            .iter()
            .filter(|choice| {
                choice
                    .target
                    .is_none_or(|target| target.canonical_name() == profile.name)
            })
            .cloned()
            .collect::<Vec<_>>();
        for chosen in chosen_defaults {
            if chosen.target == profile.name && !providers.contains(&chosen.selection) {
                providers.push(chosen.selection.clone());
            }
        }
        scopes.push(FixedLayoutTargetScope {
            target: profile.name.to_owned(),
            facts: profile.layout.clone(),
            checks_subject: checked_targets.contains(&profile.name),
            entry_declarations: entries,
            provider_declarations: providers,
        });
    }
    Ok(scopes)
}

fn include_profile<'a>(profiles: &mut Vec<&'a SourceTarget>, profile: &'a SourceTarget) {
    if !profiles.iter().any(|known| known.name == profile.name) {
        profiles.push(profile);
    }
}

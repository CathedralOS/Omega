//! Layout applicability follows authored Build entries and exact provider rows.
//! This walk borrows the shared checked graph; retaining an unrelated type or
//! policy declaration does not make it executable in another target capsule.
use super::{ConstantProviders, Diagnostic};
use build_time_evaluation::LayoutApplicationTargetInput;
use compilation_products::CheckedProductSource;
use provider_planning::SelectedProviderReviewProvenance;
use symbols::SymbolHandle;
use target::TargetProfile;

#[derive(Default)]
pub(super) struct CollectedApplications {
    pub layouts: Vec<LayoutApplicationTargetInput>,
    pub roots: Vec<checked_trees::applications::TargetApplicationRoots>,
}

pub(super) fn collect(
    source: &CheckedProductSource,
    selected: TargetProfile,
    selected_provenance: &[SelectedProviderReviewProvenance],
    supplemental: &mut Vec<(TargetProfile, ConstantProviders)>,
    supply: &mut impl FnMut(TargetProfile) -> Result<ConstantProviders, Vec<Diagnostic>>,
) -> Result<CollectedApplications, Vec<Diagnostic>> {
    let build = &source.computed_build_config.config;
    let mut targets = Vec::new();
    for binding in &build.root_bindings {
        let (owner, _) = binding.slot.rsplit_once("::").ok_or_else(|| {
            vec![Diagnostic::error(
                "layout application has an invalid Build entry slot",
            )]
        })?;
        let profile = TargetProfile::from_root_slot_owner(owner).map_err(|reason| vec![reason])?;
        if !targets.contains(&profile) {
            targets.push(profile);
        }
    }
    for choice in build
        .provider_selections
        .iter()
        .chain(&build.package_provider_selections)
    {
        if let Some(target) = choice.target {
            let profile = TargetProfile::from_canonical_target_name(target.canonical_name())
                .map_err(|reason| vec![reason])?;
            if !targets.contains(&profile) {
                targets.push(profile);
            }
        }
    }
    let unscoped = targets.is_empty();
    if unscoped {
        // A library or standalone checking input declares no narrower Build
        // application capsule. Its reusable surface must form for each
        // deployable profile provided by the compiler's target catalog.
        targets.extend(
            TargetProfile::ALL
                .into_iter()
                .filter(|profile| profile.layout_target_facts().is_some()),
        );
    }
    let mut inputs = Vec::new();
    let mut application_roots = Vec::new();
    for profile in targets {
        let targeted_choices = build
            .provider_selections
            .iter()
            .chain(&build.package_provider_selections)
            .any(|choice| {
                choice
                    .target
                    .is_none_or(|target| target.canonical_name() == profile.target_name())
            });
        // Unscoped checking already retains every ordinary declaration. With
        // no authored target choice, ordinary checked defaults are portable;
        // native catalog rows have no Source body to add to this walk.
        let needs_selection = !unscoped && targeted_choices;
        if profile != selected
            && needs_selection
            && !supplemental.iter().any(|(known, _)| *known == profile)
        {
            supplemental.push((profile, supply(profile)?));
        }
        let provenance = if profile == selected || !needs_selection {
            selected_provenance
        } else {
            &supplemental
                .iter()
                .find(|(known, _)| *known == profile)
                .expect("supplied target retained")
                .1
                .provenance
        };
        let edges = provider_edges(provenance);
        let mut roots = Vec::new();
        if build.root_bindings.iter().any(|binding| {
            binding.slot.rsplit_once("::").is_some_and(|(owner, _)| {
                TargetProfile::from_root_slot_owner(owner).ok() == Some(profile)
            })
        }) {
            if let Some(entry) = provider_planning::program_entry::selected_program_entry_machine(
                &build.root_bindings,
                Some(profile),
            )? {
                roots.push(entry.machine_symbol);
            }
        }
        // An authored provider choice scopes every member of that supplied
        // implementation even when this compilation realizes another target.
        for row in provenance {
            if build
                .provider_selections
                .iter()
                .chain(&build.package_provider_selections)
                .any(|choice| {
                    choice
                        .target
                        .is_none_or(|target| target.canonical_name() == profile.target_name())
                        && row.provider.provider_type == Some(choice.provider_type.symbol)
                })
            {
                roots.extend(
                    row.provider
                        .row_realizations
                        .iter()
                        .filter_map(|origin| origin.machine()),
                );
            }
        }
        application_roots.push(checked_trees::applications::TargetApplicationRoots {
            target: profile.target_name().to_owned(),
            machines: if unscoped {
                source
                    .program
                    .typed
                    .machines()
                    .iter()
                    .map(|machine| machine.symbol)
                    .collect()
            } else {
                roots.clone()
            },
        });
        let declarations = if unscoped {
            typed_trees::references::DeclarationSelection::from_program(&source.program.typed)
        } else {
            psi_compiler::target_application_declarations(
                &source.program,
                profile.target_name(),
                &roots,
                &edges,
            )
        };
        let facts = profile.layout_target_facts().ok_or_else(|| {
            vec![Diagnostic::error(format!(
                "target `{}` has no compiler-issued layout representation context",
                profile.target_name()
            ))]
        })?;
        for occurrence in typed_trees::references::collect_selected_program_type_occurrences(
            &source.program.typed,
            &declarations,
        ) {
            let Some(storage) = source
                .program
                .typed
                .layout_storage_arguments(occurrence.reference)
            else {
                continue;
            };
            if source
                .program
                .typed
                .closed_data_field_definition(storage.semantic_type)
                .is_none()
                || !typed_trees::domain::index_argument_is_closed(
                    &source.program.typed,
                    storage.policy,
                )
            {
                continue;
            }
            let application = source
                .program
                .typed
                .normalized_type_identity(storage.application);
            let application_identity = source
                .program
                .typed
                .type_identity_text(application)
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "layout application lost its canonical identity export",
                    )]
                })?;
            if !inputs.iter().any(|known: &LayoutApplicationTargetInput| {
                source
                    .program
                    .typed
                    .type_identity_by_text(&known.application_identity)
                    == application
                    && known.target == facts
            }) {
                inputs.push(LayoutApplicationTargetInput {
                    application_identity,
                    target: facts.clone(),
                });
            }
        }
    }
    Ok(CollectedApplications {
        layouts: inputs,
        roots: application_roots,
    })
}

pub(super) fn provider_edges(
    provenance: &[SelectedProviderReviewProvenance],
) -> Vec<(SymbolHandle, SymbolHandle)> {
    provenance
        .iter()
        .flat_map(|selected| {
            selected
                .provider
                .row_requirements
                .iter()
                .copied()
                .zip(selected.provider.row_realizations.iter().copied())
                .filter_map(|(requirement, origin)| {
                    origin.machine().map(|machine| (requirement, machine))
                })
        })
        .collect()
}

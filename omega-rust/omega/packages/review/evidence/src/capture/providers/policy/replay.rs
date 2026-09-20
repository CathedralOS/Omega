//! Validate live selected associations before projecting inert policy.

use super::rejected;
use crate::capture::PackageReviewInput;
use diagnostics::Diagnostic;
use provider_planning::{
    DerivedProviderPlan, ProviderSelectionProvenance, SelectedProviderPlanWithProvenance,
};
use semantic_vocabulary::PackageKeyIdentity;
use target::TargetProfile;

pub(super) fn validate(
    compilation: &PackageReviewInput<'_>,
    target: TargetProfile,
    package: PackageKeyIdentity,
) -> Result<(), Vec<Diagnostic>> {
    if compilation.custody.package_identity() != Some(package)
        || compilation.custody.selected_target_profile() != Some(target)
        || compilation.custody.selected_native_target() != Some(target.native_target())
        || compilation.custody.evaluated_via_bindings().target() != Some(target)
    {
        return Err(rejected(
            "package or target differs from the checked root activation",
        ));
    }
    let plans = compilation.custody.selected_provider_plans().plans();
    let provenance = compilation.custody.selected_provider_provenance();
    if plans.len() != provenance.len()
        || plans.iter().zip(provenance).any(|(plan, retained)| {
            plan != &retained.plan
                || plan.rows.len() != retained.row_compiler_intrinsic_executions.len()
        })
    {
        return Err(rejected(
            "selected plans and retained provenance are not aligned",
        ));
    }
    let selected = provenance
        .iter()
        .map(|retained| SelectedProviderPlanWithProvenance {
            derived: DerivedProviderPlan {
                plan: retained.plan.clone(),
                provenance: retained.provider.clone(),
            },
            selected_by: retained.selected_by.clone(),
        })
        .collect();
    // The component-closure join replays against the same components
    // settlement admitted: custody retains the attached descriptions, and
    // re-verifying them under the build's admission profile reproduces the
    // verified set rather than trusting the retained join outcome.
    let independent_components = build_evaluation::verify_independent_component_descriptions(
        compilation.custody.independent_component_descriptions(),
        compilation.custody.accepted_component_assumptions(),
        |_| None,
    )?;
    let (replayed, _) =
        provider_planning::selected_provider_plan_facts_with_independent_components(
            &compilation.typed,
            compilation.custody.evaluated_via_bindings(),
            selected,
            &independent_components,
        )?;
    if replayed.plans() != plans {
        return Err(rejected(
            "selected semantic plans differ from exact typed replay",
        ));
    }
    validate_authored_activation(compilation)
}

fn validate_authored_activation(
    compilation: &PackageReviewInput<'_>,
) -> Result<(), Vec<Diagnostic>> {
    let provenance = compilation.custody.selected_provider_provenance();
    let Some(build_symbol) = compilation.custody.selected_build_machine_symbol() else {
        if !compilation.custody.selected_provider_grants().is_empty()
            || provenance.iter().any(|retained| {
                matches!(
                    retained.selected_by,
                    ProviderSelectionProvenance::BuildOverride(_)
                )
            })
        {
            return Err(rejected(
                "authored provider activation has no selected build machine",
            ));
        }
        return validate_target_defaults(compilation);
    };
    let builds = compilation
        .machines()
        .iter()
        .filter(|machine| machine.symbol == build_symbol)
        .collect::<Vec<_>>();
    let [build] = builds.as_slice() else {
        return Err(rejected("selected build machine is missing or ambiguous"));
    };
    let selections = build_evaluation::harvest_provider_selections(&compilation.typed, build)?;
    for retained in provenance {
        if let ProviderSelectionProvenance::BuildOverride(declarations) = &retained.selected_by
            && declarations.iter().any(|declaration| {
                declaration.selecting_machine != build_symbol || !selections.contains(declaration)
            })
        {
            return Err(rejected(
                "build selection differs from its current authored declaration",
            ));
        }
    }
    if selections.iter().any(|selection| {
        !provenance.iter().any(|retained| {
            matches!(&retained.selected_by, ProviderSelectionProvenance::BuildOverride(declarations)
                if declarations.contains(selection))
        })
    }) {
        return Err(rejected(
            "an authored build selection is absent from the selected closure",
        ));
    }
    let authored = build_evaluation::harvest_root_grants(&compilation.typed, build)
        .map_err(|diagnostic| vec![diagnostic])?;
    let grants = trust_model::resolve_authored_selected_provider_grants(
        compilation.custody.provider_plans(),
        compilation.custody.selected_provider_plans(),
        &authored,
    )
    .map_err(|diagnostic| vec![diagnostic])?;
    if grants != compilation.custody.selected_provider_grants() {
        return Err(rejected(
            "provider grants differ from exact authored selected-plan replay",
        ));
    }
    validate_target_defaults(compilation)
}

fn validate_target_defaults(compilation: &PackageReviewInput<'_>) -> Result<(), Vec<Diagnostic>> {
    for retained in compilation.custody.selected_provider_provenance() {
        let ProviderSelectionProvenance::TargetDefault(declarations) = &retained.selected_by else {
            continue;
        };
        for declaration in declarations {
            let machines = compilation
                .machines()
                .iter()
                .filter(|machine| machine.symbol == declaration.selecting_machine)
                .collect::<Vec<_>>();
            let [machine] = machines.as_slice() else {
                return Err(rejected(
                    "target default has no exact authored selecting machine",
                ));
            };
            let current =
                build_evaluation::harvest_provider_selections(&compilation.typed, machine)?;
            if !current.contains(declaration) {
                return Err(rejected(
                    "target default differs from its current authored selection",
                ));
            }
        }
    }
    Ok(())
}

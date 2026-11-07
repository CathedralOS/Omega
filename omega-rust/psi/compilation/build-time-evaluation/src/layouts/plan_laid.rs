//! Checked fixed representation application evaluation and installation.
use crate::product::provider_declarations::{
    retain_provider_defaults, source_checking_applications,
};
use source::DependencyScope;
use typed_trees::references::collect_selected_program_type_occurrences;

mod layout_installation;
pub(crate) use layout_installation::compute_plan_laid_layouts_in;
use layout_installation::compute_prepared_plan_laid_layouts_in;

use diagnostics::Diagnostic;
use typed_trees::TypedTrees;

/// Whether any type in `typed` names fixed-layout storage. Without one,
/// [`evaluate_fixed_layout_applications`] has nothing to evaluate.
pub fn has_fixed_layout_applications(typed: &TypedTrees) -> bool {
    typed_trees::references::collect_program_type_occurrences(typed)
        .into_iter()
        .any(|occurrence| {
            typed
                .layout_storage_arguments(occurrence.reference)
                .is_some()
        })
}

/// An explicit Source checking scope, independent of Native settlement.
/// Every supplied declaration and member is checked, including defaults and
/// overrides. None retains the reusable Source surface; Some(empty) carries
/// provider declarations or authored target bodies without inventing an entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixedLayoutTargetScope {
    pub target: String,
    pub facts: Option<layout_plans::LayoutTargetFacts>,
    pub entry_declarations: Option<Vec<symbols::SymbolHandle>>,
    pub provider_declarations: Vec<crate::product::provider_declarations::ProviderSelection>,
}

/// Close Source representation applications before the settled judgment.
/// Each supplied application contributes its complete checking context, without
/// choosing a Native provider or supplying installation authority.
/// A program without fixed-layout storage is returned unchanged.
pub fn evaluate_fixed_layout_applications(
    typed: &mut TypedTrees,
    scopes: &[FixedLayoutTargetScope],
    selection_authority: Option<std::sync::Arc<dyn crate::BuildTimeSelectionAuthority>>,
) -> Result<(), Vec<Diagnostic>> {
    if !has_fixed_layout_applications(typed) {
        return Ok(());
    }
    typed_trees_to_typed_trees::prepare_execution_applications(typed)?;
    let checking = checking_target_applications(typed, scopes)?;
    let mut targets = Vec::new();
    let mut inputs = Vec::new();
    // `cross_platform_cli`, `local_unchecked`, and `alpha_bootstrap` can select
    // a provider whose body names a fixed layout, but they publish no
    // representation profile. A facts-bearing target still forms that layout.
    // Demanding a profile from the fact-less selection rejects the layout the
    // bearing target already supplies. When no scope has facts, the demand
    // stays and the refusal is unchanged.
    let facts_supplied = scopes.iter().any(|scope| scope.facts.is_some());
    for (scope, work) in scopes.iter().zip(&checking) {
        if scope.facts.is_none() && facts_supplied {
            continue;
        }
        if let Some(facts) = &scope.facts {
            facts
                .validate()
                .map_err(|reason| vec![Diagnostic::error(reason)])?;
            if facts.profile != scope.target {
                return Err(vec![Diagnostic::error(
                    "fixed representation scope differs from its explicit target facts",
                )]);
            }
            if !targets.contains(facts) {
                targets.push(facts.clone());
            }
        }
        for occurrence in collect_selected_program_type_occurrences(typed, work) {
            let Some(storage) = typed.layout_storage_arguments(occurrence.reference) else {
                continue;
            };
            if typed.data_field_definition(storage.semantic_type).is_none() {
                continue;
            }
            let facts = scope.facts.as_ref().ok_or_else(|| vec![Diagnostic::error(format!(
                "fixed representation application `{}` requires unavailable representation facts for `{}`",
                typed.display_type_reference(storage.application), scope.target,
            ))])?;
            let identity = typed.normalized_type_identity(storage.application);
            let application_identity = typed.type_identity_text(identity).ok_or_else(|| {
                vec![Diagnostic::error(
                    "fixed representation lost its canonical application identity",
                )]
            })?;
            let input = crate::LayoutApplicationTargetInput {
                application_identity,
                target: facts.clone(),
                lexical_owner: occurrence.lexical_owner,
            };
            if !inputs.contains(&input) {
                inputs.push(input);
            }
        }
    }
    let mut domain = crate::build_time_evaluation::EvaluationDomain::all();
    domain.layout_targets = targets;
    domain.layout_applications = Some(inputs);
    compute_prepared_plan_laid_layouts_in(typed, selection_authority, &domain)?;
    layout_installation::validate_layout_application_coverage(
        typed,
        domain.layout_applications.as_deref(),
    )?;
    for occurrence in typed_trees::references::collect_program_type_occurrences(typed) {
        if let Some(storage) = typed.layout_storage_arguments(occurrence.reference)
            && typed.data_field_definition(storage.semantic_type).is_some()
            && typed_trees::domain::index_argument_is_closed(typed, storage.policy)
            && typed
                .plan_laid_layout_for_type(storage.application)
                .is_none()
        {
            return Err(vec![Diagnostic::error(
                "fixed representation application has no explicit target-input closure",
            )]);
        }
    }
    Ok(())
}

/// Physical operations and authored layouts use the same supplied application
/// scopes. A private target body must not acquire another target's geometry.
pub(super) fn checking_target_applications(
    typed: &TypedTrees,
    scopes: &[FixedLayoutTargetScope],
) -> Result<Vec<typed_trees::references::DeclarationSelection>, Vec<Diagnostic>> {
    let defaults = retain_provider_defaults(
        crate::product::provider_declarations::ProviderPlanningContext::new(
            typed,
            DependencyScope::Product,
        ),
    )?;
    let mut topology =
        typed_trees::typed_trees::calls::application_graph::source_application_plan(typed);
    let dynamic = validation::describe_dynamic_conformances(typed)?;
    let symbols = &mut topology.symbols;
    topology.machines.for_each_mut(|_, machine| {
        machine.dynamic_realizations = symbols.insert_many(
            dynamic
                .selections
                .iter()
                .filter(|selection| selection.machine == machine.machine)
                .flat_map(|selection| selection.rows.iter().map(|row| row.realization_machine)),
        );
    });
    let operational = validation::infer_operational_may(typed);
    source_checking_applications(typed, scopes, &defaults, &topology, &operational)
        .map_err(|reason| vec![Diagnostic::error(reason)])
}

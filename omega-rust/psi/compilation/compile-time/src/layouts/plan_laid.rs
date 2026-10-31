//! Checked fixed representation application evaluation and installation.
mod layout_installation;
pub(crate) use layout_installation::{
    compute_plan_laid_layouts_in, validate_layout_application_coverage,
};

use diagnostics::Diagnostic;
use typed_trees::TypedTrees;

/// An explicit Source checking scope, independent of Native settlement.
/// Every supplied declaration and member is checked, including defaults and
/// overrides. None retains the reusable Source surface; Some(empty) carries
/// provider declarations or authored target bodies without inventing an entry.
#[derive(Debug, Clone)]
pub struct FixedLayoutTargetScope {
    pub target: String,
    pub facts: Option<layout_plans::LayoutTargetFacts>,
    pub entry_declarations: Option<Vec<symbols::SymbolHandle>>,
    pub provider_declarations: Vec<crate::provider_declarations::ProviderSelection>,
}

/// Close Source representation applications before the settled judgment.
/// Each supplied application contributes its complete checking context, without
/// choosing a Native provider or supplying installation authority.
pub fn evaluate_fixed_layout_applications(
    typed: &mut TypedTrees,
    scopes: &[FixedLayoutTargetScope],
    selection_authority: Option<std::sync::Arc<dyn crate::BuildTimeSelectionAuthority>>,
) -> Result<(), Vec<Diagnostic>> {
    if !typed_trees::references::collect_program_type_occurrences(typed)
        .into_iter()
        .any(|occurrence| {
            typed
                .layout_storage_arguments(occurrence.reference)
                .is_some()
        })
    {
        return Ok(());
    }
    typed_trees_to_typed_trees::prepare_execution_applications(typed)?;
    let defaults = crate::provider_declarations::retain_provider_defaults(
        crate::provider_declarations::ProviderPlanningContext::new(
            typed,
            source::DependencyScope::Product,
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
    let mut targets = Vec::new();
    let mut inputs = Vec::new();
    let checking = crate::provider_declarations::source_checking_applications(
        typed,
        scopes,
        &defaults,
        &topology,
        &operational,
    )
    .map_err(|reason| vec![Diagnostic::error(reason)])?;
    for (scope, work) in scopes.iter().zip(&checking) {
        if let Some(facts) = &scope.facts {
            facts
                .validate()
                .map_err(|reason| vec![Diagnostic::error(reason)])?;
            if facts.profile != scope.target {
                return Err(vec![Diagnostic::error(
                    "fixed representation scope differs from its explicit target facts",
                )]);
            }
            // Explicit checking inputs exist independently of whether this
            // scope happens to contain a closed consumer of an open recipe.
            if !targets.contains(facts) {
                targets.push(facts.clone());
            }
        }
        for occurrence in
            typed_trees::references::collect_selected_program_type_occurrences(typed, work)
        {
            let Some(storage) = typed.layout_storage_arguments(occurrence.reference) else {
                continue;
            };
            if typed
                .closed_data_field_definition(storage.semantic_type)
                .is_none()
            {
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
    compute_plan_laid_layouts_in(typed, selection_authority, &domain)?;
    layout_installation::validate_layout_application_coverage(
        typed,
        domain.layout_applications.as_deref(),
    )?;
    for occurrence in typed_trees::references::collect_program_type_occurrences(typed) {
        if let Some(storage) = typed.layout_storage_arguments(occurrence.reference)
            && typed
                .closed_data_field_definition(storage.semantic_type)
                .is_some()
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

//! Checked fixed representation application evaluation and installation.
mod layout_installation;
pub(crate) use layout_installation::compute_plan_laid_layouts_in;

use diagnostics::Diagnostic;
use typed_trees::{TypedTrees, references::DeclarationSelection};

/// An explicit deployment scope from Build or the embedding library checker.
/// None describes the reusable source surface. An empty deployment root roster
/// instead exports the explicit provider members without inventing a host entry.
#[derive(Debug, Clone)]
pub struct FixedLayoutTargetScope {
    pub target: String,
    pub facts: Option<layout_plans::LayoutTargetFacts>,
    pub roots: Option<Vec<symbols::SymbolHandle>>,
    pub providers: Vec<crate::provider_declarations::ProviderSelection>,
}

/// Close physical representation applications before the settled source judgment.
/// Pure policy admission checks each selected execution closure independently;
/// it neither folds source predicates nor supplies native provider authority.
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
                .is_some_and(|storage| {
                    typed
                        .closed_data_field_definition(storage.semantic_type)
                        .is_some()
                        && typed_trees::domain::index_argument_is_closed(typed, storage.policy)
                })
        })
    {
        return Ok(());
    }
    typed_trees_to_typed_trees::specialize_static_machine_calls(typed)?;
    let defaults = crate::provider_declarations::retain_provider_defaults(
        crate::provider_declarations::ProviderPlanningContext::new(
            typed,
            source::DependencyScope::Product,
        ),
    )?;
    let topology =
        typed_trees::typed_trees::calls::application_graph::source_application_plan(typed);
    let operational = validation::infer_operational_may(typed);
    let applications =
        semantic_vocabulary::proof_applications::ProofApplications::new(&topology, &operational);
    let mut targets = Vec::new();
    let mut inputs = Vec::new();
    for scope in scopes {
        let providers = crate::provider_declarations::source_application_dependencies(
            typed,
            &scope.target,
            &scope.providers,
            &defaults,
            topology.provider_candidates.storage_slice(),
        )
        .map_err(|reason| vec![Diagnostic::error(reason)])?;
        let work = if let Some(roots) = &scope.roots {
            let mut roots = roots.clone();
            if roots.is_empty() {
                // A provider-only deployment exposes its explicitly selected
                // member entries. Ambient source defaults are descriptions,
                // not externally invoked roots of that deployment.
                for (_, member) in crate::provider_declarations::source_application_dependencies(
                    typed,
                    &scope.target,
                    &scope.providers,
                    &[],
                    topology.provider_candidates.storage_slice(),
                )
                .map_err(|reason| vec![Diagnostic::error(reason)])?
                {
                    if !roots.contains(&member) {
                        roots.push(member);
                    }
                }
            }
            let reached = applications.invocation_reachability(&scope.target, &providers, &roots);
            crate::syntax_probes::PreparedEvaluationInputs::domain_for_program(
                typed,
                &reached.machines,
                &[],
            )
            .work()
            .clone()
        } else {
            DeclarationSelection::from_program(typed)
        };
        for occurrence in
            typed_trees::references::collect_selected_program_type_occurrences(typed, &work)
        {
            let Some(storage) = typed.layout_storage_arguments(occurrence.reference) else {
                continue;
            };
            if typed
                .closed_data_field_definition(storage.semantic_type)
                .is_none()
                || !typed_trees::domain::index_argument_is_closed(typed, storage.policy)
            {
                continue;
            }
            let facts = scope.facts.as_ref().ok_or_else(|| vec![Diagnostic::error(format!(
                "fixed representation application `{}` requires unavailable representation facts for `{}`",
                typed.display_type_reference(storage.application), scope.target,
            ))])?;
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

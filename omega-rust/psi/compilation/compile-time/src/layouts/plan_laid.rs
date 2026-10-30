//! Checked fixed representation application evaluation and installation.
mod layout_installation;
pub(crate) use layout_installation::compute_plan_laid_layouts_in;

use diagnostics::Diagnostic;
use typed_trees::{TypedTrees, references::DeclarationSelection};

/// An explicit deployment scope from Build or the embedding library checker.
/// Empty roots describe the reusable source surface, rather than a host entry.
#[derive(Debug, Clone)]
pub struct FixedLayoutTargetScope {
    pub target: layout_plans::LayoutTargetFacts,
    pub roots: Vec<symbols::SymbolHandle>,
}

/// Close physical representation applications before the settled source judgment.
/// Pure policy admission checks each selected execution closure independently;
/// it neither folds source predicates nor supplies native provider authority.
pub fn evaluate_fixed_layout_applications(
    typed: &mut TypedTrees,
    scopes: &[FixedLayoutTargetScope],
) -> Result<(), Vec<Diagnostic>> {
    if scopes.is_empty() {
        return Ok(());
    }
    typed_trees_to_typed_trees::specialize_static_machine_calls(typed)?;
    let mut targets = Vec::new();
    let mut inputs = Vec::new();
    for scope in scopes {
        scope
            .target
            .validate()
            .map_err(|reason| vec![Diagnostic::error(reason)])?;
        if !targets.contains(&scope.target) {
            targets.push(scope.target.clone());
        }
        let work = if scope.roots.is_empty() {
            DeclarationSelection::from_program(typed)
        } else {
            crate::syntax_probes::PreparedEvaluationInputs::domain_for_program(
                typed,
                &scope.roots,
                &[],
            )
            .work()
            .clone()
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
            let identity = typed.normalized_type_identity(storage.application);
            let application_identity = typed.type_identity_text(identity).ok_or_else(|| {
                vec![Diagnostic::error(
                    "fixed representation lost its canonical application identity",
                )]
            })?;
            let input = crate::LayoutApplicationTargetInput {
                application_identity,
                target: scope.target.clone(),
            };
            if !inputs.contains(&input) {
                inputs.push(input);
            }
        }
    }
    let mut domain = crate::build_time_evaluation::EvaluationDomain::all();
    domain.layout_targets = targets;
    domain.layout_applications = Some(inputs);
    compute_plan_laid_layouts_in(typed, None, &domain)?;
    layout_installation::validate_layout_application_coverage(
        typed,
        domain.layout_applications.as_deref(),
    )
}

//! Close the product's constants and each authored target's proof applications.
//!
//! The shared judgment kept every provider-dependent constant symbolic and
//! recorded what it needs from each. Here, after this target's providers
//! settled, Psi's build-time evaluator resumes the deferred continuation on a
//! private copy under exactly the selected rows, the values are read back by
//! roster position, the recorded obligations are discharged, and a closed
//! copy of the checked program carries the closure for Terminal production
//! and installation. The shared program is never rewritten; a program with no
//! recipe or layout query keeps its shared storage.
//! A scoped caller's constant predicates use that body's capsule, even when
//! no executable for it is requested. Private supplemental views discharge
//! those predicates without overwriting shared or selected-product values.

use checked_trees::{CheckedTrees, ConstantRecipeObligation};
use compilation_products::CheckedProductSource;
use diagnostics::Diagnostic;
use std::sync::Arc;
use typed_trees::constant_recipes::ConstantRecipeDependencyReceipt;

pub(super) fn close_constant_recipes(
    source: &CheckedProductSource,
    selected_target_profile: target::TargetProfile,
    opaque_representation_selections: &[representation_planning::OpaqueRepresentationSelection],
    selected_provider_plan_facts: &effects::SelectedProviderPlanFacts,
    selected_provider_provenance: &[provider_planning::SelectedProviderReviewProvenance],
    mut proof_providers: impl FnMut(
        target::TargetProfile,
    ) -> Result<
        provider_planning::CheckedProviderSelection,
        Vec<Diagnostic>,
    >,
) -> Result<Arc<CheckedTrees>, Vec<Diagnostic>> {
    let shared = &source.program;
    let typed = &shared.typed;
    let layout_queries: Vec<_> = typed_trees::references::collect_program_references(typed)
        .expressions
        .into_iter()
        .filter(|expression| {
            matches!(
                typed.expression_table.expression(*expression),
                typed_trees::expression::ExpressionNode::Call(call) if call.layout_query.is_some()
            )
        })
        .collect();
    // Runtime layout queries also retain a continuation even when no array,
    // range endpoint, or const application contributes a recipe roster entry.
    if typed.constant_recipes.is_empty() && layout_queries.is_empty() {
        return Ok(Arc::clone(shared));
    }
    let mut closed_typed = close_constant_values(
        source,
        selected_target_profile,
        opaque_representation_selections,
        selected_provider_plan_facts,
        &layout_queries,
    )?;
    ConstantRecipeObligation::discharge_all(
        &shared.facts.constant_recipe_obligations,
        &closed_typed,
    )?;
    let mut closed_facts = shared.facts.clone();
    let mut proof_targets = vec![selected_target_profile];
    let predicate_rows = &shared.facts.constant_predicate_obligations;
    let range_rows = &shared.facts.integer_range_obligations;
    if !predicate_rows.is_empty() || !range_rows.is_empty() {
        for machine in typed.machines() {
            if let Some(name) = typed.machine_authored_target(machine) {
                let target = target::TargetProfile::from_canonical_target_name(name)
                    .map_err(|reason| vec![reason])?;
                if !proof_targets.contains(&target) {
                    proof_targets.push(target);
                }
            }
        }
    }
    closed_facts.constant_predicate_obligations.clear();
    closed_facts.integer_range_obligations.clear();
    let mut predicate_closed = vec![false; predicate_rows.len()];
    let mut range_closed = vec![false; range_rows.len()];
    let proof_owners: Vec<_> = predicate_rows
        .iter()
        .map(|row| row.caller())
        .chain(range_rows.iter().map(|row| row.owner()))
        .collect();
    for target in proof_targets {
        if target != selected_target_profile
            && !provider_planning::target_machines::target_has_proof_demand(
                shared,
                target,
                &proof_owners,
            )
        {
            continue;
        }
        let supplemental = if target == selected_target_profile {
            None
        } else {
            Some(proof_providers(target)?)
        };
        let provenance = supplemental
            .as_ref()
            .map_or(selected_provider_provenance, |selected| {
                selected.selected_provider_provenance.as_slice()
            });
        let applications = provider_planning::target_machines::proof_application_machines(
            shared,
            target,
            target == selected_target_profile,
            provenance,
        );
        let predicate_applications: Vec<_> = predicate_rows
            .iter()
            .enumerate()
            .filter(|(_, obligation)| applications.contains(&obligation.caller()))
            .collect();
        let range_applications: Vec<_> = range_rows
            .iter()
            .enumerate()
            .filter(|(_, obligation)| applications.contains(&obligation.owner()))
            .collect();
        if predicate_applications.is_empty() && range_applications.is_empty() {
            continue;
        }
        let proof_program = match &supplemental {
            None => None,
            Some(selected) => Some(close_constant_values(
                source,
                target,
                opaque_representation_selections,
                &selected.selected_provider_plan_facts,
                &layout_queries,
            )?),
        };
        let program = proof_program.as_ref().unwrap_or(&closed_typed);
        for (position, obligation) in predicate_applications {
            let mut application = obligation.clone();
            checked_trees::ConstantPredicateObligation::discharge_all(
                std::iter::once(&mut application),
                program,
                target.target_name(),
            )?;
            closed_facts
                .constant_predicate_obligations
                .push(application);
            predicate_closed[position] = true;
        }
        for (position, obligation) in range_applications {
            let mut application = obligation.clone();
            application
                .discharge(program, target.target_name())
                .map_err(|reason| vec![reason])?;
            closed_facts
                .operators
                .decided_formations
                .push(application.expression());
            closed_facts.integer_range_obligations.push(application);
            range_closed[position] = true;
        }
        if let Some(proof_program) = proof_program {
            closed_typed
                .constant_recipe_closure
                .receipts
                .extend(proof_program.constant_recipe_closure.receipts);
        }
    }
    if predicate_closed.contains(&false) || range_closed.contains(&false) {
        return Err(vec![Diagnostic::error(
            "retained proof obligation has no target application",
        )]);
    }
    let receipts = &mut closed_typed.constant_recipe_closure.receipts;
    closed_facts
        .operators
        .decided_formations
        .sort_by_key(|expression| checked_trees::operators::handle_position(*expression));
    closed_facts.operators.decided_formations.dedup();
    receipts.sort_by(|left, right| {
        left.target_scope
            .cmp(&right.target_scope)
            .then(
                left.requirement
                    .arena_index()
                    .cmp(&right.requirement.arena_index()),
            )
            .then(left.plan_identity.cmp(&right.plan_identity))
    });
    receipts.dedup();
    if !receipts.is_empty() {
        closed_typed.constant_recipe_closure.target_scope =
            selected_target_profile.target_name().to_owned();
    }
    Ok(Arc::new(CheckedTrees::with_roots(
        closed_typed,
        closed_facts,
    )))
}

/// Resume one capsule's existing evaluator and rejoin by canonical identity.
/// The caller decides which proof applications consume this value closure.
fn close_constant_values(
    source: &CheckedProductSource,
    selected_target_profile: target::TargetProfile,
    opaque_representation_selections: &[representation_planning::OpaqueRepresentationSelection],
    selected_provider_plan_facts: &effects::SelectedProviderPlanFacts,
    layout_queries: &[typed_trees::expression::ExpressionHandle],
) -> Result<typed_trees::TypedTrees, Vec<Diagnostic>> {
    let typed = &source.program.typed;
    // The continuation folds on the authored source it was deferred from,
    // where a boundary token use is still the occurrence the selected rows
    // name; executable preparation has already normalized it in the checked
    // program. Roster positions are the same handles in both.
    let Some(authored) = source.constant_recipe_source.as_deref() else {
        return Err(vec![Diagnostic::error(
            "constant recipes are retained without their authored evaluation source",
        )]);
    };
    let evaluation = selected_dispatch::SelectedConstantEvaluation::prepare(
        provider_planning::ProviderPlanningContext::new(authored, source::DependencyScope::Product),
        selected_target_profile,
        opaque_representation_selections,
        &validation::derive_pre_flow_operator_selections(authored),
        selected_provider_plan_facts,
    )?;
    let (operators, provider_bodies) = evaluation.pending_fold_providers();
    let mut receipts: Vec<ConstantRecipeDependencyReceipt> = operators
        .iter()
        .map(|row| ConstantRecipeDependencyReceipt {
            requirement: row.requirement,
            plan_identity: *row.provider.as_bytes(),
            target_scope: selected_target_profile.target_name().to_owned(),
            provider_declaration: Default::default(),
            provider_machine: Default::default(),
        })
        .chain(
            provider_bodies
                .iter()
                .map(|row| ConstantRecipeDependencyReceipt {
                    requirement: row.requirement,
                    plan_identity: *row.provider.as_bytes(),
                    target_scope: row.target_scope.clone(),
                    provider_declaration: row.provider_declaration,
                    provider_machine: row.provider_machine,
                }),
        )
        .collect();
    receipts.sort_by(|left, right| {
        left.target_scope
            .cmp(&right.target_scope)
            .then(
                left.requirement
                    .arena_index()
                    .cmp(&right.requirement.arena_index()),
            )
            .then(left.plan_identity.cmp(&right.plan_identity))
    });
    receipts.dedup();
    // The fold lands on a private copy of the shared program; the shared
    // storage keeps its symbolic positions for every other target.
    let mut evaluated = authored.clone();
    evaluation.evaluate_pending(&mut evaluated, source.constant_recipe_continuations.clone())?;
    let closure = typed.read_constant_recipe_closure(&evaluated, receipts)?;
    let mut closed_typed = (**typed).clone().with_constant_recipe_closure(closure);
    if !layout_queries.is_empty() {
        // Evaluation already produced these answers under the selected target.
        // Rejoin by canonical expression identity instead of asking the layout
        // authority to rebuild the same output on the checked copy.
        for expression in layout_queries {
            let value = evaluated.expression_table.expression(*expression);
            if !matches!(value, typed_trees::expression::ExpressionNode::Integer(_)) {
                return Err(vec![Diagnostic::error(
                    "retained layout query did not close at its canonical expression identity",
                )]);
            }
            *closed_typed.expression_table.expression_mut(*expression) = value.clone();
        }
        closed_typed
            .tables
            .const_declarations
            .for_each_mut(|_, declaration| {
                if layout_queries.contains(&declaration.materialized_initializer) {
                    declaration.evaluated_value = evaluated
                        .const_declaration_by_symbol(declaration.symbol)
                        .and_then(|closed| closed.evaluated_value.clone());
                }
            });
    }
    // The continuation owns validated plan installation. Preserve that
    // result beside the closed constants before the checked program consumes
    // a layout query or a plan-laid field.
    closed_typed.plan_laid_layouts = evaluated.plan_laid_layouts;
    closed_typed.plan_laid_target_layouts = evaluated.plan_laid_target_layouts;
    closed_typed.plan_laid_schema_fields = evaluated.plan_laid_schema_fields;
    Ok(closed_typed)
}

/// Whether two selections consume one checked program: the same shared
/// storage, or two closed copies whose folded programs (closure values,
/// realization receipts, layout answers) and judgments agree.
pub(crate) fn same_checked_program(left: &Arc<CheckedTrees>, right: &Arc<CheckedTrees>) -> bool {
    Arc::ptr_eq(left, right) || (left.typed == right.typed && left.facts == right.facts)
}

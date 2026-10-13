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
    mut proof_providers: impl FnMut(
        target::TargetProfile,
    ) -> Result<effects::SelectedProviderPlanFacts, Vec<Diagnostic>>,
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
    let obligation_targets: Vec<_> = closed_facts
        .constant_predicate_obligations
        .iter()
        .map(|obligation| {
            let authored = typed
                .authored_machine_target(obligation.caller())
                .map_err(|reason| vec![Diagnostic::error(reason)])?;
            match authored {
                None => Ok(selected_target_profile),
                Some(name) => target::TargetProfile::from_canonical_target_name(name)
                    .map_err(|reason| vec![reason]),
            }
        })
        .collect::<Result<_, _>>()?;
    let mut proof_targets = vec![selected_target_profile];
    for target in &obligation_targets {
        if !proof_targets.contains(target) {
            proof_targets.push(*target);
        }
    }
    // Each authored body keeps its own proof application even when it is not
    // an output target. Never discharge a sibling's predicate against this
    // product's sizes, and never reuse its provider rows for another capsule.
    for target in proof_targets {
        let proof_program = if target == selected_target_profile {
            None
        } else {
            let providers = proof_providers(target)?;
            Some(close_constant_values(
                source,
                target,
                opaque_representation_selections,
                &providers,
                &layout_queries,
            )?)
        };
        checked_trees::ConstantPredicateObligation::discharge_all(
            closed_facts
                .constant_predicate_obligations
                .iter_mut()
                .zip(&obligation_targets)
                .filter_map(|(obligation, required)| (*required == target).then_some(obligation)),
            proof_program.as_ref().unwrap_or(&closed_typed),
            target.target_name(),
        )?;
        if let Some(proof_program) = proof_program {
            // Proof dependencies participate in compatibility even when the
            // executable product never invokes that target's provider.
            closed_typed
                .constant_recipe_closure
                .receipts
                .extend(proof_program.constant_recipe_closure.receipts);
        }
    }
    let receipts = &mut closed_typed.constant_recipe_closure.receipts;
    receipts.sort_by_key(|receipt| (receipt.requirement.arena_index(), receipt.plan_identity));
    receipts.dedup();
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
        })
        .chain(
            provider_bodies
                .iter()
                .map(|row| ConstantRecipeDependencyReceipt {
                    requirement: row.requirement,
                    plan_identity: *row.provider.as_bytes(),
                }),
        )
        .collect();
    receipts.sort_by(|left, right| {
        left.requirement
            .arena_index()
            .cmp(&right.requirement.arena_index())
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

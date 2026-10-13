//! Close the shared program's constant recipes under one target capsule.
//!
//! The shared judgment kept every provider-dependent constant symbolic and
//! recorded what it needs from each. Here, after this target's providers
//! settled, Psi's build-time evaluator resumes the deferred continuation on a
//! private copy under exactly the selected rows, the values are read back by
//! roster position, the recorded obligations are discharged, and a closed
//! copy of the checked program carries the closure for Terminal production
//! and installation. The shared program is never rewritten; a program with no
//! recipe keeps its shared storage.

use checked_trees::{CheckedTrees, ConstantRecipeObligation};
use compilation_products::CheckedProductSource;
use diagnostics::Diagnostic;
use std::sync::Arc;
use typed_trees::constant_recipes::ConstantRecipeDependencyReceipt;

pub(super) fn close_constant_recipes(
    source: &CheckedProductSource,
    selected_native_target: target::NativeTarget,
    opaque_representation_selections: &[representation_planning::OpaqueRepresentationSelection],
    selected_provider_plan_facts: &effects::SelectedProviderPlanFacts,
) -> Result<Arc<CheckedTrees>, Vec<Diagnostic>> {
    let shared = &source.program;
    // No deferred continuation means every rostered position already folded
    // (an admitted Build evaluates its own); the shared program is closed.
    if shared.typed.constant_recipes.is_empty() || source.constant_recipe_continuations.is_empty() {
        return Ok(Arc::clone(shared));
    }
    let typed = &shared.typed;
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
        selected_native_target,
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
    let closed_typed = (**typed).clone().with_constant_recipe_closure(closure);
    ConstantRecipeObligation::discharge_all(
        &shared.facts.constant_recipe_obligations,
        &closed_typed,
    )?;
    Ok(Arc::new(CheckedTrees::with_roots(
        closed_typed,
        shared.facts.clone(),
    )))
}

/// Whether two selections consume one checked program: the same shared
/// storage, or two closed copies whose closures (values and realization
/// receipts) and judgments agree.
pub(crate) fn same_checked_program(left: &Arc<CheckedTrees>, right: &Arc<CheckedTrees>) -> bool {
    Arc::ptr_eq(left, right)
        || (!left.typed.constant_recipe_closure.is_empty()
            && left.typed.constant_recipe_closure == right.typed.constant_recipe_closure
            && left.typed == right.typed
            && left.facts == right.facts)
}

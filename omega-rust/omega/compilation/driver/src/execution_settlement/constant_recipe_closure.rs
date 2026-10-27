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

use checked_trees::CheckedTrees;
use compilation_products::CheckedProductSource;
use diagnostics::Diagnostic;
use std::sync::Arc;

/// Proof closure consumes selected constant facts, not native calling or emission plans.
#[derive(Default)]
pub(crate) struct ConstantProviders {
    pub facts: effects::SelectedProviderPlanFacts,
    pub provenance: Vec<provider_planning::SelectedProviderReviewProvenance>,
}

pub(crate) fn close_constant_recipes(
    source: &CheckedProductSource,
    selected_target_profile: target::TargetProfile,
    opaque_representation_selections: &[representation_planning::OpaqueRepresentationSelection],
    selected_provider_plan_facts: &effects::SelectedProviderPlanFacts,
    selected_provider_provenance: &[provider_planning::SelectedProviderReviewProvenance],
    mut proof_providers: impl FnMut(target::TargetProfile) -> Result<ConstantProviders, Vec<Diagnostic>>,
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
    let needs_constant_values = !typed.constant_recipes.is_empty() || !layout_queries.is_empty();
    if !needs_constant_values && shared.facts.integer_range_obligations.is_empty() {
        return Ok(Arc::clone(shared));
    }
    // A captured model-fit obligation needs no source evaluation or dummy
    // recipe. Only real constant dependencies resume their continuation.
    let closed_typed = if needs_constant_values {
        close_constant_values(
            source,
            selected_target_profile,
            opaque_representation_selections,
            selected_provider_plan_facts,
        )?
    } else {
        (**typed).clone()
    };
    let mut proof_closure = psi_compiler::ConstantProofClosure::new(
        shared,
        closed_typed,
        selected_target_profile.target_name(),
    );
    let proof_targets = proof_closure
        .target_scopes()
        .into_iter()
        .map(target::TargetProfile::from_canonical_target_name)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|reason| vec![reason])?;
    for target in proof_targets {
        if !proof_closure.needs_application(target.target_name()) {
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
                selected.provenance.as_slice()
            });
        let provider_edges: Vec<_> = provenance
            .iter()
            .flat_map(|selected| {
                selected
                    .provider
                    .row_requirements
                    .iter()
                    .copied()
                    .zip(selected.provider.row_realizations.iter().copied())
            })
            .collect();
        let Some(application) = proof_closure.application(target.target_name(), &provider_edges)
        else {
            continue;
        };
        let proof_program = match &supplemental {
            None => None,
            Some(_) if !needs_constant_values => None,
            Some(selected) => Some(close_constant_values(
                source,
                target,
                opaque_representation_selections,
                &selected.facts,
            )?),
        };
        // Model lookup is pure. Retain its refusal for Psi to report after
        // predicate and recipe failures, preserving the original check order.
        let address_model = if application.needs_address_model() {
            close_address_model(target).map(Some)
        } else {
            Ok(None)
        };
        application.discharge(proof_program, address_model)?;
    }
    proof_closure.finish()
}

/// Supply the selected platform model without performing source proof work.
fn close_address_model(
    target: target::TargetProfile,
) -> Result<checked_trees::IntegerRangeModel, Vec<Diagnostic>> {
    let model = target::TargetSemantics::close_address_model(target).ok_or_else(|| {
        vec![Diagnostic::error(format!(
            "selected target `{}` does not close the default address-space model",
            target.target_name(),
        ))]
    })?;
    // The selected model is authority for the source bound. Lowering's
    // supported representation is an independent capability limit.
    if model.carrier() != semantic_vocabulary::IntegerType::supported_address_representation() {
        return Err(vec![Diagnostic::error(format!(
            "selected address model for `{}` has no supported Terminal representation",
            target.target_name(),
        ))]);
    }
    Ok(checked_trees::IntegerRangeModel {
        carrier: model.carrier(),
        semantics_version: model.semantics_version(),
        commitment: model.commitment(),
    })
}

/// Resume one capsule's existing evaluator and rejoin by canonical identity.
/// The caller decides which proof applications consume this value closure.
fn close_constant_values(
    source: &CheckedProductSource,
    selected_target_profile: target::TargetProfile,
    opaque_representation_selections: &[representation_planning::OpaqueRepresentationSelection],
    selected_provider_plan_facts: &effects::SelectedProviderPlanFacts,
) -> Result<typed_trees::TypedTrees, Vec<Diagnostic>> {
    // The continuation folds on the authored source it was deferred from,
    // where a boundary token use is still the occurrence the selected rows
    // name; executable preparation has already normalized it in the checked
    // program. Roster positions are the same handles in both.
    let Some(constants) = source.constants.as_ref() else {
        return Err(vec![Diagnostic::error(
            "constant recipes are retained without their authored evaluation source",
        )]);
    };
    let authored = constants.source();
    let evaluation = selected_dispatch::SelectedConstantEvaluation::prepare(
        provider_planning::ProviderPlanningContext::new(authored, source::DependencyScope::Product),
        selected_target_profile,
        opaque_representation_selections,
        &validation::derive_pre_flow_operator_selections(authored),
        selected_provider_plan_facts,
    )?;
    evaluation.close_constants(constants, &source.program.typed)
}

/// Whether two selections consume one checked program: the same shared
/// storage, or two closed copies whose folded programs (closure values,
/// realization receipts, layout answers) and judgments agree.
pub(crate) fn same_checked_program(left: &Arc<CheckedTrees>, right: &Arc<CheckedTrees>) -> bool {
    Arc::ptr_eq(left, right) || (left.typed == right.typed && left.facts == right.facts)
}

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
use typed_trees::constant_recipes::ConstantRecipeDependencyReceipt;

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
            &layout_queries,
        )?
    } else {
        (**typed).clone()
    };
    let mut proof_closure = psi_compiler::ConstantProofClosure::new(
        shared,
        closed_typed,
        selected_target_profile.target_name(),
    );
    let mut proof_targets = vec![selected_target_profile];
    let predicate_rows = &shared.facts.constant_predicate_obligations;
    let range_rows = &shared.facts.integer_range_obligations;
    let recipe_rows = &shared.facts.constant_recipe_obligations;
    if !predicate_rows.is_empty() || !range_rows.is_empty() || !recipe_rows.is_empty() {
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
    let declaration_owners =
        provider_planning::target_machines::declaration_formation_owners(shared);
    let proof_owners: Vec<_> = predicate_rows
        .iter()
        .map(|row| row.caller())
        .chain(range_rows.iter().map(|row| row.owner()))
        .chain(
            recipe_rows
                .iter()
                .map(|row| row.owner())
                .filter(|owner| owner.is_valid()),
        )
        .collect();
    for target in proof_targets {
        if target != selected_target_profile
            && !recipe_rows.iter().any(|row| !row.owner().is_valid())
            && !provider_planning::target_machines::target_has_proof_demand(
                shared,
                target,
                &proof_owners,
                &declaration_owners,
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
                selected.provenance.as_slice()
            });
        let applications = provider_planning::target_machines::proof_application_machines(
            shared,
            target,
            target == selected_target_profile,
            provenance,
        );
        let declaration_applications =
            provider_planning::target_machines::proof_application_declaration_owners(
                shared,
                &applications,
                &declaration_owners,
            );
        let Some(application) = proof_closure.application(
            target.target_name(),
            &applications,
            &declaration_applications,
            &declaration_owners,
        ) else {
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
                &layout_queries,
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
    let target_bodies = evaluation.target_body_receipts();
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
    let mut closure = typed.read_constant_recipe_closure(&evaluated, receipts)?;
    closure.target_bodies = target_bodies;
    closure.target_scope = selected_target_profile.target_name().to_owned();
    let mut closed_typed = (**typed).clone().with_constant_recipe_closure(closure);
    closed_typed.plan_laid_layouts = std::mem::take(&mut evaluated.plan_laid_layouts);
    closed_typed.plan_laid_target_layouts = std::mem::take(&mut evaluated.plan_laid_target_layouts);
    closed_typed.plan_laid_schema_fields = std::mem::take(&mut evaluated.plan_laid_schema_fields);
    if !layout_queries.is_empty() {
        // Preparation can append specialized query occurrences after the
        // authored continuation was retained. Arena coordinates from the
        // checked program are not identities in the authored forest: reading
        // an appended handle there can silently select an unrelated literal.
        // Close these actual checked subjects through the same layout owner,
        // using this target's completed recipe and validated plan inputs.
        let selection = typed_trees::references::DeclarationSelection::from_program(&closed_typed);
        provider_planning::data_layout_queries::fold_data_layout_queries(
            &mut closed_typed,
            selected_target_profile,
            opaque_representation_selections,
            false,
            &selection,
        )?;
        closed_typed
            .tables
            .const_declarations
            .for_each_mut(|_, declaration| {
                if declaration.evaluated_value.is_none() {
                    declaration.evaluated_value = evaluated
                        .const_declaration_by_symbol(declaration.symbol)
                        .and_then(|closed| closed.evaluated_value.clone());
                }
            });
    }
    Ok(closed_typed)
}

/// Whether two selections consume one checked program: the same shared
/// storage, or two closed copies whose folded programs (closure values,
/// realization receipts, layout answers) and judgments agree.
pub(crate) fn same_checked_program(left: &Arc<CheckedTrees>, right: &Arc<CheckedTrees>) -> bool {
    Arc::ptr_eq(left, right) || (left.typed == right.typed && left.facts == right.facts)
}

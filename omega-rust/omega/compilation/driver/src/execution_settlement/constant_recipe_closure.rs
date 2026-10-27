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
    let pending_initializers = source
        .constant_recipe_continuations
        .iter()
        .any(|continuation| !continuation.pending_initializer_declarations().is_empty());
    let needs_constant_values =
        !typed.constant_recipes.is_empty() || !layout_queries.is_empty() || pending_initializers;
    if !needs_constant_values && shared.facts.integer_range_obligations.is_empty() {
        return Ok(Arc::clone(shared));
    }
    // A captured model-fit obligation needs no source evaluation or dummy
    // recipe. Only real constant dependencies resume their continuation.
    let mut closed_typed = if needs_constant_values {
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
    let mut closed_facts = shared.facts.clone();
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
    closed_facts.constant_predicate_obligations.clear();
    closed_facts.integer_range_obligations.clear();
    closed_facts.constant_recipe_obligations.clear();
    let mut predicate_closed = vec![false; predicate_rows.len()];
    let mut range_closed = vec![false; range_rows.len()];
    let mut recipe_closed = vec![false; recipe_rows.len()];
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
                selected.selected_provider_provenance.as_slice()
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
        let predicate_applications: Vec<_> = predicate_rows
            .iter()
            .enumerate()
            .filter(|(_, obligation)| applications.contains(&obligation.caller()))
            .collect();
        // These new declaration recipes contain captured BigInt bounds only,
        // not open binders or source computations. The selected checked copy
        // retains global descriptions for later Terminal metadata, so close
        // every ground declaration model recipe in its primary capsule. This
        // is conservative whole-capsule formation, not a caller premise or
        // authority to execute a declaration. Supplemental capsules also close
        // their actually referenced descriptions. Existing non-ground recipes
        // retain their original machine-application selection.
        let range_applications: Vec<_> = range_rows
            .iter()
            .enumerate()
            .filter(|(_, obligation)| {
                applications.contains(&obligation.owner())
                    || (obligation.needs_address_model()
                        && (declaration_applications.contains(&obligation.owner())
                            || (target == selected_target_profile
                                && declaration_owners.contains(&obligation.owner()))))
            })
            .collect();
        let recipe_applications: Vec<_> = recipe_rows
            .iter()
            .enumerate()
            .filter(|(_, obligation)| {
                !obligation.owner().is_valid() || applications.contains(&obligation.owner())
            })
            .collect();
        if predicate_applications.is_empty()
            && range_applications.is_empty()
            && recipe_applications.is_empty()
        {
            continue;
        }
        let proof_program = match &supplemental {
            None => None,
            Some(_) if !needs_constant_values => None,
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
        // Recipe comparisons belong to the caller application just like
        // constant predicates and arithmetic snapshots. A nonselected tagged
        // body's endpoint must close under its own provider capsule.
        for (position, obligation) in recipe_applications {
            let mut application = obligation.clone();
            ConstantRecipeObligation::discharge_all(
                std::slice::from_mut(&mut application),
                program,
            )?;
            closed_facts.constant_recipe_obligations.push(application);
            recipe_closed[position] = true;
        }
        let address_model = if range_applications
            .iter()
            .any(|(_, obligation)| obligation.needs_address_model())
        {
            let model = target::TargetSemantics::close_address_model(target).ok_or_else(|| {
                vec![Diagnostic::error(format!(
                    "selected target `{}` does not close the default address-space model",
                    target.target_name(),
                ))]
            })?;
            // The selected model is authority for the source bound. Lowering's
            // supported representation is an independent capability limit.
            if model.carrier()
                != semantic_vocabulary::IntegerType::supported_address_representation()
            {
                return Err(vec![Diagnostic::error(format!(
                    "selected address model for `{}` has no supported Terminal representation",
                    target.target_name(),
                ))]);
            }
            Some(checked_trees::IntegerRangeModel {
                carrier: model.carrier(),
                semantics_version: model.semantics_version(),
                commitment: model.commitment(),
            })
        } else {
            None
        };
        for (position, obligation) in range_applications {
            let mut application = obligation.clone();
            application
                .discharge(program, target.target_name(), address_model.as_ref())
                .map_err(|reason| vec![reason])?;
            closed_facts
                .operators
                .decided_formations
                .push(application.expression());
            closed_facts.integer_range_obligations.push(application);
            range_closed[position] = true;
        }
        if let Some(proof_program) = proof_program {
            closed_typed.constant_recipe_closure.target_bodies.extend(
                proof_program
                    .constant_recipe_closure
                    .target_bodies
                    .iter()
                    .cloned(),
            );
            closed_typed
                .constant_recipe_closure
                .receipts
                .extend(proof_program.constant_recipe_closure.receipts);
        }
    }
    if predicate_closed.contains(&false)
        || range_closed.contains(&false)
        || recipe_closed.contains(&false)
    {
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
    let target_bodies = &mut closed_typed.constant_recipe_closure.target_bodies;
    target_bodies.sort_by_key(|row| {
        (
            row.target_scope.clone(),
            row.head.arena_index(),
            row.head.generation(),
            row.member.arena_index(),
            row.member.generation(),
        )
    });
    target_bodies.dedup();
    if !receipts.is_empty() {
        closed_typed.constant_recipe_closure.target_scope =
            selected_target_profile.target_name().to_owned();
    }
    let closed = CheckedTrees::with_roots(closed_typed, closed_facts);
    let pending_boundary_clauses = closed
        .facts
        .flow
        .terminal_unit_effects
        .boundary_machines
        .iter()
        .any(|plan| {
            !plan.pending_scalar_requires.is_empty() || !plan.pending_scalar_recipes.is_empty()
        });
    let closed = if pending_boundary_clauses {
        psi_compiler::settle_closed_constant_execution(closed)?
    } else {
        closed
    };
    Ok(Arc::new(closed))
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
    let mut pending_initializers = Vec::new();
    for continuation in &source.constant_recipe_continuations {
        for declaration in continuation.pending_initializer_declarations() {
            if let Some(previous) = pending_initializers.iter().find(
                |previous: &&typed_trees::constant::ConstDeclaration| {
                    previous.symbol == declaration.symbol
                },
            ) {
                if previous != declaration {
                    return Err(vec![Diagnostic::error(
                        "pending scalar initializer has conflicting continuation custody",
                    )]);
                }
            } else {
                pending_initializers.push(declaration.clone());
            }
        }
    }
    build_time_evaluation::read_back_pending_initializer_values(
        &mut closed_typed,
        &evaluated,
        &pending_initializers,
    )?;
    Ok(closed_typed)
}

/// Whether two selections consume one checked program: the same shared
/// storage, or two closed copies whose folded programs (closure values,
/// realization receipts, layout answers) and judgments agree.
pub(crate) fn same_checked_program(left: &Arc<CheckedTrees>, right: &Arc<CheckedTrees>) -> bool {
    Arc::ptr_eq(left, right) || (left.typed == right.typed && left.facts == right.facts)
}

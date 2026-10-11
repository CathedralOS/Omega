//! Settle build-selected execution against the final typed program.
//! The resulting checked program and its sidecars remain one owned handoff.
//!
//! Settlement is split from checking: [`settle_initial_execution`] records one
//! target's initial source-dependent selection — calling plans, provider
//! settlement, synchronous invocations and pending constant evaluation — as
//! data over the shared typed product. [`unify_settled_execution_views`] then
//! merges the settled views across targets — declaration-scoped completions
//! union, converged views share one storage — and
//! [`check_selected_execution`] binds that record to the language judgment.
//! The driver settles every target before checking any, so per-target
//! selection output is recorded provenance rather than interleaved mutation
//! of the input the next target reuses.

use artifacts::compile_timings::CompileTimings;
use compilation_products::{CheckedCompilationInputs, PreparedProductSource};
use diagnostics::Diagnostic;
use package_compilation::PackageCompilationInputs;
use selected_dispatch::{
    ProviderCheckingInputs, ProviderCheckingRequest, SelectedConstantEvaluation,
    SelectedExecutionRequest, settle_selected_execution,
};

/// One target's initial source-dependent selection over the shared typed
/// product. Settlement's typed mutations travel inside this record's settled
/// view; [`unify_settled_execution_views`] lets targets whose settlements
/// converged share the same storage, and equal views share one language
/// judgment.
pub(crate) struct InitialExecutionSelection {
    typed: std::sync::Arc<typed_trees::TypedTrees>,
    computed_build_config: std::sync::Arc<compilation_products::ComputedBuildConfig>,
    admission: compilation_products::BuildAdmission,
    checked_provider_selection: provider_planning::CheckedProviderSelection,
    x86_scalar_fma_provider: Option<target::AdmittedX86ScalarFmaProvider>,
    selected_build_machine_symbol: Option<symbols::SymbolHandle>,
    /// The evaluation that folded this target's pending constants — retained
    /// so the driver can reject when targets selected different providers
    /// for the same pending use before checking binds the views.
    constant_evaluation: Option<SelectedConstantEvaluation>,
}

impl InitialExecutionSelection {
    /// The provider rows this target folded pending source constants under —
    /// empty when no pending evaluation ran (the same as having folded none).
    pub(crate) fn pending_fold_providers(
        &self,
    ) -> (
        &[build_time_evaluation::SelectedBuildTimeBinaryOperator],
        &[build_time_evaluation::SelectedBuildTimeProviderBody],
    ) {
        self.constant_evaluation
            .as_ref()
            .map(SelectedConstantEvaluation::pending_fold_providers)
            .unwrap_or_default()
    }

    /// The settled view, for cross-target comparisons that run before
    /// checking binds it.
    pub(crate) fn settled_typed(&self) -> &typed_trees::TypedTrees {
        &self.typed
    }
}

/// Describe the first pending use whose selected providers disagree, for the
/// const-exception diagnostic. The use rows enumerate the same resolved
/// selections on every target's view, so an index-wise walk finds the
/// disagreement; `None` means the selections agree.
pub(crate) fn first_divergent_pending_fold(
    left: &InitialExecutionSelection,
    right: &InitialExecutionSelection,
) -> Option<String> {
    let (left_operators, left_bodies) = left.pending_fold_providers();
    let (right_operators, right_bodies) = right.pending_fold_providers();
    let typed = left.settled_typed();
    let machine_name = |symbol| {
        typed
            .machine(symbol)
            .map(|machine| machine.name.to_string())
            .unwrap_or_else(|| "an unnamed machine".to_owned())
    };
    for (left_row, right_row) in left_operators.iter().zip(right_operators.iter()) {
        if left_row != right_row {
            return Some(format!(
                "pending constant `{}` (selected operators `{:?}` and `{:?}`)",
                machine_name(left_row.requirement),
                left_row.operation,
                right_row.operation,
            ));
        }
    }
    if left_operators.len() != right_operators.len() {
        return Some("a pending constant's selected operator rows".to_owned());
    }
    for (left_row, right_row) in left_bodies.iter().zip(right_bodies.iter()) {
        if left_row != right_row {
            return Some(format!(
                "pending constant `{}` (providers `{}` and `{}`)",
                machine_name(left_row.requirement),
                machine_name(left_row.provider_machine),
                machine_name(right_row.provider_machine),
            ));
        }
    }
    if left_bodies.len() != right_bodies.len() {
        return Some("a pending constant's selected provider rows".to_owned());
    }
    None
}

/// Record one target's initial source-dependent selection: provider defaults,
/// boundary calling plans, provider settlement and pending const/layout
/// evaluation, all resolved against this target's view of the shared typed
/// product. The returned record carries the settled typed view and every
/// settlement output checking later binds.
pub(crate) fn settle_initial_execution(
    source: PreparedProductSource,
    selected_target_profile: Option<target::TargetProfile>,
    package_inputs: Option<&PackageCompilationInputs>,
) -> Result<InitialExecutionSelection, Vec<Diagnostic>> {
    let PreparedProductSource {
        mut typed,
        pending_pre_checks,
        computed_build_config,
        mut admission,
        ..
    } = source;
    admission.provider_defaults = provider_planning::target_machines::retain_provider_defaults(
        provider_planning::ProviderPlanningContext::new(&typed, source::DependencyScope::Product),
    )?;
    let selected_build_machine_symbol = computed_build_config.selected_build_machine_symbol;
    let build_config = &computed_build_config.config;
    // Selection accepts every canonical profile spelling, including profiles
    // this compiler does not natively realize (alpha_bootstrap); reject those
    // legibly at the first point native planning demands a `NativeTarget`,
    // matching the build-continuation guard.
    let selected_native_target = match selected_target_profile {
        Some(profile) => profile.native_realization().ok_or_else(|| {
            vec![Diagnostic::error(format!(
                "native realization for target profile `{}` is not implemented",
                profile.target_name(),
            ))]
        })?,
        None => target::NativeTarget::host(),
    };
    let mut boundary_calling_plan_realizations =
        provider_planning::calling_policy_plans::compute_boundary_calling_plans(
            &typed,
            selected_native_target,
            &build_config.opaque_representation_selections,
            package_inputs,
        )?;
    // Provider schemas and selection provenance commit to the complete calling
    // application. Close its native callback geometry before those consumers
    // capture it, rather than changing their predecessor after Psi checking.
    // Layout and policy evaluation consume typed structure; checked callback
    // authority is still bound only after the language judgment below.
    if selected_target_profile.is_some() {
        provider_planning::calling_policy_plans::close_outbound_callback_materializations(
            &typed,
            &mut boundary_calling_plan_realizations,
            selected_native_target,
            &build_config.opaque_representation_selections,
            package_inputs,
        )?;
    }
    let opaque_representation_selections = &build_config.opaque_representation_selections;
    let x86_scalar_fma_provider = build_config
        .settings
        .x86_deployment_claim
        .admit_for(selected_target_profile)
        .map_err(|reason| vec![Diagnostic::error(reason)])?;
    let settled_target_machines = provider_planning::target_machines::settle_provider_defaults(
        provider_planning::ProviderPlanningContext::new(&typed, source::DependencyScope::Product),
        &admission.provider_defaults,
        selected_target_profile.unwrap_or_else(target::TargetProfile::host),
    );
    // Synchronous-invocation inference is a checking product: the driver
    // computes it once here and realization consumes the plan rather than
    // re-deriving it from typed structure.
    let inferred_synchronous_invocations = validation::infer_synchronous_invocations(&typed);
    let checked_provider_selection = provider_planning::settle_checked_providers(
        &mut typed,
        settled_target_machines,
        selected_target_profile,
        package_inputs,
        &build_config.provider_selections,
        &build_config.accepted_component_assumption_digests(),
        boundary_calling_plan_realizations,
        &build_config.opaque_representation_selections,
        &inferred_synchronous_invocations,
    )?;
    // Deferred const work decides the block; a surviving `size_of`/`align_of`
    // query must fold under this target's custody even when nothing else
    // deferred (e.g. a query in a purely runtime position).
    let has_layout_queries = typed
        .expression_table
        .iter_expressions()
        .any(|(_, expression)| match expression {
            typed_trees::expression::ExpressionNode::Call(call) => call.layout_query.is_some(),
            _ => false,
        });
    let mut constant_evaluation = None;
    if !pending_pre_checks.is_empty() || has_layout_queries {
        let operator_selections =
            typed_trees_to_checked_trees::derive_pre_flow_operator_selections(&typed);
        let evaluation = SelectedConstantEvaluation::prepare(
            provider_planning::ProviderPlanningContext::new(
                &typed,
                source::DependencyScope::Product,
            ),
            selected_native_target,
            opaque_representation_selections,
            &operator_selections,
            &checked_provider_selection.selected_provider_plan_facts,
        )?;
        constant_evaluation = Some(evaluation.clone());
        evaluation.evaluate_pending(&mut typed, pending_pre_checks)?;
    }
    Ok(InitialExecutionSelection {
        typed: std::sync::Arc::new(typed),
        computed_build_config,
        admission,
        checked_provider_selection,
        x86_scalar_fma_provider,
        selected_build_machine_symbol,
        constant_evaluation,
    })
}

/// Unify the settled views across targets before any checking runs.
///
/// A boundary completion is a fact of the requirement declaration — the
/// provider that selected the external row was only the discovery channel —
/// so the view every target checks carries the union of every settle's
/// discovered declarations. Views whose settlements then converged share one
/// storage. Pending folds already landed inside each view: targets whose
/// selected evaluations disagree keep their own view, while targets that
/// folded to the same program converge.
pub(crate) fn unify_settled_execution_views<'selection>(
    selections: impl IntoIterator<Item = &'selection mut InitialExecutionSelection>,
) {
    let mut selections: Vec<&mut InitialExecutionSelection> =
        selections.into_iter().collect();
    if selections.len() < 2 {
        return;
    }
    // A requirement's completion declaration is deterministic in the shared
    // program: the same requirement can only produce the same declaration, so
    // the first view that discovered it speaks for the union.
    let mut union: Vec<typed_trees::BoundaryCompletionDeclaration> = Vec::new();
    for selection in &selections {
        for (_, declaration) in selection.typed.boundary_completion_declarations.iter() {
            if !union
                .iter()
                .any(|declared| declared.requirement == declaration.requirement)
            {
                union.push(declaration.clone());
            }
        }
    }
    if !union.is_empty() {
        let mut declarations = arena::Arena::default();
        declarations.insert_many(union);
        for selection in &mut selections {
            let typed = std::sync::Arc::make_mut(&mut selection.typed);
            if typed.boundary_completion_declarations != declarations {
                typed.boundary_completion_declarations = declarations.clone();
            }
        }
    }
    for index in 0..selections.len() {
        for next in index + 1..selections.len() {
            if !std::sync::Arc::ptr_eq(&selections[index].typed, &selections[next].typed)
                && *selections[index].typed == *selections[next].typed
            {
                let shared = selections[index].typed.clone();
                selections[next].typed = shared;
            }
        }
    }
}

/// Check one settled selection against the shared typed view it carries, then
/// bind the judgment to this target's entry, providers and selected dispatch.
pub(crate) fn check_selected_execution(
    selection: InitialExecutionSelection,
    selected_target_profile: Option<target::TargetProfile>,
    package_inputs: Option<&PackageCompilationInputs>,
    optimization_rollback: &crate::checked::OptimizationRollback,
    timings: &mut CompileTimings,
    permit_unsettled_fused_service_fields: bool,
    settled_check_reuse: compilation_products::SettledCheckReuse,
) -> Result<
    (
        CheckedCompilationInputs,
        Option<typed_trees_to_checked_trees::RetainedCheck>,
    ),
    Vec<Diagnostic>,
> {
    let InitialExecutionSelection {
        typed,
        computed_build_config,
        admission,
        checked_provider_selection,
        x86_scalar_fma_provider,
        selected_build_machine_symbol,
        constant_evaluation: _,
    } = selection;
    let provider_planning::CheckedProviderSelection {
        settlement: providers,
        selected_provider_plan_facts,
        selected_provider_provenance,
    } = checked_provider_selection;
    let build_config = &computed_build_config.config;
    let opaque_representation_selections = &build_config.opaque_representation_selections;
    let root_grants = build_config.root_grant_selectors();
    let provider_request = ProviderCheckingRequest {
        package_inputs,
        selected_build_machine: selected_build_machine_symbol,
        boundary_calling_plan_realizations: &providers.boundary_calling_plan_realizations,
        opaque_representation_selections,
        provider_plans: &providers.provider_plans,
        selected_provider_plan_facts,
        root_grants: &root_grants,
        authored_root_grants: &build_config.grants,
    };
    let prepared = ProviderCheckingInputs::prepare(&typed, &provider_request)?;
    // Source preparation/checking requests belong to the source coordinator,
    // not to provider settlement. Realization contributes admitted facts only.
    let preparation_request = typed_trees_to_typed_trees::PreparationRequest::settled()
        .with_opaque_property_receipts(prepared.opaque_property_receipts())
        .with_asm_authority(
            validation::AsmAuthorityAdmission::from_freestanding(
                build_config.settings.freestanding,
            )
            .with_grants(
                build_config.settings.privileged_services.port_io,
                build_config.settings.privileged_services.interrupt_table,
            ),
        )
        .with_wire_compatibility_demands(&build_config.wire_compatibility_demands);
    let checking = typed_trees_to_checked_trees::CheckingRequest::new().with_declaration_authority(
        package_inputs.map(|inputs| inputs as &dyn validation::DeclarationSelectionAuthority),
    );
    // Package review's bound pass hands back the discovery pass's judgment of
    // this occurrence. Bindings choose providers after the language judgment,
    // so settlement usually arrives at the same typed program; Psi takes the
    // judgment only when the whole input is equal. A taken judgment is not
    // counted as a settled check.
    let program = timings.record(
        artifacts::compile_timings::TYPED_TREES_TO_CHECKED_TREES,
        || {
            let started = std::time::Instant::now();
            let admission = preparation_request.admission();
            let retain = settled_check_reuse.retain;
            // The invocation's earlier product judgments: its own slot chain
            // plus every earlier target's retained check. A target that
            // converges with any earlier input takes that judgment instead
            // of repeating the whole-program check.
            let prior = settled_check_reuse
                .prior
                .product
                .iter()
                .chain(settled_check_reuse.prior.sibling_product_checks.iter())
                .find(|prior| prior.judges_preparation_input(&typed, &admission));
            let result = match prior {
                Some(prior) => {
                    typed_trees_to_checked_trees::reuse_retained_check(prior, &checking, retain)
                }
                None => {
                    // The retained judgment's input is the settled view
                    // itself: an Arc clone keeps it beside the admission
                    // snapshot instead of deep-cloning the whole program.
                    let retained_input = retain.then(|| (typed.clone(), admission));
                    typed_trees_to_typed_trees::prepare_typed_trees(
                        std::sync::Arc::try_unwrap(typed)
                            .unwrap_or_else(|shared| (*shared).clone()),
                        &preparation_request,
                    )
                        .and_then(|(typed, preparation)| {
                            typed_trees_to_checked_trees::lower_typed_trees_reusing(
                                typed,
                                preparation,
                                retained_input,
                                &checking,
                                prior,
                                retain,
                            )
                        })
                }
            };
            if result.as_ref().is_ok_and(|outcome| outcome.took_prior) {
                artifacts::compile_timings::counters::record_settled_check_reuse();
            } else {
                artifacts::compile_timings::counters::record_settled_check(started.elapsed());
            }
            result
        },
    )?;
    let typed_trees_to_checked_trees::CheckingOutcome {
        checked: program,
        retained: settled_check,
        ..
    } = program;
    // Entry provisioning consumes the completed source, including exact data
    // applications and landed default-domain facts. Running it on the raw
    // frontend graph makes an unrelated method binder decide whether an entry
    // context has a valid zero image. Provider selection and constant execution
    // do not depend on the selected entry; they must finish before preparation.
    let selected_program_entry = provider_planning::program_entry::select_compiler_program_entry(
        &program.typed,
        &build_config.root_bindings,
        selected_target_profile,
        provider_request.boundary_calling_plan_realizations,
        package_inputs,
        |program, definition| validation::data_requires_establishment(program, definition),
    )?;
    // Check the common source before asking which declarations this product
    // realizes. In particular, a missing portable family contract is not a
    // missing target body, and must not be disguised as one by reach analysis.
    provider_planning::target_machines::reject_reached_inert_sibling_calls(
        &program.typed,
        selected_target_profile.unwrap_or_else(target::TargetProfile::host),
        selected_program_entry
            .as_ref()
            .map(|entry| entry.machine_symbol()),
        &selected_provider_provenance,
    )?;
    let mut checked = prepared.bind(std::sync::Arc::new(program), provider_request)?;
    // Product target bodies remain checked input to Terminal production.
    // Only Omega realization may discard another target's implementation.
    // Check all authored code before product pruning and sidecar derivation.
    let product_root_machines = build_config.checked_tree_product_roots(
        selected_target_profile,
        selected_program_entry.as_ref(),
        optimization_rollback,
    );
    let (program, product_selection) = typed_trees_to_checked_trees::select_checked_tree_product(
        checked.program,
        product_root_machines,
    )?;
    checked.program = program;
    let selected_execution_settlement = settle_selected_execution(
        checked,
        SelectedExecutionRequest {
            calling_plans: &providers.boundary_calling_plan_realizations,
            selected_program_entry,
            selected_target_profile,
            package_inputs,
            x86_scalar_fma_provider,
            permit_unsettled_fused_service_fields,
            selected_provider_provenance,
            opaque_representation_selections,
        },
    )?;

    Ok((
        CheckedCompilationInputs {
            settled: selected_execution_settlement,
            build: computed_build_config,
            build_admission: admission.for_target(selected_target_profile),
            providers,
            product_selection,
        },
        settled_check,
    ))
}

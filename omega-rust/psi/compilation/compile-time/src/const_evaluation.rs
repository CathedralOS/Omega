//! Evaluating closed integer facts at build time: domain facts,
//! const-generic calls and expressions, initializers, fixed-array lengths,
//! and named range arguments and endpoints.

pub(crate) mod const_applications;
pub(crate) mod const_domain_facts;
pub(crate) mod const_generic_calls;
pub(crate) mod const_generic_expressions;
pub(crate) mod const_initializers;
pub(crate) mod const_lengths;
pub(crate) mod data_layout_queries;
pub(crate) mod range_arguments;
pub(crate) mod range_endpoints;

use typed_trees::TypedTrees;

/// The shared round preamble: rebind authored provider selections on a
/// private copy, prepare that execution, and infer its admission plan.
/// Callers keep `prepared` alive for the round's `execution` tree.
pub(crate) fn prepared_admission(
    typed: &TypedTrees,
    provider_bodies: &[crate::SelectedBuildTimeProviderBody],
    operators: &[crate::SelectedBuildTimeBinaryOperator],
    selection_authority: Option<std::sync::Arc<dyn crate::BuildTimeSelectionAuthority>>,
    domain: &crate::build_time_evaluation::EvaluationDomain,
    roots: &[symbols::SymbolHandle],
    admission: crate::build_time_evaluation::BuildTimeCheckingAdmission<'_>,
) -> Result<
    (
        crate::PreparedBuildMachineProgram,
        crate::BuildTimeAdmissionPlan,
    ),
    Vec<diagnostics::Diagnostic>,
> {
    let selected_execution =
        crate::machine_execution::selected_operators::prepare_selected_execution(
            typed,
            provider_bodies,
            operators,
        )
        .map_err(|reason| vec![diagnostics::Diagnostic::error(reason)])?;
    let selected_execution = selected_execution.as_ref().unwrap_or(typed);
    let roots = crate::machine_execution::selected_operators::selected_execution_roots(
        roots, provider_bodies,
    );
    // Complete only the actual invocation dependencies. The original selected
    // map and public requirement remain roots, so private body binding cannot
    // erase conformance/argument obligations or check unrelated pending geometry.
    let domain = domain.demanded_roots(selected_execution, &roots);
    let mut execution = domain.execution_program(selected_execution);
    crate::const_evaluation::range_endpoints::defer_pending_endpoint_calls(&mut execution)?;
    let (execution, preparation) = typed_trees_to_typed_trees::prepare_typed_trees(
        execution, &admission.preparation,
    )?;
    let checked = typed_trees_to_checked_trees::lower_typed_trees(
        execution, preparation, &admission.checking,
    )?;
    let prepared = crate::PreparedBuildMachineProgram::from_checked(checked);
    let execution = prepared.typed();
    let admission = crate::BuildTimeAdmissionPlan::infer_prepared(&prepared, selection_authority)
        .map_err(|reason| vec![diagnostics::Diagnostic::error(reason)])?
        .with_selected_operators(execution, operators)
        .map_err(|reason| vec![diagnostics::Diagnostic::error(reason)])?;
    Ok((prepared, admission))
}

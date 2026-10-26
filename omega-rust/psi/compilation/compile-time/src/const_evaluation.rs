//! Evaluating closed integer facts at build time: domain facts,
//! const-generic calls and expressions, initializers, fixed-array lengths,
//! and named range arguments and endpoints.

pub(crate) mod const_applications;
pub(crate) mod const_domain_facts;
pub(crate) mod const_generic_calls;
pub(crate) mod const_generic_expressions;
pub(crate) mod const_initializers;
pub(crate) mod const_lengths;
pub(crate) mod const_qualifications;
pub(crate) mod constant_recipes;
pub(crate) mod data_layout_queries;
mod initializer_probe;
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
    let prepared = crate::PreparedBuildMachineProgram::prepare_in(
        selected_execution.as_ref().unwrap_or(typed),
        domain,
    )?;
    let execution = prepared.typed();
    let admission = domain
        .admission(execution, selection_authority)
        .with_selected_operators(execution, operators)
        .map_err(|reason| vec![diagnostics::Diagnostic::error(reason)])?;
    Ok((prepared, admission))
}

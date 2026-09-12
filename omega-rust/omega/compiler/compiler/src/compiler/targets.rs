//! Exact-target compilation shares immutable preparation and retains each child's outcome.

use super::request::ValidatedCompileRequest;
use super::{
    ExactTargetCompileOutcome, ExplicitTargetSet, MultiTargetCompileOutcomes,
    MultiTargetCompileRequest, RequestedCompileProduct, TrustAdmissionSettlement,
    compile_checked_with_observations, compile_validated,
};
use diagnostics::Diagnostic;

/// Compile every canonical exact child while retaining each ordinary result.
///
/// Request-shape failures reject before source acquisition. Once admitted, a
/// shared source-preparation failure and every child-local failure remain one
/// ordered outcome per requested target.
pub(super) fn compile_targets(
    request: MultiTargetCompileRequest,
) -> Result<MultiTargetCompileOutcomes, Vec<Diagnostic>> {
    let request = request.validate_batch_for_execution()?;
    let (target_set, children) = request.into_parts();
    let prepared = {
        let first = children
            .first()
            .expect("validated explicit target set retains one child");
        crate::pipeline::checked_entry::PreparedCheckedSource::prepare(
            &first.options().root_path,
            first.package_inputs(),
        )
    };
    let prepared = match prepared {
        Ok(prepared) => prepared,
        Err(diagnostics) => {
            let outcomes = target_set
                .profiles()
                .iter()
                .copied()
                .map(|target| ExactTargetCompileOutcome::new(target, Err(diagnostics.clone())))
                .collect();
            return Ok(MultiTargetCompileOutcomes::new(target_set, outcomes));
        }
    };
    if children
        .first()
        .expect("validated explicit target set retains one child")
        .requested_product()
        == RequestedCompileProduct::NativeArtifact
    {
        return compile_native_targets(target_set, children, &prepared);
    }
    let outcomes = target_set
        .profiles()
        .iter()
        .copied()
        .zip(children)
        .map(|(target, child)| {
            ExactTargetCompileOutcome::new(target, compile_validated(child, Some(&prepared)))
        })
        .collect();
    Ok(MultiTargetCompileOutcomes::new(target_set, outcomes))
}

fn compile_native_targets(
    target_set: ExplicitTargetSet,
    children: Vec<ValidatedCompileRequest>,
    prepared_source: &crate::pipeline::checked_entry::PreparedCheckedSource,
) -> Result<MultiTargetCompileOutcomes, Vec<Diagnostic>> {
    let staged = children
        .into_iter()
        .map(|request| {
            let (checked, trust_settlement) =
                compile_checked_with_observations(&request, Some(prepared_source))?;
            let prepared = super::optimization::prepare_native_report(request, checked)?;
            Ok((prepared, trust_settlement))
        })
        .collect::<Vec<
            Result<
                (
                    super::optimization::PreparedNativeReport,
                    TrustAdmissionSettlement,
                ),
                Vec<Diagnostic>,
            >,
        >>();

    let mut reusable_inputs = Vec::<(
        super::optimization::NativeInputReuseKey,
        Result<native_realization::PreparedNativeRealizationInput, Vec<Diagnostic>>,
    )>::new();
    for (prepared, _) in staged.iter().filter_map(|result| result.as_ref().ok()) {
        let key = prepared.reuse_key();
        if reusable_inputs.iter().any(|(existing, _)| *existing == key) {
            continue;
        }
        reusable_inputs.push((key, prepared.prepare_reusable_input()));
    }
    let prepared_input_count = reusable_inputs.len();

    let outcomes = target_set
        .profiles()
        .iter()
        .copied()
        .zip(staged)
        .map(|(target, staged)| {
            let result = match staged {
                Err(diagnostics) => Err(diagnostics),
                Ok((prepared, trust_settlement)) => {
                    let key = prepared.reuse_key();
                    let reusable_input = reusable_inputs
                        .iter()
                        .find(|(existing, _)| *existing == key)
                        .expect("every prepared native child has one exact reuse group");
                    match &reusable_input.1 {
                        Ok(reusable_input) => prepared
                            .finish(reusable_input)
                            .map(|report| report.with_trust_admission_settlement(trust_settlement)),
                        Err(diagnostics) => Err(diagnostics.clone()),
                    }
                }
            };
            ExactTargetCompileOutcome::new(target, result)
        })
        .collect();
    Ok(MultiTargetCompileOutcomes::new(target_set, outcomes)
        .with_prepared_terminal_native_input_count(prepared_input_count))
}

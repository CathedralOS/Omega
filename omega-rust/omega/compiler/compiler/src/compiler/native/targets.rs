//! Prepare independent native targets, then reuse only exactly matching Terminal inputs.

use crate::compiler::request::ValidatedTargetCompilation;
use crate::compiler::{CompileOutcomes, CompileTargetOutcome, check_request};
use crate::pipeline::checked_entry::PreparedCheckedSource;
use diagnostics::Diagnostic;

pub(in crate::compiler) fn compile_targets(
    mut targets: Vec<ValidatedTargetCompilation>,
    prepared_source: PreparedCheckedSource,
) -> Result<CompileOutcomes, Vec<Diagnostic>> {
    let last = targets.pop().ok_or_else(|| {
        vec![Diagnostic::error(
            "validated native compilation lost every target",
        )]
    })?;
    let prepare_target = |request: ValidatedTargetCompilation, source| {
        let profile = request.profile;
        let result = check_request(&request, source).and_then(|(checked, trust_settlement)| {
            super::prepare(request, checked).map(|prepared| (prepared, trust_settlement))
        });
        (profile, result)
    };
    let mut staged = targets
        .into_iter()
        .map(|request| prepare_target(request, prepared_source.clone()))
        .collect::<Vec<_>>();
    staged.push(prepare_target(last, prepared_source));

    let mut reusable_inputs = Vec::<(
        super::NativeInputReuseKey,
        Result<native_realization::PreparedNativeRealizationInput, Vec<Diagnostic>>,
    )>::new();
    for (prepared, _) in staged.iter().filter_map(|(_, result)| result.as_ref().ok()) {
        let key = prepared.reuse_key();
        if reusable_inputs.iter().any(|(existing, _)| *existing == key) {
            continue;
        }
        reusable_inputs.push((key, prepared.prepare_reusable_input()));
    }
    let prepared_input_count = reusable_inputs.len();
    let outcomes = staged
        .into_iter()
        .map(|(profile, staged)| {
            let result = staged.and_then(|(prepared, trust_settlement)| {
                let key = prepared.reuse_key();
                let (_, reusable_input) = reusable_inputs
                    .iter()
                    .find(|(existing, _)| *existing == key)
                    .ok_or_else(|| {
                        vec![Diagnostic::error(
                            "prepared native target lost its Terminal input reuse group",
                        )]
                    })?;
                match reusable_input {
                    Ok(input) => prepared
                        .finish(input)
                        .map(|report| report.with_trust_admission_settlement(trust_settlement)),
                    Err(diagnostics) => Err(diagnostics.clone()),
                }
            });
            CompileTargetOutcome::new(profile, result)
        })
        .collect();
    Ok(CompileOutcomes::new(outcomes)
        .with_prepared_terminal_native_input_count(prepared_input_count))
}

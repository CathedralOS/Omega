//! Bind exact policy entries into the invocation's shared Product publication.
//!
//! Target-specific admission and signature preparation remain with provider
//! planning. All target children borrow this one immutable code catalog; no
//! policy invocation compiles source or falls back to the checked interpreter.
//! This owner publishes no policy-only artifact.

use compilation_products::CheckedProductSource;
use diagnostics::Diagnostic;
use provider_planning::calling_policy_plans::CallingPolicyCode;
use std::sync::Arc;

pub(crate) fn retain_calling_policy_code(
    source: &CheckedProductSource,
    timings: &mut artifacts::compile_timings::CompileTimings,
) -> Result<Option<Arc<CallingPolicyCode>>, Vec<Diagnostic>> {
    source
        .calling_policy_code
        .get_or_init(|| prepare(source, timings))
        .clone()
}

fn prepare(
    source: &CheckedProductSource,
    timings: &mut artifacts::compile_timings::CompileTimings,
) -> Result<Option<Arc<CallingPolicyCode>>, Vec<Diagnostic>> {
    let code = super::product_code::ProductCodeSource::checked(source);
    let machines = code.calling_policy_roots()?;
    if machines.is_empty() {
        return Ok(None);
    }
    let applications = machines
        .iter()
        .map(|machine| {
            psi_compiler::TerminalApplicationCustody::prepare(
                &source.program,
                psi_compiler::TerminalMachineSelection::Symbol(*machine),
                None,
                Some(psi_compiler::TerminalCompositionRoots::default()),
                source.sources.exact_toolchain_sources(),
                &mut psi_compiler::UnmeasuredProduction,
            )
        })
        .collect::<Vec<_>>();
    let program = code.publish(
        source.terminal_publication.optimizations(),
        &mut crate::observations::PsiStageTimings::new(timings),
    )?;
    let entries = machines
        .into_iter()
        .zip(applications)
        .map(|(machine, application)| {
            let entry = program
                .attach_application(&application, &mut psi_compiler::UnmeasuredProduction)
                .and_then(|application| {
                    let [root] = <[_; 1]>::try_from(application.roots).map_err(|_| {
                        vec![Diagnostic::error(
                            "calling-policy publication did not retain one exact root",
                        )]
                    })?;
                    root.map(|root| root.machine)
                });
            (machine, entry)
        })
        .collect();
    Ok(Some(Arc::new(CallingPolicyCode {
        source_owner: Arc::downgrade(&source.program),
        artifact: Arc::clone(program.artifact()),
        entries,
        source_records: program.source_records().to_vec().into(),
    })))
}

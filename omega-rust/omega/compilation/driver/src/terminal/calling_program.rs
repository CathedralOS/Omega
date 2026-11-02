//! Retain calling declarations and policy entries in the shared Product publication.
//!
//! Target-specific admission and signature preparation remain with provider
//! planning. All target children borrow this one immutable code catalog; no
//! policy invocation compiles source or falls back to the checked interpreter.
//! This owner publishes no policy-only artifact.

use compilation_products::CheckedProductSource;
use diagnostics::Diagnostic;
use provider_planning::calling_policy_plans::CallingProgram;
use std::sync::Arc;

pub(crate) fn retain_calling_program(
    source: &CheckedProductSource,
    timings: &mut artifacts::compile_timings::CompileTimings,
) -> Result<Arc<CallingProgram>, Vec<Diagnostic>> {
    source
        .calling_program
        .get_or_init(|| prepare(source, timings))
        .clone()
}

fn prepare(
    source: &CheckedProductSource,
    timings: &mut artifacts::compile_timings::CompileTimings,
) -> Result<Arc<CallingProgram>, Vec<Diagnostic>> {
    let code = super::product_code::ProductCodeSource::checked(source);
    let program = code.publish(
        source.terminal_publication.optimizations(),
        &mut crate::observations::PsiStageTimings::new(timings),
    )?;
    Ok(Arc::new(CallingProgram {
        source_owner: Arc::downgrade(&source.program),
        artifact: Arc::clone(program.artifact()),
        source_records: program.source_records().to_vec().into(),
        evaluator: build_evaluation::evaluate_terminal_calling_policy,
    }))
}

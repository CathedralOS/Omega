//! Retain calling declarations and policy entries in the shared Product publication.
//!
//! Original review correspondence and inert helper choices are captured once
//! before publication. All target children borrow this immutable catalog; no
//! policy invocation compiles source or falls back to the checked interpreter.
//! This owner publishes no policy-only artifact.

use compilation_products::CheckedProductSource;
use diagnostics::Diagnostic;
use provider_planning::calling_policy_plans::{
    CallingPolicyProviderInputs, CallingProgram, CallingSourcePreparation,
};
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
    // Capture Source relationships while code is published: both only read the
    // checked source. Publication's refusal still takes priority over the
    // capture's. Targets receive only this owned result; Calling construction
    // does not reopen Source after Terminal publication.
    let (program, source_preparation) = std::thread::scope(|scope| {
        let capture = crate::checked::checking::compile_thread::spawn_compile_worker(scope, || {
            capture_source_preparation(source)
        });
        let code = super::product_code::ProductCodeSource::checked(source);
        let program = code.publish(
            source.terminal_publication.optimizations(),
            &mut crate::observations::PsiStageTimings::new(timings),
        );
        let source_preparation = match capture {
            Ok(worker) => worker
                .join()
                .unwrap_or_else(|panic| std::panic::resume_unwind(panic)),
            // No spare thread: capture here, exactly as it ran before.
            Err(_) => capture_source_preparation(source),
        };
        (program, source_preparation)
    });
    let program = program?;
    let source_preparation = source_preparation?;
    Ok(Arc::new(CallingProgram {
        source_owner: Arc::downgrade(&source.program),
        artifact: Arc::clone(program.artifact()),
        source_records: program.source_records().to_vec().into(),
        source_preparation: Arc::new(source_preparation),
        evaluator: build_settings::evaluate_terminal_calling_policy,
    }))
}

/// Keep all fallible declaration acquisition in the captured Result, including
/// owner/census refusals. Publication retains its original diagnostic priority.
fn capture_source_preparation(
    source: &CheckedProductSource,
) -> Result<CallingSourcePreparation, Vec<Diagnostic>> {
    CallingSourcePreparation::capture(
        &source.program,
        source
            .provider_inventory
            .services_for(&source.program)
            .map_err(|error| vec![error])?,
        &source
            .computed_build_config
            .config
            .opaque_representation_selections,
        CallingPolicyProviderInputs {
            body_declarations: source
                .provider_inventory
                .direct_calls_for(&source.program)
                .map_err(|error| vec![error])?,
            origins: source
                .provider_inventory
                .origins_for(&source.program)
                .map_err(|error| vec![error])?,
            defaults: &source.provider_defaults,
            requested: &source.computed_build_config.config.provider_selections,
            exact_toolchain_sources: source.sources.exact_toolchain_sources(),
        },
    )
}

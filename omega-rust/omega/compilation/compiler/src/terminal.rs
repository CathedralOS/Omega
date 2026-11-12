/*
We publish portable Terminal Psi code for a Checked Product once and attach each
target's selected entry, provider plans, and behavior-exclusion policy.

When target provider selection needs boundary calling plans, `retain_calling_program`
captures the source calling relationships (`CallingSourcePreparation`) in parallel
with publishing the shared `PortableTerminalProgram` (`product_code.rs`) and caches
the immutable `CallingProgram` on `CheckedProductSource` so target children never
reopen Checked Psi after publication.
*/

/*
Ordinary target applications (`terminal_artifact.rs`) reuse that shared
publication, attach their selected `ProgramEntry` and callback roots, and verify
Omega behavior exclusions (`behavior_exclusions.rs`) without rerunning Psi
lowering. Named machine inspection (`TerminalProductUse::Inspection`) uses an
explicit source-producing entrance because it may select a machine root outside
the executable application closure.
*/

mod behavior_exclusions;
mod product_code;
mod terminal_artifact;

pub(crate) use terminal_artifact::{
    ProducedTerminalProduct, TerminalProductRequest, TerminalProductUse, TerminalProductionView,
    check_terminal_conditions, produce_retained_terminal_artifacts,
};

use compilation_products::CheckedProductSource;
use diagnostics::Diagnostic;
use provider_planning::calling_policy_plans::{
    CallingPolicyProviderInputs, CallingProgram, CallingSourcePreparation,
    PreparedOpaqueReviewSelection,
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
    let (program, source_preparation) = std::thread::scope(|scope| {
        let capture = crate::checked::checking::compile_thread::spawn_compile_worker(scope, || {
            capture_source_preparation(source)
        });
        let code = product_code::ProductCodeSource::checked(source);
        let program = code.publish(
            source.terminal_publication.optimizations(),
            &mut crate::observations::PsiStageTimings::new(timings),
        );
        let source_preparation = match capture {
            Ok(worker) => worker
                .join()
                .unwrap_or_else(|panic| std::panic::resume_unwind(panic)),
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
        evaluator: build_settings::CALLING_POLICY_EVALUATOR,
    }))
}

fn capture_source_preparation(
    source: &CheckedProductSource,
) -> Result<CallingSourcePreparation, Vec<Diagnostic>> {
    CallingSourcePreparation::capture(
        &source.program,
        source
            .provider_inventory
            .services_for(&source.program)
            .map_err(|error| vec![error])?,
        source
            .computed_build_config
            .config
            .opaque_representation_selections
            .iter()
            .map(|selection| PreparedOpaqueReviewSelection {
                application: selection.portable_application(&source.program.typed),
                opaque: selection.opaque(),
                carrier: selection.carrier(),
                conformance: selection.application().declaration,
            })
            .collect(),
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

//! Products and calling policies consume the same program and evidence accepted
//! by source checking. Calling metadata is captured once beside that artifact;
//! target applications add their entry receipts, provider proposals and behavior
//! exclusions without lowering source again. An explicit named inspection checks
//! its chosen entry through the same source-publication owner.

mod behavior_exclusions;
mod product_code;
pub(crate) mod source_roots;
mod terminal_artifact;

pub(crate) use product_code::ProductCodeSource;
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
    _timings: &mut artifacts::compile_timings::CompileTimings,
) -> Result<Arc<CallingProgram>, Vec<Diagnostic>> {
    // Calling plans use the same checked program and original declarations
    // as execution. Preparing metadata never lowers a second source program.
    let program = source.terminal_publication.checked_application()?;
    if !Arc::ptr_eq(program, &source.program) {
        return Err(vec![Diagnostic::error(
            "calling preparation and executable publication have different source owners",
        )]);
    }
    let source_preparation = capture_source_preparation(source)?;
    Ok(Arc::new(CallingProgram {
        source_owner: Arc::downgrade(&source.program),
        artifact: Arc::clone(program.artifact()),
        source_records: Arc::clone(&program.correspondence().records),
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

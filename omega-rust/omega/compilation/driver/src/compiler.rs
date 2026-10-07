//! Compile shared source into independently checked target products.
//!
//! `compile` prepares the source set once, then runs each requested target
//! through the stage chain in `compile_target`, isolating one target's
//! failure from the others. Every step there is one call; the folders beside
//! this file own what each call does.

use crate::checked::{
    PreparedCheckedSource, check_prepared_source, prepare_checked_source, run_on_compile_thread,
};
use crate::native::produce_native_product;
use crate::terminal::produce_terminal_product;
use crate::{CompileOutcomes, CompileReport, CompileRequest, CompileTargetOutcome};
use compilation_products::compile_request::{SharedCompileInputs, ValidatedTargetCompilation};
use compilation_products::{CheckedProductReport, PreparedBuilds};
use diagnostics::Diagnostic;

pub(crate) mod package;

/// Compile every requested target, retaining failures alongside successful products.
/// Invalid requests reject before source acquisition. Shared preparation failures
/// are reported for every target. Publication remains a separate operation.
pub fn compile(request: CompileRequest) -> Result<CompileOutcomes, Vec<Diagnostic>> {
    run_on_compile_thread(move || {
        let (shared, targets) = request.validate_for_execution()?.into_parts();
        // Psi 00-01: read, tokenize and parse the root's source set once.
        let source = prepare_checked_source(
            &shared.root_path,
            shared.package_sources.clone(),
            shared.timings,
        );
        let target_count = targets.len();
        let mut builds = PreparedBuilds::for_targets(target_count);
        let mut outcomes = Vec::with_capacity(target_count);
        // Checkpoint clones share immutable parsing. repeat_n moves the final
        // copy, so the last (and the only) target consumes the original arenas.
        let sources = std::iter::repeat_n(source, target_count);
        for (target, source) in targets.into_iter().zip(sources) {
            let profile = target.profile();
            let outcome =
                source.and_then(|source| compile_target(&shared, target, source, &mut builds));
            outcomes.push(CompileTargetOutcome::new(profile, outcome));
        }
        Ok(CompileOutcomes::new(outcomes))
    })
}

/// One target through the stage chain: assemble its sources and evaluate the
/// build (once for every target with equal build inputs), check, admit, then
/// produce the requested product.
fn compile_target(
    shared: &SharedCompileInputs,
    target: ValidatedTargetCompilation,
    source: PreparedCheckedSource,
    builds: &mut PreparedBuilds,
) -> Result<CompileReport, Vec<Diagnostic>> {
    let root_path = target.options().root_path.clone();
    let build_dir = target.options().build_dir();
    let child = target.checked_execution(&build_dir);
    let checked = check_prepared_source(&root_path, source, child, Some(builds), None)?.result?;

    // Product ownership admits the stop and retains report companions before
    // the selected stage sequence consumes the checked compilation.
    checked.admit_trust()?;
    let product = checked.prepare_product_report(
        root_path,
        shared.requested_product,
        target.optimization_rollback(),
    )?;
    match product {
        CheckedProductReport::Complete(report) => Ok(report),
        // Psi 05-07: lower, optimize and publish the Terminal artifact.
        CheckedProductReport::Terminal(settlement) => {
            let produced = produce_terminal_product(checked, target.optimization_rollback())?;
            settlement
                .with_timings(produced.timings)
                .finish_terminal(produced.artifact, produced.rollback_receipt)
        }
        // The same retained Terminal boundary, then Omega 00-09 and image emission.
        CheckedProductReport::Native(settlement) => {
            let request = target.into_native_product_request();
            let produced = produce_native_product(request, checked)?;
            settlement
                .with_timings(produced.timings)
                .finish_native(produced.artifact, produced.rollback_receipt)
                .and_then(|report| {
                    report
                        .with_runtime_provisions(produced.runtime_provisions)
                        .map_err(|message| vec![Diagnostic::error(message)])
                })
        }
    }
}

//! Check the program that a Build activation will actually consume.
//!
//! Semantic admission is required with or without package inputs. Package
//! authority is an additional obligation, not the switch that enables checking.
//! Transfer the checker-transformed typed program: checking one clone and
//! interpreting a separately specialized clone would not bind execution to
//! the admitted bodies. The Build interpreter does not yet consume every
//! checked execution fact; this checkpoint does not claim that wider closure.
//!
//! This still checks the whole provisional program. Removing that work requires
//! a sound Build-closure projection and a separately frozen authored-selection
//! frontier; selection visibility alone cannot discharge helper contracts.

use build_time_evaluation::PreparedBuildMachineProgram;
use diagnostics::Diagnostic;
use package_compilation::PackageCompilationInputs;

pub(crate) fn prepare_checked_build(
    typed: &typed_trees::TypedTrees,
    packages: Option<&PackageCompilationInputs>,
    timings: &mut artifacts::compile_timings::CompileTimings,
) -> Result<PreparedBuildMachineProgram, Vec<Diagnostic>> {
    // Package build execution can carry filesystem and other boundary
    // authority. Check the frozen ordinary source graph first; the ordinary
    // final checked pass repeats this gate after any explicit generated-source
    // handoff.
    // Other targets' sibling bodies remain for the settled product check;
    // the Build activation uses its already-selected execution-profile body.
    // Its result feeds Build execution, not the later settled check: the settled
    // program differs (the build may append generated source, target
    // selection swaps canonical bodies, provider settlement records erasure
    // authorizations, and const folds land the endpoints only the
    // preliminary mode tolerates pending), and the settled request closes
    // those tolerances and carries the specialization rosters this request
    // leaves empty. Only the exact-input check memo deduplicates it — for a
    // later pass that arrives at this same frozen graph under this same
    // request.
    let mut provisional = typed.clone();
    typed_trees_to_checked_trees::prune_target_siblings_typed(&mut provisional);
    timings.record(
        artifacts::compile_timings::TYPED_TREES_TO_PRELIMINARY_CHECKED_TREES,
        || {
            let started = std::time::Instant::now();
            let result = typed_trees_to_checked_trees::lower_typed_trees(
                provisional,
                &typed_trees_to_checked_trees::CheckingRequest::preliminary()
                    .with_declaration_authority(
                        packages
                            .map(|inputs| inputs as &dyn validation::DeclarationSelectionAuthority),
                    ),
            );
            artifacts::compile_timings::counters::record_preliminary_check(started.elapsed());
            result.map(PreparedBuildMachineProgram::from_checked)
        },
    )
}

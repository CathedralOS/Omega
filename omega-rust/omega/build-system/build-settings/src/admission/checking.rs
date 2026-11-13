/*
Admit the `Build` program that a `TerminalBuildActivation` will run
(`prepare_admitted_build`). The lowered-form checker judges it, and the
activation runs the module that judgment publishes; nothing here decides
whether the program is valid.

A package's Build sources and its Product sources are two programs checked
under two contexts (`wiki/spec/build/scoped_execution.md`, "Two checked
contexts"). The retired checker separated them by copying the Build
declarations into a second typed tree. That copy is gone. Each of the three
things it did now has one owner, and this file only sequences them:

1. Which declarations the Build program is. The request states it as a
   roster (`TerminalSourceCompilation`): the Build entry, the Build purpose,
   and the subject package. Structural elaboration owns the closure of that
   roster, claims the subject's Build declarations and refuses an entry that
   is not a Build declaration. The judged source is a frozen snapshot of the
   typed frontier taken here, prepared by the same owner that prepares
   Product source (`SourcePreparationRequest`), and it stays with the
   accepted module. The Build gets no private way past that preparation:
   while it serves no program, no Build program reaches the checker.
2. Which packages' declarations the Build may select. That is decided on the
   accepted module, whose declarations carry their package custody, by
   Terminal installation. It has no admission for a Build activation yet, so
   a request that carries package authority is refused after judgment; the
   authority is never dropped.
3. Constants and provider plans. This admission evaluates no constant, so it
   discharges none: the Product's pending work stays whole, and a Build
   program that reads an unevaluated constant is refused by the checker. It
   selects no provider plan; `BuildProviderPlans::admit` holds the published
   module's actual demand against that empty selection before anything runs.
*/

use artifacts::compile_timings::StageMeta;
use diagnostics::Diagnostic;
use package_compilation::PackageCompilationInputs;
use std::sync::Arc;

const BUILD_SOURCE_JUDGMENT: StageMeta = StageMeta::new(
    "build-source-judgment",
    "TypedTrees",
    "AcceptedTerminalSource",
);

/*
A Product frontier typed after Build ran holds the Build source again. Folding
its constants there under the Build's provider plans needs the evaluator that
owns pending constant work (`build_time_evaluation::PreCheckEvaluation`), which
serves no job yet. Admission folds nothing, so the caller asks for this only
when package inputs joined after Build ran.
*/
pub fn settle_rejoined_build_slice(
    _input: psi_compiler::BuildSourceInput<'_>,
    _entry: symbols::SymbolHandle,
    _packages: Option<&PackageCompilationInputs>,
    _dependency_activation: bool,
    _execution_profile: Option<target::TargetProfile>,
) -> Result<(), Vec<Diagnostic>> {
    Err(vec![Diagnostic::error(
        "no product route yet: settling Build constants on a rejoined Product frontier requires the constant evaluator",
    )])
}

pub(crate) fn prepare_admitted_build(
    typed: &typed_trees::TypedTrees,
    entry: symbols::SymbolHandle,
    packages: Option<&PackageCompilationInputs>,
    dependency_activation: bool,
    timings: &mut artifacts::compile_timings::CompileTimings,
) -> Result<
    (
        Arc<psi_compiler::AcceptedTerminalSource>,
        super::provider_execution::BuildProviderPlans,
        bool, // A constant was folded under the provider plans.
    ),
    Vec<Diagnostic>,
> {
    // A dependency's Build executed for its consumer checks what its entry
    // reaches and never promotes the dependency to a subject
    // (`wiki/spec/build/validation_subjects.md`, "Orchestration").
    let subject = (!dependency_activation).then(|| packages.map(PackageCompilationInputs::root));
    let accepted = timings.record_result(BUILD_SOURCE_JUDGMENT, || {
        let source = psi_compiler::SourcePreparationRequest {
            program: typed.clone(),
            admission: psi_compiler::SourceAdmission::default(),
        }
        .prepare()?;
        psi_compiler::AcceptedTerminalSource::judge(
            source.program,
            psi_compiler::TerminalSourceCompilation {
                entries: &[entry],
                subjects: subject.as_slice(),
                purpose: source::DependencyScope::Build,
                selected: &[],
                bindings: &[],
            },
            terminal_codec::TerminalPublication::Invocation,
        )
    })?;
    if packages.is_some() {
        return Err(vec![Diagnostic::error(
            "no product route yet: the accepted Build program has no declaration-selection admission for its package authority",
        )]);
    }
    Ok((
        Arc::new(accepted),
        super::provider_execution::BuildProviderPlans::default(),
        false,
    ))
}

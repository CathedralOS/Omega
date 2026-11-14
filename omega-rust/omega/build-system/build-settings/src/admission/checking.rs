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
   accepted module. Build and Product use the same preparation path.
2. Which packages' declarations the Build may select. `admit_build_program`
   produces this accepted source once and admits its source-selection companion
   against that exact invocation, using separate Build and Product package
   views. Structural validity and package permission remain separate judgments.
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
A Product frontier typed after Build ran holds the Build source again, with
its constants unsettled. What the Build admission evaluated must be settled
there too, or the Product would retain the constant and evaluate it for its
own targets.

The admission states what it owes (`BuildConstants`): whether it folded a
constant under Build's provider plans, and which constants the module it
accepted names, read from that module. When it folded nothing and the module
names no constant, no evaluation of the Build program exists to repeat and
the settlement is complete. Otherwise the rejoined slice needs the evaluator
that owns pending constant work (`build_time_evaluation::PreCheckEvaluation`),
which serves no job yet, so the settlement is refused naming the constants.
That evaluator replaces the refusal: it receives the rejoined frontier, the
Build entry there and the package inputs the Build was admitted under.
*/
pub fn settle_rejoined_build_slice(
    constants: &compilation_products::BuildConstants,
    _rejoined: psi_compiler::BuildSourceInput<'_>,
    _entry: symbols::SymbolHandle,
    _packages: Option<&PackageCompilationInputs>,
    _dependency_activation: bool,
    _execution_profile: Option<target::TargetProfile>,
) -> Result<(), Vec<Diagnostic>> {
    if !constants.folded_under_providers && constants.read.is_empty() {
        return Ok(());
    }
    let mut refusals = constants
        .read
        .iter()
        .map(|name| {
            Diagnostic::error(format!(
                "no product route yet: the Build program reads constant `{name}`, and settling \
                 it on a Product frontier typed after Build ran requires the constant evaluator"
            ))
        })
        .collect::<Vec<_>>();
    if refusals.is_empty() {
        refusals.push(Diagnostic::error(
            "no product route yet: Build admission folded a constant under its provider plans, \
             and settling it on a Product frontier typed after Build ran requires the constant \
             evaluator",
        ));
    }
    Err(refusals)
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
        compilation_products::BuildConstants,
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
    // Nothing is folded here (point 3 above), and the names come from the
    // accepted module, so a later frontier can tell there is nothing to
    // settle without assuming how this admission was reached.
    let constants = compilation_products::BuildConstants {
        folded_under_providers: false,
        read: accepted.constants_read(),
    };
    Ok((
        Arc::new(accepted),
        super::provider_execution::BuildProviderPlans::default(),
        constants,
    ))
}

#[cfg(test)]
mod tests {
    use super::settle_rejoined_build_slice;
    use compilation_products::BuildConstants;

    fn settle(constants: &BuildConstants) -> Result<(), Vec<String>> {
        let mut typed = typed_trees::TypedTrees::default();
        let mut pre_checks = Vec::new();
        settle_rejoined_build_slice(
            constants,
            psi_compiler::BuildSourceInput {
                typed: &mut typed,
                pre_checks: &mut pre_checks,
            },
            symbols::SymbolHandle::invalid(),
            None,
            false,
            None,
        )
        .map_err(|refusals| {
            refusals
                .into_iter()
                .map(|refusal| refusal.message)
                .collect()
        })
    }

    /// An admission that folded nothing, for a module that names no
    /// constant, leaves a later Product frontier nothing to settle.
    #[test]
    fn a_build_that_evaluated_no_constant_has_nothing_to_settle() {
        assert_eq!(settle(&BuildConstants::default()), Ok(()));
    }

    /// The twin: the accepted module names constants. Settling them needs
    /// the evaluator, so each is refused by name.
    #[test]
    fn a_build_that_reads_constants_is_refused_naming_each() {
        let refusals = settle(&BuildConstants {
            folded_under_providers: false,
            read: vec!["LIMIT".into(), "linux_x86_64".into()],
        })
        .unwrap_err();
        assert_eq!(refusals.len(), 2);
        assert!(
            refusals[0].contains("reads constant `LIMIT`"),
            "{refusals:?}"
        );
        assert!(refusals[1].contains("reads constant `linux_x86_64`"));
        assert!(refusals.iter().all(|refusal| {
            refusal.starts_with("no product route yet:")
                && refusal.ends_with("requires the constant evaluator")
        }));
    }

    /// A fold under provider plans is owed even when no name was recorded.
    #[test]
    fn a_provider_fold_is_refused_without_any_recorded_read() {
        let refusals = settle(&BuildConstants {
            folded_under_providers: true,
            read: Vec::new(),
        })
        .unwrap_err();
        assert_eq!(refusals.len(), 1);
        assert!(refusals[0].contains("folded a constant under its provider plans"));
    }
}

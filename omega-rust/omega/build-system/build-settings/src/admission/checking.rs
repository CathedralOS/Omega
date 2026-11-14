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
   Terminal installation before execution. The provider preparation retains
   the package authority and admits the actual invocation closure against it;
   host effects remain a separate admission, not a pure-evaluation exemption.
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
accepted names, each with its declaration and the value the judged source
recorded for it.

A constant the admission read at a recorded value owes no evaluation: no
stage ran anything to produce it, so there is nothing to repeat. It is
settled when the rejoined frontier holds that same declaration at that same
recorded value, which is checked here on the frontier itself and never
assumed from how either frontier was typed. Then the Product has no pending
work for it and reads what Build read.

Everything else needs the evaluator that owns pending constant work
(`build_time_evaluation::PreCheckEvaluation`), which serves no job yet, and
is refused naming the constant: a fold under provider plans, a constant the
rejoined frontier no longer declares or has not evaluated, and one whose
value there differs. That evaluator replaces the refusal: it receives the
rejoined frontier, the Build entry there and the package inputs the Build
was admitted under.
*/
pub fn settle_rejoined_build_slice(
    constants: &compilation_products::BuildConstants,
    rejoined: psi_compiler::BuildSourceInput<'_>,
    _entry: symbols::SymbolHandle,
    _packages: Option<&PackageCompilationInputs>,
    _dependency_activation: bool,
    _execution_profile: Option<target::TargetProfile>,
) -> Result<(), Vec<Diagnostic>> {
    let typed = &*rejoined.typed;
    let mut refusals = constants
        .read
        .iter()
        .filter(|read| !holds_recorded_value(typed, read))
        .map(|read| {
            Diagnostic::error(format!(
                "no product route yet: the Build program reads constant `{}`, and settling \
                 it on a Product frontier typed after Build ran requires the constant evaluator",
                read.name
            ))
        })
        .collect::<Vec<_>>();
    if constants.folded_under_providers && refusals.is_empty() {
        refusals.push(Diagnostic::error(
            "no product route yet: Build admission folded a constant under its provider plans, \
             and settling it on a Product frontier typed after Build ran requires the constant \
             evaluator",
        ));
    }
    if refusals.is_empty() {
        Ok(())
    } else {
        Err(refusals)
    }
}

/// Whether `typed` declares the constant Build read, at the value Build
/// read. A constant no stage had evaluated when Build read it has no value
/// to hold the frontier to.
fn holds_recorded_value(
    typed: &typed_trees::TypedTrees,
    read: &psi_compiler::ConstantRead,
) -> bool {
    let (Some(declaration), Some(value)) = (&read.declaration, &read.value) else {
        return false;
    };
    declaration
        .rejoin(
            &typed.symbols,
            typed
                .const_declarations()
                .iter()
                .map(|constant| constant.symbol),
        )
        .ok()
        .and_then(|constant| typed.const_declaration_by_symbol(constant))
        .is_some_and(|constant| constant.evaluated_value.as_ref() == Some(value))
}

pub(crate) fn prepare_admitted_build(
    typed: &typed_trees::TypedTrees,
    entry: symbols::SymbolHandle,
    packages: Option<&PackageCompilationInputs>,
    anonymous_source: Option<&package_compilation::AnonymousSourceCapture>,
    dependency_activation: bool,
    timings: &mut artifacts::compile_timings::CompileTimings,
) -> Result<
    (
        Arc<psi_compiler::AcceptedTerminalSource>,
        super::provider_execution::BuildProviderPlans,
        compilation_products::BuildConstants,
    ),
    compilation_products::CompileFailure,
> {
    let mut accounting = build_time_evaluation::SourceCompletionAccounting::default();
    let result = (|| -> Result<_, Vec<Diagnostic>> {
        if packages.is_some() && anonymous_source.is_some() {
            return Err(vec![Diagnostic::error(
                "Build scope cannot combine anonymous source custody with supplied package inputs",
            )]);
        }
        // A dependency's Build executed for its consumer checks what its entry
        // reaches and never promotes the dependency to a subject
        // (`wiki/spec/build/validation_subjects.md`, "Orchestration").
        let subject =
            (!dependency_activation).then(|| packages.map(PackageCompilationInputs::root));
        let accepted = timings.record_result(BUILD_SOURCE_JUDGMENT, || {
            let compilation = psi_compiler::TerminalSourceCompilation {
                declaration_metadata: &[],
                entries: &[entry],
                subjects: subject.as_slice(),
                purpose: source::DependencyScope::Build,
                selected: &[],
                bindings: &[],
                supplies: &[],
            };
            let source = psi_compiler::SourcePreparationRequest {
                program: typed.clone(),
                accounting: Default::default(),
                admission: psi_compiler::SourceAdmission {
                    invocation_sources: anonymous_source.map(|capture| {
                        Arc::new(capture.clone())
                            as Arc<dyn build_time_evaluation::InvocationSourceConsent>
                    }),
                    ..Default::default()
                },
                compilation,
            }
            .prepare();
            let source = match source {
                Ok(source) => {
                    accounting = source.accounting;
                    source.program
                }
                Err(failure) => {
                    accounting = failure.accounting;
                    return Err(failure.diagnostics);
                }
            };
            // This admission has no settlement owner for newly evaluated source
            // constants yet. Preserve the explicit boundary rather than dropping
            // invocation charges when a Build requests such work.
            if !accounting.receipts().is_empty() {
                return Err(vec![Diagnostic::error(
                    "no product route yet: Build constant invocation accounting settlement",
                )]);
            }
            psi_compiler::AcceptedTerminalSource::judge(
                source,
                compilation,
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
            super::provider_execution::BuildProviderPlans::for_packages(packages),
            constants,
        ))
    })();
    result.map_err(|diagnostics| {
        compilation_products::CompileFailure::source_completion(diagnostics, accounting)
    })
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

    fn read(name: &str, value: Option<&str>) -> psi_compiler::ConstantRead {
        psi_compiler::ConstantRead {
            name: name.to_owned(),
            declaration: None,
            value: value.map(|value| value.to_owned().into()),
        }
    }

    /// The twin: the accepted module names constants the later frontier
    /// does not declare, one of them never evaluated. Settling them needs
    /// the evaluator, so each is refused by name.
    #[test]
    fn a_build_that_reads_constants_the_frontier_lacks_is_refused_naming_each() {
        let refusals = settle(&BuildConstants {
            folded_under_providers: false,
            read: vec![
                read("LIMIT", None),
                read("linux_x86_64", Some("integer8:1")),
            ],
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

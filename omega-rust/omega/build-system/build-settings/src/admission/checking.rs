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

/// Check that Build's constant reads remain valid after Product sources join.
/// Resolution already records literal and case values; reading one does not
/// imply that an evaluator still owes work. Reuse normal Build admission on
/// the rejoined entry, with the same package authority and validation subjects.
/// This checks the source without executing Build again or consuming Product's
/// pending evaluations. The caller authenticates the entry's original source.
///
/// A constant folded under Build's providers needs that actual evaluation
/// context and its receipt. Rechecking source cannot replace it, so those
/// requests still refuse until their evaluator exists.
pub fn settle_rejoined_build_slice(
    constants: &compilation_products::BuildConstants,
    rejoined: psi_compiler::BuildSourceInput<'_>,
    entry: symbols::SymbolHandle,
    packages: Option<&PackageCompilationInputs>,
    dependency_activation: bool,
    _execution_profile: Option<target::TargetProfile>,
) -> Result<(), Vec<Diagnostic>> {
    if !constants.folded_under_providers && constants.read.is_empty() {
        return Ok(());
    }
    if !constants.folded_under_providers {
        prepare_admitted_build(
            rejoined.typed,
            entry,
            packages,
            dependency_activation,
            &mut Default::default(),
        )?;
        return Ok(());
    }
    let mut refusals = constants
        .read
        .iter()
        .map(|name| {
            Diagnostic::error(format!(
                "no product route yet: Build admission folded constant `{name}` under its \
                 provider plans, and settling it after Product sources join requires the \
                 constant evaluator"
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
                supplies: &[],
            },
            terminal_codec::TerminalPublication::Invocation,
        )
    })?;
    if packages.is_some() {
        return Err(vec![Diagnostic::error(
            "no product route yet: the accepted Build program has no declaration-selection admission for its package authority",
        )]);
    }
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

    fn source(build: &str, product: &str) -> (typed_trees::TypedTrees, symbols::SymbolHandle) {
        let mut sources = source::SourceMap::default();
        let mut syntax = None;
        for (path, text, scope) in [
            ("build.omg", build, source::DependencyScope::Build),
            ("main.omg", product, source::DependencyScope::Product),
        ] {
            let id = sources
                .add_checked_instance(
                    path.into(),
                    text,
                    ".".into(),
                    None,
                    source::SourceOrigin::User,
                    source::SourceResolutionStratum::Base,
                    scope,
                )
                .source_id;
            let tokens = source_files_to_tokens::Lexer::new(text).tokenize().unwrap();
            match &mut syntax {
                None => {
                    syntax = Some(
                        tokens_to_syntax_trees::parse_syntax_trees_with_id(id, &tokens).unwrap(),
                    );
                }
                Some(syntax) => {
                    tokens_to_syntax_trees::parse(syntax, id, &tokens).unwrap();
                }
            }
        }
        let mut request =
            syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(syntax.unwrap());
        request.sources = Some(std::sync::Arc::new(sources));
        let resolved = syntax_trees_to_symbol_resolved_trees::resolve(request).unwrap();
        let typed =
            symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
        let entry = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "build")
            .unwrap()
            .symbol;
        (typed, entry)
    }

    const BUILD: &str = r#"
const LIMIT: u64 = 4;
machine consume(value: u64) requires value < 5u64 {}
machine build() { consume(LIMIT); }
"#;

    #[test]
    fn already_evaluated_build_constants_settle_without_checking_product_bodies() {
        let (original, entry) = source(BUILD, "");
        let (_, _, constants) =
            super::prepare_admitted_build(&original, entry, None, true, &mut Default::default())
                .unwrap();
        assert_eq!(constants.read, ["LIMIT"]);
        let (mut rejoined, entry) = source(
            BUILD,
            "machine unrelated_product() -> u64 ensures result == 99u64 { 0u64 }",
        );
        let (_, pending) = build_time_evaluation::evaluate_pre_resolution(
            build_time_evaluation::BuildTimeEvaluationRequest {
                syntax_trees: syntax_trees::SyntaxTrees::new(source::SourceId(0)),
                source_context: Default::default(),
            },
        )
        .unwrap()
        .into_syntax_and_pre_check();
        let mut pre_checks = vec![pending.clone()];
        settle_rejoined_build_slice(
            &constants,
            psi_compiler::BuildSourceInput {
                typed: &mut rejoined,
                pre_checks: &mut pre_checks,
            },
            entry,
            None,
            true,
            None,
        )
        .unwrap();
        assert_eq!(pre_checks.len(), 1);
        assert!(pre_checks[0].same_pending_work(&pending));
    }

    fn settle_source(
        mut typed: typed_trees::TypedTrees,
        entry: symbols::SymbolHandle,
    ) -> Result<(), Vec<String>> {
        settle_rejoined_build_slice(
            &BuildConstants {
                folded_under_providers: false,
                read: vec!["LIMIT".into()],
            },
            psi_compiler::BuildSourceInput {
                typed: &mut typed,
                pre_checks: &mut Vec::new(),
            },
            entry,
            None,
            true,
            None,
        )
        .map_err(|refusals| {
            refusals
                .into_iter()
                .map(|refusal| refusal.message)
                .collect()
        })
    }

    #[test]
    fn settlement_checks_the_contract_of_the_rejoined_build() {
        let (typed, entry) = source(&BUILD.replace("= 4;", "= 5;"), "");
        let refusals = settle_source(typed, entry).unwrap_err();
        assert!(
            refusals
                .iter()
                .any(|message| message.contains("cannot prove requires contract")
                    && message.contains("5 < 5")),
            "{refusals:?}"
        );
    }

    #[test]
    fn settlement_preserves_pending_build_evaluation_refusals() {
        let (mut typed, entry) = source(BUILD, "");
        // A source pass records an unavailable evaluation on its declaration.
        // Rechecking a constant read cannot clear that request.
        typed.record_deferred_refusal(
            entry,
            typed_trees::deferred_refusals::DeferredRequest::MaterializedExpression,
            typed.symbols.symbol_source_span(entry).unwrap(),
        );
        let refusals = settle_source(typed, entry).unwrap_err();
        assert!(
            refusals
                .iter()
                .any(|message| message.contains("materialized expression")),
            "{refusals:?}"
        );
    }

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

    /// Provider-dependent folds still require their actual evaluator.
    #[test]
    fn provider_folds_are_refused_naming_each_constant() {
        let refusals = settle(&BuildConstants {
            folded_under_providers: true,
            read: vec!["LIMIT".into(), "linux_x86_64".into()],
        })
        .unwrap_err();
        assert_eq!(refusals.len(), 2);
        assert!(
            refusals[0].contains("folded constant `LIMIT`"),
            "{refusals:?}"
        );
        assert!(refusals[1].contains("folded constant `linux_x86_64`"));
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

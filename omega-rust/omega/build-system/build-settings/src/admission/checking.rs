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
3. Constants and provider plans. Source preparation evaluates demanded Build
   constants through their accepted invocations. Their charges stay with the
   prepared and evaluated Build, separately from Product materialization proof.
   A retyped Build evaluates its own demanded constants under its own request.
   The original Product pending work stays whole. No provider plan is selected here; `BuildProviderPlans::admit` holds
   the published module's actual demand against that selection before it runs.
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
Build and Product have distinct typed inputs. A rejoined Build slice therefore
prepares its own Build-purpose request and evaluates its own demanded constants.
The old declaration/value ledger checks correspondence; it cannot authorize an
old result or receipt on the new source. Both invocation occurrences remain
chargeable, while a cloned history of either occurrence is counted only once.
Provider-dependent folding still requires a provider-aware producer and is
refused before an unsupported rejoin can be treated as settled.
*/
pub fn settle_rejoined_build_slice(
    constants: &compilation_products::BuildConstants,
    rejoined: psi_compiler::BuildSourceInput<'_>,
    entry: symbols::SymbolHandle,
    packages: Option<&PackageCompilationInputs>,
    anonymous_source: Option<&package_compilation::AnonymousSourceCapture>,
    dependency_activation: bool,
    timings: &mut artifacts::compile_timings::CompileTimings,
) -> Result<build_time_evaluation::SourceCompletionAccounting, compilation_products::CompileFailure>
{
    if constants.folded_under_providers {
        return Err(vec![Diagnostic::error(
            "no product route yet: Build admission folded a constant under its provider plans, and settling it on a Product frontier typed after Build ran requires the constant evaluator",
        )].into());
    }
    if constants.read.is_empty() {
        return Ok(Default::default());
    }
    let refusals = constants.read.iter().filter(|read| read.declaration.is_none() || read.value.is_none()).map(|read| {
        Diagnostic::error(format!(
            "no product route yet: the Build program reads constant `{}`, and settling it on a Product frontier typed after Build ran requires the constant evaluator", read.name
        ))
    }).collect::<Vec<_>>();
    if !refusals.is_empty() {
        return Err(refusals.into());
    }
    // A retyped Build is a new request in its actual Build context. Nothing
    // copies its old result into Product or uses an old receipt for this input.
    let (accepted, _, current, accounting) = prepare_admitted_build(
        rejoined.typed,
        entry,
        packages,
        anonymous_source,
        dependency_activation,
        timings,
    )?;
    let result = compare_rejoined_constants(constants, &current, accepted.source());
    result.map(|()| accounting.clone()).map_err(|diagnostics| {
        compilation_products::CompileFailure::source_completion(diagnostics, accounting)
    })
}

fn compare_rejoined_constants(
    original: &compilation_products::BuildConstants,
    current: &compilation_products::BuildConstants,
    typed: &typed_trees::TypedTrees,
) -> Result<(), Vec<Diagnostic>> {
    // Index only this constant family. The source anchor still authenticates
    // bytes, owner, purpose and declaration kind inside the matching range.
    let mut family = typed
        .const_declarations()
        .iter()
        .filter_map(|constant| {
            let span = typed.symbols.symbol_source_span(constant.symbol)?;
            let source = typed.symbols.source_file(span)?;
            Some((
                source.path.as_path(),
                span.span.start,
                span.span.end,
                constant.symbol,
            ))
        })
        .collect::<Vec<_>>();
    family
        .sort_unstable_by_key(|row| (row.0, row.1, row.2, row.3.arena_index(), row.3.generation()));
    let resolve = |reads: &[psi_compiler::ConstantRead]| {
        let mut values = Vec::with_capacity(reads.len());
        for read in reads {
            let declaration = read.declaration.as_ref().ok_or_else(|| {
                vec![Diagnostic::error(format!(
                    "Build constant `{}` has no rejoinable declaration",
                    read.name
                ))]
            })?;
            let span = declaration.span();
            let key = (declaration.source().path.as_path(), span.start, span.end);
            let start = family.partition_point(|row| (row.0, row.1, row.2) < key);
            let end = family.partition_point(|row| (row.0, row.1, row.2) <= key);
            let symbol = declaration
                .rejoin(&typed.symbols, family[start..end].iter().map(|row| row.3))
                .map_err(|reason| {
                    vec![Diagnostic::error(format!(
                        "Build constant `{}` rejoin: {reason}",
                        read.name
                    ))]
                })?;
            let value = read.value.clone().ok_or_else(|| {
                vec![Diagnostic::error(format!(
                    "Build constant `{}` has no evaluated value in its own context",
                    read.name
                ))]
            })?;
            values.push(((symbol.arena_index(), symbol.generation()), value));
        }
        values.sort_unstable_by_key(|row| row.0);
        Ok::<_, Vec<Diagnostic>>(values)
    };
    if resolve(&original.read)? != resolve(&current.read)? {
        return Err(vec![Diagnostic::error(
            "retyped Build constant demand or value differs from the executed Build context",
        )]);
    }
    Ok(())
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
        build_time_evaluation::SourceCompletionAccounting,
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
        let (accepted, read) = timings.record_result(BUILD_SOURCE_JUDGMENT, || {
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
            let (source, constants) = match source {
                Ok(source) => {
                    accounting = source.accounting;
                    (source.program, source.constants)
                }
                Err(failure) => {
                    accounting = failure.accounting;
                    return Err(failure.diagnostics);
                }
            };
            psi_compiler::AcceptedTerminalSource::judge(
                source,
                compilation,
                terminal_codec::TerminalPublication::Invocation,
            )
            .map(|accepted| (accepted, constants))
        })?;
        // Source preparation retains the demanded family before literal
        // substitution. Its genuine receipts travel as Build costs, while
        // the declaration/value ledger describes a later independent rejoin.
        let constants = compilation_products::BuildConstants {
            folded_under_providers: false,
            read,
        };
        Ok((
            Arc::new(accepted),
            super::provider_execution::BuildProviderPlans::for_packages(packages),
            constants,
        ))
    })();
    match result {
        Ok((accepted, providers, constants)) => Ok((accepted, providers, constants, accounting)),
        Err(diagnostics) => Err(compilation_products::CompileFailure::source_completion(
            diagnostics,
            accounting,
        )),
    }
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
            None,
            false,
            &mut artifacts::compile_timings::CompileTimings::default(),
        )
        .map(|_| ())
        .map_err(|refusals| {
            refusals
                .into_diagnostics()
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

    fn source(text: &str) -> typed_trees::TypedTrees {
        let mut sources = source::SourceMap::default();
        let id = sources
            .add_checked_instance(
                "/unit/build.omg".into(),
                text,
                "/unit".into(),
                None,
                source::SourceOrigin::Toolchain,
                source::SourceResolutionStratum::Base,
                source::DependencyScope::Build,
            )
            .source_id;
        let sources = std::sync::Arc::new(sources);
        let tokens = source_files_to_tokens::Lexer::new(text).tokenize().unwrap();
        let syntax = tokens_to_syntax_trees::parse_syntax_trees_with_id(id, &tokens).unwrap();
        let mut request = syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(syntax);
        request.sources = Some(sources);
        let resolved = syntax_trees_to_symbol_resolved_trees::resolve(request).unwrap();
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap()
    }

    fn prepare(
        typed: typed_trees::TypedTrees,
    ) -> Result<psi_compiler::FrozenSource, psi_compiler::SourcePreparationFailure> {
        let entry = typed.realized_machine_named("build").unwrap().symbol;
        psi_compiler::SourcePreparationRequest {
            program: typed,
            accounting: Default::default(),
            admission: Default::default(),
            compilation: psi_compiler::TerminalSourceCompilation {
                declaration_metadata: &[],
                entries: &[entry],
                subjects: &[],
                purpose: source::DependencyScope::Build,
                selected: &[],
                bindings: &[],
                supplies: &[],
            },
        }
        .prepare()
    }

    #[test]
    fn independent_build_contexts_keep_actual_constant_demands_and_charge_each_run() {
        for (initializer, executions) in [("answer()", 1), ("4u64", 0)] {
            let text = format!(
                "const VALUE:u64={initializer}; const OTHER:u64=9u64; machine answer()->u64 {{4u64}} machine build()->u64 {{VALUE}}"
            );
            let first = prepare(source(&text)).unwrap();
            let next = prepare(source(&text)).unwrap();
            assert_ne!(first.program.identity.get(), next.program.identity.get());
            assert_eq!(
                first.constants.len(),
                1,
                "the actual demand survives literal substitution"
            );
            assert_eq!(first.constants[0].name, "VALUE");
            let old = BuildConstants {
                folded_under_providers: false,
                read: first.constants.clone(),
            };
            let current = BuildConstants {
                folded_under_providers: false,
                read: next.constants.clone(),
            };
            super::compare_rejoined_constants(&old, &current, &next.program).unwrap();
            let missing = BuildConstants::default();
            assert!(super::compare_rejoined_constants(&old, &missing, &next.program).is_err());
            let other = next
                .program
                .const_declarations()
                .iter()
                .find(|declaration| next.program.symbols.name(declaration.symbol) == "OTHER")
                .unwrap();
            let mut wrong = current.clone();
            wrong.read[0].value = other.evaluated_value.clone();
            assert!(super::compare_rejoined_constants(&old, &wrong, &next.program).is_err());
            assert_eq!(first.accounting.receipts().len(), executions);
            let mut joined = first.accounting.clone();
            joined.append(first.accounting.clone());
            assert_eq!(
                joined.receipts().len(),
                executions,
                "a held occurrence is charged once"
            );
            joined.append(next.accounting.clone());
            assert_eq!(
                joined.receipts().len(),
                executions * 2,
                "a fresh invocation is a second charge"
            );
            let mut retyped = source(&text);
            let entry = retyped.realized_machine_named("build").unwrap().symbol;
            let mut pre_checks = Vec::new();
            let fresh = super::settle_rejoined_build_slice(
                &old,
                psi_compiler::BuildSourceInput {
                    typed: &mut retyped,
                    pre_checks: &mut pre_checks,
                },
                entry,
                None,
                None,
                false,
                &mut artifacts::compile_timings::CompileTimings::default(),
            )
            .unwrap();
            assert_eq!(fresh.receipts().len(), executions);
            joined.append(fresh);
            assert_eq!(
                joined.receipts().len(),
                executions * 3,
                "settlement executes a third independent Build request"
            );
            joined.validate_charges().unwrap();
        }
    }

    #[test]
    fn a_retyped_value_cannot_replace_its_fresh_computation_receipt() {
        let mut typed = source(
            "const VALUE:u64=answer(); const OTHER:u64=9u64; machine answer()->u64 {4u64} machine build()->u64 {VALUE}",
        );
        let other = typed
            .const_declarations()
            .iter()
            .find(|declaration| typed.symbols.name(declaration.symbol) == "OTHER")
            .unwrap()
            .evaluated_value
            .clone();
        let value = typed
            .roots
            .const_declarations
            .handles()
            .find(|handle| {
                typed
                    .symbols
                    .name(typed.tables.const_declarations.get(*handle).symbol)
                    == "VALUE"
            })
            .unwrap();
        typed
            .tables
            .const_declarations
            .get_mut(value)
            .evaluated_value = other;
        let failure = match prepare(typed) {
            Err(failure) => failure,
            Ok(_) => panic!("a retyped value is not a receipt"),
        };
        assert!(diagnostics::join_messages(&failure.diagnostics).contains("receipt"));
        assert!(failure.accounting.receipts().is_empty());
    }

    #[test]
    fn later_refusal_and_pending_consent_retain_the_actual_earlier_work() {
        use compilation_products::CompileFailure;
        use package_compilation::build_execution::{
            RestrictedBuildBounds, RestrictedBuildOperation, RestrictedBuildRequest,
        };
        let prepared = prepare(source(
            "const VALUE:u64=answer(); machine answer()->u64 {4u64} machine build()->u64 {VALUE}",
        ))
        .unwrap();
        assert_eq!(prepared.accounting.receipts().len(), 1);
        let failure = CompileFailure::Diagnostics(vec![diagnostics::Diagnostic::error(
            "later admission refused",
        )])
        .with_source_completion_accounting(prepared.accounting.clone());
        assert_eq!(
            failure
                .source_completion_accounting()
                .unwrap()
                .receipts()
                .len(),
            1
        );
        let request = RestrictedBuildRequest::from_retained(
            RestrictedBuildOperation::ScopedFilesystemExecution,
            Vec::new(),
            Vec::new(),
            RestrictedBuildBounds::from_retained(None, None, Vec::new(), false),
            None,
            None,
        );
        let failure = CompileFailure::BuildConsentRequired {
            requests: vec![request.clone()],
            accounting: prepared.accounting.clone(),
        }
        .with_source_completion_accounting(prepared.accounting.clone());
        let CompileFailure::BuildConsentRequired {
            requests,
            accounting,
        } = failure
        else {
            panic!("costs cannot replace the pending consent kind")
        };
        assert_eq!(requests, vec![request]);
        assert_eq!(accounting.receipts().len(), 1);
        accounting.validate_charges().unwrap();
    }
}

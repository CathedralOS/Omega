//! Check an authored Build under its package scope, retain its costs, and compare
//! its actual constant reads when a returned Build rejoins the Product.

use artifacts::compile_timings::StageMeta;
use build_evaluation_result::{BuildConstants, BuildSubjectValidation, CompileFailure};
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
    constants: &BuildConstants,
    interpreter_providers: &source_compiler::InterpretedProviderSelections,
    rejoined: &source_compiler::representations::source_state::TypedTrees,
    entry: symbols::SymbolHandle,
    packages: Option<&PackageCompilationInputs>,
    anonymous_source: Option<&package_compilation::AnonymousSourceCapture>,
    subject_validation: BuildSubjectValidation,
    timings: &mut artifacts::compile_timings::CompileTimings,
) -> Result<source_compiler::compile_time_invocations::SourceCompletionAccounting, CompileFailure> {
    if constants.folded_under_providers {
        return Err(vec![Diagnostic::error(
            "no product route yet: Build admission folded a constant under its provider plans, and settling it on a Product frontier typed after Build ran requires the constant evaluator",
        )].into());
    }
    // A subject checked under a provisional owner still owes the check under
    // its real owner, even when it read no constants.
    if constants.read.is_empty() && subject_validation != BuildSubjectValidation::NeedsOwnerCheck {
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
        rejoined,
        entry,
        packages,
        anonymous_source,
        &Arc::default(),
        interpreter_providers,
        subject_validation != BuildSubjectValidation::EntryOnly,
        timings,
    )?;
    let result = compare_rejoined_constants(constants, &current, accepted.accepted().source());
    result
        .map(|()| accounting.clone())
        .map_err(|diagnostics| CompileFailure::source_completion(diagnostics, accounting))
}

fn compare_rejoined_constants(
    original: &BuildConstants,
    current: &BuildConstants,
    typed: &source_compiler::representations::source_state::TypedTrees,
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
    let resolve = |reads: &[source_compiler::publication::ConstantRead]| {
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
            let symbol = source_compiler::representations::source_symbols::rejoin_declaration(
                declaration,
                &typed.symbols,
                family[start..end].iter().map(|row| row.3),
            )
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
        // A staged Build names a constant once in every stage that reads
        // it, and the Build retyped whole names it once.
        values.dedup();
        Ok::<_, Vec<Diagnostic>>(values)
    };
    if resolve(&original.read)? != resolve(&current.read)? {
        return Err(vec![Diagnostic::error(
            "retyped Build constant demand or value differs from the executed Build context",
        )]);
    }
    Ok(())
}

/*
The compiler checks the whole Build file before Build runs
(`wiki/spec/build/command_line.md`). A root without a package graph has no
owner until Build returns, so `check_only` is the same Build stage typed with
the root's own files under the capture's provisional owner, and the boundaries
it declares can be named. Nothing this judgment produces runs or is published:
its invocation and its evaluation receipts are dropped here. Its verdict stands,
and the constants it read are anchored again on the unowned sources Build runs,
so the check under the real owner compares the same full set.
*/
pub(crate) fn check_subject_before_execution(
    check_only: &source_compiler::representations::source_state::TypedTrees,
    executed: &source_compiler::representations::source_state::TypedTrees,
    build_source: package_compilation::BuildSourceSelection,
    anonymous_source: &package_compilation::AnonymousSourceCapture,
    libraries: &[package_compilation::source_imports::SuppliedBuildLibrary],
    interpreter_providers: &source_compiler::InterpretedProviderSelections,
    timings: &mut artifacts::compile_timings::CompileTimings,
) -> Result<BuildConstants, CompileFailure> {
    let owner = anonymous_source
        .provisional_check_owner()
        .map_err(|reason| vec![Diagnostic::error(reason)])?;
    let entry =
        package_compilation::build_execution::select_build_machine(check_only, build_source)?
            .symbol;
    let judged = timings
        .record_result(BUILD_SOURCE_JUDGMENT, || {
            let scope =
                package_compilation::source_imports::SuppliedBuildLibrary::declaration_scope(
                    libraries,
                    Some(owner),
                )
                .map(Arc::new);
            source_compiler::CheckedBuildInvocation::judge(
                check_only,
                entry,
                &[Some(owner)],
                scope,
                None,
                interpreter_providers,
            )
        })
        .map_err(|failure| CompileFailure::from(failure.diagnostics))?;
    let unowned = executed.symbols.source_map().ok_or_else(|| {
        vec![Diagnostic::error(
            "the executed Build has no source map to anchor its constants",
        )]
    })?;
    let read = judged
        .constants
        .into_iter()
        .map(|read| unbind_provisional_owner(read, &unowned, owner))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(BuildConstants {
        folded_under_providers: false,
        read,
    })
}

/// Anchor a constant read by the check-only judgment on the same file Build
/// runs. The two files must differ only in the provisional owner.
fn unbind_provisional_owner(
    read: source_compiler::publication::ConstantRead,
    unowned: &source_compiler::representations::source_inputs::SourceMap,
    owner: semantic_vocabulary::PackageKeyIdentity,
) -> Result<source_compiler::publication::ConstantRead, Vec<Diagnostic>> {
    let Some(declaration) = &read.declaration else {
        return Ok(read);
    };
    let checked = declaration.source();
    let file = unowned
        .get(checked.source_id)
        .filter(|file| {
            let mut unbound = checked.clone();
            if unbound.package_identity == Some(owner) && file.package_identity.is_none() {
                unbound.package_identity = None;
            }
            **file == unbound
        })
        .ok_or_else(|| {
            vec![Diagnostic::error(format!(
                "Build constant `{}` was read from a source the executed Build does not hold",
                read.name
            ))]
        })?;
    let declaration = source_admission::RetainedDeclaration::from_source_occurrence(
        file.clone(),
        declaration.span(),
        declaration.kind(),
    );
    Ok(source_compiler::publication::ConstantRead {
        declaration: Some(declaration),
        ..read
    })
}

pub(crate) fn prepare_admitted_build(
    typed: &source_compiler::representations::source_state::TypedTrees,
    entry: symbols::SymbolHandle,
    packages: Option<&PackageCompilationInputs>,
    anonymous_source: Option<&package_compilation::AnonymousSourceCapture>,
    libraries: &Arc<[package_compilation::source_imports::SuppliedBuildLibrary]>,
    interpreter_providers: &source_compiler::InterpretedProviderSelections,
    check_subject: bool,
    timings: &mut artifacts::compile_timings::CompileTimings,
) -> Result<
    (
        source_compiler::CheckedBuildInvocation,
        super::provider_execution::BuildProviderScope,
        BuildConstants,
        source_compiler::compile_time_invocations::SourceCompletionAccounting,
    ),
    CompileFailure,
> {
    if packages.is_some() && anonymous_source.is_some() {
        return Err(CompileFailure::source_completion(
            vec![Diagnostic::error(
                "Build scope cannot combine anonymous source custody with supplied package inputs",
            )],
            Default::default(),
        ));
    }
    let subject = check_subject.then(|| packages.map(PackageCompilationInputs::root));
    let providers =
        super::provider_execution::BuildProviderScope::for_packages(packages, libraries);
    let judged = timings
        .record_result(BUILD_SOURCE_JUDGMENT, || {
            source_compiler::CheckedBuildInvocation::judge(
                typed,
                entry,
                subject.as_slice(),
                providers
                    .selection_scope(build_declarations::DependencyPurpose::Build)
                    .map(Arc::new),
                anonymous_source.map(|capture| capture.invocation_consent(Arc::clone(libraries))),
                interpreter_providers,
            )
        })
        .map_err(|failure| {
            CompileFailure::source_completion(failure.diagnostics, failure.accounting)
        })?;
    Ok((
        judged.invocation,
        providers,
        BuildConstants {
            folded_under_providers: false,
            read: judged.constants,
        },
        judged.accounting,
    ))
}

#[cfg(test)]
mod tests {
    use super::settle_rejoined_build_slice;
    use build_evaluation_result::{BuildConstants, BuildSubjectValidation};

    fn settle(constants: &BuildConstants) -> Result<(), Vec<String>> {
        let typed = source_compiler::representations::source_state::TypedTrees::default();
        settle_rejoined_build_slice(
            constants,
            &Default::default(),
            &typed,
            symbols::SymbolHandle::invalid(),
            None,
            None,
            BuildSubjectValidation::Checked,
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

    fn read(name: &str, value: Option<&str>) -> source_compiler::publication::ConstantRead {
        source_compiler::publication::ConstantRead {
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

    fn source(text: &str) -> source_compiler::representations::source_state::TypedTrees {
        let mut sources = source_compiler::representations::source_inputs::SourceMap::default();
        let id = sources
            .add_checked_instance(
                "/unit/build.omg".into(),
                text,
                "/unit".into(),
                None,
                source_admission::SourceOrigin::Toolchain,
                source_admission::SourceResolutionStratum::Base,
                source_provenance::DependencyScope::Build,
            )
            .source_id;
        let sources = std::sync::Arc::new(sources);
        let tokens = crate::frontend_fixture::Lexer::new(text)
            .tokenize()
            .unwrap();
        let syntax = crate::frontend_fixture::parse_syntax_trees_with_id(id, &tokens).unwrap();
        let mut request = source_compiler::test_support::resolution::ResolutionRequest::new(syntax);
        request.sources = Some(sources);
        let resolved = source_compiler::test_support::resolution::resolve(request).unwrap();
        source_compiler::test_support::typing::lower_symbol_resolved_trees(&resolved).unwrap()
    }

    fn prepare(
        typed: source_compiler::representations::source_state::TypedTrees,
    ) -> Result<
        source_compiler::test_support::FrozenSource,
        source_compiler::SourcePreparationFailure,
    > {
        let entry = typed.realized_machine_named("build").unwrap().symbol;
        source_compiler::test_support::prepare_source_for_test(
            source_compiler::test_support::SourcePreparationRequest {
                program: typed,
                accounting: Default::default(),
                admission: Default::default(),
                compilation: source_compiler::test_support::TerminalSourceCompilation {
                    declaration_metadata: &[],
                    entries: &[entry],
                    subjects: &[],
                    purpose: source_provenance::DependencyScope::Build,
                    selected: &[],
                    bindings: &[],
                    calling: None,
                    providers: None,
                    supplies: &[],
                },
            },
        )
    }

    #[test]
    fn independent_build_contexts_keep_actual_constant_demands_and_charge_each_run() {
        for (initializer, executions) in [("answer()", 1), ("4u64", 0)] {
            let text = format!(
                "const VALUE:u64={initializer}; const OTHER:u64=9u64; machine answer()->u64 {{4u64}} machine build()->u64 {{VALUE}}"
            );
            let first = prepare(source(&text)).unwrap();
            let next = prepare(source(&text)).unwrap();
            let first_constants =
                source_compiler::publication::ConstantRead::from_preparation(&first.program);
            let next_constants =
                source_compiler::publication::ConstantRead::from_preparation(&next.program);
            assert_ne!(
                first.program.trees().identity.get(),
                next.program.trees().identity.get()
            );
            assert_eq!(
                first_constants.len(),
                1,
                "the actual demand survives literal substitution"
            );
            assert_eq!(first_constants[0].name, "VALUE");
            let old = BuildConstants {
                folded_under_providers: false,
                read: first_constants.clone(),
            };
            let current = BuildConstants {
                folded_under_providers: false,
                read: next_constants.clone(),
            };
            super::compare_rejoined_constants(&old, &current, next.program.trees()).unwrap();
            let missing = BuildConstants::default();
            assert!(
                super::compare_rejoined_constants(&old, &missing, next.program.trees()).is_err()
            );
            let other = next
                .program
                .trees()
                .const_declarations()
                .iter()
                .find(|declaration| {
                    next.program.trees().symbols.name(declaration.symbol) == "OTHER"
                })
                .unwrap();
            let mut wrong = current.clone();
            wrong.read[0].value = other.evaluated_value.clone();
            assert!(super::compare_rejoined_constants(&old, &wrong, next.program.trees()).is_err());
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
            let retyped = source(&text);
            let entry = retyped.realized_machine_named("build").unwrap().symbol;
            let fresh = super::settle_rejoined_build_slice(
                &old,
                &Default::default(),
                &retyped,
                entry,
                None,
                None,
                BuildSubjectValidation::Checked,
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
        use build_evaluation_result::CompileFailure;
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

//! Build and Product source ownership through preparation, query and publication.

#[path = "../../../../../../../tests/support/fixture_repository.rs"]
mod fixture_repository;

use super::ImmutableSourceParseCheckpoint;
use artifacts::compile_timings::CompileTimings;
use source_admission::{SourceOrigin, SourceResolutionStratum};
use std::path::Path;
use std::sync::Arc;

#[test]
fn collection_catalog_keeps_its_build_product_and_query_source_cohorts() {
    let root = fixture_repository::root()
        .join("tests/fixtures/calls/run/scalar_contract_route_exit/main.omg")
        .canonicalize()
        .expect("the existing scalar fixture's exact source path");
    let core = crate::source::source_imports::bundled_core_root().unwrap();
    let indexing = core.join("indexing/indexing.omg");
    let collection_source = |parsed: &source_compiler::ParsedStageSources, scope| {
        let files = parsed
            .files
            .selected_files()
            .filter(|file| file.path == indexing && file.dependency_scope == scope)
            .collect::<Vec<_>>();
        let [file] = files.as_slice() else {
            panic!("one captured collection catalog in {scope:?}, found {files:?}")
        };
        assert_eq!(file.package_root, core);
        assert_eq!(file.origin, SourceOrigin::Toolchain);
        assert_eq!(file.resolution_stratum, SourceResolutionStratum::Base);
        assert_eq!(
            file.source.as_ref(),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../../../../source/library/core/indexing/indexing.omg"
            ))
        );
        file.source_id
    };
    let mut catalogue = source_compiler::representations::source_inputs::SourceCatalogue::default();
    let mut timings = CompileTimings::default();
    // This case opens an authored Build with no retained entry bindings.
    // Selecting main.omg here would request the implicit executable instead.
    let checkpoint = ImmutableSourceParseCheckpoint::prepare_from_sources(
        &root.with_file_name("build.omg"),
        None,
        None,
        &mut timings,
        &mut catalogue,
    )
    .expect("the existing program's source capture");
    let (_, mut build) = checkpoint
        .assemble(None, &Default::default(), &mut timings, &mut catalogue)
        .expect("the initial Build source assembly");
    let build_source = collection_source(&build.parsed, source_provenance::DependencyScope::Build);
    assert!(!build.parsed.files.selected_files().any(|file| {
        file.path == indexing
            && file.dependency_scope == source_provenance::DependencyScope::Product
    }));
    let continuation = build
        .product_sources
        .take()
        .expect("Product stays deferred");
    let query = continuation
        .assemble_query(
            &crate::source::source_imports::ProductQueryInputs::new(None, vec![]).unwrap(),
            &mut timings,
            &mut catalogue,
        )
        .expect("a paused Build's query uses its captured Product sources");
    let query_source =
        collection_source(query.parsed(), source_provenance::DependencyScope::Product);
    assert_ne!(build_source, query_source);
    let (_, product) = continuation
        .assemble(
            None,
            Default::default(),
            &[],
            &[],
            &Default::default(),
            &mut timings,
            &mut catalogue,
        )
        .expect("the final Product source assembly");
    for sources in [query.parsed(), &product.parsed] {
        assert!(
            sources
                .files
                .selected_files()
                .all(|file| file.origin != SourceOrigin::ToolchainContract),
            "a declaration query or final Product without entry bindings should not acquire a physical entry package"
        );
    }
    let product_source =
        collection_source(&product.parsed, source_provenance::DependencyScope::Product);
    assert_ne!(build_source, product_source);
    // An exact immutable dependency may retain its Product coordinate;
    // it is the authored consumer occurrence that each assembly renews.
    assert_eq!(query_source, product_source);
    let consumer = |parsed: &source_compiler::ParsedStageSources| {
        let sources = parsed
            .files
            .selected_files()
            .filter(|file| {
                file.path == root
                    && file.origin == SourceOrigin::User
                    && file.dependency_scope == source_provenance::DependencyScope::Product
            })
            .map(|file| file.source_id)
            .collect::<Vec<_>>();
        assert_eq!(sources.len(), 1, "one actual authored Product root");
        assert!(parsed.roots.contains(&sources[0]));
        sources[0]
    };
    assert_ne!(consumer(query.parsed()), consumer(&product.parsed));
}

#[test]
fn original_build_bindings_retain_their_real_toolchain_requirement_calls() {
    // Read the existing program and the production Build prelude through
    // the normal source owner. Preparing their calls supplies no receiver,
    // provider, invocation verdict or host activation ticket.
    let root = fixture_repository::root()
        .join("tests/fixtures/generics/run/implementation_parameter_contract/build.omg");
    let mut catalogue = source_compiler::representations::source_inputs::SourceCatalogue::default();
    let mut timings = CompileTimings::default();
    let checkpoint = ImmutableSourceParseCheckpoint::prepare_from_sources(
        &root,
        None,
        None,
        &mut timings,
        &mut catalogue,
    )
    .expect("the original program's source capture");
    let (_, assembled) = checkpoint
        .assemble(None, &Default::default(), &mut timings, &mut catalogue)
        .expect("the ordinary Build source and real toolchain prelude");
    let sources = Arc::clone(&assembled.parsed.files);
    let interval = assembled
        .build_stage_source()
        .unwrap()
        .prepare(
            &Default::default(),
            "linux_x86_64",
            None,
            source_compiler::BuildSourceCheck { providers: &Default::default(), check_entry_file: true },
        )
        .expect("exact declared requirement calls reach typed preparation");
    let typed = interval.program.as_ref();
    let unresolved: Vec<_> = typed
        .deferred_refusals
        .iter()
        .filter(|row| {
            row.request == source_compiler::representations::source_state::deferred_refusals::DeferredRequest::UnresolvedCallTarget
                && sources.file_at(row.source_span).is_some_and(|file| {
                    file.path == root || (file.origin == SourceOrigin::Toolchain
                        && source_admission::source_identity::is_build_prelude_path(&file.path))
                })
        })
        .map(|row| {
            (
                typed.symbols.display_path(row.declaration, "::"),
                sources.file_at(row.source_span).map(|file| &file.path),
                row.source_span,
                &row.detail,
            )
        })
        .collect();
    assert!(
        unresolved.is_empty(),
        "Build calls retain their declarations: {unresolved:?}"
    );
    let mut occurrences = Vec::new();
    for (_, expression) in typed.expression_table.expression_entries() {
        let source_compiler::representations::source_state::expression::ExpressionNode::Call(call) =
            expression
        else {
            continue;
        };
        if !call.build_selection.is_valid() {
            continue;
        }
        let request = typed
            .expression_table
            .build_selection_request(call.build_selection)
            .expect("a captured Build call keeps its real selection request");
        let source_compiler::representations::declaration_selection::BuildSelectionRequest::Entry(
            binding,
        ) = request
        else {
            continue;
        };
        let (owner, signature) = typed
            .trait_machine_signature(call.target_symbol)
            .expect("the protocol call retains its exact requirement declaration");
        let owner_source = typed
            .symbols
            .symbol_source_span(owner.symbol)
            .and_then(|span| sources.file_at(span))
            .expect("the requirement retains its real toolchain source");
        assert_eq!(owner_source.path, Path::new("<build-prelude>"));
        assert_eq!(owner_source.origin, SourceOrigin::Toolchain);
        assert_eq!(signature.symbol, call.target_symbol);
        let authored = sources
            .file_at(binding.source_span)
            .expect("the Build request retains its authored source");
        assert!(
            authored.path.ends_with(
                "tests/fixtures/generics/run/implementation_parameter_contract/build.omg"
            )
        );
        occurrences.push(binding.source_span);
    }
    occurrences.sort_by_key(|span| (span.source_id.0, span.span.start, span.span.end));
    occurrences.dedup();
    assert_eq!(
        occurrences.len(),
        4,
        "all four original target bindings survive"
    );
}

#[test]
fn the_build_assembly_reports_only_the_imports_needed_to_execute_it() {
    let temporary = platform_custody::temporary_directory::TemporaryDirectory::create(
        "build-import-purposes",
        false,
    )
    .unwrap();
    let root = temporary.path().join("build.omg");
    std::fs::write(
        &root,
        r#"use {scores}/score.omg as score;
use {product}/main.omg as entry;
machine build(builder: @mutable Build) {
    let n: i32 = |score|::read();
    Build::bind(builder, linux_x86_64::ProgramEntry, |entry|::main);
}
"#,
    )
    .unwrap();
    let mut catalogue = source_compiler::representations::source_inputs::SourceCatalogue::default();
    let mut timings = CompileTimings::default();
    let capture = crate::capture_anonymous_compilation_source(&root).unwrap();
    let checkpoint = ImmutableSourceParseCheckpoint::prepare_from_sources(
        &root,
        None,
        Some(&capture),
        &mut timings,
        &mut catalogue,
    )
    .unwrap();
    let (_, assembled) = checkpoint
        .assemble(None, &Default::default(), &mut timings, &mut catalogue)
        .unwrap();
    let source = assembled.build_stage_source().unwrap();
    let [required] = source.pending_imports.as_slice() else {
        panic!(
            "unexpected unbound Build imports: {:?}",
            source.pending_imports
        )
    };
    assert_eq!(required.source, source.entry.source_id);
    assert_eq!(required.target.to_string(), "{scores}/score.omg as score");
}

#[test]
fn build_imports_are_fixed_before_a_conditional_dependency_wait() {
    let conditional = r#"use {scores}/value.omg as scores;
use {omega}/build/dependency.omg;
use main.omg as entry;
machine build(builder: @mutable BuildPlan) suspends {
    builder.product = BuildProduct::Application { name: "fixed_wait" };
    let path: BuildPath = BuildOutput::resolve(@builder.output, "ran.txt");
    let writer: i32 = BuildOutput::create(@mutable builder.output, path, 0);
    _ = BuildOutput::write(@mutable builder.output, writer, "ran");
    _ = BuildOutput::close(@mutable builder.output, writer);
    builder.tests.disabled = false;
    transition builder.tests.disabled {
        true -> suspend later(builder),
        _ -> done(builder)
    }
    machine later(builder: @mutable BuildPlan) suspends {
        let request: DependencyRegistration = Build::dependency(builder, |dependency|::from_source(Source::Path { location: "lib" }));
        suspend Build::prepare_dependency(builder, request);
        let value: i32 = |scores|::value();
    }
    machine done(builder: @mutable BuildPlan) {
        Build::bind(builder, linux_x86_64::ProgramEntry, |entry|::main);
    }
}
"#;
    let product_only = r#"use {scores}/value.omg as scores;
use {omega}/build/dependency.omg as dependencies;
machine build(builder: @mutable BuildPlan) {
    let kit: DependencyDescription = |dependencies|::from_source(Source::Path { location: "lib" });
    _ = Build::dependency(builder, kit);
    Build::bind(builder, linux_x86_64::ProgramEntry, |scores|::main);
}
"#;
    let temporary = platform_custody::temporary_directory::TemporaryDirectory::create(
        "conditional-build-imports",
        false,
    )
    .unwrap();
    let root = temporary.path().join("build.omg");
    std::fs::write(
        temporary.path().join("main.omg"),
        "public machine main() {}\n",
    )
    .unwrap();
    std::fs::write(
        temporary.path().join("value.omg"),
        "public machine value() -> i32 { 7 }\n",
    )
    .unwrap();
    let local = conditional.replace("{scores}/value.omg", "value.omg");
    for (build, required) in [
        (conditional, true),
        (local.as_str(), false),
        (product_only, false),
    ] {
        std::fs::write(&root, build).unwrap();
        let mut catalogue =
            source_compiler::representations::source_inputs::SourceCatalogue::default();
        let mut timings = CompileTimings::default();
        let capture = crate::capture_anonymous_compilation_source(&root).unwrap();
        let checkpoint = ImmutableSourceParseCheckpoint::prepare_from_sources(
            &root,
            None,
            Some(&capture),
            &mut timings,
            &mut catalogue,
        )
        .unwrap();
        let (_, assembled) = checkpoint
            .assemble(None, &Default::default(), &mut timings, &mut catalogue)
            .unwrap();
        let source = assembled.build_stage_source().unwrap();
        if required {
            let [import] = source.pending_imports.as_slice() else {
                panic!(
                    "expected exactly one unbound Build import: {:?}",
                    source.pending_imports
                );
            };
            assert_eq!(import.source, source.entry.source_id);
            assert_eq!(import.target.to_string(), "{scores}/value.omg as scores");
            assert_eq!(import.span.start, 4);
            let authored = source_text::SourceSpan::new(import.source, import.span);
            let Err(errors) = source.prepare(
                &Default::default(),
                "linux_x86_64",
                None,
                source_compiler::BuildSourceCheck { providers: &Default::default(), check_entry_file: true },
            ) else {
                panic!("a conditional wait admitted an unbound Build import");
            };
            assert_eq!(errors.len(), 1);
            assert_eq!(errors[0].source_span, Some(authored));
            assert_eq!(
                errors[0].message,
                "Build import `{scores}/value.omg as scores` has no supplied source"
            );
        } else {
            assert!(
                source.pending_imports.is_empty(),
                "{:?}",
                source.pending_imports
            );
            source
                .prepare(
                    &Default::default(),
                    "linux_x86_64",
                    None,
                    source_compiler::BuildSourceCheck { providers: &Default::default(), check_entry_file: true },
                )
                .expect("local Build code and deferred Product operands both prepare");
        }
    }
}

#[test]
fn successor_selects_only_the_requested_build_from_its_frozen_package() {
    let directory = platform_custody::temporary_directory::TemporaryDirectory::create(
        "build-successor-source",
        false,
    )
    .unwrap();
    let root = directory.path().canonicalize().unwrap().join("source");
    let stages = root.join("stages");
    std::fs::create_dir_all(&stages).unwrap();
    let initial = root.join("prepare.omg");
    let ordinary = stages.join("build.omg");
    std::fs::write(&ordinary, "machine ordinary() {}").unwrap();
    let next = stages.join("build.lock.omg");
    let main = root.join("main.omg");
    std::fs::write(&initial, "machine build() {}").unwrap();
    std::fs::write(root.join("helper.omg"), "public machine helper() {}").unwrap();
    std::fs::write(
        &next,
        "use helper.omg; machine build() { |helper|::helper(); }",
    )
    .unwrap();
    std::fs::write(&main, "machine main() {}").unwrap();
    std::fs::write(stages.join("main.omg"), "machine nested() {}").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink("build.lock.omg", stages.join("alias.omg")).unwrap();
    let capture = crate::capture_anonymous_compilation_source(&initial).unwrap();
    std::fs::remove_file(&next).unwrap();
    std::fs::write(stages.join("later.omg"), "machine build() {}").unwrap();
    #[cfg(unix)]
    {
        let moved = directory.path().join("moved");
        std::fs::rename(&root, &moved).unwrap();
        std::os::unix::fs::symlink(moved, &root).unwrap();
    }
    let predecessors = [initial.clone()];
    let prepare = |path: &Path, capture: Option<&crate::CapturedLocalSource>| {
        ImmutableSourceParseCheckpoint::prepare(
            super::CapturedBuildEntry::Successor {
                source_owner: None,
                path,
                previous_build_files: &predecessors,
            },
            None,
            capture,
            &mut CompileTimings::default(),
            &mut Default::default(),
        )
    };
    let checkpoint = prepare(&next, Some(&capture)).unwrap();
    assert_eq!(
        checkpoint.product_roots.as_deref().unwrap(),
        std::slice::from_ref(&main)
    );
    let storage = &checkpoint.source_storage;
    assert!(
        storage
            .sources
            .selected_files()
            .any(|file| file.path == root.join("helper.omg"))
    );
    let subject = storage.product_subject_sources().unwrap();
    assert!(!subject.contains(&initial));
    assert!(!subject.contains(&next));
    assert!(subject.contains(&main));
    assert!(subject.contains(&ordinary));
    assert!(prepare(&next, None).is_err());
    for invalid in [
        main,
        stages.clone(),
        stages.join("later.omg"),
        stages.join("../build.omg"),
        root.join("./prepare.omg"),
        root.join("prepare.omg/"),
        stages.join("alias.omg"),
    ] {
        assert!(
            prepare(&invalid, Some(&capture)).is_err(),
            "{}",
            invalid.display()
        );
    }
}

#[test]
fn a_prepared_build_library_can_import_a_later_frozen_file() {
    use crate::source::source_imports::{
        GeneratedFiles, ImportQueue, ProductQueryDependency, ProductQueryInputs, SourceStorage,
    };
    use crate::{
        BuildDeclarationKind, PackageBuildHandoff, PackageCompilationInputs, PackageSourceBinding,
    };
    use build_output::PackageGeneratedSource;
    use semantic_vocabulary::PackageKeyIdentity;

    let owner = PackageKeyIdentity::from_digest([9; 32]).unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("prepared-library");
    let graph = PackageCompilationInputs::new(
        owner,
        BuildDeclarationKind::Package,
        vec![
            PackageSourceBinding::new(owner, "library", root.clone())
                .with_captured_source(Arc::new(
                    source_acquisition::capture::CapturedSourceTree::new([(
                        std::path::PathBuf::from("authored.omg"),
                        source_acquisition::capture::CapturedSourceNode::File {
                            bytes: Arc::from(b"public data Authored {}".as_slice()),
                            executable: false,
                        },
                    )])
                    .unwrap(),
                ))
                .unwrap(),
        ],
        vec![],
    )
    .unwrap();
    let generated = |path: &str, text: &str| {
        PackageGeneratedSource::from_retained_parts(
            path.as_bytes().into(),
            Arc::from(text.as_bytes()),
        )
    };
    let user = generated(
        "entry.omg",
        "use value.omg; public machine read() -> u8 { |value|::get() }",
    );
    let value = generated("value.omg", "public machine get() -> u8 { 7 }");
    let assemble = |sources| {
        let handoff = PackageBuildHandoff::from_checked(
            owner,
            build_declarations::DependencyPurpose::Build,
            target::TargetProfile::LinuxX64,
            Some(target::TargetProfile::LinuxX64),
            graph.dependency_closure(),
            crate::PackageSourceConsumptionCommitment::from_retained_digest([3; 32]),
            sources,
        );
        let inputs = ProductQueryInputs::for_build_stage(
            None,
            vec![ProductQueryDependency {
                alias: "library".into(),
                max_reach: Default::default(),
                inputs: graph.clone(),
                child_build_source: None,
                generated: Some(handoff),
            }],
        )
        .unwrap();
        let mut storage = SourceStorage::default();
        inputs.mount(&mut storage)?;
        super::generated::admit_generated_files(
            &mut storage,
            &mut ImportQueue::default(),
            GeneratedFiles::for_build_libraries(&inputs)?,
            super::SourceRouting::Query(&inputs),
            &mut Default::default(),
            &mut Default::default(),
        )?;
        let pending = crate::source::source_imports::pending_build_imports(&storage);
        let paths = storage
            .resolved_imports
            .into_iter()
            .map(|import| import.path)
            .collect::<Vec<_>>();
        Ok::<_, Vec<diagnostics::Diagnostic>>((paths, pending))
    };
    let expected = (
        vec![
            root.join(source_admission::GENERATED_SOURCE_DIRECTORY)
                .join("value.omg"),
        ],
        Vec::new(),
    );
    assert_eq!(
        assemble(vec![value.clone(), user.clone()]).unwrap(),
        expected
    );
    assert_eq!(assemble(vec![user.clone(), value]).unwrap(), expected);
    let (paths, pending) = assemble(vec![user]).unwrap();
    assert!(paths.is_empty());
    let [missing] = pending.as_slice() else {
        panic!("missing imported file: {pending:?}")
    };
    assert_eq!(missing.target.root, None);
    assert_eq!(missing.target.path, "value.omg");
    assert_eq!(missing.target.alias, "value");
    assert_eq!(missing.declarations.len(), 1);
    let collision = assemble(vec![generated("authored.omg", "public data Other {}")]).unwrap_err();
    assert!(
        collision[0]
            .message
            .contains("collides with an existing source"),
        "{collision:?}"
    );
}

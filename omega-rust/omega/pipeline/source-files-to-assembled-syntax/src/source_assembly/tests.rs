//! Source assembly tests: generated syntax retention, build preludes and pending imports.

use super::{
    AssembledSyntax, BUILD_PRELUDE, CompileTimings, ImportQueue, PackageCompilationInputs,
    RetainedGeneratedSyntaxExtension, RetainedGeneratedSyntaxUnit, SourceStorage, SyntaxTrees,
    append_dependency_generated_sources_to_storage, construct_build_prelude,
    generated_source_logical_path, retain_generated_syntax_extension,
};
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

struct GeneratedSourcePackages {
    directory: PathBuf,
    consumer: PathBuf,
    producer: PathBuf,
}

impl GeneratedSourcePackages {
    fn new() -> Self {
        static NEXT_FIXTURE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let sequence = NEXT_FIXTURE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let directory = std::env::temp_dir().join(format!(
            "omega-source-collision-{}-{sequence}",
            std::process::id()
        ));
        std::fs::create_dir(&directory).expect("unique temporary package directory");
        std::fs::create_dir(directory.join("consumer")).expect("consumer directory");
        std::fs::create_dir(directory.join("producer")).expect("producer directory");
        Self {
            consumer: directory.join("consumer").canonicalize().unwrap(),
            producer: directory.join("producer").canonicalize().unwrap(),
            directory,
        }
    }
}

impl Drop for GeneratedSourcePackages {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

fn generated_source(path: &[u8], bytes: &[u8]) -> build_output::PackageGeneratedSource {
    let tree = build_output::replayed_single_ordinary_file(path, bytes)
        .expect("canonical retained generated source");
    build_output::select_included_sources(&tree, &[path.to_vec()])
        .expect("explicit generated source selection")
        .pop()
        .expect("one selected source")
}

fn generated_collision_base(root: &Path, count: usize) -> Arc<source::SourceMap> {
    let mut sources = source::SourceMap::default();
    for position in 0..count {
        sources.add(
            root.join(format!(".omega/generated/base-{position}.omg")),
            String::new(),
        );
    }
    Arc::new(sources)
}

#[test]
fn generated_source_collisions_empty_extension_preserves_large_base() {
    let root = Path::new("collision-package");
    let base = generated_collision_base(root, 1024);
    let extension = retain_generated_syntax_extension(&base, root, None, &[])
        .expect("empty extension does not introduce a collision");
    assert!(extension.units.is_empty());
    assert!(extension.generated_source_custody.is_empty());
    assert_eq!(extension.sources.as_ref(), base.as_ref());
}

#[test]
fn generated_source_collisions_singleton_checks_large_base_and_appends_new_path() {
    let root = Path::new("collision-package");
    let base = generated_collision_base(root, 1024);
    for path in ["base-0.omg", "base-1023.omg"] {
        let generated = [generated_source(path.as_bytes(), b"data Generated {}")];
        let diagnostics = retain_generated_syntax_extension(&base, root, None, &generated)
            .err()
            .expect("singleton must check both ends of the base");
        assert!(diagnostics[0].message.contains(path));
        assert!(
            diagnostics[0]
                .message
                .contains("collides with an existing source")
        );
    }
    let generated = [generated_source(b"new.omg", b"data Generated {}")];
    let extension = retain_generated_syntax_extension(&base, root, None, &generated)
        .expect("singleton absent from the base remains a new source");
    assert_eq!(extension.units.len(), 1);
    assert_eq!(extension.units[0].source_id, source::SourceId(1024));
    assert_eq!(extension.sources.len(), 1025);
    assert_eq!(base.len(), 1024);
}

#[test]
fn generated_source_collisions_many_new_paths_preserve_order_after_large_base() {
    let root = Path::new("collision-package");
    let base = generated_collision_base(root, 1024);
    let generated = (0..64)
        .rev()
        .map(|position| {
            generated_source(
                format!("new-{position}.omg").as_bytes(),
                b"data Generated {}",
            )
        })
        .collect::<Vec<_>>();
    let extension = retain_generated_syntax_extension(&base, root, None, &generated)
        .expect("all new paths append in authored rather than lookup order");
    assert_eq!(extension.units.len(), 64);
    assert_eq!(extension.sources.len(), 1088);
    for (position, unit) in extension.units.iter().enumerate() {
        assert_eq!(unit.source_id, source::SourceId(1024 + position));
        assert_eq!(
            unit.path,
            generated_source_logical_path(root, &generated[position]).unwrap()
        );
        assert_eq!(
            extension.generated_source_custody[position].1,
            generated[position]
        );
    }
    assert_eq!(base.len(), 1024);
}

#[test]
fn generated_source_collisions_batch_reports_authored_order_not_base_order() {
    let root = Path::new("collision-package");
    let base = generated_collision_base(root, 1024);
    let generated = [
        generated_source(b"base-1023.omg", b"data First {}"),
        generated_source(b"base-0.omg", b"data Second {}"),
        generated_source(b"base-1023.omg", b"data Duplicate {}"),
    ];
    let diagnostics = retain_generated_syntax_extension(&base, root, None, &generated)
        .err()
        .expect("first generated occurrence collides even when base scan encounters it last");
    assert!(diagnostics[0].message.contains("base-1023.omg"));
    assert!(
        diagnostics[0]
            .message
            .contains("collides with an existing source")
    );
}

#[test]
fn generated_source_collisions_reject_existing_new_and_late_paths() {
    let root = Path::new("collision-package");
    let mut base = source::SourceMap::default();
    base.add(root.join(".omega/generated/existing.omg"), String::new());
    let base = Arc::new(base);
    for paths in [
        vec!["existing.omg"],
        vec!["new.omg", "new.omg"],
        vec!["z.omg", "a.omg", "middle.omg", "existing.omg"],
        vec!["z.omg", "a.omg", "middle.omg", "z.omg"],
    ] {
        let generated = paths
            .iter()
            .map(|path| generated_source(path.as_bytes(), b"data Generated {}"))
            .collect::<Vec<_>>();
        let diagnostics = retain_generated_syntax_extension(&base, root, None, &generated)
            .err()
            .expect("existing or earlier generated paths must reject");
        assert!(
            diagnostics[0]
                .message
                .contains("collides with an existing source")
        );
        assert!(diagnostics[0].message.contains(paths.last().unwrap()));
        assert_eq!(base.len(), 1);
    }
}

#[test]
fn generated_source_collisions_preserve_utf8_and_parse_diagnostic_order() {
    let root = Path::new("collision-package");
    let mut base = source::SourceMap::default();
    base.add(root.join(".omega/generated/existing.omg"), String::new());
    let base = Arc::new(base);
    let invalid_utf8 = [generated_source(b"existing.omg", &[0xff])];
    let diagnostics = retain_generated_syntax_extension(&base, root, None, &invalid_utf8)
        .err()
        .expect("invalid text precedes collision on the same unit");
    assert!(diagnostics[0].message.contains("is not UTF-8 Omega source"));

    let invalid_syntax = [
        generated_source(b"first.omg", b"data {"),
        generated_source(b"existing.omg", b"data Later {}"),
    ];
    let diagnostics = retain_generated_syntax_extension(&base, root, None, &invalid_syntax)
        .err()
        .expect("earlier syntax error precedes later collision");
    assert!(diagnostics[0].message.contains("first.omg"));
    assert!(!diagnostics[0].message.contains("collides"));
}

#[test]
fn generated_source_collisions_preserve_clean_authored_source_order() {
    let root = Path::new("collision-package");
    let mut base = source::SourceMap::default();
    base.add(root.join("main.omg"), String::new());
    let base = Arc::new(base);
    let generated = [
        generated_source(b"z.omg", b"data LastAlphabetically {}"),
        generated_source(b"a.omg", b"data FirstAlphabetically {}"),
        generated_source(b"middle.omg", b"data Middle {}"),
    ];
    let extension = retain_generated_syntax_extension(&base, root, None, &generated)
        .expect("unique generated paths remain in authored order");
    for (position, unit) in extension.units.iter().enumerate() {
        assert_eq!(unit.source_id, source::SourceId(position + 1));
        assert_eq!(
            unit.path,
            generated_source_logical_path(root, &generated[position]).unwrap()
        );
        assert_eq!(
            extension.sources.get(unit.source_id).unwrap().path,
            unit.path
        );
        assert_eq!(
            extension.generated_source_custody[position].1,
            generated[position]
        );
    }
    assert_eq!(extension.sources.len(), 4);
    assert_eq!(base.len(), 1);
}

fn dependency_generated_inputs(
    generated: Vec<build_output::PackageGeneratedSource>,
    consumer_root: &Path,
    dependency_root: &Path,
) -> PackageCompilationInputs {
    use package_compilation::{
        PackageDependencyBinding, PackageGeneratedSourceBundle, PackageSourceBinding,
        PackageSourceConsumptionCommitment,
    };
    let root = semantic_vocabulary::PackageKeyIdentity::from_digest([1; 32])
        .expect("nonzero root identity");
    let dependency = semantic_vocabulary::PackageKeyIdentity::from_digest([2; 32])
        .expect("nonzero dependency identity");
    let inputs = PackageCompilationInputs::new_package(
        root,
        vec![
            PackageSourceBinding::new(root, "consumer", consumer_root.to_path_buf()),
            PackageSourceBinding::new(dependency, "producer", dependency_root.to_path_buf()),
        ],
        vec![PackageDependencyBinding::new(root, "producer", dependency)],
    )
    .expect("closed dependency graph");
    let bundle = PackageGeneratedSourceBundle::from_checked(
        dependency,
        target::TargetProfile::WindowsX64,
        inputs.dependency_closure_for(dependency),
        PackageSourceConsumptionCommitment::for_test([3; 32]),
        generated,
    );
    inputs
        .with_complete_dependency_generated_sources(vec![bundle])
        .expect("complete generated-source handoff")
}

#[test]
fn dependency_generated_source_collisions_reject_late_duplicates_before_parsing() {
    let packages = GeneratedSourcePackages::new();
    let inputs = dependency_generated_inputs(
        vec![
            generated_source(b"z.omg", b"data {"),
            generated_source(b"a.omg", b"data First {}"),
            generated_source(b"z.omg", b"data Last {}"),
        ],
        &packages.consumer,
        &packages.producer,
    );
    let mut storage = SourceStorage::default();
    let diagnostics = append_dependency_generated_sources_to_storage(
        &mut storage,
        &mut ImportQueue::default(),
        Some("windows_x86_64"),
        &inputs,
        &mut CompileTimings::default(),
    )
    .expect_err("dependency path collisions precede parsing all units");
    assert!(diagnostics[0].message.contains("z.omg"));
    assert!(
        diagnostics[0]
            .message
            .contains("collides with another source")
    );
    assert_eq!(storage.next_source_id(), 0);
}

#[test]
fn dependency_generated_source_collisions_preserve_clean_authored_order() {
    let packages = GeneratedSourcePackages::new();
    let generated = vec![
        generated_source(b"z.omg", b"data LastAlphabetically {}"),
        generated_source(b"a.omg", b"data FirstAlphabetically {}"),
    ];
    let inputs =
        dependency_generated_inputs(generated.clone(), &packages.consumer, &packages.producer);
    let mut storage = SourceStorage::default();
    let retained = append_dependency_generated_sources_to_storage(
        &mut storage,
        &mut ImportQueue::default(),
        Some("windows_x86_64"),
        &inputs,
        &mut CompileTimings::default(),
    )
    .expect("clean handoff retains authored order");
    for (position, (source_id, source)) in retained.iter().enumerate() {
        assert_eq!(*source_id, source::SourceId(position));
        assert_eq!(source, &generated[position]);
        assert_eq!(
            storage.sources.get(*source_id).unwrap().path,
            generated_source_logical_path(&packages.producer, source).unwrap()
        );
    }
    assert_eq!(storage.next_source_id(), 2);
}

#[test]
fn dependency_generated_source_collisions_reject_physical_paths_only_for_dependencies() {
    let packages = GeneratedSourcePackages::new();
    let root = &packages.producer;
    let generated_directory = root.join(".omega/generated");
    std::fs::create_dir_all(&generated_directory).expect("create physical collision directory");
    let physical_path = generated_directory.join("existing.omg");
    std::fs::write(&physical_path, b"physical bytes must not be read")
        .expect("create physical collision");
    let generated = vec![generated_source(b"existing.omg", b"data Generated {}")];
    let inputs = dependency_generated_inputs(generated.clone(), &packages.consumer, root);
    let mut storage = SourceStorage::default();
    let dependency_result = append_dependency_generated_sources_to_storage(
        &mut storage,
        &mut ImportQueue::default(),
        Some("windows_x86_64"),
        &inputs,
        &mut CompileTimings::default(),
    );
    let continuation_result = retain_generated_syntax_extension(
        &Arc::new(source::SourceMap::default()),
        root,
        None,
        &generated,
    );
    let diagnostics = dependency_result.expect_err("dependency physical path must reject");
    assert!(
        diagnostics[0]
            .message
            .contains("collides with another source")
    );
    assert_eq!(storage.next_source_id(), 0);
    let extension = continuation_result.expect("own continuation does not inspect physical paths");
    assert_eq!(extension.source_count(), 1);
}

#[test]
fn retained_generated_syntax_extension_rejects_equal_but_distinct_base_sources() {
    let mut sources = source::SourceMap::default();
    sources.add(
        PathBuf::from("main.omg"),
        "const ANSWER: u32 = 42;".to_owned(),
    );
    let base_sources = Arc::new(sources);
    let substituted_sources = Arc::new((*base_sources).clone());
    assert_eq!(base_sources, substituted_sources);
    let extension = RetainedGeneratedSyntaxExtension {
        units: Vec::new(),
        sources: base_sources.clone(),
        generated_source_custody: Vec::new(),
        base_sources,
    };
    let diagnostics = extension
        .into_pre_resolution_inputs(&substituted_sources)
        .expect_err("equal source contents cannot substitute for the retained source identity");
    assert!(
        diagnostics[0]
            .message
            .contains("no longer matches its base source frontier")
    );
}

#[test]
fn retained_generated_syntax_extension_preserves_unit_custody_without_reparsing() {
    let base_text = "machine base() -> u64 { 1 }";
    let base_tokens = source_files_to_tokens::Lexer::new(base_text)
        .tokenize()
        .expect("lex base source");
    let base_syntax =
        tokens_to_syntax_trees::parse_syntax_trees_with_id(source::SourceId(0), &base_tokens)
            .expect("parse base source");
    let mut base_sources = source::SourceMap::default();
    base_sources.add(PathBuf::from("src/main.omg"), base_text.to_owned());
    let base_sources = Arc::new(base_sources);
    let assembled = AssembledSyntax {
        syntax_trees: base_syntax,
        sources: base_sources.clone(),
        build_source_id: None,
        application: None,
        source_scoped_top_level_bindings: Vec::new(),
        generated_source_custody: Vec::new(),
        build_scope_sources: std::collections::HashSet::new(),
    };

    let extension_inputs = [
        (".omega/generated/first.omg", "machine first() -> u64 { 2 }"),
        (
            ".omega/generated/second.omg",
            "data Later { value: u64 }\nmachine second() -> u64 { 3 }",
        ),
    ];
    let mut extension_units = Vec::new();
    let mut combined_sources = (*base_sources).clone();
    for (offset, (path, text)) in extension_inputs.iter().enumerate() {
        let source_id = source::SourceId(offset + 1);
        let tokens = source_files_to_tokens::Lexer::new(text)
            .tokenize()
            .expect("lex extension source");
        let parsed = tokens_to_syntax_trees::parse_syntax_trees_with_id(source_id, &tokens)
            .expect("parse extension source");
        extension_units.push(RetainedGeneratedSyntaxUnit {
            source_id,
            path: PathBuf::from(path),
            syntax_trees: parsed.clone(),
            root_item_count: parsed.root_item_count(),
        });
        combined_sources.add_with_metadata_and_resolution_stratum(
            PathBuf::from(path),
            (*text).to_owned(),
            PathBuf::from("."),
            None,
            source::SourceOrigin::User,
            source::SourceResolutionStratum::CurrentActivationExtension,
        );
    }
    let extension = RetainedGeneratedSyntaxExtension {
        units: extension_units,
        sources: Arc::new(combined_sources),
        generated_source_custody: Vec::new(),
        base_sources,
    };

    let (units, sources) = extension
        .into_pre_resolution_inputs(&assembled.sources)
        .expect("retained units should match their exact base");

    assert_eq!(assembled.sources.len(), 1);
    assert_eq!(assembled.syntax_trees.root_item_count(), 1);
    assert_eq!(sources.len(), 3);
    assert_eq!(units.len(), 2);
    assert_eq!(units[0].root_item_count(), 1);
    assert_eq!(units[1].root_item_count(), 2);
    assert_eq!(
        sources
            .get(source::SourceId(2))
            .expect("second extension source")
            .source
            .as_ref(),
        extension_inputs[1].1,
    );
    let root_names = units
        .iter()
        .flat_map(SyntaxTrees::root_items)
        .filter_map(|root| match root {
            syntax_trees::item::Item::Data(data) => Some(data.name.as_str()),
            syntax_trees::item::Item::Machine(machine) => Some(machine.name.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(root_names, ["first", "Later", "second"]);
}

#[test]
fn generated_unit_pre_resolution_is_unit_local() {
    let base_text = "data Main { value: u8; }";
    let base_tokens = source_files_to_tokens::Lexer::new(base_text)
        .tokenize()
        .expect("lex base source");
    let base_syntax =
        tokens_to_syntax_trees::parse_syntax_trees_with_id(source::SourceId(0), &base_tokens)
            .expect("parse base source");
    let mut base_sources = source::SourceMap::default();
    base_sources.add(PathBuf::from("main.omg"), base_text.to_owned());
    let base_sources = Arc::new(base_sources);
    let assembled = AssembledSyntax {
        syntax_trees: base_syntax,
        sources: base_sources.clone(),
        build_source_id: None,
        application: None,
        source_scoped_top_level_bindings: Vec::new(),
        generated_source_custody: Vec::new(),
        build_scope_sources: std::collections::HashSet::new(),
    };
    let template_text = "data Split<T> { value: T; }";
    let wrapper_text = "data SplitUse { value: Split<u32>; }";
    let template = tokens_to_syntax_trees::parse_syntax_trees_with_id(
        source::SourceId(1),
        &source_files_to_tokens::Lexer::new(template_text)
            .tokenize()
            .expect("lex split template"),
    )
    .expect("parse split template");
    let wrapper = tokens_to_syntax_trees::parse_syntax_trees_with_id(
        source::SourceId(2),
        &source_files_to_tokens::Lexer::new(wrapper_text)
            .tokenize()
            .expect("lex split wrapper"),
    )
    .expect("parse split wrapper");
    let mut sources = (*base_sources).clone();
    sources.add_with_metadata_and_resolution_stratum(
        PathBuf::from(".omega/generated/template.omg"),
        template_text.to_owned(),
        PathBuf::from("."),
        None,
        source::SourceOrigin::User,
        source::SourceResolutionStratum::CurrentActivationExtension,
    );
    sources.add_with_metadata_and_resolution_stratum(
        PathBuf::from(".omega/generated/wrapper.omg"),
        wrapper_text.to_owned(),
        PathBuf::from("."),
        None,
        source::SourceOrigin::User,
        source::SourceResolutionStratum::CurrentActivationExtension,
    );
    let sources = Arc::new(sources);
    let extension = RetainedGeneratedSyntaxExtension {
        units: vec![
            RetainedGeneratedSyntaxUnit {
                source_id: source::SourceId(1),
                path: PathBuf::from(".omega/generated/template.omg"),
                syntax_trees: template.clone(),
                root_item_count: 1,
            },
            RetainedGeneratedSyntaxUnit {
                source_id: source::SourceId(2),
                path: PathBuf::from(".omega/generated/wrapper.omg"),
                syntax_trees: wrapper.clone(),
                root_item_count: 1,
            },
        ],
        sources: sources.clone(),
        generated_source_custody: Vec::new(),
        base_sources,
    };
    let (units, _) = extension
        .into_pre_resolution_inputs(&assembled.sources)
        .expect("retained units match their base");
    let normalized_counts = units
        .into_iter()
        .map(|unit| {
            build_time_evaluation::evaluate_pre_resolution(
                build_time_evaluation::BuildTimeEvaluationRequest {
                    syntax_trees: unit,
                    source_context: Some(build_time_evaluation::BuildTimeSourceContext {
                        sources: sources.clone(),
                        source_scoped_top_level_bindings: &[],
                        selection_authority: None,
                        retained_base: None,
                    }),
                },
            )
            .expect("evaluate one extension unit")
            .into_syntax_and_pre_check()
            .0
            .root_item_count()
        })
        .collect::<Vec<_>>();
    assert_eq!(normalized_counts, [1, 1]);

    let mut combined = template;
    combined.extend_from(&wrapper);
    assert_eq!(
        syntax_trees_to_symbol_resolved_trees::pre_resolution::normalize_generic_data(
            syntax_trees_to_symbol_resolved_trees::pre_resolution::GenericDataRequest {
                syntax: combined,
                sources: Some(sources),
                top_level_bindings: Vec::new(),
                retained_base: None
            }
        )
        .expect("combined normalization demonstrates the forbidden cross-unit synthesis")
        .root_item_count(),
        3
    );
}

#[test]
fn build_prelude_owns_canonical_dependency_vocabulary() {
    let tokens = source_files_to_tokens::Lexer::new(BUILD_PRELUDE)
        .tokenize()
        .expect("toolchain build prelude must lex");
    let syntax_trees = tokens_to_syntax_trees::parse_syntax_trees(&tokens)
        .expect("toolchain build prelude must parse as ordinary Omega");

    let source = syntax_trees
        .root_items()
        .find_map(|item| match item {
            syntax_trees::item::Item::Data(data) if data.name.as_str() == "Source" => Some(data),
            _ => None,
        })
        .expect("build prelude must define Source");
    let source_cases = syntax_trees.items.data_members(source.members);
    let [
        syntax_trees::item::DataMember::Variant(path),
        syntax_trees::item::DataMember::Variant(git),
    ] = source_cases
    else {
        panic!("Source must contain exactly Path and Git cases");
    };
    assert_eq!(path.name.as_str(), "Path");
    assert_eq!(
        syntax_trees
            .items
            .data_payload_fields(path.payload)
            .iter()
            .map(|field| field.name.as_str())
            .collect::<Vec<_>>(),
        ["location"]
    );
    assert_eq!(git.name.as_str(), "Git");
    assert_eq!(
        syntax_trees
            .items
            .data_payload_fields(git.payload)
            .iter()
            .map(|field| field.name.as_str())
            .collect::<Vec<_>>(),
        ["repository", "revision", "selection"]
    );

    let package_selection = syntax_trees
        .root_items()
        .find_map(|item| match item {
            syntax_trees::item::Item::Data(data) if data.name.as_str() == "PackageSelection" => {
                Some(data)
            }
            _ => None,
        })
        .expect("build prelude must define PackageSelection");
    assert_eq!(
        syntax_trees
            .items
            .data_members(package_selection.members)
            .iter()
            .filter_map(|member| match member {
                syntax_trees::item::DataMember::Variant(variant) => {
                    Some(variant.name.as_str())
                }
                _ => None,
            })
            .collect::<Vec<_>>(),
        ["Root", "Named"]
    );

    let composition_mode = syntax_trees
        .root_items()
        .find_map(|item| match item {
            syntax_trees::item::Item::Data(data) if data.name.as_str() == "CompositionMode" => {
                Some(data)
            }
            _ => None,
        })
        .expect("build prelude must define CompositionMode");
    let composition_cases = syntax_trees.items.data_members(composition_mode.members);
    assert_eq!(composition_cases.len(), 2);
    assert_eq!(
        composition_cases
            .iter()
            .filter_map(|member| match member {
                syntax_trees::item::DataMember::Variant(variant)
                    if syntax_trees
                        .items
                        .data_payload_fields(variant.payload)
                        .is_empty() =>
                {
                    Some(variant.name.as_str())
                }
                _ => None,
            })
            .collect::<Vec<_>>(),
        ["Fused", "Independent"]
    );

    let mut dependency_methods = syntax_trees
        .root_items()
        .filter_map(|item| match item {
            syntax_trees::item::Item::Machine(machine)
                if machine
                    .attached_data
                    .as_ref()
                    .is_some_and(|owner| owner.as_str() == "Build") =>
            {
                Some(machine)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    dependency_methods.sort_by_key(|machine| machine.name.as_str());
    assert_eq!(dependency_methods.len(), 9);
    assert_eq!(dependency_methods[0].name.as_str(), "Build::application");
    assert_eq!(dependency_methods[1].name.as_str(), "Build::artifact_only");
    assert_eq!(dependency_methods[2].name.as_str(), "Build::build_depend");
    assert_eq!(
        dependency_methods[3].name.as_str(),
        "Build::build_depend_as"
    );
    assert_eq!(dependency_methods[4].name.as_str(), "Build::depend");
    assert_eq!(dependency_methods[5].name.as_str(), "Build::depend_as");
    assert_eq!(dependency_methods[6].name.as_str(), "Build::exclude_crash");
    assert_eq!(dependency_methods[7].name.as_str(), "Build::member");
    assert_eq!(dependency_methods[8].name.as_str(), "Build::package");

    let parameter_names = |machine: &syntax_trees::item::Machine| {
        let [entry] = syntax_trees.items.state_handles(machine.states) else {
            panic!("dependency method must have exactly one entry state");
        };
        syntax_trees
            .items
            .state_parameters(syntax_trees.items.state(*entry).parameters)
            .iter()
            .map(|handle| syntax_trees.items.state_parameter(*handle).name.as_str())
            .collect::<Vec<_>>()
    };
    assert_eq!(parameter_names(dependency_methods[0]), ["self", "name"]);
    assert_eq!(parameter_names(dependency_methods[1]), ["self"]);
    assert_eq!(parameter_names(dependency_methods[2]), ["self", "source"]);
    assert_eq!(
        parameter_names(dependency_methods[3]),
        ["self", "alias", "source"]
    );
    assert_eq!(parameter_names(dependency_methods[4]), ["self", "source"]);
    assert_eq!(
        parameter_names(dependency_methods[5]),
        ["self", "alias", "source"]
    );
    assert_eq!(parameter_names(dependency_methods[6]), ["self", "cause"]);
    assert_eq!(parameter_names(dependency_methods[7]), ["self", "path"]);
    assert_eq!(parameter_names(dependency_methods[8]), ["self", "name"]);
    assert!(!syntax_trees.root_items().any(|item| matches!(
        item,
        syntax_trees::item::Item::Machine(machine)
            if machine.attached_data.is_none() && machine.name.as_str() == "path"
    )));
}

#[test]
fn targeted_build_preludes_expose_one_closed_target_while_targetless_omit_it() {
    let expected_cases = [
        "LinuxArm64",
        "LinuxX86_64",
        "MacosArm64",
        "WindowsX86_64",
        "UefiX86_64",
        "CrossPlatformCli",
        "LocalUnchecked",
    ];
    for (has_exact_target, expected_target_fields) in [(false, 0), (true, 1)] {
        let prelude = construct_build_prelude(BUILD_PRELUDE, has_exact_target);
        let tokens = source_files_to_tokens::Lexer::new(&prelude)
            .tokenize()
            .expect("constructed build prelude must lex");
        let syntax_trees = tokens_to_syntax_trees::parse_syntax_trees(&tokens)
            .expect("constructed build prelude must parse as ordinary Omega");
        let target_profile = syntax_trees
            .root_items()
            .find_map(|item| match item {
                syntax_trees::item::Item::Data(data) if data.name.as_str() == "TargetProfile" => {
                    Some(data)
                }
                _ => None,
            })
            .expect("build prelude must define TargetProfile");
        assert_eq!(
            syntax_trees
                .items
                .data_members(target_profile.members)
                .iter()
                .filter_map(|member| match member {
                    syntax_trees::item::DataMember::Variant(variant) => {
                        Some(variant.name.as_str())
                    }
                    _ => None,
                })
                .collect::<Vec<_>>(),
            expected_cases
        );
        let x86_deployment_features = syntax_trees
            .root_items()
            .find_map(|item| match item {
                syntax_trees::item::Item::Data(data)
                    if data.name.as_str() == "X86DeploymentFeatures" =>
                {
                    Some(data)
                }
                _ => None,
            })
            .expect("build prelude must define X86DeploymentFeatures");
        assert_eq!(
            syntax_trees
                .items
                .data_members(x86_deployment_features.members)
                .iter()
                .filter_map(|member| match member {
                    syntax_trees::item::DataMember::Variant(variant) => {
                        Some(variant.name.as_str())
                    }
                    _ => None,
                })
                .collect::<Vec<_>>(),
            ["Baseline", "AvxFma3"]
        );
        let build = syntax_trees
            .root_items()
            .find_map(|item| match item {
                syntax_trees::item::Item::Data(data) if data.name.as_str() == "Build" => Some(data),
                _ => None,
            })
            .expect("build prelude must define Build");
        let target_fields = syntax_trees
            .items
            .data_members(build.members)
            .iter()
            .filter_map(|member| match member {
                syntax_trees::item::DataMember::Field(field) if field.name.as_str() == "target" => {
                    Some(field)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(target_fields.len(), expected_target_fields);
        if let [field] = target_fields.as_slice() {
            assert!(matches!(
                syntax_trees
                    .type_references
                    .type_reference(field.type_reference),
                syntax_trees::types::TypeReferenceNode::Named(name)
                    if name.as_str() == "TargetProfile"
            ));
        }
        let x86_feature_fields = syntax_trees
            .items
            .data_members(build.members)
            .iter()
            .filter_map(|member| match member {
                syntax_trees::item::DataMember::Field(field)
                    if field.name.as_str() == "x86_deployment_features" =>
                {
                    Some(field)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(x86_feature_fields.len(), expected_target_fields);
        if let [field] = x86_feature_fields.as_slice() {
            assert!(matches!(
                syntax_trees
                    .type_references
                    .type_reference(field.type_reference),
                syntax_trees::types::TypeReferenceNode::Named(name)
                    if name.as_str() == "X86DeploymentFeatures"
            ));
        }
    }
}

#[test]
fn build_prelude_owns_the_exact_optimization_vocabulary() {
    let expected_cases = optimization_core::Optimization::ALL
        .iter()
        .map(|optimization| optimization.build_case_name())
        .collect::<Vec<_>>();
    let expected_fields = std::iter::once("human_report")
        .chain(
            optimization_core::Optimization::ALL
                .iter()
                .map(|optimization| optimization.build_counter_field()),
        )
        .collect::<Vec<_>>();
    let prelude = construct_build_prelude(BUILD_PRELUDE, false);
    let tokens = source_files_to_tokens::Lexer::new(&prelude)
        .tokenize()
        .expect("toolchain build prelude must lex");
    let syntax_trees = tokens_to_syntax_trees::parse_syntax_trees(&tokens)
        .expect("toolchain build prelude must parse as ordinary Omega");
    let optimization = syntax_trees
        .root_items()
        .find_map(|item| match item {
            syntax_trees::item::Item::Data(data) if data.name.as_str() == "Optimization" => {
                Some(data)
            }
            _ => None,
        })
        .expect("build prelude must define Optimization");
    assert_eq!(
        syntax_trees
            .items
            .data_members(optimization.members)
            .iter()
            .filter_map(|member| match member {
                syntax_trees::item::DataMember::Variant(variant) => {
                    Some(variant.name.as_str())
                }
                _ => None,
            })
            .collect::<Vec<_>>(),
        expected_cases
    );
    let optimizations = syntax_trees
        .root_items()
        .find_map(|item| match item {
            syntax_trees::item::Item::Data(data) if data.name.as_str() == "Optimizations" => {
                Some(data)
            }
            _ => None,
        })
        .expect("build prelude must define Optimizations");
    assert_eq!(
        syntax_trees
            .items
            .data_members(optimizations.members)
            .iter()
            .filter_map(|member| match member {
                syntax_trees::item::DataMember::Field(field) => {
                    Some(field.name.as_str())
                }
                _ => None,
            })
            .collect::<Vec<_>>(),
        expected_fields
    );
    let build = syntax_trees
        .root_items()
        .find_map(|item| match item {
            syntax_trees::item::Item::Data(data) if data.name.as_str() == "Build" => Some(data),
            _ => None,
        })
        .expect("build prelude must define Build");
    assert!(
        syntax_trees
            .items
            .data_members(build.members)
            .iter()
            .any(|member| matches!(
                member,
                syntax_trees::item::DataMember::Field(field)
                    if field.name.as_str() == "optimizations"
            ))
    );
    assert!(syntax_trees.root_items().any(|item| matches!(
        item,
        syntax_trees::item::Item::Machine(machine)
            if machine.name.as_str() == "Optimizations::enable"
    )));
    assert!(syntax_trees.root_items().any(|item| matches!(
        item,
        syntax_trees::item::Item::Machine(machine)
            if machine.name.as_str() == "Optimizations::emit_report"
    )));
}

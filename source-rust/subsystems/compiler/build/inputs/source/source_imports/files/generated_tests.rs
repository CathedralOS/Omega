use super::*;
use crate::{BuildDeclarationKind, PackageSourceBinding};
use build_output::PackageGeneratedSource;
use source_compiler::representations::source_inputs::SourceCatalogue;
use source_compiler::representations::source_symbols::SourceScopedTopLevelBinding;
use std::sync::Arc;

#[test]
fn generated_files_can_import_the_checked_base_and_one_another() {
    let key = PackageKeyIdentity::from_digest([7; 32]).unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("generated-local-import");
    let base_text = "public data Base {}";
    let tree = source_acquisition::capture::CapturedSourceTree::new([
        (
            PathBuf::from("build.omg"),
            source_acquisition::capture::CapturedSourceNode::File {
                bytes: Arc::from(b"machine build(builder: @mutable BuildPlan) { builder.product = BuildProduct::Package { name: \"generated\" }; }".as_slice()),
                executable: false,
            },
        ),
        (
            PathBuf::from("main.omg"),
            source_acquisition::capture::CapturedSourceNode::File {
                bytes: Arc::from(b"use entry.omg; public machine read() -> u8 { |entry|::make() }".as_slice()),
                executable: false,
            },
        ),
        (
            PathBuf::from("types"),
            source_acquisition::capture::CapturedSourceNode::Directory,
        ),
        (
            PathBuf::from("linked.omg"),
            source_acquisition::capture::CapturedSourceNode::Symlink {
                target: "types/base.omg".into(),
            },
        ),
        (
            PathBuf::from("types/base.omg"),
            source_acquisition::capture::CapturedSourceNode::File {
                bytes: Arc::from(base_text.as_bytes()),
                executable: false,
            },
        ),
    ])
    .unwrap();
    let graph = PackageCompilationInputs::new(
        key,
        BuildDeclarationKind::Package,
        vec![
            PackageSourceBinding::new(key, "generated", root.clone())
                .with_captured_source(Arc::new(tree))
                .unwrap(),
        ],
        vec![],
    )
    .unwrap();
    let mut catalogue = SourceCatalogue::default();
    use crate::source::assembly::{
        CapturedBuildEntry, ImmutableSourceParseCheckpoint, ProductEntrySources,
    };
    let mut timings = Default::default();
    let checkpoint = ImmutableSourceParseCheckpoint::prepare(
        CapturedBuildEntry::Initial(&root.join("build.omg")),
        Some(&graph),
        None,
        &mut timings,
        &mut catalogue,
    )
    .unwrap();
    let build = checkpoint
        .assemble(
            Some(&graph),
            &Default::default(),
            &mut timings,
            &mut catalogue,
        )
        .unwrap()
        .1;
    assert!(
        build
            .parsed
            .files
            .selected_files()
            .any(|file| file.path == root.join("build.omg"))
    );
    assert!(build.parsed.files.selected_files().all(|file| {
        file.resolution_stratum
            != source_admission::SourceResolutionStratum::CurrentActivationExtension
    }));
    let deferred = build.product_sources.unwrap();
    let generated = |path: &str, text: &str| {
        PackageGeneratedSource::from_retained_parts(
            path.as_bytes().into(),
            Arc::from(text.as_bytes()),
        )
    };
    let units = [
        generated("math/value.omg", "public machine value() -> u8 { 7 }"),
        generated(
            "entry.omg",
            "use math/value.omg; use types/base.omg; use linked.omg;
             public machine make() -> u8 { |value|::value() }
             machine accepts(base: |base|::Base, linked: |linked|::Base) {}",
        ),
    ];
    let assemble =
        |units: &[PackageGeneratedSource], timings: &mut _, catalogue: &mut SourceCatalogue| {
            deferred.clone().assemble(
                Some(&graph),
                ProductEntrySources::default(),
                units,
                &[],
                &Default::default(),
                timings,
                catalogue,
            )
        };
    let product = assemble(&units, &mut timings, &mut catalogue).unwrap().1;
    let id = |logical: &str| {
        product
            .parsed
            .files
            .selected_files()
            .find(|file| {
                file.dependency_scope == DependencyScope::Product
                    && file.package_identity == Some(key)
                    && file.logical_filename().unwrap() == logical
            })
            .unwrap()
            .source_id
    };
    let value = id("math/value.omg");
    let entry = id("entry.omg");
    let authored = id("types/base.omg");
    for binding in [
        SourceScopedTopLevelBinding::file_import(id("main.omg"), entry, "entry"),
        SourceScopedTopLevelBinding::file_import(entry, value, "value"),
        SourceScopedTopLevelBinding::file_import(entry, authored, "base"),
        SourceScopedTopLevelBinding::file_import(entry, authored, "linked"),
    ] {
        assert!(product.parsed.imports.contains(&binding), "{binding:?}");
    }
    assert_eq!(
        product.generated_source_custody,
        [(value, units[0].clone()), (entry, units[1].clone())]
    );
    for taken in ["types/base.omg", "linked.omg"] {
        let duplicate = [generated(taken, "public data Other {}")];
        let Err(errors) = assemble(&duplicate, &mut timings, &mut catalogue) else {
            panic!("the authored and generated files have the same logical name");
        };
        assert!(
            errors[0]
                .message
                .contains("collides with an existing source"),
            "{errors:?}"
        );
    }
}

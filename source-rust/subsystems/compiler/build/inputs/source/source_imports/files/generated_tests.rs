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
    let tree = source_capture::CapturedSourceTree::new([
        (
            PathBuf::from("types"),
            source_capture::CapturedSourceNode::Directory,
        ),
        (
            PathBuf::from("types/base.omg"),
            source_capture::CapturedSourceNode::File {
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
    let mut base = catalogue.begin_selection();
    let authored = catalogue
        .admit_checked_instance(
            &mut base,
            root.join("types/base.omg"),
            base_text,
            root.clone(),
            Some(key),
            source_admission::SourceOrigin::User,
            source_admission::SourceResolutionStratum::Base,
            DependencyScope::Product,
        )
        .unwrap()
        .source_id;
    let base = Arc::new(base);
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
            "use math/value.omg; use types/base.omg; machine make() -> u8 { |value|::value() }",
        ),
    ];
    let admit = |units: &[PackageGeneratedSource], catalogue: &mut SourceCatalogue| {
        super::super::admit_generated_extension(&base, &root, Some(key), units, catalogue)
    };
    let extension = admit(&units, &mut catalogue).unwrap();
    let value = extension.custody()[0].0;
    let entry = extension.custody()[1].0;
    let request = extension
        .prepare_source_request(&base, &graph, &mut Default::default())
        .unwrap();
    assert_eq!(
        request.top_level_bindings,
        [
            SourceScopedTopLevelBinding::file_import(entry, value, "value"),
            SourceScopedTopLevelBinding::file_import(entry, authored, "base"),
        ]
    );
    let duplicate = [generated("types/base.omg", "public data Other {}")];
    let Err(errors) = admit(&duplicate, &mut catalogue) else {
        panic!("the authored and generated files have the same logical name");
    };
    assert!(
        errors[0]
            .message
            .contains("collides with an existing source")
    );
}

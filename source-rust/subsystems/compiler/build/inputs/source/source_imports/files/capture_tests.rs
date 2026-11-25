use super::*;
use crate::{BuildDeclarationKind, PackageDependencyBinding, PackageSourceBinding};
use source_capture::{CapturedSourceNode, CapturedSourceTree};
use source_compiler::{CompileTimings, SourceInput, SourceSyntax};
use std::sync::Arc;

#[test]
fn local_and_dependency_file_roots_select_only_their_exact_admitted_source() {
    let app = PackageKeyIdentity::from_digest([1; 32]).unwrap();
    let dependency = PackageKeyIdentity::from_digest([2; 32]).unwrap();
    let package = |key, name, paths: &[&str]| {
        PackageSourceBinding::new(key, name, Path::new(env!("CARGO_MANIFEST_DIR")).join(name))
            .with_captured_source(Arc::new(
                CapturedSourceTree::new(paths.iter().map(|path| {
                    (
                        PathBuf::from(path),
                        if Path::new(path).extension().is_none() {
                            CapturedSourceNode::Directory
                        } else {
                            CapturedSourceNode::File {
                                bytes: Arc::from(b"machine f() {}".as_slice()),
                                executable: false,
                            }
                        },
                    )
                }))
                .unwrap(),
            ))
            .unwrap()
    };
    let graph = PackageCompilationInputs::new(
        app,
        BuildDeclarationKind::Package,
        vec![
            package(
                app,
                "app",
                &[
                    "sub",
                    "sub/main.omg",
                    "kit",
                    "kit/math",
                    "kit/math/v.omg",
                    "kit/math/other.omg",
                ],
            ),
            package(dependency, "lib", &["math", "math/v.omg", "math/other.omg"]),
        ],
        vec![PackageDependencyBinding::new(app, "kit", dependency)],
    )
    .unwrap();
    let root = graph.package_root(app).unwrap();
    let imports = "use kit/math/v.omg as local; use {kit}/math/v.omg as dep; use {kit}/other/v.omg as absent; use {lib}/math/v.omg as indirect;";
    let parsed = SourceInput(
        [source_admission::LoadedSource {
            source_id: source_text::SourceId(1),
            path: root.join("sub/main.omg"),
            source: Arc::from(imports),
            origin: None,
            scope: DependencyScope::Product,
        }]
        .into_iter()
        .collect(),
    )
    .append(&mut SourceSyntax::default(), &mut CompileTimings::default())
    .unwrap();
    let unit = &parsed.sources.span_or_empty(parsed.batch)[0];
    let rooted = FileImport::from(unit, &unit.imports[1]).unwrap();
    assert!(matches!(
        rooted
            .reconciled(
                &graph,
                Some(app),
                DependencyPurpose::Build,
                None,
                graph.source_filesystem()
            )
            .unwrap(),
        FileSelection::Unbound
    ));
    let resolve = |index: usize| -> Result<Vec<ResolvedSourceImport>, Vec<Diagnostic>> {
        let file = FileImport::from(unit, &unit.imports[index]).unwrap();
        let selected = file.reconciled(
            &graph,
            Some(app),
            DependencyPurpose::Product,
            None,
            graph.source_filesystem(),
        )?;
        let FileSelection::Package(request) = selected else {
            panic!("a selected package file must come from its owner")
        };
        request.resolve_for_exact_target(&graph, &mut Vec::new())
    };
    assert_eq!(
        resolve(0)
            .unwrap()
            .into_iter()
            .map(|file| file.path)
            .collect::<Vec<_>>(),
        [root.join("kit/math/v.omg")]
    );
    assert_eq!(
        resolve(1)
            .unwrap()
            .into_iter()
            .map(|file| file.path)
            .collect::<Vec<_>>(),
        [graph.package_root(dependency).unwrap().join("math/v.omg")]
    );
    assert!(
        resolve(2).unwrap_err()[0]
            .message
            .contains("no admitted logical file `{kit}/other/v.omg`")
    );
    assert!(
        resolve(3).unwrap_err()[0]
            .message
            .contains("no direct product dependency named `lib`")
    );
}

#[test]
fn a_captured_file_link_selects_its_source_with_the_same_package_permissions() {
    let outer = PackageKeyIdentity::from_digest([1; 32]).unwrap();
    let inner = PackageKeyIdentity::from_digest([2; 32]).unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("linked-import");
    let vendor = root.join("vendor");
    let file = || CapturedSourceNode::File {
        bytes: Arc::from(b"public data Number {}".as_slice()),
        executable: false,
    };
    let link = |target: &str| CapturedSourceNode::Symlink {
        target: target.into(),
    };
    let dependency = Arc::new(
        CapturedSourceTree::new([
            (PathBuf::from("types.omg"), file()),
            (PathBuf::from("public.omg"), link("types.omg")),
            (PathBuf::from("build.omg"), file()),
            (PathBuf::from("setup.omg"), link("build.omg")),
        ])
        .unwrap(),
    );
    let mut members = vec![
        (PathBuf::from("vendor"), CapturedSourceNode::Directory),
        (PathBuf::from("borrowed.omg"), link("vendor/types.omg")),
    ];
    members.extend(
        dependency
            .entries()
            .map(|(path, node)| (Path::new("vendor").join(path), node.clone())),
    );
    let packages = PackageCompilationInputs::new(
        outer,
        BuildDeclarationKind::Package,
        vec![
            PackageSourceBinding::new(outer, "outer", root.clone())
                .with_captured_source(Arc::new(CapturedSourceTree::new(members).unwrap()))
                .unwrap(),
            PackageSourceBinding::new(inner, "inner", vendor.clone())
                .with_captured_source(dependency)
                .unwrap(),
        ],
        vec![PackageDependencyBinding::new(outer, "kit", inner)],
    )
    .unwrap();
    let importer = root.join("main.omg");
    let select = |owner, base: &Path, name, foreign| {
        ReconciledPackageImportRequest::file(owner, base, name, &importer, foreign)
            .exact_file(&packages)
    };
    assert_eq!(
        select(inner, &vendor, "public.omg", true).unwrap(),
        Some(vendor.join("types.omg"))
    );
    assert!(
        select(outer, &root, "borrowed.omg", false).unwrap_err()[0]
            .message
            .contains("belongs to a different reconciled package")
    );
    assert!(
        select(inner, &vendor, "setup.omg", true).unwrap_err()[0]
            .message
            .contains("may not load dependency build file")
    );
}

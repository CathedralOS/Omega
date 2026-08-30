use omega_compiler::compile_to_checked_with_packages;
use omega_package_manager::resolution::graph::{
    PackageSourceClosureLimits, resolve_external_local_package_closure_with_storage,
};
use omega_package_manager::review::package_compilation_inputs;
use omega_package_source::{ExternalSourceContext, LocalSourceLimits, SourceResolverStorage};
use psi_source::SourceOrigin;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

struct TempTree(PathBuf);

impl TempTree {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "omega-standard-library-package-resolution-{}-{}",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).expect("create standard-library package test tree");
        Self(path)
    }

    fn package(&self, name: &str) -> PathBuf {
        let path = self.0.join(name);
        fs::create_dir(&path).expect("create test package");
        path
    }
}

impl Drop for TempTree {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn repository_standard_library() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .map(|ancestor| ancestor.join("source/library/std"))
        .find(|candidate| candidate.join("build.omg").is_file())
        .expect("repository source/library/std package")
        .canonicalize()
        .expect("canonical standard-library package root")
}

fn omega_path(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
}

fn write_consumer(root: &Path, standard_library: Option<&Path>) {
    let dependency = standard_library.map_or_else(String::new, |standard_library| {
        format!(
            "    builder.depend(Source::Path {{ location: \"{}\" }});\n",
            omega_path(standard_library)
        )
    });
    fs::write(
        root.join("build.omg"),
        format!(
            r#"target windows_x64 {{ }}

machine build(builder: &mut Build) {{
    builder.package("standard-library-consumer");
{dependency}}}
"#
        ),
    )
    .expect("write consumer package declaration");
    fs::write(
        root.join("main.omg"),
        r#"use omega_language_std::wire;

pub data StandardCarrier {
    behavior: UnknownMemberBehavior;
}
"#,
    )
    .expect("write consumer source");
}

#[test]
fn real_standard_library_resolves_as_an_ordinary_exact_package() {
    let tree = TempTree::new();
    let live_root = tree.package("consumer");
    let live_standard_library = repository_standard_library();
    write_consumer(&live_root, Some(&live_standard_library));

    let storage = SourceResolverStorage::for_hardened_base(tree.0.join("resolved"))
        .expect("create resolver storage");
    let closure = resolve_external_local_package_closure_with_storage(
        &live_root,
        ExternalSourceContext::derive(b"ordinary-standard-library-canary"),
        omega_target::TargetProfile::CrossPlatformCli,
        &storage,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("the repository standard library should resolve as a path dependency");

    assert_eq!(closure.graph().packages().len(), 2);
    let root_node = closure
        .graph()
        .package(closure.graph().root())
        .expect("root package graph node");
    let [standard_library_edge] = root_node.dependencies() else {
        panic!("root should have exactly one standard-library dependency")
    };
    assert_eq!(standard_library_edge.alias().as_str(), "omega_language_std");
    assert_eq!(
        standard_library_edge.target().name().as_str(),
        "omega-language-std"
    );

    let standard_library_custody = closure
        .custody(standard_library_edge.target())
        .expect("standard-library source custody");
    let standard_library_snapshot = standard_library_custody.snapshot_root();
    assert_ne!(standard_library_snapshot, live_standard_library);
    assert!(!standard_library_snapshot.starts_with(&live_standard_library));
    assert!(standard_library_snapshot.join("wire.omg").is_file());

    let inputs = package_compilation_inputs(&closure).expect("compiler package handoff");
    let root_identity = inputs.root();
    let standard_library_identity = standard_library_edge.target().identity();
    assert_eq!(
        inputs.package_name(standard_library_identity),
        Some("omega-language-std")
    );
    assert_eq!(
        inputs.package_root(standard_library_identity),
        Some(standard_library_snapshot)
    );
    assert!(inputs.dependencies().any(|(requester, alias, target)| {
        requester == root_identity
            && alias == "omega_language_std"
            && target == standard_library_identity
    }));

    let root_snapshot = closure
        .source_root(closure.graph().root())
        .expect("root source custody");
    let checked = compile_to_checked_with_packages(
        &root_snapshot.join("main.omg"),
        Some("windows_x64"),
        inputs,
    )
    .expect("the reconciled standard-library import should compile");

    let imported_type = checked
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "UnknownMemberBehavior")
        .expect("imported standard-library data declaration");
    assert_eq!(
        checked
            .symbols
            .symbol_package_identity(imported_type.symbol),
        Some(standard_library_identity)
    );

    let imported_source = checked
        .symbols
        .source_files()
        .find(|source| source.path == standard_library_snapshot.join("wire.omg"))
        .expect("compiler-retained standard-library source");
    assert_eq!(imported_source.package_root, standard_library_snapshot);
    assert_eq!(
        imported_source.package_identity,
        Some(standard_library_identity)
    );
    assert_eq!(imported_source.origin, SourceOrigin::User);
}

#[test]
fn standard_library_alias_has_no_undeclared_bundled_fallback() {
    let tree = TempTree::new();
    let live_root = tree.package("missing-edge-consumer");
    write_consumer(&live_root, None);

    let storage = SourceResolverStorage::for_hardened_base(tree.0.join("missing-edge"))
        .expect("create resolver storage");
    let closure = resolve_external_local_package_closure_with_storage(
        &live_root,
        ExternalSourceContext::derive(b"missing-standard-library-edge-canary"),
        omega_target::TargetProfile::CrossPlatformCli,
        &storage,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("the root-only package should resolve");
    let inputs = package_compilation_inputs(&closure).expect("root-only compiler handoff");
    let root_snapshot = closure
        .source_root(closure.graph().root())
        .expect("root source custody");

    let diagnostics = compile_to_checked_with_packages(
        &root_snapshot.join("main.omg"),
        Some("windows_x64"),
        inputs,
    )
    .expect_err("an undeclared standard-library alias must not use bundled std");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("omega_language_std")
                && (diagnostic.message.contains("dependency")
                    || diagnostic.message.contains("resolve"))
        }),
        "unexpected diagnostics: {diagnostics:#?}"
    );
}

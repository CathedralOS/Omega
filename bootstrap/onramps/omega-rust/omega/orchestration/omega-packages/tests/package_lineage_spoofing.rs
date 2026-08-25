use omega_compiler::PackageReviewCanonicalRowKind;
use omega_packages::{
    ExternalSourceContext, LocalSourceLimits, PackageSourceClosureLimits,
    compile_resolved_package_reviews, resolve_external_local_package_closure,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

struct TempTree(PathBuf);

impl TempTree {
    fn new() -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "omega-package-lineage-spoofing-{}-{stamp}",
            std::process::id()
        ));
        fs::create_dir(&path).expect("create lineage-spoofing test tree");
        Self(path)
    }

    fn package(&self, directory: &str) -> PathBuf {
        let path = self.0.join(directory);
        fs::create_dir(&path).expect("create test package");
        path
    }

    fn cache(&self) -> PathBuf {
        self.0.join("cache")
    }

    fn compiler_workspace(&self) -> PathBuf {
        self.0.join("compiler-workspace")
    }
}

impl Drop for TempTree {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn omega_path(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
}

fn write_provider_package(root: &Path) {
    fs::write(
        root.join("build.omg"),
        r#"const PACKAGE: Package = Package {
    name: "shared-provider"
};

target windows_x64 { }

machine build(builder: &mut Build) {
}
"#,
    )
    .expect("write provider package declaration");
    fs::write(
        root.join("provider.omg"),
        r#"pub boundary trait Pair {
    machine first();
}

pub data Provider {
}

machine Provider::first()
satisfies Pair::first
via Binding::VtableSlot(1);
"#,
    )
    .expect("write provider package source");
    fs::write(
        root.join("main.omg"),
        "pub machine identity() -> u64 { 1 }\n",
    )
    .expect("write provider package root source");
}

#[test]
fn same_name_and_symbols_from_another_lineage_cannot_spoof_selected_provider() {
    let tree = TempTree::new();
    let root = tree.package("root");
    let selected = tree.package("selected-lineage");
    let lookalike = tree.package("lookalike-lineage");
    write_provider_package(&selected);
    write_provider_package(&lookalike);

    fs::write(
        root.join("build.omg"),
        format!(
            r#"const PACKAGE: Package = Package {{
    name: "lineage-probe"
}};

target windows_x64 {{ }}

machine build(builder: &mut Build) {{
    builder.depend_as("selected_provider", Source::Path {{
        location: "{}"
    }});
    builder.depend_as("lookalike_provider", Source::Path {{
        location: "{}"
    }});
    builder.select_provider<Pair, Provider>();
}}
"#,
            omega_path(&selected),
            omega_path(&lookalike),
        ),
    )
    .expect("write root package declaration");
    fs::write(
        root.join("main.omg"),
        "use selected_provider::provider;\npub machine run() -> u64 { 70 }\n",
    )
    .expect("write root package source");

    let closure = resolve_external_local_package_closure(
        &root,
        ExternalSourceContext::derive(b"same-name-different-lineage-fixture"),
        tree.cache(),
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("same-name dependencies with distinct lineage should reconcile");

    assert_eq!(closure.graph().packages().len(), 3);
    let shared = closure
        .graph()
        .packages()
        .iter()
        .map(|node| node.source().key())
        .filter(|key| key.name().as_str() == "shared-provider")
        .collect::<Vec<_>>();
    let [first, second] = shared.as_slice() else {
        panic!("both independently custodied same-name packages must remain in the graph")
    };
    assert_ne!(first, second);
    assert_ne!(first.source_lineage(), second.source_lineage());
    assert_ne!(first.identity(), second.identity());

    let root_node = closure
        .graph()
        .package(closure.graph().root())
        .expect("root graph node");
    let selected_key = root_node
        .dependencies()
        .iter()
        .find(|dependency| dependency.alias().as_str() == "selected_provider")
        .expect("selected provider dependency")
        .target();
    let lookalike_key = root_node
        .dependencies()
        .iter()
        .find(|dependency| dependency.alias().as_str() == "lookalike_provider")
        .expect("lookalike provider dependency")
        .target();
    assert_ne!(selected_key, lookalike_key);

    let reviews =
        compile_resolved_package_reviews(&closure, "windows_x64", &tree.compiler_workspace())
            .expect("compiler review should preserve exact package lineage");
    assert_eq!(reviews.reviews().len(), 3);
    for node in closure.graph().packages() {
        let review = reviews
            .review(node.source().key())
            .expect("every exact package key receives its own compiler review");
        assert_eq!(
            review.projection().package(),
            node.source().key().identity()
        );
    }

    let root_review = reviews
        .review(closure.graph().root())
        .expect("root compiler review");
    let [provider] = root_review.projection().selected_providers() else {
        panic!("root must retain exactly one selected provider")
    };
    assert_eq!(provider.realizing_package(), Some(selected_key.identity()));
    assert_eq!(
        provider.provider_type_package(),
        Some(selected_key.identity())
    );
    assert_eq!(
        provider.schema().trait_package_identity,
        Some(selected_key.identity())
    );
    assert_ne!(provider.realizing_package(), Some(lookalike_key.identity()));
    assert!(
        root_review
            .canonical_rows()
            .iter()
            .any(|row| row.kind() == PackageReviewCanonicalRowKind::SelectedProviderSet),
        "the exact selected-provider identity must cross the canonical review boundary"
    );
}

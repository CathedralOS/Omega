use super::ImmutableSourceParseCheckpoint;
use artifacts::compile_timings::CompileTimings;
use source::DependencyScope;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

struct SourceFixture(PathBuf);

impl SourceFixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "omega-source-assembly-{}-{}",
            std::process::id(),
            NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed),
        ));
        std::fs::create_dir(&root).expect("create source fixture");
        Self(root.canonicalize().expect("canonical source fixture"))
    }

    fn write(&self, name: &str, source: &str) {
        std::fs::write(self.0.join(name), source).expect("write source fixture");
    }
}

impl Drop for SourceFixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn transitive_imports_retain_distinct_build_and_product_instances() {
    let fixture = SourceFixture::new();
    fixture.write("main.omg", "use shared;\ndata Main {}\n");
    fixture.write(
        "build.omg",
        "use shared;\nmachine build(builder: &mut Build) { Build::application(builder, \"assembly\"); }\n",
    );
    fixture.write("shared.omg", "use leaf;\ndata Shared {}\n");
    fixture.write("leaf.omg", "data Leaf {}\n");
    let mut timings = CompileTimings::default();
    let checkpoint =
        ImmutableSourceParseCheckpoint::prepare(&fixture.0.join("main.omg"), None, &mut timings)
            .expect("prepare transitive source frontier");
    let retained = checkpoint.clone();
    let (_, assembled) = checkpoint
        .assemble(None, &mut timings)
        .expect("assemble source without a Build evaluator");
    let (_, reused) = retained
        .assemble(None, &mut timings)
        .expect("consume the final shared checkpoint");

    let coordinates = |syntax: &super::AssembledSyntax| {
        syntax
            .sources
            .files()
            .map(|file| {
                (
                    file.source_id,
                    file.path.clone(),
                    file.dependency_scope,
                    file.source.clone(),
                )
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(coordinates(&assembled), coordinates(&reused));
    for name in ["shared.omg", "leaf.omg"] {
        let instances = assembled
            .sources
            .files()
            .filter(|file| file.path == fixture.0.join(name))
            .collect::<Vec<_>>();
        assert_eq!(instances.len(), 2, "{name} has one instance per scope");
        assert_ne!(instances[0].source_id, instances[1].source_id);
        for scope in [DependencyScope::Build, DependencyScope::Product] {
            let instance = instances
                .iter()
                .find(|file| file.dependency_scope == scope)
                .expect("retain both dependency scopes");
            assert_eq!(
                assembled.build_scope_sources.contains(&instance.source_id),
                scope == DependencyScope::Build,
            );
        }
    }
    let build = assembled
        .sources
        .get(assembled.build_source_id.expect("selected Build source"))
        .expect("retain selected Build instance");
    assert_eq!(build.path, fixture.0.join("build.omg"));
    assert_eq!(build.dependency_scope, DependencyScope::Build);
    assert!(assembled.application.is_some());
    assert!(!assembled.source_scoped_top_level_bindings.is_empty());
}

#[test]
fn checkpoint_rejects_a_different_package_source_projection() {
    let fixture = SourceFixture::new();
    fixture.write("main.omg", "data Main {}\n");
    let mut timings = CompileTimings::default();
    let checkpoint =
        ImmutableSourceParseCheckpoint::prepare(&fixture.0.join("main.omg"), None, &mut timings)
            .expect("prepare standalone source frontier");
    let identity = semantic_vocabulary::PackageKeyIdentity::from_digest([7; 32])
        .expect("nonzero package identity");
    let inputs = crate::PackageCompilationInputs::new_package(
        identity,
        vec![crate::PackageSourceBinding::new(
            identity,
            "root",
            fixture.0.clone(),
        )],
        vec![],
    )
    .expect("valid package graph");
    let diagnostics = match checkpoint.assemble(Some(&inputs), &mut timings) {
        Ok(_) => panic!("a standalone checkpoint must not acquire package authority"),
        Err(diagnostics) => diagnostics,
    };
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("package source inputs do not match the immutable source checkpoint")
    }));
}

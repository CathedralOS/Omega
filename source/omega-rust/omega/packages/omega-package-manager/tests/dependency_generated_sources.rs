use omega_package_manager::resolution::{
    PackageSourceClosureLimits, ResolvePackageSourceError, ResolveWorkspacePackageClosureError,
    ResolvedPackageSourceClosure, resolve_workspace_package_closure_with_storage,
};
use omega_package_manager::review::compile_resolved_package_reviews;
use omega_package_source::{
    LocalSourceLimits, SourceLineage, SourceRelativePath, SourceResolverStorage,
};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(5)
        .expect("omega-package-manager should live under the Omega workspace")
        .to_path_buf()
}

fn temporary_root() -> PathBuf {
    std::env::temp_dir().join(format!(
        "omega-dependency-generated-source-{}-{}",
        std::process::id(),
        TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed),
    ))
}

fn resolve_workspace_package_closure(
    workspace_root_source: &SourceLineage,
    root_member_path: SourceRelativePath,
    live_workspace_root: impl AsRef<Path>,
    cache_dir: impl AsRef<Path>,
    source_limits: LocalSourceLimits,
    closure_limits: PackageSourceClosureLimits,
) -> Result<ResolvedPackageSourceClosure, ResolveWorkspacePackageClosureError> {
    let storage = SourceResolverStorage::for_hardened_base(cache_dir).map_err(|error| {
        ResolveWorkspacePackageClosureError::Root(ResolvePackageSourceError::Source(error))
    })?;
    resolve_workspace_package_closure_with_storage(
        workspace_root_source,
        root_member_path,
        live_workspace_root,
        omega_target::TargetProfile::CrossPlatformCli,
        &storage,
        source_limits,
        closure_limits,
    )
}

#[test]
#[ignore = "OPTIONAL-STDLIB-BUILD-PROTOCOL-AND-SEMANTIC-BINDINGS: generated-table must use compiler-owned Build facets"]
fn dependency_generated_source_enters_consumer_without_rerunning_the_dependency_build() {
    let temporary = temporary_root();
    let fixtures = workspace_root().join("tests/fixtures/packages");
    let lineage = SourceLineage::git("https://github.com/CathedralOS/Omega.git").unwrap();
    let closure = resolve_workspace_package_closure(
        &lineage,
        SourceRelativePath::parse("generated-consumer").unwrap(),
        &fixtures,
        temporary.join("cache"),
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve generated producer and consumer");

    let reviews = compile_resolved_package_reviews(
        &closure,
        "windows_x64",
        &temporary.join("compiler-build"),
    )
    .expect("dependency generated source should enter consumer compilation");

    let producer = reviews
        .reviews()
        .iter()
        .find(|review| review.key().name().as_str() == "generated-table")
        .expect("producer review");
    let consumer = reviews
        .reviews()
        .iter()
        .find(|review| review.key().name().as_str() == "generated-consumer")
        .expect("consumer review");
    let [generated] = producer.generated_source_bundle().sources() else {
        panic!("producer should retain exactly one generated source")
    };
    assert_eq!(generated.relative_path(), b"table.generated.omg");
    assert_eq!(
        generated.bytes(),
        b"pub machine table_size() -> u64 {\n    3\n}\n"
    );
    assert!(consumer.generated_source_bundle().sources().is_empty());
    assert_eq!(
        producer
            .build_observation_summary()
            .expect("producer build observations")
            .filesystem_operation_attempts()
            .len(),
        6,
        "dependency build must execute only in its own checked run",
    );
    assert_eq!(
        consumer
            .build_observation_summary()
            .expect("consumer build observations")
            .filesystem_operation_attempts()
            .len(),
        0,
        "consumer compilation must inject retained bytes without rerunning dependency build",
    );
    assert!(
        consumer
            .projection()
            .callables()
            .iter()
            .any(|callable| { callable.identity().path() == "consume_generated_table" })
    );

    let _ = std::fs::remove_dir_all(temporary);
}

use omega_packages::{
    LocalSourceLimits, PackageSourceClosureLimits, SourceLineage, WorkspaceMemberPath,
    compile_resolved_package_reviews, resolve_workspace_package_closure,
};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(6)
        .expect("omega-packages should live under the Omega workspace")
        .to_path_buf()
}

fn temporary_root() -> PathBuf {
    std::env::temp_dir().join(format!(
        "omega-dependency-generated-source-{}-{}",
        std::process::id(),
        TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed),
    ))
}

#[test]
#[ignore = "OWNER Q7: generated-table's filesystem service needs authenticated ordinary-package authority after std relocation"]
fn dependency_generated_source_enters_consumer_without_rerunning_the_dependency_build() {
    let temporary = temporary_root();
    let fixtures = workspace_root().join("tests/fixtures/packages");
    let lineage = SourceLineage::git("https://github.com/CathedralOS/Omega.git").unwrap();
    let closure = resolve_workspace_package_closure(
        &lineage,
        WorkspaceMemberPath::parse("generated-consumer").unwrap(),
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

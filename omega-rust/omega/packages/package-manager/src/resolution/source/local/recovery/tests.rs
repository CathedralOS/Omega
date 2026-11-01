//! Historical metadata comes from the accepted subject; bytes still come from
//! the real resolver's verified retained snapshot. These are custody tests,
//! not substitutes for executing the Build declaration through the CLI.

use super::*;
use crate::declarations::dependencies::read::DependencySourceRequest;
use package_source::{ExternalSourceContext, LocalSourceLimits, PrimaryGitChoices};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

struct HistoricalSource {
    live: PathBuf,
    snapshot: PathBuf,
    content: SourceContentDigest,
    current: PackageSourceCustody,
    storage: SourceResolver,
}

impl HistoricalSource {
    fn capture() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let live = std::env::temp_dir().join(format!(
            "omega-recorded-build-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed),
        ));
        std::fs::create_dir(&live).unwrap();
        std::fs::write(
            live.join("build.omg"),
            "machine build(builder: &mut Build) { configure(builder, \"recorded\"); }\n\
             machine configure(builder: &mut Build, name: &[u8]) {\n\
                 Build::application(builder, name);\n\
             }\n",
        )
        .unwrap();
        std::fs::write(live.join("value.omg"), "const VALUE: u32 = 17;\n").unwrap();
        let storage = SourceResolver::for_current_user(PrimaryGitChoices::default()).unwrap();
        let source = package_source::local::operations::resolve_local_source_snapshot_in_lane(
            &live,
            storage.external_local_sources(),
            LocalSourceLimits::default(),
        )
        .unwrap();
        let snapshot = source.snapshot_root().to_path_buf();
        let content = SourceContentDigest::derive(source.normalized().content_identity.as_bytes());
        // The candidate deliberately has a different role and empty ledgers.
        // Neither may replace the old accepted metadata during comparison.
        std::fs::write(
            live.join("build.omg"),
            "machine build(builder: &mut Build) { Build::package(builder, \"recorded\"); }\n",
        )
        .unwrap();
        let current = crate::resolution::source::resolve_external_local_project_source(
            &live,
            &storage,
            LocalSourceLimits::default(),
            ExternalSourceContext::derive(b"recorded-build-recovery-test"),
        )
        .unwrap()
        .into_custody();
        Self {
            live,
            snapshot,
            content,
            current,
            storage,
        }
    }
}

impl Drop for HistoricalSource {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.live).unwrap();
    }
}

fn recorded_dependencies() -> DependencyProjections {
    let requests = |locations: &[&str]| {
        locations
            .iter()
            .map(|location| DependencySourceRequest::Path {
                explicit_alias: None,
                location: (*location).to_owned(),
            })
            .collect::<Vec<_>>()
            .into()
    };
    DependencyProjections::new(
        requests(&["product_first", "product_second"]),
        requests(&["build_only"]),
    )
}

#[test]
fn cached_helper_declaration_retains_recorded_role_and_both_ordered_ledgers() {
    let source = HistoricalSource::capture();
    let dependencies = recorded_dependencies();
    let recovered = recover_cached_external_local_source(
        &source.current,
        &source.content,
        BuildDeclarationKind::Application,
        &dependencies,
        &source.storage,
    )
    .unwrap()
    .expect("the real historical snapshot is available");
    assert_eq!(recovered.key(), source.current.key());
    assert_eq!(recovered.role(), BuildDeclarationKind::Application);
    assert_eq!(recovered.dependency_projections(), &dependencies);
    assert_eq!(recovered.snapshot_root(), source.snapshot);
    assert_eq!(
        recovered.resolution(),
        &ImmutableSourceResolution::external_local(source.content.clone())
    );
    assert_ne!(recovered.role(), source.current.role());
    assert_ne!(
        recovered.dependency_projections(),
        source.current.dependency_projections()
    );
}

#[test]
fn recorded_metadata_does_not_authorize_tampered_historical_bytes() {
    let source = HistoricalSource::capture();
    let file = source.snapshot.join("value.omg");
    let original = std::fs::metadata(&file).unwrap().permissions();
    let mut writable = original.clone();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        writable.set_mode(0o600);
    }
    #[cfg(not(unix))]
    writable.set_readonly(false);
    std::fs::set_permissions(&file, writable).unwrap();
    std::fs::write(&file, "const VALUE: u32 = 29;\n").unwrap();
    std::fs::set_permissions(&file, original).unwrap();
    assert!(
        recover_cached_external_local_source(
            &source.current,
            &source.content,
            BuildDeclarationKind::Application,
            &recorded_dependencies(),
            &source.storage,
        )
        .is_err()
    );
}

use super::{make_tree_owner_writable, temp_root, write_package};
use crate::declarations::BuildDeclarationKind;
use crate::resolution::source::{
    ResolvePackageSourceError, bind_staged_external_local_project_source,
    resolve_external_local_project_source_with_storage,
};
use omega_package_source::local::staging::{
    StagedLocalSnapshot, stage_local_source_replacement_in_lane,
};
use omega_package_source::{
    ExternalSourceContext, LocalSourceLimits, SourceContentDigest, SourceRelativePath,
    SourceResolveError, SourceResolverStorage,
};
use sha2::{Digest, Sha256};
use std::path::Path;

const PROPOSED: &[u8] = b"machine build(builder: &mut Build) {\n    builder.application(\"staged-root\");\n    builder.depend(Source::Path { location: \"../dependency\" });\n}\n";

fn with_stage(name: &str, test: impl FnOnce(&Path, &SourceResolverStorage, &StagedLocalSnapshot)) {
    let root = temp_root(name);
    let cache = temp_root(&format!("{name}-cache"));
    write_package(&root, "staged-root");
    let storage = SourceResolverStorage::for_hardened_base(&cache).expect("retain storage");
    let original = std::fs::read(root.join("build.omg")).expect("read original build");
    let stage = stage_local_source_replacement_in_lane(
        &root.join("."),
        &SourceRelativePath::parse("build.omg").unwrap(),
        &Sha256::digest(&original).into(),
        PROPOSED,
        storage.external_local_sources(),
        LocalSourceLimits::default(),
    )
    .expect("stage proposed build");
    test(&root, &storage, &stage);
    drop(storage);
    make_tree_owner_writable(&cache);
    std::fs::remove_dir_all(root).expect("remove source fixture");
    std::fs::remove_dir_all(cache).expect("remove snapshot fixture");
}

#[test]
fn staged_binding_uses_proposed_declaration_and_original_lineage() {
    with_stage("staged-binding-lineage", |root, storage, stage| {
        let context = ExternalSourceContext::derive(b"staged-binding-context");
        let ordinary = resolve_external_local_project_source_with_storage(
            stage.requested_root(),
            storage,
            LocalSourceLimits::default(),
            context.clone(),
        )
        .expect("bind original project");
        let proposed =
            bind_staged_external_local_project_source(stage, LocalSourceLimits::default(), context)
                .expect("bind proposed project");

        assert_eq!(proposed.key(), ordinary.key());
        assert_eq!(proposed.role(), BuildDeclarationKind::Application);
        assert_eq!(ordinary.role(), BuildDeclarationKind::Package);
        assert_eq!(proposed.dependency_requests().len(), 1);
        assert!(ordinary.dependency_requests().is_empty());
        assert_ne!(proposed.resolution(), ordinary.resolution());
        assert_eq!(proposed.snapshot_root(), stage.snapshot_root());
        assert_eq!(
            proposed.materialization().content(),
            &SourceContentDigest::derive(stage.normalized().content_identity.as_bytes())
        );
        assert_eq!(
            proposed.materialization().byte_count(),
            stage.normalized().byte_count
        );
        assert!(std::ptr::eq(*proposed.source(), stage));
        assert_ne!(std::fs::read(root.join("build.omg")).unwrap(), PROPOSED);
        stage
            .verify_live_source_unchanged()
            .expect("original unchanged");

        let other_context = bind_staged_external_local_project_source(
            stage,
            LocalSourceLimits::default(),
            ExternalSourceContext::derive(b"other-staged-binding-context"),
        )
        .expect("bind another context");
        assert_ne!(other_context.key(), proposed.key());
        assert_eq!(other_context.resolution(), proposed.resolution());
    });
}

#[test]
fn staged_binding_rejects_changed_live_source() {
    with_stage("staged-binding-stale", |root, _, stage| {
        std::fs::write(root.join("main.omg"), "machine changed() {}\n").unwrap();
        assert!(matches!(
            bind_staged_external_local_project_source(
                stage,
                LocalSourceLimits::default(),
                ExternalSourceContext::derive(b"stale-staged-binding"),
            ),
            Err(ResolvePackageSourceError::Source(
                SourceResolveError::LocalSourceChanged { .. }
            ))
        ));
    });
}

#[test]
fn staged_binding_verifies_snapshot_under_caller_limits() {
    with_stage("staged-binding-limits", |_, _, stage| {
        assert!(matches!(
            bind_staged_external_local_project_source(
                stage,
                LocalSourceLimits {
                    max_bytes: 1,
                    ..LocalSourceLimits::default()
                },
                ExternalSourceContext::derive(b"bounded-staged-binding"),
            ),
            Err(ResolvePackageSourceError::Source(
                SourceResolveError::TooManyBytes { limit: 1 }
            ))
        ));
    });
}

#[cfg(unix)]
#[test]
fn staged_binding_rejects_mutated_snapshot_before_parsing() {
    use std::os::unix::fs::PermissionsExt;

    with_stage("staged-binding-mutated", |_, _, stage| {
        let build = stage.snapshot_root().join("build.omg");
        let permissions = std::fs::metadata(&build).unwrap().permissions();
        std::fs::set_permissions(&build, std::fs::Permissions::from_mode(0o600)).unwrap();
        std::fs::write(&build, b"invalid proposed declaration").unwrap();
        std::fs::set_permissions(&build, permissions).unwrap();
        stage
            .verify_live_source_unchanged()
            .expect("live tree still unchanged");
        assert!(matches!(
            bind_staged_external_local_project_source(
                stage,
                LocalSourceLimits::default(),
                ExternalSourceContext::derive(b"mutated-staged-binding"),
            ),
            Err(ResolvePackageSourceError::Source(
                SourceResolveError::SourceSnapshotContentMismatch { .. }
            ))
        ));
    });
}

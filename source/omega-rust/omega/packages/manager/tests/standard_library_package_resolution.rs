use omega_compiler::compile_to_checked_with_packages;
use omega_effects::{PortableFilesystemAuthorityFacet, ServiceTerminalAuthorityPermission};
use omega_package_compilation::AcceptedSemanticBindingRole;
use omega_package_evidence::record::{
    PackageReviewDangerousAuthorityClass, PackageReviewNominalOwner,
};
use omega_package_manager::resolution::graph::{
    PackageSourceClosureLimits, resolve_external_local_package_closure_with_storage,
};
use omega_package_manager::resolution::package_compilation_inputs;
use omega_package_manager::review::{
    ConsumerScopedSemanticBindingReviewInput, compile_resolved_package_candidate_reviews,
    compile_resolved_package_reviews, compile_resolved_package_reviews_with_semantic_bindings,
};
use omega_package_source::{ExternalSourceContext, LocalSourceLimits, SourceResolverStorage};
use psi_source::SourceOrigin;
use std::collections::{BTreeMap, BTreeSet};
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
            r#"target windows_x86_64 {{ }}

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

fn write_filesystem_consumer(root: &Path, standard_library: &Path) {
    fs::write(
        root.join("build.omg"),
        format!(
            r#"target linux_x86_64 {{ }}

machine build(builder: &mut Build) {{
    builder.package("filesystem-policy-consumer");
    builder.depend(Source::Path {{ location: "{}" }});
}}
"#,
            omega_path(standard_library),
        ),
    )
    .expect("write filesystem consumer package declaration");
    fs::write(
        root.join("main.omg"),
        r#"use omega_language_std::filesystem_host;

pub machine inspect_host()
reaches FilesystemHost
{
}
"#,
    )
    .expect("write filesystem consumer source");
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
        Some("windows_x86_64"),
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
fn real_standard_library_has_a_complete_ordinary_review_entry() {
    let tree = TempTree::new();
    let standard_library = repository_standard_library();
    let storage = SourceResolverStorage::for_hardened_base(tree.0.join("review-resolved"))
        .expect("create standard-library review storage");
    let closure = resolve_external_local_package_closure_with_storage(
        &standard_library,
        ExternalSourceContext::derive(b"ordinary-standard-library-review-entry"),
        &storage,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve the standard library as an ordinary package root");

    let reviews = compile_resolved_package_candidate_reviews(
        &closure.for_exact_target(omega_target::TargetProfile::LinuxX64),
        &tree.0.join("review-build"),
    )
    .expect("compile the complete ordinary standard-library review entry");
    let review = reviews
        .review(closure.graph().root())
        .expect("standard-library root review");
    let console = review
        .projection()
        .public_traits()
        .iter()
        .find(|shape| shape.identity().path() == "Console")
        .expect("ordinary review entry reaches the public Console surface");
    assert_eq!(
        console.identity().owner(),
        PackageReviewNominalOwner::Package(closure.graph().root().identity()),
    );
    for boundary_trait in ["FilesystemHost", "TimeHost"] {
        assert!(
            review
                .projection()
                .public_traits()
                .iter()
                .any(|shape| shape.identity().path() == boundary_trait),
            "ordinary review entry must expose the dangerous {boundary_trait} capability",
        );
    }
    assert!(
        review
            .projection()
            .public_data()
            .iter()
            .any(|shape| shape.identity().path() == "UnknownMemberBehavior"),
        "ordinary review entry reaches the public wire surface",
    );
}

#[test]
fn real_filesystem_host_schema_accepts_settled_portable_facet_rows() {
    let tree = TempTree::new();
    let consumer = tree.package("filesystem-consumer");
    let standard_library = repository_standard_library();
    write_filesystem_consumer(&consumer, &standard_library);

    let storage = SourceResolverStorage::for_hardened_base(tree.0.join("filesystem-resolved"))
        .expect("create filesystem consumer resolver storage");
    let closure = resolve_external_local_package_closure_with_storage(
        &consumer,
        ExternalSourceContext::derive(b"real-filesystem-facet-policy"),
        &storage,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve consumer with the real standard library");
    let target = closure.for_exact_target(omega_target::TargetProfile::LinuxX64);
    let preliminary =
        compile_resolved_package_reviews(&target, &tree.0.join("filesystem-preliminary-build"))
            .expect("compile preliminary filesystem review");
    let root = closure.graph().root();
    let root_review = preliminary.review(root).expect("preliminary root review");
    let candidates = root_review
        .semantic_binding_candidates()
        .iter()
        .filter(|candidate| {
            candidate.binding().role() == AcceptedSemanticBindingRole::FilesystemHostService
        })
        .collect::<Vec<_>>();
    let [candidate] = candidates.as_slice() else {
        panic!(
            "real FilesystemHost reach must expose one exact review candidate, found {}",
            candidates.len()
        );
    };
    assert_eq!(
        candidate.binding().normalized_schema_digest(),
        candidate.service_schema().identity_digest(),
    );

    // This table is explicit test-consumer policy. Readable method names only
    // locate human-reviewed rows in the real checked schema; the emitted rows
    // are keyed by the schema digest and complete normalized requirement
    // identity, and production candidate discovery never runs this mapping.
    let specifications: &[(&str, &[PortableFilesystemAuthorityFacet])] = &[
        (
            "create",
            &[
                PortableFilesystemAuthorityFacet::ContentWrite,
                PortableFilesystemAuthorityFacet::NamespaceMutation,
                PortableFilesystemAuthorityFacet::MetadataMutation,
            ],
        ),
        ("open", &PortableFilesystemAuthorityFacet::ALL),
        ("open_create", &PortableFilesystemAuthorityFacet::ALL),
        ("read", &[PortableFilesystemAuthorityFacet::ContentRead]),
        ("write", &[PortableFilesystemAuthorityFacet::ContentWrite]),
        ("read_at", &[PortableFilesystemAuthorityFacet::ContentRead]),
        (
            "write_at",
            &[PortableFilesystemAuthorityFacet::ContentWrite],
        ),
        (
            "remove",
            &[PortableFilesystemAuthorityFacet::NamespaceMutation],
        ),
        (
            "create_dir",
            &[
                PortableFilesystemAuthorityFacet::NamespaceMutation,
                PortableFilesystemAuthorityFacet::MetadataMutation,
            ],
        ),
        (
            "remove_dir",
            &[PortableFilesystemAuthorityFacet::NamespaceMutation],
        ),
        (
            "create_dir_name",
            &[
                PortableFilesystemAuthorityFacet::NamespaceMutation,
                PortableFilesystemAuthorityFacet::MetadataMutation,
            ],
        ),
        ("open_at", &PortableFilesystemAuthorityFacet::ALL),
        (
            "unlink_at",
            &[PortableFilesystemAuthorityFacet::NamespaceMutation],
        ),
        (
            "set_permissions",
            &[PortableFilesystemAuthorityFacet::MetadataMutation],
        ),
        (
            "set_file_permissions",
            &[PortableFilesystemAuthorityFacet::MetadataMutation],
        ),
        (
            "rename",
            &[PortableFilesystemAuthorityFacet::NamespaceMutation],
        ),
        (
            "hard_link",
            &[PortableFilesystemAuthorityFacet::NamespaceMutation],
        ),
        (
            "symlink",
            &[PortableFilesystemAuthorityFacet::NamespaceMutation],
        ),
        (
            "canonicalize",
            &[PortableFilesystemAuthorityFacet::MetadataQuery],
        ),
        (
            "read_dir",
            &[PortableFilesystemAuthorityFacet::DirectoryEnumeration],
        ),
        (
            "find_first",
            &[PortableFilesystemAuthorityFacet::DirectoryEnumeration],
        ),
        (
            "find_next",
            &[PortableFilesystemAuthorityFacet::DirectoryEnumeration],
        ),
        (
            "create_hard_link",
            &[PortableFilesystemAuthorityFacet::NamespaceMutation],
        ),
        ("open_path_handle", &PortableFilesystemAuthorityFacet::ALL),
        (
            "final_path_name_by_handle",
            &[PortableFilesystemAuthorityFacet::MetadataQuery],
        ),
        (
            "set_file_time",
            &[PortableFilesystemAuthorityFacet::MetadataMutation],
        ),
        (
            "remove_name",
            &[PortableFilesystemAuthorityFacet::NamespaceMutation],
        ),
        (
            "remove_dir_name",
            &[PortableFilesystemAuthorityFacet::NamespaceMutation],
        ),
        (
            "read_metadata",
            &[PortableFilesystemAuthorityFacet::MetadataQuery],
        ),
        (
            "read_file_metadata",
            &[PortableFilesystemAuthorityFacet::MetadataQuery],
        ),
        (
            "read_symlink_metadata",
            &[PortableFilesystemAuthorityFacet::MetadataQuery],
        ),
        ("set_len", &[PortableFilesystemAuthorityFacet::ContentWrite]),
        (
            "set_file_times",
            &[PortableFilesystemAuthorityFacet::MetadataMutation],
        ),
        (
            "change_owner",
            &[PortableFilesystemAuthorityFacet::MetadataMutation],
        ),
        (
            "change_owner_no_follow",
            &[PortableFilesystemAuthorityFacet::MetadataMutation],
        ),
        (
            "change_file_owner",
            &[PortableFilesystemAuthorityFacet::MetadataMutation],
        ),
    ];
    let unresolved = [
        "close",
        "seek",
        "read_link",
        "find_close",
        "close_handle",
        "get_osfhandle",
        "lock_file_ex",
        "unlock_file",
        "get_last_error",
        "sync",
        "sync_data",
        "duplicate",
        "lock_file",
        "errno",
    ];
    let classified_names = specifications
        .iter()
        .map(|(name, _)| *name)
        .chain(unresolved)
        .collect::<BTreeSet<_>>();
    assert_eq!(
        classified_names.len(),
        candidate.service_schema().methods.len(),
        "the explicit and unresolved cohorts must partition the real schema",
    );
    assert!(
        candidate
            .service_schema()
            .methods
            .iter()
            .all(|method| classified_names.contains(method.name.as_str()))
    );
    let mut expected_permissions = BTreeMap::new();
    let permissions = specifications
        .iter()
        .map(|(name, facets)| {
            let methods = candidate
                .service_schema()
                .methods
                .iter()
                .filter(|method| method.name == *name)
                .collect::<Vec<_>>();
            let [method] = methods.as_slice() else {
                panic!(
                    "real FilesystemHost schema resolved `{name}` {} times",
                    methods.len()
                );
            };
            let permission = ServiceTerminalAuthorityPermission::for_filesystem_facets(
                candidate.service_schema().identity_digest(),
                method.requirement_identity.clone(),
                facets.iter().copied(),
            );
            assert!(
                expected_permissions
                    .insert(
                        permission.requirement_identity().to_owned(),
                        permission.permitted().classes().to_vec(),
                    )
                    .is_none()
            );
            permission
        })
        .collect::<Vec<_>>();
    let binding = candidate
        .binding()
        .clone()
        .with_terminal_authority_permissions(permissions)
        .expect("attach explicit facets to exact real FilesystemHost requirements");
    let final_reviews = compile_resolved_package_reviews_with_semantic_bindings(
        &target,
        &tree.0.join("filesystem-policy-build"),
        &[ConsumerScopedSemanticBindingReviewInput::new(
            root.clone(),
            binding,
        )],
    )
    .expect("recompile real FilesystemHost policy through the checked review route");
    let final_root = final_reviews.review(root).expect("final root review");
    let filesystem_authorities = final_root
        .projection()
        .dangerous_authorities()
        .iter()
        .filter(|authority| authority.class() == PackageReviewDangerousAuthorityClass::Filesystem)
        .collect::<Vec<_>>();
    let [broad] = filesystem_authorities.as_slice() else {
        panic!(
            "final review must retain one transitional broad filesystem row, found {}",
            filesystem_authorities.len()
        );
    };
    assert_eq!(
        final_root
            .projection()
            .terminal_authority_permissions()
            .len(),
        specifications.len(),
    );
    for permission in final_root.projection().terminal_authority_permissions() {
        assert_eq!(permission.service(), broad.service());
        assert_eq!(
            permission.service_schema(),
            candidate.service_schema().identity_digest(),
        );
        assert_eq!(
            permission.permitted().classes(),
            expected_permissions
                .remove(permission.requirement_identity())
                .expect("final row retains one exact authored requirement"),
        );
    }
    assert!(expected_permissions.is_empty());
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
        Some("windows_x86_64"),
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

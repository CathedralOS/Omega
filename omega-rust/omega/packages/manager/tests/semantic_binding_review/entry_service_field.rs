use super::{
    AcceptedSemanticBindingRole, ConsumerScopedSemanticBindingReviewInput, ExternalSourceContext,
    GitResolutionOptions, LocalSourceLimits, PackageSourceClosureLimits, Path, PrimaryGitChoices,
    SemanticBindingReview, SourceResolverStorage, TemporaryTree,
    compile_resolved_package_candidate_for_production, compile_resolved_package_reviews,
    resolve_external_local_project_closure, write_file,
};

// A ProgramEntry is root-bound rather than a package callable, so a
// consumer's entry `Service` field cannot nominate through callable reach
// rows: the consumer's own review must surface the field requirement as the
// candidate for the fused boundary's service binding.
#[test]
fn entry_service_field_nominates_filesystem_host_binding() {
    let temporary = TemporaryTree::new();
    let application = temporary.package("application");
    let standard_library = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .expect("repository root")
        .join("source/library/std");
    write_file(
        application.join("main.omg"),
        r#"use omega_language_std::filesystem_host;
use omega::language::core::binding;

data Main { raw_fs: Binding<FilesystemHost>; }

machine Main::main(&mut self)
reaches FilesystemHost
invokes FilesystemHost;
{
    let fd: i32 = self.raw_fs.open("/tmp/omega-entry-service-field.control", 0);
    let rc: i32 = self.raw_fs.close(fd);
}
"#,
    );
    write_file(
        application.join("build.omg"),
        &format!(
            "machine build(builder: &mut Build) {{ builder.application(\"fs_consumer\"); builder.depend_as(\"omega_language_std\", Source::Path {{ location: {:?} }}); builder.roots.bind(linux_x86_64::ProgramEntry, Main::main); }}\n",
            standard_library.to_str().expect("fixture source path")
        ),
    );
    let storage = SourceResolverStorage::for_hardened_base(
        temporary.0.join("resolved"),
        PrimaryGitChoices::default(),
    )
    .expect("create semantic-binding resolver storage");
    let closure = resolve_external_local_project_closure(
        &application,
        ExternalSourceContext::derive(b"entry-service-fused-binding"),
        &storage,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
        GitResolutionOptions::default(),
    )
    .expect("resolve fs-consumer closure");
    let target = closure.for_exact_target(target::TargetProfile::LinuxX64);
    let candidate = compile_resolved_package_candidate_for_production(
        &target,
        &temporary.0.join("review"),
        SemanticBindingReview::Discover,
        None,
    )
    .expect("consumer review nominates from the entry Binding field requirement");
    let review = candidate.reviews().review(closure.graph().root()).unwrap();
    let entry = review
        .semantic_bindings()
        .iter()
        .find(|binding| binding.role() == AcceptedSemanticBindingRole::FilesystemHostService)
        .expect("entry Binding<FilesystemHost> field nominates the service binding");
    assert_eq!(entry.declaration_path(), "FilesystemHost");
    assert_ne!(entry.package(), closure.graph().root().identity());
    // Accepting the proposed decisions replays every discovered binding: the
    // nominated service binding settles the field, and the physical entry
    // contract carries its own program-entry binding.
    let accepted = review
        .semantic_bindings()
        .iter()
        .map(|binding| {
            ConsumerScopedSemanticBindingReviewInput::new(
                closure.graph().root().clone(),
                review.checked_context(),
                binding.clone(),
            )
        })
        .collect::<Vec<_>>();
    let strict = compile_resolved_package_reviews(
        &target,
        &temporary.0.join("strict-bound"),
        SemanticBindingReview::Explicit(&accepted),
    )
    .expect("accepted binding settles the entry Binding field without discovery");
    assert_eq!(
        strict
            .review(closure.graph().root())
            .unwrap()
            .semantic_bindings(),
        review.semantic_bindings()
    );
}

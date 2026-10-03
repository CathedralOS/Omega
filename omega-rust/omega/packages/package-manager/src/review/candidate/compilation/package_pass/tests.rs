use super::{OccurrenceCompile, TargetEntryDiscovery};
use crate::resolution::graph::GitResolutionOptions;
use crate::resolution::graph::{
    PackageSourceClosureLimits, resolve_external_local_project_closure,
};
use package_source::{ExternalSourceContext, LocalSourceLimits, PrimaryGitChoices, SourceResolver};
use std::path::PathBuf;

struct EntryConsumer(PathBuf);

impl EntryConsumer {
    fn new() -> Self {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "omega-entry-candidate-carry-{}-{stamp}",
            std::process::id()
        ));
        let standard_library = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .map(|ancestor| ancestor.join("source/library/std"))
            .find(|candidate| candidate.join("build.omg").is_file())
            .expect("bundled standard library")
            .canonicalize()
            .unwrap();
        std::fs::create_dir_all(root.join("application")).unwrap();
        std::fs::write(
            root.join("application/build.omg"),
            format!(
                r#"machine build(builder: &mut Build) {{
    Build::application(builder, "entry_candidate_consumer");
    Build::depend(builder, Source::Path {{ location: "{}" }});
    builder.roots.bind(windows_x86_64::ProgramEntry, Main::main);
}}
"#,
                standard_library.display().to_string().replace('\\', "/")
            ),
        )
        .unwrap();
        std::fs::write(
            root.join("application/main.omg"),
            "data Main {}\nmachine Main::main(&mut self) {}\n",
        )
        .unwrap();
        Self(root)
    }
}

impl Drop for EntryConsumer {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn entry_discovery_survives_memory_and_persistent_dependency_carry() {
    use super::super::{CandidateSourcePreparation, compile_pass};

    let fixture = EntryConsumer::new();
    let storage =
        SourceResolver::for_hardened_base(fixture.0.join("resolved"), PrimaryGitChoices::default())
            .unwrap();
    let closure = resolve_external_local_project_closure(
        fixture.0.join("application"),
        ExternalSourceContext::derive(b"entry-candidate-carry"),
        &storage,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
        GitResolutionOptions::default(),
    )
    .unwrap();
    let exact = closure.for_exact_target(target::TargetProfile::WindowsX64);
    let mut preparation = CandidateSourcePreparation::for_closure(&closure);
    let cold = compile_pass(
        &exact,
        &fixture.0.join("cold"),
        &[],
        None,
        None,
        None,
        TargetEntryDiscovery::Dependencies,
        None,
        &mut preparation,
    )
    .expect("fresh dependency offers its entry contract");
    let root = closure.graph().root();
    let expected = cold.reviews.review(root).unwrap().semantic_bindings();
    assert!(expected.iter().any(|binding| {
        binding.role() == package_compilation::AcceptedSemanticBindingRole::WindowsX64ProgramEntry
    }));

    let memory = compile_pass(
        &exact,
        &fixture.0.join("memory"),
        &[],
        None,
        None,
        None,
        TargetEntryDiscovery::Dependencies,
        Some(&cold),
        &mut preparation,
    )
    .expect("in-memory carry offers the same entry contract");
    assert!(memory.carried > 0);
    assert_eq!(
        memory.reviews.review(root).unwrap().semantic_bindings(),
        expected
    );

    let mut preparation = CandidateSourcePreparation::for_closure(&closure);
    let persistent = compile_pass(
        &exact,
        &fixture.0.join("persistent"),
        &[],
        None,
        None,
        None,
        TargetEntryDiscovery::Dependencies,
        None,
        &mut preparation,
    )
    .expect("disk cache carries the entry contract into a new invocation");
    assert!(
        persistent
            .occurrences
            .iter()
            .any(|occurrence| { matches!(occurrence.checked, OccurrenceCompile::Cached(_)) }),
        "must exercise decoded records, not another fresh compile"
    );
    assert_eq!(
        persistent.reviews.review(root).unwrap().semantic_bindings(),
        expected
    );

    // Source reuse is not acceptance of a different entry schema. The same
    // supplier leaves the source frontier unchanged, but the consuming check
    // must still compare the binding with that source's exact schema.
    let entry = expected
        .iter()
        .find(|binding| {
            binding.role()
                == package_compilation::AcceptedSemanticBindingRole::WindowsX64ProgramEntry
        })
        .unwrap();
    let mut digest = *entry.normalized_schema_digest().as_bytes();
    digest[0] ^= 1;
    let substituted = package_compilation::AcceptedSemanticBinding::new_service(
        entry.role(),
        entry.package(),
        entry.declaration_path(),
        effects::provider_plan::ServiceSchemaDigest::from_digest(digest),
    )
    .unwrap();
    let input = crate::review::ConsumerScopedSemanticBindingReviewInput::new(
        root.clone(),
        cold.reviews.review(root).unwrap().checked_context(),
        substituted,
    );
    let assemblies_before =
        artifacts::compile_timings::counters::source_assembly_invocation_count();
    let reuses_before = artifacts::compile_timings::counters::typed_frontier_reuse_count();
    let substituted = compile_pass(
        &exact,
        &fixture.0.join("substituted-schema"),
        &[input],
        None,
        None,
        None,
        TargetEntryDiscovery::Disabled,
        Some(&persistent),
        &mut preparation,
    );
    let Err(crate::review::CompileResolvedPackageReviewsError::Compilation {
        package,
        diagnostics,
    }) = substituted
    else {
        panic!("a retained source frontier must not authorize a substituted entry schema");
    };
    assert_eq!(&package, root);
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains(
                "does not match the exact accepted WindowsX64ProgramEntry package binding",
            )
        }),
        "{diagnostics:?}"
    );
    assert_eq!(
        artifacts::compile_timings::counters::source_assembly_invocation_count()
            - assemblies_before,
        0,
        "the invalid binding must reject after reusing the same sources"
    );
    assert_eq!(
        artifacts::compile_timings::counters::typed_frontier_reuse_count() - reuses_before,
        1,
    );

    // Availability of a cached proposal is not permission to select it when
    // the caller supplied an explicit binding set without that entry.
    let explicit = compile_pass(
        &exact,
        &fixture.0.join("explicit"),
        &[],
        None,
        None,
        None,
        TargetEntryDiscovery::Disabled,
        None,
        &mut preparation,
    );
    let Err(crate::review::CompileResolvedPackageReviewsError::Compilation {
        package,
        diagnostics,
    }) = explicit
    else {
        panic!("explicitly unbound entry must fail checking");
    };
    assert_eq!(&package, root);
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("accepted package-owned Windows x86-64 binding")
        }),
        "cached proposals must not become implicit authority: {diagnostics:?}"
    );
}

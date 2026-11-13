//! Public source-produced stop/resume control for a granted live Source scope.
use artifacts::compile_timings::{CompileTimings, counters};
use std::sync::{Arc, Mutex};

struct ExactGrant {
    expected: Vec<package_compilation::RestrictedBuildRequest>,
    observed: Arc<Mutex<usize>>,
}
impl package_compilation::RestrictedBuildGrants for ExactGrant {
    fn admit(
        &mut self,
        request: &package_compilation::RestrictedBuildRequest,
    ) -> Result<(), Vec<diagnostics::Diagnostic>> {
        assert!(
            self.expected.contains(request),
            "resume changed normalized request meaning"
        );
        *self.observed.lock().unwrap() += 1;
        Ok(())
    }
}

pub fn run() {
    let temporary = super::exclusive_workspace("restricted-consent-source");
    let project = temporary.path().join("project");
    std::fs::create_dir(&project).unwrap();
    let entry = project.join("main.omg");
    std::fs::write(&entry, "machine main() {}\n").unwrap();
    std::fs::write(project.join("input.txt"), b"cold").unwrap();
    std::fs::write(
        project.join("build.omg"),
        r#"
machine build(builder: &mut Build) crashes Trap {
    Build::application(builder, "restricted_consent_source");
    let path: BuildPath = BuildSource::resolve(&builder.source, "input.txt");
    let reader: i32 = BuildSource::open(&builder.source, path, 0);
    let mut bytes: [u8; 4];
    let count: i64 = BuildSource::read(&builder.source, reader, &mut bytes, 4);
    _ = BuildSource::close(&builder.source, reader);
    transition count == 4 && bytes[0] == 108 && bytes[1] == 105 && bytes[2] == 118 && bytes[3] == 101 {
        true -> done(builder) _ -> bad(builder)
    }
    machine done(builder: &mut Build) {}
    machine bad(builder: &mut Build) { crash Trap; }
}
"#,
    )
    .unwrap();
    let host = target::TargetProfile::host_if_supported().unwrap();
    let staging = build_time_evaluation::BuildMachineFilesystemSponsor::create_private(
        temporary.path().join("staging"),
        build_time_evaluation::BuildMachineFilesystemSponsorLimits::COMPILER_DEFAULT,
    )
    .unwrap();
    let output = staging.session_root().unwrap().join("output");
    let observed = Arc::new(Mutex::new(0));
    let mut expected = Vec::new();
    for resume in [false, true] {
        let preparation = compilation_products::SourcePreparation::default();
        let mut timings = CompileTimings::default();
        let checkpoint = {
            let mut catalogue = preparation.catalogue().unwrap();
            package_compilation::source_assembly::ImmutableSourceParseCheckpoint::prepare(
                &entry,
                None,
                None,
                &mut timings,
                &mut catalogue,
            )
            .unwrap()
        };
        let (_, mut syntax) = {
            let mut catalogue = preparation.catalogue().unwrap();
            checkpoint
                .assemble(None, &mut timings, &mut catalogue)
                .unwrap()
        };
        build_settings::select_build_helpers(
            &mut syntax.syntax_trees,
            host,
            &syntax.build_scope_sources,
        );
        let build_source = syntax.build_source;
        let sources = syntax.sources.clone();
        let generated = syntax.generated_source_custody;
        let mut continuation = psi_compiler::SourceCompilationRequest {
            syntax: syntax.syntax_trees,
            source_items: syntax.source_items,
            root_sources: syntax.root_sources,
            frontends: preparation.frontends,
            build_helper_identity: host.target_name(),
            sources: syntax.sources,
            top_level_bindings: syntax.source_scoped_top_level_bindings,
            selection_authority: None,
        }
        .prepare(&mut psi_compiler::UnmeasuredProduction)
        .unwrap();
        let input = continuation.build_input();
        let before = counters::build_execution_count();
        // This direct evaluator owns an explicitly live standalone scope. The
        // ordinary package compiler continues to supply captured source custody.
        let admitted = build_settings::prepare_build_evaluation(
            build_settings::BuildEvaluationRequest {
                execution_purpose:
                    artifacts::compile_timings::counters::BuildExecutionPurpose::Source,
                pure_build_compilation: None,
                root_path: &entry,
                typed: input.typed,
                pre_checks: input.pre_checks,
                build_source,
                sources: &sources,
                package_inputs: None,
                anonymous_source: None,
                generated_source_custody: &generated,
                execution: package_compilation::BuildExecutionInputs {
                    profile: Some(host),
                    directory: Some(output.clone()),
                    filesystem_sponsor: Some(staging.clone()),
                    evaluation_sponsor: Some(super::sponsor(super::limits(1_000_000))),
                    restricted_grants: resume.then(|| {
                        Box::new(ExactGrant {
                            expected: expected.clone(),
                            observed: observed.clone(),
                        })
                            as Box<dyn package_compilation::RestrictedBuildGrants>
                    }),
                    snapshot: None,
                    dependency_activation: false,
                },
                artifact_only: false,
                prior_checks: Vec::new(),
                retain_check: false,
                checked_slices: None,
            },
            &mut timings,
        );
        if !resume {
            let compiler::CompileFailure::BuildConsentRequired(requests) = admitted.err().unwrap()
            else {
                panic!("standalone live access must stop as structured consent");
            };
            assert_eq!(requests.len(), 1);
            super::build_sequence::print_execution_counts(
                "consent refused before source execution",
            );
            assert_eq!(counters::build_execution_count(), before);
            expected = requests;
            println!("restricted_consent_stopped_before_guest_execution");
            continue;
        }
        let mut admitted = admitted.expect("explicit exact consent must resume the admitted scope");
        assert_eq!(*observed.lock().unwrap(), 1);
        // This change occurs after admission: a captured fallback would read cold.
        std::fs::write(project.join("input.txt"), b"live").unwrap();
        let execution = admitted.execution().unwrap();
        let mut activation = execution.host().unwrap().start().unwrap();
        while !activation.advance(1).unwrap().returned {}
        let returned = activation.complete().unwrap();
        admitted.complete(Some(returned)).unwrap();
        super::build_sequence::print_execution_counts("source executed with exact consent");
        assert_eq!(counters::build_execution_count(), before + 1);
        println!("restricted_consent_resumed_once_and_read_the_granted_bytes");
    }
}

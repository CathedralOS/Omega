use super::*;

fn assert_native_exit_code(report: &CompileReport, expected: i32, fixture: &str) {
    let executable = report
        .checked_native_executable_path()
        .unwrap_or_else(|| panic!("{fixture} lost its exact executable publication receipt"));
    let output = Command::new(executable)
        .output()
        .unwrap_or_else(|error| panic!("{fixture} should run: {error}"));
    assert_eq!(
        output.status.code(),
        Some(expected),
        "{fixture} should exit {expected}, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn assert_toolchain_build_source_drops(report: &str) {
    for machine in ["Build::depend", "Build::depend_as"] {
        let state = machine
            .strip_prefix("Build::")
            .expect("toolchain build machine should have its qualified prefix");
        let expected =
            format!("- AffineDrop `source` in machine `{machine}` state `{state}` at state exit");
        let event = report
            .lines()
            .find(|line| line.starts_with(&expected))
            .unwrap_or_else(|| panic!("missing toolchain `{machine}` Source drop\n{report}"));
        assert!(
            event.contains("multiplicity=Affine, access=Owned")
                && event.contains("claim=unknown, provenance=unknown, obligation_live=false")
                && event.contains("realization=checked-no-code(trivial-affine-drop)"),
            "toolchain `{machine}` Source drop must remain a complete no-code affine cleanup, got:\n{event}"
        );
        assert_eq!(
            report.matches(&expected).count(),
            1,
            "toolchain `{machine}` must contribute exactly one Source drop\n{report}"
        );
    }
}

#[test]
fn output_only_checks_suppress_artifacts_without_suppressing_wire_validation() {
    let success_build_dir = unique_no_output_build_dir();
    let success = compiler::compile(
        CompileRequest::new(CompilerOptions {
            root_path: pass_canary("dependent/boundary_equality_recast_witness_compile")
                .join("main.omg"),
            build_dir: Some(success_build_dir.clone()),
            target_name: None,
        })
        .with_artifact_policy(ArtifactEmissionPolicy::OutputOnly),
    )
    .expect("output-only frontend check should succeed");
    assert!(!success.wrote_output());
    assert!(
        !success_build_dir.exists(),
        "output-only frontend checks should not materialize an artifact directory"
    );

    let failure_build_dir = unique_no_output_build_dir();
    let diagnostics = compiler::compile(
        CompileRequest::new(CompilerOptions {
            root_path: fail_canary("wire/wire_compatibility_preservation_unmet").join("main.omg"),
            build_dir: Some(failure_build_dir.clone()),
            target_name: None,
        })
        .with_artifact_policy(ArtifactEmissionPolicy::OutputOnly),
    )
    .expect_err("output-only mode must retain wire compatibility validation");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("unknown preservation")),
        "expected the unsatisfied wire fact, got {diagnostics:#?}"
    );
    assert!(
        !failure_build_dir.exists(),
        "a rejected output-only check should not materialize report artifacts"
    );
}

#[test]
fn full_checked_observation_emits_ordered_timings_with_checked_snapshots() {
    let build_dir = unique_no_output_build_dir();
    let report = compiler::compile(
        CompileRequest::new(CompilerOptions {
            root_path: pass_canary("dependent/boundary_equality_recast_witness_compile")
                .join("main.omg"),
            build_dir: Some(build_dir.clone()),
            target_name: None,
        })
        .with_artifact_policy(ArtifactEmissionPolicy::Full),
    )
    .expect("full frontend check should emit one checked observation bundle");
    assert!(!report.wrote_output());

    for file_name in [
        "trust_report.md",
        "05_capability_manifest.json",
        "00_timings.html",
    ] {
        assert!(
            build_dir.join(file_name).is_file(),
            "full checked observation should emit {file_name}"
        );
    }
    let timings = fs::read_to_string(build_dir.join("00_timings.html"))
        .expect("read checked timing observation");
    let mut prior = None;
    for stage in ["Stage 01", "Stage 02", "Stage 03", "Stage 04", "Stage 05"] {
        let position = timings
            .rfind(stage)
            .unwrap_or_else(|| panic!("timing report omitted {stage}\n{timings}"));
        if let Some(prior) = prior {
            assert!(
                prior < position,
                "timing report reordered {stage}\n{timings}"
            );
        }
        prior = Some(position);
    }

    let _ = fs::remove_dir_all(build_dir);
}

#[test]
fn checked_semantic_equality_excludes_timing_observations() {
    let main = pass_canary("dependent/boundary_equality_recast_witness_compile").join("main.omg");
    let first = compile_to_checked(&main, None).expect("first checked compilation");
    let replay = compile_to_checked(&main, None).expect("replayed checked compilation");
    assert_eq!(
        first, replay,
        "nondeterministic phase measurements must not enter checked semantic equality"
    );
}

#[test]
fn output_only_backend_compile_keeps_primary_image_and_certification() {
    let build_dir = unique_no_output_build_dir();
    let report = compiler::compile(
        CompileRequest::new(CompilerOptions {
            root_path: pass_canary("terminal_psi/selected_empty_component").join("main.omg"),
            build_dir: Some(build_dir.clone()),
            target_name: Some("linux_x86_64".into()),
        })
        .with_requested_product(RequestedCompileProduct::NativeArtifact)
        .with_artifact_policy(ArtifactEmissionPolicy::OutputOnly),
    )
    .expect("output-only backend compile should still certify its image")
    .publish_retained_native_artifact(&build_dir)
    .expect("output-only native artifact should publish");
    assert!(report.wrote_output());
    assert_eq!(
        report
            .checked_native_executable_path()
            .map(std::path::Path::to_path_buf),
        Some(build_dir.join("omega-program")),
    );
    assert!(build_dir.join("omega-program").is_file());
    let entries = fs::read_dir(&build_dir)
        .expect("read output-only build directory")
        .map(|entry| entry.expect("read output-only build entry").file_name())
        .collect::<Vec<_>>();
    assert_eq!(
        entries,
        [std::ffi::OsString::from("omega-program")],
        "output-only backend compilation must omit auxiliary reports"
    );
    let _ = fs::remove_dir_all(build_dir);
}

#[test]
fn typed_requested_product_stops_at_exact_check_and_native_artifact_boundaries() {
    let check_dir = unique_no_output_build_dir();
    let report = compiler::compile(
        CompileRequest::new(CompilerOptions {
            root_path: pass_canary("build/explicit_program_entry_binding").join("main.omg"),
            build_dir: Some(check_dir.clone()),
            target_name: Some("windows_x86_64".into()),
        })
        .with_requested_product(compiler::RequestedCompileProduct::Check)
        .with_artifact_policy(ArtifactEmissionPolicy::OutputOnly),
    )
    .expect("the explicit Check product must stop before native realization");
    assert!(!report.wrote_output());
    assert_eq!(report.output_kind(), compiler::CompileOutputKind::CheckOnly);
    assert!(!check_dir.exists());

    let native_dir = unique_no_output_build_dir();
    let native = compiler::compile(
        CompileRequest::new(CompilerOptions {
            root_path: pass_canary("terminal_psi/selected_empty_component").join("main.omg"),
            build_dir: Some(native_dir.clone()),
            target_name: Some("linux_x86_64".into()),
        })
        .with_requested_product(compiler::RequestedCompileProduct::NativeArtifact)
        .with_artifact_policy(ArtifactEmissionPolicy::OutputOnly),
    )
    .expect("the retained native product should stop after validated native emission");
    assert!(!native.wrote_output());
    assert_eq!(
        native.output_kind(),
        compiler::CompileOutputKind::RetainedNativeArtifact
    );
    assert!(native.executable_publication().is_none());
    assert!(native.app_bundle_publication().is_none());
    assert!(native.checked_native_executable_path().is_none());
    let artifact = native
        .retained_native_artifact()
        .expect("native-artifact report must retain exactly one payload");
    artifact
        .validate()
        .expect("retained native payload must independently replay");
    assert_eq!(artifact.target(), target::NativeTarget::linux_x64());
    assert_eq!(
        artifact.psi_artifact().manifest().semantic(),
        artifact.object().psi()
    );
    assert_eq!(artifact.object().psi(), artifact.image().psi());
    assert!(!artifact.object().text_bytes().is_empty());
    assert!(!artifact.image().output().bytes.is_empty());
    assert!(
        !native_dir.exists(),
        "output-only retained native compilation must not create a build directory"
    );
    let terminal_dir = unique_no_output_build_dir();
    let terminal = compiler::compile(
        CompileRequest::new(CompilerOptions {
            root_path: pass_canary("terminal_psi/selected_empty_component").join("main.omg"),
            build_dir: Some(terminal_dir.clone()),
            target_name: Some("linux_x86_64".into()),
        })
        .with_requested_product(compiler::RequestedCompileProduct::TerminalArtifact)
        .with_artifact_policy(ArtifactEmissionPolicy::OutputOnly),
    )
    .expect("terminal product should stop at the canonical Psi-owned artifact");
    assert!(!terminal.wrote_output());
    assert_eq!(
        terminal.output_kind(),
        compiler::CompileOutputKind::TerminalArtifact
    );
    assert!(terminal.retained_native_artifact().is_none());
    assert!(terminal.executable_publication().is_none());
    let artifact = terminal
        .artifact()
        .expect("terminal-artifact report must retain exactly one canonical payload");
    artifact
        .validate()
        .expect("retained terminal payload must independently replay");
    assert_eq!(
        artifact.manifest().semantic(),
        terminal_codec::terminal_psi_identity(
            &terminal_codec::decode_module(artifact.semantic_bytes())
                .expect("retained canonical semantics decode")
        )
        .expect("retained semantic identity")
    );
    let native_artifact = native
        .retained_native_artifact()
        .expect("native report retains its canonical native realization");
    assert_eq!(
        native_artifact.psi_artifact().manifest(),
        artifact.manifest(),
        "Terminal-only and native products must share the exact canonical handoff"
    );
    assert_eq!(
        native_artifact.psi_artifact().semantic_bytes(),
        artifact.semantic_bytes()
    );
    assert_eq!(
        native_artifact.psi_artifact().proof_bytes(),
        artifact.proof_bytes()
    );
    assert!(
        !terminal_dir.exists(),
        "terminal artifact production must not create native or report output"
    );
    terminal
        .into_retained_terminal_artifact()
        .expect("complete terminal product custody must leave the report only by value")
        .into_parts()
        .0
        .validate()
        .expect("transferred terminal artifact custody must still replay");
    native
        .into_retained_native_artifact()
        .expect("native artifact custody must leave the report only by value")
        .validate()
        .expect("transferred native artifact custody must still replay");

    let unsupported = compiler::compile(
        CompileRequest::new(CompilerOptions {
            root_path: pass_canary("build/explicit_program_entry_binding").join("main.omg"),
            build_dir: None,
            target_name: Some("windows_x86_64".into()),
        })
        .with_requested_product(compiler::RequestedCompileProduct::TerminalArtifact),
    )
    .expect_err("unsupported Terminal constructs must fail instead of selecting legacy lowering");
    assert!(unsupported.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("terminal-artifact production failed")
    }));

    let unsupported_native_dir = unique_no_output_build_dir();
    let unsupported_native = compiler::compile(
        CompileRequest::new(CompilerOptions {
            root_path: pass_canary("build/explicit_program_entry_binding").join("main.omg"),
            build_dir: Some(unsupported_native_dir.clone()),
            target_name: Some("windows_x86_64".into()),
        })
        .with_requested_product(compiler::RequestedCompileProduct::NativeArtifact)
        .with_artifact_policy(ArtifactEmissionPolicy::OutputOnly),
    )
    .expect_err("unsupported Terminal constructs must not fall back for NativeArtifact");
    assert!(unsupported_native.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("native-artifact Terminal production failed")
    }));
    assert!(
        !unsupported_native_dir.exists(),
        "failed native realization must not write output or reports"
    );
}

#[test]
fn disposable_native_canary_helper_emits_only_the_primary_image() {
    let build_dir = unique_no_output_build_dir();
    let report = compile(CanaryCompileSpec {
        root_path: pass_canary("calls/free_standing_machine_helper_compile").join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("disposable native canary should compile through the shared helper");

    assert!(report.wrote_output());
    assert!(build_dir.join(executable_name()).is_file());
    let entries = fs::read_dir(&build_dir)
        .expect("read disposable native canary build directory")
        .map(|entry| {
            entry
                .expect("read disposable native canary entry")
                .file_name()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        entries,
        [std::ffi::OsString::from(executable_name())],
        "the shared disposable helper must not emit auxiliary reports or viewers"
    );
    let _ = fs::remove_dir_all(build_dir);
}

fn artifact_file_footprint(directory: &Path) -> (usize, u64) {
    fs::read_dir(directory)
        .expect("read artifact footprint directory")
        .map(|entry| entry.expect("read artifact footprint entry"))
        .fold((0, 0), |(count, bytes), entry| {
            let metadata = entry.metadata().expect("read artifact footprint metadata");
            if metadata.is_dir() {
                let (child_count, child_bytes) = artifact_file_footprint(&entry.path());
                (count + child_count, bytes + child_bytes)
            } else {
                (count + 1, bytes + metadata.len())
            }
        })
}

#[test]
fn rooted_native_helpers_separate_disposable_and_auxiliary_artifacts() {
    let canary = pass_canary("ownership/linear_transfer_and_consume");
    let scratch = unique_no_output_build_dir();
    let output_only = scratch.join("output-only");
    let full = scratch.join("full");

    let output_only_report = compile_rooted_canary_for_native_host(&canary, output_only.clone())
        .expect("ordinary rooted native helper should compile");
    compile_rooted_canary_for_native_host_with_auxiliary_artifacts(&canary, full.clone())
        .expect("explicit rooted report helper should compile");

    assert_native_exit_code(
        &output_only_report,
        0,
        "output-only rooted linear transfer and consume canary",
    );

    assert!(output_only.join(executable_name()).is_file());
    assert_native_exit_code(&output_only_report, 0, "output-only rooted helper canary");
    assert!(!output_only.join("backend_report.txt").exists());
    assert!(full.join(executable_name()).is_file());
    assert!(full.join("backend_report.txt").is_file());
    let output_only_footprint = artifact_file_footprint(&output_only);
    let full_footprint = artifact_file_footprint(&full);
    assert_eq!(
        output_only_footprint.0, 1,
        "ordinary rooted native builds must retain only the primary image"
    );
    assert!(full_footprint.0 > output_only_footprint.0);
    assert!(full_footprint.1 > output_only_footprint.1);
    eprintln!(
        "rooted native artifact footprint: full={} files/{} bytes output-only={} files/{} bytes",
        full_footprint.0, full_footprint.1, output_only_footprint.0, output_only_footprint.1,
    );

    let _ = fs::remove_dir_all(scratch);
}

#[test]
fn boundary_trait_canary_reports_capability_use() {
    let canary = pass_canary("traits/boundary_trait_effects_host_call");
    let main_path = canary.join("main.omg");
    let scratch = std::env::temp_dir().join(format!(
        "omega-capability-manifest-canary-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let checked_dir = scratch.join("checked");

    let checked_compilation = compile_with_auxiliary_artifacts(CanaryCompileSpec {
        root_path: main_path.clone(),
        build_dir: Some(checked_dir.clone()),
        target_name: None,
        product: CanaryCompileProduct::Check,
    })
    .expect("boundary trait canary should compile with checked capability artifacts");
    assert!(!checked_compilation.wrote_output());

    let checked_manifest = fs::read_to_string(checked_dir.join("05_capability_manifest.json"))
        .expect("capability manifest should be written");
    let carry_manifest = fs::read_to_string(checked_dir.join("05_carry_manifest.json"))
        .expect("carry manifest should be written");
    let task_manifest = fs::read_to_string(checked_dir.join("05_task_activations.json"))
        .expect("task activation manifest should be written");

    let source_dir = scratch.join("source");
    fs::create_dir_all(&source_dir).expect("create exact-entry capability source directory");
    fs::copy(&main_path, source_dir.join("main.omg"))
        .expect("copy boundary-trait capability canary");
    fs::write(
        source_dir.join("build.omg"),
        hosted_main_program_entry_build("macos_arm64"),
    )
    .expect("write exact macOS AArch64 ProgramEntry binding");
    let lowered_dir = scratch.join("lowered");
    let lowered_compilation = compile_with_auxiliary_artifacts(CanaryCompileSpec {
        root_path: source_dir.join("main.omg"),
        build_dir: Some(lowered_dir.clone()),
        target_name: Some("macos_arm64".into()),
        product: CanaryCompileProduct::Check,
    })
    .expect("exact-root boundary trait canary should reach lowering reports");
    assert!(!lowered_compilation.wrote_output());

    let entry_manifest = fs::read_to_string(lowered_dir.join("05_capability_manifest.json"))
        .expect("exact-root capability manifest should be written");
    assert!(
        checked_manifest.contains("\"capability_flows\": {\"uses\": 2"),
        "capability manifest should report both boundary capability uses\n{}",
        checked_manifest
    );
    assert!(
        checked_manifest.contains("\"entry_machine\": \"<missing>\"")
            && checked_manifest.contains("\"service_reach\": []"),
        "entry-agnostic capability checking must not invent an entry reach\n{checked_manifest}"
    );
    assert!(
        entry_manifest.contains("\"service_reach\": [\"Console\"]")
            && entry_manifest.contains("\"may_suspend\": false")
            && entry_manifest.contains("\"may_block\": false"),
        "capability manifest should report canonical service reach and independent operational axes\n{}",
        entry_manifest
    );
    for manifest in [&checked_manifest, &entry_manifest] {
        assert!(
            !manifest.contains("\"effect_bits\"") && !manifest.contains("\"effects\""),
            "capability manifest must not expose the retired compatibility effect set\n{}",
            manifest
        );
    }
    assert!(
        carry_manifest.contains("\"effective\":")
            && carry_manifest.contains("\"suspension\":")
            && carry_manifest.contains("\"address\":")
            && carry_manifest.contains("\"activation_wide_carry\": [")
            && carry_manifest.contains("\"analysis_complete\":"),
        "carry manifest should expose structured checked policies\n{}",
        carry_manifest
    );
    assert!(
        task_manifest.contains("\"activations\": ["),
        "task activation artifact should always expose its normalized root\n{}",
        task_manifest
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn opaque_boundary_data_reaches_checked_facts_without_a_layout_claim() {
    let canary = pass_canary("proofs/boundary_data_opaque_contract");
    let checked = compile_to_checked(&canary.join("main.omg"), None)
        .expect("opaque boundary data should be usable in frontend contracts");
    let opaque = checked
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "OpaqueToken")
        .expect("opaque carrier");

    assert_eq!(
        opaque.supply_mode,
        language_semantics::DataSupplyMode::BoundaryOpaque
    );
    assert_eq!(
        checked
            .facts
            .carry
            .for_data(opaque.symbol)
            .expect("opaque carry fact")
            .effective,
        language_semantics::CarryPolicy::STRICT,
        "opacity must fail closed rather than deriving permissive carry from an empty visible shape"
    );
}

// Frozen decision 10 (wire eras): cross-era type changes are legal evolution
// surfaced as "requires migration" verdicts in the wire protocol compatibility
// report, and the report compares ADJACENT eras along the version chain
// (v1 -> v2, newest era -> current), never every era against current.
#[test]
fn wire_cross_era_type_change_reports_requires_migration_verdict() {
    let canary = pass_canary("wire/wire_cross_era_type_change_migration");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-wire-migration-verdict-canary-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_with_auxiliary_artifacts(CanaryCompileSpec {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        product: CanaryCompileProduct::Check,
    })
    .expect("cross-era type change canary should compile with a migration verdict, not an error");
    assert!(!compilation.wrote_output());

    let report = fs::read_to_string(build_dir.join("04_wire_protocols.txt"))
        .expect("wire protocol compatibility report should be written");
    assert!(
        report.contains("### compatibility v1 -> v2")
            && report.contains("### compatibility v2 -> current"),
        "wire report should compare adjacent eras along the version chain\n{}",
        report
    );
    assert!(
        report.contains(
            "field 0 changes type i32 -> i64; decode via the old era's table and migrate up the chain"
        ),
        "wire report should record the cross-era type change as a requires-migration verdict\n{}",
        report
    );
    assert!(
        !report.contains("### compatibility v1 -> current"),
        "wire report should not compare a non-newest era against the current body\n{}",
        report
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn wire_compatibility_demand_reports_directional_facts_and_migration_route() {
    let canary = pass_canary("wire/wire_compatibility_demand_report");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-wire-edge-demand-canary-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_with_auxiliary_artifacts(CanaryCompileSpec {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        product: CanaryCompileProduct::Check,
    })
    .expect("the declared rolling-channel demand should be satisfied");
    assert!(!compilation.wrote_output());

    let report = fs::read_to_string(build_dir.join("04_wire_protocols.txt"))
        .expect("wire protocol compatibility report should be written");
    for expected in [
        "identity-keyed schemas: 2",
        "edge compatibility demands: 1",
        "## compatibility demand RollingChannel",
        "lineage: MessageLineage",
        "local schema: LocalMessage",
        "peer schema: PeerMessage",
        "unknown-member behavior: strict",
        "codec requirement: StrictDecode<compact_binary, LocalMessage>",
        "codec requirement identity: 0x",
        "normalized plan identity: 0x",
        "realization origin: generated by Omega compiler compact_binary generator",
        "trust class: admitted by Omega compiler",
        "generated body is not yet independently checked against the public codec requirement",
        "differential canaries are validation evidence, not derived-contract proof",
        "readability: yes (required)",
        "writability: yes (required)",
        "unknown preservation: no (not required)",
        "canonicality: yes (required)",
        "migration coverage: yes (required) -- selected checked route: peer_to_local",
        "verdict: satisfied",
    ] {
        assert!(
            report.contains(expected),
            "wire compatibility report should contain `{expected}`\n{report}"
        );
    }
    assert_eq!(
        report.matches("## data LocalMessage").count(),
        1,
        "reflected schema and generated realization facts belong in one row\n{report}"
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// The canonical permission ledger must stay visible per event in the backend
// report's Artifact Semantic Spine after surviving the full spine (checked
// trees -> state graph -> control flow -> abstract -> target -> assigned ->
// machine instructions -> encoded machine). Legacy move/drop compatibility
// rows end at control flow and must not reappear in a backend artifact.
#[test]
fn backend_report_renders_ownership_summary_events() {
    let canary = pass_canary("ownership/linear_transfer_and_consume");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-ownership-spine-canary-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation =
        compile_rooted_canary_for_native_host_with_auxiliary_artifacts(&canary, build_dir.clone())
            .expect("linear transfer and consume canary should compile from its authored root");
    assert_native_exit_code(&compilation, 0, "linear transfer and consume canary");

    let report = fs::read_to_string(build_dir.join("backend_report.txt"))
        .expect("backend report should be written");
    assert_toolchain_build_source_drops(&report);
    assert!(
        report.contains("permissions: 13"),
        "spine should retain the complete permission ledger\n{}",
        report
    );
    assert!(
        report.contains("permission realizations: 13 (complete)"),
        "every permission event should have a normalized realization\n{}",
        report
    );
    assert!(
        report.contains(
            "- Establish `<unnamed>` in machine `Main::main` state `main` at statement 0 (multiplicity=Linear, access=Owned, claim=Main::main::main at statement 0 #0, provenance=Main::main::main at statement 0, obligation_live=true)"
        ),
        "spine should record linear establishment, claim identity, and provenance\n{}",
        report
    );
    assert!(
        report
            .matches("realization=selected-instructions[4, 5]")
            .count()
            == 3,
        "folded establishment and transfer should share the exact selected materialization\n{}",
        report
    );
    assert!(
        report.contains(
            "- Transfer `<unnamed>` in machine `Main::main` state `main` at statement 1 (multiplicity=Linear, access=Owned, claim=Main::main::main at statement 0 #0, provenance=Main::main::main at statement 0, obligation_live=true)"
        ),
        "spine should record transfer without minting a new claim or provenance\n{}",
        report
    );
    assert!(
        report.contains(
            "- Establish `forwarded` in machine `Main::main` state `main` at statement 1 (multiplicity=Linear, access=Owned, claim=Main::main::main at statement 0 #0, provenance=Main::main::main at statement 0, obligation_live=true)"
        ),
        "spine should record the receiving place's established obligation\n{}",
        report
    );
    assert!(
        report.contains(
            "- Consume `forwarded` in machine `Main::main` state `main` at call ordinal 0 in statement 2 (multiplicity=Linear, access=Owned, claim=Main::main::main at statement 0 #0, provenance=Main::main::main at statement 0, obligation_live=true)"
        ),
        "spine should record terminal consumption\n{}",
        report
    );
    assert!(
        report.contains("realization=checked-no-code(explicit-zero-code-consume)"),
        "the zero-code consuming call should retain an explicit checked proof\n{}",
        report
    );
    assert!(
        !report.contains("\nmoves:")
            && !report.contains("\ndrops:")
            && !report.contains("UNLINKED")
            && !report.contains("INCOMPLETE"),
        "backend artifacts must not reconstruct legacy move/drop summaries\n{}",
        report
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn backend_report_renders_transparent_record_claim_paths() {
    let canary = pass_canary("ownership/linear_transparent_record_frontier");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-transparent-record-frontier-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation =
        compile_rooted_canary_for_native_host_with_auxiliary_artifacts(&canary, build_dir.clone())
            .expect("transparent record frontier canary should compile from its authored root");
    assert_native_exit_code(&compilation, 0, "transparent record frontier canary");

    let report = fs::read_to_string(build_dir.join("backend_report.txt"))
        .expect("backend report should be written");
    assert_toolchain_build_source_drops(&report);
    assert!(
        report.contains("permissions: 24")
            && report.contains("permission realizations: 24 (complete)"),
        "both contained claims must retain complete event realizations\n{report}"
    );
    for place in ["<unnamed>.left", "<unnamed>.right"] {
        assert!(
            report.contains(&format!("- Establish `{place}`"))
                && report.contains(&format!("- Transfer `{place}`")),
            "the backend artifact must retain path-indexed events for `{place}`\n{report}"
        );
    }
    let mut claims = report
        .lines()
        .filter(|line| line.starts_with("- ") && line.contains("multiplicity=Linear"))
        .filter_map(|line| {
            line.split_once("claim=")
                .and_then(|(_, rest)| rest.split_once(", provenance="))
                .map(|(claim, _)| claim)
        })
        .collect::<Vec<_>>();
    claims.sort_unstable();
    claims.dedup();
    assert_eq!(
        claims.len(),
        2,
        "the two contained resources need distinct claim identities\n{report}"
    );
    assert!(claims.iter().all(|claim| *claim != "unknown"));
    for claim in claims {
        assert_eq!(
            report.matches(&format!("claim={claim},")).count(),
            8,
            "each identity must survive every aggregate/local transfer\n{report}"
        );
    }
    assert!(
        !report.contains("UNLINKED") && !report.contains("INCOMPLETE"),
        "path-indexed claim events must stay linked through backend realization\n{report}"
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn backend_report_realizes_state_call_entry_at_call_site() {
    let canary = pass_canary("ownership/linear_state_call_handoff");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-ownership-state-call-canary-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation =
        compile_rooted_canary_for_native_host_with_auxiliary_artifacts(&canary, build_dir.clone())
            .expect("linear state-call handoff canary should compile from its authored root");
    assert_native_exit_code(&compilation, 0, "linear state-call handoff canary");

    let report = fs::read_to_string(build_dir.join("backend_report.txt"))
        .expect("backend report should be written");
    assert_toolchain_build_source_drops(&report);
    assert!(
        report.contains("permission realizations: 12 (complete)"),
        "the state-call permission ledger should be complete\n{}",
        report
    );
    let state_entry = report
        .lines()
        .find(|line| {
            line.contains(
                "- Establish `receipt` in machine `Main::consume` state `consume` at state entry",
            )
        })
        .expect("target-state entry establishment should remain visible in the report");
    assert!(
        state_entry.contains("realization=selected-instructions[")
            && !state_entry.contains("checked-no-code"),
        "target-state entry should join the selected state-call handoff, got:\n{state_entry}"
    );
    assert!(
        !report.contains("UNLINKED") && !report.contains("INCOMPLETE"),
        "the state-call handoff must remain fail-closed and complete\n{}",
        report
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn backend_report_separates_transition_and_nested_call_ordinals() {
    let canary = pass_canary("ownership/linear_transition_nested_call_handoff");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-ownership-transition-multicall-canary-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host_with_auxiliary_artifacts(
        &canary,
        build_dir.clone(),
    )
    .expect("linear nested-call transition handoff canary should compile from its authored root");
    assert_native_exit_code(
        &compilation,
        0,
        "linear nested-call transition handoff canary",
    );

    let report = fs::read_to_string(build_dir.join("backend_report.txt"))
        .expect("backend report should be written");
    assert_toolchain_build_source_drops(&report);
    assert!(
        report.contains("permission realizations: 18 (complete)"),
        "the multi-call transition permission ledger should be complete\n{}",
        report
    );
    assert!(
        report.contains(
            "- Transfer `first` in machine `Main::main` state `main` at call ordinal 0 in statement 2"
        ) && report.contains(
            "- Transfer `second` in machine `Main::main` state `main` at call ordinal 1 in statement 2"
        ),
        "the transition target and nested value call must retain distinct canonical ordinals\n{}",
        report
    );
    assert!(
        report.contains("StateCall { role: TransitionArgument, call_ordinal: 1, target_key:"),
        "runtime state-call planning must reserve ordinal 0 for the named transition target\n{}",
        report
    );
    for event_prefix in [
        "- Establish `receipt` in machine `Main::forward` state `forward` at state entry",
        "- Transfer `receipt` in machine `Main::forward` state `forward` at statement 0",
    ] {
        let event = report
            .lines()
            .find(|line| line.contains(event_prefix))
            .expect("forwarded obligation event should remain visible in the report");
        assert!(
            event.contains("realization=selected-instructions[")
                && !event.contains("checked-no-code"),
            "the inline return transfer should map to its selected result write, got:\n{event}"
        );
    }
    assert!(
        !report.contains("UNLINKED") && !report.contains("INCOMPLETE"),
        "the multi-call handoff must remain fail-closed and complete\n{}",
        report
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn backend_report_separates_repeated_transition_call_ordinals() {
    let canary = pass_canary("ownership/linear_repeated_transition_call_handoff");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-ownership-repeated-transition-call-canary-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host_with_auxiliary_artifacts(
        &canary,
        build_dir.clone(),
    )
    .expect("repeated-target linear transition-call canary should compile from its authored root");
    assert_native_exit_code(
        &compilation,
        0,
        "repeated-target linear transition-call canary",
    );

    let report = fs::read_to_string(build_dir.join("backend_report.txt"))
        .expect("backend report should be written");
    assert_toolchain_build_source_drops(&report);
    assert!(
        report.contains("permission realizations: 18 (complete)"),
        "the repeated-target permission ledger should be complete\n{}",
        report
    );
    assert!(
        report.contains(
            "- Transfer `first` in machine `Main::main` state `main` at call ordinal 1 in statement 2"
        ) && report.contains(
            "- Transfer `second` in machine `Main::main` state `main` at call ordinal 2 in statement 2"
        ),
        "same-target calls must retain distinct canonical ordinals\n{}",
        report
    );
    assert!(
        report
            .matches("StateCall { role: TransitionArgument, call_ordinal: 1, target_key:")
            .count()
            == 1
            && report
                .matches("StateCall { role: TransitionArgument, call_ordinal: 2, target_key:")
                .count()
                == 1,
        "runtime planning must preserve both same-target call identities\n{}",
        report
    );
    for event_prefix in [
        "- Establish `receipt` in machine `Main::forward` state `forward` at state entry",
        "- Transfer `receipt` in machine `Main::forward` state `forward` at statement 0",
    ] {
        let event = report
            .lines()
            .find(|line| line.contains(event_prefix))
            .expect("shared forward event should remain visible in the report");
        assert!(
            event.contains("realization=selected-instructions[")
                && event.contains(", ")
                && !event.contains("checked-no-code"),
            "the shared target event should join both call materializations, got:\n{event}"
        );
    }
    assert!(
        !report.contains("UNLINKED") && !report.contains("INCOMPLETE"),
        "the repeated-target handoff must remain fail-closed and complete\n{}",
        report
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn backend_report_realizes_linear_boundary_entry_from_prologue() {
    let canary = pass_canary("ownership/linear_boundary_entry_handoff");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-ownership-boundary-entry-canary-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile_with_auxiliary_artifacts(CanaryCompileSpec {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("linear boundary-entry handoff canary should compile");

    let report = fs::read_to_string(build_dir.join("backend_report.txt"))
        .expect("backend report should be written");
    assert_toolchain_build_source_drops(&report);
    assert!(
        report.contains("permission realizations: 4 (complete)"),
        "the boundary-entry permission ledger should be complete\n{}",
        report
    );
    let establishment = report
        .lines()
        .find(|line| {
            line.contains(
                "- Establish `handle` in machine `Main::main` state `main` at state entry",
            )
        })
        .expect("boundary StateEntry establishment should remain visible in the report");
    assert!(
        establishment.contains("realization=selected-instructions[2]")
            && !establishment.contains("checked-no-code"),
        "the inbound platform write must realize entry establishment, got:\n{establishment}"
    );
    assert!(
        report.contains("selected #2 EntryArgumentRegisterWrite"),
        "the realization must identify the concrete entry-prologue write\n{}",
        report
    );
    let consume = report
        .lines()
        .find(|line| {
            line.contains(
                "- Consume `handle` in machine `Main::main` state `main` at call ordinal 0 in statement 1",
            )
        })
        .expect("boundary obligation consume should remain visible in the report");
    assert!(
        consume.contains("realization=checked-no-code(explicit-zero-code-consume)"),
        "the empty release body should retain its independent no-code proof, got:\n{consume}"
    );
    assert!(
        !report.contains("UNLINKED") && !report.contains("INCOMPLETE"),
        "the boundary-entry handoff must remain fail-closed and complete\n{}",
        report
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn linear_obligation_survives_dispatched_call_continuation() {
    let canary = pass_canary("ownership/linear_live_across_call_continuation");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-ownership-call-continuation-canary-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation =
        compile_rooted_canary_for_native_host_with_auxiliary_artifacts(&canary, build_dir.clone())
            .expect("linear call-continuation canary should compile");
    assert_native_exit_code(&compilation, 7, "linear call-continuation canary");

    let report = fs::read_to_string(build_dir.join("backend_report.txt"))
        .expect("backend report should be written");
    let realization_summary = report
        .lines()
        .find(|line| line.starts_with("permission realizations:"))
        .expect("permission realization summary should be written");
    assert!(
        realization_summary.ends_with("(complete)")
            && !report.contains("UNLINKED")
            && !report.contains("INCOMPLETE"),
        "the continuation ownership ledger should remain fail-closed and complete\n{}",
        report
    );
    for event_prefix in [
        "- Establish `<unnamed>` in machine `Main::main` state `main` at statement 0",
        "- Consume `<unnamed>` in machine `Main::main` state `main` at call ordinal 0 in statement 2",
    ] {
        let event = report
            .lines()
            .find(|line| line.contains(event_prefix))
            .unwrap_or_else(|| panic!("missing continuation ownership event `{event_prefix}`"));
        assert!(
            event.contains("realization=selected-instructions[")
                && !event.contains("checked-no-code"),
            "the live receipt must join concrete post-continuation instructions, got:\n{event}"
        );
    }
    assert!(
        report.contains(
            "state-call-result(AssignmentValue#0) `__call_result_2_AssignmentValue_0`: i32 offset 0"
        ) && report.contains("local `code`: i32 offset 4")
            && report.contains(
                "write place integer 7 (4b) -> omega_runtime_frame_storage[ConstOffset(0)]"
            )
            && report.contains(
                "call host operation Process.exit(scalar i32 omega_runtime_frame_storage@4)"
            ),
        "the continuation must materialize, separate, and consume the call result\n{}",
        report
    );
    assert!(
        report.contains("- #0 `self` Value: `receipt` required true")
            && report.contains("- #1 `marker` Value: `false` required true"),
        "the static attached call must restore implicit self before authored parameters\n{}",
        report
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn backend_report_preserves_fresh_state_call_result_origin() {
    let canary = pass_canary("ownership/linear_fresh_state_call_result_handoff");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-ownership-fresh-state-result-canary-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation =
        compile_rooted_canary_for_native_host_with_auxiliary_artifacts(&canary, build_dir.clone())
            .expect("fresh linear state-call result canary should compile from its authored root");
    assert_native_exit_code(&compilation, 0, "fresh linear state-call result canary");

    let report = fs::read_to_string(build_dir.join("backend_report.txt"))
        .expect("backend report should be written");
    assert_toolchain_build_source_drops(&report);
    assert!(
        report.contains("permission realizations: 12 (complete)")
            && !report.contains("UNLINKED")
            && !report.contains("INCOMPLETE"),
        "the fresh state-call result ledger must remain complete\n{}",
        report
    );
    for event_prefix in [
        "- Establish `issued` in machine `Main::issue` state `issue` at statement 0",
        "- Transfer `issued` in machine `Main::issue` state `issue` at statement 1",
        "- Establish `returned` in machine `Main::main` state `main` at statement 0",
        "- Consume `returned` in machine `Main::main` state `main` at call ordinal 0 in statement 1",
    ] {
        let event = report
            .lines()
            .find(|line| line.contains(event_prefix))
            .expect("fresh-result permission event should remain visible");
        assert!(
            event.contains("claim=Main::issue::issue at statement 0 #0")
                && event.contains("provenance=Main::issue::issue at statement 0"),
            "the event must preserve the callee-local claim and origin, got:\n{event}"
        );
    }

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn backend_report_preserves_path_aligned_multi_claim_state_result() {
    let canary = pass_canary("ownership/linear_transparent_record_state_result");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-ownership-multi-state-result-canary-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host_with_auxiliary_artifacts(
        &canary,
        build_dir.clone(),
    )
    .expect("path-aligned multi-claim state result canary should compile from its authored root");
    assert_native_exit_code(
        &compilation,
        0,
        "path-aligned multi-claim state result canary",
    );

    let report = fs::read_to_string(build_dir.join("backend_report.txt"))
        .expect("backend report should be written");
    assert_toolchain_build_source_drops(&report);
    assert!(
        report.contains("permissions: 24")
            && report.contains("permission realizations: 24 (complete)")
            && !report.contains("UNLINKED")
            && !report.contains("INCOMPLETE"),
        "the multi-claim result ledger must remain complete\n{report}"
    );
    for claim in [
        "Main::issue::issue at statement 0 #0",
        "Main::issue::issue at statement 1 #1",
    ] {
        assert_eq!(
            report.matches(&format!("claim={claim},")).count(),
            8,
            "each callee-local claim must survive its caller-side path mapping\n{report}"
        );
    }

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn backend_report_preserves_direct_aggregate_state_result_mapping() {
    let canary = pass_canary("ownership/linear_aggregate_state_result");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-ownership-aggregate-state-result-canary-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation =
        compile_rooted_canary_for_native_host_with_auxiliary_artifacts(&canary, build_dir.clone())
            .expect("direct aggregate state result canary should compile from its authored root");
    assert_native_exit_code(&compilation, 0, "direct aggregate state result canary");

    let report = fs::read_to_string(build_dir.join("backend_report.txt"))
        .expect("backend report should be written");
    assert_toolchain_build_source_drops(&report);
    assert!(
        report.contains("permissions: 20")
            && report.contains("permission realizations: 20 (complete)")
            && !report.contains("UNLINKED")
            && !report.contains("INCOMPLETE"),
        "the aggregate-result permission ledger must remain complete\n{report}"
    );
    for claim in [
        "Main::issue::issue at statement 0 #0",
        "Main::issue::issue at statement 1 #1",
    ] {
        assert_eq!(
            report.matches(&format!("claim={claim},")).count(),
            6,
            "each constructor-field claim must survive its caller-side path mapping\n{report}"
        );
    }

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn checked_only_capability_canaries_compile_in_isolation() {
    // A focused guard for the checked authority-flow canaries, independent of
    // the batched `pass_canaries_compile` sweep (which also covers them).
    for canary_name in [
        "capabilities/uses_caller_folder",
        "capabilities/uses_caller_capability_requires",
    ] {
        let canary = pass_canary(canary_name);
        if let Err(diagnostics) = check_canary(&canary) {
            panic!(
                "expected capability canary {} to reach checked semantics, but got diagnostics:\n{}",
                canary.display(),
                diagnostics
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("\n")
            );
        }
    }
}

#[test]
fn signed_rat_metric_canaries_compile_in_isolation() {
    for canary_name in [
        "proofs/rat_metric_compile",
        "proofs/signed_rat_metric_compile",
        "proofs/cauchy_predicates_compile",
    ] {
        let canary = pass_canary(canary_name);
        if let Err(diagnostics) = check_canary(&canary) {
            panic!(
                "expected signed Rat canary {} to reach checked semantics, but got diagnostics:\n{}",
                canary.display(),
                diagnostics
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("\n")
            );
        }
    }
}

#[test]
fn checked_only_canaries_are_not_backend_umbrella_members() {
    for canary_name in CHECKED_ONLY_PASS_CANARIES {
        assert!(
            !ACTIVE_PASS_CANARIES.contains(canary_name),
            "checked-only pass canary `{canary_name}` must not also use backend entry discovery"
        );
        assert!(pass_canary(canary_name).join("main.omg").is_file());
    }
    for canary_name in CHECKED_ONLY_FAIL_CANARIES {
        assert!(
            !ACTIVE_FAIL_CANARIES.contains(canary_name),
            "checked-only fail canary `{canary_name}` must not also use backend entry discovery"
        );
        let canary = fail_canary(canary_name);
        assert!(canary.join("main.omg").is_file());
        assert!(canary.join("expected.txt").is_file());
    }
}

#[test]
fn float_meaning_core_surface_compiles_in_isolation() {
    let canary = pass_canary("core/float_meaning_core_surface");
    if let Err(diagnostics) = check_canary(&canary) {
        panic!(
            "expected FloatMeaning core canary {} to reach checked semantics, but got diagnostics:\n{}",
            canary.display(),
            diagnostics
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
}

#[test]
fn capability_manifest_reports_authority_flow_verbs() {
    for (canary_name, verb) in [
        ("capabilities/acquires_filesystem_authority", "acquires"),
        ("capabilities/stores_capability", "stores"),
    ] {
        let canary = pass_canary(canary_name);
        let build_dir = std::env::temp_dir().join(format!(
            "omega-capability-verb-canary-{}-{}",
            verb,
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&build_dir);

        let compilation = compile_with_auxiliary_artifacts(CanaryCompileSpec {
            root_path: canary.join("main.omg"),
            build_dir: Some(build_dir.clone()),
            target_name: None,
            product: CanaryCompileProduct::Check,
        })
        .unwrap_or_else(|diagnostics| {
            panic!(
                "{canary_name} should compile, got:\n{}",
                diagnostics
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        });
        assert!(!compilation.wrote_output());

        let manifest = fs::read_to_string(build_dir.join("05_capability_manifest.json"))
            .expect("capability manifest should be written");
        assert!(
            !manifest.contains(&format!("\"{verb}\": 0"))
                && manifest.contains(&format!("\"{verb}\":")),
            "manifest for {canary_name} should report a non-zero {verb} verb\n{manifest}"
        );

        let boundary = fs::read_to_string(build_dir.join("10_boundary.html"))
            .expect("boundary report should be written");
        assert!(
            boundary.contains("Capability Blast Radius")
                && boundary.contains("approved provider")
                && boundary.contains("authority is the capability value"),
            "boundary report for {canary_name} should surface capability-valued authority without a service-name projection\n{boundary}"
        );
        assert!(
            !boundary.contains("authority {filesystem_io")
                && !boundary.contains("authority {host_boundary"),
            "boundary report for {canary_name} must not render service names as authority\n{boundary}"
        );
        assert!(
            !boundary.contains("Boundary Providers") && !boundary.contains("boundary providers:"),
            "boundary report for {canary_name} must not resurrect the retired primitive-provider registry\n{boundary}"
        );

        let _ = fs::remove_dir_all(&build_dir);
    }
}

#[test]
fn capability_flows_retain_exact_direct_and_propagated_sites() {
    // Capability facts must follow returns/derives/acquires across nested calls,
    // not just direct boundary calls: a helper that mints or derives authority
    // and returns it flows the same verb up to its caller, and the boundary
    // report records the helper as provenance.
    for (canary_name, propagated_routes) in [
        (
            "capabilities/acquires_through_helper_return",
            // The second line shows the verb traveling a further call level: the
            // entry machine acquires through the mid-level helper, which acquired
            // through the boundary-touching helper.
            &[
                ("Backup::stage", "acquires", "Vault::pick"),
                ("Main::main", "acquires", "Backup::stage"),
            ][..],
        ),
        (
            "capabilities/derives_through_helper",
            &[("Worker::open_main_log", "derives", "Worker::open_log")][..],
        ),
    ] {
        let canary = pass_canary(canary_name);
        let build_dir = std::env::temp_dir().join(format!(
            "omega-capability-nested-canary-{}-{}",
            canary_name.rsplit('/').next().unwrap_or("canary"),
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&build_dir);

        let compilation = compile_with_auxiliary_artifacts(CanaryCompileSpec {
            root_path: canary.join("main.omg"),
            build_dir: Some(build_dir.clone()),
            target_name: None,
            product: CanaryCompileProduct::Check,
        })
        .unwrap_or_else(|diagnostics| {
            panic!(
                "{canary_name} should compile, got:\n{}",
                diagnostics
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        });
        assert!(!compilation.wrote_output());

        let boundary = fs::read_to_string(build_dir.join("10_boundary.html"))
            .expect("boundary report should be written");
        assert!(
            boundary.lines().any(|line| line.contains(" at statement ")
                && line.contains(" call ")
                && line.contains("[named-callable(path(")
                && line.ends_with(" direct")),
            "boundary report for {canary_name} should retain direct checked flow sites with exact owner overload identity\n{boundary}"
        );
        for (state, authority_flow, via_state) in propagated_routes {
            let site_prefix = format!("`{state}` [named-callable(path({state})");
            let route = format!(" {authority_flow} at statement ");
            let route_suffix = format!(" via `{via_state}` [named-callable(path({via_state})");
            assert!(
                boundary.lines().any(|line| {
                    line.contains(&site_prefix)
                        && line.contains(&route)
                        && line.contains(" call ")
                        && line.contains(&route_suffix)
                }),
                "boundary report for {canary_name} should retain exact propagated site \
                 `{site_prefix}…{route_suffix}…`\n{boundary}"
            );
        }

        let _ = fs::remove_dir_all(&build_dir);
    }
}

#[test]
fn unapproved_boundary_call_canary_is_rejected() {
    let canary = fail_canary("capabilities/unapproved_host_call");
    let diagnostics = match compile_canary_without_output(&canary) {
        Ok(report) => panic!(
            "expected unapproved boundary call canary to reject, but it compiled: {}",
            report.summary()
        ),
        Err(diagnostics) => diagnostics,
    };
    let combined = diagnostics
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined.contains("unapproved boundary call") && combined.contains("exact capability"),
        "expected exact boundary-provider approval diagnostic, got:\n{combined}"
    );
}

use compiler::{
    ArtifactEmissionPolicy, CompileOptions, CompileRequest, RequestedCompileProduct, compile,
};
use effects::provider_plan::ProviderBinding;
use std::fs;
use std::path::PathBuf;
use target::{ForeignLocatorCandidate, TargetProfile, normalize_foreign_locator};

struct Fixture {
    root: PathBuf,
    main: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "omega-terminal-import-custody-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("create Terminal import fixture");
        let main = root.join("main.omg");
        fs::write(
            &main,
            r#"use omega::language::core::external_binding;


boundary trait Console {
    machine write(value: u8);
}

windows_x86_64 machine write_binding() -> Binding<12, 11, 0> {
    Binding::DllImport {
        import: DllImport::PeByName {
            library: "kernel32.dll",
            export: "ExitProcess",
        },
    }
}

machine write_leaf(value: u8)
    satisfies Console::write
    via write_binding();

data Main {}
machine Main::main(&mut self) {}
"#,
        )
        .expect("write Terminal import source");
        fs::write(
            root.join("build.omg"),
            r#"machine build(builder: &mut Build) {
    builder.application("terminal-import-custody");
    builder.roots.bind(windows_x86_64::ProgramEntry, Main::main);
}
"#,
        )
        .expect("write Terminal import build policy");
        Self { root, main }
    }

    fn request(&self) -> CompileRequest {
        CompileRequest::new(CompileOptions {
            root_path: self.main.clone(),
            build_dir: Some(self.root.join("build")),
            target_name: Some("windows_x86_64".to_owned()),
        })
        .with_requested_product(RequestedCompileProduct::TerminalArtifact)
        .with_artifact_policy(ArtifactEmissionPolicy::OutputOnly)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn terminal_proposal_rejoins_every_evaluated_import_exactly_once() {
    let fixture = Fixture::new();
    let report = compile(fixture.request())
        .and_then(compiler::CompileOutcomes::into_single_report)
        .unwrap_or_else(|diagnostics| {
            panic!(
                "ordinary evaluated import should reach Terminal custody:\n{}",
                diagnostics
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        });
    let retained = report
        .into_retained_terminal_artifact()
        .expect("Terminal report retains the native proposal");
    let proposal = retained
        .native_realization_proposal()
        .expect("target-constrained Terminal product retains native custody");
    let selected_imports = proposal
        .selected_provider_plans()
        .plans()
        .iter()
        .flat_map(|plan| &plan.rows)
        .filter_map(|row| match &row.binding {
            ProviderBinding::Import { evaluated } => Some(evaluated),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [selected_import] = selected_imports.as_slice() else {
        panic!("one selected evaluated import expected");
    };
    assert_ne!(selected_import.receipt().identity_digest(), [0; 32]);
    let import_rows = proposal
        .external_binding_rows()
        .iter()
        .filter(|row| {
            matches!(
                row.binding,
                calling_conventions::ExternalBindingKind::Import { .. }
            )
        })
        .collect::<Vec<_>>();
    let [external_import] = import_rows.as_slice() else {
        panic!("one retained normalized external import expected");
    };
    let calling_conventions::ExternalBindingKind::Import { locator } = &external_import.binding
    else {
        unreachable!()
    };
    assert_eq!(locator, selected_import.locator());
    proposal
        .validate_for_artifact(retained.artifact())
        .expect("exact selected and external import rows rejoin");

    let rebuild = |rows| {
        compilation_report::TerminalNativeRealizationProposal::new(
            retained.artifact(),
            proposal.target_profile(),
            proposal.native_target(),
            proposal.subsystem(),
            proposal.application_intent(),
            proposal.application_identifier().cloned(),
            proposal.application_name().map(str::to_owned),
            proposal.post_terminal_optimizations().clone(),
            proposal.program_entry().clone(),
            proposal.checked_program_entry().clone(),
            proposal.selected_provider_plans().clone(),
            rows,
            proposal.package_terminal_authority_permissions().to_vec(),
            proposal.compiler_builtins().to_vec(),
            proposal.callback_occurrences().to_vec(),
            proposal.ieee_float_fma_occurrences().to_vec(),
            proposal.ieee_float_comparison_occurrences().to_vec(),
            proposal.boundary_application_demands().clone(),
            proposal.boundary_application_realizations().clone(),
            proposal.checked_boundary_operator_scope().clone(),
            proposal.behavior_exclusions().clone(),
        )
    };

    let mut missing = proposal.external_binding_rows().to_vec();
    missing.retain(|row| {
        !matches!(
            row.binding,
            calling_conventions::ExternalBindingKind::Import { .. }
        )
    });
    assert!(
        rebuild(missing).is_err(),
        "a missing import row must reject"
    );

    let mut duplicate = proposal.external_binding_rows().to_vec();
    duplicate.push((*external_import).clone());
    assert!(
        rebuild(duplicate).is_err(),
        "a duplicate import row must reject"
    );

    let mut substituted = proposal.external_binding_rows().to_vec();
    let changed_locator = normalize_foreign_locator(
        ForeignLocatorCandidate::PeByName {
            library: b"kernel32.dll".to_vec(),
            export: b"ExitThread".to_vec(),
        },
        TargetProfile::WindowsX64,
    )
    .expect("changed PE locator remains structurally valid");
    substituted
        .iter_mut()
        .find(|row| {
            matches!(
                row.binding,
                calling_conventions::ExternalBindingKind::Import { .. }
            )
        })
        .expect("mutable import row")
        .binding = calling_conventions::ExternalBindingKind::Import {
        locator: changed_locator,
    };
    assert!(
        rebuild(substituted).is_err(),
        "a locator substitution must reject"
    );

    let mut legacy = proposal.external_binding_rows().to_vec();
    legacy
        .iter_mut()
        .find(|row| {
            matches!(
                row.binding,
                calling_conventions::ExternalBindingKind::Import { .. }
            )
        })
        .expect("mutable import row")
        .binding = calling_conventions::ExternalBindingKind::StringBackedImportBootstrap {
        module: "kernel32.dll".to_owned(),
        symbol: "ExitProcess".to_owned(),
    };
    assert!(
        rebuild(legacy).is_err(),
        "a legacy string row cannot replace an evaluated import"
    );

    let mut unmatched = proposal.external_binding_rows().to_vec();
    let mut extra = (*external_import).clone();
    extra.method = "unmatched".to_owned();
    extra.requirement_identity = "unmatched::requirement".to_owned();
    unmatched.push(extra);
    assert!(
        rebuild(unmatched).is_err(),
        "an unmatched evaluated import row must reject"
    );
}

/// Compile one scratch program whose called `Leaf::exit` leaf is supplied by
/// the given declaration text, through the complete windows_x86_64 native
/// pipeline. `Ok` means native emission succeeded; `Err` carries every
/// rendered diagnostic.
fn compile_called_leaf(leaf_declaration: &str) -> Result<(), Vec<String>> {
    // Each call needs its own fixture directory: tests in this target run in
    // parallel and a process-wide path lets one test overwrite or delete the
    // other's source while its compile is still reading it.
    static NEXT_FIXTURE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let root = std::env::temp_dir().join(format!(
        "omega-called-leaf-{}-{}",
        std::process::id(),
        NEXT_FIXTURE.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create called-leaf fixture");
    let main = root.join("main.omg");
    fs::write(
        &main,
        format!(
            r#"use omega::language::core::external_binding;

boundary trait Leaf {{
    machine exit(code: i32) -> i32;
}}

{leaf_declaration}

data Main {{ p: Leaf; }}
machine Main::main(&mut self) reaches Leaf {{
    let rc: i32 = self.p.exit(70);
    let keep: i32 = rc;
}}
"#,
        ),
    )
    .expect("write called-leaf source");
    fs::write(
        root.join("build.omg"),
        r#"machine build(builder: &mut Build) {
    builder.application("called-leaf");
    builder.roots.bind(windows_x86_64::ProgramEntry, Main::main);
}
"#,
    )
    .expect("write called-leaf build policy");
    let request = CompileRequest::new(CompileOptions {
        root_path: main,
        build_dir: Some(root.join("build")),
        target_name: Some("windows_x86_64".to_owned()),
    })
    .with_requested_product(RequestedCompileProduct::NativeArtifact)
    .with_artifact_policy(ArtifactEmissionPolicy::OutputOnly);
    let outcome = compile(request).and_then(compiler::CompileOutcomes::into_single_report);
    let _ = fs::remove_dir_all(&root);
    outcome
        .map(|_| ())
        .map_err(|diagnostics| diagnostics.iter().map(ToString::to_string).collect())
}

/// A called evaluated `via` leaf keeps its exact normalized foreign identity
/// all the way to the terminal-authority boundary: the only refusal is the
/// missing independently admitted mechanism row, never a legacy fallback.
#[test]
fn called_evaluated_import_reaches_native_authority_as_normalized_foreign() {
    let diagnostics = compile_called_leaf(
        r#"windows_x86_64 machine exit_binding() -> Binding<12, 11, 0> {
    Binding::DllImport {
        import: DllImport::PeByName {
            library: "kernel32.dll",
            export: "ExitProcess",
        },
    }
}

machine leaf_exit(code: i32) -> i32 satisfies Leaf::exit via exit_binding();"#,
    )
    .expect_err(
        "a called evaluated import still requires independently admitted terminal authority",
    );
    let expected_locator = normalize_foreign_locator(
        ForeignLocatorCandidate::PeByName {
            library: b"kernel32.dll".to_vec(),
            export: b"ExitProcess".to_vec(),
        },
        TargetProfile::WindowsX64,
    )
    .expect("valid test locator");
    let expected_digest = format!("{:?}", expected_locator.identity_digest().as_bytes());
    assert!(
        diagnostics.iter().any(|message| {
            message.contains("does not classify normalized foreign mechanism")
                && message.contains(&expected_digest)
        }),
        "the called evaluated import must reach terminal-authority classification \
         as the exact normalized foreign mechanism: {diagnostics:?}"
    );
    assert!(
        !diagnostics
            .iter()
            .any(|message| message.contains("string-backed")),
        "an evaluated `via` row must never degrade to legacy string-backed \
         bootstrap: {diagnostics:?}"
    );
}

/// A called legacy `via Binding::DllImport("module", "symbol")` leaf is
/// refused at source admission: raw foreign strings are data, never binding
/// authority, so the retired magic spelling never enters the pipeline.
#[test]
fn called_legacy_string_backed_leaf_is_refused_at_source_admission() {
    let diagnostics = compile_called_leaf(
        r#"machine leaf_exit(code: i32) -> i32 satisfies Leaf::exit via Binding::DllImport("kernel32.dll", "ExitProcess");"#,
    )
    .expect_err("a called legacy leaf must not reach native emission");
    assert!(
        diagnostics.iter().any(|message| {
            message.contains("`Binding::DllImport(\"module\", \"symbol\")` is retired")
        }),
        "the called legacy leaf must be refused at source admission: \
         {diagnostics:?}"
    );
}

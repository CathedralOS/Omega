use compiler::{
    CheckedCompileRequest, CompileOptions, CompileRequest, RequestedCompileProduct, compile,
    compile_to_checked,
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
        // Every fixture in this target shares the process temp dir, so give
        // each one a unique suffix: parallel tests must not overwrite or
        // delete a source tree another compile is still reading.
        static NEXT_FIXTURE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "omega-terminal-import-custody-{}-{}",
            std::process::id(),
            NEXT_FIXTURE.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
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
            compilation_report::TerminalNativeRealizationInputs {
                target_profile: proposal.target_profile(),
                native_target: proposal.native_target(),
                subsystem: proposal.subsystem(),
                application_intent: proposal.application_intent(),
                application_identifier: proposal.application_identifier().cloned(),
                application_name: proposal.application_name().map(str::to_owned),
                post_terminal_optimizations: proposal.post_terminal_optimizations().clone(),
                program_entry: proposal.program_entry().clone(),
                checked_program_entry: proposal.checked_program_entry().clone(),
                selected_provider_plans: proposal.selected_provider_plans().clone(),
                external_binding_rows: rows,
                package_terminal_authority_permissions: proposal
                    .package_terminal_authority_permissions()
                    .to_vec(),
                compiler_builtins: proposal.compiler_builtins().to_vec(),
                callback_occurrences: proposal.callback_occurrences().to_vec(),
                ieee_float_fma_occurrences: proposal.ieee_float_fma_occurrences().to_vec(),
                ieee_float_comparison_occurrences: proposal
                    .ieee_float_comparison_occurrences()
                    .to_vec(),
                integer_comparison_occurrences: proposal.integer_comparison_occurrences().to_vec(),
                boundary_application_demands: proposal.boundary_application_demands().clone(),
                boundary_application_realizations: proposal
                    .boundary_application_realizations()
                    .clone(),
                checked_boundary_operator_scope: proposal.checked_boundary_operator_scope().clone(),
                behavior_exclusions: proposal.behavior_exclusions().clone(),
            },
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
    let fixture = CalledLeafFixture::new(leaf_declaration);
    compile_called_leaf_with_policy(&fixture, None)
}

/// The same called-leaf compile under one explicit receiving
/// mechanism-classification policy. `None` exercises the deny-by-absence
/// default of no explicit rows.
fn compile_called_leaf_with_policy(
    fixture: &CalledLeafFixture,
    terminal_authority_policy: Option<native_realization::TerminalAuthorityPolicy>,
) -> Result<(), Vec<String>> {
    let mut request = CompileRequest::new(CompileOptions {
        root_path: fixture.main.clone(),
        build_dir: Some(fixture.root.join("build")),
        target_name: Some("windows_x86_64".to_owned()),
    })
    .with_requested_product(RequestedCompileProduct::NativeArtifact);
    if let Some(policy) = terminal_authority_policy {
        request = request.with_terminal_authority_policy(policy);
    }
    let outcome = compile(request).and_then(compiler::CompileOutcomes::into_single_report);
    outcome
        .map(|_| ())
        .map_err(|diagnostics| diagnostics.iter().map(ToString::to_string).collect())
}

/// Scratch project whose `Main::main` calls the `Leaf::exit` boundary. The
/// returned value keeps the source tree alive for both the checked read that
/// derives owner policy rows and the subsequent full native compile.
struct CalledLeafFixture {
    root: PathBuf,
    main: PathBuf,
}

impl CalledLeafFixture {
    fn new(leaf_declaration: &str) -> Self {
        // Each call needs its own fixture directory: tests in this target run
        // in parallel and a process-wide path lets one test overwrite or
        // delete the other's source while its compile is still reading it.
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
        Self { root, main }
    }
}

impl Drop for CalledLeafFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

const CALLED_LEAF_DECLARATION: &str = r#"windows_x86_64 machine exit_binding() -> Binding<12, 11, 0> {
    Binding::DllImport {
        import: DllImport::PeByName {
            library: "kernel32.dll",
            export: "ExitProcess",
        },
    }
}

machine leaf_exit(code: i32) -> i32 satisfies Leaf::exit via exit_binding();"#;

/// Rebuild the exact normalized-foreign mechanism rows an owner accepts for
/// one fixture root: each retained external binding's own normalized locator
/// and canonical boundary-entry plan, never its readable source spelling.
fn fixture_foreign_policy(
    main: &std::path::Path,
    disposition: effects::TerminalAuthorityDisposition,
) -> native_realization::TerminalAuthorityPolicy {
    let checked = compile_to_checked(CheckedCompileRequest::new(main, Some("windows_x86_64")))
        .expect("fixture reaches checked trees");
    let rows = checked
        .external_binding_rows()
        .iter()
        .filter_map(|row| {
            let calling_conventions::ExternalBindingKind::Import { locator } = &row.binding else {
                return None;
            };
            let boundary_entry_plan = row
                .boundary_entry_plan
                .as_ref()
                .expect("retained import row carries its admitted boundary plan");
            Some(native_realization::TerminalAuthorityPolicyRow::new(
                native_realization::normalized_foreign_terminal_mechanism(
                    locator,
                    boundary_entry_plan,
                )
                .expect("retained foreign boundary plan is canonical"),
                disposition.clone(),
            ))
        })
        .collect();
    native_realization::terminal_authority_policy_with_rows(rows)
        .expect("exact normalized-foreign rows form a valid receiving policy")
}

/// A called evaluated `via` leaf keeps its exact normalized foreign identity
/// all the way to the terminal-authority boundary: the only refusal is the
/// missing independently admitted mechanism row, never a legacy fallback.
#[test]
fn called_evaluated_import_reaches_native_authority_as_normalized_foreign() {
    let diagnostics = compile_called_leaf(CALLED_LEAF_DECLARATION).expect_err(
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

/// One exact owner-supplied mechanism row classifies the demanded leaf, so
/// the refusal moves past classification to the next independent axis:
/// provider-execution and same-stack custody, which a receiving policy never
/// manufactures.
#[test]
fn called_evaluated_import_with_exact_policy_row_reaches_execution_custody() {
    let fixture = CalledLeafFixture::new(CALLED_LEAF_DECLARATION);
    let policy = fixture_foreign_policy(
        &fixture.main,
        effects::TerminalAuthorityDisposition::from_classes([
            effects::TerminalAuthorityClass::ProcessTermination,
        ]),
    );
    assert_eq!(
        policy.explicit_rows().len(),
        1,
        "the fixture retains exactly one normalized-foreign mechanism"
    );
    let diagnostics = compile_called_leaf_with_policy(&fixture, Some(policy))
        .expect_err("classification authority alone cannot admit provider-execution custody");
    assert!(
        diagnostics
            .iter()
            .any(|message| message.contains("has no admitted native settlement")),
        "with its exact mechanism row accepted, the called import must reach \
         the provider-execution custody gate: {diagnostics:?}"
    );
    assert!(
        !diagnostics
            .iter()
            .any(|message| message.contains("does not classify")),
        "the accepted exact row must classify the demanded mechanism: {diagnostics:?}"
    );
}

/// A policy row keyed by a different locator is not the demanded mechanism:
/// substitution still rejects at classification with the demanded identity,
/// never by similarity of library or provider names.
#[test]
fn called_evaluated_import_with_substituted_policy_row_still_rejects() {
    let fixture = CalledLeafFixture::new(CALLED_LEAF_DECLARATION);
    let checked = compile_to_checked(CheckedCompileRequest::new(
        &fixture.main,
        Some("windows_x86_64"),
    ))
    .expect("called-leaf fixture reaches checked trees");
    let externals = checked
        .external_binding_rows()
        .iter()
        .filter(|row| {
            matches!(
                row.binding,
                calling_conventions::ExternalBindingKind::Import { .. }
            )
        })
        .collect::<Vec<_>>();
    let [external] = externals.as_slice() else {
        panic!("one retained normalized external import expected")
    };
    let boundary_entry_plan = external
        .boundary_entry_plan
        .as_ref()
        .expect("retained import row carries its admitted boundary plan");
    let substituted_locator = normalize_foreign_locator(
        ForeignLocatorCandidate::PeByName {
            library: b"kernel32.dll".to_vec(),
            export: b"ExitThread".to_vec(),
        },
        TargetProfile::WindowsX64,
    )
    .expect("substituted locator remains structurally valid");
    let policy = native_realization::terminal_authority_policy_with_rows(vec![
        native_realization::TerminalAuthorityPolicyRow::new(
            native_realization::normalized_foreign_terminal_mechanism(
                &substituted_locator,
                boundary_entry_plan,
            )
            .expect("substituted mechanism is still canonical"),
            effects::TerminalAuthorityDisposition::from_classes([
                effects::TerminalAuthorityClass::ProcessTermination,
            ]),
        ),
    ])
    .expect("a substituted-locator row still forms a valid policy");
    let diagnostics = compile_called_leaf_with_policy(&fixture, Some(policy))
        .expect_err("a substituted locator row must not classify the demanded mechanism");
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
        "a substituted policy row must leave the demanded mechanism \
         unclassified under its exact identity: {diagnostics:?}"
    );
}

/// A selected but never-called evaluated import retains its typed identity
/// without demanding classification or execution inputs; the same compile
/// also accepts the owner row for that mechanism as an unused explicit row.
#[test]
fn uncalled_evaluated_import_compiles_with_or_without_policy_row() {
    for with_policy in [false, true] {
        let fixture = Fixture::new();
        let mut request = CompileRequest::new(CompileOptions {
            root_path: fixture.main.clone(),
            build_dir: Some(fixture.root.join("build-native")),
            target_name: Some("windows_x86_64".to_owned()),
        })
        .with_requested_product(RequestedCompileProduct::NativeArtifact);
        if with_policy {
            request = request.with_terminal_authority_policy(fixture_foreign_policy(
                &fixture.main,
                effects::TerminalAuthorityDisposition::from_classes([]),
            ));
        }
        compile(request)
            .and_then(compiler::CompileOutcomes::into_single_report)
            .unwrap_or_else(|diagnostics| {
                panic!(
                    "an uncalled evaluated import keeps identity without execution \
                     inputs (policy supplied: {with_policy}):\n{}",
                    diagnostics
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join("\n")
                )
            });
    }
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

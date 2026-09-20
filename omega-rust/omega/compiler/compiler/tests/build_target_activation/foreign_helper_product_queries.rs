use super::{TempProject, application_build, foreign_helper_inputs, foreign_product_inputs};
use compiler::{
    CheckedCompileRequest, CompileOptions, CompileRequest, RequestedCompileProduct, compile,
    compile_to_checked,
};
use std::fs;

fn helper_entry_inputs(
    project: &TempProject,
    helper: &TempProject,
) -> package_compilation::PackageCompilationInputs {
    // Executing configure and selecting launch need separate checked frontiers.
    // A build edge alone does not put the helper's entry into the product.
    package_compilation::PackageCompilationInputs::new(
        super::package_identity(1),
        package_compilation::BuildDeclarationKind::Application,
        vec![
            package_compilation::PackageSourceBinding::new(
                super::package_identity(1),
                "root-binding-owner",
                project.0.clone(),
            ),
            package_compilation::PackageSourceBinding::new(
                super::package_identity(2),
                "root-binding-helper",
                helper.0.clone(),
            ),
        ],
        [
            build_declarations::DependencyPurpose::Build,
            build_declarations::DependencyPurpose::Product,
        ]
        .into_iter()
        .map(|purpose| {
            package_compilation::PackageDependencyBinding::for_purpose(
                super::package_identity(1),
                "support",
                super::package_identity(2),
                purpose,
            )
        })
        .collect(),
    )
    .expect("explicit build and product dependencies")
}

fn dual_context_product_query(
    build_body: &str,
    build_only_declarations: bool,
) -> Result<compiler::CheckedCompilation, Vec<diagnostics::Diagnostic>> {
    let helper = TempProject::new(
        "machine build(builder: &mut Build) { builder.package(\"dual-context-helper\"); }",
    );
    // A provider description names nominal data with a conformance; querying
    // it does not select or execute a boundary implementation.
    let declarations = "module setup;
        pub machine launch() { }
        pub data LaunchConfig { value: u8; }
        pub trait Pick { machine choose() -> i32; }
        pub data AudioProvider { }
        pub machine AudioProvider::choose() -> i32 satisfies Pick::choose {
            transition { _ -> (1) }
        }";
    fs::write(helper.0.join("setup.omg"), declarations).expect("shared helper source");
    fs::write(
        helper.0.join("product.omg"),
        "module product; pub data ProductOnly { }",
    )
    .expect("independent product source");
    let product_import = if build_only_declarations {
        "product"
    } else {
        "setup"
    };
    let project = TempProject::with_main(
        &format!("use support::{product_import};"),
        &format!(
            "use support::setup; machine build(builder: &mut Build) {{ builder.application(\"dual-context-owner\"); {build_body} }}"
        ),
    );
    let mut request = CheckedCompileRequest::new(&project.main(), Some("windows_x86_64"));
    request.package_inputs = Some(
        package_compilation::PackageCompilationInputs::new(
            super::package_identity(1),
            package_compilation::BuildDeclarationKind::Application,
            vec![
                package_compilation::PackageSourceBinding::new(
                    super::package_identity(1),
                    "dual-context-owner",
                    project.0.clone(),
                ),
                package_compilation::PackageSourceBinding::new(
                    super::package_identity(2),
                    "dual-context-helper",
                    helper.0.clone(),
                ),
            ],
            [
                build_declarations::DependencyPurpose::Build,
                build_declarations::DependencyPurpose::Product,
            ]
            .into_iter()
            .map(|purpose| {
                package_compilation::PackageDependencyBinding::for_purpose(
                    super::package_identity(1),
                    "support",
                    super::package_identity(2),
                    purpose,
                )
            })
            .collect(),
        )
        .expect("distinct build and product edges to the same package"),
    );
    compile_to_checked(request)
}

#[test]
fn product_entry_query_distinguishes_two_checked_instances_of_one_dependency() {
    let checked = dual_context_product_query(
        "let entry: ProductEntryRef = builder.product.entry(\"support::setup::launch\", \"windows_x86_64::ProgramEntry\"); builder.roots.bind(windows_x86_64::ProgramEntry, entry);",
        false,
    ).expect("entry query selects only the product checked instance");
    let entry = checked
        .selected_program_entry()
        .expect("selected product entry");
    let symbol = entry.source_signature().machine_symbol();
    let program = &checked;
    let span = program
        .symbols
        .symbol_source_span(symbol)
        .expect("entry declaration");
    assert_eq!(
        program
            .symbols
            .source_file(span)
            .expect("entry source")
            .dependency_scope,
        source::DependencyScope::Product
    );
}

#[test]
fn product_schema_query_distinguishes_two_checked_instances_of_one_dependency() {
    let checked = dual_context_product_query(
        "let schema: ProductTypeSchema = builder.product.schema(\"support::setup::LaunchConfig\"); builder.log.write_line(schema.path());",
        false,
    ).expect("schema query selects only the product checked instance");
    assert_eq!(
        checked
            .build_observation_summary()
            .expect("query observation")
            .build_log(),
        b"setup::LaunchConfig\n"
    );
}

#[test]
fn product_provider_query_distinguishes_two_checked_instances_of_one_dependency() {
    let checked = dual_context_product_query(
        "let provider: ProductProviderRef = builder.product.provider(\"support::setup::AudioProvider\"); builder.log.write_line(provider.path());",
        false,
    ).expect("provider query selects only the product checked instance");
    assert_eq!(
        checked
            .build_observation_summary()
            .expect("query observation")
            .build_log(),
        b"setup::AudioProvider\n"
    );
}

#[test]
fn product_queries_cannot_select_build_only_declarations_through_a_product_edge() {
    for (query, rejection) in [
        (
            "let entry: ProductEntryRef = builder.product.entry(\"support::setup::launch\", \"windows_x86_64::ProgramEntry\"); builder.roots.bind(windows_x86_64::ProgramEntry, entry);",
            "not a product declaration visible",
        ),
        (
            "let schema: ProductTypeSchema = builder.product.schema(\"support::setup::LaunchConfig\"); builder.log.write_line(schema.path());",
            "not a product declaration visible",
        ),
        (
            "let provider: ProductProviderRef = builder.product.provider(\"support::setup::AudioProvider\"); builder.log.write_line(provider.path());",
            "not a provider declaration visible",
        ),
    ] {
        let Err(diagnostics) = dual_context_product_query(query, true) else {
            panic!("a product edge must not authorize build-context declarations: {query}");
        };
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(rejection)),
            "{diagnostics:?}"
        );
    }
}

#[test]
fn borrowing_build_does_not_lend_private_product_names_to_a_foreign_helper() {
    let helper = TempProject::new(
        "machine build(builder: &mut Build) { builder.package(\"root-binding-helper\"); }",
    );
    fs::write(helper.0.join("setup.omg"),
        "module setup; pub machine configure(builder: &mut Build) { builder.roots.bind(windows_x86_64::ProgramEntry, launch); }",
    ).expect("helper source");
    let project = TempProject::with_main(
        "machine launch() { let marker: u8 = 0; }",
        "use support::setup; machine build(builder: &mut Build) { builder.application(\"root-binding-owner\"); setup::configure(builder); }",
    );
    let mut request = CheckedCompileRequest::new(&project.main(), Some("windows_x86_64"));
    request.package_inputs = Some(foreign_helper_inputs(&project, &helper));
    let diagnostics =
        compile_to_checked(request).expect_err("a helper cannot select its caller's private entry");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("not a product declaration visible from this build source's package")),
        "{diagnostics:?}"
    );
}

#[test]
fn foreign_helper_binds_its_own_package_entry_through_borrowed_root_build() {
    let helper = TempProject::new(
        "machine build(builder: &mut Build) { builder.package(\"root-binding-helper\"); }",
    );
    fs::write(
        helper.0.join("setup.omg"),
        "module setup; pub machine launch() { let marker: u8 = 0; }",
    )
    .expect("product entry source");
    fs::write(helper.0.join("configure.omg"),
        "module configure; pub machine configure(builder: &mut Build) { builder.roots.bind(windows_x86_64::ProgramEntry, launch); }",
    ).expect("build helper source");
    let project = TempProject::with_main(
        "use support::setup; const ANSWER: u32 = 42;\n",
        "use support::configure; machine build(builder: &mut Build) { builder.application(\"root-binding-owner\"); configure::configure(builder); }",
    );
    let mut request = CheckedCompileRequest::new(&project.main(), Some("windows_x86_64"));
    request.package_inputs = Some(helper_entry_inputs(&project, &helper));
    let checked = compile_to_checked(request)
        .expect("a helper may bind its own package's entry through the borrowed root Build");
    assert_eq!(checked.selected_program_entry_machine(), Some("launch"));
    let selected = checked
        .selected_program_entry()
        .expect("entry")
        .source_signature()
        .machine_symbol();
    assert_eq!(
        checked.symbols.symbol_product_package_identity(selected),
        Some(super::package_identity(2))
    );
}

#[test]
fn same_named_entry_in_another_package_rejoins_production_and_settlement_by_symbol() {
    let helper = TempProject::new(
        "machine build(builder: &mut Build) { builder.package(\"root-binding-helper\"); }",
    );
    fs::write(
        helper.0.join("setup.omg"),
        "module setup; pub machine launch() { let marker: u8 = 0; }",
    )
    .expect("product entry source");
    fs::write(helper.0.join("configure.omg"),
        "module configure; pub machine configure(builder: &mut Build) { builder.roots.bind(windows_x86_64::ProgramEntry, launch); }",
    ).expect("build helper source");
    let project = TempProject::with_main(
        "use support::setup; machine launch() { let marker: u8 = 0; }",
        "use support::configure; machine build(builder: &mut Build) { builder.application(\"root-binding-owner\"); configure::configure(builder); }",
    );
    let mut request = CheckedCompileRequest::new(&project.main(), Some("windows_x86_64"));
    request.package_inputs = Some(helper_entry_inputs(&project, &helper));
    let checked = compile_to_checked(request)
        .expect("a same-named foreign entry binds by exact symbol, not by name");
    assert_eq!(checked.selected_program_entry_machine(), Some("launch"));
    let entry = checked
        .selected_program_entry()
        .cloned()
        .expect("one exact selected ProgramEntry");
    assert_eq!(
        checked
            .symbols
            .symbol_product_package_identity(entry.source_signature().machine_symbol()),
        Some(super::package_identity(2))
    );
    let native_target = checked
        .selected_native_target()
        .expect("one selected native target");
    let psi_optimizations = checked
        .optimization_selections()
        .project_psi()
        .selections()
        .clone();
    let program = checked.into_program();
    // The lexical binding chose the helper's `setup::launch`, not the owner's
    // free `launch`: the exact symbol resolves to the helper package machine.
    let helper_launch = program
        .typed
        .machines()
        .iter()
        .find(|machine| program.typed.symbols.display_path(machine.symbol, "::") == "setup::launch")
        .expect("the helper package declares setup::launch");
    assert_eq!(
        entry.source_signature().machine_symbol(),
        helper_launch.symbol
    );
    let produced = terminal_production::TerminalProductionRequest {
        checked: &program,
        machine: terminal_production::TerminalMachineSelection::Symbol(
            entry.source_signature().machine_symbol(),
        ),
        optimization_selections: psi_optimizations,
    }
    .produce_program_entry(entry.source_signature().identity().bytes())
    .expect("Terminal production rejoins the selected machine by exact symbol");
    assert_eq!(
        produced.receipt().source_machine_name(),
        "setup::launch",
        "the receipt keeps the qualified display name for diagnostics"
    );
    assert_eq!(
        produced.receipt().source_machine_symbol(),
        entry.source_signature().machine_symbol(),
        "the receipt carries the exact selected machine symbol"
    );
    let calling_plans = entry.calling_plans().map(|plans| {
        (
            &plans.semantic_calling_application,
            &plans.physical_calling_application,
            &plans.storage_entry,
        )
    });
    native_realization::validate_native_program_entry_settlement(
        produced.artifact(),
        produced.receipt(),
        native_realization::NativeProgramEntrySettlement::new(
            entry.source_signature(),
            calling_plans,
            entry.fused_service_establishments(),
        ),
        native_target,
    )
    .expect("native entry settlement rejoins the exact selected machine symbol");

    // The retained product path selects the same exact symbol: the compiler's
    // own `TerminalProductionRequest` must not fall back to the name spelling.
    let report = compile(
        CompileRequest::new(CompileOptions {
            root_path: project.main(),
            build_dir: None,
            target_name: Some("windows_x86_64".into()),
        })
        .with_requested_product(RequestedCompileProduct::TerminalArtifact)
        .with_package_inputs(helper_entry_inputs(&project, &helper)),
    )
    .and_then(compiler::CompileOutcomes::into_single_report)
    .unwrap_or_else(|diagnostics| {
        panic!("same-named foreign entry reaches retained Terminal production: {diagnostics:#?}")
    });
    let retained = report
        .into_retained_terminal_artifact()
        .expect("Terminal report retains canonical artifact custody");
    retained
        .validate()
        .expect("same-named retained Terminal artifact verifies");
    assert_eq!(
        retained
            .native_realization_proposal()
            .expect("Terminal report retains the native proposal")
            .program_entry()
            .source_signature()
            .machine_symbol(),
        helper_launch.symbol,
        "the retained proposal keeps the helper package's exact entry symbol"
    );
}

#[test]
fn owner_selected_product_description_binds_through_foreign_helper() {
    // The owner selects its own private entry through the compiler-owned
    // `product.entry` query and hands the restricted description to a helper
    // in another package. The helper binds it without holding any product
    // namespace of its own; the target machine traps if it were ever executed
    // during selection, so a clean compile also witnesses non-execution.
    let helper = TempProject::new(
        "machine build(builder: &mut Build) { builder.package(\"root-binding-helper\"); }",
    );
    fs::write(helper.0.join("setup.omg"),
        "module setup; pub machine configure(builder: &mut Build, entry: &ProductEntryRef) { builder.roots.bind(windows_x86_64::ProgramEntry, entry); }",
    ).expect("helper source");
    let project = TempProject::with_main(
        "machine launch() { crash Trap; }",
        "use support::setup; machine build(builder: &mut Build) { builder.application(\"root-binding-owner\"); let entry: ProductEntryRef = builder.product.entry(\"launch\", \"windows_x86_64::ProgramEntry\"); setup::configure(builder, &entry); }",
    );
    let mut request = CheckedCompileRequest::new(&project.main(), Some("windows_x86_64"));
    request.package_inputs = Some(foreign_helper_inputs(&project, &helper));
    let checked = compile_to_checked(request).expect(
        "an owner-selected product description binds through a helper without executing the target",
    );
    assert_eq!(checked.selected_program_entry_machine(), Some("launch"));
}

#[test]
fn returned_owner_selected_description_binds_and_executes_after_foreign_helper_transport() {
    returned_description_native(
        "let returned: ProductEntryRef = setup::retain(entry);",
        "returned",
        b"",
    );
}

#[test]
fn inline_owner_selected_description_binds_and_executes_after_foreign_helper_transport() {
    returned_description_native("", "setup::retain(entry)", b"");
}

#[test]
fn inline_description_nested_calls_execute_their_build_effects_once() {
    returned_description_native(
        "",
        "setup::retain_logged(builder, setup::retain(entry))",
        b"operand\n",
    );
}

#[test]
fn inline_description_specialized_call_reaches_native_execution() {
    returned_description_native("", "setup::retain_generic<u8>(entry, 7)", b"");
}

fn returned_description_native(preparation: &str, operand: &str, expected_log: &[u8]) {
    let Some(profile) = target::TargetProfile::host_if_supported() else {
        eprintln!("SKIP: returned entry publication requires a supported hosted target");
        return;
    };
    let helper = TempProject::new(
        "machine build(builder: &mut Build) { builder.package(\"root-binding-helper\"); }",
    );
    fs::write(helper.0.join("setup.omg"),
        "module setup; pub machine retain(entry: ProductEntryRef) -> ProductEntryRef { transition { _ -> (entry) } }
         pub machine retain_logged(builder: &mut Build, entry: ProductEntryRef) -> ProductEntryRef {
             builder.log.write_line(\"operand\"); transition { _ -> (entry) }
         }
         pub machine retain_generic<T [copy]>(entry: ProductEntryRef, marker: T) -> ProductEntryRef {
             transition { _ -> (entry) }
         }",
    ).expect("ordinary owned-description helper");
    let slot = format!("{}::ProgramEntry", profile.root_slot_owner_name());
    let project = TempProject::with_main(
        "machine launch() { }",
        &format!(
            "use support::setup; machine build(builder: &mut Build) {{ builder.application(\"returned-entry\"); let entry: ProductEntryRef = builder.product.entry(\"launch\", \"{slot}\"); {preparation} builder.roots.bind({slot}, {operand}); }}"
        ),
    );
    let checked = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(foreign_helper_inputs(&project, &helper)),
        ..CheckedCompileRequest::new(&project.main(), Some(profile.target_name()))
    })
    .expect("description build checks");
    assert_eq!(
        checked
            .build_observation_summary()
            .expect("build observation")
            .build_log(),
        expected_log
    );
    let report = compile(
        CompileRequest::new(CompileOptions {
            root_path: project.main(),
            build_dir: None,
            target_name: Some(profile.target_name().into()),
        })
        .with_package_inputs(foreign_helper_inputs(&project, &helper))
        .with_requested_product(RequestedCompileProduct::NativeArtifact),
    )
    .and_then(compiler::CompileOutcomes::into_single_report)
    .expect("an ordinary return preserves a compiler-issued description");
    let published = report
        .publish_retained_native_artifact(&project.0.join("out"))
        .expect("returned entry publication");
    let output = std::process::Command::new(
        published
            .checked_native_executable_path()
            .expect("checked executable"),
    )
    .output()
    .expect("execute owner entry");
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
}

#[test]
fn returned_forged_description_does_not_gain_authority_from_its_result_type() {
    for binding in [
        "let entry: ProductEntryRef = setup::fabricate(); builder.roots.bind(windows_x86_64::ProgramEntry, entry);",
        "builder.roots.bind(windows_x86_64::ProgramEntry, setup::fabricate());",
    ] {
        let helper = TempProject::new(
            "machine build(builder: &mut Build) { builder.package(\"root-binding-helper\"); }",
        );
        fs::write(helper.0.join("setup.omg"),
        "module setup; pub machine fabricate() -> ProductEntryRef { transition { _ -> (ProductEntryRef {}) } }",
    ).expect("forged result source");
        let project = TempProject::new(&format!(
            "use support::setup; machine build(builder: &mut Build) {{ builder.application(\"forged-return\"); {binding} }}"
        ));
        let mut request = CheckedCompileRequest::new(&project.main(), Some("windows_x86_64"));
        request.package_inputs = Some(foreign_helper_inputs(&project, &helper));
        let diagnostics = compile_to_checked(request)
            .expect_err("an ordinary return cannot fabricate selection authority");
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("not a compiler-issued product entry description")),
            "{diagnostics:?}"
        );
    }
}

#[test]
fn product_entry_query_is_scoped_to_the_query_occurrences_package() {
    // The helper performs the query itself: `launch` exists only in the
    // owner's package, so the borrowed Build cannot reach it. Selection
    // authority belongs to the query occurrence's package, not the caller's.
    let helper = TempProject::new(
        "machine build(builder: &mut Build) { builder.package(\"root-binding-helper\"); }",
    );
    fs::write(helper.0.join("setup.omg"),
        "module setup; pub machine configure(builder: &mut Build) { let entry: ProductEntryRef = builder.product.entry(\"launch\", \"windows_x86_64::ProgramEntry\"); builder.roots.bind(windows_x86_64::ProgramEntry, entry); }",
    ).expect("helper source");
    let project = TempProject::with_main(
        "machine launch() { let marker: u8 = 0; }",
        "use support::setup; machine build(builder: &mut Build) { builder.application(\"root-binding-owner\"); setup::configure(builder); }",
    );
    let mut request = CheckedCompileRequest::new(&project.main(), Some("windows_x86_64"));
    request.package_inputs = Some(foreign_helper_inputs(&project, &helper));
    let diagnostics = compile_to_checked(request)
        .expect_err("a foreign helper cannot select the caller's private product entry")
        .into_iter()
        .map(|diagnostic| diagnostic.message)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        diagnostics
            .contains("not a product declaration visible from this build occurrence's package"),
        "unexpected diagnostics: {diagnostics}"
    );
}

fn inline_description_checked(
    declarations: &str,
    preparation: &str,
    operand: &str,
) -> Result<compiler::CheckedCompilation, Vec<diagnostics::Diagnostic>> {
    let helper = TempProject::new(
        "machine build(builder: &mut Build) { builder.package(\"inline-helper\"); }",
    );
    fs::write(
        helper.0.join("setup.omg"),
        format!("module setup; {declarations}"),
    )
    .expect("inline operand helper source");
    let project = TempProject::with_main(
        "machine launch() { }",
        &format!("use support::setup; machine build(builder: &mut Build) {{
            builder.application(\"inline-owner\");
            let entry: ProductEntryRef = builder.product.entry(\"launch\", \"windows_x86_64::ProgramEntry\");
            {preparation}
            builder.roots.bind(windows_x86_64::ProgramEntry, {operand});
        }}"),
    );
    compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(foreign_helper_inputs(&project, &helper)),
        ..CheckedCompileRequest::new(&project.main(), Some("windows_x86_64"))
    })
}

#[test]
fn inline_description_calls_preserve_required_premises() {
    let helper = "pub machine retain(entry: ProductEntryRef, value: i32) -> ProductEntryRef
        requires value > 0 { transition { _ -> (entry) } }";
    inline_description_checked(helper, "", "setup::retain(entry, 1)")
        .expect("established call premise");
    let errors = inline_description_checked(helper, "", "setup::retain(entry, 0)")
        .map(|_| ())
        .expect_err("inline operand cannot bypass a call requirement");
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("requires")),
        "{errors:?}"
    );
}

#[test]
fn inline_description_calls_preserve_affine_argument_consumption() {
    let helper = "pub data Token { }
        pub machine consume(entry: ProductEntryRef, token: Token) -> ProductEntryRef {
            transition { _ -> (entry) }
        }";
    let preparation = "let token: setup::Token = setup::Token {};";
    inline_description_checked(helper, preparation, "setup::consume(entry, token)")
        .expect("one affine transfer");
    let errors = inline_description_checked(
        helper,
        &format!("{preparation} let first: ProductEntryRef = setup::consume(entry, token);"),
        "setup::consume(entry, token)",
    )
    .map(|_| ())
    .expect_err("the inline operand cannot spend its affine argument twice");
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("mov") || error.message.contains("consum")),
        "{errors:?}"
    );
}

#[test]
fn inline_description_arguments_preserve_construction_bounds() {
    let helper = "pub data Payload [copy] { value: u8 [1..=9]; }
        pub machine retain(entry: ProductEntryRef, payload: Payload) -> ProductEntryRef {
            transition { _ -> (entry) }
        }";
    inline_description_checked(
        helper,
        "",
        "setup::retain(entry, setup::Payload { value: 1 })",
    )
    .expect("established construction bounds");
    let errors = inline_description_checked(
        helper,
        "",
        "setup::retain(entry, setup::Payload { value: 0 })",
    )
    .map(|_| ())
    .expect_err("inline argument construction must establish its fields");
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("range") || error.message.contains("bound")),
        "{errors:?}"
    );
}

#[test]
fn forged_product_entry_description_is_rejected() {
    // An authored `ProductEntryRef {}` has the static type but carries no
    // compiler-issued description payload, so the bind refuses it at
    // evaluation rather than trusting the shape.
    let project = TempProject::new(&application_build(
        "    let forged: ProductEntryRef = ProductEntryRef {};\n    builder.roots.bind(windows_x86_64::ProgramEntry, forged);",
    ));
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(
        &project.main(),
        Some("windows_x86_64"),
    ))
    .expect_err("an authored ProductEntryRef value is not a product description")
    .into_iter()
    .map(|diagnostic| diagnostic.message)
    .collect::<Vec<_>>()
    .join("\n");
    assert!(
        diagnostics.contains("not a compiler-issued product entry description"),
        "unexpected diagnostics: {diagnostics}"
    );
}

#[test]
fn inline_description_calls_require_blocking_acknowledgement() {
    let helper = "pub machine retain(entry: ProductEntryRef) -> ProductEntryRef blocks; {
        transition { _ -> (entry) }
    }";
    inline_description_checked(helper, "", "block setup::retain(entry)")
        .expect("acknowledged blocking operand");
    let errors = inline_description_checked(helper, "", "setup::retain(entry)")
        .map(|_| ())
        .expect_err("an inline operand is still a blocking call");
    assert!(
        errors.iter().any(|error| error.message.contains("block")),
        "{errors:?}"
    );
}

#[test]
fn inline_description_arguments_establish_nominal_field_domains() {
    let helper = "pub domain u8::Positive requires self > 0;
        pub data Payload [copy] { value: u8 in u8::Positive; }
        pub machine retain(entry: ProductEntryRef, payload: Payload) -> ProductEntryRef {
            transition { _ -> (entry) }
        }";
    inline_description_checked(
        helper,
        "let value: u8 = 1;",
        "setup::retain(entry, setup::Payload { value: value })",
    )
    .expect("flow-proven field domain");
    let errors = inline_description_checked(
        helper,
        "let value: u8 = 0;",
        "setup::retain(entry, setup::Payload { value: value })",
    )
    .map(|_| ())
    .expect_err("description argument construction cannot assert an unproven domain");
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("domain") || error.message.contains("prove")),
        "{errors:?}"
    );
}

#[test]
fn inline_description_match_subject_establishes_aggregate_field_domains() {
    let helper = "pub domain [u8; 4]::Ascii requires ascii_only(self);
        pub data Payload [copy] { bytes: [u8; 4] in Ascii; }";
    let operand = |text| format!("match (setup::Payload {{ bytes: \"{text}\" }}) {{ _ -> entry }}");
    inline_description_checked(helper, "", &operand("AAAA"))
        .expect("a checked match subject can precede description selection");
    let errors = inline_description_checked(helper, "", &operand("éAA"))
        .map(|_| ())
        .expect_err("a match subject cannot hide a malformed field qualification");
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("domain") || error.message.contains("prove")),
        "{errors:?}"
    );
}

#[test]
fn delegated_root_binding_requires_a_product_entry_ref_operand() {
    let errors = inline_description_checked("", "", "12")
        .map(|_| ())
        .expect_err("parsing a value expression does not make it a product description");
    assert!(
        errors.iter().any(|error| {
            error
                .message
                .contains("not a compiler-issued product entry description")
                || error.message.contains("ProductEntryRef")
        }),
        "{errors:?}"
    );
    // A typed-but-undescribed local cannot mint selection authority: it is not
    // a compiler-issued description, so the bind refuses it. Build evaluation
    // runs before checked authored selections finalize, so the refusal is the
    // evaluation-time marker check.
    let project = TempProject::new(&application_build(
        "    let marker: u8 = 1;\n    builder.roots.bind(windows_x86_64::ProgramEntry, marker);",
    ));
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(
        &project.main(),
        Some("windows_x86_64"),
    ))
    .expect_err("a non-description local cannot stand in for a product entry")
    .into_iter()
    .map(|diagnostic| diagnostic.message)
    .collect::<Vec<_>>()
    .join("\n");
    assert!(
        diagnostics.contains("not a compiler-issued product entry description")
            || diagnostics.contains("ProductEntryRef"),
        "unexpected diagnostics: {diagnostics}"
    );
}

#[test]
fn product_description_binds_only_the_slot_it_was_selected_for() {
    let project = TempProject::with_main(
        "machine launch() { let marker: u8 = 0; }",
        "machine build(builder: &mut Build) { builder.application(\"slot-mismatch\"); let entry: ProductEntryRef = builder.product.entry(\"launch\", \"linux_x86_64::ProgramEntry\"); builder.roots.bind(windows_x86_64::ProgramEntry, entry); }",
    );
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(
        &project.main(),
        Some("windows_x86_64"),
    ))
    .expect_err("a description selected for another slot must not bind here")
    .into_iter()
    .map(|diagnostic| diagnostic.message)
    .collect::<Vec<_>>()
    .join("\n");
    assert!(
        diagnostics.contains("does not match the product description's slot"),
        "unexpected diagnostics: {diagnostics}"
    );
}

#[test]
fn owner_selected_product_schema_inspects_through_foreign_helper() {
    // The owner selects its own private product data declaration through the
    // compiler-owned `product.schema` query and hands the restricted
    // description to a helper in another package. The helper inspects it
    // through `path()` without holding any product namespace of its own. A
    // clean compile plus the emitted log line also witnesses non-execution:
    // had the declared `BuildProduct::schema` body run, the returned authored
    // `ProductTypeSchema {}` would carry no description and `path()` would
    // trap.
    let helper = TempProject::new(
        "machine build(builder: &mut Build) { builder.package(\"root-binding-helper\"); }",
    );
    fs::write(helper.0.join("setup.omg"),
        "module setup; pub machine configure(builder: &mut Build, schema: &ProductTypeSchema) { builder.log.write_line(schema.path()); }",
    ).expect("helper source");
    let project = TempProject::with_main(
        "data LaunchConfig { value: u8; }",
        "use support::setup; machine build(builder: &mut Build) { builder.application(\"root-binding-owner\"); let schema: ProductTypeSchema = builder.product.schema(\"LaunchConfig\"); setup::configure(builder, &schema); _ = builder.product.schema(\"LaunchConfig\"); _ = schema.path(); }",
    );
    let mut request = CheckedCompileRequest::new(&project.main(), Some("windows_x86_64"));
    request.package_inputs = Some(foreign_helper_inputs(&project, &helper));
    let checked = compile_to_checked(request).expect(
        "an owner-selected product schema inspects through a helper without executing the declaration",
    );
    let observation = checked
        .build_observation_summary()
        .expect("schema inspection retains a build observation");
    assert_eq!(observation.build_log(), b"LaunchConfig\n");
}

#[test]
fn product_schema_query_is_scoped_to_the_query_occurrences_package() {
    // The helper performs the query itself: `LaunchConfig` exists only in the
    // owner's package, so the borrowed Build cannot reach it. Selection
    // authority belongs to the query occurrence's package, not the caller's.
    let helper = TempProject::new(
        "machine build(builder: &mut Build) { builder.package(\"root-binding-helper\"); }",
    );
    fs::write(helper.0.join("setup.omg"),
        "module setup; pub machine configure(builder: &mut Build) { let schema: ProductTypeSchema = builder.product.schema(\"LaunchConfig\"); builder.log.write_line(schema.path()); }",
    ).expect("helper source");
    let project = TempProject::with_main(
        "data LaunchConfig { value: u8; }",
        "use support::setup; machine build(builder: &mut Build) { builder.application(\"root-binding-owner\"); setup::configure(builder); }",
    );
    let mut request = CheckedCompileRequest::new(&project.main(), Some("windows_x86_64"));
    request.package_inputs = Some(foreign_helper_inputs(&project, &helper));
    let diagnostics = compile_to_checked(request)
        .expect_err("a foreign helper cannot select the caller's private product schema")
        .into_iter()
        .map(|diagnostic| diagnostic.message)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        diagnostics
            .contains("not a product declaration visible from this build occurrence's package"),
        "unexpected diagnostics: {diagnostics}"
    );
}

#[test]
fn forged_product_schema_description_is_rejected() {
    // An authored `ProductTypeSchema {}` has the static type but carries no
    // compiler-issued description payload, so inspection refuses it at
    // evaluation rather than trusting the shape.
    let project = TempProject::new(&application_build(
        "    let forged: ProductTypeSchema = ProductTypeSchema {};\n    builder.log.write_line(forged.path());",
    ));
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(
        &project.main(),
        Some("windows_x86_64"),
    ))
    .expect_err("an authored ProductTypeSchema value is not a product schema description")
    .into_iter()
    .map(|diagnostic| diagnostic.message)
    .collect::<Vec<_>>()
    .join("\n");
    assert!(
        diagnostics.contains("requires a compiler-issued ProductTypeSchema"),
        "unexpected diagnostics: {diagnostics}"
    );
}

#[test]
fn product_schema_query_rejects_a_non_facet_receiver() {
    // An authored `BuildProduct` lookalike resolves the toolchain machine by
    // member lookup but carries no facet marker: selection authority lives on
    // the compiler-issued `Build.product` value only.
    let project = TempProject::with_main(
        "data LaunchConfig { value: u8; }",
        "machine build(builder: &mut Build) { builder.application(\"lookalike-facet\"); let product: BuildProduct = BuildProduct {}; let schema: ProductTypeSchema = product.schema(\"LaunchConfig\"); }",
    );
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(
        &project.main(),
        Some("windows_x86_64"),
    ))
    .expect_err("an authored BuildProduct value cannot perform the schema query")
    .into_iter()
    .map(|diagnostic| diagnostic.message)
    .collect::<Vec<_>>()
    .join("\n");
    assert!(
        diagnostics.contains("requires the compiler-issued Build.product facet"),
        "unexpected diagnostics: {diagnostics}"
    );
}

#[test]
fn product_schema_query_requires_an_exact_data_declaration() {
    // `launch` names a machine, not a data declaration; a schema operand
    // selects only authored product data in the occurrence's package.
    let project = TempProject::with_main(
        "machine launch() { let marker: u8 = 0; }",
        "machine build(builder: &mut Build) { builder.application(\"schema-miss\"); let schema: ProductTypeSchema = builder.product.schema(\"launch\"); }",
    );
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(
        &project.main(),
        Some("windows_x86_64"),
    ))
    .expect_err("a machine name cannot stand in for a product schema")
    .into_iter()
    .map(|diagnostic| diagnostic.message)
    .collect::<Vec<_>>()
    .join("\n");
    assert!(
        diagnostics.contains("names no data declaration in this build occurrence's package"),
        "unexpected diagnostics: {diagnostics}"
    );
}

#[test]
fn product_schema_cannot_stand_in_for_an_entry_description() {
    // The description kinds are distinct: `roots.bind` consumes only the
    // entry marker, so a schema description is refused wherever an entry
    // description is required.
    let project = TempProject::with_main(
        "machine launch() { let marker: u8 = 0; }\ndata LaunchConfig { value: u8; }",
        "machine build(builder: &mut Build) { builder.application(\"schema-not-entry\"); let schema: ProductTypeSchema = builder.product.schema(\"LaunchConfig\"); builder.roots.bind(windows_x86_64::ProgramEntry, schema); }",
    );
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(
        &project.main(),
        Some("windows_x86_64"),
    ))
    .expect_err("a product schema description cannot stand in as a root binding operand")
    .into_iter()
    .map(|diagnostic| diagnostic.message)
    .collect::<Vec<_>>()
    .join("\n");
    assert!(
        diagnostics.contains("not a compiler-issued product entry description")
            || diagnostics.contains("ProductEntryRef"),
        "unexpected diagnostics: {diagnostics}"
    );
}

#[test]
fn product_schema_query_rejects_an_ambiguous_declaration() {
    // Two same-named data declarations in the query occurrence's own package
    // cannot mint one description: the query admits exactly one candidate.
    let project = TempProject::with_main(
        "use other;\ndata LaunchConfig { value: u8; }",
        "machine build(builder: &mut Build) { builder.application(\"schema-ambiguous\"); let schema: ProductTypeSchema = builder.product.schema(\"LaunchConfig\"); }",
    );
    fs::write(
        project.0.join("other.omg"),
        "module other; pub data LaunchConfig { flag: u8; }",
    )
    .expect("ambiguous sibling source");
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(
        &project.main(),
        Some("windows_x86_64"),
    ))
    .expect_err("an ambiguous product schema name is refused")
    .into_iter()
    .map(|diagnostic| diagnostic.message)
    .collect::<Vec<_>>()
    .join("\n");
    assert!(
        diagnostics.contains("is ambiguous within its package"),
        "unexpected diagnostics: {diagnostics}"
    );
}

#[test]
fn owner_selected_product_provider_inspects_through_foreign_helper() {
    // The owner selects its own private provider declaration through the
    // compiler-owned `product.provider` query and hands the restricted
    // description to a helper in another package. The helper inspects it
    // through `path()` without holding any product namespace of its own. A
    // clean compile plus the emitted log line also witnesses non-execution:
    // had the declared `BuildProduct::provider` body run, the returned
    // authored `ProductProviderRef {}` would carry no description and
    // `path()` would trap.
    let helper = TempProject::new(
        "machine build(builder: &mut Build) { builder.package(\"root-binding-helper\"); }",
    );
    fs::write(helper.0.join("setup.omg"),
        "module setup; pub machine configure(builder: &mut Build, provider: &ProductProviderRef) { builder.log.write_line(provider.path()); }",
    ).expect("helper source");
    let project = TempProject::with_main(
        "boundary trait Pick {\n    machine choose() -> i32;\n}\ndata AudioProvider { }\nmachine AudioProvider::choose() -> i32 satisfies Pick::choose {\n    transition { _ -> (1) }\n}",
        "use support::setup; machine build(builder: &mut Build) { builder.application(\"root-binding-owner\"); let provider: ProductProviderRef = builder.product.provider(\"AudioProvider\"); setup::configure(builder, &provider); _ = builder.product.provider(\"AudioProvider\"); _ = provider.path(); }",
    );
    let mut request = CheckedCompileRequest::new(&project.main(), Some("windows_x86_64"));
    request.package_inputs = Some(foreign_helper_inputs(&project, &helper));
    let checked = compile_to_checked(request).expect(
        "an owner-selected product provider inspects through a helper without executing the declaration",
    );
    let observation = checked
        .build_observation_summary()
        .expect("provider inspection retains a build observation");
    assert_eq!(observation.build_log(), b"AudioProvider\n");
}

#[test]
fn product_provider_query_is_scoped_to_the_query_occurrences_package() {
    // The helper performs the query itself: `AudioProvider` exists only in
    // the owner's package, so the borrowed Build cannot reach it. Selection
    // authority belongs to the query occurrence's package, not the caller's.
    let helper = TempProject::new(
        "machine build(builder: &mut Build) { builder.package(\"root-binding-helper\"); }",
    );
    fs::write(helper.0.join("setup.omg"),
        "module setup; pub machine configure(builder: &mut Build) { let provider: ProductProviderRef = builder.product.provider(\"AudioProvider\"); builder.log.write_line(provider.path()); }",
    ).expect("helper source");
    let project = TempProject::with_main(
        "boundary trait Pick {\n    machine choose() -> i32;\n}\ndata AudioProvider { }\nmachine AudioProvider::choose() -> i32 satisfies Pick::choose {\n    transition { _ -> (1) }\n}",
        "use support::setup; machine build(builder: &mut Build) { builder.application(\"root-binding-owner\"); setup::configure(builder); }",
    );
    let mut request = CheckedCompileRequest::new(&project.main(), Some("windows_x86_64"));
    request.package_inputs = Some(foreign_helper_inputs(&project, &helper));
    let diagnostics = compile_to_checked(request)
        .expect_err("a foreign helper cannot select the caller's private product provider")
        .into_iter()
        .map(|diagnostic| diagnostic.message)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        diagnostics
            .contains("not a provider declaration visible from this build occurrence's package"),
        "unexpected diagnostics: {diagnostics}"
    );
}

#[test]
fn forged_product_provider_description_is_rejected() {
    // An authored `ProductProviderRef {}` has the static type but carries no
    // compiler-issued description payload, so inspection refuses it at
    // evaluation rather than trusting the shape.
    let project = TempProject::new(&application_build(
        "    let forged: ProductProviderRef = ProductProviderRef {};\n    builder.log.write_line(forged.path());",
    ));
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(
        &project.main(),
        Some("windows_x86_64"),
    ))
    .expect_err("an authored ProductProviderRef value is not a product provider description")
    .into_iter()
    .map(|diagnostic| diagnostic.message)
    .collect::<Vec<_>>()
    .join("\n");
    assert!(
        diagnostics.contains("requires a compiler-issued ProductProviderRef"),
        "unexpected diagnostics: {diagnostics}"
    );
}

#[test]
fn product_provider_query_rejects_a_non_facet_receiver() {
    // An authored `BuildProduct` lookalike resolves the toolchain machine by
    // member lookup but carries no facet marker: selection authority lives on
    // the compiler-issued `Build.product` value only.
    let project = TempProject::with_main(
        "boundary trait Pick {\n    machine choose() -> i32;\n}\ndata AudioProvider { }\nmachine AudioProvider::choose() -> i32 satisfies Pick::choose {\n    transition { _ -> (1) }\n}",
        "machine build(builder: &mut Build) { builder.application(\"lookalike-facet\"); let product: BuildProduct = BuildProduct {}; let provider: ProductProviderRef = product.provider(\"AudioProvider\"); }",
    );
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(
        &project.main(),
        Some("windows_x86_64"),
    ))
    .expect_err("an authored BuildProduct value cannot perform the provider query")
    .into_iter()
    .map(|diagnostic| diagnostic.message)
    .collect::<Vec<_>>()
    .join("\n");
    assert!(
        diagnostics.contains("requires the compiler-issued Build.product facet"),
        "unexpected diagnostics: {diagnostics}"
    );
}

#[test]
fn product_provider_query_requires_a_provider_declaration() {
    // `PlainConfig` names data with no `satisfies` machines and `launch`
    // names a machine: a provider operand selects only authored provider
    // declarations in the occurrence's package, never plain data or entries.
    let project = TempProject::with_main(
        "machine launch() { let marker: u8 = 0; }\ndata PlainConfig { value: u8; }",
        "machine build(builder: &mut Build) { builder.application(\"provider-miss\"); let provider: ProductProviderRef = builder.product.provider(\"PlainConfig\"); }",
    );
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(
        &project.main(),
        Some("windows_x86_64"),
    ))
    .expect_err("a plain data declaration cannot stand in for a product provider")
    .into_iter()
    .map(|diagnostic| diagnostic.message)
    .collect::<Vec<_>>()
    .join("\n");
    assert!(
        diagnostics.contains("names no provider declaration in this build occurrence's package"),
        "unexpected diagnostics: {diagnostics}"
    );

    let project = TempProject::with_main(
        "machine launch() { let marker: u8 = 0; }",
        "machine build(builder: &mut Build) { builder.application(\"provider-miss\"); let provider: ProductProviderRef = builder.product.provider(\"launch\"); }",
    );
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(
        &project.main(),
        Some("windows_x86_64"),
    ))
    .expect_err("a machine name cannot stand in for a product provider")
    .into_iter()
    .map(|diagnostic| diagnostic.message)
    .collect::<Vec<_>>()
    .join("\n");
    assert!(
        diagnostics.contains("names no provider declaration in this build occurrence's package"),
        "unexpected diagnostics: {diagnostics}"
    );
}

#[test]
fn product_provider_cannot_stand_in_for_an_entry_description() {
    // The description kinds are distinct: `roots.bind` consumes only the
    // entry marker, so a provider description is refused wherever an entry
    // description is required.
    let project = TempProject::with_main(
        "machine launch() { let marker: u8 = 0; }\nboundary trait Pick {\n    machine choose() -> i32;\n}\ndata AudioProvider { }\nmachine AudioProvider::choose() -> i32 satisfies Pick::choose {\n    transition { _ -> (1) }\n}",
        "machine build(builder: &mut Build) { builder.application(\"provider-not-entry\"); let provider: ProductProviderRef = builder.product.provider(\"AudioProvider\"); builder.roots.bind(windows_x86_64::ProgramEntry, provider); }",
    );
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(
        &project.main(),
        Some("windows_x86_64"),
    ))
    .expect_err("a product provider description cannot stand in as a root binding operand")
    .into_iter()
    .map(|diagnostic| diagnostic.message)
    .collect::<Vec<_>>()
    .join("\n");
    assert!(
        diagnostics.contains("not a compiler-issued product entry description")
            || diagnostics.contains("ProductEntryRef"),
        "unexpected diagnostics: {diagnostics}"
    );
}

#[test]
fn product_provider_query_rejects_an_ambiguous_declaration() {
    // Two same-named provider declarations in the query occurrence's own
    // package cannot mint one description: the query admits exactly one
    // candidate.
    let project = TempProject::with_main(
        "use other;\nboundary trait Pick {\n    machine choose() -> i32;\n}\ndata AudioProvider { }\nmachine AudioProvider::choose() -> i32 satisfies Pick::choose {\n    transition { _ -> (1) }\n}",
        "machine build(builder: &mut Build) { builder.application(\"provider-ambiguous\"); let provider: ProductProviderRef = builder.product.provider(\"AudioProvider\"); }",
    );
    fs::write(
        project.0.join("other.omg"),
        "module other;\nboundary trait OtherPick {\n    machine choose() -> i32;\n}\npub data AudioProvider { }\npub machine AudioProvider::choose() -> i32 satisfies OtherPick::choose {\n    transition { _ -> (1) }\n}",
    )
    .expect("ambiguous sibling source");
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(
        &project.main(),
        Some("windows_x86_64"),
    ))
    .expect_err("an ambiguous product provider name is refused")
    .into_iter()
    .map(|diagnostic| diagnostic.message)
    .collect::<Vec<_>>()
    .join("\n");
    assert!(
        diagnostics.contains("is ambiguous within its package"),
        "unexpected diagnostics: {diagnostics}"
    );
}

#[test]
fn product_provider_description_is_not_a_static_provider_argument() {
    // `select_provider` still takes its exact static slot and provider type
    // paths; a retained description place cannot substitute for the provider
    // type argument, and the one-type-argument spelling never reaches
    // evaluation.
    let project = TempProject::with_main(
        "boundary trait Pick {\n    machine choose() -> i32;\n}\ndata AudioProvider { }\nmachine AudioProvider::choose() -> i32 satisfies Pick::choose {\n    transition { _ -> (1) }\n}",
        "machine build(builder: &mut Build) { builder.application(\"provider-operand\"); let provider: ProductProviderRef = builder.product.provider(\"AudioProvider\"); builder.select_provider<Pick>(provider); }",
    );
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(
        &project.main(),
        Some("windows_x86_64"),
    ))
    .expect_err("a product provider description is not a select_provider type argument")
    .into_iter()
    .map(|diagnostic| diagnostic.message)
    .collect::<Vec<_>>()
    .join("\n");
    assert!(
        diagnostics.contains("`select_provider` requires exactly two plain type paths"),
        "unexpected diagnostics: {diagnostics}"
    );
}

#[test]
fn qualified_product_entry_query_selects_a_public_declaration_in_an_authorized_dependency() {
    // The owner's `support` edge is product scope, so its own build machine
    // resolves `support::setup::launch` through the occurrence package's
    // authorized product dependencies and binds the exact foreign machine --
    // product selection uses the authored dependency roster, never a caller's
    // borrowed namespace.
    let helper = TempProject::new(
        "machine build(builder: &mut Build) { builder.package(\"root-binding-helper\"); }",
    );
    fs::write(
        helper.0.join("setup.omg"),
        "module setup; pub machine launch() { let marker: u8 = 0; }",
    )
    .expect("helper source");
    let project = TempProject::with_main(
        "use support::setup;\nmachine root_probe() { setup::launch(); }",
        "machine build(builder: &mut Build) { builder.application(\"root-binding-owner\"); let entry: ProductEntryRef = builder.product.entry(\"support::setup::launch\", \"windows_x86_64::ProgramEntry\"); builder.roots.bind(windows_x86_64::ProgramEntry, entry); }",
    );
    let mut request = CheckedCompileRequest::new(&project.main(), Some("windows_x86_64"));
    request.package_inputs = Some(foreign_product_inputs(&project, &helper));
    let checked = compile_to_checked(request).expect(
        "a qualified query selects a public declaration in an authorized product dependency",
    );
    assert_eq!(checked.selected_program_entry_machine(), Some("launch"));
}

#[test]
fn qualified_product_entry_query_rejects_a_build_scope_alias() {
    // `support` is a build-scope edge here: the same qualified spelling that
    // resolves under a product dependency authorizes nothing, because the two
    // dependency scopes never stand in for one another.
    let helper = TempProject::new(
        "machine build(builder: &mut Build) { builder.package(\"root-binding-helper\"); }",
    );
    fs::write(helper.0.join("setup.omg"),
        "module setup; pub machine launch() { let marker: u8 = 0; } pub machine configure(builder: &mut Build) { let marker: u8 = 0; }",
    ).expect("helper source");
    let project = TempProject::with_main(
        "const ANSWER: u32 = 42;\n",
        "use support::setup; machine build(builder: &mut Build) { builder.application(\"root-binding-owner\"); setup::configure(builder); let entry: ProductEntryRef = builder.product.entry(\"support::setup::launch\", \"windows_x86_64::ProgramEntry\"); builder.roots.bind(windows_x86_64::ProgramEntry, entry); }",
    );
    let mut request = CheckedCompileRequest::new(&project.main(), Some("windows_x86_64"));
    request.package_inputs = Some(foreign_helper_inputs(&project, &helper));
    let diagnostics = compile_to_checked(request)
        .expect_err("a build-scope dependency alias authorizes no product selection")
        .into_iter()
        .map(|diagnostic| diagnostic.message)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        diagnostics
            .contains("not a product declaration visible from this build occurrence's package"),
        "unexpected diagnostics: {diagnostics}"
    );
}

#[test]
fn qualified_product_entry_query_rejects_a_private_dependency_declaration() {
    // The dependency's `launch` is not public: a qualified path may only
    // select public declarations, so the query cannot enumerate private
    // product names across the package boundary.
    let helper = TempProject::new(
        "machine build(builder: &mut Build) { builder.package(\"root-binding-helper\"); }",
    );
    fs::write(
        helper.0.join("setup.omg"),
        "module setup; machine launch() { let marker: u8 = 0; }",
    )
    .expect("helper source");
    let project = TempProject::with_main(
        "use support::setup;\nmachine root_probe() { }",
        "machine build(builder: &mut Build) { builder.application(\"root-binding-owner\"); let entry: ProductEntryRef = builder.product.entry(\"support::setup::launch\", \"windows_x86_64::ProgramEntry\"); builder.roots.bind(windows_x86_64::ProgramEntry, entry); }",
    );
    let mut request = CheckedCompileRequest::new(&project.main(), Some("windows_x86_64"));
    request.package_inputs = Some(foreign_product_inputs(&project, &helper));
    let diagnostics = compile_to_checked(request)
        .expect_err("a private dependency declaration is not selectable")
        .into_iter()
        .map(|diagnostic| diagnostic.message)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        diagnostics
            .contains("not a product declaration visible from this build occurrence's package"),
        "unexpected diagnostics: {diagnostics}"
    );
}

#[test]
fn qualified_product_schema_query_selects_a_public_declaration_in_an_authorized_dependency() {
    let helper = TempProject::new(
        "machine build(builder: &mut Build) { builder.package(\"root-binding-helper\"); }",
    );
    fs::write(
        helper.0.join("setup.omg"),
        "module setup; pub data LaunchConfig { value: u8; }",
    )
    .expect("helper source");
    let project = TempProject::with_main(
        "use support::setup;",
        "machine build(builder: &mut Build) { builder.application(\"root-binding-owner\"); let schema: ProductTypeSchema = builder.product.schema(\"support::setup::LaunchConfig\"); builder.log.write_line(schema.path()); }",
    );
    let mut request = CheckedCompileRequest::new(&project.main(), Some("windows_x86_64"));
    request.package_inputs = Some(foreign_product_inputs(&project, &helper));
    let checked = compile_to_checked(request).expect(
        "a qualified schema query selects a public declaration in an authorized product dependency",
    );
    let observation = checked
        .build_observation_summary()
        .expect("schema inspection retains a build observation");
    assert_eq!(observation.build_log(), b"setup::LaunchConfig\n");
}

#[test]
fn qualified_product_provider_query_selects_a_public_declaration_in_an_authorized_dependency() {
    let helper = TempProject::new(
        "machine build(builder: &mut Build) { builder.package(\"root-binding-helper\"); }",
    );
    fs::write(helper.0.join("setup.omg"),
        "module setup; pub boundary trait Pick {\n    machine choose() -> i32;\n}\npub data AudioProvider { }\npub machine AudioProvider::choose() -> i32 satisfies Pick::choose {\n    transition { _ -> (1) }\n}",
    ).expect("helper source");
    let project = TempProject::with_main(
        "use support::setup;",
        "machine build(builder: &mut Build) { builder.application(\"root-binding-owner\"); let provider: ProductProviderRef = builder.product.provider(\"support::setup::AudioProvider\"); builder.log.write_line(provider.path()); }",
    );
    let mut request = CheckedCompileRequest::new(&project.main(), Some("windows_x86_64"));
    request.package_inputs = Some(foreign_product_inputs(&project, &helper));
    let checked = compile_to_checked(request).expect(
        "a qualified provider query selects a public declaration in an authorized product dependency",
    );
    let observation = checked
        .build_observation_summary()
        .expect("provider inspection retains a build observation");
    assert_eq!(observation.build_log(), b"setup::AudioProvider\n");
}

#[test]
fn product_entry_query_resolves_an_own_package_module_path() {
    // A `module::name` spelling inside the query occurrence's own package is
    // the same logical declaration path the dependency form uses: the helper
    // selects its own (private) `setup::launch` by its in-package path.
    let helper = TempProject::new(
        "machine build(builder: &mut Build) { builder.package(\"root-binding-helper\"); }",
    );
    fs::write(
        helper.0.join("setup.omg"),
        "module setup; machine launch() { let marker: u8 = 0; }",
    )
    .expect("private product entry source");
    fs::write(helper.0.join("configure.omg"),
        "module configure; pub machine configure(builder: &mut Build) { let entry: ProductEntryRef = builder.product.entry(\"setup::launch\", \"windows_x86_64::ProgramEntry\"); builder.roots.bind(windows_x86_64::ProgramEntry, entry); }",
    ).expect("build helper source");
    let project = TempProject::with_main(
        "use support::setup; const ANSWER: u32 = 42;\n",
        "use support::configure; machine build(builder: &mut Build) { builder.application(\"root-binding-owner\"); configure::configure(builder); }",
    );
    let mut request = CheckedCompileRequest::new(&project.main(), Some("windows_x86_64"));
    request.package_inputs = Some(helper_entry_inputs(&project, &helper));
    let checked = compile_to_checked(request)
        .expect("a module-qualified path selects the occurrence package's own declaration");
    assert_eq!(checked.selected_program_entry_machine(), Some("launch"));
    let selected = checked
        .selected_program_entry()
        .expect("entry")
        .source_signature()
        .machine_symbol();
    assert_eq!(
        checked.symbols.symbol_product_package_identity(selected),
        Some(super::package_identity(2))
    );
}

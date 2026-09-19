use super::{TempProject, application_build, foreign_helper_inputs, foreign_product_inputs};
use compiler::{
    CheckedCompileRequest, CompileOptions, CompileRequest, RequestedCompileProduct, compile,
    compile_to_checked,
};
use std::fs;

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
    fs::write(helper.0.join("setup.omg"),
        "module setup; pub machine launch() { let marker: u8 = 0; } pub machine configure(builder: &mut Build) { builder.roots.bind(windows_x86_64::ProgramEntry, launch); }",
    ).expect("helper source");
    let project = TempProject::with_main(
        "const ANSWER: u32 = 42;\n",
        "use support::setup; machine build(builder: &mut Build) { builder.application(\"root-binding-owner\"); setup::configure(builder); }",
    );
    let mut request = CheckedCompileRequest::new(&project.main(), Some("windows_x86_64"));
    request.package_inputs = Some(foreign_helper_inputs(&project, &helper));
    let checked = compile_to_checked(request)
        .expect("a helper may bind its own package's entry through the borrowed root Build");
    assert_eq!(checked.selected_program_entry_machine(), Some("launch"));
}

#[test]
fn same_named_entry_in_another_package_rejoins_production_and_settlement_by_symbol() {
    let helper = TempProject::new(
        "machine build(builder: &mut Build) { builder.package(\"root-binding-helper\"); }",
    );
    fs::write(helper.0.join("setup.omg"),
        "module setup; pub machine launch() { let marker: u8 = 0; } pub machine configure(builder: &mut Build) { builder.roots.bind(windows_x86_64::ProgramEntry, launch); }",
    ).expect("helper source");
    let project = TempProject::with_main(
        "machine launch() { let marker: u8 = 0; }",
        "use support::setup; machine build(builder: &mut Build) { builder.application(\"root-binding-owner\"); setup::configure(builder); }",
    );
    let mut request = CheckedCompileRequest::new(&project.main(), Some("windows_x86_64"));
    request.package_inputs = Some(foreign_helper_inputs(&project, &helper));
    let checked = compile_to_checked(request)
        .expect("a same-named foreign entry binds by exact symbol, not by name");
    assert_eq!(checked.selected_program_entry_machine(), Some("launch"));
    let entry = checked
        .selected_program_entry()
        .cloned()
        .expect("one exact selected ProgramEntry");
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
        .with_package_inputs(foreign_helper_inputs(&project, &helper)),
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
fn delegated_root_binding_requires_a_product_entry_ref_operand() {
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
fn delegated_root_binding_rejects_a_computed_description_result() {
    // The delegated `roots.bind` operand admits only a retained
    // `ProductEntryRef` place: a call result spelling stays fenced until
    // ordinary call-result authority can carry it.
    let project = TempProject::with_main(
        "machine launch() { let marker: u8 = 0; }\nmachine choose_entry() -> ProductEntryRef {\n    transition { _ -> (ProductEntryRef {}) }\n}",
        "machine build(builder: &mut Build) { builder.application(\"computed-operand\"); builder.roots.bind(windows_x86_64::ProgramEntry, choose_entry()); }",
    );
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(
        &project.main(),
        Some("windows_x86_64"),
    ))
    .expect_err("a computed call result cannot serve as the root binding operand")
    .into_iter()
    .map(|diagnostic| diagnostic.message)
    .collect::<Vec<_>>()
    .join("\n");
    assert!(
        diagnostics.contains(
            "root-slot binding requires exactly one slot path and one implementation path"
        ) || diagnostics.contains("computed description results are not implemented"),
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
    fs::write(helper.0.join("setup.omg"),
        "module setup; machine launch() { let marker: u8 = 0; } pub machine configure(builder: &mut Build) { let entry: ProductEntryRef = builder.product.entry(\"setup::launch\", \"windows_x86_64::ProgramEntry\"); builder.roots.bind(windows_x86_64::ProgramEntry, entry); }",
    ).expect("helper source");
    let project = TempProject::with_main(
        "const ANSWER: u32 = 42;\n",
        "use support::setup; machine build(builder: &mut Build) { builder.application(\"root-binding-owner\"); setup::configure(builder); }",
    );
    let mut request = CheckedCompileRequest::new(&project.main(), Some("windows_x86_64"));
    request.package_inputs = Some(foreign_helper_inputs(&project, &helper));
    let checked = compile_to_checked(request)
        .expect("a module-qualified path selects the occurrence package's own declaration");
    assert_eq!(checked.selected_program_entry_machine(), Some("launch"));
}

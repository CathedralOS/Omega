//! Dependency-qualified product queries name complete declaration paths.

use super::{TempProject, foreign_product_inputs, package_identity};
use compiler::{CheckedCompileRequest, compile_to_checked};
use std::fs;

struct CheckedQuery {
    project: TempProject,
    _dependency: TempProject,
    checked: compiler::CheckedCompilation,
}

fn queried_dependency(
    root_source: &str,
    other_source: &str,
    operation: &str,
) -> Result<CheckedQuery, Vec<diagnostics::Diagnostic>> {
    let dependency = TempProject::new(
        "machine build(builder: &mut Build) { builder.package(\"query-library\"); }",
    );
    fs::write(dependency.0.join("Entry.omg"), root_source).expect("exact declaration source");
    fs::write(dependency.0.join("other.omg"), other_source).expect("other module source");
    let project = TempProject::with_main(
        "use support::Entry; use support::other;",
        &format!(
            "machine build(builder: &mut Build) {{ builder.application(\"query-paths\"); {operation} }}"
        ),
    );
    let mut request = CheckedCompileRequest::new(&project.main(), Some("windows_x86_64"));
    request.package_inputs = Some(foreign_product_inputs(&project, &dependency));
    let checked = compile_to_checked(request)?;
    Ok(CheckedQuery {
        project,
        _dependency: dependency,
        checked,
    })
}

#[test]
fn dependency_entry_query_does_not_confuse_an_attached_name_with_an_exact_path() {
    let query = queried_dependency(
        "module Entry; pub machine launch() { }",
        "module other; pub data Entry { } pub machine Entry::launch(&mut self) { }",
        "let entry: ProductEntryRef = builder.product.entry(\"support::Entry::launch\", \"windows_x86_64::ProgramEntry\"); builder.roots.bind(windows_x86_64::ProgramEntry, entry);",
    ).expect("qualified query selects the exact module declaration");
    let checked = &query.checked;
    let selected = checked
        .selected_program_entry()
        .expect("entry")
        .source_signature()
        .machine_symbol();
    assert_eq!(
        checked.symbols.symbol_product_package_identity(selected),
        Some(package_identity(2))
    );
    assert_eq!(
        checked.symbols.display_path(selected, "::"),
        "Entry::launch"
    );
    let report = compiler::retained_terminal_report_from_checked_package(
        query.project.main(),
        query.checked,
        proof_admission::AdmissionProfile::default(),
    )
    .expect("exact selected entry reaches Terminal production");
    let artifact = report
        .into_retained_terminal_artifact()
        .expect("retained Terminal artifact");
    artifact
        .validate()
        .expect("independent Terminal verification");
    assert_eq!(
        artifact
            .native_realization_proposal()
            .expect("native entry proposal")
            .checked_program_entry()
            .source_machine_symbol(),
        selected
    );
}

#[test]
fn dependency_schema_query_does_not_search_other_modules_by_leaf_name() {
    let checked = queried_dependency(
        "pub data Settings { value: u8; }",
        "module other; pub data Settings { alternative: u64; }",
        "let schema: ProductTypeSchema = builder.product.schema(\"support::Settings\"); builder.log.write_line(schema.path());",
    ).expect("qualified schema selects the exact dependency-root declaration");
    assert_eq!(
        checked
            .checked
            .build_observation_summary()
            .expect("query observation")
            .build_log(),
        b"Settings\n"
    );
}

#[test]
fn dependency_provider_query_does_not_search_other_modules_by_leaf_name() {
    let provider = "pub trait Choice { machine pick() -> i32; } pub data Provider { } pub machine Provider::pick() -> i32 satisfies Choice::pick { transition { _ -> (1) } }";
    let checked = queried_dependency(
        provider,
        &format!("module other; {provider}"),
        "let provider: ProductProviderRef = builder.product.provider(\"support::Provider\"); builder.log.write_line(provider.path());",
    ).expect("qualified provider selects the exact dependency-root declaration");
    assert_eq!(
        checked
            .checked
            .build_observation_summary()
            .expect("query observation")
            .build_log(),
        b"Provider\n"
    );
}

#[test]
fn dependency_queries_reject_a_missing_exact_path_despite_a_matching_relative_name() {
    for (root_source, other_source, operation, rejection) in [
        (
            "module Entry; pub data Placeholder { }",
            "module other; pub data Entry { } pub machine Entry::launch(&mut self) { }",
            "let entry: ProductEntryRef = builder.product.entry(\"support::Entry::launch\", \"windows_x86_64::ProgramEntry\"); builder.roots.bind(windows_x86_64::ProgramEntry, entry);",
            "not a product declaration visible",
        ),
        (
            "pub data Placeholder { }",
            "module other; pub data Settings { value: u8; }",
            "let schema: ProductTypeSchema = builder.product.schema(\"support::Settings\"); builder.log.write_line(schema.path());",
            "not a product declaration visible",
        ),
        (
            "pub data Placeholder { }",
            "module other; pub trait Choice { machine pick() -> i32; } pub data Provider { } pub machine Provider::pick() -> i32 satisfies Choice::pick { transition { _ -> (1) } }",
            "let provider: ProductProviderRef = builder.product.provider(\"support::Provider\"); builder.log.write_line(provider.path());",
            "not a provider declaration visible",
        ),
    ] {
        let Err(diagnostics) = queried_dependency(root_source, other_source, operation) else {
            panic!("a relative declaration satisfied an absent qualified path: {operation}");
        };
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(rejection)),
            "{diagnostics:?}"
        );
    }
}

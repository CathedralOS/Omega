//! Product entry descriptions select by the expected slot signature before binding.

use super::{TempProject, application_build};
use compiler::{
    CheckedCompileRequest, CompileOptions, CompileRequest, RequestedCompileProduct, compile,
    compile_to_checked,
};

fn query_entry(source: &str, slot: &str, statement_position: bool) -> Result<(), String> {
    let query = format!("builder.product.entry(\"launch\", \"{slot}\")");
    let statement = if statement_position {
        format!("{query};")
    } else {
        format!("let description: ProductEntryRef = {query};")
    };
    let project = TempProject::with_main(source, &application_build(&statement));
    compile_to_checked(CheckedCompileRequest::new(
        &project.main(),
        Some("windows_x86_64"),
    ))
    .map(|_| ())
    .map_err(|diagnostics| {
        diagnostics
            .into_iter()
            .map(|diagnostic| diagnostic.message)
            .collect::<Vec<_>>()
            .join("\n")
    })
}

#[test]
fn discarded_product_entry_query_rejects_unknown_slot() {
    for (slot, reason) in [
        ("not_a_root_slot", "is not target-qualified"),
        ("unknown_target::ProgramEntry", "unknown target profile"),
        (
            "windows_x86_64::UnknownEntry",
            "declares no required root slot",
        ),
    ] {
        let errors = query_entry("machine launch() { }", slot, false)
            .expect_err("a description must name an actual entry slot without requiring a binding");
        assert!(errors.contains(reason), "{errors}");
    }
}

#[test]
fn discarded_product_entry_query_rejects_another_target() {
    let errors = query_entry("machine launch() { }", "linux_x86_64::ProgramEntry", false)
        .expect_err("an executed query describes only its activation's target");
    assert!(
        errors.contains("does not match the selected product target"),
        "{errors}"
    );
}

#[test]
fn discarded_product_entry_query_rejects_incompatible_parameters() {
    for statement_position in [false, true] {
        let errors = query_entry(
            "machine launch(value: u8) { }",
            "windows_x86_64::ProgramEntry",
            statement_position,
        )
        .expect_err("discarding a query must not bypass its expected entry signature");
        assert!(errors.contains("parameter"), "{errors}");
    }
}

#[test]
fn discarded_product_entry_query_rejects_results_and_generic_entries() {
    for (source, reason) in [
        ("machine launch() -> u8 { 7 }", "returns a value"),
        ("machine launch<T>() { }", "is generic"),
    ] {
        let errors = query_entry(source, "windows_x86_64::ProgramEntry", false)
            .expect_err("entry descriptions must satisfy the slot before binding");
        assert!(errors.contains(reason), "{errors}");
    }
}

#[test]
fn product_entry_query_keeps_compatible_candidates_ambiguous() {
    let project = TempProject::with_main(
        "use first; use second;",
        &application_build("builder.product.entry(\"launch\", \"windows_x86_64::ProgramEntry\");"),
    );
    for module in ["first", "second"] {
        std::fs::write(
            project.0.join(format!("{module}.omg")),
            format!("module {module}; pub machine launch() {{ }}"),
        )
        .unwrap();
    }
    let errors = compile_to_checked(CheckedCompileRequest::new(
        &project.main(),
        Some("windows_x86_64"),
    ))
    .map(|_| ())
    .expect_err("two compatible entries remain ambiguous");
    assert!(
        errors.iter().any(|error| error
            .message
            .contains("more than one visible declaration is compatible")),
        "{errors:?}"
    );
}

#[test]
fn product_entry_query_selects_compatible_candidate_through_native_execution() {
    let Some(profile) = target::TargetProfile::host_if_supported() else {
        eprintln!("SKIP: entry execution requires a supported hosted target");
        return;
    };
    let slot = format!("{}::ProgramEntry", profile.root_slot_owner_name());
    let project = TempProject::with_main(
        "use takes_value; use ready;",
        &application_build(&format!(
            "let entry: ProductEntryRef = builder.product.entry(\"launch\", \"{slot}\"); builder.roots.bind({slot}, entry);"
        )),
    );
    std::fs::write(
        project.0.join("takes_value.omg"),
        "module takes_value; pub machine launch(value: u8) { }",
    )
    .unwrap();
    std::fs::write(
        project.0.join("ready.omg"),
        "module ready; pub machine launch() { let marker: u8 = 37; }",
    )
    .unwrap();
    let report = compile(
        CompileRequest::new(CompileOptions {
            root_path: project.main(),
            build_dir: None,
            target_name: Some(profile.target_name().into()),
        })
        .with_requested_product(RequestedCompileProduct::NativeArtifact),
    )
    .and_then(compiler::CompileOutcomes::into_single_report)
    .unwrap_or_else(|diagnostics| {
        panic!("compatible entry must reach native publication: {diagnostics:?}")
    });
    let published = report
        .publish_retained_native_artifact(&project.0.join("out"))
        .expect("publish exact compatible entry");
    std::fs::remove_file(project.0.join("ready.omg")).expect("remove selected entry source");
    let output = std::process::Command::new(published.checked_native_executable_path().unwrap())
        .output()
        .expect("execute selected product entry");
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert!(
        output.stdout.is_empty() && output.stderr.is_empty(),
        "{output:?}"
    );
}

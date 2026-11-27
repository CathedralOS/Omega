// Console adapters that translate parsed CLI requests into orchestration or package-manager calls and format their results.

pub(crate) mod compile;
pub(crate) mod inspect;
pub(crate) mod packages;
pub(crate) mod run;

pub(crate) fn report_unused_imports(imports: &project_build::FileImportAdvisories) {
    let warnings = imports.warnings();
    if !warnings.is_empty() {
        let mut text = String::new();
        diagnostics::write_diagnostics(&mut text, &warnings).expect("writing to a String succeeds");
        eprintln!("{text}");
    }
}

pub(crate) fn report_unused_declarations(declarations: &project_build::DeclarationUsageAdvisories) {
    let warnings = declarations.warnings();
    if !warnings.is_empty() {
        let mut text = String::new();
        diagnostics::write_diagnostics(&mut text, &warnings).expect("writing to a String succeeds");
        eprintln!("{text}");
    }
}

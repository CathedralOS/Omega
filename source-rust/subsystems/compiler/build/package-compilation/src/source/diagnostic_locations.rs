//! Resolve spans against the exact admitted source cohort while its package names are available.
use crate::PackageCompilationInputs;
use diagnostics::Diagnostic;

pub fn attribute_diagnostics(
    diagnostics: &mut [Diagnostic],
    sources: &source_compiler::source_inputs::SourceMap,
    package_inputs: Option<&PackageCompilationInputs>,
) {
    for diagnostic in diagnostics {
        if diagnostic.resolved_location.is_some() {
            continue;
        }
        let Some(source_span) = diagnostic.source_span else {
            continue;
        };
        if source_span == source_text::SourceSpan::default() {
            continue;
        }
        let Some(file) = sources.file_at(source_span) else {
            continue;
        };
        let position = source_text::SourcePosition::of(&file.source, source_span.span.start);
        let package = file
            .package_identity
            .and_then(|identity| package_inputs.and_then(|inputs| inputs.package_name(identity)))
            .map(str::to_string);
        diagnostic.resolved_location = Some(diagnostics::ResolvedSourceLocation {
            package,
            path: file.path.clone(),
            line: position.line as u32,
            column: position.column as u32,
        });
    }
}

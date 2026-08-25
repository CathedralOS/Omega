use psi_diagnostics::Diagnostic;
use psi_language_semantics::declaration_selection::{
    AuthoredDeclarationSelectionExposure, AuthoredDeclarationSelectionTarget,
};
use psi_symbols::SymbolKind;
use psi_typed_trees::TypedTrees;

pub(crate) fn validate_declaration_visibility(
    program: &TypedTrees,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for selection in program.authored_declaration_selections() {
        if selection.exposure() != AuthoredDeclarationSelectionExposure::PublicInterface {
            continue;
        }
        let AuthoredDeclarationSelectionTarget::Resolved(target) = selection.target() else {
            continue;
        };
        let symbol = target.selected_symbol();
        if program.symbols.get(symbol).kind != SymbolKind::Proposition {
            continue;
        }
        let Some(declaration) = program
            .propositions()
            .iter()
            .find(|declaration| declaration.symbol == symbol)
        else {
            diagnostics.push(
                Diagnostic::error(format!(
                    "public interface selects proposition `{}` without retained declaration visibility",
                    program.symbols.display_path(symbol, "::")
                ))
                .with_source_span(selection.source_span()),
            );
            continue;
        };
        if !declaration.is_public {
            diagnostics.push(
                Diagnostic::error(format!(
                    "public interface selects private proposition `{}`",
                    program.symbols.display_path(symbol, "::")
                ))
                .with_source_span(selection.source_span()),
            );
        }
    }
}

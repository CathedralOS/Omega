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
        let (kind, is_public) = match program.symbols.get(symbol).kind {
            SymbolKind::Proposition => (
                "proposition",
                program
                    .propositions()
                    .iter()
                    .find(|declaration| declaration.symbol == symbol)
                    .map(|declaration| declaration.is_public),
            ),
            SymbolKind::Const => (
                "const",
                program
                    .const_declarations()
                    .iter()
                    .find(|declaration| declaration.symbol == symbol)
                    .map(|declaration| declaration.is_public),
            ),
            _ => continue,
        };
        let Some(is_public) = is_public else {
            diagnostics.push(
                Diagnostic::error(format!(
                    "public interface selects {kind} `{}` without retained declaration visibility",
                    program.symbols.display_path(symbol, "::")
                ))
                .with_source_span(selection.source_span()),
            );
            continue;
        };
        if !is_public {
            diagnostics.push(
                Diagnostic::error(format!(
                    "public interface selects private {kind} `{}`",
                    program.symbols.display_path(symbol, "::")
                ))
                .with_source_span(selection.source_span()),
            );
        }
    }
}

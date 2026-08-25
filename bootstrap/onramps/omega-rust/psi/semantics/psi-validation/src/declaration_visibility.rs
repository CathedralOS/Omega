use psi_diagnostics::Diagnostic;
use psi_language_semantics::declaration_selection::{
    AuthoredDeclarationSelectionExposure, AuthoredDeclarationSelectionTarget,
};
use psi_symbols::SymbolKind;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

pub(crate) fn collect_declaration_visibility_diagnostics(
    program: &TypedTrees,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // This collector is rerun after checked late-binding finalizes receiver
    // calls, making it the common gate for legality tied to exact authored
    // declaration identity as well as visibility.
    crate::cleanup::collect_reserved_cleanup_selection_diagnostics(program, diagnostics);
    validate_public_const_declared_types(program, diagnostics);

    for selection in program.authored_declaration_selections() {
        if selection.exposure() != AuthoredDeclarationSelectionExposure::PublicInterface {
            continue;
        }
        let AuthoredDeclarationSelectionTarget::Resolved(target) = selection.target() else {
            continue;
        };
        let symbol = target.selected_symbol();
        let symbol_kind = program.symbols.get(symbol).kind;
        if !psi_typed_trees::visibility::requires_declaration_visibility(symbol_kind) {
            continue;
        }
        let Some(visibility) = psi_typed_trees::visibility::declaration_visibility(program, symbol)
        else {
            diagnostics.push(
                Diagnostic::error(format!(
                    "public interface selects {:?} `{}` without retained declaration visibility",
                    symbol_kind,
                    program.symbols.display_path(symbol, "::")
                ))
                .with_source_span(selection.source_span()),
            );
            continue;
        };
        if !visibility.is_public() {
            diagnostics.push(
                Diagnostic::error(format!(
                    "public interface selects private {} `{}`",
                    visibility.kind(),
                    program.symbols.display_path(symbol, "::")
                ))
                .with_source_span(selection.source_span()),
            );
        }
    }
}

pub fn validate_declaration_visibility(program: &TypedTrees) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    collect_declaration_visibility_diagnostics(program, &mut diagnostics);
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

fn validate_public_const_declared_types(program: &TypedTrees, diagnostics: &mut Vec<Diagnostic>) {
    for declaration in program
        .const_declarations()
        .iter()
        .filter(|declaration| declaration.is_public)
    {
        let mut visited = Vec::new();
        validate_public_const_type_reference(
            program,
            declaration.symbol,
            declaration.declared_type,
            &mut visited,
            diagnostics,
        );
    }
}

fn validate_public_const_type_reference(
    program: &TypedTrees,
    const_symbol: psi_symbols::SymbolHandle,
    type_reference: TypeReferenceHandle,
    visited: &mut Vec<TypeReferenceHandle>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if visited.contains(&type_reference) {
        return;
    }
    visited.push(type_reference);

    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. }
        | TypeReferenceNode::Constrained {
            base_type: referee, ..
        } => validate_public_const_type_reference(
            program,
            const_symbol,
            *referee,
            visited,
            diagnostics,
        ),
        TypeReferenceNode::FixedArray { element_type, .. }
        | TypeReferenceNode::Slice { element_type } => validate_public_const_type_reference(
            program,
            const_symbol,
            *element_type,
            visited,
            diagnostics,
        ),
        TypeReferenceNode::Generic {
            base_symbol,
            arguments,
            ..
        } => {
            validate_public_const_data_visibility(program, const_symbol, *base_symbol, diagnostics);
            for argument in program
                .type_reference_table
                .type_reference_handles(*arguments)
            {
                validate_public_const_type_reference(
                    program,
                    const_symbol,
                    *argument,
                    visited,
                    diagnostics,
                );
            }
        }
        TypeReferenceNode::Named { symbol, .. } => {
            validate_public_const_data_visibility(program, const_symbol, *symbol, diagnostics);
        }
        TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::Unit => {}
    }
}

fn validate_public_const_data_visibility(
    program: &TypedTrees,
    const_symbol: psi_symbols::SymbolHandle,
    selected_symbol: psi_symbols::SymbolHandle,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if program.symbols.get(selected_symbol).kind != SymbolKind::Data {
        return;
    }
    let Some(data) = program
        .data_definitions()
        .iter()
        .find(|declaration| declaration.symbol == selected_symbol)
    else {
        diagnostics.push(Diagnostic::error(format!(
            "public const `{}` selects data `{}` without retained declaration visibility",
            program.symbols.display_path(const_symbol, "::"),
            program.symbols.display_path(selected_symbol, "::"),
        )));
        return;
    };
    if !data.is_public {
        diagnostics.push(Diagnostic::error(format!(
            "public const `{}` exposes private data `{}` in its declared type",
            program.symbols.display_path(const_symbol, "::"),
            program.symbols.display_path(selected_symbol, "::"),
        )));
    }
}

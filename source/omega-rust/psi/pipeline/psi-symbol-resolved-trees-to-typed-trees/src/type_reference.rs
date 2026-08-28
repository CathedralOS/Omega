use crate::lowerer::Lowerer;
use psi_diagnostics::Diagnostic;
use psi_symbol_resolved_trees as resolved;
use psi_typed_trees as typed;

mod constraints;
mod direct;
mod table;

pub(crate) use constraints::lower_element_applicable_constraints;
use direct::lower_type_reference_handle_with_context;
use table::lower_type_reference_handle_from_table_with_context;

pub(crate) fn lower_type_reference_into_table(
    lowerer: &mut Lowerer,
    type_reference: &resolved::types::TypeReference,
) -> Result<typed::types::TypeReferenceHandle, Diagnostic> {
    lower_type_reference_into_trees_with_exposure(
        lowerer.source_trees,
        &mut lowerer.typed_trees,
        type_reference,
        lowerer.type_reference_exposure,
    )
}

pub(crate) fn lower_type_reference_into_trees(
    source_trees: &resolved::SymbolResolvedTrees,
    typed_trees: &mut typed::TypedTrees,
    type_reference: &resolved::types::TypeReference,
) -> Result<typed::types::TypeReferenceHandle, Diagnostic> {
    lower_type_reference_into_trees_with_exposure(
        source_trees,
        typed_trees,
        type_reference,
        psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PrivateImplementation,
    )
}

pub(super) fn lower_type_reference_into_trees_with_exposure(
    source_trees: &resolved::SymbolResolvedTrees,
    typed_trees: &mut typed::TypedTrees,
    type_reference: &resolved::types::TypeReference,
    exposure: psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure,
) -> Result<typed::types::TypeReferenceHandle, Diagnostic> {
    lower_type_reference_handle_with_context(source_trees, typed_trees, type_reference, exposure)
}

pub(crate) fn lower_type_reference_handle_from_table(
    lowerer: &mut Lowerer,
    type_reference: resolved::types::TypeReferenceHandle,
) -> Result<typed::types::TypeReferenceHandle, Diagnostic> {
    lower_type_reference_handle_from_table_with_context(
        lowerer.source_trees,
        &mut lowerer.typed_trees,
        type_reference,
        psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PrivateImplementation,
    )
}

pub(super) fn retain_type_reference_selection(
    source_trees: &resolved::SymbolResolvedTrees,
    typed_trees: &mut typed::TypedTrees,
    name: &resolved::name::DiagnosticName,
    symbol: psi_symbols::SymbolHandle,
    exposure: psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure,
    kind: psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionKind,
) -> Result<(), Diagnostic> {
    if !name.is_source_backed()
        || !symbol.is_valid()
        || resolved::types::PrimitiveType::from_name(name.as_str()).is_some()
    {
        return Ok(());
    }
    if matches!(
        source_trees.symbols.get(symbol).kind,
        psi_symbols::SymbolKind::Parameter
            | psi_symbols::SymbolKind::TypeParameter
            | psi_symbols::SymbolKind::MachineParameter
            | psi_symbols::SymbolKind::ConformanceParameter
            | psi_symbols::SymbolKind::PropositionParameter
            | psi_symbols::SymbolKind::PropositionMachineParameter
            | psi_symbols::SymbolKind::Local
    ) {
        return Ok(());
    }
    typed_trees
        .record_resolved_authored_declaration_selection_once(
            name.source_span(),
            exposure,
            kind,
            symbol,
        )
        .map(|_| ())
        .map_err(|error| {
            Diagnostic::error(format!(
                "failed to retain authored type-reference selection: {error:?}"
            ))
            .with_source_span(name.source_span())
        })
}

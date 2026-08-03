use crate::lowerer::Lowerer;
use psi_diagnostics::Diagnostic;
use psi_symbol_resolved_trees as resolved;
use psi_typed_trees as typed;

mod constraints;
mod direct;
mod table;

pub(crate) use constraints::lower_element_applicable_constraints;
pub(crate) use constraints::lower_type_constraints;
use direct::lower_type_reference_handle_with_context;
use table::lower_type_reference_handle_from_table_with_context;

pub(crate) fn lower_type_reference_into_table(
    lowerer: &mut Lowerer,
    type_reference: &resolved::types::TypeReference,
) -> Result<typed::types::TypeReferenceHandle, Diagnostic> {
    lower_type_reference_into_trees(
        lowerer.source_trees,
        &mut lowerer.typed_trees,
        type_reference,
    )
}

pub(crate) fn lower_type_reference_into_trees(
    source_trees: &resolved::SymbolResolvedTrees,
    typed_trees: &mut typed::TypedTrees,
    type_reference: &resolved::types::TypeReference,
) -> Result<typed::types::TypeReferenceHandle, Diagnostic> {
    lower_type_reference_handle_with_context(source_trees, typed_trees, type_reference)
}

pub(crate) fn lower_type_reference_handle_from_table(
    lowerer: &mut Lowerer,
    type_reference: resolved::types::TypeReferenceHandle,
) -> Result<typed::types::TypeReferenceHandle, Diagnostic> {
    lower_type_reference_handle_from_table_with_context(
        lowerer.source_trees,
        &mut lowerer.typed_trees,
        type_reference,
    )
}

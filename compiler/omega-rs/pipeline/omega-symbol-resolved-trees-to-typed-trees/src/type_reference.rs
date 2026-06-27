use crate::lowerer::Lowerer;
use omega_core::diagnostics::Diagnostic;
use omega_symbol_resolved_trees as resolved;
use omega_typed_trees as typed;

mod constraints;
mod direct;
mod table;

pub(crate) use constraints::lower_type_constraints;
pub(crate) use constraints::lower_type_constraint_node_span_with_context;
pub(crate) use constraints::lower_element_applicable_constraints;
use direct::lower_type_reference_handle_with_context;
use table::lower_type_reference_handle_from_table_with_context;

pub(crate) fn lower_type_reference_into_table(
    lowerer: &mut Lowerer,
    type_reference: &resolved::types::TypeReference,
) -> Result<typed::types::TypeReferenceHandle, Diagnostic> {
    lower_type_reference_handle_with_context(
        lowerer.source_trees,
        &mut lowerer.typed_trees,
        type_reference,
    )
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

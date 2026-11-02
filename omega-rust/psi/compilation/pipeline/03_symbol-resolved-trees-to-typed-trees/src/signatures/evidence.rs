//! Preserve the exact evidence telescope shared by callable and data declarations.

use crate::lowerer::Lowerer;
use crate::lowerer::name::lower_name;
use crate::type_reference::lower_type_reference_into_table;
use diagnostics::Diagnostic;

pub(crate) fn lower_conformance_bounds<'source>(
    lowerer: &mut Lowerer,
    bounds: impl IntoIterator<Item = &'source symbol_resolved_trees::machine::GenericConformanceBound>,
) -> Result<Vec<typed_trees::machine::GenericConformanceBound>, Diagnostic> {
    bounds
        .into_iter()
        .map(|bound| {
            let mut arguments = Vec::new();
            for argument in lowerer.source_trees.child_type_references(bound.arguments) {
                arguments.push(lower_type_reference_into_table(lowerer, argument)?);
            }
            Ok(typed_trees::machine::GenericConformanceBound {
                binder: bound.binder,
                binder_name: lower_name(&bound.binder_name),
                trait_symbol: bound.trait_symbol,
                trait_name: lower_name(&bound.trait_name),
                arguments,
            })
        })
        .collect()
}

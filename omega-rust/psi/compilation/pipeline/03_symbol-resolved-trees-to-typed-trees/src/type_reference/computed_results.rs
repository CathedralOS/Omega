//! Retain operation-result type vocabulary while typing owns table mutation.
//! These shells declare no operation, loan, or value predicate. Read-only result
//! queries still validate builtin meaning and exact operands before selecting one.
//! Collection elements retain inherited arithmetic policy over their complete
//! declaration, so projection and construction share one destination type after
//! typing freezes the tables. A non-Exact element policy takes precedence.
use typed_trees::TypedTrees;
use typed_trees::types::TypeReferenceHandle;

pub(crate) fn retain_computed_result_types(
    source_symbols: &symbols::SymbolTable,
    program: &mut TypedTrees,
    reference: TypeReferenceHandle,
    cast_domain: Option<numerics::arithmetic::ArithmeticDomain>,
) {
    super::retain_arithmetic_result_type(source_symbols, program, reference, cast_domain);
    program
        .type_reference_table
        .retain_collection_result_types(source_symbols, reference);
}

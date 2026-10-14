mod references;
mod traversal;

pub(crate) use references::reject_unqualified_case_values;
pub(super) use references::{
    assign_member_symbol, assign_membership_symbol, assign_name_symbol,
    assign_struct_literal_symbols, lexical_value_head, normalize_qualified_case_value,
};
pub(super) use traversal::{
    assign_expression_span_symbols, assign_expression_table_symbols,
    assign_statement_expression_symbols, assign_static_arguments_type_symbols,
};

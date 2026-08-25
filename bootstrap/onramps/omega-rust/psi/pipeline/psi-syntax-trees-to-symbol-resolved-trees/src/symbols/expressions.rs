mod references;
mod traversal;

pub(super) use references::{
    assign_member_symbol, assign_membership_symbol, assign_name_symbol,
    assign_struct_literal_symbols,
};
pub(super) use traversal::{
    assign_expression_span_symbols, assign_expression_table_symbols,
    assign_statement_expression_symbols,
};

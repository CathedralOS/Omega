mod references;
mod traversal;

pub(super) use traversal::{
    assign_expression_span_symbols, assign_expression_table_symbols,
    assign_statement_expression_symbols,
};

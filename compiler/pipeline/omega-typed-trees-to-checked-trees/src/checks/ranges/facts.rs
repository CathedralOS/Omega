use omega_core::symbols::SymbolHandle;

mod proofs;
mod values;

#[derive(Clone)]
pub(super) struct RangeFacts<'field> {
    fields: &'field [(SymbolHandle, String, usize)],
    integer_fields: Vec<(SymbolHandle, String, i64)>,
    locals: Vec<(SymbolHandle, String, usize)>,
    integer_locals: Vec<(SymbolHandle, String, i64)>,
    proven_indexes: Vec<(String, String)>,
    proven_index_upper_bounds: Vec<(String, i64)>,
    proven_orderings: Vec<(String, String)>,
    proven_range_bounds: Vec<(String, String)>,
    minimum_lengths: Vec<(String, i64)>,
}

impl<'field> RangeFacts<'field> {
    pub(super) fn new(fields: &'field [(SymbolHandle, String, usize)]) -> Self {
        Self {
            fields,
            integer_fields: Vec::new(),
            locals: Vec::new(),
            integer_locals: Vec::new(),
            proven_indexes: Vec::new(),
            proven_index_upper_bounds: Vec::new(),
            proven_orderings: Vec::new(),
            proven_range_bounds: Vec::new(),
            minimum_lengths: Vec::new(),
        }
    }
}

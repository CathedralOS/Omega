mod assignment;
mod queries;

pub(super) use assignment::{
    assign_type_reference_symbol_with_locals,
    assign_type_reference_symbol_with_locals_and_self_type,
    assign_type_reference_symbol_with_self_type, assign_type_reference_symbols,
};
pub(super) use queries::call_target_for_type_reference;

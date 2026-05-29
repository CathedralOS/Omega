mod builtin;
mod data;
mod machines;
mod operators;
mod platforms;
mod traits;

pub(super) use builtin::insert_builtin_type_symbol_children;
pub(super) use data::insert_data_symbol_children;
pub(super) use machines::insert_machine_symbol_children;
pub(super) use operators::{insert_domain_symbol_children, insert_operator_symbol_children};
pub(super) use platforms::insert_platform_symbol_children;
pub(super) use traits::insert_trait_symbol_children;

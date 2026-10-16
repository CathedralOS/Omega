//! Admit callback interfaces and rejoin registrar imports to their selected calling applications.

mod operations;
mod registrar_imports;
pub(crate) use operations::CallbackTerminalOperations;

pub(crate) use registrar_imports::{
    CallbackRegistrarOccurrence, settle_callback_registrar_imports,
};

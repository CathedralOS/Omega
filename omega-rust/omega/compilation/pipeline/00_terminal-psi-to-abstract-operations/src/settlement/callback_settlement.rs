//! Admit callback interfaces and rejoin registrar imports to their selected calling applications.

mod operations;
mod placement;
pub use placement::settle_native_callback_placements;
mod registrar_imports;
pub(crate) use operations::CallbackTerminalOperations;

pub(crate) use registrar_imports::{
    CallbackRegistrarOccurrence, settle_callback_registrar_imports,
};

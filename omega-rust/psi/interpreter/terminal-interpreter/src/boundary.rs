//! What crosses between an execution and its host. `structural_inputs` and
//! `values` are what it starts from; `effects` is what it hands to the host
//! and how host results bind; `results`, `entry_results`, `result_custody` and
//! `errors` are what it reports; `memory_ordering` is the host's one choice
//! about the interpreter's private sequential order.

pub(crate) mod effects;
pub(crate) mod entry_results;
pub(crate) mod errors;
pub(crate) mod memory_ordering;
pub(crate) mod native_code_layouts;
pub(crate) mod result_custody;
pub(crate) mod results;
pub(crate) mod structural_inputs;
pub(crate) mod values;

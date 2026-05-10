pub(crate) mod artifact_inputs;
pub mod artifacts;
pub(crate) mod backend_report;
pub mod compiler;
pub mod import_queue;
pub mod compiler_options;
pub mod old_bullshit;
pub mod source_file;
pub mod trust;

pub use compiler::{CheckOutput, CompileOutput, check, compile};
pub use omega_artifacts::PhaseTiming;
pub use compiler_options::CompileOptions;

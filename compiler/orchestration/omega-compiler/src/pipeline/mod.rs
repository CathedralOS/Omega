pub(crate) mod artifact_inputs;
pub mod artifacts;
pub mod compile;
pub mod options;
pub mod trust;

pub use compile::{CheckOutput, CompileOutput, check, compile};
pub use omega_artifacts::PhaseTiming;
pub use options::CompileOptions;

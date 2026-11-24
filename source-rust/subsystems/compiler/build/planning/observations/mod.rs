//! Compare what the host and Psi observed when executing the same program.
mod process_exit;
mod streams;

pub use process_exit::ProcessExitObservation;
pub use streams::{OutputDivergence, ProgramStream, ProgramStreams, first_output_difference};

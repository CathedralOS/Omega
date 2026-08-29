//! Executable custody and bounded, sealed Git process execution.

mod executable;
mod process;

pub(in crate::source::acquisition) use executable::*;
pub(in crate::source::acquisition) use process::*;

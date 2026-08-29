//! Executable custody and bounded, sealed Git process execution.

mod executable;
mod process;

pub(in crate::resolution::source) use executable::*;
pub(in crate::resolution::source) use process::*;

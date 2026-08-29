//! Executable custody and bounded, sealed Git process execution.

mod executable;
mod process;

pub(in crate::resolution::acquisition) use executable::*;
pub(in crate::resolution::acquisition) use process::*;

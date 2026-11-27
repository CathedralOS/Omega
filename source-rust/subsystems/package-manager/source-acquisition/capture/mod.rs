//! The immutable trees that local and Git acquisition supply to package compilation.

mod filesystem;
pub mod read_accounting;
mod tree;

pub use filesystem::SourceFileSystem;
pub use tree::{CapturedSourceNode, CapturedSourceTree};

#[cfg(test)]
mod tests;

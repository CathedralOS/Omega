#![forbid(unsafe_code)]

//! Fail-closed optimized-native realization.
//!
//! The ordinary empty-selection compiler path never enters this crate. The
//! explicit optimizer begins at [`coordination`], then descends through the
//! named custody stages cataloged by [`stages`].

mod coordination;
mod stages;

pub use coordination::*;
pub use stages::*;

#[cfg(test)]
mod tests;

//! Checked source trees and their independently established facts.
//!
//! Begin at [`checked_trees`]; validation adds facts to typed trees without
//! replacing them with a second copy of the same source vocabulary.

pub mod checked_trees;

pub use checked_trees::*;

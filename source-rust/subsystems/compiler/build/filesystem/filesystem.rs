//! Filesystem records that a Build evaluation and package review share: the
//! roots and grants a Build may use, its captured input and staged output
//! trees, and the consent requests a reviewer approves. Package review reads
//! these without depending on the compilers.

#![forbid(unsafe_code)]
#![cfg_attr(windows, feature(windows_by_handle))]

pub mod authority;
pub mod consent;
pub mod trees;

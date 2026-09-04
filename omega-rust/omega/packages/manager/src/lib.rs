#![forbid(unsafe_code)]

//! The compiler and command operations for Omega's registry-free package
//! manager.
//!
//! Start with [`operations`]. It owns complete operations and leads into the
//! checked [`declarations`], exact [`resolution`], and compiler-issued
//! [`review`] facts each operation composes.

pub mod admission;
pub mod declarations;
pub mod operations;
pub mod resolution;
pub mod review;

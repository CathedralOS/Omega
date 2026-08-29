//! Scalar-identity accounting entrance.
//!
//! Both obligation-free and proof-certified identities remove one scalar node
//! and substitute its live result. Their semantic admission remains separate;
//! only exact custody reconstruction is shared here.

use super::super::*;

mod common;
mod proof_certified;
mod total;

pub(crate) use proof_certified::*;
pub(crate) use total::*;

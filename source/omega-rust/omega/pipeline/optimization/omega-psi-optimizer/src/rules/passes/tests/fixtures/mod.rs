//! Typed optimization units shared by the pass-family test suites.

use super::*;

mod common;
mod control_flow_cleanup;
mod dead_scalar_elimination;
mod global_value_numbering;
mod proof_check_elision;
mod sparse_conditional_constant_propagation;

pub(crate) use common::*;
pub(crate) use control_flow_cleanup::*;
pub(crate) use dead_scalar_elimination::*;
pub(crate) use global_value_numbering::*;
pub(crate) use proof_check_elision::*;
pub(crate) use sparse_conditional_constant_propagation::*;

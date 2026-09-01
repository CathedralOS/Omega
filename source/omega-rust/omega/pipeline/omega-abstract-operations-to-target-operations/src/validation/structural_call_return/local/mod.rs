//! Optimizer module role: stage group. Exact local receipts for the two independently replayed closure roles.

mod callee;
mod caller;

pub(crate) use callee::{is_candidate as is_callee_candidate, validate as validate_callee};
pub(crate) use caller::{is_candidate as is_caller_candidate, validate as validate_caller};

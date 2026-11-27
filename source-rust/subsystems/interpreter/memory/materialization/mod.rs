//! Interpret admitted layout records to read, stage and write storage bytes.
//! This service never chooses a layout, activates source or generates executable
//! code.
#![forbid(unsafe_code)]

mod materialization;
pub(crate) use materialization::field_codec::{decode_aggregate_field_into, stage_scalar_field};
pub(crate) use materialization::stage_aggregate_layout;

#[cfg(test)]
use layout_plans::test_support;

#[cfg(test)]
mod contract_tests;

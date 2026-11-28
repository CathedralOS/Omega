#![forbid(unsafe_code)]

#[macro_use]
extern crate selected_instruction_analyses;

mod allocation;

pub use allocation::{
    AllocationResult, RegisterAllocationError, RetainedAllocation, stage_register_allocation,
};

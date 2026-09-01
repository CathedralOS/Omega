//! Bounded-process module role: executable entrance. Bounded duplex child execution.
//!
//! `model` owns the closed request, result, and error vocabulary. `budget`
//! owns shared retained-output accounting. `execution` coordinates transfer,
//! deadline enforcement, process-container closure, and completion.

mod budget;
mod execution;
mod model;

pub use budget::{BoundedCaptureBudget, BoundedCaptureBudgetExceeded};
pub use model::{
    BoundedCaptureLimits, BoundedProcessInput, BoundedProcessOutput, BoundedProcessRunError,
    BoundedProcessStream,
};

use crate::BoundedProcessPrepared;

pub fn run_bounded_process(
    prepared: BoundedProcessPrepared,
    input: BoundedProcessInput,
    limits: BoundedCaptureLimits,
    captured_output_budget: BoundedCaptureBudget,
) -> Result<BoundedProcessOutput, BoundedProcessRunError> {
    execution::execute(prepared, input, limits, captured_output_budget)
}

#[cfg(test)]
mod tests;

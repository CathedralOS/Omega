// Shared captured-output byte accounting (`GitCapturedOutputBudget`) across all
// child commands within one Git resolution.

pub(crate) use bounded_process::BoundedCaptureBudget as GitCapturedOutputBudget;

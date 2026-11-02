//! Scope vocabulary retained by selected-composition accounting.

/// The execution scope whose known executable entries are being described.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionScope {
    CallerAddressSpace,
    IsolatedProvider(u64),
}

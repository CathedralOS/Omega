#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionScope {
    CallerAddressSpace,
    IsolatedProvider(u64),
}

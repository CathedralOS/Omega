use super::crash_route::CrashRouteRef;

/// The index belongs to the forest that typed the indirect invocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GuardedInvocationRef {
    Local(usize),
    Library(usize),
}

/// A checked crash route for an execution in this typed stage. A
/// specialization is the direct call that supplies an indirect callee.
#[derive(Debug, Eq, PartialEq)]
pub enum GuardedCall {
    /// `call` indexes `TypedStage::calls`.
    Direct { call: usize, route: CrashRouteRef },
    /// `specialization` indexes the caller forest's `TypedStage::calls`.
    Indirect {
        call: GuardedInvocationRef,
        route: CrashRouteRef,
        specialization: Option<usize>,
    },
}

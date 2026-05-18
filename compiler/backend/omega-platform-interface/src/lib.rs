use omega_calling_conventions::{HostOperationKey, PlatformCallData, PlatformCallLoweringHandle};
use omega_checked_trees::expression::{ExpressionHandle, ExpressionTable};
use omega_control_flow::StateKey;
use omega_core::arena::{Arena, HandleSpan};
use omega_target::NativeTarget;
use std::sync::Arc;

mod host_calls;
mod place_keys;

pub use host_calls::{build_host_call_plan, build_host_call_plan_with_workers};
pub use place_keys::PlaceKey;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostCallPlan {
    pub expressions: ExpressionTable,
    pub calls: Arena<HostCall>,
    pub unsupported_calls: Arena<UnsupportedHostCall>,
    pub operations: Arena<LoweredHostOperation>,
    pub arguments: Arena<HostCallArgument>,
}

impl Default for HostCallPlan {
    fn default() -> Self {
        Self::with_capacity(0, 0, 0, 0)
    }
}

impl HostCallPlan {
    pub fn with_capacity(
        call_capacity: usize,
        unsupported_call_capacity: usize,
        operation_capacity: usize,
        argument_capacity: usize,
    ) -> Self {
        Self {
            expressions: ExpressionTable::new(),
            calls: Arena::with_capacity(call_capacity),
            unsupported_calls: Arena::with_capacity(unsupported_call_capacity),
            operations: Arena::with_capacity(operation_capacity),
            arguments: Arena::with_capacity(argument_capacity),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct UnsupportedHostCall {
    pub source_key: StateKey,
    pub statement_index: usize,
    pub platform_call: String,
    pub reason: UnsupportedHostCallReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnsupportedHostCallReason {
    NoNativeLowering { target: NativeTarget },
}

impl Default for UnsupportedHostCallReason {
    fn default() -> Self {
        Self::NoNativeLowering {
            target: NativeTarget::host(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostCall {
    pub source_key: StateKey,
    pub statement_index: usize,
    pub lowering: PlatformCallLoweringHandle,
    pub data: PlatformCallData,
    pub operations: HandleSpan<LoweredHostOperation>,
    pub arguments: HandleSpan<HostCallArgument>,
}

impl Default for HostCall {
    fn default() -> Self {
        Self {
            source_key: StateKey::default(),
            statement_index: 0,
            lowering: PlatformCallLoweringHandle::invalid(),
            data: PlatformCallData::None,
            operations: HandleSpan::empty(),
            arguments: HandleSpan::empty(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredHostOperation {
    pub operation_key: HostOperationKey,
}

impl Default for LoweredHostOperation {
    fn default() -> Self {
        Self {
            operation_key: HostOperationKey::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostCallArgument {
    pub kind: HostCallArgumentKind,
}

impl Default for HostCallArgument {
    fn default() -> Self {
        Self {
            kind: HostCallArgumentKind::Expression(ExpressionHandle::invalid()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostCallArgumentKind {
    Text(Arc<str>),
    Integer(i64),
    Expression(ExpressionHandle),
}

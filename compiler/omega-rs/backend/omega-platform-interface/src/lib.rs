use omega_calling_conventions::{HostOperationKey, PlatformCallData, PlatformCallLoweringHandle};
use omega_control_flow::StateKey;
use omega_target::NativeTarget;
use psi_arena::{Arena, HandleSpan};
use psi_checked_trees::expression::{ExpressionHandle, ExpressionTable};
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
            expressions: ExpressionTable::with_expression_capacity(
                operation_capacity.saturating_add(argument_capacity),
            ),
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
    pub call_ordinal: usize,
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
    pub call_ordinal: usize,
    pub lowering: PlatformCallLoweringHandle,
    pub data: PlatformCallData,
    pub operations: HandleSpan<LoweredHostOperation>,
    pub arguments: HandleSpan<HostCallArgument>,
    /// True when the call's RESULT PLACE was prepended as argument[0] (the
    /// assignment / let-initializer collectors). Catalog ops key result
    /// handling per operation; source external ops (Unknown key) have no
    /// bespoke arm, so this flag is selection's only signal that argument[0]
    /// is a writable place rather than the first declared argument.
    pub has_result: bool,
}

impl Default for HostCall {
    fn default() -> Self {
        Self {
            source_key: StateKey::default(),
            statement_index: 0,
            call_ordinal: 0,
            lowering: PlatformCallLoweringHandle::invalid(),
            data: PlatformCallData::None,
            operations: HandleSpan::empty(),
            arguments: HandleSpan::empty(),
            has_result: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredHostOperation {
    pub operation_key: HostOperationKey,
    pub fixed_leading_immediate: Option<i64>,
}

impl Default for LoweredHostOperation {
    fn default() -> Self {
        Self {
            operation_key: HostOperationKey::default(),
            fixed_leading_immediate: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostCallArgument {
    pub kind: HostCallArgumentKind,
    /// The checked source argument retained an explicit mutable-borrow wrapper.
    /// Lowering strips that wrapper to resolve the pointee place, so native
    /// selection carries this bit to preserve its address semantics.
    pub is_borrowed: bool,
    /// The resolved boundary requirement expects a reference at this position.
    /// Immutable `&place` syntax is erased during parsing, so this formal-type
    /// fact is required to recover borrowed-place passing honestly.
    pub expects_reference: bool,
    /// The resolved boundary requirement expects an inert raw address at this
    /// position. Immutable `&place` syntax is erased during parsing, so a
    /// non-address place must contribute its storage address; an `addr`-typed
    /// actual already contains the value and must not acquire another layer.
    pub expects_address: bool,
}

impl Default for HostCallArgument {
    fn default() -> Self {
        Self {
            kind: HostCallArgumentKind::Expression(ExpressionHandle::invalid()),
            is_borrowed: false,
            expects_reference: false,
            expects_address: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostCallArgumentKind {
    Text(Arc<[u8]>),
    Integer(i64),
    Expression(ExpressionHandle),
}

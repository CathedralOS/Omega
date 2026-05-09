use crate::abi::PlatformCallData;
use crate::control_flow::StateKey;
use omega_core::arena::{Arena, HandleSpan};
use omega_typed_program::expression::Expression;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostCallPlan {
    pub calls: Arena<HostCall>,
    pub unsupported_calls: Arena<UnsupportedHostCall>,
    pub operations: Arena<LoweredHostOperation>,
    pub arguments: Arena<HostCallArgument>,
}

impl Default for HostCallPlan {
    fn default() -> Self {
        Self {
            calls: Arena::new(),
            unsupported_calls: Arena::new(),
            operations: Arena::new(),
            arguments: Arena::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct UnsupportedHostCall {
    pub source_key: StateKey,
    pub statement_index: usize,
    pub platform_call: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostCall {
    pub source_key: StateKey,
    pub statement_index: usize,
    pub platform_call: String,
    pub data: PlatformCallData,
    pub operations: HandleSpan<LoweredHostOperation>,
    pub arguments: HandleSpan<HostCallArgument>,
}

impl Default for HostCall {
    fn default() -> Self {
        Self {
            source_key: StateKey::default(),
            statement_index: 0,
            platform_call: String::new(),
            data: PlatformCallData::None,
            operations: HandleSpan::empty(),
            arguments: HandleSpan::empty(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredHostOperation {
    pub capability: String,
    pub operation: String,
}

impl Default for LoweredHostOperation {
    fn default() -> Self {
        Self {
            capability: String::new(),
            operation: String::new(),
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
            kind: HostCallArgumentKind::Expression(Expression::Integer(0)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostCallArgumentKind {
    Text(String),
    Integer(i64),
    Expression(Expression),
}

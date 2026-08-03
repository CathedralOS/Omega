use omega_control_flow::StateKey;
use psi_arena::{Arena, Handle, HandleSpan};
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbstractDataPlan {
    pub objects: Arena<AbstractDataObject>,
    pub bytes: Arena<u8>,
}

impl Default for AbstractDataPlan {
    fn default() -> Self {
        Self::with_capacity(0, 0)
    }
}

impl AbstractDataPlan {
    pub fn with_capacity(object_capacity: usize, byte_capacity: usize) -> Self {
        Self {
            objects: Arena::with_capacity(object_capacity),
            bytes: Arena::with_capacity(byte_capacity),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbstractDataObject {
    pub symbol: Arc<str>,
    pub kind: AbstractDataObjectKind,
    pub offset: usize,
    pub bytes: HandleSpan<u8>,
    pub alignment: usize,
    pub source_key: StateKey,
    pub source_statement: usize,
}

pub type AbstractDataObjectHandle = Handle<AbstractDataObject>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AbstractDataObjectKind {
    StaticString,
    RuntimeTextBuffer,
    HostNewline,
    #[default]
    Other,
}

impl Default for AbstractDataObject {
    fn default() -> Self {
        Self {
            symbol: Arc::from(""),
            kind: AbstractDataObjectKind::Other,
            offset: 0,
            bytes: HandleSpan::empty(),
            alignment: 1,
            source_key: StateKey::default(),
            source_statement: 0,
        }
    }
}

pub type TargetDataPlan = AbstractDataPlan;
pub type TargetDataObject = AbstractDataObject;
pub type TargetDataObjectHandle = AbstractDataObjectHandle;
pub type TargetDataObjectKind = AbstractDataObjectKind;

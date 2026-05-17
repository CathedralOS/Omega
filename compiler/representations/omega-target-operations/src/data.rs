use omega_control_flow::StateKey;
use omega_core::arena::{Arena, Handle, HandleSpan};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetDataPlan {
    pub objects: Arena<TargetDataObject>,
    pub bytes: Arena<u8>,
}

impl Default for TargetDataPlan {
    fn default() -> Self {
        Self {
            objects: Arena::new(),
            bytes: Arena::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetDataObject {
    pub symbol: String,
    pub kind: TargetDataObjectKind,
    pub offset: usize,
    pub bytes: HandleSpan<u8>,
    pub alignment: usize,
    pub source_key: StateKey,
    pub source_statement: usize,
}

pub type TargetDataObjectHandle = Handle<TargetDataObject>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TargetDataObjectKind {
    StaticString,
    RuntimeTextBuffer,
    HostNewline,
    #[default]
    Other,
}

impl Default for TargetDataObject {
    fn default() -> Self {
        Self {
            symbol: String::new(),
            kind: TargetDataObjectKind::Other,
            offset: 0,
            bytes: HandleSpan::empty(),
            alignment: 1,
            source_key: StateKey::default(),
            source_statement: 0,
        }
    }
}

use crate::control_flow::StateKey;
use omega_core::arena::{Arena, HandleSpan};
use omega_typed_program::name::ProgramName;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeDataPlan {
    pub objects: Arena<NativeDataObject>,
    pub bytes: Arena<u8>,
}

impl Default for NativeDataPlan {
    fn default() -> Self {
        Self {
            objects: Arena::new(),
            bytes: Arena::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeDataObject {
    pub symbol: String,
    pub offset: usize,
    pub bytes: HandleSpan<u8>,
    pub alignment: usize,
    pub source_key: StateKey,
    pub source_machine: ProgramName,
    pub source_state: ProgramName,
    pub source_statement: usize,
}

impl Default for NativeDataObject {
    fn default() -> Self {
        Self {
            symbol: String::new(),
            offset: 0,
            bytes: HandleSpan::empty(),
            alignment: 1,
            source_key: StateKey::default(),
            source_machine: ProgramName::default(),
            source_state: ProgramName::default(),
            source_statement: 0,
        }
    }
}

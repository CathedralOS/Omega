use crate::name::ProgramName;
use crate::signature::StateSignature;
use omega_core::symbols::SymbolHandle;
use std::ops::{Deref, DerefMut};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Platform {
    pub symbol: SymbolHandle,
    pub name: ProgramName,
    pub storage: PlatformStorage,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlatformStorage {
    pub states: Vec<StateSignature>,
}

impl Deref for Platform {
    type Target = PlatformStorage;

    fn deref(&self) -> &Self::Target {
        &self.storage
    }
}

impl DerefMut for Platform {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.storage
    }
}

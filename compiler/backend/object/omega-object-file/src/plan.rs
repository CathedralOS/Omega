use crate::{ObjectSymbolHandle, SectionPlan, SymbolPlan};
use omega_core::arena::Arena;
use omega_target::NativeTarget;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectFileLayout {
    pub sections: Arena<SectionPlan>,
    pub symbols: Arena<SymbolPlan>,
    pub entry_symbol: ObjectSymbolHandle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectPlan {
    pub target: NativeTarget,
    pub layout: ObjectFileLayout,
}

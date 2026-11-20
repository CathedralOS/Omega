use crate::ValueType;
use omega_target_contract::{InstructionParameter, InstructionSemantics, TargetIdentity};
use std::{collections::HashMap, sync::Arc};

/// Compiler-owned scalar instruction catalog selected for this source package.
/// The same immutable catalog supplies name resolution and all typing retries.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssemblyTypes {
    pub target: TargetIdentity,
    pub registers: HashMap<String, ValueType>,
    pub instructions: HashMap<String, Vec<InstructionVariant>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstructionVariant {
    pub parameters: Vec<InstructionParameter<ValueType>>,
    pub semantics: Arc<InstructionSemantics>,
}

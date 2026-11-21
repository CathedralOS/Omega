use crate::ValueType;
use omega_target_contract::{InstructionParameter, InstructionSemantics, LocationLifetime, LocationRole, TargetIdentity};
use std::{collections::HashMap, sync::Arc};

/// Compiler-owned scalar instruction catalog selected for this source package.
/// The same immutable catalog supplies name resolution and all typing retries.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssemblyTypes {
    pub target: TargetIdentity,
    /// Every named location the catalog's contracts may read or write.
    pub locations: HashMap<String, AssemblyLocation>,
    pub instructions: HashMap<String, Vec<InstructionVariant>>,
}

impl AssemblyTypes {
    /// The location with this name when it is a register, the only kind source code can name.
    pub fn register(
        &self,
        name: &str,
    ) -> Option<&AssemblyLocation> {
        self.locations
            .get(name)
            .filter(|location| location.role == LocationRole::Register)
    }

    /// Register names in a stable order, for name resolution.
    pub fn register_names(&self) -> Vec<&str> {
        let mut names: Vec<_> = self
            .locations
            .iter()
            .filter(|(_, location)| location.role == LocationRole::Register)
            .map(|(name, _)| name.as_str())
            .collect();
        names.sort_unstable();
        names
    }
}

/// One catalog location. Its role and lifetime are independent declarations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssemblyLocation {
    pub ty: ValueType,
    pub role: LocationRole,
    pub lifetime: LocationLifetime,
}

impl AssemblyLocation {
    /// A working register or scratch flag: a register scoped to one region.
    pub fn working_register(ty: ValueType) -> Self {
        Self {
            ty,
            role: LocationRole::Register,
            lifetime: LocationLifetime::Region,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstructionVariant {
    pub parameters: Vec<InstructionParameter<ValueType>>,
    pub semantics: Arc<InstructionSemantics>,
}

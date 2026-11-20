use arena::Arena;
use semantic_vocabulary::{ScalarApplicationId, ScalarType, StructuralTypeId};

use crate::TargetMachineCallable;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScalarApplications {
    declarations: Arena<ScalarApplication>,
    pub conditions: super::ScalarConditions,
    pub proof_scope: super::ProofApplicationScope,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ScalarApplication {
    #[default]
    Missing,
    LayoutObservation {
        subject: StructuralTypeId,
        measurement: LayoutMeasurement,
        semantics_version: u32,
        scalar_type: ScalarType,
    },
    Evaluate {
        entry: TargetMachineCallable,
        scalar_type: ScalarType,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutMeasurement {
    Size,
    Alignment,
}

impl LayoutMeasurement {
    pub const SEMANTICS_VERSION: u32 = 1;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ArrayExtent {
    Literal(u64),
    Application(ScalarApplicationId),
    /// A generic body's exact const input. Zero is admitted: unlike an
    /// integer width, an array extent contributes no positivity premise.
    Parameter {
        owner: semantic_vocabulary::MachineId,
        value: semantic_vocabulary::ValueId,
    },
}

impl ArrayExtent {
    pub const fn literal(self) -> Option<u64> {
        match self {
            Self::Literal(length) => Some(length),
            Self::Application(_) | Self::Parameter { .. } => None,
        }
    }
}

impl From<u64> for ArrayExtent {
    fn from(length: u64) -> Self {
        Self::Literal(length)
    }
}

impl std::fmt::Display for ArrayExtent {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Literal(length) => write!(formatter, "{length}"),
            Self::Application(application) => {
                write!(formatter, "pending application #{}", application.get())
            }
            Self::Parameter { owner, value } => write!(
                formatter,
                "const input #{} of machine #{}",
                value.get(),
                owner.get()
            ),
        }
    }
}

impl ScalarApplication {
    pub const fn scalar_type(self) -> Option<ScalarType> {
        match self {
            Self::Missing => None,
            Self::LayoutObservation { scalar_type, .. }
            | Self::Evaluate { scalar_type, .. } => Some(scalar_type),
        }
    }
}

impl ScalarApplications {
    pub fn insert(
        &mut self,
        declaration: ScalarApplication,
    ) -> Result<ScalarApplicationId, &'static str> {
        if declaration == ScalarApplication::Missing {
            return Err("a scalar application cannot publish the arena sentinel");
        }
        let coordinate = u64::try_from(self.declarations.len())
            .ok()
            .and_then(|length| length.checked_add(1))
            .and_then(ScalarApplicationId::new)
            .ok_or("scalar application namespace is exhausted")?;
        self.declarations.insert(declaration);
        Ok(coordinate)
    }

    pub fn declaration(&self, application: ScalarApplicationId) -> Option<&ScalarApplication> {
        let position = usize::try_from(application.get().checked_sub(1)?).ok()?;
        self.declarations.storage_slice().get(position)
    }

    pub fn declarations(&self) -> &[ScalarApplication] {
        self.declarations.storage_slice()
    }

    pub fn is_empty(&self) -> bool {
        self.declarations.is_empty() && self.conditions.is_empty()
    }
}

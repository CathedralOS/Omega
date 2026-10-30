//! Portable scalar computations whose values belong to an admitted target.
//!
//! The catalog is immutable semantic input, not a cache of selected answers.
//! Executable reads, contract terms and array extents name the same application.
//! Ordinary machines own arithmetic and calls; this catalog adds no expression
//! interpreter. An application has no value until its target closure admits it.

use arena::Arena;
use semantic_vocabulary::{ScalarApplicationId, ScalarType, StructuralTypeId};

use crate::TerminalCallable;

/// A canonical, append-only application namespace. IDs are one-based wire
/// coordinates; arena generations never escape as portable identities.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScalarApplications {
    declarations: Arena<ScalarApplication>,
    pub conditions: super::ScalarConditions,
    pub proof_scope: super::ProofApplicationScope,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ScalarApplication {
    /// Arena sentinel only. A published catalog cannot contain this row.
    #[default]
    Missing,
    LayoutObservation {
        subject: StructuralTypeId,
        measurement: LayoutMeasurement,
        semantics_version: u32,
        scalar_type: ScalarType,
    },
    /// The complete closed computation, not its first nested helper. Runtime
    /// arguments are empty; other application reads are explicit operations in
    /// the retained body. Selected families keep their portable head identity.
    Evaluate {
        entry: TerminalCallable,
        scalar_type: ScalarType,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutMeasurement {
    Size,
    Alignment,
}

impl LayoutMeasurement {
    /// Bytes of the selected structural representation, including final padding.
    pub const SEMANTICS_VERSION: u32 = 1;
}

/// Declared element count, independent of the target's admitted storage view.
/// An unresolved application is never interpreted as an empty array.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ArrayExtent {
    Literal(u64),
    Application(ScalarApplicationId),
}

impl ArrayExtent {
    pub const fn literal(self) -> Option<u64> {
        match self {
            Self::Literal(length) => Some(length),
            Self::Application(_) => None,
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
        }
    }
}

impl ScalarApplication {
    pub const fn scalar_type(self) -> Option<ScalarType> {
        match self {
            Self::Missing => None,
            Self::LayoutObservation { scalar_type, .. } | Self::Evaluate { scalar_type, .. } => {
                Some(scalar_type)
            }
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

    /// Wire order is application identity order. There is no removal or reuse.
    pub fn declarations(&self) -> &[ScalarApplication] {
        self.declarations.storage_slice()
    }

    pub fn is_empty(&self) -> bool {
        self.declarations.is_empty() && self.conditions.is_empty()
    }
}

//! The two meanings of `as` (programmable-layouts brief §5b): `as` on a
//! VALUE converts; `as` on a BORROW re-views the place's bytes under a
//! second stated shape. The form rides the one Cast node through every
//! tree so no phase can silently treat a re-view as a conversion.

/// Which `as` a cast expression spells.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CastForm {
    /// `x as T` — value conversion (decision 17's domain retag rides here).
    #[default]
    Value,
    /// Re-view the same backing with the stated reference access. Shared
    /// views require source-to-target validity; both exclusive access modes
    /// require reverse validity as well. Write-only access remains distinct
    /// from readable mutable access at formation, transport and observation.
    Recast(crate::ReferenceAccess),
}

impl CastForm {
    pub fn is_recast(self) -> bool {
        matches!(self, Self::Recast(_))
    }

    pub const fn recast_access(self) -> Option<crate::ReferenceAccess> {
        match self {
            Self::Value => None,
            Self::Recast(access) => Some(access),
        }
    }
}

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
    /// `&x as &T` — the borrowed place's bytes revealed under the stated
    /// shape. Address identity at runtime; legality is the static §5b
    /// judgment (size/align/fact implication, source→target).
    RecastShared,
    /// `&mut x as &mut T` — as above, writable: the judgment requires fact
    /// implication in BOTH directions (writes through the view must leave
    /// the source valid at release).
    RecastMutable,
}

impl CastForm {
    pub fn is_recast(self) -> bool {
        !matches!(self, Self::Value)
    }

    /// The `as`-target spelling for diagnostics (`&T` / `&mut T` prefixes).
    pub fn target_prefix(self) -> &'static str {
        match self {
            Self::Value => "",
            Self::RecastShared => "&",
            Self::RecastMutable => "&mut ",
        }
    }
}

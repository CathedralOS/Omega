//! The PLACE algebra (codegen cleanup Phase 6, the Copy* pilot): one memory
//! location = a BASE region plus a short path of composable steps. The
//! Copy*/Write* operation-kind PRODUCT (operation x source-addressing x
//! dest-addressing) collapses into this data -- addressing modes become
//! OPERANDS, not opcodes, and one per-target materializer replaces the
//! per-variant hand encoders (see wiki/architecture/
//! codegen_representation_cleanup.md Phase 6 for the accepted diagnosis).
//!
//! ZII: a `Place` is a flat `Copy` value -- an inline fixed-capacity step
//! array, no allocation; the zero value is "the base of the Machine region",
//! which is inert until steps are pushed.

use super::storage::RuntimeStorageRegion;

/// One composable addressing step. A nested member of an indexed element of
/// a pointee is a path of three steps, not a new opcode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaceStep {
    /// Advance the address by a constant byte offset.
    ConstOffset(usize),
    /// Load the 8-byte pointer AT the current address and continue from the
    /// loaded value (a slice descriptor's data pointer, a `&mut` reference
    /// slot). A `ConstOffset` BEFORE the deref positions the pointer slot;
    /// one AFTER walks into the pointee.
    Deref,
    /// Advance by a runtime index times a constant element size. The index
    /// is read from `index_region + index_offset` when the address is
    /// materialized.
    ScaledIndex {
        index_region: RuntimeStorageRegion,
        index_offset: usize,
        /// Declared width of the unsigned index slot. Address materializers
        /// must load exactly this many bytes and zero-extend them; guessing a
        /// machine word can consume adjacent storage, while truncating to a
        /// canonical width can alias a distinct high-bit index.
        index_byte_size: usize,
        element_byte_size: usize,
    },
}

impl Default for PlaceStep {
    fn default() -> Self {
        PlaceStep::ConstOffset(0)
    }
}

/// The maximum path depth. Six covers a frame-held pointer slot, its deref,
/// one fixed offset on either side of two scaled indices, and the two indices
/// themselves. The builder saturates loudly past it (`push_step` returns
/// `false`) so a too-deep place becomes a legalization refusal, never a silent
/// truncation.
pub const PLACE_MAX_STEPS: usize = 6;

/// A memory place: base region + path. `Copy`, flat, ZII-inert.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Place {
    pub region: RuntimeStorageRegion,
    steps: [PlaceStep; PLACE_MAX_STEPS],
    step_count: u8,
}

impl Place {
    /// The base of `region` advanced by `byte_offset` -- the plain
    /// (non-indexed, non-deref) place every direct operand uses.
    pub fn at(region: RuntimeStorageRegion, byte_offset: usize) -> Self {
        let mut place = Place {
            region,
            ..Place::default()
        };
        let pushed = place.push_step(PlaceStep::ConstOffset(byte_offset));
        debug_assert!(pushed);
        place
    }

    /// Append a step, merging adjacent const offsets (the normalization that
    /// keeps materialized addressing minimal). Returns `false` -- refuse
    /// loudly at the call site -- when the path is full.
    #[must_use]
    pub fn push_step(&mut self, step: PlaceStep) -> bool {
        if let PlaceStep::ConstOffset(added) = step {
            if added == 0 && self.step_count > 0 {
                return true;
            }
            if let Some(PlaceStep::ConstOffset(existing)) = self.last_step_mut() {
                *existing += added;
                return true;
            }
        }
        if usize::from(self.step_count) == PLACE_MAX_STEPS {
            return false;
        }
        self.steps[usize::from(self.step_count)] = step;
        self.step_count += 1;
        true
    }

    /// Builder form of [`push_step`]; `None` when the path is full.
    #[must_use]
    pub fn with_step(mut self, step: PlaceStep) -> Option<Self> {
        self.push_step(step).then_some(self)
    }

    pub fn steps(&self) -> &[PlaceStep] {
        &self.steps[..usize::from(self.step_count)]
    }

    fn last_step_mut(&mut self) -> Option<&mut PlaceStep> {
        match self.step_count {
            0 => None,
            count => Some(&mut self.steps[usize::from(count) - 1]),
        }
    }

    /// The region of the place's (single) ScaledIndex slot, if any step
    /// carries one -- the relocation walker patches cross-region index
    /// bases from it.
    pub fn scaled_index_region(&self) -> Option<RuntimeStorageRegion> {
        self.steps().iter().find_map(|step| match step {
            PlaceStep::ScaledIndex { index_region, .. } => Some(*index_region),
            _ => None,
        })
    }

    /// EVERY ScaledIndex step's index region, in walk order -- the
    /// double-index rung's walker feed (`scaled_index_region` keeps
    /// returning the first).
    pub fn scaled_index_regions(&self) -> impl Iterator<Item = RuntimeStorageRegion> + '_ {
        self.steps().iter().filter_map(|step| match step {
            PlaceStep::ScaledIndex { index_region, .. } => Some(*index_region),
            _ => None,
        })
    }

    /// The place's constant byte offset when the whole path is const -- the
    /// direct-operand fast path (and the shape the plain Copy encoders
    /// expect). `None` if any step derefs or indexes.
    pub fn const_offset(&self) -> Option<usize> {
        let mut total = 0usize;
        for step in self.steps() {
            match step {
                PlaceStep::ConstOffset(offset) => total += offset,
                PlaceStep::Deref | PlaceStep::ScaledIndex { .. } => return None,
            }
        }
        Some(total)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adjacent_const_offsets_merge() {
        let mut place = Place::at(RuntimeStorageRegion::RuntimeFrame, 16);
        assert!(place.push_step(PlaceStep::ConstOffset(8)));
        assert_eq!(place.steps().len(), 1);
        assert_eq!(place.const_offset(), Some(24));
    }

    #[test]
    fn deref_paths_are_not_const() {
        let place = Place::at(RuntimeStorageRegion::RuntimeFrame, 16)
            .with_step(PlaceStep::Deref)
            .unwrap()
            .with_step(PlaceStep::ConstOffset(4))
            .unwrap();
        assert_eq!(place.const_offset(), None);
        assert_eq!(place.steps().len(), 3);
    }

    #[test]
    fn path_depth_saturates_loudly() {
        let mut place = Place::at(RuntimeStorageRegion::Machine, 0);
        assert!(place.push_step(PlaceStep::Deref));
        assert!(place.push_step(PlaceStep::Deref));
        assert!(place.push_step(PlaceStep::Deref));
        assert!(place.push_step(PlaceStep::Deref));
        assert!(place.push_step(PlaceStep::Deref));
        assert!(!place.push_step(PlaceStep::Deref));
    }

    #[test]
    fn pointee_double_index_geometry_retains_every_step() {
        let place = Place::at(RuntimeStorageRegion::RuntimeFrame, 24)
            .with_step(PlaceStep::Deref)
            .unwrap()
            .with_step(PlaceStep::ConstOffset(4))
            .unwrap()
            .with_step(PlaceStep::ScaledIndex {
                index_region: RuntimeStorageRegion::Machine,
                index_offset: 80,
                index_byte_size: 8,
                element_byte_size: 12,
            })
            .unwrap()
            .with_step(PlaceStep::ScaledIndex {
                index_region: RuntimeStorageRegion::RuntimeFrame,
                index_offset: 40,
                index_byte_size: 4,
                element_byte_size: 2,
            })
            .unwrap()
            .with_step(PlaceStep::ConstOffset(6))
            .unwrap();

        assert_eq!(
            place.steps(),
            &[
                PlaceStep::ConstOffset(24),
                PlaceStep::Deref,
                PlaceStep::ConstOffset(4),
                PlaceStep::ScaledIndex {
                    index_region: RuntimeStorageRegion::Machine,
                    index_offset: 80,
                    index_byte_size: 8,
                    element_byte_size: 12,
                },
                PlaceStep::ScaledIndex {
                    index_region: RuntimeStorageRegion::RuntimeFrame,
                    index_offset: 40,
                    index_byte_size: 4,
                    element_byte_size: 2,
                },
                PlaceStep::ConstOffset(6),
            ]
        );
        assert_eq!(
            place.scaled_index_regions().collect::<Vec<_>>(),
            vec![
                RuntimeStorageRegion::Machine,
                RuntimeStorageRegion::RuntimeFrame
            ]
        );
        assert!(place.with_step(PlaceStep::Deref).is_none());
    }

    #[test]
    fn zero_value_is_the_machine_base() {
        let place = Place::default();
        assert_eq!(place.region, RuntimeStorageRegion::Machine);
        assert_eq!(place.const_offset(), Some(0));
    }
}

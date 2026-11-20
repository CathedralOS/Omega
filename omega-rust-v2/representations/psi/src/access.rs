use crate::ValueId;
/// An operation owns each bound in its path. The receiving checker reconstructs
/// the extent from the root carrier and the preceding path segments.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct RuntimeIndex {
    pub selector: ValueId,
    pub obligation: BoundsObligationId,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct BoundsObligationId(pub std::num::NonZeroUsize);

impl BoundsObligationId {
    pub fn get(self) -> usize {
        self.0.get()
    }
}

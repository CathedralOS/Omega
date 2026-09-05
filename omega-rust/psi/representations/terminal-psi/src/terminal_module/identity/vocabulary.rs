/// Marker for the single unstable terminal-Psi semantic vocabulary.
///
/// The in-memory representation accepts only the vocabulary it was built with.
/// The terminal codec may migrate an explicitly supported prior wire vocabulary
/// before constructing this marker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VocabularyMarker;

impl VocabularyMarker {
    pub const CURRENT: Self = Self;

    pub const fn new(raw: u16) -> Option<Self> {
        if raw == Self::CURRENT.get() {
            Some(Self::CURRENT)
        } else {
            None
        }
    }

    pub const fn get(self) -> u16 {
        80
    }
}

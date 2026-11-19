//! The implementations supplied for the five indexing declarations in the standard interface.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CollectionPrimitive {
    Read,
    Element,
    ElementMut,
    Range,
    RangeMut,
}

impl CollectionPrimitive {
    pub const ALL: [Self; 5] = [
        Self::Read,
        Self::Element,
        Self::ElementMut,
        Self::Range,
        Self::RangeMut,
    ];

    pub fn path(self) -> &'static str {
        match self {
            Self::Read => "indexing::read",
            Self::Element => "indexing::element",
            Self::ElementMut => "indexing::element_mut",
            Self::Range => "indexing::range",
            Self::RangeMut => "indexing::range_mut",
        }
    }
}

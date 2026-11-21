use super::{DescriptorLocation, DescriptorPathSegment};
use crate::storage::borrowed_storage::Projection;
use semantic_vocabulary::{PlaceId, ScalarTerm, StructuralTypeId};
use terminal_psi::StructuralAccess;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum DescriptorContentsStep {
    Address(DescriptorPathSegment),
    Element {
        element: StructuralTypeId,
        // The same typed selector term as ScalarReadProjection::Index.
        // Without one the query ranges over all possible elements.
        selector: Option<Box<ScalarTerm>>,
    },
}

impl DescriptorContentsStep {
    pub(crate) fn may_match(&self, other: &Self) -> bool {
        self == other
            || matches!(
                (self, other),
                (
                    Self::Element { .. },
                    Self::Address(DescriptorPathSegment::FixedIndex(_))
                ) | (
                    Self::Address(DescriptorPathSegment::FixedIndex(_)),
                    Self::Element { .. }
                )
            )
            || matches!(
                (self, other),
                (Self::Element { element: left, .. }, Self::Element { element: right, .. })
                    if left == right
            )
    }
    pub fn any_element(element: StructuralTypeId) -> Self {
        Self::Element {
            element,
            selector: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct DescriptorContentsQuery {
    pub root: PlaceId,
    pub steps: Vec<DescriptorContentsStep>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DescriptorFlowSource {
    Location(DescriptorLocation),
    Contents(DescriptorContentsQuery),
    Unavailable(PlaceId),
}

impl From<DescriptorLocation> for DescriptorFlowSource {
    fn from(location: DescriptorLocation) -> Self {
        Self::Location(location)
    }
}

impl DescriptorFlowSource {
    pub fn from_query(root: PlaceId, query: Option<DescriptorContentsQuery>) -> Self {
        query.map(Self::Contents).unwrap_or(Self::Unavailable(root))
    }
    pub fn exact_location(&self) -> Option<&DescriptorLocation> {
        match self {
            Self::Location(location) => Some(location),
            _ => None,
        }
    }
    pub fn observation(&self) -> DescriptorLocation {
        match self {
            Self::Location(location) => location.clone(),
            Self::Contents(query) => DescriptorLocation::whole(query.root),
            Self::Unavailable(root) => DescriptorLocation::whole(*root),
        }
    }
}

#[derive(Clone)]
pub struct DescriptorFlowBinding {
    pub source: DescriptorFlowSource,
    pub parameter: DescriptorLocation,
    pub access: StructuralAccess,
    /// Permission held by the actual projection before this edge.
    /// A weaker header starts a child borrow, not a second name for
    /// its parent's authority.
    pub source_access: StructuralAccess,
    /// The actual descriptor and relative address captured before the
    /// edge installs its parameter. Contents queries may erase selectors;
    /// this projection retains their original formation coordinates.
    pub captured_storage: Option<(DescriptorLocation, Projection)>,
}

impl DescriptorFlowBinding {
    pub fn new(
        source: impl Into<DescriptorFlowSource>,
        parameter: DescriptorLocation,
        actual_access: StructuralAccess,
        parameter_access: StructuralAccess,
    ) -> Option<Self> {
        (actual_access == parameter_access).then_some(Self {
            source: source.into(),
            parameter,
            access: parameter_access,
            source_access: actual_access,
            captured_storage: None,
        })
    }

    pub fn locations(&self) -> Option<(&DescriptorLocation, &DescriptorLocation)> {
        Some((self.source.exact_location()?, &self.parameter))
    }
}

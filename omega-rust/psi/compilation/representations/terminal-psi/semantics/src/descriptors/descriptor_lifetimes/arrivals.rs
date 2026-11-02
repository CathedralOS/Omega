//! Typed simultaneous arrivals retain value selection separately from storage.
//! Every receiving parameter has one row. A family of possible contents is not
//! an exact address, and unavailable contents supply no provenance authority.
use super::{DescriptorLocation, DescriptorPathSegment};
use semantic_vocabulary::{PlaceId, StructuralTypeId};
use terminal_psi::StructuralAccess;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum DescriptorContentsStep {
    Address(DescriptorPathSegment),
    AnyElement { element: StructuralTypeId },
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct DescriptorContentsQuery {
    pub root: PlaceId,
    pub steps: Vec<DescriptorContentsStep>,
}

#[derive(Clone)]
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

/// One simultaneous carrier arrival, retaining the access agreed by its typed
/// actual and receiving parameter. This analysis data grants no permission.
#[derive(Clone)]
pub struct DescriptorFlowBinding {
    pub source: DescriptorFlowSource,
    pub parameter: DescriptorLocation,
    pub access: StructuralAccess,
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
        })
    }

    pub fn locations(&self) -> Option<(&DescriptorLocation, &DescriptorLocation)> {
        Some((self.source.exact_location()?, &self.parameter))
    }
}

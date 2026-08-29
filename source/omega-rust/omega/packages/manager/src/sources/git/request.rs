//! One package selection over one validated Git acquisition request.

use crate::project::dependencies::read::PackageSelection;
use omega_package_source::{GitSourceRequest, GitTransportProfile};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitPackageSourceRequest {
    acquisition: GitSourceRequest,
    selection: PackageSelection,
}

impl GitPackageSourceRequest {
    pub fn new(acquisition: GitSourceRequest, selection: PackageSelection) -> Self {
        Self {
            acquisition,
            selection,
        }
    }

    pub fn root(acquisition: GitSourceRequest) -> Self {
        Self::new(acquisition, PackageSelection::Root)
    }

    pub const fn acquisition(&self) -> &GitSourceRequest {
        &self.acquisition
    }

    pub const fn selection(&self) -> &PackageSelection {
        &self.selection
    }

    pub fn requested_locator(&self) -> &str {
        self.acquisition.requested_locator()
    }

    pub fn requested_revision(&self) -> &str {
        self.acquisition.requested_revision()
    }

    pub fn transport_profile(&self) -> GitTransportProfile {
        self.acquisition.transport_profile()
    }
}

//! One package selection over one validated Git acquisition request.

use crate::declarations::dependencies::read::PackageSelection;
use package_source::{GitSourceRequest};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GitPackageSourceRequest {
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


    pub const fn acquisition(&self) -> &GitSourceRequest {
        &self.acquisition
    }

    pub const fn selection(&self) -> &PackageSelection {
        &self.selection
    }



}

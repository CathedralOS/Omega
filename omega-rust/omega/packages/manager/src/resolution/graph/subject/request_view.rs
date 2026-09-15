//! Borrow the common authored request fields without cloning hostile strings.

use super::CanonicalDependencySourceRequest;
use crate::declarations::dependencies::read::DependencySourceRequest;
use crate::declarations::{AliasName, PackageSelection};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Request<'a> {
    Path {
        explicit_alias: Option<&'a AliasName>,
        location: &'a str,
    },
    Git {
        explicit_alias: Option<&'a AliasName>,
        repository: &'a str,
        revision: &'a str,
        selection: &'a PackageSelection,
    },
}

impl<'a> From<&'a CanonicalDependencySourceRequest> for Request<'a> {
    fn from(value: &'a CanonicalDependencySourceRequest) -> Self {
        match value {
            CanonicalDependencySourceRequest::Path {
                explicit_alias,
                location,
            } => Self::Path {
                explicit_alias: explicit_alias.as_ref(),
                location,
            },
            CanonicalDependencySourceRequest::Git {
                explicit_alias,
                repository,
                revision,
                selection,
            } => Self::Git {
                explicit_alias: explicit_alias.as_ref(),
                repository,
                revision,
                selection,
            },
        }
    }
}

impl<'a> From<&'a DependencySourceRequest> for Request<'a> {
    fn from(value: &'a DependencySourceRequest) -> Self {
        match value {
            DependencySourceRequest::Path {
                explicit_alias,
                location,
            } => Self::Path {
                explicit_alias: explicit_alias.as_ref(),
                location,
            },
            DependencySourceRequest::Git {
                explicit_alias,
                repository,
                revision,
                selection,
            } => Self::Git {
                explicit_alias: explicit_alias.as_ref(),
                repository,
                revision,
                selection,
            },
        }
    }
}

impl<'a> Request<'a> {
    pub(super) fn explicit_alias(self) -> Option<&'a AliasName> {
        match self {
            Self::Path { explicit_alias, .. } | Self::Git { explicit_alias, .. } => explicit_alias,
        }
    }
}

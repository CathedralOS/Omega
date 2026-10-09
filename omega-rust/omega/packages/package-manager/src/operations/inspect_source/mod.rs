//! Read-only package-source inspection.

mod execution;
mod report;
mod request;

pub(crate) use request::{ PackageSourceRequest, PackageSourceRequestParseError,
    SourceAdapter,
};

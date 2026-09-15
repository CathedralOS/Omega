use crate::declarations::dependencies::read::{
    DependencyPurpose, DependencySourceRequest, PackageSelection,
};
use build_declarations::DependencyOperation;
use sha2::{Digest, Sha256};

use super::BUILDER_PARAMETER_NAME;

/// Render one ordinary Omega statement for the row's authorized scope.
/// Caller-controlled strings cannot add syntax, lines, comments, or review
/// prose because they remain escaped bytes inside string literals.
pub fn canonical_dependency_statement(
    purpose: DependencyPurpose,
    request: &DependencySourceRequest,
) -> String {
    let operation = match (purpose.is_product(), request.explicit_alias().is_some()) {
        (true, true) => DependencyOperation::DependAs,
        (true, false) => DependencyOperation::Depend,
        (false, true) => DependencyOperation::BuildDependAs,
        (false, false) => DependencyOperation::BuildDepend,
    };
    let alias = match request.explicit_alias() {
        Some(alias) => format!(
            "{}, ",
            source::display_literal_bytes(alias.as_str().as_bytes())
        ),
        None => String::new(),
    };
    let source = match request {
        DependencySourceRequest::Path { location, .. } => format!(
            "Source::Path {{ location: {} }}",
            source::display_literal_bytes(location.as_bytes())
        ),
        DependencySourceRequest::Git {
            repository,
            revision,
            selection,
            ..
        } => {
            let selection = match selection {
                PackageSelection::Root => String::new(),
                PackageSelection::Named(package) => format!(
                    ", selection: PackageSelection::Named {{ package: {} }}",
                    source::display_literal_bytes(package.as_str().as_bytes())
                ),
            };
            format!(
                "Source::Git {{ repository: {}, revision: {}{selection} }}",
                source::display_literal_bytes(repository.as_bytes()),
                source::display_literal_bytes(revision.as_bytes())
            )
        }
    };
    format!(
        "{BUILDER_PARAMETER_NAME}.{}({alias}{source});",
        operation.name()
    )
}

pub(super) fn source_digest(source: &str) -> [u8; 32] {
    Sha256::digest(source.as_bytes()).into()
}

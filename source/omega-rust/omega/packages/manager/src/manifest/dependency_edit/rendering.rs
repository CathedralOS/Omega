use crate::manifest::dependency_projection::DependencySourceRequest;
use sha2::{Digest, Sha256};

use super::BUILDER_PARAMETER_NAME;

/// Render one ordinary Omega statement. Caller-controlled strings cannot add
/// syntax, lines, comments, or review prose because they remain escaped bytes
/// inside string literals.
pub fn canonical_dependency_statement(request: &DependencySourceRequest) -> String {
    let (operation, alias) = match request.explicit_alias() {
        Some(alias) => (
            "depend_as",
            format!(
                "{}, ",
                psi_source::display_literal_bytes(alias.as_str().as_bytes())
            ),
        ),
        None => ("depend", String::new()),
    };
    let source = match request {
        DependencySourceRequest::Path { location, .. } => format!(
            "Source::Path {{ location: {} }}",
            psi_source::display_literal_bytes(location.as_bytes())
        ),
        DependencySourceRequest::Git {
            repository,
            revision,
            ..
        } => format!(
            "Source::Git {{ repository: {}, revision: {} }}",
            psi_source::display_literal_bytes(repository.as_bytes()),
            psi_source::display_literal_bytes(revision.as_bytes())
        ),
    };
    format!("{BUILDER_PARAMETER_NAME}.{operation}({alias}{source});")
}

pub(super) fn source_digest(source: &str) -> [u8; 32] {
    Sha256::digest(source.as_bytes()).into()
}

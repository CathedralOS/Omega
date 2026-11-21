//! The source provider tests publish these programs and the native tests check their plans.

use std::path::Path;

pub const UNREACHED_DEFAULT: &str = "unreached_default.psi";
pub const UNREACHED_HELPER: &str = "unreached_helper.psi";
pub const REACHED_DEFAULT: &str = "reached_default.psi";
pub const OVERRIDE: &str = "override.psi";
pub const MISSING: &str = "missing.psi";
pub const FOREIGN_DEFAULT: &str = "foreign_default.psi";

pub fn open(repository: &Path, file_name: &str) -> terminal_codec::CanonicalTerminalArtifact {
    let path = repository
        .join("tests/fixtures/providers/pass/source_provider_choices")
        .join(file_name);
    let bytes = std::fs::read(&path)
        .unwrap_or_else(|error| panic!("could not read {}: {error}", path.display()));
    terminal_codec::CanonicalTerminalArtifact::from_bytes(&bytes)
        .unwrap_or_else(|error| panic!("could not open {}: {error:?}", path.display()))
}

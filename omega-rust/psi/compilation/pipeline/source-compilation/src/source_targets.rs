//! Target observations admitted before source preparation. These are data:
//! source closure cannot call a native planner to discover more declarations.

use diagnostics::Diagnostic;

#[derive(Debug, Clone)]
pub struct SourceTarget {
    pub name: &'static str,
    pub entry: semantic_vocabulary::ProgramEntrySourceContract,
    pub boundary_schema: Option<&'static str>,
    pub entry_contract: Option<SourceEntryContract>,
    pub layout: Option<layout_plans::LayoutTargetFacts>,
    pub address_model: Option<semantic_vocabulary::ScalarAddressModel>,
    pub numeric_support: language_core::float_operations::NumericTargetCapabilities,
}

#[derive(Debug, Clone)]
pub struct SourceEntryContract {
    pub manifest_identity: &'static str,
    pub relative_source: &'static str,
    pub owner: SourceEntryContractOwner,
    pub accepted_declaration: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceEntryContractOwner {
    Bundled,
    Package(semantic_vocabulary::PackageKeyIdentity),
    Unavailable,
}

pub(crate) fn target_named<'a>(
    targets: &'a [SourceTarget],
    name: &str,
) -> Result<&'a SourceTarget, Diagnostic> {
    targets
        .iter()
        .find(|target| target.name == name)
        .ok_or_else(|| Diagnostic::error(format!("unknown canonical target profile `{name}`")))
}

pub(crate) fn target_owner<'a>(
    targets: &'a [SourceTarget],
    owner: &str,
) -> Result<&'a SourceTarget, Diagnostic> {
    targets
        .iter()
        .find(|target| target.entry.owner == owner)
        .ok_or_else(|| Diagnostic::error(format!("unknown target root-slot owner `{owner}`")))
}

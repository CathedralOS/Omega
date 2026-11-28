//! Target observations admitted before source preparation. These are data:
//! source closure cannot call a native planner to discover more declarations.

use checked_program_to_psi::provider_choice_inputs;
use diagnostics::Diagnostic;

/// Layout observations and the exact source declarations that demand them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixedLayoutTargetScope {
    pub target: String,
    pub facts: Option<layout_plans::LayoutTargetFacts>,
    /// Check geometry consumed by subject declarations for this requested output.
    pub checks_subject: bool,
    pub entry_declarations: Vec<symbols::SymbolHandle>,
    pub provider_declarations: Vec<provider_choice_inputs::ProviderSelection>,
}

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
    pub package_owner: Option<semantic_vocabulary::PackageKeyIdentity>,
    pub installed_root: std::path::PathBuf,
    pub source_digest: [u8; 32],
}

impl SourceEntryContract {
    pub(crate) fn owns(&self, file: &source_admission::SourceFile) -> bool {
        file.package_identity == self.package_owner
            && file.package_root == self.installed_root
            && file.path.strip_prefix(&file.package_root).ok()
                == Some(std::path::Path::new(self.relative_source))
            && file.resolution_stratum == source_admission::SourceResolutionStratum::Base
            && file.dependency_scope == source_provenance::DependencyScope::Product
            && matches!(
                (file.origin, file.package_identity),
                (source_admission::SourceOrigin::ToolchainContract, None)
                    | (source_admission::SourceOrigin::User, Some(_))
            )
            && source_provenance::entry_contract_digest(
                self.manifest_identity,
                self.relative_source,
                file.source.as_bytes(),
            ) == self.source_digest
    }
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

#[cfg(test)]
mod tests {
    use super::SourceEntryContract;
    use source_admission::{SourceFile, SourceOrigin, SourceResolutionStratum};
    use source_provenance::DependencyScope;

    #[test]
    fn entry_custody_requires_the_designated_package_file_and_product_occurrence() {
        let mut contract = SourceEntryContract {
            package_owner: None,
            installed_root: "/installed/system".into(),
            manifest_identity: "target::entry",
            relative_source: "target/entry.omg",
            source_digest: source_provenance::entry_contract_digest(
                "target::entry",
                "target/entry.omg",
                b"contract",
            ),
        };
        let mut file = SourceFile {
            source_id: source_text::SourceId(0),
            path: contract.installed_root.join(contract.relative_source),
            package_root: contract.installed_root.clone(),
            package_identity: None,
            dependency_scope: DependencyScope::Product,
            origin: SourceOrigin::ToolchainContract,
            resolution_stratum: SourceResolutionStratum::Base,
            source: "contract".into(),
        };
        assert!(contract.owns(&file));
        file.origin = SourceOrigin::User;
        assert!(!contract.owns(&file));
        file.package_identity =
            Some(semantic_vocabulary::PackageKeyIdentity::from_digest([1; 32]).unwrap());
        assert!(!contract.owns(&file));
        contract.package_owner =
            Some(semantic_vocabulary::PackageKeyIdentity::from_digest([2; 32]).unwrap());
        assert!(!contract.owns(&file));
        contract.package_owner = file.package_identity;
        assert!(contract.owns(&file));
        file.package_root = "/dependency".into();
        file.path = file.package_root.join(contract.relative_source);
        assert!(!contract.owns(&file));
        file.package_root = contract.installed_root.clone();
        file.path = file
            .package_root
            .join("impostor")
            .join(contract.relative_source);
        assert!(!contract.owns(&file));
        file.path = file.package_root.join(contract.relative_source);
        file.dependency_scope = DependencyScope::Build;
        assert!(!contract.owns(&file));
        file.dependency_scope = DependencyScope::Product;
        file.source = "changed".into();
        assert!(!contract.owns(&file));
    }
}

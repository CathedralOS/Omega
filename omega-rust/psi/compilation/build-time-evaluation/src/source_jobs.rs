//! Source jobs do not execute through retired tree/checked-program engines.
use crate::BuildTimeSelectionAuthority;
use diagnostics::Diagnostic;
use std::sync::Arc;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;

fn unavailable(job: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(format!("no product route yet: {job}"))]
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixedLayoutTargetScope {
    pub target: String,
    pub facts: Option<layout_plans::LayoutTargetFacts>,
    pub entry_declarations: Option<Vec<symbols::SymbolHandle>>,
    pub provider_declarations: Vec<crate::product::provider_declarations::ProviderSelection>,
}
pub fn has_fixed_layout_applications(typed: &TypedTrees) -> bool {
    typed_trees::references::collect_program_type_occurrences(typed)
        .into_iter()
        .any(|occurrence| {
            typed
                .layout_storage_arguments(occurrence.reference)
                .is_some()
        })
}
pub fn evaluate_fixed_layout_applications(
    typed: &mut TypedTrees,
    _scopes: &[FixedLayoutTargetScope],
    _authority: Option<Arc<dyn BuildTimeSelectionAuthority>>,
) -> Result<(), Vec<Diagnostic>> {
    if has_fixed_layout_applications(typed) {
        return Err(unavailable("source layout-policy evaluation"));
    }
    Ok(())
}
pub fn is_exact_toolchain_build_prelude_data(
    typed: &TypedTrees,
    symbol: SymbolHandle,
    expected_name: &str,
) -> bool {
    typed.data_definitions().iter().any(|definition| {
        definition.symbol == symbol
            && definition.name.as_str() == expected_name
            && typed.symbols.is_build_prelude_symbol(symbol)
    })
}
pub fn admit_toolchain_build_activation(_typed: &TypedTrees) -> Result<bool, Vec<Diagnostic>> {
    Err(unavailable("source Build activation admission"))
}

pub use crate::evaluation_requests::{
    CapturedFilesystemRoot as BuildMachineCapturedFilesystemRoot,
    FilesystemAccess as BuildMachineFilesystemAccess,
};
pub use evaluation_accounting::{BuildEvaluationSponsor, BuildEvaluationSponsorLimits};
pub use filesystem_authority::{
    FilesystemGrantRoot as BuildMachineFilesystemGrantRoot,
    FilesystemGrantRootIdentity as BuildMachineFilesystemGrantRootIdentity,
    FilesystemMetadataLayout as BuildMachineFilesystemMetadataLayout,
    FilesystemSponsor as BuildMachineFilesystemSponsor,
    FilesystemSponsorLimits as BuildMachineFilesystemSponsorLimits,
    FsGrants as BuildMachineFilesystemGrants,
};
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BuildMachineExecutionMode {
    Pure,
    Granted {
        filesystem: BuildMachineFilesystemAccess,
        filesystem_metadata_layout: BuildMachineFilesystemMetadataLayout,
    },
}
/// The input source is retained as metadata, never as a checked-program witness.
pub struct PreparedBuildMachineProgram {
    source: Arc<TypedTrees>,
}
#[derive(Clone)]
pub struct PreparedBuildMachineEntry {
    source: Arc<TypedTrees>,
    symbol: SymbolHandle,
}
impl PreparedBuildMachineEntry {
    pub const fn symbol(&self) -> SymbolHandle {
        self.symbol
    }
    pub fn source(&self) -> &TypedTrees {
        &self.source
    }
}
impl PreparedBuildMachineProgram {
    pub fn prepare(typed: &TypedTrees) -> Result<Self, Vec<Diagnostic>> {
        if typed != &TypedTrees::default() {
            return Err(unavailable("source Build machine preparation"));
        }
        Ok(Self {
            source: Arc::new(typed.clone()),
        })
    }
    pub fn typed(&self) -> &TypedTrees {
        &self.source
    }
    pub fn entry(&self, symbol: SymbolHandle) -> Result<PreparedBuildMachineEntry, String> {
        if self.source.machine_by_symbol(symbol).is_none() {
            return Err("no product route yet: source Build entry is unavailable".into());
        }
        Ok(PreparedBuildMachineEntry {
            source: Arc::clone(&self.source),
            symbol,
        })
    }
}

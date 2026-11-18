//! The portable result of resolving source coordinates and package permissions.
//! Native planning receives the canonical artifact and closed logical choices.
use semantic_vocabulary::{
    BoundaryMachineId, MachineId, compiler_evaluation::ProductDeclarationScope,
};
use std::sync::Arc;
use terminal_semantics::{
    callable_values::CallableBoundaryImplementation, logical_providers::ClosedLogicalProviders,
};

pub struct ProviderExecutionOrigins {
    pub artifact: Arc<terminal_codec::CanonicalTerminalArtifact>,
    pub bindings: Vec<
        Result<
            (
                BoundaryMachineId,
                evaluation_receipts::NativeBindingMaterialization,
            ),
            String,
        >,
    >,
    pub schema_custody:
        Result<terminal_installation::selection_questions::ProductSchemaCustody, String>,
    pub package_permissions: Option<ProductDeclarationScope>,
    pub authority_permissions: Result<
        (
            terminal_psi::TerminalIdentityTable,
            Vec<effects::ServiceTerminalAuthorityPermission>,
        ),
        String,
    >,
}

pub enum PublishedProviderDemand {
    Unscoped {
        artifact: Arc<terminal_codec::CanonicalTerminalArtifact>,
        roots: Vec<MachineId>,
    },
    Target {
        profile: target::TargetProfile,
        logical: ClosedLogicalProviders,
        folded_entry: Result<crate::FoldedEntryReach, String>,
        bodies: Vec<CallableBoundaryImplementation>,
        labels: Vec<(BoundaryMachineId, String)>,
        origins: ProviderExecutionOrigins,
        review: Option<crate::package_review::PackageReviewRequirements>,
    },
}

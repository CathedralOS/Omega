//! Canonical installation record: the manifest metadata sealed over one
//! emitted executable image. `record_types.rs` holds the record carriers and
//! `installation_errors.rs` names the errors. The record grants no executable
//! authority.

pub(crate) mod graph_structural;
mod installation_errors;
pub(crate) mod record_shape;
mod record_types;

pub use installation_errors::InstallationError;

pub(crate) use record_types::{
    ImageFingerprint, InitializedDataFingerprint, InstallationFingerprint, InstallationRecord,
    InstalledCompilerPrivateFunction, InstalledComponentProgress, InstalledForeignCallStack,
    InstalledFunction, InstalledImageSections, InstalledInternalUnitCall,
    InstalledInternalUnitScalarCall, InstalledStructuralReturn, SelectedProviderPlanReportIdentity};

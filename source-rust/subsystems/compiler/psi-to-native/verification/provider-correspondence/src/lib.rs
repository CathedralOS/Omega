//! Match selected plans, applications and candidate bodies to their exact declarations.
//! Admission and native planning both use these checks over accepted Terminal records.

mod bodies;
mod boundary;
pub mod callbacks;
pub mod external_providers;
mod installation_reach;
pub use installation_reach::{InstallationReachRow, resolve_installation_reach};

pub use bodies::{
    CallbackProviderDeclarations, SelectedCallbackRow, select_callback_declarations,
    selected_external_execution,
};

pub use boundary::{CompilerIntrinsicPlanProvenance, select_boundary_applications};

pub mod trust;

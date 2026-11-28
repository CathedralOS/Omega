//! Selected optimization alone issues the completed input to allocation.
#![forbid(unsafe_code)]
pub struct SelectedBodyIssuer(());
pub const SELECTED_BODY: SelectedBodyIssuer = SelectedBodyIssuer(());

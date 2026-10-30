//! Closed declaration associations for already selected provider plans.
//!
//! These records contain no checked trees or source handles. Keeping each whole
//! plan with its selecting declaration distinguishes providers that share code.
//! Neither this carrier nor constructing it grants execution authority.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeProviderSources {
    pub declarations: Vec<crate::TerminalTargetProviderPlan>,
}

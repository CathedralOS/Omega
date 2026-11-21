use crate::builtins::SelectedCompilerBuiltin;
use std::collections::BTreeSet;

/// Provider rows selected for the reachable boundaries of one native entry.
pub struct ScopedProviderBindings {
    pub external_bindings: Vec<calling_conventions::ExternalBindingRow>,
    pub demands: BTreeSet<String>,
    pub compiler_builtins: Vec<SelectedCompilerBuiltin>,
    pub boundary_application_coverage: boundary_applications::TerminalBoundaryApplicationCoverage,
}

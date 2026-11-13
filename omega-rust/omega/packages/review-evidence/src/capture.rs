/*
Compiler-owned translation from checked semantic and execution outputs into
inert package-review evidence.

We coordinate checked semantic interpretation (`semantics`), public API and
contract projection (`api`, `contracts`), operational behavior and calling plans
(`behavior`, `callables`, `calling`), provider and representation disclosure
(`providers`, `representation`), and source custody (`source`, `input`). Each
projection module defines its own diagnostic sentence via `refusal!` and maps
missing lookups into `Vec<Diagnostic>` via `EvidenceOption::or_evidence`.
*/

mod api;
mod authority;
mod behavior;
mod callables;
mod calling;
mod contracts;
mod input;
mod package_policy;
mod package_review;
pub use package_review::{PackageReviewProjectionError, project_package_review};
mod providers;
mod representation;
mod semantic_bindings;
mod semantics;
pub use semantic_bindings::{
    SemanticBindingProjectionError, candidate_hosted_exit_bindings, candidate_service_bindings,
    candidate_target_entry_binding,
};
mod runtime_permissions;
mod source;
pub use runtime_permissions::PreparedRuntimePolicy;
mod terminal_authority_permissions;

macro_rules! refusal {
    ($parameter:ident, $sentence:literal) => {
        fn rejected($parameter: &str) -> Vec<diagnostics::Diagnostic> {
            vec![diagnostics::Diagnostic::error(format!($sentence))]
        }
    };
    ($visibility:vis $parameter:ident, $sentence:literal) => {
        $visibility fn rejected($parameter: &str) -> Vec<diagnostics::Diagnostic> {
            vec![diagnostics::Diagnostic::error(format!($sentence))]
        }
    };
}
pub(crate) use refusal;

pub(crate) trait EvidenceOption<T> {
    fn or_evidence(self, message: impl Into<String>) -> Result<T, Vec<diagnostics::Diagnostic>>;
}

impl<T> EvidenceOption<T> for Option<T> {
    fn or_evidence(self, message: impl Into<String>) -> Result<T, Vec<diagnostics::Diagnostic>> {
        self.ok_or_else(|| vec![diagnostics::Diagnostic::error(message)])
    }
}

pub(crate) use callables::project_checked_callable_policy;
pub(crate) use input::SourceDeclarationInput;
pub use input::{IntoPackageReviewInput, PackageReviewInput};
pub use package_policy::project_checked_package_policy;
pub(crate) use providers::project_checked_selected_provider_policy;
pub(crate) use representation::project_checked_representation_policy;
pub(crate) use semantics::conformances::project_checked_conformance_policy;
pub(crate) use semantics::declarations::nominal_identity;
pub use terminal_authority_permissions::project_checked_terminal_permission_policy;

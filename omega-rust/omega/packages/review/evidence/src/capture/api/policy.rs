//! Public declaration policy captured from exact checked declaration owners.
//!
//! Legacy review shapes remain unchanged. Their ordinary public-surface joins
//! are reused before policy-specific signature and behavior projection.

mod declarations;
pub(crate) use declarations::conformances;
mod traits;
use crate::capture::semantics::signatures::policy as signatures;
use signatures::values;

use crate::record::PackagePolicyPublicApi;
use omega_compiler::CheckedCompilation;
use psi_core::PackageKeyIdentity;
use psi_diagnostics::Diagnostic;

pub(crate) fn project(
    compilation: &CheckedCompilation,
    package: PackageKeyIdentity,
) -> Result<PackagePolicyPublicApi, Vec<Diagnostic>> {
    Ok(PackagePolicyPublicApi {
        traits: traits::project(compilation, package)?,
        conformances: declarations::conformances(compilation, package)?,
        domains: declarations::domains(compilation, package)?,
        propositions: super::propositions::project_public_propositions(compilation, package)?
            .into_iter()
            .map(|projected| projected.row)
            .collect(),
        consts: super::constants::project_public_consts(compilation, package)?
            .into_iter()
            .map(|projected| projected.row)
            .collect(),
        operators: declarations::operators(compilation, package)?,
        data: declarations::data(compilation, package)?,
    })
}

fn rejected(message: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(format!("public API policy: {message}"))]
}

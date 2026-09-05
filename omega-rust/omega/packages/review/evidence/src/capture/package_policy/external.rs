//! All authored executable supplies, independently of public visibility or use.

use crate::capture::callables::project_checked_external_supply_policy;
use crate::capture::semantics::declarations::{nominal_identity, reviewed_package_owns};
use crate::record::PackagePolicyExternalExecutableSupply;
use omega_compiler::CheckedCompilation;
use psi_core::PackageKeyIdentity;
use psi_diagnostics::Diagnostic;
use psi_language_semantics::MachineSupplyMode;

pub(super) fn project(
    compilation: &CheckedCompilation,
    package: PackageKeyIdentity,
) -> Result<Vec<PackagePolicyExternalExecutableSupply>, Vec<Diagnostic>> {
    let mut rows = Vec::new();
    for machine in compilation.machines().iter().filter(|machine| {
        matches!(
            machine.supply_mode,
            MachineSupplyMode::ExternalRealization { .. }
        )
    }) {
        let identity = nominal_identity(compilation, machine.symbol)?;
        if !reviewed_package_owns(&identity, package)? {
            continue;
        }
        rows.extend(project_checked_external_supply_policy(
            compilation,
            machine.symbol,
        )?);
    }
    rows.sort();
    if rows.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(super::rejected(
            "external supply repeats a complete policy identity",
        ));
    }
    Ok(rows)
}

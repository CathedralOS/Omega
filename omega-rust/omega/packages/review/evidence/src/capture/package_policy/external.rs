//! All authored executable supplies, independently of public visibility or use.

use crate::capture::callables::project_checked_external_supply_policy;
use crate::capture::semantics::declarations::{nominal_identity, reviewed_package_owns};
use crate::record::PackagePolicyExternalExecutableSupply;
use compiler::CheckedCompilation;
use diagnostics::Diagnostic;
use language_semantics::MachineSupplyMode;
use semantic_vocabulary::PackageKeyIdentity;

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

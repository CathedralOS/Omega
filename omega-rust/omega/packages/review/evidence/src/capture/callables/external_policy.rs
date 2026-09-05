//! Lossless executable-supply policy from exact checked machine handles.
mod bindings;
mod requirements;
mod signatures;

use crate::capture::semantics::conformances::policy_callable_identity;
use crate::record::{PackagePolicyExternalBinding, PackagePolicyExternalExecutableSupply};
use compiler::CheckedCompilation;
use diagnostics::Diagnostic;
use symbols::SymbolHandle;

/// A legacy review row cannot recover result absence or nested static policy.
/// Resolve the exact checked leaf and project those fields from their owners.
pub fn project_checked_external_supply_policy(
    compilation: &CheckedCompilation,
    symbol: SymbolHandle,
) -> Result<Vec<PackagePolicyExternalExecutableSupply>, Vec<Diagnostic>> {
    let machines = compilation
        .machines()
        .iter()
        .filter(|machine| machine.symbol == symbol)
        .collect::<Vec<_>>();
    let [machine] = machines.as_slice() else {
        return Err(rejected("supply has no unique checked machine"));
    };
    let conformances = compilation.machine_trait_conformances(machine);
    let [conformance] = conformances else {
        return Err(rejected(
            "external leaf must have one exact realization edge",
        ));
    };
    let binding = bindings::project(compilation, machine, conformance)?;
    let (binders, signature) = signatures::project(compilation, machine)?;
    let requirement = requirements::project(compilation, machine, conformance, &binders, &binding)?;
    if let crate::record::PackagePolicyExternalRequirement::TopLevelRequirement {
        signature: required,
        ..
    } = &requirement
        && !super::boundary_requirements::provider_conformance_bounds_refine(
            required.conformance_bounds(),
            signature.conformance_bounds(),
        )
    {
        return Err(rejected(
            "external provider demands a conformance bound not guaranteed by its requirement",
        ));
    }
    let row = PackagePolicyExternalExecutableSupply {
        callable: policy_callable_identity(compilation, machine.symbol)?,
        signature,
        requirement,
        binding: PackagePolicyExternalBinding::from(&binding),
    };
    row.validate_canonical_structure().map_err(rejected)?;
    Ok(vec![row])
}

fn rejected(reason: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(format!(
        "external-supply policy rejects {reason}"
    ))]
}

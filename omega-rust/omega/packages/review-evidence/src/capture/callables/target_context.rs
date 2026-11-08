// Authored target-scope projection through the exact family-member association.
// Target selection mutates `Machine::target` without changing the family-member
// association, so portability is never inferred from a selected member's cleared
// target marker.

use crate::capture::PackageReviewInput;
use crate::record::PackagePolicyTargetContext;
use diagnostics::Diagnostic;
use typed_trees::machine::Machine;

pub(super) fn project(
    compilation: &PackageReviewInput<'_>,
    machine: &Machine,
) -> Result<PackagePolicyTargetContext, Vec<Diagnostic>> {
    Ok(match compilation.authored_machine_target(machine.symbol)? {
        Some(target) => PackagePolicyTargetContext::TargetMember {
            target: target.to_owned(),
        },
        None => PackagePolicyTargetContext::TargetIndependent,
    })
}

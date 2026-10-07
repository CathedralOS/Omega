//! Project authored target scope through the exact family-member association.

use crate::capture::PackageReviewInput;
use crate::record::PackagePolicyTargetContext;
use diagnostics::Diagnostic;
use typed_trees::machine::Machine;

pub(super) fn project(
    compilation: &PackageReviewInput<'_>,
    machine: &Machine,
) -> Result<PackagePolicyTargetContext, Vec<Diagnostic>> {
    // Selection changes Machine::target, but never the member association.
    // Do not infer portability from the selected member's cleared marker.
    Ok(match compilation.authored_machine_target(machine.symbol)? {
        Some(target) => PackagePolicyTargetContext::TargetMember {
            target: target.to_owned(),
        },
        None => PackagePolicyTargetContext::TargetIndependent,
    })
}

// Complete checked callable policy projection for one root activation and target.
// This captures semantics only and neither accepts assumptions nor reconstitutes
// compiler certificates, build replay, or native authority. Selected dispatch
// changes no checked body, so the settled trees are the authored source the policy
// callables classify. A source checked in both the product scope and the build scope
// yields two machine instances with identical policy facts, so identical sorted rows
// collapse via `dedup()`.

use super::surface;
use crate::capture::PackageReviewInput;
use crate::capture::behavior::policy as behavior;
use crate::capture::refusal;
use crate::capture::semantics::conformances::policy_callable_identity;
use crate::capture::semantics::declarations::{is_checked_source_instance, nominal_identity};
use crate::capture::source::service_reach::validate_machine_service_reach;
use crate::record::{
    PackagePolicyCallable, PackagePolicyCallableRole, PackagePolicyCallables,
    PackageReviewCallableRole, PackageReviewNominalOwner,
};
use diagnostics::Diagnostic;
use language_semantics::MachineSupplyMode;
use semantic_vocabulary::PackageKeyIdentity;
use target::TargetProfile;

pub(crate) fn project_checked_callable_policy<'a>(
    _input: impl Into<PackageReviewInput<'a>>,
    _target: TargetProfile,
    _package: PackageKeyIdentity,
) -> Result<PackagePolicyCallables, Vec<Diagnostic>> {
    Err(vec![diagnostics::Diagnostic::error(
        "no product route yet: package review requires lowered-form evidence",
    )])
}

refusal!(reason, "callable policy rejects {reason}");

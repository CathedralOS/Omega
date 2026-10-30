use crate::record::PackagePolicyCallable;
use crate::record::PackagePolicyCallableConformance;
use crate::record::PackagePolicyCallableRole;
use crate::record::PackagePolicyCapabilityFlow;
use crate::record::PackagePolicyCrash;
use crate::record::PackagePolicyMutation;
use crate::record::PackagePolicyServiceReachDependency;
use crate::record::PackagePolicyTermination;
use crate::record::PackagePolicyTypeParameter;
use crate::record::PackageReviewCallableContract;
use crate::record::PackageReviewCallableSupply;
use crate::record::PackageReviewCheckedServiceReach;
use crate::record::PackageReviewConformanceBound;
use crate::record::PackageReviewInstallationReach;
use crate::record::PackageReviewNominalIdentity;
use crate::record::PackageReviewOperatorRealization;
use crate::record::PackageReviewSignatureParameter;
use crate::record::PackageReviewSynchronousInvocation;
use crate::record::PackageReviewTypeIdentity;
use crate::record::accessors::field_projection_accessors;

impl PackagePolicyCallable {
    field_projection_accessors!(copied spelling: Option<language_core::OperatorSpelling>);
    field_projection_accessors!(copied lifetime_parameter_count: usize);
    field_projection_accessors!(sliced type_parameters: &[PackagePolicyTypeParameter]);
    field_projection_accessors!(sliced conformance_bounds: &[PackageReviewConformanceBound]);
    field_projection_accessors!(sliced parameters: &[PackageReviewSignatureParameter]);
    field_projection_accessors!(optional return_type: Option<&PackageReviewTypeIdentity>);
    field_projection_accessors!(borrowed target_context: &crate::record::PackagePolicyTargetContext);
    field_projection_accessors!(copied role: PackagePolicyCallableRole);

    field_projection_accessors!(borrowed identity: &PackageReviewNominalIdentity);

    field_projection_accessors!(copied supply: PackageReviewCallableSupply);
    field_projection_accessors!(sliced conformances: &[PackagePolicyCallableConformance]);
    field_projection_accessors!(sliced operator_realizations: &[PackageReviewOperatorRealization]);
    field_projection_accessors!(sliced contracts: &[PackageReviewCallableContract]);

    pub fn declared_service_reach(&self) -> Option<&[PackageReviewNominalIdentity]> {
        self.declared_service_reach.as_deref()
    }

    /// A callable publishes its explicit/governing ceiling, or the full
    /// checked realized row when its body requests inference. Concrete
    /// realization cannot attenuate that contract; unknown bodies supply no
    /// inferred fallback.
    pub fn published_service_reach(&self) -> Option<&[PackageReviewNominalIdentity]> {
        self.declared_service_reach()
            .or_else(|| self.checked_service_reach().realized())
    }

    field_projection_accessors!(borrowed checked_service_reach: &PackageReviewCheckedServiceReach);
    field_projection_accessors!(borrowed service_reach_dependency: &PackagePolicyServiceReachDependency);
    field_projection_accessors!(sliced unresolved_installation_reaches: &[PackageReviewInstallationReach]);

    pub fn declared_synchronous_invocations(
        &self,
    ) -> Option<&[PackageReviewSynchronousInvocation]> {
        self.declared_synchronous_invocations.as_deref()
    }

    field_projection_accessors!(sliced realized_synchronous_invocations: &[PackageReviewSynchronousInvocation]);
    field_projection_accessors!(sliced capability_flows: &[PackagePolicyCapabilityFlow]);
    field_projection_accessors!(sliced reachable_capability_flows: &[PackagePolicyCapabilityFlow]);
    field_projection_accessors!(copied checked_may_suspend: bool);
    field_projection_accessors!(copied declared_may_suspend: Option<bool>);
    field_projection_accessors!(copied declared_may_block: Option<bool>);
    field_projection_accessors!(optional declared_termination: Option<&PackagePolicyTermination>);
    field_projection_accessors!(copied checked_may_block: bool);
    field_projection_accessors!(borrowed checked_termination: &PackagePolicyTermination);
    field_projection_accessors!(borrowed checked_crash: &PackagePolicyCrash);
    field_projection_accessors!(borrowed mutation: &PackagePolicyMutation);
}

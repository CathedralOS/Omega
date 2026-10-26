use crate::record::PackagePolicyCallable;
use crate::record::PackagePolicyCallableConformance;
use crate::record::PackagePolicyCallableRole;
use crate::record::PackagePolicyCapabilityFlow;
use crate::record::PackagePolicyCrash;
use crate::record::PackagePolicyMutation;
use crate::record::PackagePolicyServiceReachDependency;
use crate::record::PackagePolicyTermination;
use crate::record::PackageReviewCallableContract;
use crate::record::PackageReviewCallableSupply;
use crate::record::PackageReviewCheckedServiceReach;
use crate::record::PackageReviewInstallationReach;
use crate::record::PackageReviewNominalIdentity;
use crate::record::PackageReviewOperatorRealization;
use crate::record::PackageReviewSynchronousInvocation;

impl PackagePolicyCallable {
    crate::record::accessors::record_accessors!(
        spelling,
        lifetime_parameter_count,
        type_parameters,
        conformance_bounds,
        parameters,
        return_type
    );
    pub const fn target_context(&self) -> &crate::record::PackagePolicyTargetContext {
        &self.target_context
    }

    pub const fn role(&self) -> PackagePolicyCallableRole {
        self.role
    }

    pub fn identity(&self) -> &PackageReviewNominalIdentity {
        &self.identity
    }

    pub const fn supply(&self) -> PackageReviewCallableSupply {
        self.supply
    }

    pub fn conformances(&self) -> &[PackagePolicyCallableConformance] {
        &self.conformances
    }

    pub fn operator_realizations(&self) -> &[PackageReviewOperatorRealization] {
        &self.operator_realizations
    }

    pub fn contracts(&self) -> &[PackageReviewCallableContract] {
        &self.contracts
    }

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

    pub const fn checked_service_reach(&self) -> &PackageReviewCheckedServiceReach {
        &self.checked_service_reach
    }

    pub const fn service_reach_dependency(&self) -> &PackagePolicyServiceReachDependency {
        &self.service_reach_dependency
    }

    pub fn unresolved_installation_reaches(&self) -> &[PackageReviewInstallationReach] {
        &self.unresolved_installation_reaches
    }

    pub fn declared_synchronous_invocations(
        &self,
    ) -> Option<&[PackageReviewSynchronousInvocation]> {
        self.declared_synchronous_invocations.as_deref()
    }

    pub fn realized_synchronous_invocations(&self) -> &[PackageReviewSynchronousInvocation] {
        &self.realized_synchronous_invocations
    }

    pub fn capability_flows(&self) -> &[PackagePolicyCapabilityFlow] {
        &self.capability_flows
    }

    pub fn reachable_capability_flows(&self) -> &[PackagePolicyCapabilityFlow] {
        &self.reachable_capability_flows
    }

    pub const fn checked_may_suspend(&self) -> bool {
        self.checked_may_suspend
    }

    pub const fn declared_may_suspend(&self) -> Option<bool> {
        self.declared_may_suspend
    }

    pub const fn declared_may_block(&self) -> Option<bool> {
        self.declared_may_block
    }

    pub const fn declared_termination(&self) -> Option<&PackagePolicyTermination> {
        self.declared_termination.as_ref()
    }

    pub const fn checked_may_block(&self) -> bool {
        self.checked_may_block
    }

    pub const fn checked_termination(&self) -> &PackagePolicyTermination {
        &self.checked_termination
    }

    pub const fn checked_crash(&self) -> &PackagePolicyCrash {
        &self.checked_crash
    }

    pub const fn mutation(&self) -> &PackagePolicyMutation {
        &self.mutation
    }
}

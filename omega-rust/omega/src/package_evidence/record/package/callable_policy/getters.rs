use crate::package_evidence::record::PackagePolicyCallable;
use crate::package_evidence::record::PackagePolicyCallableConformance;
use crate::package_evidence::record::PackagePolicyCallableRole;
use crate::package_evidence::record::PackagePolicyCapabilityFlow;
use crate::package_evidence::record::PackagePolicyCrash;
use crate::package_evidence::record::PackagePolicyMutation;
use crate::package_evidence::record::PackagePolicyServiceReachDependency;
use crate::package_evidence::record::PackagePolicyTermination;
use crate::package_evidence::record::PackagePolicyTypeParameter;
use crate::package_evidence::record::PackageReviewCallableContract;
use crate::package_evidence::record::PackageReviewCallableParameter;
use crate::package_evidence::record::PackageReviewCallableSupply;
use crate::package_evidence::record::PackageReviewCheckedServiceReach;
use crate::package_evidence::record::PackageReviewConformanceBound;
use crate::package_evidence::record::PackageReviewInstallationReach;
use crate::package_evidence::record::PackageReviewNominalIdentity;
use crate::package_evidence::record::PackageReviewOperatorRealization;
use crate::package_evidence::record::PackageReviewSynchronousInvocation;
use crate::package_evidence::record::PackageReviewTypeIdentity;

impl PackagePolicyCallable {
    pub const fn role(&self) -> PackagePolicyCallableRole {
        self.role
    }

    pub fn identity(&self) -> &PackageReviewNominalIdentity {
        &self.identity
    }

    pub const fn supply(&self) -> PackageReviewCallableSupply {
        self.supply
    }

    pub const fn spelling(&self) -> Option<language_core::OperatorSpelling> {
        self.spelling
    }

    pub const fn lifetime_parameter_count(&self) -> usize {
        self.lifetime_parameter_count
    }

    pub fn type_parameters(&self) -> &[PackagePolicyTypeParameter] {
        &self.type_parameters
    }

    pub fn conformance_bounds(&self) -> &[PackageReviewConformanceBound] {
        &self.conformance_bounds
    }

    pub fn parameters(&self) -> &[PackageReviewCallableParameter] {
        &self.parameters
    }

    pub const fn return_type(&self) -> Option<&PackageReviewTypeIdentity> {
        self.return_type.as_ref()
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

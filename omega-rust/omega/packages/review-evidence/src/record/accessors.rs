//! Accessors restated by every opaque-representation selection record: the
//! carrying opaque identity, its conformance application, and the review
//! dispositions. Declared once so each record impl names only its own type.

/// The seven accessors every opaque-representation selection record carries:
/// identity fields plus the shared review dispositions.
macro_rules! opaque_representation_selection_accessors {
    () => {
        pub const fn opaque(&self) -> &crate::record::PackageReviewNominalIdentity {
            &self.opaque
        }

        pub const fn carrier(&self) -> &crate::record::PackageReviewNominalIdentity {
            &self.carrier
        }

        pub const fn selection_owner(&self) -> crate::record::PackageReviewNominalOwner {
            self.selection_owner
        }

        pub const fn application(
            &self,
        ) -> &crate::record::PackagePolicyClosedConformanceApplication {
            &self.application
        }

        pub const fn origin(
            &self,
        ) -> crate::record::PackageReviewOpaqueRepresentationApplicationOrigin {
            self.origin
        }

        pub const fn lifecycle(
            &self,
        ) -> crate::record::PackageReviewOpaqueRepresentationLifecycleDisposition {
            self.lifecycle
        }

        pub const fn copy_disposition(
            &self,
        ) -> crate::record::PackageReviewOpaqueRepresentationCopyDisposition {
            self.copy_disposition
        }
    };
}
pub(crate) use opaque_representation_selection_accessors;

/// The callable-signature surface shared by trait-requirement and
/// machine-parameter signature shapes: identical `self` projections over
/// identically-named fields on both record types.
macro_rules! package_policy_callable_signature_accessors {
    () => {
        pub const fn return_type(&self) -> Option<&crate::record::PackageReviewTypeIdentity> {
            self.return_type.as_ref()
        }
        pub fn contracts(&self) -> &[crate::record::PackageReviewCallableContract] {
            &self.contracts
        }
        pub fn published_crash(&self) -> &[crate::record::PackagePolicyCrashRoute] {
            &self.published_crash
        }
        pub fn service_reach(&self) -> &[crate::record::PackageReviewNominalIdentity] {
            &self.service_reach
        }
        pub const fn service_reach_is_installation_bound(&self) -> bool {
            self.service_reach_is_installation_bound
        }
        pub fn synchronous_invocations(
            &self,
        ) -> &[crate::record::PackageReviewSynchronousInvocation] {
            &self.synchronous_invocations
        }
        pub const fn suspends(&self) -> bool {
            self.suspends
        }
        pub const fn blocks(&self) -> bool {
            self.blocks
        }
        pub const fn termination(&self) -> &crate::record::PackagePolicyTermination {
            &self.termination
        }
    };
}
pub(crate) use package_policy_callable_signature_accessors;

/// The name/const/mutable parameter surface shared by the callable,
/// trait-requirement, and machine-parameter signature records: identical
/// `self` projections over identically-named fields.
macro_rules! signature_parameter_accessors {
    () => {
        pub fn name(&self) -> &str {
            &self.name
        }

        pub const fn type_identity(&self) -> &crate::record::PackageReviewTypeIdentity {
            &self.type_identity
        }

        pub const fn is_const(&self) -> bool {
            self.is_const
        }

        pub const fn is_mutable(&self) -> bool {
            self.is_mutable
        }
    };
}
pub(crate) use signature_parameter_accessors;

/// The single-field accessor surfaces the review and policy record impls
/// restate: identical `self` projections over identically-named fields,
/// declared once so each record impl names only the accessors it carries.
macro_rules! record_accessors {
    ($($name:ident),+ $(,)?) => {
        $( $crate::record::accessors::record_accessors!(@emit $name); )+
    };
    (@emit identity) => {
        pub const fn identity(&self) -> &crate::record::PackageReviewNominalIdentity {
            &self.identity
        }
    };
    (@emit package) => {
        pub const fn package(&self) -> semantic_vocabulary::PackageKeyIdentity {
            self.package
        }
    };
    (@emit target) => {
        pub const fn target(&self) -> target::TargetProfile {
            self.target
        }
    };
    (@emit lifetime_parameter_count) => {
        pub const fn lifetime_parameter_count(&self) -> usize {
            self.lifetime_parameter_count
        }
    };
    (@emit type_parameters) => {
        pub fn type_parameters(&self) -> &[crate::record::PackagePolicyTypeParameter] {
            &self.type_parameters
        }
    };
    (@emit static_parameters) => {
        pub fn static_parameters(&self) -> &[crate::record::PackagePolicyTypeParameter] {
            &self.static_parameters
        }
    };
    (@emit parameters) => {
        pub fn parameters(&self) -> &[crate::record::PackageReviewSignatureParameter] {
            &self.parameters
        }
    };
    (@emit conformance_bounds) => {
        pub fn conformance_bounds(&self) -> &[crate::record::PackageReviewConformanceBound] {
            &self.conformance_bounds
        }
    };
    (@emit return_type) => {
        pub const fn return_type(&self) -> Option<&crate::record::PackageReviewTypeIdentity> {
            self.return_type.as_ref()
        }
    };
    (@emit spelling) => {
        pub const fn spelling(&self) -> Option<language_core::OperatorSpelling> {
            self.spelling
        }
    };
    (@emit requirement) => {
        pub fn requirement(&self) -> &crate::record::PackageReviewNominalIdentity {
            &self.requirement
        }
    };
}
pub(crate) use record_accessors;

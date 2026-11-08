// Shared field-projection macros for package review and policy records.
//
// Opaque-representation selections, callable signatures, and parameter records
// expose identical read-only projections over identically named fields. We
// declare those projections once here so each record type names only the fields
// it exposes without restating boilerplate getter bodies.

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

macro_rules! field_projection_accessors {
    (valued $($name:ident : $type:ty),+ $(,)?) => {
        $( pub const fn $name(self) -> $type { self.$name } )+
    };
    (copied $($name:ident : $type:ty),+ $(,)?) => {
        $( pub const fn $name(&self) -> $type { self.$name } )+
    };
    (borrowed $($name:ident : $type:ty),+ $(,)?) => {
        $( pub const fn $name(&self) -> $type { &self.$name } )+
    };
    (sliced $($name:ident : $type:ty),+ $(,)?) => {
        $( pub fn $name(&self) -> $type { &self.$name } )+
    };
    (optional $($name:ident : $type:ty),+ $(,)?) => {
        $( pub const fn $name(&self) -> $type { self.$name.as_ref() } )+
    };
}
pub(crate) use field_projection_accessors;

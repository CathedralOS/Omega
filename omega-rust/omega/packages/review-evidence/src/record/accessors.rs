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

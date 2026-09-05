use super::{Encoder, PackageReviewEncodingError};
use crate::encoding::encode::membership::{Observer, PackagePolicyMembershipError};

impl<'identity> Encoder<'identity> {
    pub(in crate::encoding::encode) fn policy_membership(
        observer: &'identity mut dyn Observer,
    ) -> Self {
        Self {
            membership: Some(observer),
            ..Self::policy_bounded(4 * 1024 * 1024)
        }
    }

    pub(in crate::encoding) fn membership_error(&self) -> Option<PackagePolicyMembershipError> {
        self.membership_error
    }

    pub(super) fn record_membership_result(
        &mut self,
        result: Result<(), PackagePolicyMembershipError>,
    ) {
        if self.membership_error.is_none() {
            self.membership_error = result.err();
        }
    }

    pub(crate) fn observe_type_identity(
        &mut self,
        identity: &str,
    ) -> Result<(), PackageReviewEncodingError> {
        if let Some(observer) = &mut self.membership {
            let result = observer.type_identity(identity);
            self.record_membership_result(result);
        }
        self.check()
    }

    pub(crate) fn observe_nominal_path(
        &mut self,
        path: &str,
    ) -> Result<(), PackageReviewEncodingError> {
        if let Some(observer) = &mut self.membership {
            let result = observer.nominal_path(path);
            self.record_membership_result(result);
        }
        self.check()
    }
}

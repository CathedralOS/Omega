//! Canonical verification uses only the scratch already charged by recovery.

use super::{Encoder, PackagePolicyBaseline, PackageReviewEncodingError, framed_policy};

impl PackagePolicyBaseline {
    pub(in crate::encoding) fn canonical_bytes_for_recovery(
        &self,
        expected_length: usize,
    ) -> Result<Vec<u8>, PackageReviewEncodingError> {
        self.validate_canonical_structure()
            .map_err(PackageReviewEncodingError::new)?;
        let mut encoder = Encoder::policy_preallocated(expected_length)?;
        framed_policy(&mut encoder, self)?;
        let bytes = encoder.finish()?;
        if bytes.len() != expected_length {
            return Err(PackageReviewEncodingError::new(
                "package policy verification differs from its expected length",
            ));
        }
        Ok(bytes)
    }
}

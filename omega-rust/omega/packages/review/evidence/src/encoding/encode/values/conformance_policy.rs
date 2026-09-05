//! Canonical semantic conformance applications, without compiler receipts.

use super::identity::encode_nominal;
use crate::encoding::encode::encoder::Encoder;
use crate::encoding::{
    CONFORMANCE_POLICY_MAGIC, PACKAGE_CONFORMANCE_POLICY_VERSION, PackageReviewEncodingError,
};
use crate::record::{
    PackagePolicyClosedConformanceApplication, PackagePolicyConformanceConstArgument,
    PackageReviewTypeIdentity,
};

impl PackagePolicyClosedConformanceApplication {
    /// Complete inert application bytes, not a proof or selection receipt.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, PackageReviewEncodingError> {
        let mut encoder = Encoder::policy_bounded(4 * 1024 * 1024);
        encoder.fixed_bytes(CONFORMANCE_POLICY_MAGIC);
        encoder.u16(PACKAGE_CONFORMANCE_POLICY_VERSION);
        encode_nominal(&mut encoder, &self.declaration)?;
        encoder.sequence(&self.lifetime_arguments, ordinal)?;
        encoder.sequence(&self.type_arguments, type_identity)?;
        encoder.sequence(&self.const_arguments, const_argument)?;
        encoder.sequence(&self.machine_arguments, encode_nominal)?;
        encoder.option(self.subject.as_ref(), type_identity)?;
        encode_nominal(&mut encoder, &self.trait_identity)?;
        encoder.sequence(&self.trait_lifetime_arguments, ordinal)?;
        encoder.sequence(&self.trait_arguments, type_identity)?;
        encoder.sequence(&self.rows, |encoder, row| {
            encode_nominal(encoder, &row.declaring_trait)?;
            encode_nominal(encoder, &row.requirement)?;
            encode_nominal(encoder, &row.realization_machine)?;
            encode_nominal(encoder, &row.realization_state)
        })?;
        encoder.finish()
    }
}

fn ordinal(encoder: &mut Encoder, ordinal: &u32) -> Result<(), PackageReviewEncodingError> {
    encoder.u32(*ordinal);
    Ok(())
}

fn type_identity(
    encoder: &mut Encoder,
    identity: &PackageReviewTypeIdentity,
) -> Result<(), PackageReviewEncodingError> {
    encoder.string(identity.canonical())
}

fn const_argument(
    encoder: &mut Encoder,
    argument: &PackagePolicyConformanceConstArgument,
) -> Result<(), PackageReviewEncodingError> {
    match argument {
        PackagePolicyConformanceConstArgument::Evaluated {
            parameter_carrier,
            declared_carrier,
            canonical_value_encoding,
        } => {
            encoder.byte(0);
            type_identity(encoder, parameter_carrier)?;
            type_identity(encoder, declared_carrier)?;
            encoder.string(canonical_value_encoding)
        }
        PackagePolicyConformanceConstArgument::CallerBinder {
            parameter_carrier,
            binder,
            binder_carrier,
        } => {
            encoder.byte(1);
            type_identity(encoder, parameter_carrier)?;
            encode_nominal(encoder, binder)?;
            type_identity(encoder, binder_carrier)
        }
    }
}

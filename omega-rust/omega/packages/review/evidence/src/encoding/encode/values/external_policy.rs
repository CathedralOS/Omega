//! Canonical receipt-free external-supply policy, independent of review rows.

mod signatures;
use super::identity::encode_nominal;
use crate::encoding::encode::encoder::Encoder;
use crate::encoding::{
    EXTERNAL_SUPPLY_POLICY_MAGIC, PACKAGE_EXTERNAL_SUPPLY_POLICY_VERSION,
    PackageReviewEncodingError,
};
use crate::record::{
    PackagePolicyEvaluatedBindingProducer, PackagePolicyExternalBinding,
    PackagePolicyExternalExecutableSupply, PackageReviewForeignLocator,
};

const MAXIMUM_BYTES: usize = 4 * 1024 * 1024;

impl PackagePolicyExternalExecutableSupply {
    /// Bounded component-schema bytes for policy comparison. They contain no
    /// acceptance decision, evaluation receipt or executable authority.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, PackageReviewEncodingError> {
        encode(self, MAXIMUM_BYTES)
    }
}

fn encode(
    supply: &PackagePolicyExternalExecutableSupply,
    maximum_bytes: usize,
) -> Result<Vec<u8>, PackageReviewEncodingError> {
    let mut encoder = Encoder::policy_bounded(maximum_bytes);
    encoder.fixed_bytes(EXTERNAL_SUPPLY_POLICY_MAGIC);
    encoder.u16(PACKAGE_EXTERNAL_SUPPLY_POLICY_VERSION);
    policy(&mut encoder, supply)?;
    encoder.finish()
}

pub(in crate::encoding::encode) fn policy(
    encoder: &mut Encoder,
    supply: &PackagePolicyExternalExecutableSupply,
) -> Result<(), PackageReviewEncodingError> {
    supply
        .validate_canonical_structure()
        .map_err(PackageReviewEncodingError::new)?;
    encode_nominal(encoder, &supply.callable)?;
    signatures::signature(encoder, &supply.signature)?;
    signatures::requirement(encoder, &supply.requirement)?;
    match &supply.binding {
        PackagePolicyExternalBinding::Import { library, symbol } => {
            encoder.byte(0);
            encoder.string(library)?;
            encoder.string(symbol)?;
        }
        PackagePolicyExternalBinding::Syscall { number } => {
            encoder.byte(1);
            encoder.i64(*number);
        }
        PackagePolicyExternalBinding::CompilerIntrinsic => encoder.byte(2),
        PackagePolicyExternalBinding::VtableSlot { index } => {
            encoder.byte(3);
            encoder.i64(*index);
        }
        PackagePolicyExternalBinding::VtableField { field } => {
            encoder.byte(4);
            encoder.string(field)?;
        }
        PackagePolicyExternalBinding::TableFunction { field } => {
            encoder.byte(5);
            encoder.string(field)?;
        }
        PackagePolicyExternalBinding::NormalizedImport {
            target,
            locator,
            producer,
        } => {
            encoder.byte(6);
            encode_target(encoder, target)?;
            encode_locator(encoder, locator)?;
            encode_producer(encoder, producer)?;
        }
        PackagePolicyExternalBinding::NormalizedSyscall {
            target,
            number,
            producer,
        } => {
            encoder.byte(7);
            encode_target(encoder, target)?;
            encoder.i64(*number);
            encode_producer(encoder, producer)?;
        }
    }
    Ok(())
}

fn encode_target(encoder: &mut Encoder, target: &str) -> Result<(), PackageReviewEncodingError> {
    if !omega_target::TargetProfile::ALL
        .iter()
        .any(|profile| profile.identity().as_str() == target)
    {
        return Err(PackageReviewEncodingError::new(
            "external-supply policy requires an exact versioned target identity",
        ));
    }
    encoder.string(target)
}

pub(crate) fn encode_producer(
    encoder: &mut Encoder,
    producer: &PackagePolicyEvaluatedBindingProducer,
) -> Result<(), PackageReviewEncodingError> {
    encode_nominal(encoder, &producer.declaration)?;
    encoder.optional_package_identity(producer.package);
    encoder.string(&producer.callable_identity)
}

pub(crate) fn encode_locator(
    encoder: &mut Encoder,
    locator: &PackageReviewForeignLocator,
) -> Result<(), PackageReviewEncodingError> {
    match locator {
        PackageReviewForeignLocator::PeByName { library, export } => {
            encoder.byte(0);
            encoder.bytes(library)?;
            encoder.bytes(export)?;
        }
        PackageReviewForeignLocator::PeByOrdinal { library, ordinal } => {
            encoder.byte(1);
            encoder.bytes(library)?;
            encoder.u16(*ordinal);
        }
        PackageReviewForeignLocator::ElfVersioned {
            object,
            symbol,
            version,
        } => {
            encoder.byte(2);
            encoder.bytes(object)?;
            encoder.bytes(symbol)?;
            encoder.bytes(version)?;
        }
        PackageReviewForeignLocator::MachODylibSymbol {
            install_name,
            symbol,
        } => {
            encoder.byte(3);
            encoder.bytes(install_name)?;
            encoder.bytes(symbol)?;
        }
    }
    Ok(())
}

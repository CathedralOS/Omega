//! Canonical receipt-free external-supply policy, independent of review rows.

mod locator;
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
pub(crate) use locator::encode_locator;

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
    validated_value(encoder, supply)
}

/// An enclosing, privately constructed baseline already validated this value.
pub(in crate::encoding::encode) fn validated_value(
    encoder: &mut Encoder,
    supply: &PackagePolicyExternalExecutableSupply,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field("callable", |encoder| {
        encode_nominal(encoder, &supply.callable)
    })?;
    encoder.field("signature", |encoder| {
        signatures::signature(encoder, &supply.signature)
    })?;
    encoder.field("requirement", |encoder| {
        signatures::requirement(encoder, &supply.requirement)
    })?;
    encoder.field("binding", |encoder| {
        match &supply.binding {
            PackagePolicyExternalBinding::Import { library, symbol } => {
                encoder.tag("import", 0);
                encoder.field("library", |encoder| encoder.string(library))?;
                encoder.field("symbol", |encoder| encoder.string(symbol))?;
            }
            PackagePolicyExternalBinding::Syscall { number } => {
                encoder.tag("syscall", 1);
                encoder.field("number", |encoder| {
                    encoder.i64(*number);
                    Ok(())
                })?;
            }
            PackagePolicyExternalBinding::CompilerIntrinsic => encoder.tag("compiler_intrinsic", 2),
            PackagePolicyExternalBinding::VtableSlot { index } => {
                encoder.tag("vtable_slot", 3);
                encoder.field("index", |encoder| {
                    encoder.i64(*index);
                    Ok(())
                })?;
            }
            PackagePolicyExternalBinding::VtableField { field } => {
                encoder.tag("vtable_field", 4);
                encoder.field("field", |encoder| encoder.string(field))?;
            }
            PackagePolicyExternalBinding::TableFunction { field } => {
                encoder.tag("table_function", 5);
                encoder.field("field", |encoder| encoder.string(field))?;
            }
            PackagePolicyExternalBinding::NormalizedImport {
                target,
                locator,
                producer,
            } => {
                encoder.tag("normalized_import", 6);
                encoder.field("target", |encoder| encode_target(encoder, target))?;
                encoder.field("locator", |encoder| encode_locator(encoder, locator))?;
                encoder.field("producer", |encoder| encode_producer(encoder, producer))?;
            }
            PackagePolicyExternalBinding::NormalizedSyscall {
                target,
                number,
                producer,
            } => {
                encoder.tag("normalized_syscall", 7);
                encoder.field("target", |encoder| encode_target(encoder, target))?;
                encoder.field("number", |encoder| {
                    encoder.i64(*number);
                    Ok(())
                })?;
                encoder.field("producer", |encoder| encode_producer(encoder, producer))?;
            }
        }
        Ok(())
    })
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
    encoder.field("declaration", |encoder| {
        encode_nominal(encoder, &producer.declaration)
    })?;
    encoder.field("package", |encoder| {
        encoder.optional_package_identity(producer.package);
        Ok(())
    })?;
    encoder.field("callable_identity", |encoder| {
        encoder.string(&producer.callable_identity)
    })
}

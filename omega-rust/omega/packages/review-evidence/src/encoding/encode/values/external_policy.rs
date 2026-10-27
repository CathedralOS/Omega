//! Canonical receipt-free external-supply policy, independent of review rows.

mod locator;
mod signatures;
use super::identity::encode_nominal;
use crate::encoding::encode::encoder::{Encoder, encode_canonical_component};
use crate::encoding::{
    EXTERNAL_SUPPLY_POLICY_MAGIC, PACKAGE_EXTERNAL_SUPPLY_POLICY_VERSION,
    PackageReviewEncodingError,
};
use crate::record::{
    PackagePolicyEvaluatedBindingProducer, PackagePolicyExternalBinding,
    PackagePolicyExternalExecutableSupply,
};
pub(crate) use locator::encode_locator;

impl PackagePolicyExternalExecutableSupply {
    encode_canonical_component!(
        EXTERNAL_SUPPLY_POLICY_MAGIC,
        PACKAGE_EXTERNAL_SUPPLY_POLICY_VERSION,
        policy,
    );
}

pub(in crate::encoding::encode) fn policy(
    encoder: &mut Encoder,
    supply: &PackagePolicyExternalExecutableSupply,
) -> Result<(), PackageReviewEncodingError> {
    validated_value(encoder, supply)
}

/// An enclosing, privately constructed baseline already validated this value.
pub(in crate::encoding::encode) fn validated_value(
    encoder: &mut Encoder,
    supply: &PackagePolicyExternalExecutableSupply,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field_by("callable", &supply.callable, encode_nominal)?;
    encoder.field_by(
        "target_context",
        &supply.target_context,
        super::identity::encode_target_context,
    )?;
    encoder.field_by("signature", &supply.signature, signatures::signature)?;
    encoder.field_by("requirement", &supply.requirement, signatures::requirement)?;
    encoder.field("binding", |encoder| {
        // Tag 0 carried the retired string-backed import row; recovery
        // rejects it by name and no policy produces it again.
        match &supply.binding {
            PackagePolicyExternalBinding::Syscall { number } => {
                encoder.tag("syscall", 1);
                encoder.i64_field("number", *number)?;
            }
            PackagePolicyExternalBinding::CompilerIntrinsic => encoder.tag("compiler_intrinsic", 2),
            PackagePolicyExternalBinding::VtableSlot { index } => {
                encoder.tag("vtable_slot", 3);
                encoder.i64_field("index", *index)?;
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
                encoder.i64_field("number", *number)?;
                encoder.field("producer", |encoder| encode_producer(encoder, producer))?;
            }
        }
        Ok(())
    })
}

fn encode_target(encoder: &mut Encoder, target: &str) -> Result<(), PackageReviewEncodingError> {
    if !target::TargetProfile::ALL
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
    encoder.field_by("declaration", &producer.declaration, encode_nominal)?;
    encoder.optional_package_identity_field("package", producer.package)?;
    encoder.field("callable_identity", |encoder| {
        encoder.string(&producer.callable_identity)
    })
}

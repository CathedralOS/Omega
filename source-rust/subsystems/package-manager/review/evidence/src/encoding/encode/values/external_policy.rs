// Canonical receipt-free external-supply policy encoding, independent of review rows.
// Enclosing privately constructed baselines validate `validated_value` inputs before encoding.
// Tag `0` on external bindings carried the retired string-backed import row; recovery rejects it by name and no policy produces it again.

use super::declarations::encode_operator_coordinate;
use super::identity::encode_nominal;
use crate::encoding::encode::callable_policy::encode_callable_conformance;
use crate::encoding::encode::declarations::{encode_conformance_bound, encode_type_identity};
use crate::encoding::encode::encoder::{Encoder, encode_canonical_component};
use crate::encoding::encode::public_api::type_parameter;
use crate::encoding::{
    EXTERNAL_SUPPLY_POLICY_MAGIC, PACKAGE_EXTERNAL_SUPPLY_POLICY_VERSION,
    PackageReviewEncodingError,
};
use crate::record::{
    PackagePolicyEvaluatedBindingProducer, PackagePolicyExternalBinding,
    PackagePolicyExternalCallableSignature, PackagePolicyExternalExecutableSupply,
    PackagePolicyExternalRequirement, PackageReviewForeignLocator,
};

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
    encoder.field_by("signature", &supply.signature, signature)?;
    encoder.field_by("requirement", &supply.requirement, requirement)?;
    encoder.field("binding", |encoder| {
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
    encoder.string_field("callable_identity", &producer.callable_identity)
}

// Exact foreign locator vocabulary shared by policy provider bindings.

pub(crate) fn encode_locator(
    encoder: &mut Encoder,
    locator: &PackageReviewForeignLocator,
) -> Result<(), PackageReviewEncodingError> {
    match locator {
        PackageReviewForeignLocator::PeByName { library, export } => {
            encoder.tag("pe_by_name", 0);
            encoder.field("library", |encoder| encoder.bytes(library))?;
            encoder.field("export", |encoder| encoder.bytes(export))?;
        }
        PackageReviewForeignLocator::PeByOrdinal { library, ordinal } => {
            encoder.tag("pe_by_ordinal", 1);
            encoder.field("library", |encoder| encoder.bytes(library))?;
            encoder.u16_field("ordinal", *ordinal)?;
        }
        PackageReviewForeignLocator::ElfVersioned {
            object,
            symbol,
            version,
        } => {
            encoder.tag("elf_versioned", 2);
            encoder.field("object", |encoder| encoder.bytes(object))?;
            encoder.field("symbol", |encoder| encoder.bytes(symbol))?;
            encoder.field("version", |encoder| encoder.bytes(version))?;
        }
        PackageReviewForeignLocator::MachODylibSymbol {
            install_name,
            symbol,
        } => {
            encoder.tag("macho_dylib_symbol", 3);
            encoder.field("install_name", |encoder| encoder.bytes(install_name))?;
            encoder.field("symbol", |encoder| encoder.bytes(symbol))?;
        }
    }
    Ok(())
}

// Canonical encoding for external callable signatures and external trait/operator/top-level requirements.

fn signature(
    encoder: &mut Encoder,
    value: &PackagePolicyExternalCallableSignature,
) -> Result<(), PackageReviewEncodingError> {
    encoder.usize_field("lifetime_parameter_count", value.lifetime_parameter_count)?;
    encoder.field("static_parameters", |encoder| {
        encoder.sequence(&value.static_parameters, type_parameter)
    })?;
    encoder.field("conformance_bounds", |encoder| {
        encoder.sequence(&value.conformance_bounds, encode_conformance_bound)
    })?;
    encoder.field("parameters", |encoder| {
        encoder.sequence(&value.parameters, |encoder, parameter| {
            encoder.field_by("type", &parameter.type_identity, encode_type_identity)?;
            encoder.boolean_field("const", parameter.is_const)?;
            encoder.boolean_field("mutable", parameter.is_mutable)?;
            Ok(())
        })
    })?;
    encoder.option_field(
        "return_type",
        value.return_type.as_ref(),
        encode_type_identity,
    )
}

fn requirement(
    encoder: &mut Encoder,
    value: &PackagePolicyExternalRequirement,
) -> Result<(), PackageReviewEncodingError> {
    match value {
        PackagePolicyExternalRequirement::Trait(value) => {
            encoder.tag("trait", 0);
            encoder.field_by("conformance", value, encode_callable_conformance)
        }
        PackagePolicyExternalRequirement::Operator { coordinate, alias } => {
            encoder.tag("operator", 1);
            encoder.field_by("coordinate", coordinate, encode_operator_coordinate)?;
            encoder.field("alias", |encoder| {
                encoder.option(alias.as_ref(), |encoder, value| encoder.string(value))
            })
        }
        PackagePolicyExternalRequirement::TopLevelRequirement {
            identity,
            signature: value,
            alias,
        } => {
            encoder.tag("top_level_requirement", 2);
            encoder.field("identity", |encoder| encode_nominal(encoder, identity))?;
            encoder.field("signature", |encoder| signature(encoder, value))?;
            encoder.field("alias", |encoder| {
                encoder.option(alias.as_ref(), |encoder, value| encoder.string(value))
            })
        }
    }
}

use crate::encoding::PackageReviewEncodingError;
use crate::encoding::encode::values::expressions::Encoder;
use crate::encoding::encode::values::expressions::encode_type_identity;
use crate::encoding::encode::values::identity::encode_nominal;
use crate::record::PackageReviewContractStaticArgument;

pub(crate) fn encode_contract_static_argument(
    encoder: &mut Encoder,
    argument: &PackageReviewContractStaticArgument,
) -> Result<(), PackageReviewEncodingError> {
    encoder.nested(|encoder| encode_contract_static_argument_body(encoder, argument))
}

fn encode_contract_static_argument_body(
    encoder: &mut Encoder,
    argument: &PackageReviewContractStaticArgument,
) -> Result<(), PackageReviewEncodingError> {
    match argument {
        PackageReviewContractStaticArgument::Type(identity) => {
            encoder.tag("type", 0);
            encoder.field_by("identity", identity, encode_type_identity)?;
        }
        PackageReviewContractStaticArgument::GenericTypeBinder(position) => {
            encoder.tag("generic_type_binder", 5);
            encoder.u32_field("position", *position)?;
        }
        PackageReviewContractStaticArgument::GenericType {
            base,
            lifetime_arguments,
            arguments,
        } => {
            encoder.tag("generic_type", 1);
            encoder.field("base", |encoder| encode_type_identity(encoder, base))?;
            encoder.field("lifetime_arguments", |encoder| {
                encoder.sequence(lifetime_arguments, |encoder, argument| {
                    encoder.u32_field("argument", *argument)?;
                    Ok(())
                })
            })?;
            encoder.field("arguments", |encoder| {
                encoder.sequence(arguments, encode_contract_static_argument)
            })?;
        }
        PackageReviewContractStaticArgument::ConstInteger(value) => {
            encoder.tag("const_integer", 2);
            encoder.field("value", |encoder| encoder.string(value))?;
        }
        PackageReviewContractStaticArgument::ConstBoolean(value) => {
            encoder.tag("const_boolean", 7);
            encoder.boolean_field("value", *value)?;
        }
        PackageReviewContractStaticArgument::ConstStructured {
            declared_type,
            canonical_value_encoding,
        } => {
            encoder.tag("const_structured", 9);
            encoder.field_by("declared_type", declared_type, encode_type_identity)?;
            encoder.field("canonical_value_encoding", |encoder| {
                encoder.string(canonical_value_encoding)
            })?;
        }
        PackageReviewContractStaticArgument::GenericConstBinder(position) => {
            encoder.tag("generic_const_binder", 6);
            encoder.u32_field("position", *position)?;
        }
        PackageReviewContractStaticArgument::GenericMachineBinder(position) => {
            encoder.tag("generic_machine_binder", 3);
            encoder.u32_field("position", *position)?;
        }
        PackageReviewContractStaticArgument::ConcreteMachine(identity) => {
            encoder.tag("concrete_machine", 4);
            encoder.field("identity", |encoder| encode_nominal(encoder, identity))?;
        }
        PackageReviewContractStaticArgument::ConformanceApplication {
            declaration,
            arguments,
            trait_identity,
            trait_arguments,
        } => {
            encoder.tag("conformance_application", 8);
            encoder.field_by("declaration", declaration, encode_nominal)?;
            encoder.field("arguments", |encoder| {
                encoder.sequence(arguments, encode_contract_static_argument)
            })?;
            encoder.field_by("trait_identity", trait_identity, encode_nominal)?;
            encoder.field("trait_arguments", |encoder| {
                encoder.sequence(trait_arguments, encode_type_identity)
            })?;
        }
    }
    Ok(())
}

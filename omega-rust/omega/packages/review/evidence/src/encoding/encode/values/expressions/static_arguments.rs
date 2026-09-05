use super::*;

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
            encoder.field("identity", |encoder| {
                encode_type_identity(encoder, identity)
            })?;
        }
        PackageReviewContractStaticArgument::GenericTypeBinder(position) => {
            encoder.tag("generic_type_binder", 5);
            encoder.field("position", |encoder| {
                encoder.u32(*position);
                Ok(())
            })?;
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
                    encoder.field("argument", |encoder| {
                        encoder.u32(*argument);
                        Ok(())
                    })?;
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
            encoder.field("value", |encoder| {
                encoder.boolean(*value);
                Ok(())
            })?;
        }
        PackageReviewContractStaticArgument::ConstStructured {
            declared_type,
            canonical_value_encoding,
        } => {
            encoder.tag("const_structured", 9);
            encoder.field("declared_type", |encoder| {
                encode_type_identity(encoder, declared_type)
            })?;
            encoder.field("canonical_value_encoding", |encoder| {
                encoder.string(canonical_value_encoding)
            })?;
        }
        PackageReviewContractStaticArgument::GenericConstBinder(position) => {
            encoder.tag("generic_const_binder", 6);
            encoder.field("position", |encoder| {
                encoder.u32(*position);
                Ok(())
            })?;
        }
        PackageReviewContractStaticArgument::GenericMachineBinder(position) => {
            encoder.tag("generic_machine_binder", 3);
            encoder.field("position", |encoder| {
                encoder.u32(*position);
                Ok(())
            })?;
        }
        PackageReviewContractStaticArgument::ConcreteMachine(identity) => {
            encoder.tag("concrete_machine", 4);
            encoder.field("identity", |encoder| encode_nominal(encoder, identity))?;
        }
        PackageReviewContractStaticArgument::ConformanceApplication {
            declaration,
            arguments,
            subject,
            trait_identity,
            trait_arguments,
        } => {
            encoder.tag("conformance_application", 8);
            encoder.field("declaration", |encoder| {
                encode_nominal(encoder, declaration)
            })?;
            encoder.field("arguments", |encoder| {
                encoder.sequence(arguments, encode_contract_static_argument)
            })?;
            encoder.field("subject", |encoder| {
                encode_contract_static_argument(encoder, subject)
            })?;
            encoder.field("trait_identity", |encoder| {
                encode_nominal(encoder, trait_identity)
            })?;
            encoder.field("trait_arguments", |encoder| {
                encoder.sequence(trait_arguments, encode_type_identity)
            })?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encoded(argument: PackageReviewContractStaticArgument) -> Vec<u8> {
        let mut encoder = Encoder::bounded(16);
        encode_contract_static_argument(&mut encoder, &argument)
            .expect("encode contract static argument");
        encoder.finish().expect("bounded static-argument bytes")
    }

    #[test]
    fn boolean_static_arguments_have_a_distinct_closed_tag_and_value_byte() {
        assert_eq!(
            encoded(PackageReviewContractStaticArgument::ConstBoolean(false)),
            [7, 0]
        );
        assert_eq!(
            encoded(PackageReviewContractStaticArgument::ConstBoolean(true)),
            [7, 1]
        );
        assert_ne!(
            encoded(PackageReviewContractStaticArgument::ConstBoolean(true)),
            encoded(PackageReviewContractStaticArgument::ConstInteger(
                "1".to_owned()
            ))
        );
    }
}

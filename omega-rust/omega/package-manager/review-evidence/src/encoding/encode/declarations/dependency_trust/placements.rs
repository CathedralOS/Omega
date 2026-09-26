use super::encode_machine_register;
use crate::encoding::{PackageReviewEncodingError, encode::encoder::Encoder};
use crate::record::{
    PackageReviewBoundaryValueClass, PackageReviewBoundaryValueLocation,
    PackageReviewBoundaryValuePlacement, PackageReviewIndirectPointerLocation,
    PackageReviewSystemVEightbyteClass,
};

pub(crate) fn encode_value_placement(
    encoder: &mut Encoder,
    placement: &PackageReviewBoundaryValuePlacement,
) -> Result<(), PackageReviewEncodingError> {
    let shape = placement.shape();
    encoder.field("class", |encoder| {
        match shape.class() {
            PackageReviewBoundaryValueClass::Integer => encoder.tag("integer", 0),
            PackageReviewBoundaryValueClass::Float => encoder.tag("float", 1),
            PackageReviewBoundaryValueClass::BorrowedReference => {
                encoder.tag("borrowed_reference", 4)
            }
            PackageReviewBoundaryValueClass::HomogeneousFloatAggregate { members } => {
                encoder.tag("homogeneous_float_aggregate", 2);
                encoder.field("members", |encoder| {
                    encoder.byte(members);
                    Ok(())
                })?;
            }
            PackageReviewBoundaryValueClass::SystemVAggregate { first, second } => {
                encoder.tag("system_v_aggregate", 3);
                encoder.field("first", |encoder| {
                    encode_system_v_class(encoder, first);
                    Ok(())
                })?;
                encoder.field("second", |encoder| {
                    encode_system_v_class(encoder, second);
                    Ok(())
                })?;
            }
        };
        Ok(())
    })?;
    encoder.field("byte_size", |encoder| {
        encoder.u16(shape.byte_size());
        Ok(())
    })?;
    encoder.field("alignment", |encoder| {
        encoder.u16(shape.alignment());
        Ok(())
    })?;
    encoder.field("locations", |encoder| {
        encoder.sequence(placement.locations(), encode_value_location)
    })
}

fn encode_value_location(
    encoder: &mut Encoder,
    location: &PackageReviewBoundaryValueLocation,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field("location", |encoder| {
        match *location {
            PackageReviewBoundaryValueLocation::Register {
                register,
                value_byte_offset,
                byte_size,
            } => {
                encoder.tag("register", 0);
                encoder.field("register", |encoder| {
                    encode_machine_register(encoder, register);
                    Ok(())
                })?;
                encoder.field("value_byte_offset", |encoder| {
                    encoder.u16(value_byte_offset);
                    Ok(())
                })?;
                encoder.field("byte_size", |encoder| {
                    encoder.u16(byte_size);
                    Ok(())
                })?;
            }
            PackageReviewBoundaryValueLocation::Stack {
                stack_byte_offset,
                value_byte_offset,
                byte_size,
                alignment,
            } => {
                encoder.tag("stack", 1);
                encoder.field("stack_byte_offset", |encoder| {
                    encoder.u32(stack_byte_offset);
                    Ok(())
                })?;
                encoder.field("value_byte_offset", |encoder| {
                    encoder.u16(value_byte_offset);
                    Ok(())
                })?;
                encoder.field("byte_size", |encoder| {
                    encoder.u16(byte_size);
                    Ok(())
                })?;
                encoder.field("alignment", |encoder| {
                    encoder.u16(alignment);
                    Ok(())
                })?;
            }
            PackageReviewBoundaryValueLocation::Indirect {
                pointer,
                copy_stack_byte_offset,
                byte_size,
                alignment,
            } => {
                encoder.tag("indirect", 2);
                encoder.field("pointer", |encoder| {
                    match pointer {
                        PackageReviewIndirectPointerLocation::Register(register) => {
                            encoder.tag("register", 0);
                            encoder.field("register", |encoder| {
                                encode_machine_register(encoder, register);
                                Ok(())
                            })?;
                        }
                        PackageReviewIndirectPointerLocation::Stack {
                            stack_byte_offset,
                            alignment,
                        } => {
                            encoder.tag("stack", 1);
                            encoder.field("stack_byte_offset", |encoder| {
                                encoder.u32(stack_byte_offset);
                                Ok(())
                            })?;
                            encoder.field("alignment", |encoder| {
                                encoder.u16(alignment);
                                Ok(())
                            })?;
                        }
                    };
                    Ok(())
                })?;
                encoder.field("copy_stack_byte_offset", |encoder| {
                    encoder.option(copy_stack_byte_offset.as_ref(), |encoder, offset| {
                        encoder.field("offset", |encoder| {
                            encoder.u32(*offset);
                            Ok(())
                        })?;
                        Ok(())
                    })
                })?;
                encoder.field("byte_size", |encoder| {
                    encoder.u16(byte_size);
                    Ok(())
                })?;
                encoder.field("alignment", |encoder| {
                    encoder.u16(alignment);
                    Ok(())
                })?;
            }
        };
        Ok(())
    })?;
    Ok(())
}

fn encode_system_v_class(encoder: &mut Encoder, class: PackageReviewSystemVEightbyteClass) {
    match class {
        PackageReviewSystemVEightbyteClass::Integer => encoder.tag("integer", 0),
        PackageReviewSystemVEightbyteClass::Sse => encoder.tag("sse", 1),
    }
}

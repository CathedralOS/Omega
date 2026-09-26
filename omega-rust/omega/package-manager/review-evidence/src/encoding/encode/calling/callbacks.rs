use super::{Encoder, PackageReviewEncodingError, type_identity};
use crate::encoding::encode::values::conformance_policy::encode_application;
use crate::encoding::encode::values::identity::encode_nominal;
use crate::record::{
    PackagePolicyCallbackDestination, PackagePolicyCallbackLayoutApplication,
    PackagePolicyCallbacks,
};

pub(super) fn encode(
    encoder: &mut Encoder,
    callbacks: &PackagePolicyCallbacks,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field("binders", |encoder| {
        encoder.sequence(&callbacks.binders, |encoder, binder| {
            encoder.field("parameter", |encoder| {
                encode_nominal(encoder, &binder.parameter)
            })?;
            encoder.field("static_parameter_ordinal", |encoder| {
                encoder.u32(binder.static_parameter_ordinal);
                Ok(())
            })?;
            encoder.field("static_machine_ordinal", |encoder| {
                encoder.u32(binder.static_machine_ordinal);
                Ok(())
            })?;
            encoder.field("requirement", |encoder| {
                encode_nominal(encoder, &binder.requirement)
            })
        })
    })?;
    encoder.field("demands", |encoder| {
        encoder.sequence(&callbacks.demands, |encoder, demand| {
            encoder.field("destination", |encoder| {
                destination(encoder, &demand.destination);
                Ok(())
            })?;
            encoder.field("requirement", |encoder| {
                encode_nominal(encoder, &demand.requirement)
            })
        })
    })?;
    encoder.field("materializations", |encoder| {
        encoder.sequence(&callbacks.materializations, |encoder, materialization| {
            encoder.field("binder_index", |encoder| {
                encoder.u32(materialization.binder_index);
                Ok(())
            })?;
            encoder.field("destination", |encoder| {
                destination(encoder, &materialization.destination);
                Ok(())
            })?;
            Ok(())
        })
    })?;
    encoder.field("layouts", |encoder| {
        encoder.sequence(&callbacks.layouts, |encoder, layout| {
            encoder.field("formal_ordinal", |encoder| {
                encoder.u32(layout.formal_ordinal);
                Ok(())
            })?;
            encoder.field("native_ordinal", |encoder| {
                encoder.u32(layout.native_ordinal);
                Ok(())
            })?;
            encoder.field("root_layout", |encoder| {
                layout_application(encoder, &layout.root_layout)
            })?;
            encoder.field("inline_field", |encoder| {
                encoder.option(layout.inline_field.as_ref(), |encoder, field| {
                    encoder.field("field", |encoder| encode_nominal(encoder, &field.field))?;
                    encoder.field("offset", |encoder| {
                        encoder.u64(field.offset);
                        Ok(())
                    })?;
                    encoder.field("extent", |encoder| {
                        encoder.u64(field.extent);
                        Ok(())
                    })?;
                    encoder.field("alignment", |encoder| {
                        encoder.u64(field.alignment);
                        Ok(())
                    })?;
                    encoder.field("child_layout", |encoder| {
                        layout_application(encoder, &field.child_layout)
                    })
                })
            })?;
            encoder.field("terminal_slot", |encoder| {
                encode_application(encoder, &layout.terminal_slot)
            })?;
            encoder.field("terminal_offset", |encoder| {
                encoder.u64(layout.terminal_offset);
                Ok(())
            })?;
            encoder.field("terminal_byte_size", |encoder| {
                encoder.u64(layout.terminal_byte_size);
                Ok(())
            })?;
            encoder.field("terminal_alignment", |encoder| {
                encoder.u64(layout.terminal_alignment);
                Ok(())
            })?;
            encoder.field("composed_offset", |encoder| {
                encoder.u64(layout.composed_offset);
                Ok(())
            })?;
            Ok(())
        })
    })
}

fn destination(encoder: &mut Encoder, destination: &PackagePolicyCallbackDestination) {
    match destination {
        PackagePolicyCallbackDestination::Parameter { native_ordinal } => {
            encoder.tag("parameter", 0);
            let _ = encoder.field("native_ordinal", |encoder| {
                encoder.u32(*native_ordinal);
                Ok(())
            });
        }
        PackagePolicyCallbackDestination::Field {
            native_ordinal,
            layout_index,
        } => {
            encoder.tag("field", 1);
            let _ = encoder.field("native_ordinal", |encoder| {
                encoder.u32(*native_ordinal);
                Ok(())
            });
            let _ = encoder.field("layout_index", |encoder| {
                encoder.u32(*layout_index);
                Ok(())
            });
        }
    }
}

fn layout_application(
    encoder: &mut Encoder,
    layout: &PackagePolicyCallbackLayoutApplication,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field("policy", |encoder| encode_nominal(encoder, &layout.policy))?;
    encoder.field("schema", |encoder| type_identity(encoder, &layout.schema))?;
    encoder.field("byte_size", |encoder| {
        encoder.u64(layout.byte_size);
        Ok(())
    })?;
    encoder.field("alignment", |encoder| {
        encoder.u64(layout.alignment);
        Ok(())
    })?;
    Ok(())
}

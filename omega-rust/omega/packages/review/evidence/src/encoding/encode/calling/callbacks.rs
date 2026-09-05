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
    encoder.sequence(&callbacks.binders, |encoder, binder| {
        encode_nominal(encoder, &binder.parameter)?;
        encoder.u32(binder.static_parameter_ordinal);
        encoder.u32(binder.static_machine_ordinal);
        encode_nominal(encoder, &binder.requirement)
    })?;
    encoder.sequence(&callbacks.demands, |encoder, demand| {
        destination(encoder, &demand.destination);
        encode_nominal(encoder, &demand.requirement)
    })?;
    encoder.sequence(&callbacks.materializations, |encoder, materialization| {
        encoder.u32(materialization.binder_index);
        destination(encoder, &materialization.destination);
        Ok(())
    })?;
    encoder.sequence(&callbacks.layouts, |encoder, layout| {
        encoder.u32(layout.formal_ordinal);
        encoder.u32(layout.native_ordinal);
        layout_application(encoder, &layout.root_layout)?;
        encoder.option(layout.inline_field.as_ref(), |encoder, field| {
            encode_nominal(encoder, &field.field)?;
            encoder.u64(field.offset);
            encoder.u64(field.extent);
            encoder.u64(field.alignment);
            layout_application(encoder, &field.child_layout)
        })?;
        encode_application(encoder, &layout.terminal_slot)?;
        encoder.u64(layout.terminal_offset);
        encoder.u64(layout.terminal_byte_size);
        encoder.u64(layout.terminal_alignment);
        encoder.u64(layout.composed_offset);
        Ok(())
    })
}

fn destination(encoder: &mut Encoder, destination: &PackagePolicyCallbackDestination) {
    match destination {
        PackagePolicyCallbackDestination::Parameter { native_ordinal } => {
            encoder.byte(0);
            encoder.u32(*native_ordinal);
        }
        PackagePolicyCallbackDestination::Field {
            native_ordinal,
            layout_index,
        } => {
            encoder.byte(1);
            encoder.u32(*native_ordinal);
            encoder.u32(*layout_index);
        }
    }
}

fn layout_application(
    encoder: &mut Encoder,
    layout: &PackagePolicyCallbackLayoutApplication,
) -> Result<(), PackageReviewEncodingError> {
    encode_nominal(encoder, &layout.policy)?;
    type_identity(encoder, &layout.schema)?;
    encoder.u64(layout.byte_size);
    encoder.u64(layout.alignment);
    Ok(())
}

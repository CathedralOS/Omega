// Canonical encoding for calling-policy callback binders, demands, materializations, and layouts.

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
            encoder.field_by("parameter", &binder.parameter, encode_nominal)?;
            encoder.u32_field("static_parameter_ordinal", binder.static_parameter_ordinal)?;
            encoder.u32_field("static_machine_ordinal", binder.static_machine_ordinal)?;
            encoder.field_by("requirement", &binder.requirement, encode_nominal)
        })
    })?;
    encoder.field("demands", |encoder| {
        encoder.sequence(&callbacks.demands, |encoder, demand| {
            encoder.field_by("destination", &demand.destination, destination)?;
            encoder.field_by("requirement", &demand.requirement, encode_nominal)
        })
    })?;
    encoder.field("materializations", |encoder| {
        encoder.sequence(&callbacks.materializations, |encoder, materialization| {
            encoder.u32_field("binder_index", materialization.binder_index)?;
            encoder.field_by("destination", &materialization.destination, destination)?;
            Ok(())
        })
    })?;
    encoder.field("layouts", |encoder| {
        encoder.sequence(&callbacks.layouts, |encoder, layout| {
            encoder.u32_field("formal_ordinal", layout.formal_ordinal)?;
            encoder.u32_field("native_ordinal", layout.native_ordinal)?;
            encoder.field_by("root_layout", &layout.root_layout, layout_application)?;
            encoder.field("inline_fields", |encoder| {
                encoder.sequence(&layout.inline_fields, |encoder, field| {
                    encoder.field("field", |encoder| encode_nominal(encoder, &field.field))?;
                    encoder.u64_field("offset", field.offset)?;
                    encoder.u64_field("extent", field.extent)?;
                    encoder.u64_field("alignment", field.alignment)?;
                    encoder.field_by("child_layout", &field.child_layout, layout_application)
                })
            })?;
            encoder.field_by("terminal_slot", &layout.terminal_slot, encode_application)?;
            encoder.field("terminal_placements", |encoder| {
                encoder.sequence(&layout.terminal_placements, placement)
            })?;
            encoder.u64_field("terminal_byte_size", layout.terminal_byte_size)?;
            encoder.u64_field("terminal_alignment", layout.terminal_alignment)?;
            encoder.field("composed_placements", |encoder| {
                encoder.sequence(&layout.composed_placements, placement)
            })?;
            Ok(())
        })
    })
}

fn destination(encoder: &mut Encoder, destination: &PackagePolicyCallbackDestination) {
    match destination {
        PackagePolicyCallbackDestination::Parameter { native_ordinal } => {
            encoder.tag("parameter", 0);
            let _ = encoder.u32_field("native_ordinal", *native_ordinal);
        }
        PackagePolicyCallbackDestination::Field {
            native_ordinal,
            layout_index,
        } => {
            encoder.tag("field", 1);
            let _ = encoder.u32_field("native_ordinal", *native_ordinal);
            let _ = encoder.u32_field("layout_index", *layout_index);
        }
    }
}

fn layout_application(
    encoder: &mut Encoder,
    layout: &PackagePolicyCallbackLayoutApplication,
) -> Result<(), PackageReviewEncodingError> {
    encoder.option_field("policy", layout.policy.as_ref(), encode_nominal)?;
    encoder.field("schema", |encoder| type_identity(encoder, &layout.schema))?;
    encoder.u64_field("byte_size", layout.byte_size)?;
    encoder.u64_field("alignment", layout.alignment)?;
    Ok(())
}

fn placement(
    encoder: &mut Encoder,
    placement: &layout_plans::LayoutPlacementReport,
) -> Result<(), PackageReviewEncodingError> {
    use layout_plans::{IntegerInterpretation, LayoutPlacementReport};
    match *placement {
        LayoutPlacementReport::At { offset } => {
            encoder.tag("at", 0);
            encoder.u64_field("offset", offset)?;
        }
        LayoutPlacementReport::IntegerAt {
            offset,
            stored_width,
            interpretation,
        } => {
            encoder.tag("integer_at", 1);
            encoder.u64_field("offset", offset)?;
            encoder.u64_field("stored_width", stored_width)?;
            match interpretation {
                IntegerInterpretation::Signed => encoder.tag("signed", 0),
                IntegerInterpretation::Unsigned => encoder.tag("unsigned", 1),
            }
        }
        LayoutPlacementReport::Bits {
            container,
            container_width,
            destination_lsb,
            source_lsb,
            width,
        } => {
            encoder.tag("bits", 2);
            encoder.u64_field("container", container)?;
            encoder.u64_field("container_width", container_width)?;
            encoder.u64_field("destination_lsb", destination_lsb)?;
            encoder.u64_field("source_lsb", source_lsb)?;
            encoder.u64_field("width", width)?;
        }
    }
    Ok(())
}

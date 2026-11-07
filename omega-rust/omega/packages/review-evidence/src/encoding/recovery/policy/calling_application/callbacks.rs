// Bounded recovery for calling-plan callback binders, demands, materializations, and layouts.

use super::{Error, Reader, nominal, type_identity};
use crate::encoding::recovery::policy::conformance;
use crate::record::{
    PackagePolicyCallbackBinder, PackagePolicyCallbackDemand, PackagePolicyCallbackDestination,
    PackagePolicyCallbackInlineField, PackagePolicyCallbackLayout,
    PackagePolicyCallbackLayoutApplication, PackagePolicyCallbackMaterialization,
    PackagePolicyCallbacks,
};

pub(in crate::encoding::recovery) fn decode(
    reader: &mut Reader<'_>,
) -> Result<PackagePolicyCallbacks, Error> {
    Ok(PackagePolicyCallbacks {
        binders: reader.sequence(90, |reader| {
            Ok(PackagePolicyCallbackBinder {
                parameter: nominal(reader)?,
                static_parameter_ordinal: reader.u32()?,
                static_machine_ordinal: reader.u32()?,
                requirement: nominal(reader)?,
            })
        })?,
        demands: reader.sequence(46, |reader| {
            Ok(PackagePolicyCallbackDemand {
                destination: destination(reader)?,
                requirement: nominal(reader)?,
            })
        })?,
        materializations: reader.sequence(9, |reader| {
            Ok(PackagePolicyCallbackMaterialization {
                binder_index: reader.u32()?,
                destination: destination(reader)?,
            })
        })?,
        layouts: reader.sequence(1, |reader| {
            Ok(PackagePolicyCallbackLayout {
                formal_ordinal: reader.u32()?,
                native_ordinal: reader.u32()?,
                root_layout: layout_application(reader)?,
                inline_fields: reader.sequence(1, |reader| {
                    Ok(PackagePolicyCallbackInlineField {
                        field: nominal(reader)?,
                        offset: reader.u64()?,
                        extent: reader.u64()?,
                        alignment: reader.u64()?,
                        child_layout: layout_application(reader)?,
                    })
                })?,
                terminal_slot: conformance::application(reader)?,
                terminal_placements: reader.sequence(9, placement)?,
                terminal_byte_size: reader.u64()?,
                terminal_alignment: reader.u64()?,
                composed_placements: reader.sequence(9, placement)?,
            })
        })?,
    })
}

pub(in crate::encoding::recovery) fn destination(
    reader: &mut Reader<'_>,
) -> Result<PackagePolicyCallbackDestination, Error> {
    Ok(super::super::tag!(reader => {
        0 => PackagePolicyCallbackDestination::Parameter {
            native_ordinal: reader.u32()?,
        },
        1 => PackagePolicyCallbackDestination::Field {
            native_ordinal: reader.u32()?,
            layout_index: reader.u32()?,
        },
    }))
}

pub(in crate::encoding::recovery) fn layout_application(
    reader: &mut Reader<'_>,
) -> Result<PackagePolicyCallbackLayoutApplication, Error> {
    Ok(PackagePolicyCallbackLayoutApplication {
        policy: reader.option(nominal)?,
        schema: type_identity(reader)?,
        byte_size: reader.u64()?,
        alignment: reader.u64()?,
    })
}

fn placement(reader: &mut Reader<'_>) -> Result<layout_plans::LayoutPlacementReport, Error> {
    use layout_plans::{IntegerInterpretation, LayoutPlacementReport};
    Ok(super::super::tag!(reader => {
        0 => LayoutPlacementReport::At { offset: reader.u64()? },
        1 => LayoutPlacementReport::IntegerAt {
            offset: reader.u64()?,
            stored_width: reader.u64()?,
            interpretation: super::super::tag!(reader => {
                0 => IntegerInterpretation::Signed,
                1 => IntegerInterpretation::Unsigned,
            }),
        },
        2 => LayoutPlacementReport::Bits {
            container: reader.u64()?,
            container_width: reader.u64()?,
            destination_lsb: reader.u64()?,
            source_lsb: reader.u64()?,
            width: reader.u64()?,
        },
    }))
}

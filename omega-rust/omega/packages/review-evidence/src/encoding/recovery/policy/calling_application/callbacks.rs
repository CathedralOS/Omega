use super::{Error, Reader, nominal, type_identity};
use crate::encoding::recovery::policy::conformance;
use crate::record::{
    PackagePolicyCallbackBinder, PackagePolicyCallbackDemand, PackagePolicyCallbackDestination,
    PackagePolicyCallbackInlineField, PackagePolicyCallbackLayout,
    PackagePolicyCallbackLayoutApplication, PackagePolicyCallbackMaterialization,
    PackagePolicyCallbacks,
};

pub(super) fn decode(reader: &mut Reader<'_>) -> Result<PackagePolicyCallbacks, Error> {
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
                inline_field: reader.option(|reader| {
                    Ok(PackagePolicyCallbackInlineField {
                        field: nominal(reader)?,
                        offset: reader.u64()?,
                        extent: reader.u64()?,
                        alignment: reader.u64()?,
                        child_layout: layout_application(reader)?,
                    })
                })?,
                terminal_slot: conformance::application(reader)?,
                terminal_offset: reader.u64()?,
                terminal_byte_size: reader.u64()?,
                terminal_alignment: reader.u64()?,
                composed_offset: reader.u64()?,
            })
        })?,
    })
}

fn destination(reader: &mut Reader<'_>) -> Result<PackagePolicyCallbackDestination, Error> {
    Ok(match reader.byte()? {
        0 => PackagePolicyCallbackDestination::Parameter {
            native_ordinal: reader.u32()?,
        },
        1 => PackagePolicyCallbackDestination::Field {
            native_ordinal: reader.u32()?,
            layout_index: reader.u32()?,
        },
        _ => return Err(Error::InvalidTag),
    })
}

fn layout_application(
    reader: &mut Reader<'_>,
) -> Result<PackagePolicyCallbackLayoutApplication, Error> {
    Ok(PackagePolicyCallbackLayoutApplication {
        policy: nominal(reader)?,
        schema: type_identity(reader)?,
        byte_size: reader.u64()?,
        alignment: reader.u64()?,
    })
}

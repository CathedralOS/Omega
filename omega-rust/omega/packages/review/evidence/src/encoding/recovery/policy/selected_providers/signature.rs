use super::{Error, Reader};
use crate::encoding::recovery::policy::{identity::type_identity, signatures::type_parameter};
use crate::record::{PackagePolicyServiceSignature, PackageReviewTraitRequirementParameter};

pub(super) fn signature(reader: &mut Reader<'_>) -> Result<PackagePolicyServiceSignature, Error> {
    Ok(PackagePolicyServiceSignature {
        schema_arguments: reader.sequence(8, type_identity)?,
        schema_lifetime_parameter_count: reader.u32()?,
        requirement_arguments: reader.sequence(8, type_identity)?,
        requirement_lifetime_arguments: reader.sequence(4, Reader::u32)?,
        requirement_lifetime_parameter_count: reader.u32()?,
        static_parameters: reader.sequence(3, type_parameter)?,
        parameters: reader.sequence(19, |reader| {
            Ok(PackageReviewTraitRequirementParameter {
                name: reader.string()?,
                type_identity: type_identity(reader)?,
                is_const: reader.boolean()?,
                is_mutable: reader.boolean()?,
                is_self: reader.boolean()?,
            })
        })?,
        result: reader.option(type_identity)?,
    })
}

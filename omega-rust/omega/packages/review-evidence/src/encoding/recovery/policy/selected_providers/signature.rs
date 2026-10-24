use super::{Error, Reader};
use crate::encoding::recovery::policy::{
    identity::{signature_parameter, type_identity},
    public_api::type_parameter,
};
use crate::record::PackagePolicyServiceSignature;

pub(in crate::encoding::recovery) fn signature(
    reader: &mut Reader<'_>,
) -> Result<PackagePolicyServiceSignature, Error> {
    Ok(PackagePolicyServiceSignature {
        schema_arguments: reader.sequence(8, type_identity)?,
        schema_lifetime_parameter_count: reader.u32()?,
        schema_lifetime_arguments: reader.sequence(4, |reader| reader.u32())?,
        application_lifetime_parameter_count: reader.u32()?,
        requirement_arguments: reader.sequence(8, type_identity)?,
        requirement_lifetime_arguments: reader.sequence(4, Reader::u32)?,
        requirement_lifetime_parameter_count: reader.u32()?,
        static_parameters: reader.sequence(3, type_parameter)?,
        parameters: reader.sequence(18, signature_parameter)?,
        result: reader.option(type_identity)?,
    })
}

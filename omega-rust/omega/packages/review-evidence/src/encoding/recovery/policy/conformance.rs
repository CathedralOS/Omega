//! Bounded recovery of the complete inert conformance application component.

use super::{
    Error,
    identity::{nominal, type_identity},
    reader::Reader,
};
use crate::record::{
    PackagePolicyClosedConformanceApplication, PackagePolicyConformanceConstArgument,
    PackagePolicyConformanceRow,
};

pub(in crate::encoding::recovery) fn application(
    reader: &mut Reader<'_>,
) -> Result<PackagePolicyClosedConformanceApplication, Error> {
    Ok(PackagePolicyClosedConformanceApplication {
        declaration: nominal(reader)?,
        lifetime_arguments: reader.sequence(4, Reader::u32)?,
        type_arguments: reader.sequence(8, type_identity)?,
        const_arguments: reader.sequence(1, const_argument)?,
        machine_arguments: reader.sequence(41, nominal)?,
        trait_identity: nominal(reader)?,
        trait_lifetime_arguments: reader.sequence(4, Reader::u32)?,
        trait_arguments: reader.sequence(8, type_identity)?,
        rows: reader.sequence(164, |reader| {
            Ok(PackagePolicyConformanceRow {
                declaring_trait: nominal(reader)?,
                requirement: nominal(reader)?,
                realization_machine: nominal(reader)?,
                realization_state: nominal(reader)?,
            })
        })?,
    })
}

pub(in crate::encoding::recovery) fn const_argument(
    reader: &mut Reader<'_>,
) -> Result<PackagePolicyConformanceConstArgument, Error> {
    Ok(super::tag!(reader => {
        0 => PackagePolicyConformanceConstArgument::Evaluated {
            parameter_carrier: type_identity(reader)?,
            declared_carrier: type_identity(reader)?,
            canonical_value_encoding: reader.string()?},
        2 => PackagePolicyConformanceConstArgument::CallerExpression {
            parameter_carrier: type_identity(reader)?,
            expression: type_identity(reader)?},
        1 => PackagePolicyConformanceConstArgument::CallerBinder {
            parameter_carrier: type_identity(reader)?,
            binder: nominal(reader)?,
            binder_carrier: type_identity(reader)?}}))
}

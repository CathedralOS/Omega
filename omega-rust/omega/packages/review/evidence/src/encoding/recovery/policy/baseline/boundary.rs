use super::super::{
    identity::{nominal, operator_coordinate, type_identity},
    intrinsic,
};
use super::*;

pub(super) fn applications(
    reader: &mut Reader<'_>,
) -> Result<PackagePolicyBoundaryApplications, Error> {
    Ok(PackagePolicyBoundaryApplications {
        demands: reader.sequence(1, |reader| {
            Ok(PackagePolicyBoundaryApplicationDemand {
                operator_coordinate: operator_coordinate(reader)?,
                producer_callable: nominal(reader)?,
                arguments: reader.sequence(9, |reader| match reader.byte()? {
                    0 => Ok(
                        PackageReviewSymbolicBoundaryApplicationArgument::TypeBinder {
                            requirement_binder_ordinal: reader.u32()?,
                            producer_binder_ordinal: reader.u32()?,
                        },
                    ),
                    _ => Err(Error::InvalidTag),
                })?,
            })
        })?,
        realizations: reader.sequence(1, |reader| {
            Ok(PackagePolicyBoundaryApplicationRealization {
                operator_coordinate: operator_coordinate(reader)?,
                requirement_identity: reader.string()?,
                application: application(reader)?,
                selected_plan_index: reader.u32()?,
                realization: match reader.byte()? {
                    0 => PackagePolicyBoundaryRealization::NongenericCheckedBody {
                        declaration: nominal(reader)?,
                        realization: nominal(reader)?,
                    },
                    1 => PackagePolicyBoundaryRealization::SpecializedCheckedBody {
                        declaration: nominal(reader)?,
                        template: nominal(reader)?,
                    },
                    2 => PackagePolicyBoundaryRealization::ExactCompilerIntrinsic {
                        execution: intrinsic::execution(reader)?,
                    },
                    _ => return Err(Error::InvalidTag),
                },
            })
        })?,
    })
}

fn application(reader: &mut Reader<'_>) -> Result<PackageReviewBoundaryApplication, Error> {
    Ok(match reader.byte()? {
        0 => PackageReviewBoundaryApplication::Empty,
        1 => PackageReviewBoundaryApplication::Exact(reader.sequence(1, |reader| {
            Ok(match reader.byte()? {
                0 => PackageReviewBoundaryApplicationArgument::Type {
                    binder_ordinal: reader.u32()?,
                    type_identity: type_identity(reader)?,
                },
                1 => PackageReviewBoundaryApplicationArgument::Const {
                    binder_ordinal: reader.u32()?,
                    declared_carrier: type_identity(reader)?,
                    value_type: reader.string()?,
                    value_encoding: reader.string()?,
                },
                _ => return Err(Error::InvalidTag),
            })
        })?),
        _ => return Err(Error::InvalidTag),
    })
}

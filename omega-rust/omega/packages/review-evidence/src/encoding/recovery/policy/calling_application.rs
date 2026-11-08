//! One bounded decoder for the complete inert calling-policy graph.

pub(in crate::encoding::recovery) mod callbacks;
pub(in crate::encoding::recovery) mod opaque;
pub(in crate::encoding::recovery) mod shapes;

use super::identity::{nominal, type_identity};
use super::{Error, PackagePolicyRecoveryLimits, reader::Reader};
use crate::encoding::{CALLING_POLICY_MAGIC, PACKAGE_CALLING_POLICY_VERSION};
use crate::record::{
    PackagePolicyCallingParameter, PackagePolicyCallingPlan, PackagePolicyNativeParameter,
    PackagePolicyNativeParameterOrigin,
};

impl PackagePolicyCallingPlan {
    super::recover_canonical_component!(
        CALLING_POLICY_MAGIC,
        PACKAGE_CALLING_POLICY_VERSION,
        application,
        validate,
    );
}

/// Decode inner fields with the caller's aggregate budgets and framing.
/// The enclosing record validates all structural associations after decoding.
pub(in crate::encoding::recovery) fn application(
    reader: &mut Reader<'_>,
) -> Result<PackagePolicyCallingPlan, Error> {
    Ok(PackagePolicyCallingPlan {
        abi: declaration_abi(reader)?,
        boundary_trait: nominal(reader)?,
        boundary_arguments: reader.sequence(8, type_identity)?,
        boundary_lifetime_parameter_count: reader.u32()?,
        boundary_lifetime_arguments: reader.sequence(4, |reader| reader.u32())?,
        application_lifetime_parameter_count: reader.u32()?,
        requirement: nominal(reader)?,
        requirement_trait: nominal(reader)?,
        requirement_arguments: reader.sequence(8, type_identity)?,
        requirement_lifetime_arguments: reader.sequence(4, Reader::u32)?,
        requirement_lifetime_parameter_count: reader.u32()?,
        static_parameters: reader.sequence(1, super::public_api::type_parameter)?,
        target: shapes::target(reader)?,
        shape_graph: shapes::graph(reader)?,
        semantic_parameters: reader.sequence(20, |reader| {
            Ok(PackagePolicyCallingParameter {
                name: reader.string()?,
                value_type: type_identity(reader)?,
                is_mutable: reader.boolean()?,
                is_const: reader.boolean()?,
                shape_root: reader.u16()?,
            })
        })?,
        semantic_result: reader.option(type_identity)?,
        native_parameters: reader.sequence(15, |reader| {
            Ok(PackagePolicyNativeParameter {
                name: reader.string()?,
                origin: super::tag!(reader => {
                    0 => PackagePolicyNativeParameterOrigin::SemanticFormal {
                        formal_ordinal: reader.u32()?,
                        shape_root: reader.u16()?,
                    },
                    1 => PackagePolicyNativeParameterOrigin::PrivateCallback {
                        binder_index: reader.u32()?,
                        byte_size: reader.u16()?,
                        alignment: reader.u16()?,
                    },
                }),
            })
        })?,
        callbacks: callbacks::decode(reader)?,
        opaque_uses: reader.sequence(1, opaque::decode)?,
        physical: super::physical_calling_policy::physical(reader)?,
    })
}

fn declaration_abi(
    reader: &mut Reader<'_>,
) -> Result<semantic_vocabulary::CallingAbiIdentity, Error> {
    use semantic_vocabulary::{CallingAbiIdentity, CallingOperandKind, CallingStaticOperand};
    let coordinate = reader.u64()?;
    let abi = CallingAbiIdentity {
        requirement_identity: reader.string()?, declaring_application: reader.string()?,
        operands: reader.sequence(13, |reader| Ok(CallingStaticOperand {
            ordinal: reader.u32()?,
            kind: super::tag!(reader => { 0 => CallingOperandKind::Type, 1 => CallingOperandKind::Const, 2 => CallingOperandKind::Value, 3 => CallingOperandKind::Machine, 4 => CallingOperandKind::Implementation, 5 => CallingOperandKind::Proposition }),
            identity: reader.string()?,
        }))?,
    };
    if abi.validate().is_err() || abi.id().get() != coordinate {
        return Err(Error::NonCanonicalEncoding);
    }
    Ok(abi)
}

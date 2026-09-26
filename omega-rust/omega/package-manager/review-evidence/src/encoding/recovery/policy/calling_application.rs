//! One bounded decoder for the complete inert calling-policy graph.

#[cfg(test)]
pub(super) mod budgets;
mod callbacks;
#[cfg(test)]
mod malformed;
mod opaque;
pub(super) mod shapes;
#[cfg(test)]
pub(super) mod tests;

use super::identity::{nominal, type_identity};
use super::{Error, PackagePolicyRecoveryLimits, reader::Reader};
use crate::encoding::{CALLING_POLICY_MAGIC, PACKAGE_CALLING_POLICY_VERSION};
use crate::record::{
    PackagePolicyCallingParameter, PackagePolicyCallingPlan, PackagePolicyNativeParameter,
    PackagePolicyNativeParameterOrigin,
};

impl PackagePolicyCallingPlan {
    /// Recover policy meaning with one shared byte/element/allocation budget.
    /// This never recovers a checked plan, native evidence, or project decision.
    pub fn recover_canonical(
        bytes: &[u8],
        limits: PackagePolicyRecoveryLimits,
    ) -> Result<Self, Error> {
        let mut reader = Reader::new(bytes, limits)?;
        reader.literal(CALLING_POLICY_MAGIC)?;
        if reader.u16()? != PACKAGE_CALLING_POLICY_VERSION {
            return Err(Error::UnsupportedVersion);
        }
        let application = application(&mut reader)?;
        reader.finish()?;
        application
            .validate_canonical_structure()
            .map_err(|_| Error::InvalidValue)?;
        reader.canonical_scratch(bytes.len())?;
        if application
            .canonical_bytes()
            .map_err(|_| Error::NonCanonicalEncoding)?
            != bytes
        {
            return Err(Error::NonCanonicalEncoding);
        }
        Ok(application)
    }
}

/// Decode inner fields with the caller's aggregate budgets and framing.
/// The enclosing record validates all structural associations after decoding.
pub(super) fn application(reader: &mut Reader<'_>) -> Result<PackagePolicyCallingPlan, Error> {
    Ok(PackagePolicyCallingPlan {
        boundary_trait: nominal(reader)?,
        boundary_arguments: reader.sequence(8, type_identity)?,
        boundary_lifetime_parameter_count: reader.u32()?,
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
                origin: match reader.byte()? {
                    0 => PackagePolicyNativeParameterOrigin::SemanticFormal {
                        formal_ordinal: reader.u32()?,
                        shape_root: reader.u16()?,
                    },
                    1 => PackagePolicyNativeParameterOrigin::PrivateCallback {
                        binder_index: reader.u32()?,
                        byte_size: reader.u16()?,
                        alignment: reader.u16()?,
                    },
                    _ => return Err(Error::InvalidTag),
                },
            })
        })?,
        callbacks: callbacks::decode(reader)?,
        opaque_uses: reader.sequence(1, opaque::decode)?,
        physical: super::physical_calling_policy::physical(reader)?,
    })
}

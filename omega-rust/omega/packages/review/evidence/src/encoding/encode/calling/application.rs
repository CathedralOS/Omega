use super::{Encoder, PackageReviewEncodingError, callbacks, opaque, ordinal, type_identity};
use crate::encoding::encode::declarations::{
    encode_boundary_shape_graph, encode_representation_target, encode_type_parameter,
};
use crate::encoding::encode::values::identity::encode_nominal;
use crate::encoding::encode::values::physical_calling_policy::encode_physical;
use crate::encoding::{CALLING_POLICY_MAGIC, PACKAGE_CALLING_POLICY_VERSION};
use crate::record::{PackagePolicyCallingPlan, PackagePolicyNativeParameterOrigin};

impl PackagePolicyCallingPlan {
    /// Complete normalized calling-policy bytes. Native/compiler identities,
    /// realization receipts, and acceptance decisions do not enter this record.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, PackageReviewEncodingError> {
        self.validate_canonical_structure()
            .map_err(PackageReviewEncodingError::new)?;
        let mut encoder = Encoder::policy_bounded(4 * 1024 * 1024);
        encoder.fixed_bytes(CALLING_POLICY_MAGIC);
        encoder.u16(PACKAGE_CALLING_POLICY_VERSION);
        encode_nominal(&mut encoder, &self.boundary_trait)?;
        encoder.sequence(&self.boundary_arguments, type_identity)?;
        encoder.u32(self.boundary_lifetime_parameter_count);
        encode_nominal(&mut encoder, &self.requirement)?;
        encode_nominal(&mut encoder, &self.requirement_trait)?;
        encoder.sequence(&self.requirement_arguments, type_identity)?;
        encoder.sequence(&self.requirement_lifetime_arguments, ordinal)?;
        encoder.u32(self.requirement_lifetime_parameter_count);
        encoder.sequence(&self.static_parameters, encode_type_parameter)?;
        encode_representation_target(&mut encoder, self.target);
        encode_boundary_shape_graph(&mut encoder, &self.shape_graph)?;
        encoder.sequence(&self.semantic_parameters, |encoder, parameter| {
            encoder.string(&parameter.name)?;
            type_identity(encoder, &parameter.value_type)?;
            encoder.boolean(parameter.is_mutable);
            encoder.boolean(parameter.is_const);
            encoder.u16(parameter.shape_root);
            Ok(())
        })?;
        encoder.option(self.semantic_result.as_ref(), type_identity)?;
        encoder.sequence(&self.native_parameters, |encoder, parameter| {
            encoder.string(&parameter.name)?;
            match parameter.origin {
                PackagePolicyNativeParameterOrigin::SemanticFormal {
                    formal_ordinal,
                    shape_root,
                } => {
                    encoder.byte(0);
                    encoder.u32(formal_ordinal);
                    encoder.u16(shape_root);
                }
                PackagePolicyNativeParameterOrigin::PrivateCallback {
                    binder_index,
                    byte_size,
                    alignment,
                } => {
                    encoder.byte(1);
                    encoder.u32(binder_index);
                    encoder.u16(byte_size);
                    encoder.u16(alignment);
                }
            }
            Ok(())
        })?;
        callbacks::encode(&mut encoder, &self.callbacks)?;
        encoder.sequence(&self.opaque_uses, opaque::encode)?;
        encode_physical(&mut encoder, &self.physical)?;
        encoder.finish()
    }
}

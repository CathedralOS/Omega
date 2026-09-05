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
        encode_application(&mut encoder, self)?;
        encoder.finish()
    }
}

/// Inner semantic fields share their enclosing component's writer budget.
pub(crate) fn encode_application(
    encoder: &mut Encoder,
    application: &PackagePolicyCallingPlan,
) -> Result<(), PackageReviewEncodingError> {
    encode_nominal(encoder, &application.boundary_trait)?;
    encoder.sequence(&application.boundary_arguments, type_identity)?;
    encoder.u32(application.boundary_lifetime_parameter_count);
    encode_nominal(encoder, &application.requirement)?;
    encode_nominal(encoder, &application.requirement_trait)?;
    encoder.sequence(&application.requirement_arguments, type_identity)?;
    encoder.sequence(&application.requirement_lifetime_arguments, ordinal)?;
    encoder.u32(application.requirement_lifetime_parameter_count);
    encoder.sequence(&application.static_parameters, encode_type_parameter)?;
    encode_representation_target(encoder, application.target);
    encode_boundary_shape_graph(encoder, &application.shape_graph)?;
    encoder.sequence(&application.semantic_parameters, |encoder, parameter| {
        encoder.string(&parameter.name)?;
        type_identity(encoder, &parameter.value_type)?;
        encoder.boolean(parameter.is_mutable);
        encoder.boolean(parameter.is_const);
        encoder.u16(parameter.shape_root);
        Ok(())
    })?;
    encoder.option(application.semantic_result.as_ref(), type_identity)?;
    encoder.sequence(&application.native_parameters, |encoder, parameter| {
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
    callbacks::encode(encoder, &application.callbacks)?;
    encoder.sequence(&application.opaque_uses, opaque::encode)?;
    encode_physical(encoder, &application.physical)
}

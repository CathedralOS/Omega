use super::super::encoder::encode_canonical_component;
use super::{Encoder, PackageReviewEncodingError, callbacks, opaque, ordinal, type_identity};
use crate::encoding::encode::declarations::{
    encode_boundary_shape_graph, encode_representation_target,
};
use crate::encoding::encode::public_api::type_parameter as encode_type_parameter;
use crate::encoding::encode::values::identity::encode_nominal;
use crate::encoding::encode::values::physical_calling_policy::encode_physical;
use crate::encoding::{CALLING_POLICY_MAGIC, PACKAGE_CALLING_POLICY_VERSION};
use crate::record::{PackagePolicyCallingPlan, PackagePolicyNativeParameterOrigin};

impl PackagePolicyCallingPlan {
    encode_canonical_component!(
        CALLING_POLICY_MAGIC,
        PACKAGE_CALLING_POLICY_VERSION,
        encode_application
    );
}

/// Inner semantic fields share their enclosing component's writer budget.
pub(crate) fn encode_application(
    encoder: &mut Encoder,
    application: &PackagePolicyCallingPlan,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field_by(
        "boundary_trait",
        &application.boundary_trait,
        encode_nominal,
    )?;
    encoder.field("boundary_arguments", |encoder| {
        encoder.sequence(&application.boundary_arguments, type_identity)
    })?;
    encoder.u32_field(
        "boundary_lifetime_parameter_count",
        application.boundary_lifetime_parameter_count,
    )?;
    encoder.field("boundary_lifetime_arguments", |encoder| {
        encoder.sequence(
            &application.boundary_lifetime_arguments,
            |encoder, ordinal| {
                encoder.u32(*ordinal);
                Ok(())
            },
        )
    })?;
    encoder.u32_field(
        "application_lifetime_parameter_count",
        application.application_lifetime_parameter_count,
    )?;
    encoder.field_by("requirement", &application.requirement, encode_nominal)?;
    encoder.field_by(
        "requirement_trait",
        &application.requirement_trait,
        encode_nominal,
    )?;
    encoder.field("requirement_arguments", |encoder| {
        encoder.sequence(&application.requirement_arguments, type_identity)
    })?;
    encoder.field("requirement_lifetime_arguments", |encoder| {
        encoder.sequence(&application.requirement_lifetime_arguments, ordinal)
    })?;
    encoder.u32_field(
        "requirement_lifetime_parameter_count",
        application.requirement_lifetime_parameter_count,
    )?;
    encoder.field("static_parameters", |encoder| {
        encoder.sequence(&application.static_parameters, encode_type_parameter)
    })?;
    encoder.field("target", |encoder| {
        encode_representation_target(encoder, application.target);
        Ok(())
    })?;
    encoder.field_by(
        "shape_graph",
        &application.shape_graph,
        encode_boundary_shape_graph,
    )?;
    encoder.field("semantic_parameters", |encoder| {
        encoder.sequence(&application.semantic_parameters, |encoder, parameter| {
            encoder.field("name", |encoder| encoder.string(&parameter.name))?;
            encoder.field_by("value_type", &parameter.value_type, type_identity)?;
            encoder.boolean_field("is_mutable", parameter.is_mutable)?;
            encoder.boolean_field("is_const", parameter.is_const)?;
            encoder.u16_field("shape_root", parameter.shape_root)?;
            Ok(())
        })
    })?;
    encoder.field("semantic_result", |encoder| {
        encoder.option(application.semantic_result.as_ref(), type_identity)
    })?;
    encoder.field("native_parameters", |encoder| {
        encoder.sequence(&application.native_parameters, |encoder, parameter| {
            encoder.field("name", |encoder| encoder.string(&parameter.name))?;
            encoder.field("origin", |encoder| {
                match parameter.origin {
                    PackagePolicyNativeParameterOrigin::SemanticFormal {
                        formal_ordinal,
                        shape_root,
                    } => {
                        encoder.tag("semantic_formal", 0);
                        encoder.u32_field("formal_ordinal", formal_ordinal)?;
                        encoder.u16_field("shape_root", shape_root)?;
                    }
                    PackagePolicyNativeParameterOrigin::PrivateCallback {
                        binder_index,
                        byte_size,
                        alignment,
                    } => {
                        encoder.tag("private_callback", 1);
                        encoder.u32_field("binder_index", binder_index)?;
                        encoder.u16_field("byte_size", byte_size)?;
                        encoder.u16_field("alignment", alignment)?;
                    }
                };
                Ok(())
            })?;
            Ok(())
        })
    })?;
    encoder.field_by("callbacks", &application.callbacks, callbacks::encode)?;
    encoder.field("opaque_uses", |encoder| {
        encoder.sequence(&application.opaque_uses, opaque::encode)
    })?;
    encoder.field_by("physical", &application.physical, encode_physical)
}

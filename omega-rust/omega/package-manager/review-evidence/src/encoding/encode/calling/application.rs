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
    encoder.field("boundary_trait", |encoder| {
        encode_nominal(encoder, &application.boundary_trait)
    })?;
    encoder.field("boundary_arguments", |encoder| {
        encoder.sequence(&application.boundary_arguments, type_identity)
    })?;
    encoder.field("boundary_lifetime_parameter_count", |encoder| {
        encoder.u32(application.boundary_lifetime_parameter_count);
        Ok(())
    })?;
    encoder.field("requirement", |encoder| {
        encode_nominal(encoder, &application.requirement)
    })?;
    encoder.field("requirement_trait", |encoder| {
        encode_nominal(encoder, &application.requirement_trait)
    })?;
    encoder.field("requirement_arguments", |encoder| {
        encoder.sequence(&application.requirement_arguments, type_identity)
    })?;
    encoder.field("requirement_lifetime_arguments", |encoder| {
        encoder.sequence(&application.requirement_lifetime_arguments, ordinal)
    })?;
    encoder.field("requirement_lifetime_parameter_count", |encoder| {
        encoder.u32(application.requirement_lifetime_parameter_count);
        Ok(())
    })?;
    encoder.field("static_parameters", |encoder| {
        encoder.sequence(&application.static_parameters, encode_type_parameter)
    })?;
    encoder.field("target", |encoder| {
        encode_representation_target(encoder, application.target);
        Ok(())
    })?;
    encoder.field("shape_graph", |encoder| {
        encode_boundary_shape_graph(encoder, &application.shape_graph)
    })?;
    encoder.field("semantic_parameters", |encoder| {
        encoder.sequence(&application.semantic_parameters, |encoder, parameter| {
            encoder.field("name", |encoder| encoder.string(&parameter.name))?;
            encoder.field("value_type", |encoder| {
                type_identity(encoder, &parameter.value_type)
            })?;
            encoder.field("is_mutable", |encoder| {
                encoder.boolean(parameter.is_mutable);
                Ok(())
            })?;
            encoder.field("is_const", |encoder| {
                encoder.boolean(parameter.is_const);
                Ok(())
            })?;
            encoder.field("shape_root", |encoder| {
                encoder.u16(parameter.shape_root);
                Ok(())
            })?;
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
                        encoder.field("formal_ordinal", |encoder| {
                            encoder.u32(formal_ordinal);
                            Ok(())
                        })?;
                        encoder.field("shape_root", |encoder| {
                            encoder.u16(shape_root);
                            Ok(())
                        })?;
                    }
                    PackagePolicyNativeParameterOrigin::PrivateCallback {
                        binder_index,
                        byte_size,
                        alignment,
                    } => {
                        encoder.tag("private_callback", 1);
                        encoder.field("binder_index", |encoder| {
                            encoder.u32(binder_index);
                            Ok(())
                        })?;
                        encoder.field("byte_size", |encoder| {
                            encoder.u16(byte_size);
                            Ok(())
                        })?;
                        encoder.field("alignment", |encoder| {
                            encoder.u16(alignment);
                            Ok(())
                        })?;
                    }
                };
                Ok(())
            })?;
            Ok(())
        })
    })?;
    encoder.field("callbacks", |encoder| {
        callbacks::encode(encoder, &application.callbacks)
    })?;
    encoder.field("opaque_uses", |encoder| {
        encoder.sequence(&application.opaque_uses, opaque::encode)
    })?;
    encoder.field("physical", |encoder| {
        encode_physical(encoder, &application.physical)
    })
}

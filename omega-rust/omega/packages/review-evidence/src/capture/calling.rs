//! Project the calling application already validated by the target planner.
//! Review carries its exact public telescope and physical geometry as inert data.

mod physical;

use super::declarations::unavailable;
use crate::record::{
    PackagePolicyCallbacks, PackagePolicyCallingParameter, PackagePolicyCallingPlan,
    PackagePolicyNativeParameter, PackagePolicyNativeParameterOrigin, PackagePolicyServiceMethod,
    PackageReviewBoundaryShape, PackageReviewBoundaryShapeField, PackageReviewBoundaryShapeGraph,
};
use calling_conventions::{
    BoundaryCallingApplication, BoundaryNativeParameterOrigin, BoundaryNativeParameterShape,
};
use diagnostics::Diagnostic;
use target::TargetProfile;

pub(super) fn project(
    application: &BoundaryCallingApplication,
    method: &PackagePolicyServiceMethod,
    target: TargetProfile,
) -> Result<PackagePolicyCallingPlan, Vec<Diagnostic>> {
    let signature = &application.signature;
    if application.target != target.native_target()
        || application.dispatch_only_parameter_count != 0
        || application
            .recomputed_commitment()
            .map_err(|reason| unavailable(&reason))?
            != application.commitment
        || signature.parameters.len() != method.signature.parameters.len()
        || signature.result.is_some() != method.signature.result.is_some()
    {
        return Err(unavailable(
            "the exact validated boundary calling application",
        ));
    }
    // These associations need their authored telescope. Disclose an empty
    // inventory only when the validated application also has none.
    if !application.callback_context.binders.is_empty()
        || !application.callback_context.demands.is_empty()
        || !signature.callback_binding_rows.is_empty()
        || !signature.callback_layout_rows.is_empty()
        || !application
            .plan
            .plan()
            .call
            .callback_materializations
            .is_empty()
        || !application.opaque_applications.is_empty()
    {
        return Err(unavailable(
            "the calling application's complete callback or opaque custody",
        ));
    }
    let semantic_parameters = method
        .signature
        .parameters
        .iter()
        .zip(&signature.parameters)
        .map(|(parameter, &shape_root)| PackagePolicyCallingParameter {
            name: parameter.name.clone(),
            value_type: parameter.type_identity.clone(),
            is_mutable: parameter.is_mutable,
            is_const: parameter.is_const,
            shape_root,
        })
        .collect::<Vec<_>>();
    let projection = application
        .call_projection()
        .map_err(|reason| unavailable(&reason))?;
    let native_parameters = projection
        .parameters()
        .map(|parameter| {
            let (
                BoundaryNativeParameterOrigin::SemanticFormal { formal_ordinal },
                BoundaryNativeParameterShape::Semantic(shape_root),
            ) = (parameter.origin, parameter.shape)
            else {
                return Err(unavailable(
                    "the reviewed native parameter's authored origin",
                ));
            };
            let formal = semantic_parameters
                .get(formal_ordinal as usize)
                .filter(|formal| formal.shape_root == shape_root)
                .ok_or_else(|| unavailable("the reviewed native parameter's semantic formal"))?;
            if calling_conventions::nominal_callback_native_parameter_id(
                &application.requirement_identity,
                &formal.name,
            ) != parameter.identity
            {
                return Err(unavailable(
                    "the reviewed native parameter's exact authored name",
                ));
            }
            Ok(PackagePolicyNativeParameter {
                name: formal.name.clone(),
                origin: PackagePolicyNativeParameterOrigin::SemanticFormal {
                    formal_ordinal,
                    shape_root,
                },
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(PackagePolicyCallingPlan {
        abi: application.abi.clone(),
        boundary_trait: method.requirement_owner.clone(),
        boundary_arguments: method.signature.schema_arguments.clone(),
        boundary_lifetime_parameter_count: method.signature.schema_lifetime_parameter_count,
        boundary_lifetime_arguments: method.signature.schema_lifetime_arguments.clone(),
        application_lifetime_parameter_count: method.signature.application_lifetime_parameter_count,
        requirement: method.requirement.clone(),
        requirement_trait: method.requirement_owner.clone(),
        requirement_arguments: method.signature.requirement_arguments.clone(),
        requirement_lifetime_arguments: method.signature.requirement_lifetime_arguments.clone(),
        requirement_lifetime_parameter_count: method.signature.requirement_lifetime_parameter_count,
        static_parameters: method.signature.static_parameters.clone(),
        target: super::policy::representation_target(target)?,
        shape_graph: PackageReviewBoundaryShapeGraph {
            shapes: signature
                .shapes
                .iter()
                .map(|shape| PackageReviewBoundaryShape {
                    class: shape.class(),
                    byte_size: shape.byte_size(),
                    alignment: shape.alignment(),
                })
                .collect(),
            fields: signature
                .fields
                .iter()
                .map(|field| PackageReviewBoundaryShapeField {
                    shape: field.shape(),
                    byte_offset: field.byte_offset(),
                })
                .collect(),
            parameters: signature.parameters.clone(),
            result: signature.result,
        },
        semantic_parameters,
        semantic_result: method.signature.result.clone(),
        native_parameters,
        callbacks: PackagePolicyCallbacks {
            binders: Vec::new(),
            demands: Vec::new(),
            materializations: Vec::new(),
            layouts: Vec::new(),
        },
        opaque_uses: Vec::new(),
        physical: crate::record::PackagePolicyPhysicalCallingContract::from_validated_plan(
            &application.plan,
        ),
    })
}

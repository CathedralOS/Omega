// Materialize default ABI applications from canonical Terminal declarations
// and validate their actual physical plans. Source calling-request preparation
// is unavailable; it cannot be replaced by an empty request roster.

use crate::calling_policy_plans::MaterializedBoundarySignature;
use crate::calling_policy_plans::boundary_signatures::BoundaryOpaqueMovementGeometry;
use calling_conventions::ValidatedBoundaryEntryPlan;
use calling_conventions::callback_nominal_report_fingerprint;
use calling_conventions::{CallingPolicy, evaluate_ordinary_boundary_entry_plan};

pub(crate) fn evaluate_default_calling_applications(
    program: &super::CallingProgram,
    declarations: &[semantic_vocabulary::CallingAbiIdentity],
    target: target::TargetProfile,
    policy: CallingPolicy,
    dispatch_only_parameter_count: usize,
) -> Result<Vec<calling_conventions::BoundaryCallingApplication>, String> {
    let mut applications = Vec::with_capacity(declarations.len());
    for declaration in declarations {
        let canonical = program.materialize_default(declaration, target)?;
        let signature = &canonical.policy;
        let geometry = &signature.geometry;
        let dispatch_only_parameter_count = u32::try_from(dispatch_only_parameter_count)
            .map_err(|_| "default dispatch prefix exceeds u32")?;
        let projection = calling_conventions::NativeCallProjection::new(
            signature.native_parameters()?,
            geometry.parameters.len(),
            dispatch_only_parameter_count,
        )?;
        let graph = calling_conventions::BoundaryValueGraph {
            shapes: &geometry.shapes,
            fields: &geometry.fields,
        };
        let mut parameters = Vec::new();
        for parameter in projection.parameters() {
            let shape = match parameter.shape {
                calling_conventions::BoundaryNativeParameterShape::Semantic(root) => {
                    graph.classify(root, policy)?
                }
                calling_conventions::BoundaryNativeParameterShape::TargetFunctionPointer {
                    byte_size,
                    alignment,
                } => calling_conventions::ValueShape::integer(u32::from(byte_size), alignment),
            };
            parameters.push(shape);
        }
        let classified = calling_conventions::CallSignature {
            parameters,
            result: geometry
                .result
                .map(|root| graph.classify(root, policy))
                .transpose()?,
        };
        let ordinary = evaluate_ordinary_boundary_entry_plan(policy, &classified)
            .map_err(|diagnostic| diagnostic.to_string())?;
        // Direct callback slots already declare their binder and native
        // destination. The default ABI must carry that exact mapping alongside
        // its placements; it neither chooses an inbound entry plan nor guesses
        // which binder supplies a private layout field.
        let mut plan = ordinary.plan().clone();
        for parameter in projection.parameters() {
            if let calling_conventions::BoundaryNativeParameterOrigin::PrivateCallback {
                binder,
                ..
            } = parameter.origin
            {
                plan.call.callback_materializations.push(
                    calling_conventions::CallbackMaterialization {
                        binder,
                        destination: calling_conventions::NativePlace::Parameter(
                            parameter.identity,
                        ),
                    },
                );
            }
        }
        let validated =
            calling_conventions::validate_boundary_entry_plan_with_callback_materializations(
                plan,
                &classified,
                &signature.callbacks,
            )
            .map_err(|diagnostic| diagnostic.to_string())?;
        let movement = BoundaryOpaqueMovementGeometry {
            shapes: &geometry.shapes,
            fields: &geometry.fields,
            parameters: &geometry.parameters,
            result: geometry.result,
            native_parameters: signature.native_parameters()?,
        };
        for representation in &canonical.opaque_uses {
            movement.movement(
                representation.shape_root,
                &validated,
                usize::try_from(dispatch_only_parameter_count)
                    .map_err(|_| "default dispatch prefix exceeds usize")?,
            )?;
        }
        applications.push(canonical.native_application(&validated, dispatch_only_parameter_count)?);
    }
    Ok(applications)
}

pub(crate) fn boundary_plan_application_identity(
    signature: &MaterializedBoundarySignature,
    validated: &ValidatedBoundaryEntryPlan,
    calling_owner_application: &str,
) -> Result<(u64, [u8; 32]), String> {
    let application =
        publish_materialized_application(signature, validated, calling_owner_application)?;
    Ok((application.report_fingerprint, application.commitment))
}

fn publish_materialized_application(
    signature: &MaterializedBoundarySignature,
    validated: &ValidatedBoundaryEntryPlan,
    calling_owner_application: &str,
) -> Result<calling_conventions::BoundaryCallingApplication, String> {
    let mut application = super::callback_bindings::source_free_application(
        signature,
        validated,
        calling_owner_application,
    )?;
    application.commitment = application.recomputed_commitment()?;
    application.report_fingerprint = callback_nominal_report_fingerprint(
        b"omega.boundary-plan-application-report.v3",
        &[&application.commitment],
    );
    Ok(application)
}

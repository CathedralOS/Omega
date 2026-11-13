/*
We compute per-target boundary calling plans (`PreparedBoundaryCallingPlans`)
and default calling applications (`evaluate_default_calling_applications`) from
owned original correspondence and canonical Terminal storage.

Target choice and helper selection need no Source owner, catalog, or signature
traversal. Each policy activation runs in its own sponsor and invocation account
over immutable code in parallel across worker threads, with results consumed in
deterministic request order so the first refusal in request order is always the
one reported. Both authored and default policies publish the complete source-free
C shape together with its validated plan so opaque representation graphs survive
into runtime host admission.
*/

use crate::calling_policy_plans::boundary_signatures::BoundaryOpaqueMovementGeometry;
use crate::calling_policy_plans::callback_bindings::validate_retained_callback_binders;
use crate::calling_policy_plans::{BoundaryCallingPlanRealization, MaterializedBoundarySignature};
use calling_conventions::ValidatedBoundaryEntryPlan;
use calling_conventions::callback_nominal_report_fingerprint;
use calling_conventions::{CallingPolicy, evaluate_ordinary_boundary_entry_plan};
use diagnostics::Diagnostic;

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

fn activate_in_parallel<Item: Sync, Output: Send>(
    items: &[Item],
    activate: impl Fn(&Item) -> Output + Sync,
) -> Vec<Output> {
    let workers = std::thread::available_parallelism()
        .map_or(1, std::num::NonZeroUsize::get)
        .min(items.len());
    if workers <= 1 {
        return items.iter().map(&activate).collect();
    }
    let activate = &activate;
    let mut slots: Vec<Option<Output>> =
        std::iter::repeat_with(|| None).take(items.len()).collect();
    std::thread::scope(|scope| {
        let handles: Vec<_> = (1..workers)
            .map(|worker| {
                std::thread::Builder::new()
                    .name(format!("omega-calling-{worker}"))
                    .spawn_scoped(scope, move || {
                        (worker..items.len())
                            .step_by(workers)
                            .map(|index| (index, activate(&items[index])))
                            .collect::<Vec<_>>()
                    })
                    .map_err(|_| worker)
            })
            .collect();
        for index in (0..items.len()).step_by(workers) {
            slots[index] = Some(activate(&items[index]));
        }
        for handle in handles {
            match handle {
                Ok(handle) => {
                    for (index, output) in handle
                        .join()
                        .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
                    {
                        slots[index] = Some(output);
                    }
                }
                Err(worker) => {
                    for index in (worker..items.len()).step_by(workers) {
                        slots[index] = Some(activate(&items[index]));
                    }
                }
            }
        }
    });
    slots
        .into_iter()
        .map(|slot| slot.expect("every policy slot is filled"))
        .collect()
}

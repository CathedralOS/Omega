//! Default native policies consume the same complete Product ABI catalog as
//! authored policies. A declaration may have several closed applications with
//! identical physical shape; each keeps its own semantic key and custody.

use super::CanonicalCallingSignature;
use calling_conventions::{
    BoundaryCallingApplication, BoundaryNativeParameterShape, BoundaryValueGraph, CallSignature,
    CallingPolicy, ValueShape, evaluate_ordinary_boundary_entry_plan,
};
use evaluation_accounting::{
    BuildEvaluationSponsor, BuildEvaluationSponsorLimits, EvaluationMeasurementIdentity,
    TerminalStorageScheduleIdentity,
};
use terminal_codec::CanonicalTerminalArtifact;

pub(crate) fn evaluate_default_calling_applications(
    artifact: &CanonicalTerminalArtifact,
    target: target::TargetProfile,
    requirement: &str,
    policy: CallingPolicy,
    dispatch_only_parameters: usize,
) -> Result<Vec<BoundaryCallingApplication>, String> {
    if requirement.is_empty() {
        return Err("default calling request has no exact requirement identity".into());
    }
    let module = artifact.semantic_module();
    let declarations = module.calling_abi_signatures.iter().filter(|declaration| {
        declaration.default_eligible
            && module.identities.text(declaration.requirement_identity) == Some(requirement)
            && declaration.storage.is_ok()
    });
    let mut applications = Vec::new();
    for declaration in declarations {
        let requested = CanonicalCallingSignature::extent_demands(module, declaration.id, target)?;
        let completed = if requested.is_empty() {
            None
        } else {
            let measurement = EvaluationMeasurementIdentity::TerminalPsi {
                fuel_schedule: terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
                storage_schedule: TerminalStorageScheduleIdentity::CURRENT,
            };
            let limits = BuildEvaluationSponsorLimits::new(
                100_000, 1, 1, 1, 1_000_000, 1_000_000, 1_000_000, 1_000_000,
            )
            .ok_or("invalid default calling scalar limits")?;
            let sponsor = BuildEvaluationSponsor::new_with_measurement(limits, measurement)
                .map_err(|error| error.to_string())?;
            let account = sponsor
                .begin_invocation(measurement, limits.maximum_fuel_units())
                .map_err(|error| error.to_string())?;
            let mut meter = terminal_fuel::TerminalFuelMeter::with_invocation(account, None)
                .map_err(|error| error.to_string())?;
            let providers =
                terminal_installation::admit_provider_installation(artifact.view(), &[], &[])
                    .map_err(|error| format!("default ABI scalar providers: {error:?}"))?;
            Some(
                super::scalar_completion::complete_declaration_scalar_inputs(
                    artifact, target, &requested, &providers, &mut meter,
                )?,
            )
        };
        let canonical = CanonicalCallingSignature::materialize(
            module,
            declaration.id,
            target,
            &|application| {
                let value = completed
                    .as_ref()
                    .and_then(|values| values.values().value(application));
                let integer = match value {
                    Some(terminal_installation::TerminalScalarValue::Integer {
                        value: semantic_vocabulary::IntegerValue::Unsigned(value),
                        ..
                    }) => u64::try_from(value).ok(),
                    Some(terminal_installation::TerminalScalarValue::Integer {
                        value: semantic_vocabulary::IntegerValue::Signed(value),
                        ..
                    }) => u64::try_from(value).ok(),
                    _ => None,
                };
                integer.ok_or(terminal_semantics::StorageGeometryError::UnresolvedExtent(
                    application,
                ))
            },
        )?;
        let classified = compatibility_signature(&canonical, policy, dispatch_only_parameters)?;
        let validated = evaluate_ordinary_boundary_entry_plan(policy, &classified)
            .map_err(|error| error.to_string())?;
        if dispatch_only_parameters == 0 {
            super::policy_signature::validate_calling_policy_result(
                calling_conventions::BoundaryPlanResult::Accepted(validated.plan().clone()),
                &canonical.policy,
            )?;
        } else if !canonical.policy.callbacks.binders.is_empty()
            || !canonical.policy.callbacks.demands.is_empty()
        {
            return Err("dispatch-only default ABI cannot discard callback obligations".into());
        }
        applications.push(canonical.native_application(&validated)?);
    }
    Ok(applications)
}

fn compatibility_signature(
    signature: &CanonicalCallingSignature,
    policy: CallingPolicy,
    dispatch_only_parameters: usize,
) -> Result<CallSignature, String> {
    let parameters = signature.policy.native_parameters()?;
    if dispatch_only_parameters > parameters.len() {
        return Err(
            "default ABI removes more dispatch parameters than its complete native roster".into(),
        );
    }
    let geometry = &signature.policy.geometry;
    let graph = BoundaryValueGraph {
        shapes: &geometry.shapes,
        fields: &geometry.fields,
    };
    Ok(CallSignature {
        parameters: parameters
            .iter()
            .skip(dispatch_only_parameters)
            .map(|parameter| match parameter.shape {
                BoundaryNativeParameterShape::Semantic(root) => graph.classify(root, policy),
                BoundaryNativeParameterShape::TargetFunctionPointer {
                    byte_size,
                    alignment,
                } => Ok(ValueShape::integer(u32::from(byte_size), alignment)),
            })
            .collect::<Result<_, String>>()?,
        result: geometry
            .result
            .map(|root| graph.classify(root, policy))
            .transpose()?,
    })
}

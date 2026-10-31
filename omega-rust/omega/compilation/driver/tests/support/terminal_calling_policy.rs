//! Execute authored calling code after external canonical artifact ingress.

use evaluation_accounting::{
    BuildEvaluationSponsor, BuildEvaluationSponsorLimits, EvaluationMeasurementIdentity,
    TerminalStorageScheduleIdentity,
};
use provider_planning::calling_policy_plans::{
    MaterializedBoundarySignature, TerminalCallingPolicyInvocation,
    evaluate_terminal_calling_policy,
};
use terminal_codec::CanonicalTerminalArtifact;
use terminal_psi::CallingSignatureDeclaration;

pub(super) fn observe(
    artifact: &CanonicalTerminalArtifact,
    target: &str,
    accepted: &str,
    rejected: &str,
) -> Result<(), String> {
    let target =
        target::TargetProfile::from_canonical_target_name(target).map_err(|error| error.message)?;
    let transported = CanonicalTerminalArtifact::from_bytes(&artifact.to_bytes())
        .map_err(|error| error.to_string())?;
    for artifact in [artifact, &transported] {
        let module = artifact.semantic_module();
        // Authored inspection selectors are presentation labels, not compiler
        // semantic matching. The selected declaration supplies every real ID.
        let description = |requirement: &str| {
            let mut matching = module.boundary_machines.iter().filter(|row| {
                module
                    .identities
                    .text(row.identity)
                    .is_some_and(|identity| identity.contains(requirement))
            });
            let declaration = matching.next().ok_or("calling fixture lost its boundary")?;
            if matching.next().is_some() {
                return Err("calling fixture selector is ambiguous".into());
            }
            Ok::<_, String>(CallingSignatureDeclaration {
                boundary: declaration.id,
                requirement_identity: declaration.identity,
                parameters: Vec::new(),
                binders: Vec::new(),
                direct_callbacks: Vec::new(),
            })
        };
        let accepted = description(accepted)?;
        let rejected = description(rejected)?;
        let description = accepted;
        let mut substituted = description.clone();
        substituted.requirement_identity = terminal_psi::TerminalIdentityIndex::ABSENT;
        let error = MaterializedBoundarySignature::from_terminal(module, &substituted, target)
            .err()
            .ok_or("calling signature accepted a substituted requirement identity")?;
        if !error.contains("substitutes another requirement identity") {
            return Err(format!(
                "calling identity control reached an unrelated refusal: {error}"
            ));
        }
        let signature = MaterializedBoundarySignature::from_terminal(module, &description, target)?;
        let measurement = EvaluationMeasurementIdentity::TerminalPsi {
            fuel_schedule: terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
            storage_schedule: TerminalStorageScheduleIdentity::CURRENT,
        };
        let sponsor = BuildEvaluationSponsor::new_with_measurement(
            BuildEvaluationSponsorLimits::new(
                1_000_000, 64, 64, 64, 1_000_000, 1_000_000, 1_000_000, 1_000_000,
            )
            .ok_or("invalid calling fixture limits")?,
            measurement,
        )
        .map_err(|error| error.to_string())?;
        let account = sponsor
            .begin_invocation(measurement, 1_000_000)
            .map_err(|error| error.to_string())?;
        let (plan, usage) = evaluate_terminal_calling_policy(TerminalCallingPolicyInvocation {
            artifact,
            entry: module.entry.ok_or("calling fixture has no policy entry")?,
            target,
            signature: &signature,
            account,
        })?;
        if !plan.plan().call.parameters.is_empty()
            || plan
                .plan()
                .call
                .result
                .as_ref()
                .is_none_or(|result| result.shape.byte_size != 8)
        {
            return Err("calling fixture did not produce its authored scalar ABI".into());
        }
        if usage.total_units() == 0 {
            return Err("calling fixture recorded no execution fuel".into());
        }
        for (control, description, fuel) in [
            ("rejection", &rejected, 1_000_000),
            ("fuel", &description, 1),
            ("target", &description, 1_000_000),
        ] {
            let signature =
                MaterializedBoundarySignature::from_terminal(module, description, target)?;
            let selected_target = if control == "target" {
                target::TargetProfile::from_canonical_target_name("linux_x86_64")
                    .map_err(|error| error.message)?
            } else {
                target
            };
            let account = sponsor
                .begin_invocation(measurement, fuel)
                .map_err(|error| error.to_string())?;
            let error = evaluate_terminal_calling_policy(TerminalCallingPolicyInvocation {
                artifact,
                entry: module.entry.ok_or("calling fixture has no policy entry")?,
                target: selected_target,
                signature: &signature,
                account,
            })
            .err()
            .ok_or_else(|| format!("calling policy accepted {control} control"))?;
            let expected = match control {
                "rejection" => "scalar result required",
                "fuel" => "AllowanceExhausted(Fuel)",
                _ => "target differs",
            };
            if !error.contains(expected) {
                return Err(format!(
                    "{control} control reached an unrelated refusal: {error}"
                ));
            }
        }
        if sponsor.live_storage_cells() != 0 || sponsor.live_payload_bytes() != 0 {
            return Err("calling fixture retained invocation backing after close".into());
        }
    }
    Ok(())
}

//! Execute authored calling code after external canonical artifact ingress.

use calling_conventions::{MachineRegister, ValueLocation};
use evaluation_accounting::{
    BuildEvaluationSponsor, BuildEvaluationSponsorLimits, EvaluationMeasurementIdentity,
    TerminalStorageScheduleIdentity,
};
use provider_planning::calling_policy_plans::{
    MaterializedBoundarySignature, TerminalCallingPolicyInvocation,
    evaluate_terminal_calling_policy,
};
use terminal_codec::CanonicalTerminalArtifact;

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
            Ok::<_, String>(declaration.id)
        };
        let accepted = description(accepted)?;
        let rejected = description(rejected)?;
        let description = accepted;
        let mut substituted = module.clone();
        substituted
            .calling_signatures
            .iter_mut()
            .find(|signature| signature.boundary == description)
            .ok_or("calling fixture lost its published source telescope")?
            .requirement_identity = terminal_psi::TerminalIdentityIndex::ABSENT;
        let encoded =
            terminal_codec::encode_module(&substituted).map_err(|error| error.to_string())?;
        let error = terminal_codec::decode_module(&encoded)
            .err()
            .ok_or("calling signature accepted a substituted requirement identity")?;
        if !error.to_string().contains("requirement identity") {
            return Err(format!(
                "calling identity control reached an unrelated refusal: {error}"
            ));
        }
        let mut substituted = module.clone();
        let telescope = substituted
            .calling_signatures
            .iter_mut()
            .find(|signature| signature.boundary == description)
            .ok_or("calling fixture lost its published source telescope")?
            .telescope
            .as_mut()
            .map_err(|error| error.to_owned())?;
        let [left, right] = telescope.parameters.as_mut_slice() else {
            return Err("calling fixture did not retain its two distinct native formals".into());
        };
        right.identity = left.identity;
        let encoded =
            terminal_codec::encode_module(&substituted).map_err(|error| error.to_string())?;
        let error = terminal_codec::decode_module(&encoded)
            .err()
            .ok_or("calling telescope accepted duplicate native formal identity")?;
        if !error.to_string().contains("native formal identity") {
            return Err(format!(
                "calling formal control reached an unrelated refusal: {error}"
            ));
        }
        let signature = MaterializedBoundarySignature::from_terminal(module, description, target)?;
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
        let expected_locations = [0, 1].map(|index| ValueLocation::Register {
            register: MachineRegister::Aarch64X(index),
            value_byte_offset: 0,
            byte_size: 8,
        });
        if plan.plan().call.parameters.len() != 2
            || plan
                .plan()
                .call
                .parameters
                .iter()
                .zip(&expected_locations)
                .any(|(parameter, expected)| parameter.locations.as_slice() != [*expected])
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
        // Envelope ingress retains obligations, not producer authority. Even
        // otherwise valid calling code must refuse a false module predicate.
        let mut false_module = module.clone();
        let mut nodes = arena::Arena::new();
        let root = nodes.insert(terminal_psi::ScalarPredicateNode(
            numerics::scalar_predicates::PredicateNode::Value(
                numerics::scalar_predicates::ScalarValue::Boolean(false),
            ),
        ));
        let control = "calling policy false condition ingress control";
        false_module
            .scalar_applications
            .conditions
            .retain_predicate(
                terminal_psi::ScalarConditionSubject::Module,
                control.into(),
                nodes,
                root,
            )?;
        let optimization =
            terminal_codec::build_identity_optimization_execution_record(&false_module)
                .map_err(|error| format!("false calling optimization: {error:?}"))?;
        let false_artifact =
            CanonicalTerminalArtifact::from_parts(false_module, optimization, None)
                .map_err(|error| format!("false calling artifact: {error:?}"))?;
        let false_artifact = CanonicalTerminalArtifact::from_bytes(&false_artifact.to_bytes())
            .map_err(|error| format!("false calling transport: {error:?}"))?;
        let signature = MaterializedBoundarySignature::from_terminal(
            false_artifact.semantic_module(),
            description,
            target,
        )?;
        let account = sponsor
            .begin_invocation(measurement, 1_000_000)
            .map_err(|error| error.to_string())?;
        let error = evaluate_terminal_calling_policy(TerminalCallingPolicyInvocation {
            artifact: &false_artifact,
            entry: false_artifact
                .semantic_module()
                .entry
                .ok_or("false calling fixture has no policy entry")?,
            target,
            signature: &signature,
            account,
        })
        .err()
        .ok_or("calling policy accepted a false retained scalar predicate")?;
        if !error.contains("predicate is false") || !error.contains(control) {
            return Err(format!(
                "false calling predicate reached an unrelated refusal: {error}"
            ));
        }
        for (control, description, fuel) in [
            ("rejection", rejected, 1_000_000),
            ("fuel", description, 1),
            ("target", description, 1_000_000),
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

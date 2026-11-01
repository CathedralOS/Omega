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
        let argument = module
            .boundary_machine(description)
            .and_then(|boundary| boundary.structural_parameters.first())
            .and_then(|parameter| module.structural_type(parameter.structural_type))
            .ok_or("reference calling fixture lost its record declaration")?;
        let terminal_psi::StructuralTypeShape::Record { fields, .. } = &argument.shape else {
            return Err("reference calling fixture has no canonical record body".into());
        };
        let [field] = fields.as_slice() else {
            return Err("reference calling fixture must retain its one declared field".into());
        };
        let terminal_psi::StructuralFieldType::Structural(reference) = field.field_type else {
            return Err("reference calling fixture field lost its structural carrier".into());
        };
        let Some(terminal_psi::StructuralTypeShape::Reference { referent, .. }) =
            module.structural_type(reference).map(|row| &row.shape)
        else {
            return Err("reference calling fixture lost its reference edge".into());
        };
        if module.structural_type(*referent).is_some()
            || module.foreign_reference_descriptor(*referent) != Some(false)
        {
            return Err(
                "reference calling fixture did not preserve its identity-only pointer target"
                    .into(),
            );
        }
        let record = signature
            .parameters()
            .first()
            .and_then(|root| signature.shapes().get(usize::from(*root)))
            .ok_or("reference calling fixture lost its record argument")?;
        let calling_conventions::BoundaryValueClass::Record {
            first_field,
            field_count: 1,
        } = record.class
        else {
            return Err("reference calling fixture lost its stored reference field".into());
        };
        let field = signature
            .fields()
            .get(usize::from(first_field))
            .and_then(|field| signature.shapes().get(usize::from(field.shape)))
            .ok_or("reference calling fixture lost its field geometry")?;
        if field.class != calling_conventions::BoundaryValueClass::Reference
            || field.byte_size != 8
            || field.alignment != 8
        {
            return Err("reference calling fixture did not preserve its pointer geometry".into());
        }
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
        let signature = signature.policy_signature()?;
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
            return Err("calling fixture did not produce its authored word-sized ABI".into());
        }
        if usage.total_units() == 0 {
            return Err("calling fixture recorded no execution fuel".into());
        }
        // Change only the detached canonical graph. The authored policy still
        // returns an integer placement, which cannot classify a float input.
        let mut wrong_class = signature.geometry.clone();
        let root = *wrong_class
            .parameters
            .first()
            .ok_or("missing policy formal")?;
        wrong_class.shapes[usize::from(root)].class =
            calling_conventions::BoundaryValueClass::Float;
        let wrong_class = provider_planning::calling_policy_plans::CallingPolicySignature {
            target: signature.target,
            geometry: wrong_class,
            callbacks: signature.callbacks.clone(),
        };
        let account = sponsor
            .begin_invocation(measurement, 1_000_000)
            .map_err(|error| error.to_string())?;
        let error = evaluate_terminal_calling_policy(TerminalCallingPolicyInvocation {
            artifact,
            entry: module.entry.ok_or("calling fixture has no policy entry")?,
            target,
            signature: &wrong_class,
            account,
        })
        .err()
        .ok_or("calling policy accepted an integer placement for a float input")?;
        if !error.contains("parameter 0 classification is invalid") {
            return Err(format!(
                "calling shape control reached an unrelated refusal: {error}"
            ));
        }
        for (control, description, fuel) in [
            ("rejection", rejected, 1_000_000),
            ("fuel", description, 1),
            ("target", description, 1_000_000),
        ] {
            let signature =
                MaterializedBoundarySignature::from_terminal(module, description, target)?
                    .policy_signature()?;
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

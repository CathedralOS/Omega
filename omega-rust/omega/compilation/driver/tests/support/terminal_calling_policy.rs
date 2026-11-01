//! Execute authored calling code after external canonical artifact ingress.

use calling_conventions::{MachineRegister, ValueLocation};
use evaluation_accounting::{
    BuildEvaluationSponsor, BuildEvaluationSponsorLimits, EvaluationMeasurementIdentity,
    TerminalStorageScheduleIdentity,
};
use provider_planning::calling_policy_plans::{
    CallingPolicySignature, CanonicalCallingSignature, TerminalCallingPolicyInvocation,
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
        let declaration_id = *module
            .calling_signatures
            .iter()
            .find(|signature| signature.boundary == description)
            .ok_or("calling fixture lost its exact ABI correspondence")?
            .application
            .as_ref()
            .map_err(Clone::clone)?;
        let mut substituted = module.clone();
        substituted
            .calling_abi_signatures
            .iter_mut()
            .find(|signature| signature.id == declaration_id)
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
            .calling_abi_signatures
            .iter_mut()
            .find(|signature| signature.id == declaration_id)
            .ok_or("calling fixture lost its published source telescope")?
            .storage
            .as_mut()
            .map_err(|error| error.to_owned())?;
        let telescope = &mut telescope.telescope;
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
        for replacement in [
            None,
            Some(semantic_vocabulary::StructuralTypeId::new(u64::MAX).ok_or("invalid control id")?),
        ] {
            let mut substituted = module.clone();
            let storage = substituted
                .calling_abi_signatures
                .iter_mut()
                .find(|signature| signature.id == declaration_id)
                .ok_or("calling fixture lost its declaration ABI")?
                .storage
                .as_mut()
                .map_err(|reason| reason.clone())?;
            storage
                .parameters
                .first_mut()
                .ok_or("calling fixture lost its record formal")?
                .callback_backing = replacement;
            let encoded =
                terminal_codec::encode_module(&substituted).map_err(|error| error.to_string())?;
            let error = terminal_codec::decode_module(&encoded)
                .err()
                .ok_or("calling ABI accepted an omitted or substituted represented backing")?;
            if !error.to_string().contains("callback backing") {
                return Err(format!(
                    "calling backing control reached another refusal: {error}"
                ));
            }
        }
        if !module
            .calling_abi_signatures
            .iter()
            .filter_map(|row| row.storage.as_ref().ok())
            .flat_map(|storage| &storage.parameters)
            .any(|parameter| {
                parameter.access != terminal_psi::StructuralAccess::Owned
                    && parameter.callback_backing.is_none()
                    && module.structural_type(parameter.structural_type).is_none()
                    && module
                        .foreign_reference_descriptor(parameter.structural_type)
                        .is_some()
            })
        {
            return Err(
                "calling fixture lost abstract pointer transport without invented backing".into(),
            );
        }
        let signature = CanonicalCallingSignature::materialize(
            module,
            declaration_id,
            target,
            &unresolved_extent,
        )?
        .policy;
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
        if module
            .structural_referents
            .iter()
            .any(|row| row.id == *referent && row.abstract_opaque)
        {
            return Err(
                "ordinary nominal pointer was marked as an abstract opaque referent".into(),
            );
        }
        let mut substituted = module.clone();
        let abstract_parameter = substituted
            .calling_abi_signatures
            .iter_mut()
            .filter_map(|row| row.storage.as_mut().ok())
            .flat_map(|storage| &mut storage.parameters)
            .find(|parameter| {
                parameter.callback_backing.is_none()
                    && parameter.access != terminal_psi::StructuralAccess::Owned
            })
            .ok_or("calling fixture lost its abstract borrowed formal")?;
        abstract_parameter.structural_type = *referent;
        let encoded =
            terminal_codec::encode_module(&substituted).map_err(|error| error.to_string())?;
        let error = terminal_codec::decode_module(&encoded)
            .err()
            .ok_or("ordinary identity-only pointer claimed abstract calling backing exemption")?;
        if !error.to_string().contains("callback backing") {
            return Err(format!(
                "abstract referent control reached another refusal: {error}"
            ));
        }
        let record = signature
            .geometry
            .parameters
            .first()
            .and_then(|root| signature.geometry.shapes.get(usize::from(*root)))
            .ok_or("reference calling fixture lost its record argument")?;
        let calling_conventions::BoundaryValueClass::Record {
            first_field,
            field_count: 1,
        } = record.class
        else {
            return Err("reference calling fixture lost its stored reference field".into());
        };
        let field = signature
            .geometry
            .fields
            .get(usize::from(first_field))
            .and_then(|field| signature.geometry.shapes.get(usize::from(field.shape)))
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
        let (plan, usage) = evaluate_terminal_calling_policy(TerminalCallingPolicyInvocation {
            artifact,
            entry: module.entry.ok_or("calling fixture has no policy entry")?,
            target,
            signature: &signature,
            account,
            providers: &[],
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
        let false_signature = signature_for_boundary(
            false_artifact.semantic_module(),
            description,
            target,
            &unresolved_extent,
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
            signature: &false_signature,
            account,
            providers: &[],
        })
        .err()
        .ok_or("calling policy accepted a false retained scalar predicate")?;
        if !error.contains("predicate is false") || !error.contains(control) {
            return Err(format!(
                "false calling predicate reached an unrelated refusal: {error}"
            ));
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
            providers: &[],
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
                signature_for_boundary(module, description, target, &unresolved_extent)?;
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
                providers: &[],
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

fn signature_for_boundary(
    module: &terminal_psi::TerminalModule,
    boundary: semantic_vocabulary::BoundaryMachineId,
    target: target::TargetProfile,
    extents: &dyn Fn(
        semantic_vocabulary::ScalarApplicationId,
    ) -> Result<u64, terminal_semantics::StorageGeometryError>,
) -> Result<CallingPolicySignature, String> {
    let declaration = module
        .calling_signatures
        .iter()
        .find(|row| row.boundary == boundary)
        .ok_or("calling fixture lost its declaration ABI association")?;
    Ok(CanonicalCallingSignature::materialize(
        module,
        *declaration.application.as_ref().map_err(Clone::clone)?,
        target,
        extents,
    )?
    .policy)
}

fn unresolved_extent(
    application: semantic_vocabulary::ScalarApplicationId,
) -> Result<u64, terminal_semantics::StorageGeometryError> {
    Err(terminal_semantics::StorageGeometryError::UnresolvedExtent(
        application,
    ))
}

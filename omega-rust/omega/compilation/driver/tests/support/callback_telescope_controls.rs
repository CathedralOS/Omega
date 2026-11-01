//! Replay actual callback placement after dropping source, through both the
//! retained and decoded artifacts. Perturb ABI proposals, not source programs.

use calling_conventions::{BoundaryNativeParameterOrigin, BoundaryNativeParameterShape};
use terminal_codec::CanonicalTerminalArtifact;
use terminal_psi_to_abstract_operations::{
    RetainedTerminalArtifact, settle_native_callback_placements,
};

fn compare_signature(
    module: &terminal_psi::TerminalModule,
    application: &calling_conventions::BoundaryCallingApplication,
    target: target::TargetProfile,
) -> Result<provider_planning::calling_policy_plans::CallingPolicySignature, String> {
    let mut declarations = module.calling_abi_signatures.iter().filter(|declaration| {
        module.identities.text(declaration.requirement_identity)
            == Some(application.requirement_identity.as_str())
    });
    let (Some(declaration), None) = (declarations.next(), declarations.next()) else {
        return Err("callback signature has no unique canonical declaration association".into());
    };
    let signature =
        provider_planning::calling_policy_plans::CanonicalCallingSignature::materialize(
            module,
            declaration.id,
            target,
            &|application| {
                Err(terminal_semantics::StorageGeometryError::UnresolvedExtent(
                    application,
                ))
            },
        )?
        .policy;
    if signature.geometry != application.signature
        || signature.callbacks != application.callback_context
    {
        return Err("canonical callback signature differs from its native application".into());
    }
    Ok(signature)
}

pub(super) fn inspect(
    retained: &RetainedTerminalArtifact,
    target: target::TargetProfile,
    policy_selector: &str,
) -> Result<(), String> {
    let proposal = retained
        .native_realization_proposal()
        .ok_or("callback telescope controls need a native proposal")?;
    let calling = &proposal.boundary_applications().calling;
    let selected = &proposal.providers().selection;
    let loaded = CanonicalTerminalArtifact::from_bytes(&retained.artifact().to_bytes())
        .map_err(|error| format!("callback telescope transport: {error:?}"))?;
    let baseline = settle_native_callback_placements(
        retained.artifact().semantic_module(),
        selected,
        calling,
    )?;
    if baseline.is_empty() {
        return Err("callback telescope controls need a reached foreign callback".into());
    }
    let registrar = calling
        .iter()
        .position(|application| {
            application
                .signature
                .native_parameters
                .span(application.signature.native_parameter_rows)
                .is_some_and(|rows| {
                    rows.iter().any(|row| {
                        matches!(
                            row.origin,
                            BoundaryNativeParameterOrigin::PrivateCallback { .. }
                        )
                    })
                })
        })
        .ok_or("callback telescope controls need a direct callback argument")?;

    for artifact in [retained.artifact(), &loaded] {
        let signature = compare_signature(artifact.semantic_module(), &calling[registrar], target)?;
        execute_policy(
            artifact,
            target,
            policy_selector,
            &signature,
            &calling[registrar],
        )?;
        let settle = |calling: &[_]| {
            settle_native_callback_placements(artifact.semantic_module(), selected, calling)
        };
        if settle(calling)? != baseline {
            return Err("decoded Terminal callback placement changed".into());
        }
        let mut wrong_ordinal = artifact.semantic_module().clone();
        let declaration = wrong_ordinal
            .calling_abi_signatures
            .iter_mut()
            .find(|signature| {
                wrong_ordinal
                    .identities
                    .text(signature.requirement_identity)
                    == Some(calling[registrar].requirement_identity.as_str())
            })
            .ok_or("callback ordinal control lost its declaration")?;
        declaration
            .storage
            .as_mut()
            .map_err(|reason| reason.to_owned())?
            .telescope
            .binders
            .first_mut()
            .ok_or("callback ordinal control lost its binder")?
            .static_machine_ordinal = 99;
        let encoded =
            terminal_codec::encode_module(&wrong_ordinal).map_err(|error| error.to_string())?;
        let error = terminal_codec::decode_module(&encoded)
            .err()
            .ok_or("callback ordinal control accepted a substituted generic position")?;
        if !error.to_string().contains("wrong declared machine ordinal") {
            return Err(format!(
                "callback ordinal control reached an unrelated refusal: {error}"
            ));
        }
        let mut declaration = artifact.semantic_module().clone();
        let signature = declaration
            .calling_abi_signatures
            .iter_mut()
            .find(|signature| {
                declaration.identities.text(signature.requirement_identity)
                    == Some(calling[registrar].requirement_identity.as_str())
            })
            .ok_or("callback telescope control lost its exact declaration")?;
        let telescope = &mut signature
            .storage
            .as_mut()
            .map_err(|reason| reason.to_owned())?
            .telescope;
        let semantic = telescope
            .parameters
            .first()
            .ok_or("mixed telescope has no semantic parameter")?;
        let callback = telescope
            .direct_callbacks
            .first_mut()
            .ok_or("mixed telescope has no direct callback")?;
        callback.native_ordinal = semantic.native_ordinal;
        require_refusal(
            settle_native_callback_placements(&declaration, selected, calling),
            "native parameter telescope has position",
        )?;
        let mut changed = calling.clone();
        let inbound = changed
            .iter_mut()
            .find(|application| {
                application.requirement_identity == baseline[0].canonical_requirement_overload
            })
            .ok_or("callback reference control lost its inbound application")?;
        let shape = inbound
            .signature
            .shapes
            .iter_mut()
            .find(|shape| shape.class == calling_conventions::BoundaryValueClass::Reference)
            .ok_or("callback reference control has no borrowed parameter")?;
        shape.byte_size = 16;
        require_refusal(
            settle(&changed),
            "reference geometry differs from the Terminal transport role",
        )?;
        let mut changed = calling.clone();
        let signature = &mut changed[registrar].signature;
        let binders = signature
            .callback_bindings
            .span_mut(signature.callback_binding_rows)
            .ok_or("callback control lost its binder rows")?;
        let binder = binders
            .first_mut()
            .ok_or("callback control needs a registrar binder")?;
        binder.invokes_synchronously = !binder.invokes_synchronously;
        require_refusal(
            settle(&changed),
            "callback binders differ from the Terminal telescope",
        )?;

        let mut changed = calling.clone();
        let signature = &mut changed[registrar].signature;
        let binders = signature
            .callback_bindings
            .span_mut(signature.callback_binding_rows)
            .ok_or("callback control lost its binder ordinals")?;
        let binder = binders
            .first_mut()
            .ok_or("callback control needs a registrar binder")?;
        binder.static_machine_ordinal += 1;
        require_refusal(
            settle(&changed),
            "callback binders differ from the Terminal telescope",
        )?;

        let mut changed = calling.clone();
        let signature = &mut changed[registrar].signature;
        let binders = signature
            .callback_bindings
            .span_mut(signature.callback_binding_rows)
            .ok_or("callback control lost its requirement identities")?;
        let binder = binders
            .first_mut()
            .ok_or("callback control needs a registrar binder")?;
        binder.requirement_identity.push_str("::substituted");
        require_refusal(
            settle(&changed),
            "callback binders differ from the Terminal telescope",
        )?;

        let mut changed = calling.clone();
        let signature = &mut changed[registrar].signature;
        let rows = signature
            .native_parameters
            .span_mut(signature.native_parameter_rows)
            .ok_or("callback control lost its native rows")?;
        let pointer = rows
            .iter_mut()
            .find(|row| {
                matches!(
                    row.origin,
                    BoundaryNativeParameterOrigin::PrivateCallback { .. }
                )
            })
            .ok_or("callback control lost its direct callback argument")?;
        pointer.shape = BoundaryNativeParameterShape::TargetFunctionPointer {
            byte_size: 4,
            alignment: 4,
        };
        require_refusal(
            settle(&changed),
            "native parameters differ from the Terminal telescope",
        )?;

        let mut changed = calling.clone();
        let signature = &mut changed[registrar].signature;
        let rows = signature
            .native_parameters
            .span_mut(signature.native_parameter_rows)
            .ok_or("callback control lost its native order")?;
        if rows.len() < 2 {
            return Err("callback telescope controls need mixed native arguments".into());
        }
        let first = rows[0].native_ordinal;
        rows[0].native_ordinal = rows[1].native_ordinal;
        rows[1].native_ordinal = first;
        require_refusal(
            settle(&changed),
            "native parameters differ from the Terminal telescope",
        )?;
    }
    Ok(())
}

fn execute_policy(
    artifact: &CanonicalTerminalArtifact,
    target: target::TargetProfile,
    selector: &str,
    signature: &provider_planning::calling_policy_plans::CallingPolicySignature,
    application: &calling_conventions::BoundaryCallingApplication,
) -> Result<(), String> {
    use evaluation_accounting::{
        BuildEvaluationSponsor, BuildEvaluationSponsorLimits, EvaluationMeasurementIdentity,
        TerminalStorageScheduleIdentity,
    };
    use provider_planning::calling_policy_plans::{
        TerminalCallingPolicyInvocation, evaluate_terminal_calling_policy,
    };
    use terminal_psi::TargetMachineCallable;
    let module = artifact.semantic_module();
    // This selector is authored inspection data, not production policy lookup.
    // A family is one logical callable; select its member through the ordinary
    // Terminal target installation instead of matching a target's display name.
    let logical = module
        .target_machine_families
        .iter()
        .map(|family| family.canonical)
        .chain(module.machines.iter().filter_map(|machine| {
            let callable = TargetMachineCallable::Machine(machine.id);
            (!module.target_machine_families.iter().any(|family| {
                family.canonical == callable
                    || family
                        .members
                        .iter()
                        .any(|member| member.callable == callable)
            }))
            .then_some(callable)
        }));
    let identity = |callable| {
        let identity = match callable {
            TargetMachineCallable::Machine(machine) => module
                .machine(machine)
                .and_then(|row| row.declaration.as_ref())
                .map(|row| row.semantic_declaration),
            TargetMachineCallable::Boundary(boundary) => module
                .boundary_machine(boundary)
                .and_then(|row| row.declaration.as_ref())
                .map(|row| row.source.semantic_declaration),
        };
        identity.and_then(|identity| module.identities.text(identity))
    };
    let suffix = format!("::{selector}");
    let mut matching = logical
        .clone()
        .filter(|callable| identity(*callable).is_some_and(|identity| identity.ends_with(&suffix)));
    let (Some(entry), None) = (matching.next(), matching.next()) else {
        let namespace = selector.split("::").next().unwrap_or(selector);
        let related = logical
            .filter_map(identity)
            .filter(|identity| identity.contains(namespace))
            .take(6)
            .collect::<Vec<_>>();
        return Err(format!(
            "callback policy selector `{selector}` has no unique retained logical callable; namespace candidates: {related:?}"
        ));
    };
    let measurement = EvaluationMeasurementIdentity::TerminalPsi {
        fuel_schedule: terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
        storage_schedule: TerminalStorageScheduleIdentity::CURRENT,
    };
    let sponsor = BuildEvaluationSponsor::new_with_measurement(
        BuildEvaluationSponsorLimits::new(
            1_000_000, 64, 64, 64, 1_000_000, 1_000_000, 1_000_000, 1_000_000,
        )
        .ok_or("invalid callback policy inspection limits")?,
        measurement,
    )
    .map_err(|error| error.to_string())?;
    let account = sponsor
        .begin_invocation(measurement, 1_000_000)
        .map_err(|error| error.to_string())?;
    let (plan, fuel) = evaluate_terminal_calling_policy(TerminalCallingPolicyInvocation {
        artifact,
        entry,
        target,
        signature,
        account,
        providers: &[],
    })?;
    if plan != application.plan || fuel.total_units() == 0 {
        return Err(
            "canonical callback input did not execute the actual native calling policy".into(),
        );
    }
    if sponsor.live_storage_cells() != 0 || sponsor.live_payload_bytes() != 0 {
        return Err("callback policy inspection retained invocation storage".into());
    }
    Ok(())
}

fn require_refusal<T>(result: Result<T, String>, expected: &str) -> Result<(), String> {
    match result {
        Err(reason) if reason.contains(expected) => Ok(()),
        Err(reason) => Err(format!(
            "callback telescope control expected {expected}: {reason}"
        )),
        Ok(_) => Err(format!(
            "callback telescope control accepted; expected {expected}"
        )),
    }
}

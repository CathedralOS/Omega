//! A source-produced component lends the embedding's original opaque input to
//! a real child. A host computation pause retains that child and its caller;
//! resumption must neither replay an observation nor retire the lender.

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

use terminal_codec::CanonicalTerminalArtifact;
use terminal_fuel::TerminalFuelMeter;
use terminal_interpreter::{
    TerminalComponentBinding, TerminalEffect, TerminalEffectHandler, TerminalEffectRejection,
    TerminalEffectResult, TerminalExecutionResult, TerminalExecutionStatus, TerminalImportAdapter,
    TerminalImportSignature, TerminalInstantiation, TerminalProgram, TerminalRuntimeSupplies,
    TerminalRuntimeSupplyRequest, TerminalScalarValue, TerminalStructuralInputs,
    TerminalStructuralValue,
};
use terminal_psi::{BoundaryMachineResult, TargetMachineCallable};

pub(super) enum InputExpectation {
    Observations(usize),
    StorageRefusal,
    WriteRefusal,
}

struct Observer {
    signature: TerminalImportSignature,
    seen: Arc<Mutex<Vec<(u64, u64)>>>,
    changes: Arc<Mutex<u64>>,
    ready: Arc<AtomicBool>,
}

impl TerminalImportAdapter for Observer {
    fn implemented_signature(&self) -> TerminalImportSignature {
        self.signature.clone()
    }
}

impl TerminalEffectHandler for Observer {
    fn poll_host_computation(
        &mut self,
        _: &TerminalEffect,
    ) -> Result<bool, TerminalEffectRejection> {
        Ok(self.ready.load(Ordering::Relaxed))
    }

    fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        Err(TerminalEffectRejection::new(
            "result-aware observation required",
        ))
    }

    fn handle_effect_result(
        &mut self,
        effect: &TerminalEffect,
    ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
        let TerminalEffect::BoundaryCall {
            arguments,
            structural_arguments,
            result,
            ..
        } = effect
        else {
            return Err(TerminalEffectRejection::new(
                "structural observation required",
            ));
        };
        if !arguments.is_empty()
            || structural_arguments.len() != 1
            || *result != BoundaryMachineResult::Unit
        {
            return Err(TerminalEffectRejection::new("wrong observation contract"));
        }
        let identity = structural_arguments[0].opaque_identity;
        if identity != 77 {
            return Err(TerminalEffectRejection::new(
                "another host object was substituted",
            ));
        }
        let [parameter] = self.signature.structural_parameters.as_slice() else {
            return Err(TerminalEffectRejection::new(
                "one original host object required",
            ));
        };
        match parameter.access {
            terminal_psi::StructuralAccess::MutableBorrow => *self.changes.lock().unwrap() += 1,
            terminal_psi::StructuralAccess::SharedBorrow => {
                self.seen
                    .lock()
                    .unwrap()
                    .push((identity, *self.changes.lock().unwrap()));
            }
            _ => return Err(TerminalEffectRejection::new("unexpected host access")),
        }
        Ok(TerminalEffectResult::Unit)
    }
}

fn supplies(
    program: &TerminalProgram<'_>,
    selected: Option<&terminal_installation::AdmittedComponentProvider>,
    entry: Option<TargetMachineCallable>,
    imports: &[semantic_vocabulary::BoundaryMachineId],
) -> Result<TerminalRuntimeSupplies, String> {
    program
        .admit_runtime_supplies(TerminalRuntimeSupplyRequest {
            host_imports: imports,
            installation: selected.map(|selected| selected.selection()),
            entry,
            x86_flags: None,
            memory_ordering: None,
            target_machines: None,
            target_layouts: None,
        })
        .map_err(|error| format!("component supplies: {error:?}"))
}

fn select_component(
    artifact: &CanonicalTerminalArtifact,
    requirement: &str,
) -> Result<terminal_installation::AdmittedComponentProvider, String> {
    let view = artifact.view();
    let module = view.module();
    let candidates = module
        .provider_candidates
        .iter()
        .filter(|candidate| {
            module
                .identities
                .text(candidate.source_requirement_identity)
                .is_some_and(|identity| identity.contains(&format!("path\\({requirement}\\)")))
        })
        .collect::<Vec<_>>();
    let [candidate] = candidates.as_slice() else {
        return Err(format!(
            "component input control needs one {requirement} provider"
        ));
    };
    let selection = terminal_installation::ProviderInstallationSelection {
        boundary: candidate.boundary,
        provider_identity: module
            .identities
            .text(candidate.provider_identity)
            .ok_or("provider identity")?
            .into(),
        provider_declaration: module
            .identities
            .text(candidate.provider_declaration.identity())
            .ok_or("provider declaration")?
            .into(),
        candidate: candidate.candidate,
    };
    terminal_installation::admit_component_provider(view, candidate.boundary, view, &selection)
        .map_err(|error| format!("component admission: {error:?}"))
}

fn make_component<'artifact>(
    artifact: &'artifact CanonicalTerminalArtifact,
    requirements: &[&str],
    seen: &Arc<Mutex<Vec<(u64, u64)>>>,
    changes: &Arc<Mutex<u64>>,
    ready: &Arc<AtomicBool>,
) -> Result<
    (
        TerminalComponentBinding<'static, 'artifact>,
        terminal_installation::AdmittedComponentProvider,
    ),
    String,
> {
    let mut nested: Option<(
        TerminalComponentBinding<'static, 'artifact>,
        terminal_installation::AdmittedComponentProvider,
    )> = None;
    for requirement in requirements.iter().rev() {
        let selected = select_component(artifact, requirement)?;
        let provider = TerminalProgram::borrow(artifact.view());
        let entry = Some(TargetMachineCallable::Machine(selected.candidate()));
        let provisional = supplies(&provider, Some(&selected), entry, &[])?;
        let demands = provider
            .execution_requirements(Some(&provisional))
            .map_err(|error| format!("provider requirements: {error:?}"))?;
        let nested_boundary = nested
            .as_ref()
            .map(|(_, admission)| admission.receiving_boundary());
        let imports = demands
            .imports
            .iter()
            .chain(&demands.missing_providers)
            .copied()
            .filter(|boundary| Some(*boundary) != nested_boundary)
            .collect::<Vec<_>>();
        let mut admitted = supplies(&provider, Some(&selected), entry, &imports)?;
        if let Some((_, child)) = &nested {
            admitted = admitted
                .with_component_provider(child)
                .map_err(|error| format!("nested component supply: {error:?}"))?;
        }
        let mut component =
            TerminalComponentBinding::new(selected.clone(), provider, admitted, None)?;
        for boundary in component.unbound_imports().collect::<Vec<_>>() {
            if Some(boundary) != nested_boundary {
                let signature = TerminalImportSignature::from(
                    component.import(boundary).ok_or("child import")?,
                );
                component.bind_import(
                    boundary,
                    Observer {
                        signature,
                        seen: Arc::clone(seen),
                        changes: Arc::clone(changes),
                        ready: Arc::clone(ready),
                    },
                )?;
            }
        }
        if let Some((child, _)) = nested {
            component.bind_component(child)?;
        }
        nested = Some((component, selected));
    }
    nested.ok_or_else(|| "component input control needs an installed requirement".into())
}

pub(super) fn inspect(
    artifact: &CanonicalTerminalArtifact,
    requirement: &str,
    expectation: InputExpectation,
    nested: &[&str],
) -> Result<(), String> {
    let view = artifact.view();
    let module = view.module();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let changes = Arc::new(Mutex::new(0));
    let ready = Arc::new(AtomicBool::new(false));
    let chain = std::iter::once(requirement)
        .chain(nested.iter().copied())
        .collect::<Vec<_>>();
    let (component, selected) = make_component(artifact, &chain, &seen, &changes, &ready)?;
    let component_boundary = selected.receiving_boundary();
    let receiver = TerminalProgram::borrow(view);
    let requirements = receiver
        .execution_requirements(None)
        .map_err(|error| format!("receiver requirements: {error:?}"))?;
    let imports = requirements
        .imports
        .iter()
        .chain(&requirements.missing_providers)
        .copied()
        .filter(|boundary| *boundary != component_boundary)
        .collect::<Vec<_>>();
    let receiver_supplies = supplies(&receiver, None, None, &imports)?;
    let receiver_supplies = receiver
        .defer_component(receiver_supplies, component_boundary)
        .map_err(|error| format!("component reservation: {error:?}"))?;
    let observer = |signature| Observer {
        signature,
        seen: Arc::clone(&seen),
        changes: Arc::clone(&changes),
        ready: Arc::clone(&ready),
    };
    let entry = receiver
        .entry_installed(&receiver_supplies)
        .map_err(|error| format!("receiver entry: {error:?}"))?;
    let [parameter] = entry.structural_parameters else {
        return Err("component input control requires one original opaque input".into());
    };
    let required = module
        .boundary_machines
        .iter()
        .find(|boundary| boundary.id == component_boundary)
        .and_then(|boundary| boundary.structural_parameters.first())
        .ok_or("component input parameter")?;
    let expected_changes =
        u64::from(required.access == terminal_psi::StructuralAccess::MutableBorrow);
    // This is the public embedding's real opaque input. Guest-created storage
    // with the same structural type must never inherit this token's backing.
    let input = [TerminalStructuralValue::minted(
        77,
        parameter.structural_type,
        Vec::new(),
    )];
    let mut instance = receiver
        .instantiate(TerminalInstantiation {
            installation: Some(&receiver_supplies),
            structural_inputs: TerminalStructuralInputs {
                arguments: &input,
                ..Default::default()
            },
            ..Default::default()
        })
        .map_err(|error| format!("receiver instantiation: {error:?}"))?;
    for boundary in instance.unbound_imports().collect::<Vec<_>>() {
        if boundary != component_boundary {
            let signature =
                TerminalImportSignature::from(instance.import(boundary).ok_or("receiver import")?);
            instance
                .bind_import(boundary, observer(signature))
                .map_err(|error| format!("receiver binding: {error:?}"))?;
        }
    }
    instance
        .bind_component(component)
        .map_err(|error| format!("component binding: {error:?}"))?;
    let mut meter = TerminalFuelMeter::unbounded();
    let first = instance.invoke(&mut meter);
    let expected_observations = match expectation {
        InputExpectation::Observations(count) => count,
        InputExpectation::StorageRefusal | InputExpectation::WriteRefusal => {
            let write = matches!(expectation, InputExpectation::WriteRefusal);
            let error = first
                .err()
                .ok_or("guest storage impersonated an opaque host input")?;
            let required_reason = if write {
                "opaque component loan lacks original guest storage for mutation"
            } else {
                "actual unchanged opaque host storage"
            };
            if !error.to_string().contains(required_reason)
                || !seen.lock().unwrap().is_empty()
                || *changes.lock().unwrap() != 0
                || instance.active_component_invocation_count() != usize::from(write)
            {
                return Err(format!(
                    "guest storage must refuse before child effects: {error:?}"
                ));
            }
            if instance.invoke(&mut meter) != Err(error) {
                return Err("refused component invocation changed or resumed its lender".into());
            }
            return Ok(());
        }
    };
    let paused = first.map_err(|error| format!("component initial invocation: {error:?}"))?;
    if !matches!(paused, TerminalExecutionStatus::HostComputationPending(_))
        || !seen.lock().unwrap().is_empty()
        || *changes.lock().unwrap() != 0
        || instance.active_component_invocation_count() != 1
        || instance
            .invoke(&mut meter)
            .map_err(|error| format!("component pending resume: {error:?}"))?
            != paused
        || !seen.lock().unwrap().is_empty()
    {
        return Err("component pause lost custody or repeated an effect".into());
    }
    ready.store(true, Ordering::Relaxed);
    let completed = TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(
        TerminalScalarValue::Integer {
            scalar_type: semantic_vocabulary::IntegerType::new(
                semantic_vocabulary::IntegerSign::Unsigned,
                64,
            )
            .map_err(|error| format!("u64 scalar: {error:?}"))?,
            value: semantic_vocabulary::IntegerValue::Unsigned(73),
        },
    ));
    let actual = instance
        .invoke(&mut meter)
        .map_err(|error| format!("component completion: {error:?}"))?;
    if actual != completed
        || *seen.lock().unwrap() != vec![(77, expected_changes); expected_observations]
        || *changes.lock().unwrap() != expected_changes
        || instance.active_component_invocation_count() != 0
        || instance
            .invoke(&mut meter)
            .map_err(|error| format!("completed invocation: {error:?}"))?
            != completed
        || *seen.lock().unwrap() != vec![(77, expected_changes); expected_observations]
        || *changes.lock().unwrap() != expected_changes
    {
        return Err(format!(
            "component did not restore its original lender exactly once: {actual:?}"
        ));
    }
    instance
        .close()
        .map_err(|error| format!("component caller close: {error:?}"))?;
    Ok(())
}

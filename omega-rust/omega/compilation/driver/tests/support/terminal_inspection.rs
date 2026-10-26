//! Authored machine selections exercise the public inspection producer and
//! the canonical artifact's external ingress, independently of native entry
//! selection. Source fixtures, not compiler-internal constructors, supply
//! the semantic module and its debug companion.

use std::path::Path;

use compiler::CheckedCompileRequest;
use diagnostics::Diagnostic;
use terminal_codec::{CanonicalTerminalArtifact, CanonicalTerminalArtifactError, DebugMapError};

use crate::fixture_package_inputs::{
    fixture_build_snapshot, reviewed_repository_fixture_package_inputs,
};

#[path = "closed_reach_controls.rs"]
mod closed_reach_controls;
#[path = "terminal_host_provisions.rs"]
mod host_provisions;
#[path = "indexed_family_controls.rs"]
mod indexed_family_controls;
#[path = "mathematical_actuals.rs"]
mod mathematical_actuals;

pub(super) const ORACLE: &str = "expected_terminal_inspections.txt";

#[derive(Clone, Copy)]
enum InspectionControls {
    MethodApplications,
    BoundaryCallables,
    IndexedFamily,
    MathematicalActuals,
    ClosedReach,
    ClosedReachCaptures,
    FoldedLayouts,
    FoldedStorage,
}

#[derive(Clone, Copy)]
enum RuntimeHost {
    LinuxX86_64,
    Portable,
}

pub(super) fn inspect(root: &Path, fixture: &Path) -> Result<Vec<String>, Vec<Diagnostic>> {
    inspect_machines(root, fixture).map_err(|message| vec![Diagnostic::error(message)])
}

fn inspect_machines(root: &Path, fixture: &Path) -> Result<Vec<String>, String> {
    let host_provisions = match std::env::var("OMEGA_CORPUS_HOST_PROVISIONS") {
        Ok(value) if value == "1" => true,
        Ok(value) if value == "0" => false,
        Err(std::env::VarError::NotPresent) => false,
        _ => return Err("OMEGA_CORPUS_HOST_PROVISIONS requires 0 or 1".to_owned()),
    };
    let mut observations = Vec::new();
    let selections = std::fs::read_to_string(fixture.join(ORACLE))
        .map_err(|error| format!("cannot read {ORACLE}: {error}"))?;
    let selections = selections
        .lines()
        .map(str::trim)
        .filter(|name| !name.is_empty());
    let mut inspected = false;
    for selection in selections {
        let words = selection.split_whitespace().collect::<Vec<_>>();
        let (machine, runtime_words) = match words.as_slice() {
            [machine, "runtime", host, provisions @ ..] if !provisions.is_empty() => {
                let host = match *host {
                    "linux_x86_64" => RuntimeHost::LinuxX86_64,
                    "portable" => RuntimeHost::Portable,
                    _ => return Err(format!("unknown Terminal provision host {host}")),
                };
                (*machine, Some((host, provisions)))
            }
            [machine, ..] => (*machine, None),
            _ => return Err(format!("{ORACLE} requires a machine")),
        };
        let inspection_words = if runtime_words.is_some() {
            vec![machine]
        } else {
            words.clone()
        };
        let (machine, requires_debug, conformance_controls) = match inspection_words.as_slice() {
            [machine] => (*machine, false, None),
            [machine, "debug"] => (*machine, true, None),
            [machine, "closed-conformance-controls"] => (
                *machine,
                false,
                Some(InspectionControls::MethodApplications),
            ),
            [machine, "closed-reach-capture-controls"] => (
                *machine,
                false,
                Some(InspectionControls::ClosedReachCaptures),
            ),
            [machine, "closed-reach-controls"] => {
                (*machine, false, Some(InspectionControls::ClosedReach))
            }
            [machine, "folded-storage-controls"] => {
                (*machine, false, Some(InspectionControls::FoldedStorage))
            }
            [machine, "folded-layout-controls"] => {
                (*machine, false, Some(InspectionControls::FoldedLayouts))
            }
            [machine, "boundary-conformance-controls"] => {
                (*machine, false, Some(InspectionControls::BoundaryCallables))
            }
            [machine, "mathematical-actuals"] => (
                *machine,
                false,
                Some(InspectionControls::MathematicalActuals),
            ),
            [machine, "indexed-family-controls"] => {
                (*machine, false, Some(InspectionControls::IndexedFamily))
            }
            _ => {
                return Err(format!(
                    "{ORACLE} needs a machine name and optional `debug`, `closed-conformance-controls`, `closed-reach-controls`, `closed-reach-capture-controls`, `boundary-conformance-controls`, `indexed-family-controls` or `mathematical-actuals`"
                ));
            }
        };
        inspected = true;
        let prepared = reviewed_repository_fixture_package_inputs(root, None)
            .map_err(|errors| format!("inspection {machine} package inputs: {errors:?}"))?;
        let snapshot = fixture_build_snapshot(&prepared.root_path)
            .map_err(|errors| format!("inspection {machine} build inputs: {errors:?}"))?;
        let mut request = CheckedCompileRequest::new(&prepared.root_path, None);
        request.package_inputs = prepared.package_inputs;
        request.build.snapshot = snapshot;
        if matches!(
            conformance_controls,
            Some(InspectionControls::MathematicalActuals)
        ) {
            let _artifact = mathematical_actuals::produce(request, machine)?;
            continue;
        }
        if matches!(
            conformance_controls,
            Some(InspectionControls::IndexedFamily)
        ) {
            let _artifact = indexed_family_controls::produce(request, machine)?;
            continue;
        }
        let produced = compiler::CompilationSession::default()
            .produce(
                request,
                compiler::CompilationProduct::Inspection(machine.to_owned()),
            )
            .and_then(compiler::CompilationOutput::into_inspection)
            .map_err(compiler::CompilationProductError::into_diagnostics)
            .map_err(|errors| format!("inspection {machine} production: {errors:?}"))?;
        let artifact = produced.artifact().map_err(|error| error.to_string())?;
        let mut transported = artifact.to_bytes();
        let decoded = CanonicalTerminalArtifact::from_bytes(&transported)
            .map_err(|error| format!("inspection {machine} transport: {error:?}"))?;
        inspect_cleanup_obligations(&decoded)?;
        if decoded.semantic_bytes() != artifact.semantic_bytes()
            || decoded.manifest() != artifact.manifest()
        {
            return Err(format!("inspection {machine} changed during transport"));
        }
        if let Some((host, words)) = runtime_words {
            let fingerprint = artifact
                .manifest()
                .semantic()
                .program_fingerprint
                .as_bytes()
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>();
            if let Some(capture) = crate::artifact_capture::Capture::from_environment(
                "inspection",
                &fingerprint,
                "portable",
            )? {
                capture.observe("terminal.psiart", &transported)?;
            }
            if !host_provisions {
                observations.push(format!("terminal-runtime.{machine}:unrequested"));
            } else {
                match host {
                    RuntimeHost::Portable => {
                        host_provisions::observe_portable(
                            &decoded,
                            produced.build_bindings(),
                            produced.build_requests(),
                            words,
                        )
                        .map_err(|error| format!("inspection {machine} runtime: {error}"))?;
                        observations.push(format!("terminal-runtime.{machine}:match"));
                    }
                    RuntimeHost::LinuxX86_64
                        if cfg!(all(target_os = "linux", target_arch = "x86_64")) =>
                    {
                        host_provisions::observe(&decoded, words)
                            .map_err(|error| format!("inspection {machine} runtime: {error}"))?;
                        observations.push(format!("terminal-runtime.{machine}:match"));
                    }
                    RuntimeHost::LinuxX86_64 => observations.push(format!(
                        "terminal-runtime.{machine}:unavailable-linux_x86_64"
                    )),
                }
            }
        }
        if let Some(controls) = conformance_controls {
            if let Some(directory) = std::env::var_os("OMEGA_CORPUS_ARTIFACT_DIR") {
                let directory = std::path::PathBuf::from(directory);
                std::fs::create_dir_all(&directory)
                    .map_err(|error| format!("cannot preserve canonical artifact: {error}"))?;
                let fingerprint = artifact
                    .manifest()
                    .semantic()
                    .program_fingerprint
                    .as_bytes()
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<String>();
                std::fs::write(
                    directory.join(format!("{fingerprint}.psiart")),
                    &transported,
                )
                .map_err(|error| format!("cannot preserve canonical artifact: {error}"))?;
            }
            match controls {
                InspectionControls::MethodApplications => closed_conformance_controls(&decoded)?,
                InspectionControls::ClosedReach => closed_reach_controls::inspect(&decoded)?,
                InspectionControls::ClosedReachCaptures => {
                    closed_reach_controls::inspect_captures(&decoded)?
                }
                InspectionControls::FoldedLayouts => folded_layout_controls(&decoded)?,
                InspectionControls::FoldedStorage => folded_storage_controls(&decoded)?,
                InspectionControls::BoundaryCallables => boundary_conformance_controls(&decoded)?,
                InspectionControls::IndexedFamily | InspectionControls::MathematicalActuals => {
                    unreachable!("handled before ordinary inspection")
                }
            }
        }
        let Some(debug) = artifact.debug_bytes() else {
            if requires_debug {
                return Err(format!(
                    "inspection {machine} omitted its expected debug companion"
                ));
            }
            continue;
        };
        // Replace one bit of the embedded program identity, leaving the
        // envelope and semantic section intact. External ingress must
        // reject a debug map belonging to a different semantic product.
        let fingerprint = artifact.manifest().semantic().program_fingerprint;
        let identity_position = debug
            .windows(fingerprint.as_bytes().len())
            .position(|bytes| bytes == fingerprint.as_bytes())
            .ok_or_else(|| format!("inspection {machine} debug map has no program identity"))?;
        let debug_start = transported.len() - debug.len();
        transported[debug_start + identity_position] ^= 1;
        if !matches!(
            CanonicalTerminalArtifact::from_bytes(&transported),
            Err(CanonicalTerminalArtifactError::Debug(
                DebugMapError::SemanticIdentityMismatch { .. }
            ))
        ) {
            return Err(format!(
                "inspection {machine} did not reject a changed debug identity"
            ));
        }
    }
    if !inspected {
        return Err(format!("{ORACLE} needs at least one machine selection"));
    }
    Ok(observations)
}

/// A source-produced policy condition must survive transport on its result,
/// independently of the erased policy call and the caller's entry contract.
fn folded_layout_controls(artifact: &CanonicalTerminalArtifact) -> Result<(), String> {
    let target = "macos_arm64";
    let physical = Some(semantic_vocabulary::PhysicalDataLayout {
        address_byte_size: 8,
        address_byte_alignment: 8,
    });
    let selections =
        terminal_installation::admit_target_machine_selections(artifact.view(), target);
    terminal_installation::admit_target_layouts(artifact.view(), &selections, physical)
        .map_err(|reason| format!("original folded layout inputs: {reason}"))?;
    let ordinary = artifact
        .semantic_module()
        .structural_types
        .iter()
        .find(|declaration| {
            matches!(
                declaration.shape,
                terminal_psi::StructuralTypeShape::Record {
                    storage: terminal_psi::StructuralRecordStorage::Packed,
                    ..
                }
            )
        })
        .ok_or("folded layout owner control needs an ordinary source record")?
        .id;
    let mut reassociated = artifact.semantic_module().clone();
    reassociated
        .folded_constants
        .layout_requirements
        .iter_mut()
        .find(|row| matches!(row.owner, terminal_psi::FoldedLayoutOwner::Carrier(_)))
        .ok_or("policy result lost its folded layout requirement")?
        .owner = terminal_psi::FoldedLayoutOwner::Carrier(ordinary);
    reassociated.folded_constants.layout_requirements.sort();
    match terminal_codec::encode_module(&reassociated)
        .and_then(|bytes| terminal_codec::decode_module(&bytes))
    {
        Err(reason)
            if reason
                .to_string()
                .contains("folded layout carrier owner has no target layout application") => {}
        Err(reason) => {
            return Err(format!(
                "reassociated layout owner failed for an unrelated reason: {reason}"
            ));
        }
        Ok(_) => {
            return Err(
                "canonical ingress accepted a layout condition without an enforcing owner"
                    .to_owned(),
            );
        }
    }
    let mut changed = artifact.semantic_module().clone();
    let requirement = changed
        .folded_constants
        .layout_requirements
        .iter_mut()
        .find(|requirement| {
            matches!(
                requirement.owner,
                terminal_psi::FoldedLayoutOwner::Carrier(_)
            ) && requirement
                .alternatives
                .iter()
                .any(|alternative| alternative.target == target)
        })
        .ok_or("policy result lost its qualified folded layout requirement")?;
    for alternative in requirement
        .alternatives
        .iter_mut()
        .filter(|alternative| alternative.target == target)
    {
        let observation = alternative
            .observations
            .first_mut()
            .ok_or("folded layout alternative lost its observation inputs")?;
        observation.value = observation
            .value
            .checked_add(1)
            .ok_or("test observation overflows")?;
    }
    requirement.alternatives.sort();
    changed.folded_constants.layout_requirements.sort();
    let execution = terminal_codec::build_identity_optimization_execution_record(&changed)
        .map_err(|reason| format!("changed folded layout identity stage: {reason:?}"))?;
    let changed = CanonicalTerminalArtifact::from_parts(changed, execution, None)
        .map_err(|reason| format!("changed folded layout encoding: {reason:?}"))?;
    let received = CanonicalTerminalArtifact::from_bytes(&changed.to_bytes())
        .map_err(|reason| format!("changed folded layout transport: {reason:?}"))?;
    let selections =
        terminal_installation::admit_target_machine_selections(received.view(), target);
    match terminal_installation::admit_target_layouts(received.view(), &selections, physical) {
        Err(terminal_installation::TargetLayoutSelectionError::FoldedLayoutRequirement {
            ..
        }) => Ok(()),
        Err(reason) => Err(format!(
            "changed policy condition failed for an unrelated reason: {reason}"
        )),
        Ok(_) => Err("receiving layout admission ignored a changed policy condition".to_owned()),
    }
}

/// The indexed-array source supplies a real transported storage condition.
/// Mutate that condition independently of the surviving machine/policy rows:
/// neither catalog presence nor a sibling condition can satisfy the receiver.
fn folded_storage_controls(artifact: &CanonicalTerminalArtifact) -> Result<(), String> {
    let module = artifact.semantic_module();
    let position = module
        .folded_constants
        .layout_requirements
        .iter()
        .position(|row| matches!(row.owner, terminal_psi::FoldedLayoutOwner::Storage { .. }))
        .ok_or("array result lost its folded storage requirement")?;
    let terminal_psi::FoldedLayoutOwner::Storage { machine, carrier } =
        module.folded_constants.layout_requirements[position].owner
    else {
        unreachable!()
    };
    let target = "macos_arm64";
    let physical = Some(semantic_vocabulary::PhysicalDataLayout {
        address_byte_size: 8,
        address_byte_alignment: 8,
    });
    let receive = |artifact: &CanonicalTerminalArtifact| {
        let selections =
            terminal_installation::admit_target_machine_selections(artifact.view(), target);
        let layouts =
            terminal_installation::admit_target_layouts(artifact.view(), &selections, physical)
                .map_err(|reason| format!("storage layout inputs: {reason}"))?;
        let geometry = layouts
            .geometry(artifact.view())
            .map_err(|reason| format!("storage geometry: {reason}"))?;
        terminal_semantics::admit_folded_layout_requirements(
            &artifact
                .semantic_module()
                .folded_constants
                .layout_requirements,
            geometry.declarations(),
            physical,
            target,
            terminal_psi::FoldedLayoutOwner::Machine(machine),
        )
        .map_err(|reason| reason.to_string())
    };
    receive(artifact).map_err(|reason| format!("original folded storage inputs: {reason}"))?;
    let unrelated = module
        .machines
        .iter()
        .find(|candidate| {
            candidate.id != machine && !candidate.uses_structural_type(module, carrier)
        })
        .ok_or("storage owner control needs a machine without this array")?
        .id;
    let other_carrier = module
        .structural_types
        .iter()
        .find(|declaration| {
            !matches!(
                declaration.shape,
                terminal_psi::StructuralTypeShape::FixedArray { .. }
            )
        })
        .ok_or("storage owner control needs a non-array source type")?
        .id;
    for owner in [
        terminal_psi::FoldedLayoutOwner::Storage {
            machine: unrelated,
            carrier,
        },
        terminal_psi::FoldedLayoutOwner::Storage {
            machine,
            carrier: other_carrier,
        },
    ] {
        let mut reassociated = module.clone();
        reassociated.folded_constants.layout_requirements[position].owner = owner;
        reassociated.folded_constants.layout_requirements.sort();
        match terminal_codec::encode_module(&reassociated)
            .and_then(|bytes| terminal_codec::decode_module(&bytes))
        {
            Err(reason)
                if reason
                    .to_string()
                    .contains("folded storage requirement has no exact consumed array carrier") => {
            }
            Err(reason) => {
                return Err(format!(
                    "reassociated storage owner failed for an unrelated reason: {reason}"
                ));
            }
            Ok(_) => {
                return Err(
                    "canonical ingress accepted storage outside its consuming machine".to_owned(),
                );
            }
        }
    }
    let mut changed = module.clone();
    let requirement = &mut changed.folded_constants.layout_requirements[position];
    for alternative in requirement
        .alternatives
        .iter_mut()
        .filter(|row| row.target == target)
    {
        let observation = alternative
            .observations
            .first_mut()
            .ok_or("storage condition lost its observation")?;
        observation.value = observation
            .value
            .checked_add(1)
            .ok_or("test observation overflows")?;
    }
    requirement.alternatives.sort();
    changed.folded_constants.layout_requirements.sort();
    let execution = terminal_codec::build_identity_optimization_execution_record(&changed)
        .map_err(|reason| format!("changed storage identity stage: {reason:?}"))?;
    let changed = CanonicalTerminalArtifact::from_parts(changed, execution, None)
        .map_err(|reason| format!("changed storage encoding: {reason:?}"))?;
    let received = CanonicalTerminalArtifact::from_bytes(&changed.to_bytes())
        .map_err(|reason| format!("changed storage transport: {reason:?}"))?;
    match receive(&received) {
        Err(reason) if reason.contains("has no matching joint assignment") => Ok(()),
        Err(reason) => Err(format!(
            "changed storage condition failed for an unrelated reason: {reason}"
        )),
        Ok(_) => Err("receiving machine admission ignored a changed storage condition".to_owned()),
    }
}

/// Source-produced cleanup invocations retain one positional obligation for
/// each selected hook prerequisite. Runtime agreement alone cannot detect a
/// lost proof slot: the hook can still execute with the expected output.
fn inspect_cleanup_obligations(artifact: &CanonicalTerminalArtifact) -> Result<(), String> {
    let module = artifact.semantic_module();
    for machine in &module.machines {
        for block in &machine.blocks {
            for (_, actions) in block.terminator.cleanup_rosters() {
                for action in actions {
                    let terminal_psi::TerminalAffineCleanupAction::InvokeNominal(cleanup) = action
                    else {
                        continue;
                    };
                    let target = module.machine(cleanup.cleanup_machine).ok_or_else(|| {
                        format!(
                            "inspection cleanup target {:?} is absent",
                            cleanup.cleanup_machine
                        )
                    })?;
                    if cleanup.requirement_obligations.len() != target.contract.requires.len() {
                        return Err(format!(
                            "inspection cleanup {:?} in machine {:?}, block {:?} has {} obligation slots for {} prerequisites",
                            cleanup.cleanup_machine,
                            machine.id,
                            block.id,
                            cleanup.requirement_obligations.len(),
                            target.contract.requires.len(),
                        ));
                    }
                }
            }
        }
    }
    Ok(())
}

/// Mutate actual Source-produced payloads at the public artifact ingress seam.
/// These are fixture-authored invalid exchange controls, not fabricated compiler
/// trees or logical Source proofs. Ordinary producer trust remains explicit.
fn boundary_conformance_controls(artifact: &CanonicalTerminalArtifact) -> Result<(), String> {
    use terminal_psi::TargetMachineCallable;
    let module = artifact.semantic_module();
    let (position, row, boundary) = module
        .closed_conformance_applications
        .iter()
        .enumerate()
        .find_map(|(position, application)| {
            application
                .static_applications
                .iter()
                .enumerate()
                .find_map(|(row, applied)| {
                    if let TargetMachineCallable::Boundary(boundary) = applied.callable {
                        Some((position, row, boundary))
                    } else {
                        None
                    }
                })
        })
        .ok_or("artifact control fixture needs a static boundary callable")?;
    let original = module
        .boundary_machine(boundary)
        .ok_or("static boundary is absent")?;
    let declaration = original
        .declaration
        .as_ref()
        .ok_or("static boundary has no source declaration")?;
    let other = module
        .boundary_machines
        .iter()
        .find(|candidate| {
            candidate.id != boundary
                && candidate.result == original.result
                && candidate.owner != original.owner
        })
        .ok_or("retarget control needs another same-result boundary")?;
    let mut controls = Vec::new();
    let mut bad = module.clone();
    let replacement = bad
        .boundary_machines
        .iter_mut()
        .find(|candidate| candidate.id == other.id)
        .ok_or("replacement boundary is absent")?;
    replacement.declaration = Some(declaration.clone());
    let application = &mut bad.closed_conformance_applications[position];
    application.static_applications[row].callable = TargetMachineCallable::Boundary(other.id);
    for callable in &mut application.realization_callables {
        if callable.callable == TargetMachineCallable::Boundary(boundary) {
            callable.callable = TargetMachineCallable::Boundary(other.id);
        }
    }
    refresh_conformance_commitment(&mut bad, position);
    controls.push((
        "retargeted boundary with copied declaration",
        "exact machine owner",
        bad,
    ));

    let mut bad = module.clone();
    bad.boundary_machines
        .iter_mut()
        .find(|candidate| candidate.id == boundary)
        .ok_or("boundary is absent")?
        .role = terminal_psi::BoundaryDeclarationRole::ContractEvidence;
    controls.push((
        "contract evidence as runtime callable",
        "contract evidence",
        bad,
    ));

    let mut bad = module.clone();
    bad.boundary_machines
        .iter_mut()
        .find(|candidate| candidate.id == boundary)
        .ok_or("boundary is absent")?
        .declaration = None;
    controls.push((
        "missing boundary source declaration",
        "checked declaration",
        bad,
    ));

    let mut bad = module.clone();
    bad.boundary_machines
        .iter_mut()
        .find(|candidate| candidate.id == boundary)
        .and_then(|candidate| candidate.declaration.as_mut())
        .ok_or("boundary declaration is absent")?
        .source
        .contract = [7; 32];
    controls.push(("wrong family contract", "family contract", bad));

    let mut bad = module.clone();
    bad.boundary_machines
        .iter_mut()
        .find(|candidate| candidate.id == boundary)
        .and_then(|candidate| candidate.declaration.as_mut())
        .ok_or("boundary declaration is absent")?
        .source
        .state =
        terminal_psi::NominalDeclarationIdentity::new(terminal_psi::TerminalIdentityIndex::ABSENT);
    controls.push((
        "invalid boundary declaration identity",
        "invalid or empty identity",
        bad,
    ));

    let mut bad = module.clone();
    let unexpected = declaration.source.callable;
    bad.boundary_machines
        .iter_mut()
        .find(|candidate| candidate.id == boundary)
        .and_then(|candidate| candidate.declaration.as_mut())
        .ok_or("boundary declaration is absent")?
        .method_arguments
        .push(terminal_psi::ClosedReachArgument::Const(unexpected));
    controls.push((
        "extra boundary invocation argument",
        if declaration.source.specialization.is_some() {
            "retained boundary invocation"
        } else {
            "unspecialized boundary carries method arguments"
        },
        bad,
    ));

    for (name, reason, invalid) in controls {
        reject_module_at_ingress(&invalid, name, reason)?;
    }
    Ok(())
}

fn closed_conformance_controls(artifact: &CanonicalTerminalArtifact) -> Result<(), String> {
    let module = artifact.semantic_module();
    let position = module
        .closed_conformance_applications
        .iter()
        .position(|application| {
            application
                .static_applications
                .iter()
                .any(|row| !row.method_arguments.is_empty())
        })
        .ok_or("artifact control fixture needs a generic exact static application")?;
    let application = &module.closed_conformance_applications[position];
    let row = application
        .static_applications
        .iter()
        .position(|row| !row.method_arguments.is_empty())
        .ok_or("missing generic application")?;
    let mut controls = Vec::new();
    let mut bad = module.clone();
    bad.closed_conformance_applications[position]
        .schemas
        .push(application.schemas[0].clone());
    controls.push(("duplicate original schema", "canonical", bad));
    let mut bad = module.clone();
    bad.closed_conformance_applications[position]
        .schemas
        .clear();
    controls.push(("missing original schema", "schema", bad));
    let mut bad = module.clone();
    bad.closed_conformance_applications[position]
        .static_applications
        .push(application.static_applications[row].clone());
    controls.push(("duplicate exact application", "canonical", bad));
    let other = module
        .closed_conformance_applications
        .iter()
        .flat_map(|application| &application.static_applications)
        .find(|other| {
            other.public_requirement_identity
                == application.static_applications[row].public_requirement_identity
                && other.realization_identity
                    == application.static_applications[row].realization_identity
                && other.method_arguments != application.static_applications[row].method_arguments
        })
        .ok_or("wrong-tuple control needs distinct actual member applications")?;
    let mut bad = module.clone();
    bad.closed_conformance_applications[position].static_applications[row].method_arguments =
        other.method_arguments.clone();
    refresh_conformance_commitment(&mut bad, position);
    controls.push((
        "wrong member invocation tuple",
        "retained executable invocation",
        bad,
    ));
    let mut bad = module.clone();
    bad.closed_conformance_applications[position].static_applications[row]
        .method_arguments
        .clear();
    controls.push(("missing member invocation tuple", "telescope", bad));
    let mut bad = module.clone();
    bad.closed_conformance_applications[position].static_applications[row].application_commitment =
        None;
    controls.push(("missing executable commitment", "commitment", bad));
    let mut bad = module.clone();
    bad.closed_conformance_applications[position].schemas[0]
        .definition
        .push(0);
    controls.push(("trailing original definition", "trailing", bad));
    let callable = application
        .realization_callables
        .iter()
        .find(|callable| {
            callable.source_callable_identity
                == application.static_applications[row].source_callable_identity
        })
        .ok_or("exact application lost its callable registry")?;
    let mut bad = module.clone();
    bad.closed_conformance_applications[position]
        .realization_callables
        .push(callable.clone());
    controls.push(("duplicate callable registry", "registry", bad));
    let mut bad = module.clone();
    bad.closed_conformance_applications[position]
        .static_applications
        .clear();
    bad.closed_conformance_applications[position]
        .realization_callables
        .push(callable.clone());
    refresh_conformance_commitment(&mut bad, position);
    controls.push(("duplicate unreferenced callable registry", "duplicate", bad));
    for (name, reason, invalid) in controls {
        reject_module_at_ingress(&invalid, name, reason)?;
    }
    let domain = b"omega.requirement-schema.v1\0";
    let mut wire = artifact.to_bytes();
    let position = wire
        .windows(domain.len())
        .position(|bytes| bytes == domain)
        .ok_or("artifact has no original definition bytes")?;
    wire[position] ^= 1;
    let error = CanonicalTerminalArtifact::from_bytes(&wire)
        .err()
        .ok_or("wire ingress accepted an unknown original schema domain")?;
    if !format!("{error:?}").contains("unknown definition domain") {
        return Err(format!(
            "malformed schema wire refused for another reason: {error:?}"
        ));
    }
    Ok(())
}

fn reject_module_at_ingress(
    module: &terminal_psi::TerminalModule,
    name: &str,
    reason: &str,
) -> Result<(), String> {
    // Producer serialization is not admission. Exercise the same module
    // decoder used by artifact ingress, without an unrelated stale optimizer
    // identity masking the structural defect. Noncanonical wire order may
    // already refuse while encoding; canonical malformed payloads must decode.
    let error = terminal_codec::encode_module(module)
        .and_then(|bytes| terminal_codec::decode_module(&bytes))
        .err()
        .ok_or_else(|| format!("artifact ingress accepted {name}"))?;
    if !format!("{error:?}").contains(reason) {
        return Err(format!(
            "artifact control {name} refused for another reason: {error:?}"
        ));
    }
    Ok(())
}

fn refresh_conformance_commitment(module: &mut terminal_psi::TerminalModule, position: usize) {
    let application = &mut module.closed_conformance_applications[position];
    application.report_fingerprint =
        terminal_psi::closed_conformance_application_report_fingerprint(
            &module.identities,
            application,
        );
    application.commitment =
        terminal_psi::closed_conformance_application_commitment(&module.identities, application);
}

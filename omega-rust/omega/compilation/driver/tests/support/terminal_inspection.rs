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

pub(super) const ORACLE: &str = "expected_terminal_inspections.txt";

#[derive(Clone, Copy)]
enum ConformanceControls {
    MethodApplications,
    BoundaryCallables,
}

pub(super) fn inspect(root: &Path, fixture: &Path) -> Result<(), Vec<Diagnostic>> {
    inspect_machines(root, fixture).map_err(|message| vec![Diagnostic::error(message)])
}

fn inspect_machines(root: &Path, fixture: &Path) -> Result<(), String> {
    let selections = std::fs::read_to_string(fixture.join(ORACLE))
        .map_err(|error| format!("cannot read {ORACLE}: {error}"))?;
    let selections = selections
        .lines()
        .map(str::trim)
        .filter(|name| !name.is_empty());
    let mut inspected = false;
    for selection in selections {
        let words = selection.split_whitespace().collect::<Vec<_>>();
        let (machine, requires_debug, conformance_controls) = match words.as_slice() {
            [machine] => (*machine, false, None),
            [machine, "debug"] => (*machine, true, None),
            [machine, "closed-conformance-controls"] => (
                *machine,
                false,
                Some(ConformanceControls::MethodApplications),
            ),
            [machine, "boundary-conformance-controls"] => (
                *machine,
                false,
                Some(ConformanceControls::BoundaryCallables),
            ),
            _ => {
                return Err(format!(
                    "{ORACLE} needs a machine name and optional `debug`, `closed-conformance-controls` or `boundary-conformance-controls`"
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
        request.build_snapshot = snapshot;
        let artifact = compiler::CompilationSession::default()
            .produce(
                request,
                compiler::CompilationProduct::Inspection(machine.to_owned()),
            )
            .and_then(compiler::CompilationOutput::into_inspection)
            .map_err(compiler::CompilationProductError::into_diagnostics)
            .map_err(|errors| format!("inspection {machine} production: {errors:?}"))?;
        let mut transported = artifact.to_bytes();
        let decoded = CanonicalTerminalArtifact::from_bytes(&transported)
            .map_err(|error| format!("inspection {machine} transport: {error:?}"))?;
        if decoded.semantic_bytes() != artifact.semantic_bytes()
            || decoded.manifest() != artifact.manifest()
        {
            return Err(format!("inspection {machine} changed during transport"));
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
                ConformanceControls::MethodApplications => closed_conformance_controls(&decoded)?,
                ConformanceControls::BoundaryCallables => boundary_conformance_controls(&decoded)?,
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
        let error = CanonicalTerminalArtifact::from_parts(
            invalid,
            terminal_codec::decode_psi_optimization_execution_record(artifact.optimization_bytes())
                .map_err(|error| format!("original optimization record: {error:?}"))?,
            None,
        )
        .err()
        .ok_or_else(|| format!("artifact ingress accepted {name}"))?;
        if !format!("{error:?}").contains(reason) {
            return Err(format!(
                "artifact control {name} refused for another reason: {error:?}"
            ));
        }
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
        let error = CanonicalTerminalArtifact::from_parts(
            invalid,
            terminal_codec::decode_psi_optimization_execution_record(artifact.optimization_bytes())
                .map_err(|error| format!("original optimization record: {error:?}"))?,
            None,
        )
        .err()
        .ok_or_else(|| format!("artifact ingress accepted {name}"))?;
        let error = format!("{error:?}");
        if !error.contains(reason) {
            return Err(format!(
                "artifact control {name} refused for another reason: {error}"
            ));
        }
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

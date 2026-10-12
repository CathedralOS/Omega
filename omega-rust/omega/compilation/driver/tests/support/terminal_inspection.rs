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
        let (machine, requires_debug) = match words.as_slice() {
            [machine] => (*machine, false),
            [machine, "debug"] => (*machine, true),
            _ => {
                return Err(format!(
                    "{ORACLE} needs a machine name and optional `debug`"
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

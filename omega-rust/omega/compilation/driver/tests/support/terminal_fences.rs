//! An authored fixture oracle observes the exact completed fence stream through
//! the public Terminal SDK. One retained source artifact supplies every target;
//! no separate compiler test, forged module or host hardware authority is used.

use std::path::Path;

use terminal_codec::CanonicalTerminalArtifact;
use terminal_fuel::TerminalFuelMeter;
use terminal_interpreter::{
    TerminalEffect, TerminalExecutionResult, TerminalExecutionStatus, TerminalInstantiation,
    TerminalMemoryOrderingEnvironment, TerminalProgram, TerminalRuntimeSupplyRequest,
};
use terminal_psi::MemoryFenceKind;

const ORACLE: &str = "expected_terminal_fences.txt";

fn kind(name: &str) -> Option<MemoryFenceKind> {
    Some(match name {
        "lfence" => MemoryFenceKind::Load,
        "sfence" => MemoryFenceKind::Store,
        "mfence" => MemoryFenceKind::Full,
        "dmb_ld" => MemoryFenceKind::Aarch64Load,
        "dmb_st" => MemoryFenceKind::Aarch64Store,
        "dmb_sy" => MemoryFenceKind::Aarch64Full,
        _ => return None,
    })
}

pub(super) fn observe(
    artifact: &CanonicalTerminalArtifact,
    fixture: &Path,
    allowance: u64,
) -> super::Observation {
    let text = match std::fs::read_to_string(fixture.join(ORACLE)) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return super::Observation::default();
        }
        Err(error) => return failure(format!("cannot read {ORACLE}: {error}")),
    };
    let decoded = match CanonicalTerminalArtifact::from_bytes(&artifact.to_bytes()) {
        Ok(decoded) => decoded,
        Err(error) => return failure(format!("canonical fence artifact ingress: {error}")),
    };
    if artifact.semantic_bytes() != decoded.semantic_bytes()
        || artifact.view().identity() != decoded.view().identity()
    {
        return failure("canonical fence roundtrip changed semantics or identity".into());
    }
    let artifact = &decoded;
    let bytes = artifact.semantic_bytes().to_vec();
    let before = artifact.view();
    let mut observation = super::Observation::default();
    let mut targets = Vec::new();
    let mut matched = true;
    for (index, line) in text.lines().enumerate() {
        let words = line.split_whitespace().collect::<Vec<_>>();
        if words.is_empty() {
            continue;
        }
        let [target, names @ ..] = words.as_slice() else {
            unreachable!("empty lines were excluded");
        };
        if names.is_empty() || targets.contains(target) {
            return failure(format!(
                "{ORACLE}: line {} needs a unique target and nonempty fence sequence",
                index + 1
            ));
        }
        let expected = match names
            .iter()
            .map(|name| kind(name))
            .collect::<Option<Vec<_>>>()
        {
            Some(expected) => expected,
            None => {
                return failure(format!(
                    "{ORACLE}: line {} contains an unknown fence",
                    index + 1
                ));
            }
        };
        targets.push(*target);
        match observe_target(artifact, target, &expected, allowance) {
            Ok(()) => observation
                .facts
                .push(format!("terminal-fences.{target}:{}", names.join(","))),
            Err(error) => {
                observation
                    .facts
                    .push(format!("terminal-fences.{target}:differs"));
                observation
                    .diagnostics
                    .push(format!("Terminal fences {target}: {error}"));
                matched = false;
            }
        }
    }
    if targets.is_empty() {
        return failure(format!("{ORACLE}: no target expectations"));
    }
    let after = artifact.view();
    let unchanged = bytes == artifact.semantic_bytes()
        && before.identity() == after.identity()
        && std::ptr::eq(before.module(), after.module());
    observation.facts.push(format!(
        "terminal-fence-artifact:{}",
        if unchanged { "unchanged" } else { "changed" }
    ));
    observation.facts.push(format!(
        "terminal-fences:{}",
        if matched && unchanged {
            "match"
        } else {
            "differs"
        }
    ));
    observation
}

fn failure(message: String) -> super::Observation {
    super::Observation {
        facts: vec!["terminal-fences:invalid".into()],
        diagnostics: vec![message],
    }
}

fn observe_target(
    artifact: &CanonicalTerminalArtifact,
    target: &str,
    expected: &[MemoryFenceKind],
    allowance: u64,
) -> Result<(), String> {
    let program = TerminalProgram::borrow(artifact.view());
    let selected = terminal_interpreter::admit_target_machine_selections(artifact.view(), target);
    let layouts = terminal_interpreter::admit_target_layouts(artifact.view(), &selected)
        .map_err(|error| error.to_string())?;
    let supplies = |model| {
        program.admit_runtime_supplies(TerminalRuntimeSupplyRequest {
            x86_flags: None,
            memory_ordering: model,
            entry: None,
            installation: None,
            host_imports: &[],
            target_machines: Some(&selected),
            target_layouts: Some(&layouts),
        })
    };
    let absent = supplies(None).map_err(|error| error.to_string())?;
    if !matches!(
        program.execution_requirements(Some(&absent)),
        Err(
            terminal_interpreter::TerminalInterpretError::UnsupportedCapability(
                terminal_interpreter::TerminalUnsupportedCapability::RegisterAccess {
                    operation: terminal_psi::RegisterOperation::MemoryFence(_)
                }
            )
        )
    ) {
        return Err("missing model did not refuse before entry effects".into());
    }
    let model = TerminalMemoryOrderingEnvironment::private_sequential(target)
        .ok_or("target has no private sequential model")?;
    let other_target = if target.ends_with("arm64") {
        "linux_x86_64"
    } else {
        "linux_arm64"
    };
    if supplies(TerminalMemoryOrderingEnvironment::private_sequential(
        other_target,
    ))
    .is_ok()
    {
        return Err("wrong architecture supplied an ordering capability".into());
    }
    let installed = supplies(Some(model)).map_err(|error| error.to_string())?;
    let requirements = program
        .execution_requirements(Some(&installed))
        .map_err(|error| error.to_string())?;
    if !requirements.imports.is_empty()
        || !requirements.missing_providers.is_empty()
        || !requirements.external_completions.is_empty()
    {
        return Err("fence observation needs an unprovided external capability".into());
    }
    let instantiate = || {
        program
            .instantiate(TerminalInstantiation {
                installation: Some(&installed),
                ..Default::default()
            })
            .map_err(|error| error.to_string())
    };
    let complete = TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit);
    let mut reference = instantiate()?;
    let mut reference_fuel = TerminalFuelMeter::with_allowance(allowance);
    if reference
        .invoke(&mut reference_fuel)
        .map_err(|error| error.to_string())?
        != complete
    {
        return Err("fence observation did not complete normally within its allowance".into());
    }
    let observed = reference
        .effects()
        .iter()
        .map(|effect| match effect {
            TerminalEffect::MemoryFence { kind, .. } => Ok(*kind),
            other => Err(format!("unexpected effect: {other:?}")),
        })
        .collect::<Result<Vec<_>, _>>()?;
    if observed != expected {
        return Err(format!("expected {expected:?}, observed {observed:?}"));
    }
    let mut paused = instantiate()?;
    let mut fuel = TerminalFuelMeter::with_allowance(0);
    let mut finished = false;
    let mut paused_inside_sequence = false;
    for _ in 0..=reference_fuel.usage().total_units() {
        match paused
            .invoke(&mut fuel)
            .map_err(|error| error.to_string())?
        {
            TerminalExecutionStatus::SponsorExhausted(exhaustion) => {
                if !reference.effects().starts_with(paused.effects()) {
                    return Err("paused effects are not the completed reference prefix".into());
                }
                paused_inside_sequence |=
                    !paused.effects().is_empty() && paused.effects().len() < expected.len();
                let before = paused.effects().to_vec();
                let usage = fuel.usage().clone();
                let allowance_before = fuel.remaining_allowance();
                if paused
                    .invoke(&mut fuel)
                    .map_err(|error| error.to_string())?
                    != TerminalExecutionStatus::SponsorExhausted(exhaustion)
                    || paused.effects() != before
                    || fuel.usage() != &usage
                    || fuel.remaining_allowance() != allowance_before
                {
                    return Err("unfunded retry repeated progress, effects or fuel charges".into());
                }
                let additional = exhaustion
                    .required_units
                    .checked_sub(exhaustion.remaining_units)
                    .filter(|units| *units != 0)
                    .ok_or("refused site requires no new fuel")?;
                fuel.replenish(additional)
                    .map_err(|error| error.to_string())?;
            }
            status => {
                if status != complete {
                    return Err(format!("paused fence outcome: {status:?}"));
                }
                finished = true;
                break;
            }
        }
    }
    if !finished
        || (expected.len() > 1 && !paused_inside_sequence)
        || paused.effects() != reference.effects()
        || fuel.usage() != reference_fuel.usage()
    {
        return Err(
            "fuel-paused fence stream or attribution differs, or no interior pause occurred".into(),
        );
    }
    let effects = paused.effects().to_vec();
    let mut empty = TerminalFuelMeter::with_allowance(0);
    if paused
        .invoke(&mut empty)
        .map_err(|error| error.to_string())?
        != complete
        || empty.usage().total_units() != 0
        || paused.effects() != effects
    {
        return Err("completed retry repeated a fence or charge".into());
    }
    paused.close().map_err(|error| format!("{error:?}"))?;
    reference.close().map_err(|error| format!("{error:?}"))?;
    Ok(())
}

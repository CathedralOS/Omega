//! An authored fixture oracle observes the exact completed fence stream through
//! the public Terminal SDK. One retained source artifact supplies every target;
//! no separate compiler test, forged module or host hardware authority is used.

use std::path::Path;

use psi_work::WorkMeter;
use terminal_codec::CanonicalTerminalArtifact;
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
    proposals: &[backend_plan::TerminalTargetProviderProposal],
    root_provisions: &[&compilation_products::TerminalRuntimeProvisions],
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
        match observe_target(
            artifact,
            proposals,
            root_provisions,
            target,
            &expected,
            allowance,
        ) {
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
    proposals: &[backend_plan::TerminalTargetProviderProposal],
    root_provisions: &[&compilation_products::TerminalRuntimeProvisions],
    target: &str,
    expected: &[MemoryFenceKind],
    allowance: u64,
) -> Result<(), String> {
    let program = TerminalProgram::borrow(artifact.view());
    let selected = terminal_interpreter::admit_target_machine_selections(artifact.view(), target);
    let target_profile = target::TargetProfile::from_canonical_target_name(target)
        .map_err(|_| format!("unknown fence target `{target}`"))?;
    let target_facts = target_profile
        .layout_target_facts()
        .ok_or_else(|| format!("fence target `{target}` has no representation facts"))?;
    let layouts =
        terminal_interpreter::admit_target_layouts(artifact.view(), &selected, &target_facts)
            .map_err(|error| error.to_string())?;
    // Probe the same authored provider choices as ordinary target execution.
    // Without their admitted installation, preflight stops at a missing
    // boundary provider before it can inspect that provider's fence demands.
    let mut matching = proposals
        .iter()
        .filter(|proposal| proposal.target == target);
    let proposal = matching.next();
    if matching.next().is_some() {
        return Err("duplicate target provider proposal".into());
    }
    let installation = proposal
        .map(|proposal| {
            if proposal.terminal_psi != artifact.view().identity() {
                return Err("provider proposal names another Terminal artifact".into());
            }
            proposal
                .selection
                .as_ref()
                .map_err(|reasons| reasons.join("; "))?
                .admit(artifact.view(), target)
        })
        .transpose()?;
    // Use the entry produced for this target, just as target execution does.
    // The portable artifact's default entry need not be this application's root.
    let mut matching_roots = root_provisions
        .iter()
        .copied()
        .filter(|provisions| provisions.target_slot().owner.target_name() == target);
    let root = matching_roots.next();
    if matching_roots.next().is_some() {
        return Err("duplicate admitted target root".into());
    }
    if root.is_some_and(|root| root.identity() != artifact.view().identity()) {
        return Err("target root names another Terminal artifact".into());
    }
    let entry = root.map(|root| terminal_psi::TargetMachineCallable::Machine(root.entry()));
    let supplies = |model| {
        program.admit_runtime_supplies(TerminalRuntimeSupplyRequest {
            native_code_layouts: None,
            x86_flags: None,
            memory_ordering: model,
            entry,
            installation: installation.as_ref(),
            host_imports: &[],
            target_machines: Some(&selected),
            target_layouts: Some(&layouts),
        })
    };
    let absent = supplies(None).map_err(|error| error.to_string())?;
    match program.execution_requirements(Some(&absent)) {
        Err(terminal_interpreter::TerminalInterpretError::UnsupportedCapability(
            terminal_interpreter::TerminalUnsupportedCapability::RegisterAccess {
                operation: terminal_psi::RegisterOperation::MemoryFence(_),
            },
        )) => {}
        Err(error) => {
            return Err(format!(
                "missing ordering model refused for another reason: {error}"
            ));
        }
        Ok(requirements) => {
            return Err(format!(
                "missing ordering model was accepted; {} imports, {} missing providers and {} external completions remain",
                requirements.imports.len(),
                requirements.missing_providers.len(),
                requirements.external_completions.len(),
            ));
        }
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
    let mut reference_work = WorkMeter::with_budget(allowance);
    if reference
        .invoke(&mut reference_work)
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
    let mut work = WorkMeter::with_budget(0);
    let mut finished = false;
    let mut paused_inside_sequence = false;
    for _ in 0..=reference_work.usage().total_units() {
        match paused
            .invoke(&mut work)
            .map_err(|error| error.to_string())?
        {
            TerminalExecutionStatus::SponsorExhausted(exhaustion) => {
                if !reference.effects().starts_with(paused.effects()) {
                    return Err("paused effects are not the completed reference prefix".into());
                }
                paused_inside_sequence |=
                    !paused.effects().is_empty() && paused.effects().len() < expected.len();
                let before = paused.effects().to_vec();
                let usage = work.usage().clone();
                let allowance_before = work.work_remaining();
                if paused
                    .invoke(&mut work)
                    .map_err(|error| error.to_string())?
                    != TerminalExecutionStatus::SponsorExhausted(exhaustion)
                    || paused.effects() != before
                    || work.usage() != &usage
                    || work.work_remaining() != allowance_before
                {
                    return Err("unfunded retry repeated progress, effects or work charges".into());
                }
                let additional = exhaustion
                    .required_units
                    .checked_sub(exhaustion.remaining_units)
                    .filter(|units| *units != 0)
                    .ok_or("refused site requires no new work")?;
                work.replenish(additional)
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
        || work.usage() != reference_work.usage()
    {
        return Err(
            "work-budget-paused fence stream or attribution differs, or no interior pause occurred"
                .into(),
        );
    }
    let effects = paused.effects().to_vec();
    let mut empty = WorkMeter::with_budget(0);
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

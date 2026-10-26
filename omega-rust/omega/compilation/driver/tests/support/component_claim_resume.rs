//! Compare real component custody with uninterrupted and fuel-paused execution.
//! Both consume the same retained product. Only fresh instance namespaces are
//! relabeled for observation equality; provider identities and all effects stay.
use std::{collections::BTreeSet, num::NonZeroU64, path::Path};

use terminal_interpreter::{TerminalEffect, TerminalExecutionResult, TerminalExecutionStatus};
use terminal_runtime::{
    TerminalDomainExitDelegation, TerminalRuntimeObservation, TerminalRuntimeRequest,
};

pub(super) fn observe(
    fixture: &Path,
    artifact: &terminal_codec::CanonicalTerminalArtifact,
    provisions: Option<&compilation_products::TerminalRuntimeProvisions>,
    target: Option<&str>,
    ordinary: &TerminalRuntimeObservation,
    fuel_allowance: u64,
) -> Result<(), String> {
    let oracle = fixture.join("expected_component_resume_calls.txt");
    let expected = match std::fs::read_to_string(&oracle) {
        Ok(text) => text
            .trim()
            .parse::<usize>()
            .map_err(|error| error.to_string())?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(format!("read component resume oracle: {error}")),
    };
    if expected == 0
        || ordinary.status != TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
        || ordinary.fuel_usage.total_units() < 2
    {
        return Err(
            "component resume oracle requires nonzero calls and normal unit completion".into(),
        );
    }
    let expected_effects = normalized_calls(&ordinary.effects, expected)?;
    let execute = |allowance, quantum| {
        terminal_runtime::execute_sliced(
            TerminalRuntimeRequest {
                native_provider: None,
                artifact,
                provisions,
                provider_proposal: None,
                target,
                x86_flags: target
                    .map(terminal_runtime::capture_host_x86_flags)
                    .transpose()
                    .map_err(|error| format!("component resume flags: {error}"))?
                    .flatten(),
                fuel_allowance: allowance,
                domain_exit: TerminalDomainExitDelegation::IsolatedContainingDomain,
                standard_input: None,
                clock: None,
            },
            quantum,
        )
        .map_err(|error| format!("component resume execution: {error}"))
    };
    let quantum = NonZeroU64::new(1).ok_or("component resume quantum is zero")?;
    let resumed = execute(fuel_allowance, quantum)?;
    if resumed.status != ordinary.status
        || resumed.stdout != ordinary.stdout
        || resumed.stderr != ordinary.stderr
        || resumed.private_entry_ram != ordinary.private_entry_ram
        || resumed.fuel_usage != ordinary.fuel_usage
        || resumed.fuel_suspensions == 0
        || resumed.component_suspensions == 0
        || normalized_calls(&resumed.effects, expected)? != expected_effects
    {
        return Err(
            "resumed component calls differ in custody effects, output or exact fuel attribution"
                .into(),
        );
    }
    // A fresh isolated invocation with one unit less must remain exhausted;
    // resuming a pending child cannot replenish its original total allowance.
    let short = ordinary.fuel_usage.total_units() - 1;
    for quantum in [NonZeroU64::MAX, quantum] {
        let stopped = execute(short, quantum)?;
        if !matches!(stopped.status, TerminalExecutionStatus::SponsorExhausted(_))
            || stopped.fuel_usage.total_units() > short
        {
            return Err("component continuation escaped the sponsor's original total fuel".into());
        }
    }
    Ok(())
}

fn normalized_calls(
    effects: &[TerminalEffect],
    expected: usize,
) -> Result<Vec<TerminalEffect>, String> {
    if effects.len() != expected {
        return Err(format!(
            "component call count {} differs from oracle {expected}",
            effects.len()
        ));
    }
    let mut namespaces = BTreeSet::new();
    let mut normalized = effects.to_vec();
    for (ordinal, effect) in normalized.iter_mut().enumerate() {
        let TerminalEffect::ComponentCall {
            instance_namespace,
            effects,
            ..
        } = effect
        else {
            return Err("component resume oracle requires actual component effects".into());
        };
        if !namespaces.insert(*instance_namespace)
            || effects.len() != 1
            || !matches!(effects[0], TerminalEffect::BoundaryCall { .. })
        {
            return Err(
                "component invocation lacks its distinct instance or actual boundary effect".into(),
            );
        }
        *instance_namespace = u64::try_from(ordinal).map_err(|error| error.to_string())?;
    }
    Ok(normalized)
}

//! Reconstruct selected behavior after source release and canonical transport.
//! Missing history must refuse absence, even for an executable no-op body.

use effects::{BehaviorExclusionVerdict, EvidenceGapKind};
use terminal_codec::{CanonicalTerminalArtifact, decode_module, encode_module};
use terminal_psi::{CrashCause, TargetMachineCallable};
use terminal_psi_to_abstract_operations::RetainedTerminalArtifact;

pub(super) fn inspect(retained: &RetainedTerminalArtifact) -> Result<(), String> {
    let proposal = retained
        .native_realization_proposal()
        .ok_or("behavior controls need the authored native proposal")?;
    let exclusions = proposal.behavior_exclusions();
    if !exclusions.crash_causes().contains(&CrashCause::Trap) {
        return Err("behavior controls require the authored Trap exclusion".into());
    }
    let loaded = CanonicalTerminalArtifact::from_bytes(&retained.artifact().to_bytes())
        .map_err(|error| format!("behavior artifact transport: {error:?}"))?;
    for artifact in [retained.artifact(), &loaded] {
        let module = artifact.semantic_module();
        let Some(TargetMachineCallable::Machine(entry)) = module.entry else {
            return Err("behavior controls require an executable entry".into());
        };
        let inspect = |module: &terminal_psi::TerminalModule| {
            effects::establish_behavior_exclusions(
                module,
                &[entry],
                exclusions,
                &proposal.providers().selection,
                proposal.target_profile().target_name(),
            )
        };
        let actual = inspect(module);
        if actual.verdict() != BehaviorExclusionVerdict::Satisfied {
            return Err(format!("selected no-op behavior refused: {actual:?}"));
        }

        let selected = proposal
            .providers()
            .selection
            .plans()
            .iter()
            .find_map(|plan| {
                plan.rows.iter().find_map(|row| {
                    let effects::provider_plan::ProviderBinding::CheckedAdapter {
                        machine_identity,
                        ..
                    } = row.binding
                    else {
                        return None;
                    };
                    module
                        .provider_candidates
                        .iter()
                        .find(|candidate| {
                            module.identities.text(candidate.requirement_identity)
                                == Some(plan.identity_text(row.requirement_identity))
                                && module.identities.text(candidate.candidate_identity)
                                    == Some(plan.identity_text(machine_identity))
                        })
                        .map(|candidate| candidate.candidate)
                })
            })
            .ok_or("behavior controls need the exact selected checked body")?;
        let selected_behavior = module
            .unoptimized_machine_behavior(selected)
            .ok_or("selected no-op lacks behavior history")?;
        if !selected_behavior.operations.is_empty() {
            return Err("fixture selected body must be a semantic no-op".into());
        }
        let mut missing_noop = module.clone();
        missing_noop
            .unoptimized_behavior
            .retain(|row| row.machine != selected);
        let missing_noop_bytes = encode_module(&missing_noop)
            .map_err(|error| format!("missing no-op history encoding: {error:?}"))?;
        let missing_noop = decode_module(&missing_noop_bytes)
            .map_err(|error| format!("missing no-op history transport: {error:?}"))?;
        let report = inspect(&missing_noop);
        if report.verdict() != BehaviorExclusionVerdict::InsufficientEvidence
            || !report
                .gaps
                .iter()
                .any(|gap| gap.kind == EvidenceGapKind::MissingUnoptimizedBehavior(selected))
        {
            return Err(format!(
                "missing no-op history was trusted as absence: {report:?}"
            ));
        }

        let mut missing = module.clone();
        missing
            .unoptimized_behavior
            .retain(|row| row.machine != entry);
        let missing_bytes = encode_module(&missing)
            .map_err(|error| format!("missing history encoding: {error:?}"))?;
        let missing = decode_module(&missing_bytes)
            .map_err(|error| format!("missing history transport: {error:?}"))?;
        let report = inspect(&missing);
        if report.verdict() != BehaviorExclusionVerdict::InsufficientEvidence
            || !report
                .gaps
                .iter()
                .any(|gap| gap.kind == EvidenceGapKind::MissingUnoptimizedBehavior(entry))
        {
            return Err(format!(
                "missing behavior did not refuse absence: {report:?}"
            ));
        }

        let mut empty = module.clone();
        let root = empty
            .unoptimized_behavior
            .iter_mut()
            .find(|row| row.machine == entry)
            .ok_or("fixture entry has no retained behavior")?;
        if root.operations.is_empty() {
            return Err("fixture entry must contribute an actual semantic operation".into());
        }
        root.operations.clear();
        let empty_bytes =
            encode_module(&empty).map_err(|error| format!("empty history encoding: {error:?}"))?;
        match decode_module(&empty_bytes) {
            Err(error)
                if format!("{error:?}")
                    .contains("unoptimized behavior omits an executable semantic contribution") => {
            }
            Err(error) => return Err(format!("empty history has wrong refusal: {error:?}")),
            Ok(_) => return Err("empty history hid the fixture's executable operation".into()),
        }
    }
    Ok(())
}

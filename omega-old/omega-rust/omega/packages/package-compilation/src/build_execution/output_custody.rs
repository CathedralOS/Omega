// Staged-output obligation settlement (`settle_build_output_obligations`) and
// generated-source selection (`select_generated_sources`). Verifies that every
// linear `BuildOutput::require` obligation completed against its exact receipt,
// underwent no post-completion mutations, and resides as a sealed regular file
// in `BuildStagedOutputTree`.

use super::filesystem_scope::BUILD_OUTPUT_ROOT_IDENTITY;
use super::observations::{BuildIncludedSourceHandoff, BuildRequiredOutputSettlement};
use build_output::{
    BuildStagedOutputEntryKind, BuildStagedOutputTree, PackageGeneratedSource,
    select_included_sources,
};
use diagnostics::Diagnostic;

pub(super) fn settle_build_output_obligations(
    obligations: &[filesystem_authority::BuildOutputObligation],
    receipts: &[filesystem_authority::BuildOutputReceipt],
    staged_output_tree: Option<&BuildStagedOutputTree>,
    machine_name: &str,
) -> Result<Vec<BuildRequiredOutputSettlement>, Vec<Diagnostic>> {
    let mut settlements = Vec::with_capacity(obligations.len());
    for (index, obligation) in obligations.iter().enumerate() {
        let name = String::from_utf8_lossy(obligation.relative_path());
        if obligation.root() != BUILD_OUTPUT_ROOT_IDENTITY {
            return Err(vec![Diagnostic::error(format!(
                "build-time evaluation of `{machine_name}` issued required output `{name}` outside the compiler-issued Output root"
            ))]);
        }
        let filesystem_authority::BuildOutputObligationState::Completed {
            receipt: receipt_index,
            ..
        } = obligation.state()
        else {
            let message = match obligation.state() {
                filesystem_authority::BuildOutputObligationState::Pending => format!(
                    "required output `{name}` of `{machine_name}` was declared but never completed"
                ),
                filesystem_authority::BuildOutputObligationState::Failed { diagnostic, .. } => {
                    format!(
                        "required output `{name}` of `{machine_name}` failed: {}",
                        String::from_utf8_lossy(diagnostic)
                    )
                }
                filesystem_authority::BuildOutputObligationState::Completed { .. } => {
                    unreachable!("completed obligations settle")
                }
            };
            return Err(vec![Diagnostic::error(message)]);
        };
        let Some(receipt) = receipts.get(*receipt_index) else {
            return Err(vec![Diagnostic::error(format!(
                "build-time evaluation of `{machine_name}` issued no completion receipt for required output `{name}`"
            ))]);
        };
        if receipt.obligation() != index
            || receipt.root() != obligation.root()
            || receipt.relative_path() != obligation.relative_path()
        {
            return Err(vec![Diagnostic::error(format!(
                "build-time evaluation of `{machine_name}` completed required output `{name}` against a different output than it declared"
            ))]);
        }
        if !obligation.post_completion_mutations().is_empty() {
            return Err(vec![Diagnostic::error(format!(
                "required output `{name}` of `{machine_name}` was mutated after its completion was accepted"
            ))]);
        }
        match staged_output_tree.and_then(|tree| tree.sealed_entry(obligation.relative_path())) {
            Some(entry) if matches!(entry.kind(), BuildStagedOutputEntryKind::File { .. }) => {}
            Some(_) => {
                return Err(vec![Diagnostic::error(format!(
                    "required output `{name}` of `{machine_name}` is not a sealed regular file in staged-output custody"
                ))]);
            }
            None => {
                return Err(vec![Diagnostic::error(format!(
                    "required output `{name}` of `{machine_name}` was completed without sealed staged-output custody"
                ))]);
            }
        }
        settlements.push(BuildRequiredOutputSettlement {
            relative_path: obligation.relative_path().to_vec(),
            sealed_attempt_ordinal: u64::try_from(receipt.sealed_at()).map_err(|_| {
                vec![Diagnostic::error(format!(
                    "build-time evaluation of `{machine_name}` produced a sealed-output ordinal outside canonical u64"
                ))]
            })?,
        });
    }
    Ok(settlements)
}

pub(super) fn select_generated_sources(
    staged_output_tree: Option<&BuildStagedOutputTree>,
    included_source_handoffs: &[BuildIncludedSourceHandoff],
    machine_name: &str,
) -> Result<Vec<PackageGeneratedSource>, Vec<Diagnostic>> {
    Ok(match staged_output_tree {
        Some(tree) => {
            let included_source_paths = included_source_handoffs
                .iter()
                .map(|handoff| handoff.relative_path.clone())
                .collect::<Vec<_>>();
            select_included_sources(tree, &included_source_paths)?
        }
        None if included_source_handoffs.is_empty() => Vec::new(),
        None => {
            return Err(vec![Diagnostic::error(format!(
                "build-time evaluation of `{machine_name}` handed off generated source without sponsored staged-output custody"
            ))]);
        }
    })
}

//! Staged-output custody: settling obligations against captured Output trees,
//! settling the compiler-owned required-output obligations, and selecting the
//! generated sources the run handed off.

use crate::evidence::filesystem_scope::BUILD_OUTPUT_ROOT_IDENTITY;
use crate::evidence::observations::{BuildIncludedSourceHandoff, BuildRequiredOutputSettlement};
use build_output::{
    BuildStagedOutputEntryKind, BuildStagedOutputTree, PackageGeneratedSource,
    select_included_sources,
};
use diagnostics::Diagnostic;

/// Settle the compiler-owned required-output obligations recorded by a
/// successful build evaluation against retained staged-output custody.
///
/// Settlement is linear and compiler-checked: every issued obligation must
/// have been completed exactly once by the sealed regular file its receipt
/// names. A pending obligation, a sticky `fail`, a receipt rejoined to a
/// different output, a completed output mutated afterward, or a completion
/// whose file never reached sealed staged custody each reject the activation
/// before any product may publish. The returned rows are the durable
/// observation evidence; `build.rs` publication still re-derives the file
/// from staged custody.
pub(super) fn settle_build_output_obligations(
    obligations: &[checked_interpreter::BuildOutputObligation],
    receipts: &[checked_interpreter::BuildOutputReceipt],
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
        let checked_interpreter::BuildOutputObligationState::Completed {
            receipt: receipt_index,
            ..
        } = obligation.state()
        else {
            let message = match obligation.state() {
                checked_interpreter::BuildOutputObligationState::Pending => format!(
                    "required output `{name}` of `{machine_name}` was declared but never completed"
                ),
                checked_interpreter::BuildOutputObligationState::Failed { diagnostic, .. } => {
                    format!(
                        "required output `{name}` of `{machine_name}` failed: {}",
                        String::from_utf8_lossy(diagnostic)
                    )
                }
                checked_interpreter::BuildOutputObligationState::Completed { .. } => {
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

/// Select the generated sources the run handed off from sealed staged custody.
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

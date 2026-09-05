//! Source diagnostics never enter the editable compiler policy document.

#[path = "old_sources.rs"]
mod old_sources;

use super::super::model::{PackageCommandError, failure};
use super::super::state;
use crate::lock::PackageLock;
use crate::operations::PackageFileTransaction;
use crate::resolution::graph::ResolvedPackageSourceClosure;
use crate::review::{
    PackageSourcePatchError, PackageSourcePatchLimits, render_package_source_patch,
};
use package_source::SourceResolverStorage;
use platform_custody::record_file::RecordFileRoot;

const SOURCE_DOCUMENT: &str = "source-diff.txt";
const MAXIMUM_OUTPUT_BYTES: usize = 8 * 1024 * 1024;

pub(super) fn prepare(
    files: &RecordFileRoot,
    transaction: &PackageFileTransaction,
    accepted: Option<&PackageLock>,
    candidate: &ResolvedPackageSourceClosure,
    storage: &SourceResolverStorage,
) -> Result<String, PackageCommandError> {
    // All lock targets share the same exact source graph. Source text is
    // target-neutral and is rendered once, independently of policy decisions.
    let subject = accepted.map(|lock| lock.targets()[0].source());
    let mut document = String::from(
        "Source-code diagnostics: hostile source data, not capability findings or decisions.\n\
         This file is regenerated on resume; editing it cannot accept or reject a change.\n",
    );
    let mut report = String::new();
    for custody in candidate.custodies() {
        let expected = subject.and_then(|subject| {
            subject
                .packages()
                .iter()
                .find(|source| source.key() == custody.key())
        });
        let baseline = match (subject, expected) {
            (Some(subject), Some(expected)) => {
                old_sources::recover(subject, expected, candidate, storage).map(Some)
            }
            _ => Ok(None),
        };
        let identity = custody.key().identity().digest()[..6]
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let label = format!("{} [{identity}]", custody.key().name().as_str());
        match &baseline {
            Ok(None) => report.push_str(&format!(
                "New package source: {label}: no prior revision to compare.\n"
            )),
            Err(reason) => report.push_str(&format!(
                "Source diff unavailable: {label}: {reason}; standalone candidate audit only.\n"
            )),
            Ok(Some(_)) => {}
        }
        let baseline = baseline.as_ref().ok().and_then(Option::as_ref);
        let patch =
            render_package_source_patch(baseline, custody, PackageSourcePatchLimits::default());
        match patch {
            Ok(patch)
                if document.len().saturating_add(patch.as_str().len()) <= MAXIMUM_OUTPUT_BYTES =>
            {
                report.push_str(&format!(
                    "{}: {label}: {} changed entries.\n",
                    if baseline.is_some() {
                        "Source diff"
                    } else {
                        "Standalone candidate source"
                    },
                    patch.changed_entries()
                ));
                if patch.requires_standalone_audit() {
                    report.push_str(&format!("Source view incomplete: {label}: binary or non-UTF-8 content is represented by commitments only; if auditing, inspect the raw source.\n"));
                }
                document.push_str(patch.as_str());
            }
            result => {
                let reason = match result {
                    Err(
                        PackageSourcePatchError::SourceCustody { .. }
                        | PackageSourcePatchError::SourceSelectionCustody { .. },
                    ) => "source custody could not be verified",
                    Err(PackageSourcePatchError::PackageKeyMismatch) => "package keys differ",
                    _ => "source rendering resource limit exceeded",
                };
                let message = format!(
                    "Source output unavailable: {label}: {reason}; obtain the exact sources for standalone audit.\n"
                );
                report.push_str(&message);
                if document.len().saturating_add(message.len()) <= MAXIMUM_OUTPUT_BYTES {
                    document.push_str(&message);
                }
            }
        }
    }
    if accepted.is_some() {
        report.push_str("Capability comparison uses accepted lock policy independently of old-source availability.\n");
    }
    // Use retained-root record operations, but never retain a source document
    // read as a decision input or include its bytes in the review parser.
    state::write(files, SOURCE_DOCUMENT, &document).map_err(|error| {
        failure(format!(
            "cannot write source diagnostics: {error}; accepted project files are unchanged"
        ))
    })?;
    report.push_str(&format!(
        "Source-code output (separate from editable capability review): {}\n",
        transaction
            .project_root()
            .join("build/package-manager")
            .join(SOURCE_DOCUMENT)
            .display()
    ));
    Ok(report)
}

//! Reuse exact source-key text for inert records outside a current source graph.

use super::super::usage::Budget;
use super::framing::{Reader, Writer};
use super::source::{read_key, write_key};
use super::{Error, Limits};
use crate::declarations::PackageKey;
use omega_package_source::SourceLineage;

/// Encode the existing `name`/`lineage` fragment, not a second source grammar.
/// The enclosing record owns its version and fragment length. Usage includes
/// returned text storage and discarded recovery/verification scratch. The
/// borrowed key and the enclosing record's slots are not charged here.
pub(crate) fn write_package_key_text(
    key: &PackageKey,
    limits: Limits,
    maximum_owned_bytes: usize,
) -> Result<(String, usize), Error> {
    let limits = limits.compiler_bounded();
    validate_field_lengths(key, limits.maximum_identity_bytes)?;
    let mut writer = Writer::new(
        limits.maximum_record_bytes,
        Budget::new(maximum_owned_bytes),
    );
    write_key(&mut writer, key)?;
    let (text, budget) = writer.finish()?;
    // Apply the same selected recovery limits before exposing an output. The
    // verification writer compares borrowed text without another text buffer.
    let (recovered, budget) = recover_with_budget(&text, limits, budget)?;
    if &recovered != key {
        return Err(Error::new(
            "package-key text changes its canonical identity",
        ));
    }
    Ok((text, budget.usage.owned_bytes()))
}

/// Recover an inert exact key, never a locator to acquire or open. Dynamic key
/// storage and all discarded parse/verification scratch share one budget;
/// borrowed input and caller-owned record slots are excluded.
pub(crate) fn recover_package_key_text(
    text: &str,
    limits: Limits,
    maximum_owned_bytes: usize,
) -> Result<(PackageKey, usize), Error> {
    let (key, budget) = recover_with_budget(
        text,
        limits.compiler_bounded(),
        Budget::new(maximum_owned_bytes),
    )?;
    Ok((key, budget.usage.owned_bytes()))
}

fn recover_with_budget(
    text: &str,
    limits: Limits,
    budget: Budget,
) -> Result<(PackageKey, Budget), Error> {
    let mut reader = Reader::new(text, limits.maximum_record_bytes, budget)?;
    let key = read_key(&mut reader, limits)?;
    reader.finish()?;
    let mut writer = Writer::verifying(limits.maximum_record_bytes, text, reader.budget);
    write_key(&mut writer, &key)?;
    let (_, budget) = writer.finish()?;
    Ok((key, budget))
}

fn validate_field_lengths(key: &PackageKey, maximum: usize) -> Result<(), Error> {
    let fits = |value: &str| value.len() <= maximum;
    let lineage_fits = match key.source_lineage() {
        SourceLineage::GitHub(lineage) => fits(lineage.owner()) && fits(lineage.repository()),
        SourceLineage::GitLab(lineage) => fits(lineage.repository_path()),
        SourceLineage::Git(lineage) => {
            fits(lineage.user().unwrap_or_default())
                && fits(lineage.host())
                && fits(lineage.repository_path())
        }
        SourceLineage::Workspace(lineage) => fits(lineage.member_path().as_str()),
        SourceLineage::ExternalLocal(lineage) => {
            lineage.canonical_absolute_path().to_str().is_some_and(fits)
        }
    };
    if fits(key.name().as_str()) && lineage_fits {
        Ok(())
    } else {
        Err(Error::new(
            "package-key text exceeds its identity field limit or requires UTF-8",
        ))
    }
}

#[cfg(test)]
#[path = "keys_tests.rs"]
mod tests;

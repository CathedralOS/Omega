//! Inert pending-review input, with no acceptance or compiler authority.
//!
//! Version 1 uses fixed-order LF rows, lowercase hexadecimal digests, and
//! canonical target identities. Targets are strictly ascending by
//! `target.target_name()` (not enum order).
//! Each `proposed-build N` / `source N` row precedes exactly N UTF-8 bytes and
//! one framing LF, even when the payload already ends in LF. `end\n` closes
//! the envelope. Kind and old-project digests only support stale-state checks.

mod framing;
#[cfg(test)]
mod tests;

use super::model::PackageCommandKind;
use crate::resolution::graph::{
    CanonicalSourceClosureSubject, CanonicalSourceClosureSubjectLimits,
};
use crate::resolution::source::git::workspace::MAX_BUILD_DECLARATION_BYTES;
use framing::{Reader, Writer, read_digest, write_digest};
use target::TargetProfile;

const HEADER: &str = "omega-package-proposal 1";
const MAXIMUM_TEXT_BYTES: usize = 128 * 1024 * 1024;
const MAXIMUM_TARGETS: usize = 32;

pub(super) struct PendingPackageChange {
    pub kind: PackageCommandKind,
    pub before_build: [u8; 32],
    pub before_lock: Option<[u8; 32]>,
    pub original_content: [u8; 32],
    pub proposed_build: String,
    pub source: CanonicalSourceClosureSubject,
    pub targets: Vec<TargetProfile>,
}

impl PendingPackageChange {
    pub(super) fn encode(&self) -> Result<String, String> {
        if self.proposed_build.len() > MAX_BUILD_DECLARATION_BYTES {
            return Err("package proposal build exceeds declaration byte limit".into());
        }
        validate_targets(&self.targets, self.source.target_profile())?;
        let source = self
            .source
            .canonical_text(CanonicalSourceClosureSubjectLimits::default())
            .map_err(|error| error.to_string())?;
        let mut writer = Writer::new(MAXIMUM_TEXT_BYTES);
        writer.append(HEADER)?;
        writer.append("\n")?;
        writer.row(
            "kind",
            match self.kind {
                PackageCommandKind::Install => "install",
                PackageCommandKind::Update => "update",
            },
        )?;
        writer.row("before-build", write_digest(&self.before_build))?;
        match self.before_lock {
            Some(digest) => writer.row("before-lock", write_digest(&digest))?,
            None => writer.row("before-lock", "absent")?,
        }
        writer.row("original-content", write_digest(&self.original_content))?;
        writer.row("targets", self.targets.len())?;
        for target in &self.targets {
            writer.row("target", target.identity().as_str())?;
        }
        writer.section("proposed-build", &self.proposed_build)?;
        writer.section("source", &source)?;
        writer.append("end\n")?;
        Ok(writer.finish())
    }

    /// Decode bounded data only. Source acquisition, live-project comparison,
    /// fresh compilation, and review decisions remain command operations.
    pub(super) fn recover(text: &str) -> Result<Self, String> {
        if text.len() > MAXIMUM_TEXT_BYTES {
            return Err("package proposal exceeds text byte limit".into());
        }
        let mut reader = Reader::new(text);
        reader.expect(HEADER)?;
        let kind = match reader.field("kind")? {
            "install" => PackageCommandKind::Install,
            "update" => PackageCommandKind::Update,
            _ => return Err("invalid package proposal kind".into()),
        };
        let before_build = read_digest(reader.field("before-build")?)?;
        let before_lock = match reader.field("before-lock")? {
            "absent" => None,
            digest => Some(read_digest(digest)?),
        };
        let original_content = read_digest(reader.field("original-content")?)?;
        let count = reader.count("targets", MAXIMUM_TARGETS)?;
        if count == 0 {
            return Err("package proposal targets must be nonempty".into());
        }
        // Keep untrusted target rows on the stack until every frame is checked.
        let mut targets = [TargetProfile::CrossPlatformCli; MAXIMUM_TARGETS];
        for target in &mut targets[..count] {
            let identity = reader.field("target")?;
            *target = TargetProfile::ALL
                .into_iter()
                .find(|profile| profile.identity().as_str() == identity)
                .ok_or_else(|| "unknown package proposal target".to_owned())?;
        }
        let targets = &targets[..count];
        validate_targets(targets, targets[0])?;
        let proposed_build = reader.section("proposed-build", MAX_BUILD_DECLARATION_BYTES)?;
        let source_limits = CanonicalSourceClosureSubjectLimits::default();
        let source_text = reader.section("source", source_limits.maximum_record_bytes)?;
        reader.expect("end")?;
        reader.finish()?;

        // The source codec checks its own counts, canonical text and graph
        // consistency. Successful recovery issues no compiler evidence.
        let source = CanonicalSourceClosureSubject::recover_text(source_text, source_limits)
            .map_err(|error| error.to_string())?;
        validate_targets(targets, source.target_profile())?;
        let mut owned_build = String::new();
        owned_build
            .try_reserve_exact(proposed_build.len())
            .map_err(|_| "package proposal allocation failed".to_owned())?;
        owned_build.push_str(proposed_build);
        let mut owned_targets = Vec::new();
        owned_targets
            .try_reserve_exact(count)
            .map_err(|_| "package proposal allocation failed".to_owned())?;
        owned_targets.extend_from_slice(targets);
        Ok(Self {
            kind,
            before_build,
            before_lock,
            original_content,
            proposed_build: owned_build,
            source,
            targets: owned_targets,
        })
    }
}

fn validate_targets(targets: &[TargetProfile], source_target: TargetProfile) -> Result<(), String> {
    if targets.is_empty() || targets.len() > MAXIMUM_TARGETS {
        return Err("package proposal target count exceeds bounds".into());
    }
    if targets
        .windows(2)
        .any(|pair| pair[0].target_name() >= pair[1].target_name())
    {
        return Err("package proposal targets are repeated or not canonically ordered".into());
    }
    if targets[0] != source_target {
        return Err("package proposal source target must be the canonical first target".into());
    }
    Ok(())
}

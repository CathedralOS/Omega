//! A pending package command retains its staged declaration, input selection
//! and editable Build-review findings. Once the Build has returned, the same
//! record also pins its resolved source graph for Product review and publication.

use super::PackageCommandKind;
use crate::hex_digest;
use crate::resolution::graph::{
    CanonicalSourceClosureSubject, CanonicalSourceClosureSubjectLimits,
};
use crate::resolution::source::git::workspace::MAX_BUILD_DECLARATION_BYTES;
use package_compilation::{BuildSourceCaptureObligation, BuildSourceCaptureRequest};
use std::collections::BTreeSet;
use target::TargetProfile;

// Proposal sections end with an LF, and every framing fault names its own
// failure point so a damaged proposal says what to discard.
type Reader<'text> = crate::framing::Reader<'text, &'static str>;
type Writer = crate::framing::Writer<&'static str>;

static PROPOSAL: crate::framing::Framing<&'static str> = crate::framing::Framing {
    errors: crate::framing::FramingErrors {
        byte_limit: "package proposal exceeds text byte limit",
        allocation: "package proposal allocation failed",
        missing_line: "invalid proposal framing",
        unexpected_line: "unexpected package proposal row",
        malformed_field: "invalid proposal field",
        unexpected_field: "unexpected package proposal field",
        noncanonical_count: "noncanonical package proposal count",
        count_overflow: "package proposal count overflow",
        count_limit: "package proposal count exceeds limit",
        section_length: "invalid package proposal payload length or UTF-8 boundary",
        missing_section_terminator: "missing proposal framing LF",
        trailing_bytes: "trailing package proposal bytes",
    },
    section_terminator: true,
};

fn write_digest(digest: &[u8; 32]) -> String {
    crate::Hex(digest).to_string()
}

fn read_digest(text: &str) -> Result<[u8; 32], String> {
    hex_digest(text).ok_or("package proposal digest must be 64 lowercase hexadecimal digits".into())
}

const HEADER: &str = "omega-package-proposal 3";
const MAXIMUM_TEXT_BYTES: usize = 128 * 1024 * 1024;
const MAXIMUM_TARGETS: usize = 32;
const MAXIMUM_BUILD_INPUTS: usize =
    build_filesystem::authority::CANONICAL_FILESYSTEM_METADATA_ROW_LIMIT - 1;
const MAXIMUM_BUILD_INPUT_PATH_BYTES: usize =
    build_filesystem::authority::FILESYSTEM_ROOT_RELATIVE_PATH_BYTE_LIMIT;

pub(super) struct PendingPackageChange {
    pub kind: PackageCommandKind,
    pub before_build: [u8; 32],
    pub before_lock: Option<[u8; 32]>,
    pub original_content: [u8; 32],
    pub proposed_build: String,
    pub source: Option<CanonicalSourceClosureSubject>,
    pub refresh: Option<BTreeSet<[u8; 32]>>,
    pub build_requests: super::restricted_build::Requests,
    pub targets: Vec<TargetProfile>,
    pub build_inputs: Option<BuildSourceCaptureRequest>,
}

impl PendingPackageChange {
    pub(super) fn resume_updates(
        &self,
        accepted: Option<&crate::lock::PackageLock>,
    ) -> Result<Option<Vec<crate::declarations::PackageKey>>, String> {
        let Some(requested) = &self.refresh else {
            return Ok(None);
        };
        if requested.is_empty() {
            return Ok(Some(Vec::new()));
        }
        let accepted = accepted.ok_or("selected package update has no accepted source graph")?;
        let keys = accepted.targets()[0]
            .source()
            .packages()
            .iter()
            .map(|package| package.key())
            .filter(|key| requested.contains(&key.identity().digest()))
            .cloned()
            .collect::<Vec<_>>();
        if keys.len() != requested.len() {
            return Err("package update selection differs from the accepted source graph".into());
        }
        Ok(Some(keys))
    }

    pub(super) fn encode(&self) -> Result<String, String> {
        if self.proposed_build.len() > MAX_BUILD_DECLARATION_BYTES {
            return Err("package proposal build exceeds declaration byte limit".into());
        }
        let first = self
            .targets
            .first()
            .copied()
            .ok_or("package proposal has no targets")?;
        validate_targets(
            &self.targets,
            self.source
                .as_ref()
                .map_or(first, |source| source.target_profile()),
        )?;
        let source = self
            .source
            .as_ref()
            .map(|source| source.canonical_text(CanonicalSourceClosureSubjectLimits::default()))
            .transpose()
            .map_err(|error| error.to_string())?;
        if source.is_none() && self.build_requests.is_empty() {
            return Err(
                "package proposal has neither a resolved graph nor pending Build requests".into(),
            );
        }
        let mut writer = Writer::new(&PROPOSAL, MAXIMUM_TEXT_BYTES);
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
        match &self.refresh {
            None => writer.row("refresh", "all")?,
            Some(keys) => {
                if keys.len() > CanonicalSourceClosureSubjectLimits::default().maximum_packages {
                    return Err("package update selection exceeds package limit".into());
                }
                writer.row("refresh", "selected")?;
                writer.row("refresh-count", keys.len())?;
                for key in keys {
                    writer.row("refresh-key", write_digest(key))?;
                }
            }
        }
        write_build_inputs(&mut writer, self.build_inputs.as_ref())?;
        writer.section("proposed-build", &self.proposed_build)?;
        if self.build_requests.len() > super::restricted_build::MAX_REQUESTS {
            return Err("too many restricted Build requests".into());
        }
        writer.row("build-requests", self.build_requests.len())?;
        for (key, description) in &self.build_requests {
            writer.row("build-request", write_digest(key))?;
            writer.section("description", description)?;
        }
        writer.section("source", source.as_deref().unwrap_or(""))?;
        writer.append("end\n")?;
        Ok(writer.finish()?)
    }

    pub(super) fn recover(text: &str) -> Result<Self, String> {
        if text.len() > MAXIMUM_TEXT_BYTES {
            return Err("package proposal exceeds text byte limit".into());
        }
        let mut reader = Reader::new(&PROPOSAL, text);
        reader.expect(HEADER).map_err(|_| {
            "unsupported or malformed package proposal header; use --discard-review, then start a fresh install/update review".to_owned()
        })?;
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
        let refresh = match reader.field("refresh")? {
            "all" => None,
            "selected" => {
                let count = reader.count(
                    "refresh-count",
                    CanonicalSourceClosureSubjectLimits::default().maximum_packages,
                )?;
                let mut keys = BTreeSet::new();
                for _ in 0..count {
                    let key = read_digest(reader.field("refresh-key")?)?;
                    if keys.last().is_some_and(|last| last >= &key) {
                        return Err("package update selections must be distinct and ordered".into());
                    }
                    keys.insert(key);
                }
                Some(keys)
            }
            _ => return Err("unknown package update selection".into()),
        };
        let build_inputs = read_build_inputs(&mut reader)?;
        let proposed_build = reader.section("proposed-build", MAX_BUILD_DECLARATION_BYTES)?;
        let mut build_requests = super::restricted_build::Requests::new();
        let count_requests =
            reader.count("build-requests", super::restricted_build::MAX_REQUESTS)?;
        for _ in 0..count_requests {
            let key = read_digest(reader.field("build-request")?)?;
            if build_requests
                .last_key_value()
                .is_some_and(|(last, _)| last >= &key)
            {
                return Err("Build requests must be distinct and ordered".into());
            }
            let description = reader.section("description", MAXIMUM_TEXT_BYTES)?;
            build_requests.insert(key, description.to_owned());
        }
        let source_limits = CanonicalSourceClosureSubjectLimits::default();
        let source_text = reader.section("source", source_limits.maximum_record_bytes)?;
        reader.expect("end")?;
        reader.finish()?;

        let source = if source_text.is_empty() {
            if build_requests.is_empty() {
                return Err("empty package review proposal".into());
            }
            None
        } else {
            if refresh.as_ref().is_none_or(|keys| !keys.is_empty()) {
                return Err(
                    "a resolved package proposal must preserve the selected revisions".into(),
                );
            }
            let source = CanonicalSourceClosureSubject::recover_text(source_text, source_limits)
                .map_err(|error| error.to_string())?;
            validate_targets(targets, source.target_profile())?;
            Some(source)
        };
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
            refresh,
            build_requests,
            targets: owned_targets,
            build_inputs,
        })
    }
}

fn write_build_inputs(
    writer: &mut Writer,
    request: Option<&BuildSourceCaptureRequest>,
) -> Result<(), String> {
    let Some(request) = request else {
        return Ok(writer.row("build-inputs", "absent")?);
    };
    let count = request.entries().count();
    if count > MAXIMUM_BUILD_INPUTS {
        return Err("package proposal build input count exceeds limit".into());
    }
    writer.row("build-inputs", "present")?;
    writer.row("build-input-count", count)?;
    let mut remaining_path_bytes = MAXIMUM_BUILD_INPUT_PATH_BYTES;
    for (path, obligation) in request.entries() {
        remaining_path_bytes = remaining_path_bytes
            .checked_sub(path.len())
            .ok_or("package proposal build input paths exceed byte limit")?;
        let path = std::str::from_utf8(path)
            .map_err(|_| "package proposal build input path must be UTF-8")?;
        writer.row(
            "build-input",
            match obligation {
                BuildSourceCaptureObligation::Required => "required",
                BuildSourceCaptureObligation::Optional => "optional",
            },
        )?;
        writer.section("build-input-path", path)?;
    }
    Ok(())
}

fn read_build_inputs(reader: &mut Reader<'_>) -> Result<Option<BuildSourceCaptureRequest>, String> {
    match reader.field("build-inputs")? {
        "absent" => return Ok(None),
        "present" => {}
        _ => return Err("invalid package proposal build input selection".into()),
    }
    let count = reader.count("build-input-count", MAXIMUM_BUILD_INPUTS)?;
    let mut entries = Vec::new();
    entries
        .try_reserve_exact(count)
        .map_err(|_| "package proposal allocation failed")?;
    let mut remaining_path_bytes = MAXIMUM_BUILD_INPUT_PATH_BYTES;
    let mut previous_path: Option<&str> = None;
    for _ in 0..count {
        let obligation = match reader.field("build-input")? {
            "required" => BuildSourceCaptureObligation::Required,
            "optional" => BuildSourceCaptureObligation::Optional,
            _ => return Err("invalid package proposal build input obligation".into()),
        };
        let path = reader.section("build-input-path", remaining_path_bytes)?;
        remaining_path_bytes -= path.len();
        if previous_path.is_some_and(|previous| previous.as_bytes() >= path.as_bytes()) {
            return Err(
                "package proposal build input paths are repeated or not canonically ordered".into(),
            );
        }
        previous_path = Some(path);
        let mut owned_path = Vec::new();
        owned_path
            .try_reserve_exact(path.len())
            .map_err(|_| "package proposal allocation failed")?;
        owned_path.extend_from_slice(path.as_bytes());
        entries.push((owned_path, obligation));
    }
    BuildSourceCaptureRequest::new(entries).map(Some)
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

#[cfg(test)]
mod tests {
    use super::*;

    // A proposal written before the framing was shared must still resume, and
    // a damaged one must still name the framing fault that refused it.
    #[test]
    fn a_proposal_keeps_its_canonical_bytes_and_framing_refusals() {
        let pending = PendingPackageChange {
            kind: PackageCommandKind::Update,
            before_build: [1; 32],
            before_lock: Some([4; 32]),
            original_content: [2; 32],
            proposed_build: "machine build(builder: @mutable Build) {}".into(),
            source: None,
            refresh: Some(BTreeSet::from([[7; 32]])),
            build_requests: [([3; 32], "one restricted Build action".into())].into(),
            targets: vec![TargetProfile::CrossPlatformCli],
            build_inputs: None,
        };
        let expected = concat!(
            "omega-package-proposal 3\n",
            "kind update\n",
            "before-build 0101010101010101010101010101010101010101010101010101010101010101\n",
            "before-lock 0404040404040404040404040404040404040404040404040404040404040404\n",
            "original-content 0202020202020202020202020202020202020202020202020202020202020202\n",
            "targets 1\n",
            "target omega.target-profile.v1:cross_platform_cli\n",
            "refresh selected\n",
            "refresh-count 1\n",
            "refresh-key 0707070707070707070707070707070707070707070707070707070707070707\n",
            "build-inputs absent\n",
            "proposed-build 41\n",
            "machine build(builder: @mutable Build) {}\n",
            "build-requests 1\n",
            "build-request 0303030303030303030303030303030303030303030303030303030303030303\n",
            "description 27\n",
            "one restricted Build action\n",
            "source 0\n",
            "\n",
            "end\n",
        );
        assert_eq!(pending.encode().unwrap(), expected);
        let restored = PendingPackageChange::recover(expected).unwrap();
        assert_eq!(restored.encode().unwrap(), expected);
        for (damaged, refusal) in [
            (
                expected.replace("targets 1", "targets 01"),
                "noncanonical package proposal count",
            ),
            (
                expected.replace("source 0\n\n", "source 0\n"),
                "missing proposal framing LF",
            ),
            (format!("{expected}x"), "trailing package proposal bytes"),
        ] {
            assert_eq!(
                PendingPackageChange::recover(&damaged).err().as_deref(),
                Some(refusal)
            );
        }
    }

    #[test]
    fn a_build_review_pause_keeps_the_requested_update_selection() {
        for refresh in [None, Some(BTreeSet::new()), Some(BTreeSet::from([[7; 32]]))] {
            let pending = PendingPackageChange {
                kind: PackageCommandKind::Update,
                before_build: [1; 32],
                before_lock: None,
                original_content: [2; 32],
                proposed_build: "machine build(builder: @mutable Build) {}".into(),
                source: None,
                refresh: refresh.clone(),
                build_requests: [([3; 32], "one restricted Build action".into())].into(),
                targets: vec![TargetProfile::CrossPlatformCli],
                build_inputs: None,
            };
            let restored = PendingPackageChange::recover(&pending.encode().unwrap()).unwrap();
            assert_eq!(restored.refresh, refresh);
            if refresh.as_ref().is_some_and(|keys| !keys.is_empty()) {
                assert!(restored.resume_updates(None).is_err());
            } else {
                assert_eq!(
                    restored.resume_updates(None).unwrap(),
                    refresh.map(|_| vec![])
                );
            }
        }
    }
}

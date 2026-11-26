//! A package may ask for a restricted operation before it has declared its
//! package name or dependencies. Review that checked Build first. The normal
//! graph and Product review publishes the resulting named policy later.
use super::{PackageCommandError, failure};
use package_compilation::build_execution::{
    RestrictedBuildGrants, RestrictedBuildRequest, RestrictedBuildSite,
};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;
use target::TargetProfile;

pub(super) const DOCUMENT: &str = "review-build.txt";
pub(super) const MAX_REQUESTS: usize = 4096;

type Key = [u8; 32];
pub(super) type Requests = BTreeMap<Key, String>;

/// These decisions belong to a transaction's exact staged root. Install this
/// gate only on that root's Build, including its selected successor entries.
#[derive(Clone)]
pub(crate) struct BuildPhaseGrants {
    context: Key,
    targets: Vec<TargetProfile>,
    accepted: Requests,
}

impl BuildPhaseGrants {
    pub(super) fn new(
        root: &std::path::Path,
        candidate: &[u8],
        targets: &[TargetProfile],
        accepted: Requests,
    ) -> Result<Self, PackageCommandError> {
        let mut scope = Sha256::new();
        frame(&mut scope, b"omega-package-root-build-review-v1");
        frame(&mut scope, root.as_os_str().as_encoded_bytes());
        frame(&mut scope, candidate);
        frame(
            &mut scope,
            &crate::dependencies::compiler_input_identity().map_err(failure)?,
        );
        for target in targets {
            frame(&mut scope, target.identity().as_str().as_bytes());
        }
        Ok(Self {
            context: scope.finalize().into(),
            targets: targets.to_vec(),
            accepted,
        })
    }

    pub(super) fn findings(
        &self,
        site: &RestrictedBuildSite,
        requests: &[RestrictedBuildRequest],
    ) -> Result<Requests, PackageCommandError> {
        let mut rows = Requests::new();
        let mut bytes = 0usize;
        for request in requests {
            for &target in &self.targets {
                let request = request.clone().for_target(Some(target));
                let key = self.key(site, &request).map_err(failure)?;
                let text = describe(site, &request).map_err(failure)?;
                bytes = bytes
                    .checked_add(text.len())
                    .filter(|&n| n <= super::state::LIMITS.maximum_bytes)
                    .ok_or_else(|| failure("Build review exceeds its byte limit"))?;
                rows.insert(key, text);
                if rows.len() > MAX_REQUESTS {
                    return Err(failure("too many restricted Build requests"));
                }
            }
        }
        Ok(rows)
    }

    fn key(
        &self,
        site: &RestrictedBuildSite,
        request: &RestrictedBuildRequest,
    ) -> Result<Key, String> {
        let meaning = review_evidence::encoding::restricted_build_request_payload(request)
            .map_err(|error| error.to_string())?;
        let mut hash = Sha256::new();
        frame(&mut hash, &self.context);
        frame(&mut hash, site.entry.as_bytes());
        frame(&mut hash, &site.program);
        frame(&mut hash, meaning.as_bytes());
        Ok(hash.finalize().into())
    }
}

impl RestrictedBuildGrants for BuildPhaseGrants {
    fn admits(
        &mut self,
        site: &RestrictedBuildSite,
        request: &RestrictedBuildRequest,
    ) -> Result<bool, Vec<diagnostics::Diagnostic>> {
        for &target in &self.targets {
            let request = request.clone().for_target(Some(target));
            let error = |why| vec![diagnostics::Diagnostic::error(why)];
            let key = self.key(site, &request).map_err(error)?;
            let description = describe(site, &request).map_err(error)?;
            if self.accepted.get(&key) != Some(&description) {
                return Ok(false);
            }
        }
        Ok(!self.targets.is_empty())
    }
}

fn describe(
    site: &RestrictedBuildSite,
    request: &RestrictedBuildRequest,
) -> Result<String, String> {
    let operation =
        crate::review::render_build_request(request, super::state::LIMITS.maximum_bytes)
            .map_err(|why| why.to_string())?;
    Ok(format!(
        "root Build entry {:?}\nchecked program {}\n{operation}",
        site.entry,
        crate::Hex(&site.program)
    ))
}

fn frame(digest: &mut Sha256, bytes: &[u8]) {
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
}

pub(super) fn render(
    rows: &Requests,
    accepted: &BTreeSet<Key>,
) -> Result<String, PackageCommandError> {
    let mut text = String::from(
        "Restricted Build review. Generated-source and final Product review are incomplete.\n",
    );
    for (key, row) in rows {
        let extra = row
            .len()
            .checked_add(row.lines().count().saturating_mul(3))
            .and_then(|n| n.checked_add(120))
            .ok_or_else(|| failure("Build review length overflow"))?;
        if text.len().saturating_add(extra) > super::state::LIMITS.maximum_bytes {
            return Err(failure("Build review exceeds its byte limit"));
        }
        text.try_reserve(extra).map_err(failure)?;
        for line in row.lines() {
            writeln!(text, "  {line}").map_err(failure)?;
        }
        let decision = if accepted.contains(key) {
            "accept"
        } else {
            "pending"
        };
        writeln!(text, "decision build {} {decision}", crate::Hex(key)).map_err(failure)?;
    }
    Ok(text)
}

pub(super) fn decisions(
    rows: &Requests,
    edited: &str,
) -> Result<BTreeSet<Key>, PackageCommandError> {
    let expected = render(rows, &BTreeSet::new())?;
    let changed = || failure("Build findings changed; edit only the decision words");
    if !edited.ends_with('\n') || edited.contains('\r') {
        return Err(changed());
    }
    let mut lines = edited.lines();
    let mut accepted = BTreeSet::new();
    let mut keys = rows.keys();
    for expected in expected.lines() {
        let line = lines.next().ok_or_else(changed)?;
        if expected.starts_with("decision build ") {
            let prefix = expected.strip_suffix("pending").ok_or_else(changed)?;
            let key = *keys.next().ok_or_else(changed)?;
            match line.strip_prefix(prefix) {
                Some("accept") => {
                    accepted.insert(key);
                }
                Some("pending" | "reject") => {}
                _ => return Err(changed()),
            }
        } else if line != expected {
            return Err(changed());
        }
    }
    if lines.next().is_some() || keys.next().is_some() {
        return Err(changed());
    }
    Ok(accepted)
}

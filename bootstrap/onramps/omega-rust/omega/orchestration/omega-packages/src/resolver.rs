use crate::source::{
    GitSourceSpec, LocalSourceLimits, SourceResolveError, resolve_git_source, resolve_local_source,
};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

pub const SOURCE_CACHE_POLICY_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceCacheRequest {
    LocalPath(PathBuf),
    Git { url: String, rev: Option<String> },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SourceCacheVerdict {
    Accepted,
    Rejected,
}

impl SourceCacheVerdict {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Accepted => "accepted",
            Self::Rejected => "rejected",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceCachePolicyRecord {
    pub schema_version: u32,
    pub verdict: SourceCacheVerdict,
    pub source_kind: String,
    pub locator: String,
    pub requested_rev: Option<String>,
    pub resolved_commit: Option<String>,
    pub resolved_tree: Option<String>,
    pub content_identity: Option<String>,
    pub cache_path: Option<String>,
    pub file_count: Option<usize>,
    pub byte_count: Option<u64>,
    pub max_files: usize,
    pub max_bytes: u64,
    pub max_depth: usize,
    pub submodule_policy: String,
    pub path_policy: String,
    pub rejection: Option<String>,
}

impl SourceCachePolicyRecord {
    pub fn fingerprint(&self) -> String {
        format_sha256(&Sha256::digest(self.to_json().as_bytes()))
    }

    pub fn to_json(&self) -> String {
        let mut json = String::new();
        json.push_str("{\n");
        push_number_field(&mut json, 1, "schema_version", self.schema_version, true);
        push_string_field(&mut json, 1, "verdict", self.verdict.as_str(), true);
        push_string_field(&mut json, 1, "source_kind", &self.source_kind, true);
        push_string_field(&mut json, 1, "locator", &self.locator, true);
        push_optional_string_field(
            &mut json,
            1,
            "requested_rev",
            self.requested_rev.as_deref(),
            true,
        );
        push_optional_string_field(
            &mut json,
            1,
            "resolved_commit",
            self.resolved_commit.as_deref(),
            true,
        );
        push_optional_string_field(
            &mut json,
            1,
            "resolved_tree",
            self.resolved_tree.as_deref(),
            true,
        );
        push_optional_string_field(
            &mut json,
            1,
            "content_identity",
            self.content_identity.as_deref(),
            true,
        );
        push_optional_string_field(&mut json, 1, "cache_path", self.cache_path.as_deref(), true);
        push_optional_usize_field(&mut json, 1, "file_count", self.file_count, true);
        push_optional_u64_field(&mut json, 1, "byte_count", self.byte_count, true);
        push_usize_field(&mut json, 1, "max_files", self.max_files, true);
        push_u64_field(&mut json, 1, "max_bytes", self.max_bytes, true);
        push_usize_field(&mut json, 1, "max_depth", self.max_depth, true);
        push_string_field(
            &mut json,
            1,
            "submodule_policy",
            &self.submodule_policy,
            true,
        );
        push_string_field(&mut json, 1, "path_policy", &self.path_policy, true);
        push_optional_string_field(&mut json, 1, "rejection", self.rejection.as_deref(), false);
        json.push_str("}\n");
        json
    }
}

pub fn resolve_source_cache_record(
    request: SourceCacheRequest,
    cache_dir: impl AsRef<Path>,
    limits: LocalSourceLimits,
) -> SourceCachePolicyRecord {
    match request {
        SourceCacheRequest::LocalPath(path) => match resolve_local_source(&path, limits) {
            Ok(resolved) => SourceCachePolicyRecord {
                schema_version: SOURCE_CACHE_POLICY_SCHEMA_VERSION,
                verdict: SourceCacheVerdict::Accepted,
                source_kind: "local-path".to_owned(),
                locator: path.display().to_string(),
                requested_rev: None,
                resolved_commit: None,
                resolved_tree: None,
                content_identity: Some(resolved.content_identity),
                cache_path: Some(resolved.root.display().to_string()),
                file_count: Some(resolved.file_count),
                byte_count: Some(resolved.byte_count),
                max_files: limits.max_files,
                max_bytes: limits.max_bytes,
                max_depth: limits.max_depth,
                submodule_policy: "git-submodules-not-applicable".to_owned(),
                path_policy: "canonical-root-contained; symlink-escapes-rejected; dot-git-excluded"
                    .to_owned(),
                rejection: None,
            },
            Err(error) => rejected_record(
                "local-path",
                path.display().to_string(),
                None,
                limits,
                error,
            ),
        },
        SourceCacheRequest::Git { url, rev } => {
            let requested_rev = rev.clone().unwrap_or_else(|| "HEAD".to_owned());
            match resolve_git_source(
                &GitSourceSpec {
                    url: url.clone(),
                    rev,
                },
                cache_dir,
                limits,
            ) {
                Ok(resolved) => SourceCachePolicyRecord {
                    schema_version: SOURCE_CACHE_POLICY_SCHEMA_VERSION,
                    verdict: SourceCacheVerdict::Accepted,
                    source_kind: "git".to_owned(),
                    locator: url,
                    requested_rev: Some(resolved.requested_rev),
                    resolved_commit: Some(resolved.commit),
                    resolved_tree: Some(resolved.tree),
                    content_identity: Some(resolved.local.content_identity),
                    cache_path: Some(resolved.checkout_root.display().to_string()),
                    file_count: Some(resolved.local.file_count),
                    byte_count: Some(resolved.local.byte_count),
                    max_files: limits.max_files,
                    max_bytes: limits.max_bytes,
                    max_depth: limits.max_depth,
                    submodule_policy: "gitmodules-rejected-until-submodules-are-explicit-package-edges"
                        .to_owned(),
                    path_policy:
                        "detached-commit-checkout; canonical-root-contained; symlink-escapes-rejected; dot-git-excluded"
                            .to_owned(),
                    rejection: None,
                },
                Err(error) => rejected_record("git", url, Some(requested_rev), limits, error),
            }
        }
    }
}

fn rejected_record(
    source_kind: &str,
    locator: String,
    requested_rev: Option<String>,
    limits: LocalSourceLimits,
    error: SourceResolveError,
) -> SourceCachePolicyRecord {
    SourceCachePolicyRecord {
        schema_version: SOURCE_CACHE_POLICY_SCHEMA_VERSION,
        verdict: SourceCacheVerdict::Rejected,
        source_kind: source_kind.to_owned(),
        locator,
        requested_rev,
        resolved_commit: None,
        resolved_tree: None,
        content_identity: None,
        cache_path: None,
        file_count: None,
        byte_count: None,
        max_files: limits.max_files,
        max_bytes: limits.max_bytes,
        max_depth: limits.max_depth,
        submodule_policy: if source_kind == "git" {
            "gitmodules-rejected-until-submodules-are-explicit-package-edges".to_owned()
        } else {
            "git-submodules-not-applicable".to_owned()
        },
        path_policy: "canonical-root-contained; symlink-escapes-rejected; dot-git-excluded"
            .to_owned(),
        rejection: Some(error.to_string()),
    }
}

fn push_number_field(json: &mut String, indent: usize, name: &str, value: u32, comma: bool) {
    push_indent(json, indent);
    push_json_string(json, name);
    json.push_str(": ");
    json.push_str(&value.to_string());
    if comma {
        json.push(',');
    }
    json.push('\n');
}

fn push_u64_field(json: &mut String, indent: usize, name: &str, value: u64, comma: bool) {
    push_indent(json, indent);
    push_json_string(json, name);
    json.push_str(": ");
    json.push_str(&value.to_string());
    if comma {
        json.push(',');
    }
    json.push('\n');
}

fn push_usize_field(json: &mut String, indent: usize, name: &str, value: usize, comma: bool) {
    push_indent(json, indent);
    push_json_string(json, name);
    json.push_str(": ");
    json.push_str(&value.to_string());
    if comma {
        json.push(',');
    }
    json.push('\n');
}

fn push_string_field(json: &mut String, indent: usize, name: &str, value: &str, comma: bool) {
    push_indent(json, indent);
    push_json_string(json, name);
    json.push_str(": ");
    push_json_string(json, value);
    if comma {
        json.push(',');
    }
    json.push('\n');
}

fn push_optional_string_field(
    json: &mut String,
    indent: usize,
    name: &str,
    value: Option<&str>,
    comma: bool,
) {
    push_indent(json, indent);
    push_json_string(json, name);
    json.push_str(": ");
    if let Some(value) = value {
        push_json_string(json, value);
    } else {
        json.push_str("null");
    }
    if comma {
        json.push(',');
    }
    json.push('\n');
}

fn push_optional_usize_field(
    json: &mut String,
    indent: usize,
    name: &str,
    value: Option<usize>,
    comma: bool,
) {
    push_indent(json, indent);
    push_json_string(json, name);
    json.push_str(": ");
    if let Some(value) = value {
        json.push_str(&value.to_string());
    } else {
        json.push_str("null");
    }
    if comma {
        json.push(',');
    }
    json.push('\n');
}

fn push_optional_u64_field(
    json: &mut String,
    indent: usize,
    name: &str,
    value: Option<u64>,
    comma: bool,
) {
    push_indent(json, indent);
    push_json_string(json, name);
    json.push_str(": ");
    if let Some(value) = value {
        json.push_str(&value.to_string());
    } else {
        json.push_str("null");
    }
    if comma {
        json.push(',');
    }
    json.push('\n');
}

fn push_json_string(json: &mut String, value: &str) {
    json.push('"');
    for ch in value.chars() {
        match ch {
            '"' => json.push_str("\\\""),
            '\\' => json.push_str("\\\\"),
            '\n' => json.push_str("\\n"),
            '\r' => json.push_str("\\r"),
            '\t' => json.push_str("\\t"),
            ch if ch.is_control() => json.push_str(&format!("\\u{:04x}", ch as u32)),
            ch => json.push(ch),
        }
    }
    json.push('"');
}

fn push_indent(json: &mut String, level: usize) {
    for _ in 0..level {
        json.push_str("  ");
    }
}

fn format_sha256(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(64);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "omega-source-cache-record-{name}-{}-{stamp}",
            std::process::id()
        ))
    }

    fn run_test_git<I, S>(directory: &Path, args: I)
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let output = Command::new("git")
            .current_dir(directory)
            .args(args)
            .output()
            .expect("spawn git");
        assert!(
            output.status.success(),
            "git command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn local_source_cache_record_captures_limits_and_identity() {
        let root = temp_root("local");
        let cache = temp_root("cache");
        std::fs::create_dir_all(&root).expect("create source");
        std::fs::write(root.join("main.omg"), "machine Main::main() {}\n").expect("write source");
        let limits = LocalSourceLimits {
            max_files: 8,
            max_bytes: 1024,
            max_depth: 8,
        };

        let record = resolve_source_cache_record(
            SourceCacheRequest::LocalPath(root.clone()),
            &cache,
            limits,
        );

        assert_eq!(record.verdict, SourceCacheVerdict::Accepted);
        assert_eq!(record.source_kind, "local-path");
        assert_eq!(record.file_count, Some(1));
        assert_eq!(record.max_files, 8);
        assert_eq!(
            record.content_identity.as_ref().expect("identity").len(),
            64
        );
        assert!(
            record
                .to_json()
                .contains("\"submodule_policy\": \"git-submodules-not-applicable\"")
        );
        assert_eq!(record.fingerprint().len(), 64);

        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&cache);
    }

    #[test]
    fn rejected_source_cache_record_captures_policy_failure() {
        let root = temp_root("reject");
        let cache = temp_root("cache");
        std::fs::create_dir_all(&root).expect("create source");
        std::fs::write(root.join("main.omg"), "").expect("write source");

        let record = resolve_source_cache_record(
            SourceCacheRequest::LocalPath(root.clone()),
            &cache,
            LocalSourceLimits {
                max_files: 0,
                ..LocalSourceLimits::default()
            },
        );

        assert_eq!(record.verdict, SourceCacheVerdict::Rejected);
        assert!(record.content_identity.is_none());
        assert!(
            record
                .rejection
                .as_deref()
                .expect("rejection")
                .contains("file limit")
        );

        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&cache);
    }

    #[test]
    fn git_source_cache_record_captures_commit_tree_and_submodule_policy() {
        let repo = temp_root("git");
        let cache = temp_root("git-cache");
        std::fs::create_dir_all(&repo).expect("create git package");
        run_test_git(&repo, ["init", "--quiet"]);
        run_test_git(&repo, ["config", "user.email", "omega@example.invalid"]);
        run_test_git(&repo, ["config", "user.name", "Omega Tests"]);
        std::fs::write(repo.join("main.omg"), "machine Main::main() {}\n").expect("write source");
        run_test_git(&repo, ["add", "main.omg"]);
        run_test_git(&repo, ["commit", "--quiet", "-m", "initial"]);

        let record = resolve_source_cache_record(
            SourceCacheRequest::Git {
                url: repo.display().to_string(),
                rev: Some("HEAD".to_owned()),
            },
            &cache,
            LocalSourceLimits::default(),
        );

        assert_eq!(record.verdict, SourceCacheVerdict::Accepted);
        assert_eq!(record.source_kind, "git");
        assert_eq!(record.requested_rev.as_deref(), Some("HEAD"));
        assert_eq!(record.resolved_commit.as_ref().expect("commit").len(), 40);
        assert_eq!(record.resolved_tree.as_ref().expect("tree").len(), 40);
        assert!(record.submodule_policy.contains("gitmodules-rejected"));

        let _ = std::fs::remove_dir_all(&repo);
        let _ = std::fs::remove_dir_all(&cache);
    }
}

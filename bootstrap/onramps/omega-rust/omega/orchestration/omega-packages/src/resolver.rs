use crate::json::{JsonParseError, JsonParser, JsonValue};
use crate::source::{
    GitSourceSpec, LocalSourceLimits, SourceResolveError, resolve_git_source, resolve_local_source,
};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

pub const SOURCE_CACHE_POLICY_SCHEMA_VERSION: u32 = 2;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceCacheRequest {
    LocalPath(PathBuf),
    Git { url: String, rev: Option<String> },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SourceCacheVerdict {
    DiagnosticObserved,
    Rejected,
}

impl SourceCacheVerdict {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DiagnosticObserved => "diagnostic-observed",
            Self::Rejected => "rejected",
        }
    }
}

/// Legacy machine-readable source diagnostics.
///
/// This value records what the exploratory resolver observed. It is not a
/// `SourceResolutionReceipt`, cannot authorize compilation or lock mutation,
/// and deliberately has no accepted verdict.
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceCachePolicyRecordParseError {
    InvalidJson { message: String },
    MissingField { field: String },
    UnexpectedField { field: String },
    InvalidField { field: String, message: String },
    UnsupportedSchemaVersion { found: u32, supported: u32 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceCachePolicyRecordPersistenceError {
    Io { path: PathBuf, message: String },
    Parse(SourceCachePolicyRecordParseError),
}

impl SourceCachePolicyRecord {
    pub fn fingerprint(&self) -> String {
        format_sha256(&Sha256::digest(self.to_json().as_bytes()))
    }

    pub fn from_json(json: &str) -> Result<Self, SourceCachePolicyRecordParseError> {
        let value = JsonParser::new(json)
            .parse()
            .map_err(source_cache_policy_json_error)?;
        let fields = value_object(&value, "$")?;
        ensure_fields(
            fields,
            "$",
            &[
                "schema_version",
                "verdict",
                "source_kind",
                "locator",
                "requested_rev",
                "resolved_commit",
                "resolved_tree",
                "content_identity",
                "cache_path",
                "file_count",
                "byte_count",
                "max_files",
                "max_bytes",
                "max_depth",
                "submodule_policy",
                "path_policy",
                "rejection",
            ],
        )?;
        let schema_version = value_u32(field(fields, "schema_version", "$")?, "$.schema_version")?;
        if schema_version != SOURCE_CACHE_POLICY_SCHEMA_VERSION {
            return Err(
                SourceCachePolicyRecordParseError::UnsupportedSchemaVersion {
                    found: schema_version,
                    supported: SOURCE_CACHE_POLICY_SCHEMA_VERSION,
                },
            );
        }
        let verdict = parse_verdict(value_string(field(fields, "verdict", "$")?, "$.verdict")?)?;
        Ok(Self {
            schema_version,
            verdict,
            source_kind: value_string(field(fields, "source_kind", "$")?, "$.source_kind")?
                .to_owned(),
            locator: value_string(field(fields, "locator", "$")?, "$.locator")?.to_owned(),
            requested_rev: optional_string(field(fields, "requested_rev", "$")?)?,
            resolved_commit: optional_string(field(fields, "resolved_commit", "$")?)?,
            resolved_tree: optional_string(field(fields, "resolved_tree", "$")?)?,
            content_identity: optional_string(field(fields, "content_identity", "$")?)?,
            cache_path: optional_string(field(fields, "cache_path", "$")?)?,
            file_count: optional_usize(field(fields, "file_count", "$")?, "$.file_count")?,
            byte_count: optional_u64(field(fields, "byte_count", "$")?, "$.byte_count")?,
            max_files: value_usize(field(fields, "max_files", "$")?, "$.max_files")?,
            max_bytes: value_u64(field(fields, "max_bytes", "$")?, "$.max_bytes")?,
            max_depth: value_usize(field(fields, "max_depth", "$")?, "$.max_depth")?,
            submodule_policy: value_string(
                field(fields, "submodule_policy", "$")?,
                "$.submodule_policy",
            )?
            .to_owned(),
            path_policy: value_string(field(fields, "path_policy", "$")?, "$.path_policy")?
                .to_owned(),
            rejection: optional_string(field(fields, "rejection", "$")?)?,
        })
    }

    pub fn read_from_path(
        path: impl AsRef<Path>,
    ) -> Result<Self, SourceCachePolicyRecordPersistenceError> {
        let path = path.as_ref();
        let contents = fs::read_to_string(path).map_err(|error| {
            SourceCachePolicyRecordPersistenceError::Io {
                path: path.to_path_buf(),
                message: error.to_string(),
            }
        })?;
        Self::from_json(&contents).map_err(SourceCachePolicyRecordPersistenceError::Parse)
    }

    pub fn write_to_path(
        &self,
        path: impl AsRef<Path>,
    ) -> Result<(), SourceCachePolicyRecordPersistenceError> {
        let path = path.as_ref();
        let temp_path = temporary_source_cache_policy_path(path, self);
        fs::write(&temp_path, self.to_json()).map_err(|error| {
            SourceCachePolicyRecordPersistenceError::Io {
                path: temp_path.clone(),
                message: error.to_string(),
            }
        })?;
        if let Err(error) = fs::rename(&temp_path, path) {
            let _ = fs::remove_file(&temp_path);
            return Err(SourceCachePolicyRecordPersistenceError::Io {
                path: path.to_path_buf(),
                message: error.to_string(),
            });
        }
        Ok(())
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

fn source_cache_policy_json_error(error: JsonParseError) -> SourceCachePolicyRecordParseError {
    match error {
        JsonParseError::InvalidJson { message } => {
            SourceCachePolicyRecordParseError::InvalidJson { message }
        }
    }
}

fn parse_verdict(value: &str) -> Result<SourceCacheVerdict, SourceCachePolicyRecordParseError> {
    match value {
        "diagnostic-observed" => Ok(SourceCacheVerdict::DiagnosticObserved),
        "rejected" => Ok(SourceCacheVerdict::Rejected),
        _ => Err(SourceCachePolicyRecordParseError::InvalidField {
            field: "$.verdict".to_owned(),
            message: format!("unsupported source-cache verdict `{value}`"),
        }),
    }
}

fn ensure_fields(
    fields: &[(String, JsonValue)],
    path: &str,
    expected: &[&str],
) -> Result<(), SourceCachePolicyRecordParseError> {
    for expected_field in expected {
        if !fields.iter().any(|(name, _)| name == expected_field) {
            return Err(SourceCachePolicyRecordParseError::MissingField {
                field: format!("{path}.{expected_field}"),
            });
        }
    }
    for (name, _) in fields {
        if !expected.iter().any(|expected| expected == name) {
            return Err(SourceCachePolicyRecordParseError::UnexpectedField {
                field: format!("{path}.{name}"),
            });
        }
    }
    Ok(())
}

fn field<'a>(
    fields: &'a [(String, JsonValue)],
    name: &str,
    path: &str,
) -> Result<&'a JsonValue, SourceCachePolicyRecordParseError> {
    fields
        .iter()
        .find(|(field, _)| field == name)
        .map(|(_, value)| value)
        .ok_or_else(|| SourceCachePolicyRecordParseError::MissingField {
            field: format!("{path}.{name}"),
        })
}

fn value_object<'a>(
    value: &'a JsonValue,
    path: &str,
) -> Result<&'a [(String, JsonValue)], SourceCachePolicyRecordParseError> {
    value
        .as_object()
        .ok_or_else(|| SourceCachePolicyRecordParseError::InvalidField {
            field: path.to_owned(),
            message: "expected object".to_owned(),
        })
}

fn value_string<'a>(
    value: &'a JsonValue,
    path: &str,
) -> Result<&'a str, SourceCachePolicyRecordParseError> {
    match value {
        JsonValue::String(value) => Ok(value),
        _ => Err(SourceCachePolicyRecordParseError::InvalidField {
            field: path.to_owned(),
            message: "expected string".to_owned(),
        }),
    }
}

fn optional_string(value: &JsonValue) -> Result<Option<String>, SourceCachePolicyRecordParseError> {
    match value {
        JsonValue::Null => Ok(None),
        JsonValue::String(value) => Ok(Some(value.clone())),
        _ => Err(SourceCachePolicyRecordParseError::InvalidField {
            field: "optional string".to_owned(),
            message: "expected string or null".to_owned(),
        }),
    }
}

fn value_u64(value: &JsonValue, path: &str) -> Result<u64, SourceCachePolicyRecordParseError> {
    match value {
        JsonValue::Number(value) => Ok(*value),
        _ => Err(SourceCachePolicyRecordParseError::InvalidField {
            field: path.to_owned(),
            message: "expected integer".to_owned(),
        }),
    }
}

fn value_u32(value: &JsonValue, path: &str) -> Result<u32, SourceCachePolicyRecordParseError> {
    let value = value_u64(value, path)?;
    u32::try_from(value).map_err(|_| SourceCachePolicyRecordParseError::InvalidField {
        field: path.to_owned(),
        message: "integer does not fit in u32".to_owned(),
    })
}

fn value_usize(value: &JsonValue, path: &str) -> Result<usize, SourceCachePolicyRecordParseError> {
    let value = value_u64(value, path)?;
    usize::try_from(value).map_err(|_| SourceCachePolicyRecordParseError::InvalidField {
        field: path.to_owned(),
        message: "integer does not fit in usize".to_owned(),
    })
}

fn optional_u64(
    value: &JsonValue,
    path: &str,
) -> Result<Option<u64>, SourceCachePolicyRecordParseError> {
    match value {
        JsonValue::Null => Ok(None),
        _ => value_u64(value, path).map(Some),
    }
}

fn optional_usize(
    value: &JsonValue,
    path: &str,
) -> Result<Option<usize>, SourceCachePolicyRecordParseError> {
    match value {
        JsonValue::Null => Ok(None),
        _ => value_usize(value, path).map(Some),
    }
}

fn temporary_source_cache_policy_path(path: &Path, record: &SourceCachePolicyRecord) -> PathBuf {
    let mut temp = path.to_path_buf();
    temp.set_extension(format!(
        "tmp.{}.{}",
        std::process::id(),
        &record.fingerprint()[..12]
    ));
    temp
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
                verdict: SourceCacheVerdict::DiagnosticObserved,
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
                    verdict: SourceCacheVerdict::DiagnosticObserved,
                    source_kind: "git".to_owned(),
                    locator: url,
                    requested_rev: Some(resolved.requested_rev),
                    resolved_commit: Some(resolved.commit),
                    resolved_tree: Some(resolved.tree),
                    content_identity: Some(resolved.local.content_identity),
                    cache_path: Some(resolved.snapshot_root.display().to_string()),
                    file_count: Some(resolved.local.file_count),
                    byte_count: Some(resolved.local.byte_count),
                    max_files: limits.max_files,
                    max_bytes: limits.max_bytes,
                    max_depth: limits.max_depth,
                    submodule_policy: "gitmodules-rejected-until-submodules-are-explicit-package-edges"
                        .to_owned(),
                    path_policy:
                        "validated-object-snapshot; canonical-root-contained; symlink-escapes-rejected; dot-git-excluded"
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

        assert_eq!(record.verdict, SourceCacheVerdict::DiagnosticObserved);
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
    fn source_cache_policy_record_json_round_trip_is_normalized() {
        let record = SourceCachePolicyRecord {
            schema_version: SOURCE_CACHE_POLICY_SCHEMA_VERSION,
            verdict: SourceCacheVerdict::DiagnosticObserved,
            source_kind: "git".to_owned(),
            locator: "git@github.com:CathedralOS/file-journal.git".to_owned(),
            requested_rev: Some("0123456789abcdef0123456789abcdef01234567".to_owned()),
            resolved_commit: Some("0123456789abcdef0123456789abcdef01234567".to_owned()),
            resolved_tree: Some("89abcdef0123456789abcdef0123456789abcdef".to_owned()),
            content_identity: Some(
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned(),
            ),
            cache_path: Some("/tmp/omega-cache/file-journal".to_owned()),
            file_count: Some(3),
            byte_count: Some(1024),
            max_files: 4096,
            max_bytes: 268435456,
            max_depth: 64,
            submodule_policy: "gitmodules-rejected-until-submodules-are-explicit-package-edges"
                .to_owned(),
            path_policy:
                "validated-object-snapshot; canonical-root-contained; symlink-escapes-rejected; dot-git-excluded"
                    .to_owned(),
            rejection: None,
        };

        let parsed = SourceCachePolicyRecord::from_json(&record.to_json())
            .expect("record JSON should parse");

        assert_eq!(parsed, record);
        assert_eq!(parsed.to_json(), record.to_json());
        assert_eq!(parsed.fingerprint(), record.fingerprint());
    }

    #[test]
    fn source_cache_policy_record_read_write_round_trip() {
        let root = temp_root("persist-record");
        std::fs::create_dir_all(&root).expect("create record temp");
        let path = root.join("source-cache-policy.json");
        let record = SourceCachePolicyRecord {
            schema_version: SOURCE_CACHE_POLICY_SCHEMA_VERSION,
            verdict: SourceCacheVerdict::Rejected,
            source_kind: "local-path".to_owned(),
            locator: "./missing-package".to_owned(),
            requested_rev: None,
            resolved_commit: None,
            resolved_tree: None,
            content_identity: None,
            cache_path: None,
            file_count: None,
            byte_count: None,
            max_files: 4096,
            max_bytes: 268435456,
            max_depth: 64,
            submodule_policy: "git-submodules-not-applicable".to_owned(),
            path_policy: "canonical-root-contained; symlink-escapes-rejected; dot-git-excluded"
                .to_owned(),
            rejection: Some("missing source".to_owned()),
        };

        record.write_to_path(&path).expect("write policy record");
        let read = SourceCachePolicyRecord::read_from_path(&path).expect("read policy record");

        assert_eq!(read, record);
        assert_eq!(
            std::fs::read_to_string(&path).expect("record file"),
            record.to_json()
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn source_cache_policy_record_parse_rejects_unknown_schema_and_fields() {
        let unknown_schema = "{\n  \"schema_version\": 99,\n  \"verdict\": \"accepted\",\n  \"source_kind\": \"local-path\",\n  \"locator\": \".\",\n  \"requested_rev\": null,\n  \"resolved_commit\": null,\n  \"resolved_tree\": null,\n  \"content_identity\": null,\n  \"cache_path\": null,\n  \"file_count\": null,\n  \"byte_count\": null,\n  \"max_files\": 4096,\n  \"max_bytes\": 268435456,\n  \"max_depth\": 64,\n  \"submodule_policy\": \"git-submodules-not-applicable\",\n  \"path_policy\": \"canonical-root-contained\",\n  \"rejection\": null\n}\n";
        assert_eq!(
            SourceCachePolicyRecord::from_json(unknown_schema),
            Err(
                SourceCachePolicyRecordParseError::UnsupportedSchemaVersion {
                    found: 99,
                    supported: SOURCE_CACHE_POLICY_SCHEMA_VERSION,
                }
            )
        );

        let unexpected_field = "{\n  \"schema_version\": 2,\n  \"verdict\": \"diagnostic-observed\",\n  \"source_kind\": \"local-path\",\n  \"locator\": \".\",\n  \"requested_rev\": null,\n  \"resolved_commit\": null,\n  \"resolved_tree\": null,\n  \"content_identity\": null,\n  \"cache_path\": null,\n  \"file_count\": null,\n  \"byte_count\": null,\n  \"max_files\": 4096,\n  \"max_bytes\": 268435456,\n  \"max_depth\": 64,\n  \"submodule_policy\": \"git-submodules-not-applicable\",\n  \"path_policy\": \"canonical-root-contained\",\n  \"rejection\": null,\n  \"extra\": \"no\"\n}\n";
        assert_eq!(
            SourceCachePolicyRecord::from_json(unexpected_field),
            Err(SourceCachePolicyRecordParseError::UnexpectedField {
                field: "$.extra".to_owned(),
            })
        );
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

        assert_eq!(record.verdict, SourceCacheVerdict::DiagnosticObserved);
        assert_eq!(record.source_kind, "git");
        assert_eq!(record.requested_rev.as_deref(), Some("HEAD"));
        assert_eq!(record.resolved_commit.as_ref().expect("commit").len(), 40);
        assert_eq!(record.resolved_tree.as_ref().expect("tree").len(), 40);
        assert!(record.submodule_policy.contains("gitmodules-rejected"));

        let _ = std::fs::remove_dir_all(&repo);
        let _ = std::fs::remove_dir_all(&cache);
    }
}

use crate::diff::{ManifestDiff, ManifestSeverity};
use crate::json::{JsonParseError, JsonParser, JsonValue};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

pub const CAPABILITY_CHANGE_RECEIPT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct AcceptedManifestDelta {
    pub section: String,
    pub severity: ManifestSeverity,
    pub old_fingerprint: String,
    pub new_fingerprint: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityChangeReceipt {
    pub schema_version: u32,
    pub reviewer: String,
    pub reason: String,
    pub old_source_identity: String,
    pub new_source_identity: String,
    pub old_manifest_fingerprint: String,
    pub new_manifest_fingerprint: String,
    pub accepted_deltas: Vec<AcceptedManifestDelta>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapabilityReviewError {
    EmptyDiff,
    MissingReviewer,
    MissingReason,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapabilityChangeReceiptParseError {
    InvalidJson { message: String },
    MissingField { field: String },
    UnexpectedField { field: String },
    InvalidField { field: String, message: String },
    UnsupportedSchemaVersion { found: u32, supported: u32 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapabilityChangeReceiptPersistenceError {
    Io { path: PathBuf, message: String },
    Parse(CapabilityChangeReceiptParseError),
}

impl CapabilityChangeReceipt {
    pub fn from_diff(
        diff: &ManifestDiff,
        old_source_identity: impl Into<String>,
        new_source_identity: impl Into<String>,
        reviewer: impl Into<String>,
        reason: impl Into<String>,
    ) -> Result<Self, CapabilityReviewError> {
        if diff.is_empty() {
            return Err(CapabilityReviewError::EmptyDiff);
        }
        let reviewer = reviewer.into();
        if reviewer.trim().is_empty() {
            return Err(CapabilityReviewError::MissingReviewer);
        }
        let reason = reason.into();
        if reason.trim().is_empty() {
            return Err(CapabilityReviewError::MissingReason);
        }
        let mut accepted_deltas = diff
            .deltas
            .iter()
            .map(|delta| AcceptedManifestDelta {
                section: delta.section.clone(),
                severity: delta.severity,
                old_fingerprint: delta.old_fingerprint.clone(),
                new_fingerprint: delta.new_fingerprint.clone(),
            })
            .collect::<Vec<_>>();
        accepted_deltas.sort();
        Ok(Self {
            schema_version: CAPABILITY_CHANGE_RECEIPT_SCHEMA_VERSION,
            reviewer,
            reason,
            old_source_identity: old_source_identity.into(),
            new_source_identity: new_source_identity.into(),
            old_manifest_fingerprint: diff.old_manifest_fingerprint.clone(),
            new_manifest_fingerprint: diff.new_manifest_fingerprint.clone(),
            accepted_deltas,
        })
    }

    pub fn accepts(&self, diff: &ManifestDiff) -> bool {
        let mut deltas = diff
            .deltas
            .iter()
            .map(|delta| AcceptedManifestDelta {
                section: delta.section.clone(),
                severity: delta.severity,
                old_fingerprint: delta.old_fingerprint.clone(),
                new_fingerprint: delta.new_fingerprint.clone(),
            })
            .collect::<Vec<_>>();
        deltas.sort();
        self.old_manifest_fingerprint == diff.old_manifest_fingerprint
            && self.new_manifest_fingerprint == diff.new_manifest_fingerprint
            && self.accepted_deltas == deltas
    }

    pub fn fingerprint(&self) -> String {
        format_sha256(&Sha256::digest(self.to_json().as_bytes()))
    }

    pub fn from_json(json: &str) -> Result<Self, CapabilityChangeReceiptParseError> {
        let value = JsonParser::new(json)
            .parse()
            .map_err(capability_receipt_json_error)?;
        let root = value_object(&value, "$")?;
        ensure_fields(
            root,
            "$",
            &[
                "schema_version",
                "reviewer",
                "reason",
                "old_source_identity",
                "new_source_identity",
                "old_manifest_fingerprint",
                "new_manifest_fingerprint",
                "accepted_deltas",
            ],
        )?;

        let schema_version = value_u32(field(root, "schema_version", "$")?, "$.schema_version")?;
        if schema_version != CAPABILITY_CHANGE_RECEIPT_SCHEMA_VERSION {
            return Err(
                CapabilityChangeReceiptParseError::UnsupportedSchemaVersion {
                    found: schema_version,
                    supported: CAPABILITY_CHANGE_RECEIPT_SCHEMA_VERSION,
                },
            );
        }
        let reviewer = required_non_empty_string(field(root, "reviewer", "$")?, "$.reviewer")?;
        let reason = required_non_empty_string(field(root, "reason", "$")?, "$.reason")?;
        let old_source_identity = required_non_empty_string(
            field(root, "old_source_identity", "$")?,
            "$.old_source_identity",
        )?;
        let new_source_identity = required_non_empty_string(
            field(root, "new_source_identity", "$")?,
            "$.new_source_identity",
        )?;
        let old_manifest_fingerprint = required_non_empty_string(
            field(root, "old_manifest_fingerprint", "$")?,
            "$.old_manifest_fingerprint",
        )?;
        let new_manifest_fingerprint = required_non_empty_string(
            field(root, "new_manifest_fingerprint", "$")?,
            "$.new_manifest_fingerprint",
        )?;
        let mut accepted_deltas =
            parse_accepted_deltas(field(root, "accepted_deltas", "$")?, "$.accepted_deltas")?;
        accepted_deltas.sort();

        Ok(Self {
            schema_version,
            reviewer,
            reason,
            old_source_identity,
            new_source_identity,
            old_manifest_fingerprint,
            new_manifest_fingerprint,
            accepted_deltas,
        })
    }

    pub fn read_from_path(
        path: impl AsRef<Path>,
    ) -> Result<Self, CapabilityChangeReceiptPersistenceError> {
        let path = path.as_ref();
        let contents = fs::read_to_string(path).map_err(|error| {
            CapabilityChangeReceiptPersistenceError::Io {
                path: path.to_path_buf(),
                message: error.to_string(),
            }
        })?;
        Self::from_json(&contents).map_err(CapabilityChangeReceiptPersistenceError::Parse)
    }

    pub fn write_to_path(
        &self,
        path: impl AsRef<Path>,
    ) -> Result<(), CapabilityChangeReceiptPersistenceError> {
        let path = path.as_ref();
        let temp_path = temporary_receipt_path(path, self);
        fs::write(&temp_path, self.to_json()).map_err(|error| {
            CapabilityChangeReceiptPersistenceError::Io {
                path: temp_path.clone(),
                message: error.to_string(),
            }
        })?;
        if let Err(error) = fs::rename(&temp_path, path) {
            let _ = fs::remove_file(&temp_path);
            return Err(CapabilityChangeReceiptPersistenceError::Io {
                path: path.to_path_buf(),
                message: error.to_string(),
            });
        }
        Ok(())
    }

    pub fn to_json(&self) -> String {
        let mut receipt = self.clone();
        receipt.accepted_deltas.sort();
        let mut json = String::new();
        json.push_str("{\n");
        push_number_field(&mut json, 1, "schema_version", receipt.schema_version, true);
        push_string_field(&mut json, 1, "reviewer", &receipt.reviewer, true);
        push_string_field(&mut json, 1, "reason", &receipt.reason, true);
        push_string_field(
            &mut json,
            1,
            "old_source_identity",
            &receipt.old_source_identity,
            true,
        );
        push_string_field(
            &mut json,
            1,
            "new_source_identity",
            &receipt.new_source_identity,
            true,
        );
        push_string_field(
            &mut json,
            1,
            "old_manifest_fingerprint",
            &receipt.old_manifest_fingerprint,
            true,
        );
        push_string_field(
            &mut json,
            1,
            "new_manifest_fingerprint",
            &receipt.new_manifest_fingerprint,
            true,
        );
        push_deltas(&mut json, &receipt.accepted_deltas);
        json.push_str("\n}\n");
        json
    }
}

fn capability_receipt_json_error(error: JsonParseError) -> CapabilityChangeReceiptParseError {
    match error {
        JsonParseError::InvalidJson { message } => {
            CapabilityChangeReceiptParseError::InvalidJson { message }
        }
    }
}

fn parse_accepted_deltas(
    value: &JsonValue,
    path: &str,
) -> Result<Vec<AcceptedManifestDelta>, CapabilityChangeReceiptParseError> {
    let values = value_array(value, path)?;
    if values.is_empty() {
        return Err(CapabilityChangeReceiptParseError::InvalidField {
            field: path.to_owned(),
            message: "expected at least one accepted delta".to_owned(),
        });
    }
    let mut deltas = Vec::new();
    let mut seen = BTreeSet::new();
    for (index, value) in values.iter().enumerate() {
        let delta = parse_accepted_delta(value, &format!("{path}[{index}]"))?;
        if !seen.insert(delta.clone()) {
            return Err(CapabilityChangeReceiptParseError::InvalidField {
                field: format!("{path}[{index}]"),
                message: "duplicate accepted delta".to_owned(),
            });
        }
        deltas.push(delta);
    }
    Ok(deltas)
}

fn parse_accepted_delta(
    value: &JsonValue,
    path: &str,
) -> Result<AcceptedManifestDelta, CapabilityChangeReceiptParseError> {
    let fields = value_object(value, path)?;
    ensure_fields(
        fields,
        path,
        &["section", "severity", "old_fingerprint", "new_fingerprint"],
    )?;
    Ok(AcceptedManifestDelta {
        section: required_non_empty_string(
            field(fields, "section", path)?,
            &format!("{path}.section"),
        )?,
        severity: parse_manifest_severity(
            field(fields, "severity", path)?,
            &format!("{path}.severity"),
        )?,
        old_fingerprint: required_non_empty_string(
            field(fields, "old_fingerprint", path)?,
            &format!("{path}.old_fingerprint"),
        )?,
        new_fingerprint: required_non_empty_string(
            field(fields, "new_fingerprint", path)?,
            &format!("{path}.new_fingerprint"),
        )?,
    })
}

fn parse_manifest_severity(
    value: &JsonValue,
    path: &str,
) -> Result<ManifestSeverity, CapabilityChangeReceiptParseError> {
    match value_string(value, path)? {
        "low" => Ok(ManifestSeverity::Low),
        "medium" => Ok(ManifestSeverity::Medium),
        "high" => Ok(ManifestSeverity::High),
        "critical" => Ok(ManifestSeverity::Critical),
        severity => Err(CapabilityChangeReceiptParseError::InvalidField {
            field: path.to_owned(),
            message: format!("unsupported manifest severity `{severity}`"),
        }),
    }
}

fn ensure_fields(
    fields: &[(String, JsonValue)],
    path: &str,
    allowed: &[&str],
) -> Result<(), CapabilityChangeReceiptParseError> {
    let actual = fields
        .iter()
        .map(|(name, _)| name.as_str())
        .collect::<BTreeSet<_>>();
    let expected = allowed.iter().copied().collect::<BTreeSet<_>>();
    for name in actual.difference(&expected) {
        return Err(CapabilityChangeReceiptParseError::UnexpectedField {
            field: format!("{path}.{name}"),
        });
    }
    for name in expected.difference(&actual) {
        return Err(CapabilityChangeReceiptParseError::MissingField {
            field: format!("{path}.{name}"),
        });
    }
    Ok(())
}

fn field<'a>(
    fields: &'a [(String, JsonValue)],
    name: &str,
    path: &str,
) -> Result<&'a JsonValue, CapabilityChangeReceiptParseError> {
    fields
        .iter()
        .find(|(field_name, _)| field_name == name)
        .map(|(_, value)| value)
        .ok_or_else(|| CapabilityChangeReceiptParseError::MissingField {
            field: format!("{path}.{name}"),
        })
}

fn value_object<'a>(
    value: &'a JsonValue,
    path: &str,
) -> Result<&'a [(String, JsonValue)], CapabilityChangeReceiptParseError> {
    value
        .as_object()
        .ok_or_else(|| CapabilityChangeReceiptParseError::InvalidField {
            field: path.to_owned(),
            message: "expected JSON object".to_owned(),
        })
}

fn value_array<'a>(
    value: &'a JsonValue,
    path: &str,
) -> Result<&'a [JsonValue], CapabilityChangeReceiptParseError> {
    value
        .as_array()
        .ok_or_else(|| CapabilityChangeReceiptParseError::InvalidField {
            field: path.to_owned(),
            message: "expected JSON array".to_owned(),
        })
}

fn value_string<'a>(
    value: &'a JsonValue,
    path: &str,
) -> Result<&'a str, CapabilityChangeReceiptParseError> {
    match value {
        JsonValue::String(value) => Ok(value),
        _ => Err(CapabilityChangeReceiptParseError::InvalidField {
            field: path.to_owned(),
            message: "expected JSON string".to_owned(),
        }),
    }
}

fn required_non_empty_string(
    value: &JsonValue,
    path: &str,
) -> Result<String, CapabilityChangeReceiptParseError> {
    let value = value_string(value, path)?;
    if value.trim().is_empty() {
        return Err(CapabilityChangeReceiptParseError::InvalidField {
            field: path.to_owned(),
            message: "expected non-empty string".to_owned(),
        });
    }
    Ok(value.to_owned())
}

fn value_u32(value: &JsonValue, path: &str) -> Result<u32, CapabilityChangeReceiptParseError> {
    match value {
        JsonValue::Number(value) => {
            u32::try_from(*value).map_err(|error| CapabilityChangeReceiptParseError::InvalidField {
                field: path.to_owned(),
                message: error.to_string(),
            })
        }
        _ => Err(CapabilityChangeReceiptParseError::InvalidField {
            field: path.to_owned(),
            message: "expected JSON integer".to_owned(),
        }),
    }
}

fn temporary_receipt_path(path: &Path, receipt: &CapabilityChangeReceipt) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("capability-change.receipt.json");
    let temp_name = format!(
        ".{file_name}.tmp-{}-{}",
        std::process::id(),
        receipt.fingerprint()
    );
    path.with_file_name(temp_name)
}

fn push_deltas(json: &mut String, deltas: &[AcceptedManifestDelta]) {
    push_indent(json, 1);
    push_json_string(json, "accepted_deltas");
    json.push_str(": [");
    for (index, delta) in deltas.iter().enumerate() {
        if index == 0 {
            json.push('\n');
        } else {
            json.push_str(",\n");
        }
        push_indent(json, 2);
        json.push('{');
        push_inline_string_field(json, "section", &delta.section, true);
        push_inline_string_field(json, "severity", delta.severity.as_str(), true);
        push_inline_string_field(json, "old_fingerprint", &delta.old_fingerprint, true);
        push_inline_string_field(json, "new_fingerprint", &delta.new_fingerprint, false);
        json.push('}');
    }
    if !deltas.is_empty() {
        json.push('\n');
        push_indent(json, 1);
    }
    json.push(']');
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

fn push_inline_string_field(json: &mut String, name: &str, value: &str, comma: bool) {
    push_json_string(json, name);
    json.push_str(": ");
    push_json_string(json, value);
    if comma {
        json.push_str(", ");
    }
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
    use crate::diff::diff_package_capability_manifests;
    use crate::manifest::{PackageCapabilityManifest, PackageName, SourceIdentity};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "omega-package-review-{name}-{}-{stamp}",
            std::process::id()
        ))
    }

    fn manifest(package: &str, reach: &[&str]) -> PackageCapabilityManifest {
        let mut manifest = PackageCapabilityManifest::new(
            PackageName::parse(package).unwrap(),
            SourceIdentity {
                kind: "git".to_owned(),
                locator: format!("https://github.com/CathedralOS/{package}"),
                resolved: "commit:old".to_owned(),
            },
        );
        manifest.exported_service_reach = reach.iter().map(|value| (*value).to_owned()).collect();
        manifest
    }

    #[test]
    fn receipt_records_exact_diff_fingerprints() {
        let old = manifest("file-journal", &[]);
        let new = manifest("file-journal", &["FilesystemHost"]);
        let diff = diff_package_capability_manifests(&old, &new);

        let receipt = CapabilityChangeReceipt::from_diff(
            &diff,
            "commit:old",
            "commit:new",
            "reviewer@example.invalid",
            "audited filesystem path handling",
        )
        .expect("receipt");

        assert!(receipt.accepts(&diff));
        assert_eq!(receipt.accepted_deltas.len(), 1);
        assert!(receipt.to_json().contains("\"severity\": \"high\""));
        assert_eq!(receipt.fingerprint().len(), 64);
    }

    #[test]
    fn receipt_json_parse_round_trip_is_normalized() {
        let old = manifest("file-journal", &[]);
        let new = manifest("file-journal", &["FilesystemHost"]);
        let diff = diff_package_capability_manifests(&old, &new);
        let receipt = CapabilityChangeReceipt::from_diff(
            &diff,
            "commit:old",
            "commit:new",
            "reviewer@example.invalid",
            "audited filesystem path handling",
        )
        .expect("receipt");

        let parsed =
            CapabilityChangeReceipt::from_json(&receipt.to_json()).expect("parse receipt JSON");

        assert_eq!(parsed, receipt);
        assert_eq!(parsed.to_json(), receipt.to_json());
        assert!(parsed.accepts(&diff));
    }

    #[test]
    fn receipt_read_write_round_trip_uses_strict_parser() {
        let old = manifest("file-journal", &[]);
        let new = manifest("file-journal", &["FilesystemHost"]);
        let diff = diff_package_capability_manifests(&old, &new);
        let receipt = CapabilityChangeReceipt::from_diff(
            &diff,
            "commit:old",
            "commit:new",
            "reviewer@example.invalid",
            "audited filesystem path handling",
        )
        .expect("receipt");
        let root = temp_root("read-write");
        let path = root.join("capability-change.receipt.json");
        std::fs::create_dir_all(&root).expect("create receipt dir");

        receipt.write_to_path(&path).expect("write receipt");
        let read = CapabilityChangeReceipt::read_from_path(&path).expect("read receipt");

        assert_eq!(read.to_json(), receipt.to_json());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn receipt_parse_rejects_malformed_or_untrusted_fields() {
        assert!(matches!(
            CapabilityChangeReceipt::from_json("{"),
            Err(CapabilityChangeReceiptParseError::InvalidJson { .. })
        ));
        assert_eq!(
            CapabilityChangeReceipt::from_json(
                r#"{
  "schema_version": 99,
  "reviewer": "reviewer@example.invalid",
  "reason": "reason",
  "old_source_identity": "old",
  "new_source_identity": "new",
  "old_manifest_fingerprint": "old-manifest",
  "new_manifest_fingerprint": "new-manifest",
  "accepted_deltas": [
    {"section": "source", "severity": "low", "old_fingerprint": "old", "new_fingerprint": "new"}
  ]
}
"#
            ),
            Err(
                CapabilityChangeReceiptParseError::UnsupportedSchemaVersion {
                    found: 99,
                    supported: CAPABILITY_CHANGE_RECEIPT_SCHEMA_VERSION,
                }
            )
        );
        assert_eq!(
            CapabilityChangeReceipt::from_json(
                r#"{
  "schema_version": 1,
  "reviewer": "reviewer@example.invalid",
  "reason": "reason",
  "old_source_identity": "old",
  "new_source_identity": "new",
  "old_manifest_fingerprint": "old-manifest",
  "new_manifest_fingerprint": "new-manifest",
  "accepted_deltas": []
}
"#
            ),
            Err(CapabilityChangeReceiptParseError::InvalidField {
                field: "$.accepted_deltas".to_owned(),
                message: "expected at least one accepted delta".to_owned(),
            })
        );
    }

    #[test]
    fn receipt_rejects_empty_diff_and_empty_review_fields() {
        let old = manifest("arithmetic-kernels", &[]);
        let new = manifest("arithmetic-kernels", &[]);
        let diff = diff_package_capability_manifests(&old, &new);

        assert_eq!(
            CapabilityChangeReceipt::from_diff(&diff, "a", "b", "reviewer", "reason"),
            Err(CapabilityReviewError::EmptyDiff)
        );

        let changed = diff_package_capability_manifests(
            &manifest("file-journal", &[]),
            &manifest("file-journal", &["FilesystemHost"]),
        );
        assert_eq!(
            CapabilityChangeReceipt::from_diff(&changed, "a", "b", "", "reason"),
            Err(CapabilityReviewError::MissingReviewer)
        );
        assert_eq!(
            CapabilityChangeReceipt::from_diff(&changed, "a", "b", "reviewer", ""),
            Err(CapabilityReviewError::MissingReason)
        );
    }
}

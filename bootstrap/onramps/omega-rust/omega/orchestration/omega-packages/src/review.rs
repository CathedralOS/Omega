use crate::diff::{ManifestDiff, ManifestSeverity};
use sha2::{Digest, Sha256};

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

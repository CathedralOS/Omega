#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageSourceInspection {
    pub source_kind: String,
    pub locator: String,
    pub transport_profile: Option<String>,
    pub requested_rev: Option<String>,
    pub resolved_commit: Option<String>,
    pub resolved_tree: Option<String>,
    pub network_transfer_ceiling: Option<u64>,
    pub network_uploaded_bytes: Option<u64>,
    pub network_downloaded_bytes: Option<u64>,
    pub content_identity: String,
    pub file_count: usize,
    pub byte_count: u64,
}

impl PackageSourceInspection {
    pub fn to_text(&self) -> String {
        let mut report = String::new();
        report.push_str("package source inspection\n");
        report.push_str("source kind: ");
        report.push_str(&self.source_kind);
        report.push('\n');
        report.push_str("locator: ");
        report.push_str(&self.locator);
        report.push('\n');
        if let Some(transport_profile) = &self.transport_profile {
            report.push_str("transport profile: ");
            report.push_str(transport_profile);
            report.push('\n');
        }
        if let Some(rev) = &self.requested_rev {
            report.push_str("requested rev: ");
            report.push_str(rev);
            report.push('\n');
        }
        if let Some(commit) = &self.resolved_commit {
            report.push_str("resolved commit: ");
            report.push_str(commit);
            report.push('\n');
        }
        if let Some(tree) = &self.resolved_tree {
            report.push_str("resolved tree: ");
            report.push_str(tree);
            report.push('\n');
        }
        if let Some(ceiling) = self.network_transfer_ceiling {
            report.push_str("broker transfer ceiling: ");
            report.push_str(&ceiling.to_string());
            report.push('\n');
        }
        if let Some(uploaded) = self.network_uploaded_bytes {
            report.push_str("broker uploaded bytes: ");
            report.push_str(&uploaded.to_string());
            report.push('\n');
        }
        if let Some(downloaded) = self.network_downloaded_bytes {
            report.push_str("broker downloaded bytes: ");
            report.push_str(&downloaded.to_string());
            report.push('\n');
        }
        report.push_str("content identity: ");
        report.push_str(&self.content_identity);
        report.push('\n');
        report.push_str("files: ");
        report.push_str(&self.file_count.to_string());
        report.push('\n');
        report.push_str("bytes: ");
        report.push_str(&self.byte_count.to_string());
        report.push('\n');
        report
    }
}

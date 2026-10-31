#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageSourceInspection {
    pub source_kind: String,
    pub locator: String,
    pub transport_profile: Option<String>,
    pub requested_rev: Option<String>,
    pub resolved_commit: Option<String>,
    pub resolved_tree: Option<String>,
    pub content_identity: String,
    pub file_count: usize,
    pub byte_count: u64,
}

impl PackageSourceInspection {
    pub fn to_text(&self) -> String {
        let mut report = String::new();
        report.push_str("package source inspection\n");
        let mut row = |label: &str, value: Option<&str>| {
            if let Some(value) = value {
                report.push_str(label);
                report.push_str(value);
                report.push('\n');
            }
        };
        row("source kind: ", Some(self.source_kind.as_str()));
        row("locator: ", Some(self.locator.as_str()));
        row("transport profile: ", self.transport_profile.as_deref());
        row("requested rev: ", self.requested_rev.as_deref());
        row("resolved commit: ", self.resolved_commit.as_deref());
        row("resolved tree: ", self.resolved_tree.as_deref());
        row("content identity: ", Some(self.content_identity.as_str()));
        row("files: ", Some(self.file_count.to_string().as_str()));
        row("bytes: ", Some(self.byte_count.to_string().as_str()));
        report
    }
}

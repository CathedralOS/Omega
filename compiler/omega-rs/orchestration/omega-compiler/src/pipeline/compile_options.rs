use std::path::PathBuf;

/// Controls auxiliary compiler reports independently from executable/object
/// installation. Semantic validation, trust-lock enforcement, and requested
/// output installation run under both policies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtifactEmissionPolicy {
    Full,
    OutputOnly,
}

impl ArtifactEmissionPolicy {
    pub const fn emits_auxiliary_artifacts(self) -> bool {
        matches!(self, Self::Full)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileOptions {
    pub root_path: PathBuf,
    pub build_dir: Option<PathBuf>,
    pub target_name: Option<String>,
    pub write_output: bool,
}

impl CompileOptions {
    pub fn build_dir(&self) -> PathBuf {
        self.build_dir.clone().unwrap_or_else(|| {
            self.root_path
                .parent()
                .map(|parent| parent.join("build"))
                .unwrap_or_else(|| PathBuf::from("build"))
        })
    }
}

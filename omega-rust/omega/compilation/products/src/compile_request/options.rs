use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileOptions {
    pub root_path: PathBuf,
    pub build_dir: Option<PathBuf>,
    pub target_name: Option<String>,
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

    /// Resolve and retain the artifact root before a caller replaces the
    /// source entrypoint with an immutable compilation-custody path.
    ///
    /// An explicit build directory is preserved byte-for-byte. Otherwise the
    /// default remains beside the entrypoint that the user supplied.
    pub fn retain_build_dir(&mut self) -> PathBuf {
        let build_dir = self.build_dir();
        self.build_dir = Some(build_dir.clone());
        build_dir
    }
}

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

#[cfg(test)]
mod tests {
    use super::CompileOptions;
    use std::path::{Path, PathBuf};

    #[test]
    fn retained_default_build_dir_survives_compilation_root_retargeting() {
        let mut options = CompileOptions {
            root_path: PathBuf::from("authored/project/main.omg"),
            build_dir: None,
            target_name: None,
        };

        let retained = options.retain_build_dir();
        options.root_path = PathBuf::from("resolver/snapshots/source/source/main.omg");

        assert_eq!(retained, Path::new("authored/project/build"));
        assert_eq!(options.build_dir(), retained);
    }

    #[test]
    fn retained_explicit_build_dir_remains_exact() {
        let explicit = PathBuf::from("../exact-artifacts");
        let mut options = CompileOptions {
            root_path: PathBuf::from("authored/project/main.omg"),
            build_dir: Some(explicit.clone()),
            target_name: None,
        };

        let retained = options.retain_build_dir();
        options.root_path = PathBuf::from("resolver/snapshots/source/source/main.omg");

        assert_eq!(retained, explicit);
        assert_eq!(options.build_dir, Some(explicit));
    }
}

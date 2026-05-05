use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileOptions {
    pub root_path: PathBuf,
}

use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileReport {
    pub root_path: PathBuf,
    pub source_file_count: usize,
    pub wrote_output: bool,
}

impl CompileReport {
    pub fn summary(&self) -> String {
        format!(
            "compiled {} source file(s) from {}; write_output={}",
            self.source_file_count,
            self.root_path.display(),
            self.wrote_output
        )
    }
}

//! Invoker requests. A type spelling is a selector in the reviewed source
//! graph; it becomes a grant only after an exact declaration is resolved.
use std::{collections::BTreeMap, path::PathBuf, sync::Arc, time::Duration};

#[derive(Clone, Debug, Default)]
pub struct BuildProgramInvocation {
    pub allow: bool,
    /// When a reviewed graph names the root, its name qualifies root-file selectors.
    pub reviewed_root_name: Option<String>,
    pub worker: Option<PathBuf>,
    pub bindings: Vec<BuildProgramBinding>,
    /// Sealed images explicitly forwarded by the Build that registered this occurrence.
    pub inherited: Vec<DelegatedProgram>,
}

/// A forwarded grant retains the proved source occurrence and executable bytes.
#[derive(Clone, Debug)]
pub struct DelegatedProgram {
    pub declaration: source_admission::SourceFile,
    pub declaration_span: source_text::Span,
    pub image: build_program_launch::ReviewedProgram,
    pub meaning: build_filesystem::consent::RestrictedProgramLaunch,
    pub files: Option<Arc<ForwardedProgramFiles>>,
}

/// The activation's private staging service. A forwarded grant exposes only
/// the paths retained in ProgramOutputBacking.
pub trait ProgramOutputSink: Send + Sync {
    fn require_live(&self) -> Result<(), String>;
    fn write(&self, paths: &[Vec<u8>], files: Vec<Option<Vec<u8>>>) -> Result<(), String>;
}

pub struct ProgramOutputBacking {
    sink: Arc<dyn ProgramOutputSink>,
    paths: Vec<Vec<u8>>,
}

impl ProgramOutputBacking {
    pub fn new(sink: Arc<dyn ProgramOutputSink>, paths: Vec<Vec<u8>>) -> Self {
        Self { sink, paths }
    }

    pub fn require_live(&self) -> Result<(), String> {
        self.sink.require_live()
    }

    pub fn write(&self, files: Vec<Option<Vec<u8>>>) -> Result<(), String> {
        self.sink.write(&self.paths, files)
    }
}

pub struct ForwardedProgramFiles {
    pub initial: BTreeMap<Vec<u8>, Arc<[u8]>>,
    pub output: Option<ProgramOutputBacking>,
    pub storage: build_model::accounting::EvaluationStorageLease,
}

impl std::fmt::Debug for ForwardedProgramFiles {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ForwardedProgramFiles")
            .field("inputs", &self.initial.keys())
            .field("has_output", &self.output.is_some())
            .finish()
    }
}

impl ForwardedProgramFiles {
    fn same_view(&self, other: &Self) -> bool {
        self.initial == other.initial
            && match (&self.output, &other.output) {
                (Some(left), Some(right)) => {
                    left.paths == right.paths && Arc::ptr_eq(&left.sink, &right.sink)
                }
                (None, None) => true,
                _ => false,
            }
    }
}

impl DelegatedProgram {
    pub fn is_supplied_by(&self, available: &[Self]) -> bool {
        available.iter().any(|grant| {
            grant.meaning == self.meaning
                && grant.names(&self.declaration, self.declaration_span)
                && match (&self.files, &grant.files) {
                    (Some(left), Some(right)) => left.same_view(right),
                    (None, None) => true,
                    _ => false,
                }
        })
    }

    pub fn names(&self, file: &source_admission::SourceFile, span: source_text::Span) -> bool {
        self.declaration_span == span && file.continues_capture_of(&self.declaration)
    }
}

#[derive(Clone, Debug)]
pub struct BuildProgramBinding {
    pub selector: String,
    pub executable: PathBuf,
    pub read: Vec<Vec<u8>>,
    pub write: Vec<Vec<u8>>,
    pub timeout: Option<Duration>,
}

impl BuildProgramBinding {
    pub fn new(selector: String, executable: PathBuf) -> Result<Self, String> {
        if selector.split("::").any(str::is_empty) {
            return Err(format!(
                "invalid qualified program declaration `{selector}`"
            ));
        }
        Ok(Self {
            selector,
            executable,
            read: Vec::new(),
            write: Vec::new(),
            timeout: None,
        })
    }

    pub fn add_path(&mut self, value: Vec<u8>, write: bool) -> Result<(), String> {
        if !build_filesystem::authority::canonical_filesystem_metadata_path_is_canonical(
            &value, false,
        ) {
            return Err("program resource requires a canonical relative logical file path".into());
        }
        let paths = if write {
            &mut self.write
        } else {
            &mut self.read
        };
        if paths.contains(&value) {
            return Err(format!(
                "duplicate program resource `{}`",
                String::from_utf8_lossy(&value)
            ));
        }
        paths.push(value);
        Ok(())
    }
}

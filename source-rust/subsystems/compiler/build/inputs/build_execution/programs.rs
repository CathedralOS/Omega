//! Invoker requests. A type spelling is a selector in the reviewed source
//! graph; it becomes a grant only after an exact declaration is resolved.
use std::{path::PathBuf, time::Duration};

#[derive(Clone, Debug, Default)]
pub struct BuildProgramInvocation {
    pub allow: bool,
    /// When a reviewed graph names the root, its name qualifies root-file selectors.
    pub reviewed_root_name: Option<String>,
    pub worker: Option<PathBuf>,
    pub bindings: Vec<BuildProgramBinding>,
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
        if !filesystem_authority::canonical_filesystem_metadata_path_is_canonical(&value, false) {
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

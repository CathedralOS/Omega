//! Stage the declared inputs before starting a cold compiler observation.
//! Every sample keeps its project layout, user cache and temporary files in
//! its own directory. The journal still identifies the original input bytes.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

pub(crate) struct Client<'a> {
    root: &'a Path,
    project: PathBuf,
}

impl<'a> Client<'a> {
    pub(crate) fn new(root: &'a Path, sample: &Path) -> Self {
        Self {
            root,
            project: sample.join("client").join("project"),
        }
    }

    pub(crate) fn path(&self, original: &Path) -> Result<PathBuf, String> {
        let relative = original
            .strip_prefix(self.root)
            .map_err(|_| format!("{} lies outside cold_client_root", original.display()))?;
        Ok(self.project.join(relative))
    }
}

pub(crate) fn prepare<'a>(
    root: &'a Path,
    inputs: &[PathBuf],
    sample: &Path,
    environment: &mut BTreeMap<String, String>,
) -> Result<Client<'a>, String> {
    if inputs.iter().any(|input| sample.starts_with(input)) {
        return Err("cold sample output lies inside a declared input".into());
    }
    let base = sample.join("client");
    fs::create_dir(&base).map_err(|e| format!("create fresh cold client: {e}"))?;
    let client = Client::new(root, sample);
    fs::create_dir(&client.project).map_err(|e| e.to_string())?;
    for input in inputs {
        let destination = client.path(input)?;
        copy_input(input, &destination)?;
    }
    for (subdirectory, variables) in [
        ("cache", &["LOCALAPPDATA", "XDG_CACHE_HOME"][..]),
        ("home", &["HOME"][..]),
        ("temp", &["TMP", "TEMP", "TMPDIR"][..]),
    ] {
        let path = base.join(subdirectory);
        fs::create_dir(&path).map_err(|e| e.to_string())?;
        for &variable in variables {
            environment.insert(variable.into(), path.display().to_string());
        }
    }
    Ok(client)
}

fn copy_input(input: &Path, destination: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(input).map_err(|e| e.to_string())?;
    if metadata.is_dir() {
        fs::create_dir_all(destination).map_err(|e| e.to_string())?;
        for entry in fs::read_dir(input).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            copy_input(&entry.path(), &destination.join(entry.file_name()))?;
        }
    } else if metadata.is_file() {
        fs::create_dir_all(destination.parent().ok_or("missing cold input parent")?)
            .map_err(|e| e.to_string())?;
        fs::copy(input, destination).map_err(|e| format!("copy {}: {e}", input.display()))?;
    } else {
        return Err(format!(
            "cold input is not a regular file or directory: {}",
            input.display()
        ));
    }
    Ok(())
}

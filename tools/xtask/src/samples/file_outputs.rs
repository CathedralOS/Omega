//! Sample-owned file oracles. All names are relative to the sample or the guest's
//! private working directory. Observing an output grants no paths to an engine.

#[cfg(test)]
use crate::execution;
use std::{
    collections::BTreeMap,
    fs,
    io::{self, BufRead, BufReader},
    path::{Component, Path, PathBuf},
};

pub(crate) type Declarations = BTreeMap<String, Option<String>>;
pub(super) type ObservedFiles = BTreeMap<String, Option<PathBuf>>;

pub(super) fn declarations(
    value: Option<&serde_json::Value>,
    manifest: &Path,
) -> Result<Declarations, String> {
    let Some(value) = value else {
        return Ok(Declarations::new());
    };
    let files: Declarations = serde_json::from_value(value.clone()).map_err(|e| {
        format!(
            "{}: output_files requires a map of output paths to fixture paths or null: {e}",
            manifest.display()
        )
    })?;
    for name in files.keys().chain(files.values().flatten()) {
        if name.is_empty()
            || name.contains(['\\', ':'])
            || name.split('/').any(|part| matches!(part, "" | "." | ".."))
            || Path::new(name)
                .components()
                .any(|part| !matches!(part, Component::Normal(_)))
        {
            return Err(format!(
                "{}: invalid relative output_files path {name:?}",
                manifest.display()
            ));
        }
    }
    Ok(files)
}

fn snapshot(root: &Path, relative: &str, saved: &Path) -> Result<Option<PathBuf>, String> {
    if !fs::symlink_metadata(root)
        .map_err(|e| format!("{}: {e}", root.display()))?
        .is_dir()
    {
        return Err(format!(
            "{} is not an observation directory",
            root.display()
        ));
    }
    let mut path = PathBuf::from(root);
    for part in relative.split('/') {
        path.push(part);
        match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(format!("cannot observe symbolic link {}", path.display()));
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(format!("{}: {error}", path.display())),
        }
    }
    if !fs::metadata(&path)
        .map_err(|e| format!("{}: {e}", path.display()))?
        .is_file()
    {
        return Err(format!("{} is not a regular file", path.display()));
    }
    fs::create_dir_all(
        saved
            .parent()
            .expect("a snapshot under the evidence directory"),
    )
    .map_err(|e| format!("{}: {e}", saved.display()))?;
    let mut source = fs::File::open(&path).map_err(|e| format!("open {}: {e}", path.display()))?;
    let mut snapshot =
        fs::File::create_new(saved).map_err(|e| format!("create {}: {e}", saved.display()))?;
    io::copy(&mut source, &mut snapshot)
        .map_err(|e| format!("snapshot {}: {e}", path.display()))?;
    Ok(Some(saved.to_owned()))
}

pub(super) fn expected(
    directory: &Path,
    declarations: &Declarations,
    evidence: &Path,
) -> Result<ObservedFiles, String> {
    declarations
        .iter()
        .map(|(name, fixture)| {
            let bytes = match fixture {
                Some(path) => Some(
                    snapshot(directory, path, &evidence.join(name))?.ok_or_else(|| {
                        format!("missing expected file fixture {path} for {name}")
                    })?,
                ),
                None => None,
            };
            Ok((name.clone(), bytes))
        })
        .collect()
}

pub(super) fn collect(
    directory: &Path,
    expected: &ObservedFiles,
    evidence: &Path,
) -> Result<ObservedFiles, String> {
    expected
        .keys()
        .map(|name| {
            Ok((
                name.clone(),
                snapshot(directory, name, &evidence.join(name))?,
            ))
        })
        .collect()
}

pub(super) fn compare(left: &ObservedFiles, right: &ObservedFiles) -> Result<(), String> {
    for (name, first) in left {
        match (
            first,
            right
                .get(name)
                .ok_or_else(|| format!("missing observation for {name}"))?,
        ) {
            (Some(a), Some(b)) => {
                if let Some(offset) =
                    first_difference(a, b).map_err(|e| format!("compare {name}: {e}"))?
                {
                    return Err(format!("{name} differs at byte {offset}"));
                }
            }
            (None, None) => {}
            (Some(_), None) => return Err(format!("{name} differs: present vs absent")),
            (None, Some(_)) => return Err(format!("{name} differs: absent vs present")),
        }
    }
    Ok(())
}

fn first_difference(a: &Path, b: &Path) -> io::Result<Option<u64>> {
    let mut left = BufReader::new(fs::File::open(a)?);
    let mut right = BufReader::new(fs::File::open(b)?);
    let mut offset = 0;
    loop {
        let a = left.fill_buf()?;
        let b = right.fill_buf()?;
        let common = a.len().min(b.len());
        if common == 0 {
            return Ok((a.is_empty() != b.is_empty()).then_some(offset));
        }
        if let Some(index) = a.iter().zip(b).position(|(x, y)| x != y) {
            return Ok(Some(offset + index as u64));
        }
        offset += common as u64;
        left.consume(common);
        right.consume(common);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_missing_recorded_file_differs_from_an_empty_file() {
        let root = execution::reserve_directory(&std::env::temp_dir()).unwrap();
        let project = root.join("project");
        let native = root.join("native");
        let psi = root.join("psi");
        for path in [&project, &native, &psi] {
            fs::create_dir(path).unwrap();
        }
        fs::write(project.join("empty.img"), []).unwrap();
        let fields =
            declarations(Some(&json!({"disk.img":"empty.img","lock":null})), &project).unwrap();
        let authored = expected(&project, &fields, &root.join("expected")).unwrap();
        fs::write(native.join("disk.img"), []).unwrap();
        let first = collect(&native, &authored, &root.join("native-files")).unwrap();
        let second = collect(&psi, &authored, &root.join("psi-files")).unwrap();
        fs::write(native.join("disk.img"), [42]).unwrap();
        compare(&authored, &first).unwrap();
        assert!(
            compare(&first, &second)
                .unwrap_err()
                .contains("disk.img differs: present vs absent")
        );
        fs::write(psi.join("disk.img"), [42]).unwrap();
        assert!(
            compare(
                &first,
                &collect(&psi, &authored, &root.join("psi-files")).unwrap()
            )
            .unwrap_err()
            .contains("disk.img differs at byte 0")
        );
        let bytes = vec![0; 20_000];
        let image = native.join("disk.img");
        fs::write(&image, &bytes).unwrap();
        fs::write(psi.join("disk.img"), &bytes[..19_000]).unwrap();
        assert_eq!(
            first_difference(&image, &psi.join("disk.img")).unwrap(),
            Some(19_000)
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn file_oracles_use_paths_owned_by_the_sample() {
        for value in [
            json!({"../escape": null}),
            json!({"/outside": null}),
            json!({"sub\\other": null}),
            json!({"file": "../oracle"}),
            json!(["file"]),
        ] {
            assert!(declarations(Some(&value), Path::new("sample.json")).is_err());
        }
    }
}

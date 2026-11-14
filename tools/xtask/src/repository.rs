//! Select the checkout being operated on, not the checkout that built xtask.

use std::io::ErrorKind;
use std::path::{Path, PathBuf};

pub(crate) fn current() -> Result<PathBuf, String> {
    let directory = std::env::current_dir()
        .map_err(|error| format!("cannot read invocation directory: {error}"))?;
    enclosing(&directory)
}

fn enclosing(directory: &Path) -> Result<PathBuf, String> {
    // Worktrees nest under the primary checkout, so the nearest complete
    // workspace wins. Repository contents identify source archives too; neither
    // a .git directory nor the binary's installation location is required.
    for candidate in directory.ancestors() {
        let mut workspace = true;
        for marker in [
            "Cargo.toml",
            "tools/xtask/Cargo.toml",
            "omega-rust/pipeline.md",
        ] {
            let marker = candidate.join(marker);
            match std::fs::metadata(&marker) {
                Ok(metadata) if metadata.is_file() => {}
                Ok(_) => {
                    workspace = false;
                    break;
                }
                Err(error) if error.kind() == ErrorKind::NotFound => {
                    workspace = false;
                    break;
                }
                Err(error) => {
                    return Err(format!("cannot inspect {}: {error}", marker.display()));
                }
            }
        }
        if workspace {
            return Ok(candidate.to_owned());
        }
        // An incomplete or unrelated nested checkout must not redirect the
        // operation into an enclosing primary checkout.
        let git = candidate.join(".git");
        match std::fs::metadata(&git) {
            Ok(_) => {
                return Err(format!(
                    "Git checkout {} is not an Omega workspace",
                    candidate.display()
                ));
            }
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => return Err(format!("cannot inspect {}: {error}", git.display())),
        }
    }
    Err(format!(
        "no Omega workspace contains {}; run from the intended checkout or use the command's explicit repository option",
        directory.display()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    struct Checkouts(PathBuf);

    impl Checkouts {
        fn new() -> Self {
            let unique = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let directory = std::env::temp_dir()
                .join(format!("omega-xtask-roots-{}-{unique}", std::process::id()));
            fs::create_dir_all(&directory).unwrap();
            Self(directory)
        }

        fn checkout(&self, relative: &str) -> PathBuf {
            let root = self.0.join(relative);
            fs::create_dir_all(root.join("tools/xtask/src")).unwrap();
            fs::create_dir_all(root.join("omega-rust")).unwrap();
            for marker in [
                "Cargo.toml",
                "tools/xtask/Cargo.toml",
                "omega-rust/pipeline.md",
            ] {
                fs::write(root.join(marker), "").unwrap();
            }
            root
        }
    }

    impl Drop for Checkouts {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn root_follows_the_invoked_checkout_instead_of_another_checkout() {
        let checkouts = Checkouts::new();
        let first = checkouts.checkout("first");
        let second = checkouts.checkout("second");
        assert_eq!(enclosing(&first).unwrap(), first);
        assert_eq!(enclosing(&second.join("tools/xtask/src")).unwrap(), second);
    }

    #[test]
    fn nested_worktree_wins_over_its_primary_checkout() {
        let checkouts = Checkouts::new();
        checkouts.checkout("primary");
        let worker = checkouts.checkout("primary/.agents/worktree/worker-1");
        assert_eq!(enclosing(&worker.join("omega-rust")).unwrap(), worker);
    }

    #[test]
    fn outside_or_incomplete_workspace_does_not_fall_back_to_the_build_checkout() {
        let checkouts = Checkouts::new();
        checkouts.checkout("unrelated");
        fs::write(checkouts.0.join("Cargo.toml"), "[workspace]").unwrap();
        let error = enclosing(&checkouts.0).unwrap_err();
        assert!(error.contains("no Omega workspace contains"), "{error}");
        assert!(
            error.contains(&checkouts.0.display().to_string()),
            "{error}"
        );
    }

    #[test]
    fn incomplete_nested_checkout_does_not_escape_to_the_primary() {
        let checkouts = Checkouts::new();
        let primary = checkouts.checkout("primary");
        let nested = primary.join(".agents/worktree/incomplete");
        fs::create_dir_all(nested.join("src")).unwrap();
        fs::write(nested.join(".git"), "gitdir: unused by this resolver").unwrap();
        let error = enclosing(&nested.join("src")).unwrap_err();
        assert!(error.contains("is not an Omega workspace"), "{error}");
        assert!(error.contains(&nested.display().to_string()), "{error}");
    }
}

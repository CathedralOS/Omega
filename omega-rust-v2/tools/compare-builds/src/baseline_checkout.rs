//! A commit checked out beside the working tree for the length of one comparison.

use std::path::{Path, PathBuf};
use std::process::Command;

/// A git worktree holding the baseline's source. Dropping it removes the worktree, so every way
/// out of a comparison, an error included, leaves the repository as it was.
pub struct BaselineCheckout {
    repository_directory: PathBuf,
    checkout_directory: PathBuf,
}

impl BaselineCheckout {
    /// Checks `git_reference` out into `checkout_directory`, replacing anything a killed run left there.
    pub fn create(
        repository_directory: &Path,
        git_reference: &str,
        checkout_directory: &Path,
    ) -> Result<Self, String> {
        Self::remove_worktree(repository_directory, checkout_directory);

        let checkout_output = Command::new("git")
            .current_dir(repository_directory)
            .args(["worktree", "add", "--detach", "--force"])
            .arg(checkout_directory)
            .arg(git_reference)
            .output()
            .map_err(|io_error| format!("Cannot run git: {io_error}."))?;

        if !checkout_output.status.success() {
            return Err(format!(
                "git could not check out `{git_reference}`: {}",
                String::from_utf8_lossy(&checkout_output.stderr).trim()
            ));
        }

        Ok(Self {
            repository_directory: repository_directory.to_path_buf(),
            checkout_directory: checkout_directory.to_path_buf(),
        })
    }

    pub fn get_directory(&self) -> &Path {
        &self.checkout_directory
    }

    // Failure is expected and ignored: on the way in there is usually nothing to remove, and on
    // the way out there is nobody left to tell. The next run starts by trying again.
    fn remove_worktree(
        repository_directory: &Path,
        checkout_directory: &Path,
    ) {
        let _ = Command::new("git")
            .current_dir(repository_directory)
            .args(["worktree", "remove", "--force"])
            .arg(checkout_directory)
            .output();
        let _ = Command::new("git")
            .current_dir(repository_directory)
            .args(["worktree", "prune"])
            .output();
    }
}

impl Drop for BaselineCheckout {
    fn drop(&mut self) {
        Self::remove_worktree(&self.repository_directory, &self.checkout_directory);
    }
}
